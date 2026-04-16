use std::fmt::Debug;
use std::marker::{Send, Sync};
use std::time::{SystemTime, UNIX_EPOCH};

use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    date_time::{format_date, now, DateFormat},
    errors::CustomResult,
    types::MinorUnit,
};
use domain_types::payment_method_data::RawCardNumber;
use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;
use crate::utils;

use super::PayboxRouterData;
use domain_types::router_data::ErrorResponse;

// ============================================================================
// RESPONSE TYPE ALIASES
// ============================================================================
pub type PayboxAuthorizeResponse = PayboxPaymentResponse;
pub type PayboxCaptureResponse = PayboxPaymentResponse;
pub type PayboxVoidResponse = PayboxPaymentResponse;

// ============================================================================
// AUTHENTICATION
// ============================================================================

#[derive(Debug, Clone)]
pub struct PayboxAuthType {
    pub site: Secret<String>,
    pub rank: Secret<String>,
    pub key: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PayboxAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paybox {
                site,
                rank,
                key,
                merchant_id,
                ..
            } => Ok(Self {
                site: site.to_owned(),
                rank: rank.to_owned(),
                key: key.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// ============================================================================
// COMMON ENUMS AND TYPES
// ============================================================================

const VERSION_PAYBOX: &str = "00104";
const AUTH_REQUEST: &str = "00001";
const CAPTURE_REQUEST: &str = "00002";
const AUTH_AND_CAPTURE_REQUEST: &str = "00003";
const CANCEL_REQUEST: &str = "00005";
const REFUND_REQUEST: &str = "00014";
const SYNC_REQUEST: &str = "00017";
const SUCCESS_CODE: &str = "00000";
const PAY_ORIGIN_INTERNET: &str = "024";
const PAY_ORIGIN_RECURRING: &str = "027";
const SUBSCRIBER_AUTH_REQUEST: &str = "00051";
const SUBSCRIBER_AUTH_AND_CAPTURE_REQUEST: &str = "00053";
const REGISTER_SUBSCRIBER_REQUEST: &str = "00056";

#[derive(Debug, Serialize, Deserialize)]
pub struct PayboxMeta {
    pub connector_request_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PayboxStatus {
    #[serde(rename = "Remboursé")]
    Refunded,
    #[serde(rename = "Annulé")]
    Cancelled,
    #[serde(rename = "Autorisé")]
    Authorised,
    #[serde(rename = "Capturé")]
    Captured,
    #[serde(rename = "Refusé")]
    Rejected,
}

impl From<PayboxStatus> for AttemptStatus {
    fn from(item: PayboxStatus) -> Self {
        match item {
            PayboxStatus::Cancelled => Self::Voided,
            PayboxStatus::Authorised => Self::Authorized,
            PayboxStatus::Captured => Self::Charged,
            PayboxStatus::Rejected => Self::Failure,
            PayboxStatus::Refunded => Self::AutoRefunded,
        }
    }
}

impl From<PayboxStatus> for RefundStatus {
    fn from(item: PayboxStatus) -> Self {
        match item {
            PayboxStatus::Refunded => Self::Success,
            PayboxStatus::Cancelled
            | PayboxStatus::Authorised
            | PayboxStatus::Captured
            | PayboxStatus::Rejected => Self::Failure,
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn get_transaction_type(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<&'static str, Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::Automatic) => Ok(AUTH_AND_CAPTURE_REQUEST),
        Some(common_enums::CaptureMethod::Manual) | None => Ok(AUTH_REQUEST),
        _ => Err(IntegrationError::CaptureMethodNotSupported {
            context: Default::default(),
        })?,
    }
}

fn generate_request_id() -> CustomResult<String, IntegrationError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?
        .as_millis()
        .to_string();

    timestamp.get(4..).map(|s| s.to_string()).ok_or_else(|| {
        Report::new(IntegrationError::InvalidDataFormat {
            field_name: "request_id",
            context: Default::default(),
        })
    })
}

fn generate_date_time() -> CustomResult<String, IntegrationError> {
    format_date(now(), DateFormat::DDMMYYYYHHmmss).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )
}

// ============================================================================
// AUTHORIZE FLOW
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PayboxPaymentRequest<T: PaymentMethodDataTypes> {
    pub version: String,
    #[serde(rename = "TYPE")]
    pub transaction_type: String,
    pub site: Secret<String>,
    #[serde(rename = "RANG")]
    pub rank: Secret<String>,
    #[serde(rename = "CLE")]
    pub key: Secret<String>,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: String,
    #[serde(rename = "MONTANT")]
    pub amount: MinorUnit,
    #[serde(rename = "DEVISE")]
    pub currency: String,
    pub reference: String,
    #[serde(rename = "DATEQ")]
    pub date: String,
    #[serde(rename = "PORTEUR")]
    pub card_number: RawCardNumber<T>,
    #[serde(rename = "DATEVAL")]
    pub expiration_date: Secret<String>,
    pub cvv: Secret<String>,
    #[serde(rename = "ACTIVITE")]
    pub activity: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayboxPaymentRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let card_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(req_card) => req_card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Only card payments are supported".to_string(),
                    connector: "Paybox",
                    context: Default::default(),
                }
                .into())
            }
        };

