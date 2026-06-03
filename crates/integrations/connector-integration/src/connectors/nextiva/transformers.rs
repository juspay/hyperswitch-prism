use std::fmt::Debug;

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    types::{MinorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::{report, Report, ResultExt};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::nextiva::NextivaRouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================
#[derive(Debug, Clone)]
pub struct NextivaAuthType {
    pub api_accesskey: Secret<String>,
    pub account_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NextivaAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nextiva {
                api_key,
                account_id,
                ..
            } => Ok(Self {
                api_accesskey: api_key.to_owned(),
                account_id: account_id.to_owned(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: Default::default()
            })),
        }
    }
}

// =============================================================================
// COMMON ENUMS
// =============================================================================
#[derive(Debug, Clone, Serialize)]
pub enum TenderType {
    #[serde(rename = "CARD")]
    Card,
}

#[derive(Debug, Clone, Serialize)]
pub enum TransactionType {
    #[serde(rename = "SALE")]
    Sale,
    #[serde(rename = "AUTHORIZATION")]
    Authorization,
    #[serde(rename = "CAPTURE")]
    Capture,
    #[serde(rename = "REFUND")]
    Refund,
}

#[derive(Debug, Clone, Serialize)]
pub enum ResponseFormat {
    #[serde(rename = "JSON")]
    Json,
}

#[derive(Debug, Clone, Serialize)]
pub enum TsapiAction {
    #[serde(rename = "GET_TRANSACTION_STATUS")]
    GetTransactionStatus,
}

// =============================================================================
// FLEXIBLE BOOLEAN — PayConex returns "1"/"0"/true/false/null for booleans
// =============================================================================
fn de_flexible_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexibleBool {
        Bool(bool),
        Int(i64),
        Str(String),
    }
    let value = Option::<FlexibleBool>::deserialize(deserializer)?;
    Ok(match value {
        Some(FlexibleBool::Bool(b)) => b,
        Some(FlexibleBool::Int(i)) => i != 0,
        Some(FlexibleBool::Str(s)) => {
            let s = s.trim().to_ascii_lowercase();
            s == "1" || s == "true" || s == "y" || s == "yes"
        }
        None => false,
    })
}

// =============================================================================
// AUTHORIZE / SALE REQUEST
// =============================================================================
#[derive(Debug, Serialize)]
pub struct NextivaPaymentsRequest<T: PaymentMethodDataTypes + Serialize + Debug> {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub tender_type: TenderType,
    pub transaction_type: TransactionType,
    pub transaction_amount: StringMajorUnit,
    pub card_number: RawCardNumber<T>,
    pub card_expiration: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_verification: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    pub custom_id: String,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NextivaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NextivaPaymentsRequest<T>
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: NextivaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let auth = NextivaAuthType::try_from(&router_data.connector_config)?;

        if router_data.resource_common_data.is_three_ds() {
            return Err(report!(IntegrationError::NotSupported {
                message: "3DS card payments".to_string(),
                connector: "Nextiva",
                context: Default::default(),
            }));
        }

        let transaction_type = if request.is_auto_capture() {
            TransactionType::Sale
        } else {
            TransactionType::Authorization
        };

        let transaction_amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        match request.payment_method_data.clone() {
            PaymentMethodData::Card(card) => Ok(Self {
                account_id: auth.account_id,
                api_accesskey: auth.api_accesskey,
                tender_type: TenderType::Card,
                transaction_type,
                transaction_amount,
                card_number: card.card_number.clone(),
                card_expiration: card
                    .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
                card_verification: Some(card.card_cvc.clone()),
                first_name: card.card_holder_name.clone(),
                last_name: None,
                custom_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                response_format: ResponseFormat::Json,
            }),
            _ => Err(report!(IntegrationError::NotImplemented(
                "Payment method".to_string(),
                Default::default()
            ))),
        }
    }
}

// =============================================================================
// CAPTURE REQUEST
// =============================================================================
#[derive(Debug, Serialize)]
pub struct NextivaCaptureRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub tender_type: TenderType,
    pub transaction_type: TransactionType,
    pub token_id: String,
    pub transaction_amount: StringMajorUnit,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NextivaRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NextivaCaptureRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: NextivaRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NextivaAuthType::try_from(&router_data.connector_config)?;
        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            tender_type: TenderType::Card,
            transaction_type: TransactionType::Capture,
            token_id: router_data.request.get_connector_transaction_id()?,
            transaction_amount,
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// PSYNC REQUEST (TSAPI)
// =============================================================================
#[derive(Debug, Serialize)]
pub struct NextivaSyncRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub action: TsapiAction,
    pub transaction_id: String,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NextivaRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for NextivaSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: NextivaRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NextivaAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            action: TsapiAction::GetTransactionStatus,
            transaction_id,
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// REFUND REQUEST
// =============================================================================
#[derive(Debug, Serialize)]
pub struct NextivaRefundRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub tender_type: TenderType,
    pub transaction_type: TransactionType,
    pub token_id: String,
    pub transaction_amount: StringMajorUnit,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NextivaRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for NextivaRefundRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: NextivaRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NextivaAuthType::try_from(&router_data.connector_config)?;
        let transaction_amount = item
            .connector
            .amount_converter
            .convert(
                MinorUnit::new(router_data.request.refund_amount),
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            tender_type: TenderType::Card,
            transaction_type: TransactionType::Refund,
            token_id: router_data.request.connector_transaction_id.clone(),
            transaction_amount,
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// RSYNC REQUEST (TSAPI)
// =============================================================================
#[derive(Debug, Serialize)]
pub struct NextivaRefundSyncRequest {
    pub account_id: Secret<String>,
    pub api_accesskey: Secret<String>,
    pub action: TsapiAction,
    pub transaction_id: String,
    pub response_format: ResponseFormat,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NextivaRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for NextivaRefundSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: NextivaRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = NextivaAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            account_id: auth.account_id,
            api_accesskey: auth.api_accesskey,
            action: TsapiAction::GetTransactionStatus,
            transaction_id: router_data.request.connector_refund_id.clone(),
            response_format: ResponseFormat::Json,
        })
    }
}

