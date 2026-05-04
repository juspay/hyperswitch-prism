use common_enums::{self, AttemptStatus, CaptureMethod, CountryAlpha2, Currency, RefundStatus};
use common_utils::{consts, errors::ParsingError, pii, types::MinorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::SyncRequestType,
    utils::is_payment_failure,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::imerchantsolutions::ImerchantsolutionsRouterData,
    types::ResponseRouterData,
    utils::{self, is_manual_capture},
};

const IMERCHANTSOLUTIONS: &str = "imerchantsolutions";

pub struct ImerchantsolutionsAuthType {
    pub(super) api_key: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImerchantsolutionsErrorResponse {
    pub error: String,
    pub message: Option<String>,
    pub code: Option<String>,
    pub suggestion: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ImerchantsolutionsAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Imerchantsolutions { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some("Provide AuthType as HeaderKey".to_string()),
                    doc_url: Some("https://imerchantsolutions.com/docs#authentication".to_string()),
                    additional_context: Some(
                        "Provided AuthType is incorrect. AuthType should be HeaderKey.".to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsPaymentsRequestData<T: PaymentMethodDataTypes> {
    amount: MinorUnit,
    currency: Currency,
    reference: String,
    card: CardDetails<T>,
    shopper_email: Option<pii::Email>,
    shopper_name: Option<ShopperName>,
    telephone_number: Option<Secret<String>>,
    billing: Option<AddressDetails>,
    delivery_address: Option<AddressDetails>,
    manual_capture: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    capture_delay_hours: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ImerchantsolutionsMetadata {
    capture_delay_hours: Option<u32>,
}

fn get_imerchantsolutions_metadata(
    metadata: Option<serde_json::Value>,
) -> error_stack::Result<ImerchantsolutionsMetadata, errors::IntegrationError> {
    metadata
        .map(|meta| {
            serde_json::from_value::<ImerchantsolutionsMetadata>(meta).change_context(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "connector_metadata",
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(
                            "Failed to deserialize Imerchantsolutions metadata".to_string(),
                        ),
                    },
                },
            )
        })
        .transpose()
        .map(|opt| opt.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct CardDetails<T: PaymentMethodDataTypes> {
    number: RawCardNumber<T>,
    cvv: Secret<String>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    holder: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ShopperName {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AddressDetails {
    address: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
    country: Option<CountryAlpha2>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ImerchantsolutionsPaymentsRequestData<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref card_data) => {
                let card = CardDetails {
                    number: card_data.card_number.clone(),
                    cvv: card_data.card_cvc.clone(),
                    expiry_month: card_data.get_card_expiry_month_2_digit()?,
                    expiry_year: card_data.get_expiry_year_4_digit(),
                    holder: card_data.get_optional_cardholder_name(),
                };
                let shopper_email = item.router_data.request.get_optional_email().or_else(|| {
                    item.router_data
                        .resource_common_data
                        .get_optional_billing_email()
                });
                let shopper_name = Some(ShopperName {
                    first_name: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_first_name(),
                    last_name: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_last_name(),
                });
                let billing = Some(AddressDetails {
                    address: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_line1(),
                    city: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_city(),
                    state: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_state(),
                    postal_code: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_zip(),
                    country: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_country(),
                });
                let delivery_address = Some(AddressDetails {
                    address: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_line1(),
                    city: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_city(),
                    state: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_state(),
                    postal_code: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_zip(),
                    country: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_country(),
                });
                let imerchantsolutions_metadata = get_imerchantsolutions_metadata(
                    item.router_data.request.metadata.clone().expose_option(),
                )?;
                Ok(Self {
                    amount: item.router_data.request.amount,
                    currency: item.router_data.request.currency,
                    reference: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    card,
                    shopper_email,
                    shopper_name,
                    telephone_number: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_phone_number(),
                    billing,
                    delivery_address,
                    manual_capture: is_manual_capture(item.router_data.request.capture_method),
                    capture_delay_hours: imerchantsolutions_metadata.capture_delay_hours,
                })
            }
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
                Err(errors::IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Imerchantsolutions"),
                    Default::default(),
                ))?
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsPaymentsResponseData {
    payment_id: String,
    psp_reference: String,
    merchant_reference: Option<String>,
    amount: AmountDetails,
    result_code: ResultCode,
    status: ImerchantsolutionsPaymentStatus,
    capture_mode: Option<CaptureMode>,
    capture_delay_hours: Option<i32>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AmountDetails {
    value: MinorUnit,
    currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum ResultCode {
    Authorised,
    Refused,
    Pending,
    Error,
    Cancelled,
    RedirectShopper,
    ChallengeShopper,
    IdentifyShopper,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ImerchantsolutionsPaymentStatus {
    #[serde(alias = "AUTHORISED")]
    Authorised,
    Authorized,
    #[serde(rename = "pending_3ds")]
    Pending3ds,
    Cancelled,
    #[serde(alias = "PENDING_CAPTURE")]
    PendingCapture,
    PartiallyCaptured,
    Captured,
    Pending,
    Refused,
    Failed,
    PartiallyRefunded,
    Refunded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    Auto,
    Manual,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ImerchantsolutionsPaymentsResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsPaymentsResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::foreign_try_from((
            item.response.status.clone(),
            item.router_data.request.capture_method,
            item.http_code,
        ))?;

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.psp_reference),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.psp_reference.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(item.response.payment_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsPSyncResponseData {
    payment_id: String,
    psp_reference: String,
    merchant_reference: Option<String>,
    authorized_amount: Option<MinorUnit>,
    total_captured: Option<MinorUnit>,
    remaining_amount: Option<MinorUnit>,
    captures: Vec<Captures>,
    currency: Currency,
    status: ImerchantsolutionsPaymentStatus,
    capture_mode: CaptureMode,
    captured_at: Option<String>,
    can_capture: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Captures {
    amount: MinorUnit,
    currency: Currency,
    psp_reference: String,
    captured_at: Option<String>,
}

#[derive(Clone, Debug)]
struct CaptureWithStatus<'a> {
    capture: &'a Captures,
    status: &'a ImerchantsolutionsPaymentStatus,
    psp_reference: &'a String,
}

impl<'a> utils::MultipleCaptureSyncResponse for CaptureWithStatus<'a> {
    fn get_connector_capture_id(&self) -> String {
        self.capture.psp_reference.clone()
    }

    // Connector does not provide per-capture status.
    // We derive capture status from overall payment status.
    // This assumes uniform outcome across all captures.
    fn get_capture_attempt_status(&self) -> AttemptStatus {
        self.status.clone().into()
    }

    fn is_capture_response(&self) -> bool {
        true
    }

    fn get_connector_reference_id(&self) -> Option<String> {
        Some(self.psp_reference.clone())
    }

    fn get_amount_captured(&self) -> Result<Option<MinorUnit>, error_stack::Report<ParsingError>> {
        Ok(Some(self.capture.amount))
    }
}

impl<F> TryFrom<ResponseRouterData<ImerchantsolutionsPSyncResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsPSyncResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let is_multiple_capture_psync_flow = match item.router_data.request.sync_type {
            SyncRequestType::MultipleCaptureSync => true,
            SyncRequestType::SinglePaymentSync => false,
        };

        let status = AttemptStatus::foreign_try_from((
            item.response.status.clone(),
            item.router_data.request.capture_method,
            item.http_code,
        ))?;

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.psp_reference),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        } else if is_multiple_capture_psync_flow {
            let wrapped_captures: Vec<CaptureWithStatus<'_>> = item
                .response
                .captures
                .iter()
                .map(|c| CaptureWithStatus {
                    capture: c,
                    status: &item.response.status,
                    psp_reference: &item.response.psp_reference,
                })
                .collect();

            let capture_sync_response_list =
                utils::construct_captures_response_hashmap(wrapped_captures).change_context(
                    utils::response_handling_fail_for_connector(item.http_code, IMERCHANTSOLUTIONS),
                )?;

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: item.response.status.clone().into(),
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::MultipleCaptureResponse {
                    capture_sync_response_list,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.psp_reference.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(item.response.payment_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsVoidRequestData {
    psp_reference: String,
    reason: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ImerchantsolutionsVoidRequestData
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            psp_reference: item.router_data.request.connector_transaction_id,
            reason: item.router_data.request.cancellation_reason.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsVoidResponseData {
    success: bool,
    psp_reference: String,
    original_reference: String,
    status: ImerchantsolutionsVoidStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ImerchantsolutionsVoidStatus {
    Received,
    Cancelled,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsVoidResponseData, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsVoidResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status {
            ImerchantsolutionsVoidStatus::Received => AttemptStatus::VoidInitiated,
            ImerchantsolutionsVoidStatus::Cancelled => AttemptStatus::Voided,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.original_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.psp_reference.clone()),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsCaptureRequestData {
    psp_reference: String,
    amount: MinorUnit,
    currency: Currency,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for ImerchantsolutionsCaptureRequestData
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let psp_reference = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: Some(
                        "https://imerchantsolutions.com/docs/api#post--payments-capture"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "Expected connector transaction ID not found".to_string(),
                    ),
                },
            })?;

        Ok(Self {
            psp_reference,
            amount: item.router_data.request.minor_amount_to_capture,
            currency: item.router_data.request.currency,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsCaptureResponseData {
    success: bool,
    psp_reference: String,
    original_reference: String,
    captured_amount: Option<MinorUnit>,
    total_captured: Option<MinorUnit>,
    currency: Currency,
    status: ImerchantsolutionsCaptureStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ImerchantsolutionsCaptureStatus {
    Received,
    PartiallyCaptured,
    Captured,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsCaptureResponseData, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsCaptureResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::foreign_try_from((
            item.response.status,
            item.router_data.request.capture_method,
            item.http_code,
        ))?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.original_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.psp_reference.clone()),
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

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsRefundRequestData {
    psp_reference: String,
    amount: MinorUnit,
    currency: Currency,
    reference: Option<String>,
    reason: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for ImerchantsolutionsRefundRequestData
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            psp_reference: item.router_data.request.connector_transaction_id.clone(),
            amount: item.router_data.request.minor_refund_amount,
            currency: item.router_data.request.currency,
            reference: Some(item.router_data.request.refund_id.clone()),
            reason: item.router_data.request.reason,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsRefundResponseData {
    success: bool,
    psp_reference: String,
    original_reference: String,
    refunded_amount: MinorUnit,
    total_refunded: MinorUnit,
    currency: Currency,
    status: ImerchantsolutionsRefundStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ImerchantsolutionsRefundStatus {
    Received,
    PartiallyRefunded,
    Refunded,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsRefundResponseData, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsRefundResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item.response.status.into();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.psp_reference.to_string(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsRsyncResponse {
    payment_id: String,
    psp_reference: String,
    merchant_reference: Option<String>,
    payment_amount: Option<MinorUnit>,
    total_captured: Option<MinorUnit>,
    total_refunded: Option<MinorUnit>,
    remaining_amount: Option<MinorUnit>,
    currency: Currency,
    status: ImerchantsolutionsRefundStatus,
    can_refund: bool,
    refunds: Vec<Refunds>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Refunds {
    psp_reference: String,
    amount: MinorUnit,
    currency: Currency,
    reason: Option<String>,
    created_at: Option<String>,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsRsyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsRsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item.response.status.clone().into();
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

impl ForeignTryFrom<(ImerchantsolutionsPaymentStatus, Option<CaptureMethod>, u16)>
    for AttemptStatus
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn foreign_try_from(
        (item, capture_method, http_status): (
            ImerchantsolutionsPaymentStatus,
            Option<CaptureMethod>,
            u16,
        ),
    ) -> Result<Self, Self::Error> {
        Ok(match item {
            ImerchantsolutionsPaymentStatus::Authorised
            | ImerchantsolutionsPaymentStatus::Authorized
            | ImerchantsolutionsPaymentStatus::PendingCapture => Self::Authorized,

            ImerchantsolutionsPaymentStatus::Pending3ds => Self::AuthenticationPending,

            ImerchantsolutionsPaymentStatus::Cancelled => Self::Voided,

            ImerchantsolutionsPaymentStatus::PartiallyCaptured => match capture_method {
                Some(CaptureMethod::ManualMultiple) => Self::PartialChargedAndChargeable,
                Some(CaptureMethod::Manual) => Self::PartialCharged,
                Some(CaptureMethod::Automatic)
                | Some(CaptureMethod::SequentialAutomatic)
                | Some(CaptureMethod::Scheduled)
                | None => {
                    return Err(error_stack::Report::new(
                        errors::ConnectorError::response_handling_failed_with_context(
                            http_status,
                            Some("capture method not supported".to_string()),
                        ),
                    ));
                }
            },

            ImerchantsolutionsPaymentStatus::Captured
            | ImerchantsolutionsPaymentStatus::PartiallyRefunded
            | ImerchantsolutionsPaymentStatus::Refunded => Self::Charged,

            ImerchantsolutionsPaymentStatus::Pending => Self::Pending,

            ImerchantsolutionsPaymentStatus::Refused | ImerchantsolutionsPaymentStatus::Failed => {
                Self::Failure
            }
        })
    }
}

impl ForeignTryFrom<(ImerchantsolutionsCaptureStatus, Option<CaptureMethod>, u16)>
    for AttemptStatus
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn foreign_try_from(
        (capture_status, capture_method, http_status): (
            ImerchantsolutionsCaptureStatus,
            Option<CaptureMethod>,
            u16,
        ),
    ) -> Result<Self, Self::Error> {
        Ok(match capture_status {
            ImerchantsolutionsCaptureStatus::Received => Self::CaptureInitiated,

            ImerchantsolutionsCaptureStatus::PartiallyCaptured => match capture_method {
                Some(CaptureMethod::ManualMultiple) => Self::PartialChargedAndChargeable,
                Some(CaptureMethod::Manual) => Self::PartialCharged,
                Some(CaptureMethod::Automatic)
                | Some(CaptureMethod::SequentialAutomatic)
                | Some(CaptureMethod::Scheduled)
                | None => {
                    return Err(error_stack::Report::new(
                        errors::ConnectorError::response_handling_failed_with_context(
                            http_status,
                            Some("capture method not supported".to_string()),
                        ),
                    ));
                }
            },

            ImerchantsolutionsCaptureStatus::Captured => Self::Charged,
        })
    }
}

impl From<ImerchantsolutionsPaymentStatus> for AttemptStatus {
    fn from(status: ImerchantsolutionsPaymentStatus) -> Self {
        match status {
            ImerchantsolutionsPaymentStatus::Authorised
            | ImerchantsolutionsPaymentStatus::Authorized
            | ImerchantsolutionsPaymentStatus::PendingCapture
            | ImerchantsolutionsPaymentStatus::Pending3ds
            | ImerchantsolutionsPaymentStatus::Pending => Self::Pending,

            ImerchantsolutionsPaymentStatus::Cancelled
            | ImerchantsolutionsPaymentStatus::Refused
            | ImerchantsolutionsPaymentStatus::Failed => Self::CaptureFailed,

            ImerchantsolutionsPaymentStatus::PartiallyCaptured
            | ImerchantsolutionsPaymentStatus::Captured
            | ImerchantsolutionsPaymentStatus::PartiallyRefunded
            | ImerchantsolutionsPaymentStatus::Refunded => Self::Charged,
        }
    }
}

impl From<ImerchantsolutionsRefundStatus> for RefundStatus {
    fn from(status: ImerchantsolutionsRefundStatus) -> Self {
        match status {
            ImerchantsolutionsRefundStatus::Received => Self::Pending,
            ImerchantsolutionsRefundStatus::PartiallyRefunded
            | ImerchantsolutionsRefundStatus::Refunded => Self::Success,
        }
    }
}