        let expiration_date = Secret::new(
            card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_owned())?
                .peek()
                .to_string(),
        );
        let transaction_type = get_transaction_type(router_data.request.capture_method)?;

        Ok(Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: transaction_type.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            amount,
            currency: router_data.request.currency.iso_4217().to_string(),
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            date: generate_date_time()?,
            card_number: card_data.card_number.clone(),
            expiration_date,
            cvv: card_data.card_cvc.clone(),
            activity: PAY_ORIGIN_INTERNET.to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PayboxPaymentResponse {
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: Option<String>,
    pub site: Option<String>,
    #[serde(rename = "RANG")]
    pub rank: Option<String>,
    #[serde(rename = "AUTORISATION")]
    pub authorization: Option<String>,
    #[serde(rename = "CODEREPONSE")]
    pub response_code: String,
    #[serde(rename = "COMMENTAIRE")]
    pub response_message: String,
    #[serde(rename = "PORTEUR")]
    pub carrier_id: Option<Secret<String>>,
    #[serde(rename = "REFABONNE")]
    pub customer_id: Option<Secret<String>>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PayboxAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayboxAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
        );

        if item.response.response_code == SUCCESS_CODE {
            let status = if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            };

            // Create connector_metadata with NUMTRANS
            let connector_metadata = serde_json::json!(PayboxMeta {
                connector_request_id: item.response.transaction_number.clone()
            });

            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.paybox_order_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: Some(connector_metadata),
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    reference_id: Some(item.response.transaction_number.clone()), // Store NUMTRANS in reference_id
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(item.response.transaction_number.clone()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// PSYNC FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct PayboxSyncRequest {
    #[serde(rename = "VERSION")]
    pub version: String,
    #[serde(rename = "TYPE")]
    pub transaction_type: String,
    #[serde(rename = "SITE")]
    pub site: Secret<String>,
    #[serde(rename = "RANG")]
    pub rank: Secret<String>,
    #[serde(rename = "CLE")]
    pub key: Secret<String>,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: String,
    #[serde(rename = "DATEQ")]
    pub date: String,
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PayboxSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let numappel = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into())
            }
        };

        // Try reading from multiple sources in order of preference
        let numtrans = router_data
            .request
            .connector_feature_data
            .as_ref()
            .and_then(|meta| utils::to_connector_meta_from_secret(Some(meta.clone())).ok())
            .map(|meta: PayboxMeta| meta.connector_request_id)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_request_id (NUMTRANS)",
                context: Default::default(),
            })?;

        Ok(Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: SYNC_REQUEST.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            date: generate_date_time()?,
            transaction_number: numtrans,
            paybox_order_id: numappel,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PayboxPSyncResponse {
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: Option<String>,
    pub site: Option<String>,
    #[serde(rename = "RANG")]
    pub rank: Option<String>,
    #[serde(rename = "AUTORISATION")]
    pub authorization: Option<String>,
    #[serde(rename = "CODEREPONSE")]
    pub response_code: String,
    #[serde(rename = "COMMENTAIRE")]
    pub response_message: String,
    pub status: PayboxStatus,
}

impl TryFrom<ResponseRouterData<PayboxPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PayboxPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let connector_payment_status = item.response.status;
        let status = AttemptStatus::from(connector_payment_status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.paybox_order_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// CAPTURE FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct PayboxCaptureRequest {
    #[serde(rename = "VERSION")]
    pub version: String,
    #[serde(rename = "TYPE")]
    pub transaction_type: String,
    #[serde(rename = "SITE")]
    pub site: Secret<String>,
    #[serde(rename = "RANG")]
    pub rank: Secret<String>,
    #[serde(rename = "CLE")]
    pub key: Secret<String>,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: String,
    #[serde(rename = "MONTANT")]
    pub amount: MinorUnit,
    #[serde(rename = "DEVISE")]
    pub currency: String,
    #[serde(rename = "REFERENCE")]
    pub reference: String,
    #[serde(rename = "DATEQ")]
    pub date: String,
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PayboxCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;
        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let numappel = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into())
            }
        };

        // Try reading from multiple sources in order of preference
        let numtrans = router_data
            .request
            .connector_feature_data
            .as_ref()
            .and_then(|meta| serde_json::from_value::<PayboxMeta>(meta.peek().clone()).ok())
            .map(|meta| meta.connector_request_id)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_request_id (NUMTRANS)",
                context: Default::default(),
            })?;

        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let capture_request = Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: CAPTURE_REQUEST.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            amount,
            currency: router_data.request.currency.iso_4217().to_string(),
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            date: generate_date_time()?,
            transaction_number: numtrans,
            paybox_order_id: numappel,
        };

        Ok(capture_request)
    }
}