// =============================================================================
// RESPONSES
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NextivaErrorResponse {
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub error: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub authorization_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NextivaPaymentsResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl NextivaPaymentsResponse {
    fn is_approved(&self) -> bool {
        self.transaction_approved
    }

    fn get_transaction_id(&self, http_code: u16) -> CustomResult<String, ConnectorError> {
        self.transaction_id.clone().ok_or_else(|| {
            report!(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("missing transaction_id in Nextiva response".to_string()),
            ))
        })
    }

    fn error_code_string(&self) -> String {
        self.error_code
            .as_ref()
            .map(value_to_string)
            .unwrap_or_else(|| NO_ERROR_CODE.to_string())
    }

    fn error_message_string(&self) -> String {
        self.error_message
            .clone()
            .or_else(|| self.authorization_message.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
    }
}

pub fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---- Authorize response ----
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NextivaPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NextivaPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = item.router_data.request.is_auto_capture();
        if item.response.is_approved() {
            let status = if is_auto_capture {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            };
            let transaction_id = item.response.get_transaction_id(item.http_code)?;
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: item.response.error_code_string(),
                    message: item.response.error_message_string(),
                    reason: item.response.authorization_message.clone(),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: item.response.transaction_id.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ---- Capture response ----
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NextivaCaptureResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl NextivaCaptureResponse {
    fn is_approved(&self) -> bool {
        self.transaction_approved
    }
    fn get_transaction_id(&self, http_code: u16) -> CustomResult<String, ConnectorError> {
        self.transaction_id.clone().ok_or_else(|| {
            report!(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("missing transaction_id in Nextiva capture response".to_string()),
            ))
        })
    }
    fn error_code_string(&self) -> String {
        self.error_code
            .as_ref()
            .map(value_to_string)
            .unwrap_or_else(|| NO_ERROR_CODE.to_string())
    }
    fn error_message_string(&self) -> String {
        self.error_message
            .clone()
            .or_else(|| self.authorization_message.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
    }
}

impl TryFrom<ResponseRouterData<NextivaCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NextivaCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.is_approved() {
            let transaction_id = item.response.get_transaction_id(item.http_code)?;
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Charged,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: item.response.error_code_string(),
                    message: item.response.error_message_string(),
                    reason: item.response.authorization_message.clone(),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            })
        }
    }
}

// ---- PSync response (TSAPI) ----
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NextivaSyncResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub found: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl TryFrom<ResponseRouterData<NextivaSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<NextivaSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = if !item.response.found {
            common_enums::AttemptStatus::Pending
        } else if item.response.transaction_approved {
            common_enums::AttemptStatus::Charged
        } else {
            common_enums::AttemptStatus::Failure
        };

        let response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: item
                    .response
                    .error_code
                    .as_ref()
                    .map(value_to_string)
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .error_message
                    .clone()
                    .or_else(|| item.response.authorization_message.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.authorization_message.clone(),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: item.response.transaction_id.clone(),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// ---- Refund / RSync response ----
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NextivaRefundResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub found: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl NextivaRefundResponse {
    fn error_code_string(&self) -> String {
        self.error_code
            .as_ref()
            .map(value_to_string)
            .unwrap_or_else(|| NO_ERROR_CODE.to_string())
    }
    fn error_message_string(&self) -> String {
        self.error_message
            .clone()
            .or_else(|| self.authorization_message.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
    }
}

impl TryFrom<ResponseRouterData<NextivaRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NextivaRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = if item.response.transaction_approved {
            common_enums::RefundStatus::Success
        } else {
            common_enums::RefundStatus::Failure
        };
        let response = if refund_status == common_enums::RefundStatus::Success {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id.clone().ok_or_else(|| {
                    report!(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("missing transaction_id in Nextiva refund response".to_string()),
                    ))
                })?,
                refund_status,
                status_code: item.http_code,
            })
        } else {
            Err(ErrorResponse {
                code: item.response.error_code_string(),
                message: item.response.error_message_string(),
                reason: item.response.authorization_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        };
        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NextivaRefundSyncResponse {
    pub transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub transaction_approved: bool,
    #[serde(default, deserialize_with = "de_flexible_bool")]
    pub found: bool,
    pub authorization_message: Option<String>,
    pub error_code: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

impl NextivaRefundSyncResponse {
    fn error_code_string(&self) -> String {
        self.error_code
            .as_ref()
            .map(value_to_string)
            .unwrap_or_else(|| NO_ERROR_CODE.to_string())
    }
    fn error_message_string(&self) -> String {
        self.error_message
            .clone()
            .or_else(|| self.authorization_message.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
    }
}

impl TryFrom<ResponseRouterData<NextivaRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NextivaRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = if !item.response.found {
            common_enums::RefundStatus::Pending
        } else if item.response.transaction_approved {
            common_enums::RefundStatus::Success
        } else {
            common_enums::RefundStatus::Failure
        };
        let response = if refund_status == common_enums::RefundStatus::Failure {
            Err(ErrorResponse {
                code: item.response.error_code_string(),
                message: item.response.error_message_string(),
                reason: item.response.authorization_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item
                    .response
                    .transaction_id
                    .clone()
                    .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone()),
                refund_status,
                status_code: item.http_code,
            })
        };
        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}
