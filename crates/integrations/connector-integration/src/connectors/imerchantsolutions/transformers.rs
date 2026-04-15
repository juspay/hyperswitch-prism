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
                context: Default::default(),
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
) -> ImerchantsolutionsMetadata {
    metadata
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
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
                );
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
        let status = get_attempt_status(
            item.response.status.clone(),
            item.router_data.request.capture_method,
        );

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

impl utils::MultipleCaptureSyncResponse for ImerchantsolutionsPSyncResponseData {
    fn get_connector_capture_id(&self) -> String {
        self.payment_id.clone()
    }

    fn get_capture_attempt_status(&self) -> AttemptStatus {
        let capture_method = match self.capture_mode {
            CaptureMode::Auto => CaptureMethod::Automatic,
            CaptureMode::Manual => {
                if !self.captures.is_empty() && self.can_capture {
                    CaptureMethod::ManualMultiple
                } else {
                    CaptureMethod::Manual
                }
            }
        };

        get_attempt_status(self.status.clone(), Some(capture_method))
    }

    fn get_connector_reference_id(&self) -> Option<String> {
        Some(self.psp_reference.clone())
    }

    fn is_capture_response(&self) -> bool {
        matches!(
            self.status,
            ImerchantsolutionsPaymentStatus::Captured
                | ImerchantsolutionsPaymentStatus::PartiallyCaptured
                | ImerchantsolutionsPaymentStatus::Pending
        )
    }

    fn get_amount_captured(&self) -> Result<Option<MinorUnit>, error_stack::Report<ParsingError>> {
        Ok(self.total_captured)
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

        let status = get_attempt_status(
            item.response.status.clone(),
            item.router_data.request.capture_method,
        );

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
            let capture_sync_response_list =
                utils::construct_captures_response_hashmap(vec![item.response.clone()])
                    .change_context(utils::response_handling_fail_for_connector(
                        item.http_code,
                        "imerchantsolutions",
                    ))?;

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::MultipleCaptureResponse {
                    capture_sync_response_list,
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
                context: Default::default(),
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
        let status = get_capture_status(
            item.response.status,
            item.router_data.request.capture_method,
        );

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
        let refund_status = get_refund_status(item.response.status);

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
        let refund_status = get_refund_status(item.response.status.clone());
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

fn get_attempt_status(
    item: ImerchantsolutionsPaymentStatus,
    capture_method: Option<CaptureMethod>,
) -> AttemptStatus {
    match item {
        ImerchantsolutionsPaymentStatus::Authorised
        | ImerchantsolutionsPaymentStatus::Authorized
        | ImerchantsolutionsPaymentStatus::PendingCapture => AttemptStatus::Authorized,
        ImerchantsolutionsPaymentStatus::Pending3ds => AttemptStatus::AuthenticationPending,
        ImerchantsolutionsPaymentStatus::Cancelled => AttemptStatus::Voided,
        ImerchantsolutionsPaymentStatus::PartiallyCaptured => match capture_method {
            Some(CaptureMethod::ManualMultiple) => AttemptStatus::PartialChargedAndChargeable,
            _ => AttemptStatus::PartialCharged,
        },
        ImerchantsolutionsPaymentStatus::Captured
        | ImerchantsolutionsPaymentStatus::PartiallyRefunded
        | ImerchantsolutionsPaymentStatus::Refunded => AttemptStatus::Charged,
        ImerchantsolutionsPaymentStatus::Pending => AttemptStatus::Pending,
        ImerchantsolutionsPaymentStatus::Refused | ImerchantsolutionsPaymentStatus::Failed => {
            AttemptStatus::Failure
        }
    }
}

fn get_capture_status(
    capture_status: ImerchantsolutionsCaptureStatus,
    capture_method: Option<CaptureMethod>,
) -> AttemptStatus {
    match capture_status {
        ImerchantsolutionsCaptureStatus::Received => AttemptStatus::CaptureInitiated,
        ImerchantsolutionsCaptureStatus::PartiallyCaptured => match capture_method {
            Some(CaptureMethod::ManualMultiple) => AttemptStatus::PartialChargedAndChargeable,
            _ => AttemptStatus::PartialCharged,
        },
        ImerchantsolutionsCaptureStatus::Captured => AttemptStatus::Charged,
    }
}

fn get_refund_status(refund_status: ImerchantsolutionsRefundStatus) -> RefundStatus {
    match refund_status {
        ImerchantsolutionsRefundStatus::Received => RefundStatus::Pending,
        ImerchantsolutionsRefundStatus::PartiallyRefunded
        | ImerchantsolutionsRefundStatus::Refunded => RefundStatus::Success,
    }
}