impl TryFrom<ResponseRouterData<PayboxCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayboxCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.response_code == SUCCESS_CODE {
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.paybox_order_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Charged,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(item.response.transaction_number.clone()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// VOID FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct PayboxVoidRequest {
    #[serde(rename = "VERSION")]
    pub version: String,
    #[serde(rename = "TYPE")]
    pub transaction_type: String,
    #[serde(rename = "SITE")]
    pub site: Secret<String>,
    #[serde(rename = "RANG")]
    pub rank: Secret<String>,
    #[serde(rename = "CLE")]
    pub key: Secret<String>,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: String,
    #[serde(rename = "MONTANT")]
    pub amount: MinorUnit,
    #[serde(rename = "DEVISE")]
    pub currency: String,
    #[serde(rename = "REFERENCE")]
    pub reference: String,
    #[serde(rename = "DATEQ")]
    pub date: String,
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PayboxVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;
        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let numappel = router_data.request.connector_transaction_id.clone();

        // Try to get NUMTRANS from stored metadata, fallback to NUMAPPEL if not available
        // Note: connector_metadata in request may contain merchant custom data
        let numtrans = router_data
            .request
            .connector_feature_data
            .clone()
            .and_then(|meta| utils::to_connector_meta_from_secret::<PayboxMeta>(Some(meta)).ok())
            .map(|meta| meta.connector_request_id)
            .unwrap_or_else(|| numappel.clone());

        let amount = router_data
            .request
            .amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: Default::default(),
            })?;

        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;

        let amount = connector
            .amount_converter
            .convert(amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: CANCEL_REQUEST.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            amount,
            currency: currency.iso_4217().to_string(),
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            date: generate_date_time()?,
            transaction_number: numtrans,
            paybox_order_id: numappel,
        })
    }
}

impl TryFrom<ResponseRouterData<PayboxVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PayboxVoidResponse, Self>) -> Result<Self, Self::Error> {
        if item.response.response_code == SUCCESS_CODE {
            let connector_metadata = serde_json::json!(PayboxMeta {
                connector_request_id: item.response.transaction_number.clone()
            });

            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.paybox_order_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: Some(connector_metadata),
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Voided,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(item.response.transaction_number.clone()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// REFUND FLOW
// ============================================================================

#[derive(Debug, Serialize)]
pub struct PayboxRefundRequest {
    #[serde(rename = "VERSION")]
    pub version: String,
    #[serde(rename = "TYPE")]
    pub transaction_type: String,
    #[serde(rename = "SITE")]
    pub site: Secret<String>,
    #[serde(rename = "RANG")]
    pub rank: Secret<String>,
    #[serde(rename = "CLE")]
    pub key: Secret<String>,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: String,
    #[serde(rename = "MONTANT")]
    pub amount: MinorUnit,
    #[serde(rename = "DEVISE")]
    pub currency: String,
    #[serde(rename = "REFERENCE")]
    pub reference: String,
    #[serde(rename = "DATEQ")]
    pub date: String,
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
    #[serde(rename = "ACTIVITE")]
    pub activity: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for PayboxRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;
        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let numappel = router_data.request.connector_transaction_id.clone();

        let numtrans = router_data
            .request
            .connector_feature_data
            .expose_option()
            .as_ref()
            .and_then(|meta| serde_json::from_value::<PayboxMeta>(meta.clone()).ok())
            .map(|meta| meta.connector_request_id)
            .unwrap_or_else(|| numappel.clone());

        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: REFUND_REQUEST.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            amount,
            currency: router_data.request.currency.iso_4217().to_string(),
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            date: generate_date_time()?,
            transaction_number: numtrans,
            paybox_order_id: numappel,
            activity: PAY_ORIGIN_INTERNET.to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayboxRefundResponse {
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: Option<String>,
    #[serde(rename = "SITE")]
    pub site: Option<String>,
    #[serde(rename = "RANG")]
    pub rank: Option<String>,
    #[serde(rename = "AUTORISATION")]
    pub authorization: Option<String>,
    #[serde(rename = "CODEREPONSE")]
    pub response_code: String,
    #[serde(rename = "COMMENTAIRE")]
    pub response_message: String,
}

impl TryFrom<ResponseRouterData<PayboxRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PayboxRefundResponse, Self>) -> Result<Self, Self::Error> {
        if item.response.response_code == SUCCESS_CODE {
            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: item.response.paybox_order_id.clone(),
                    refund_status: RefundStatus::Success,
                    status_code: item.http_code,
                }),
                resource_common_data: RefundFlowData {
                    status: RefundStatus::Success,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(item.response.transaction_number.clone()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// RSYNC FLOW
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for PayboxSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let connector_refund_id = router_data.request.connector_refund_id.clone();

        Ok(Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: SYNC_REQUEST.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            date: generate_date_time()?,
            transaction_number: connector_refund_id.clone(),
            paybox_order_id: router_data.request.connector_transaction_id.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayboxRSyncResponse {
    #[serde(rename = "NUMTRANS")]
    pub transaction_number: String,
    #[serde(rename = "NUMAPPEL")]
    pub paybox_order_id: String,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: Option<String>,
    #[serde(rename = "SITE")]
    pub site: Option<String>,
    #[serde(rename = "RANG")]
    pub rank: Option<String>,
    #[serde(rename = "AUTORISATION")]
    pub authorization: Option<String>,
    #[serde(rename = "CODEREPONSE")]
    pub response_code: String,
    #[serde(rename = "COMMENTAIRE")]
    pub response_message: String,
    #[serde(rename = "STATUS")]
    pub status: Option<PayboxStatus>,
}

impl TryFrom<ResponseRouterData<PayboxRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PayboxRSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Determine refund status from either STATUS field or CODEREPONSE
        let refund_status = match item.response.status {
            Some(status) => RefundStatus::from(status),
            None => {
                // If STATUS field is not present, derive from CODEREPONSE
                // "00000" indicates success
                if item.response.response_code == "00000" {
                    RefundStatus::Success
                } else {
                    RefundStatus::Failure
                }
            }
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.paybox_order_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// SETUP MANDATE FLOW (Subscriber Registration - TYPE 00056)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PayboxSetupMandateRequest<T: PaymentMethodDataTypes> {
    pub version: String,
    #[serde(rename = "TYPE")]
    pub transaction_type: String,
    pub site: Secret<String>,
    #[serde(rename = "RANG")]
    pub rank: Secret<String>,
    #[serde(rename = "CLE")]
    pub key: Secret<String>,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: String,
    #[serde(rename = "MONTANT")]
    pub amount: MinorUnit,
    #[serde(rename = "DEVISE")]
    pub currency: String,
    pub reference: String,
    #[serde(rename = "DATEQ")]
    pub date: String,
    #[serde(rename = "PORTEUR")]
    pub card_number: RawCardNumber<T>,
    #[serde(rename = "DATEVAL")]
    pub expiration_date: Secret<String>,
    pub cvv: Secret<String>,
    #[serde(rename = "REFABONNE")]
    pub subscriber_ref: String,
    #[serde(rename = "ACTIVITE")]
    pub activity: String,
}

pub type PayboxSetupMandateResponse = PayboxPaymentResponse;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayboxSetupMandateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let card_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(req_card) => req_card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Only card payments are supported for SetupMandate".to_string(),
                    connector: "Paybox",
                    context: Default::default(),
                }
                .into())
            }
        };

        let expiration_date = Secret::new(
            card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_owned())?
                .peek()
                .to_string(),
        );

        // Use minor_amount if available, otherwise default to zero for zero-dollar auth
        let amount = match router_data.request.minor_amount {
            Some(minor_amount) => connector
                .amount_converter
                .convert(minor_amount, router_data.request.currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
            None => connector
                .amount_converter
                .convert(MinorUnit::zero(), router_data.request.currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: Default::default(),
                })?,
        };

        // Generate a unique subscriber reference from the connector_request_reference_id
        let subscriber_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: REGISTER_SUBSCRIBER_REQUEST.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            amount,
            currency: router_data.request.currency.iso_4217().to_string(),
            reference: subscriber_ref.clone(),
            date: generate_date_time()?,
            card_number: card_data.card_number.clone(),
            expiration_date,
            cvv: card_data.card_cvc.clone(),
            subscriber_ref,
            activity: PAY_ORIGIN_INTERNET.to_string(),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PayboxSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayboxSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.response_code == SUCCESS_CODE {
            // Build connector metadata with NUMTRANS for future operations
            let connector_metadata = serde_json::json!(PayboxMeta {
                connector_request_id: item.response.transaction_number.clone()
            });

            // Pack PORTEUR (stored card token) and card expiry into connector_mandate_id
            // using "::" delimiter so MIT can extract both PORTEUR and DATEVAL.
            // REFABONNE (customer_id) = subscriber reference for connector_mandate_request_reference_id.
            let carrier_with_expiry = item.response.carrier_id.as_ref().map(|carrier| {
                let card_expiry = match &item.router_data.request.payment_method_data {
                    PaymentMethodData::Card(card) => card
                        .get_card_expiry_month_year_2_digit_with_delimiter("".to_owned())
                        .ok()
                        .map(|s| s.peek().to_string()),
                    _ => None,
                };
                match card_expiry {
                    Some(expiry) => format!("{}::{}", carrier.peek(), expiry),
                    None => carrier.peek().to_string(),
                }
            });
            let mandate_reference = Some(Box::new(MandateReference {
                connector_mandate_id: carrier_with_expiry,
                payment_method_id: None,
                connector_mandate_request_reference_id: item
                    .response
                    .customer_id
                    .as_ref()
                    .map(|id| id.peek().to_string()),
            }));

            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.paybox_order_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference,
                    connector_metadata: Some(connector_metadata),
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Charged,
                    reference_id: Some(item.response.transaction_number.clone()),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(item.response.transaction_number.clone()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// REPEAT PAYMENT FLOW (Subscriber Authorization - TYPE 00051/00053)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PayboxRepeatPaymentRequest {
    pub version: String,
    #[serde(rename = "TYPE")]
    pub transaction_type: String,
    pub site: Secret<String>,
    #[serde(rename = "RANG")]
    pub rank: Secret<String>,
    #[serde(rename = "CLE")]
    pub key: Secret<String>,
    #[serde(rename = "NUMQUESTION")]
    pub paybox_request_number: String,
    #[serde(rename = "MONTANT")]
    pub amount: MinorUnit,
    #[serde(rename = "DEVISE")]
    pub currency: String,
    pub reference: String,
    #[serde(rename = "DATEQ")]
    pub date: String,
    #[serde(rename = "PORTEUR")]
    pub subscriber_number: String,
    #[serde(rename = "DATEVAL")]
    pub expiration_date: Secret<String>,
    #[serde(rename = "REFABONNE")]
    pub subscriber_ref: String,
    #[serde(rename = "ACTIVITE")]
    pub activity: String,
}

pub type PayboxRepeatPaymentResponse = PayboxPaymentResponse;

/// Get the transaction type for repeat/recurring subscriber payments
fn get_subscriber_transaction_type(
    capture_method: Option<common_enums::CaptureMethod>,
) -> &'static str {
    match capture_method {
        Some(common_enums::CaptureMethod::Automatic) => SUBSCRIBER_AUTH_AND_CAPTURE_REQUEST,
        _ => SUBSCRIBER_AUTH_REQUEST,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayboxRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayboxRepeatPaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PayboxRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth = PayboxAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // connector_mandate_id is packed as "PORTEUR::DATEVAL" (e.g. "CMDLpSqLLDS::1228")
        let packed_mandate_id = router_data.request.connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            },
        )?;

        // Split packed mandate id into PORTEUR (stored card token) and DATEVAL (card expiry)
        let (porteur, expiry_str) = if let Some(idx) = packed_mandate_id.find("::") {
            (
                packed_mandate_id[..idx].to_string(),
                packed_mandate_id[idx + 2..].to_string(),
            )
        } else {
            (packed_mandate_id.clone(), "0000".to_string())
        };

        // REFABONNE = connector_mandate_request_reference_id (subscriber reference)
        let refabonne = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => {
                connector_mandate_ids
                    .get_connector_mandate_request_reference_id()
                    .unwrap_or_else(|| porteur.clone())
            }
            _ => porteur.clone(),
        };

        let expiration_date = Secret::new(expiry_str);

        let transaction_type = get_subscriber_transaction_type(router_data.request.capture_method);

        Ok(Self {
            version: VERSION_PAYBOX.to_string(),
            transaction_type: transaction_type.to_string(),
            site: auth.site,
            rank: auth.rank,
            key: auth.key,
            paybox_request_number: generate_request_id()?,
            amount,
            currency: router_data.request.currency.iso_4217().to_string(),
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            date: generate_date_time()?,
            subscriber_number: porteur,
            expiration_date,
            subscriber_ref: refabonne,
            activity: PAY_ORIGIN_RECURRING.to_string(),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PayboxRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayboxRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.response_code == SUCCESS_CODE {
            let is_auto_capture = item.router_data.request.is_auto_capture();

            let status = if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            };

            let connector_metadata = serde_json::json!(PayboxMeta {
                connector_request_id: item.response.transaction_number.clone()
            });

            // Preserve the mandate reference from the subscriber
            let mandate_reference = item.response.customer_id.as_ref().map(|subscriber_id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(subscriber_id.peek().to_string()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                })
            });

            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.paybox_order_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference,
                    connector_metadata: Some(connector_metadata),
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    reference_id: Some(item.response.transaction_number.clone()),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(ErrorResponse {
                    code: item.response.response_code.clone(),
                    message: item.response.response_message.clone(),
                    reason: Some(item.response.response_message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(item.response.transaction_number.clone()),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// ERROR RESPONSE
// ============================================================================

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PayboxErrorResponse {
    pub status_code: u16,
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}
