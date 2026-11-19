use common_enums::Currency;
use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::RepeatPayment,
    connector_types::{
        MandateReferenceId, PaymentFlowData, PaymentsResponseData, PaymentsSyncData,
        RepeatPaymentData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::hyperswitch::HyperswitchRouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================
pub struct HyperswitchAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for HyperswitchAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Hyperswitch { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// =============================================================================
// ENUMS
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperswitchIntentStatus {
    Succeeded,
    Failed,
    Cancelled,
    CancelledPostCapture,
    Processing,
    RequiresCustomerAction,
    RequiresMerchantAction,
    RequiresPaymentMethod,
    RequiresConfirmation,
    RequiresCapture,
    PartiallyCaptured,
    PartiallyCapturedAndCapturable,
    PartiallyAuthorizedAndRequiresCapture,
    PartiallyCapturedAndProcessing,
    Conflicted,
    Expired,
    #[serde(other)]
    Unknown,
}

// =============================================================================
// REQUEST: REPEAT PAYMENT (MIT recurring charge)  (POST /payments + recurring_details)
// =============================================================================
// RepeatPayment charges off-session against a payment method vaulted by a prior
// SetupMandate. Hyperswitch references its own vaulted payment method by id, so
// the request uses `recurring_details { type: "payment_method_id", data: "pm_..." }`
// together with the owning `customer_id` — NOT the `processor_payment_token`
// object (that form is for raw processor tokens and leaves Hyperswitch with no
// card data, which the underlying gateway rejects). See tech spec
// "RepeatPayment — Merchant-Initiated Recurring Charge (MIT)".
#[derive(Debug, Serialize)]
pub struct HyperswitchPaymentMethodIdRecurringDetails {
    #[serde(rename = "type")]
    pub recurring_type: String,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchRepeatPaymentRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub confirm: bool,
    pub off_session: bool,
    pub customer_id: String,
    pub recurring_details: HyperswitchPaymentMethodIdRecurringDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HyperswitchRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let amount = item
            .connector
            .amount_converter
            .convert(&router_data.request.money)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "hyperswitch repeat payment: failed to convert amount to minor units"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        // The vaulted payment-method id comes from the mandate reference
        // established by a prior SetupMandate (the connector maps Hyperswitch's
        // `payment_method_id` into `connector_mandate_id`). Only a connector
        // mandate id is supported here.
        let payment_method_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate) => connector_mandate
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotImplemented(
                    "hyperswitch repeat payment only supports connector mandate id references"
                        .to_string(),
                    Default::default(),
                )
                .into())
            }
        };

        // Hyperswitch requires the owning `customer_id` to charge a vaulted
        // payment method off-session. Mirror SetupMandate's resolution.
        let customer_id = router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "customer_id",
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            confirm: true,
            off_session: true,
            customer_id,
            recurring_details: HyperswitchPaymentMethodIdRecurringDetails {
                recurring_type: "payment_method_id".to_string(),
                data: payment_method_id,
            },
        })
    }
}

// =============================================================================
// RESPONSE: PAYMENTS (PSync / RepeatPayment)
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchPaymentsResponse {
    pub payment_id: String,
    pub status: HyperswitchIntentStatus,
    pub connector_transaction_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

// Distinct identifiers per flow so the macro generates unique `*Templating`
// marker types; all resolve to the same concrete response struct, so the
// `TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, _>>` impls apply.
pub type HyperswitchPSyncResponse = HyperswitchPaymentsResponse;
pub type HyperswitchRepeatPaymentResponse = HyperswitchPaymentsResponse;

pub(crate) fn map_intent_status(
    status: &HyperswitchIntentStatus,
    _is_auto_capture: bool,
) -> common_enums::AttemptStatus {
    match status {
        // Hyperswitch returns `requires_capture` (not `succeeded`) for an
        // authorized-but-uncaptured manual-capture payment, so `succeeded`
        // always means a charge has completed.
        HyperswitchIntentStatus::Succeeded => common_enums::AttemptStatus::Charged,
        HyperswitchIntentStatus::Failed
        | HyperswitchIntentStatus::Conflicted
        | HyperswitchIntentStatus::Expired => common_enums::AttemptStatus::Failure,
        HyperswitchIntentStatus::Cancelled | HyperswitchIntentStatus::CancelledPostCapture => {
            common_enums::AttemptStatus::Voided
        }
        HyperswitchIntentStatus::Processing
        | HyperswitchIntentStatus::PartiallyCapturedAndProcessing
        | HyperswitchIntentStatus::RequiresMerchantAction => common_enums::AttemptStatus::Pending,
        HyperswitchIntentStatus::RequiresCapture
        | HyperswitchIntentStatus::PartiallyAuthorizedAndRequiresCapture => {
            common_enums::AttemptStatus::Authorized
        }
        HyperswitchIntentStatus::PartiallyCaptured => common_enums::AttemptStatus::PartialCharged,
        HyperswitchIntentStatus::PartiallyCapturedAndCapturable => {
            common_enums::AttemptStatus::PartialChargedAndChargeable
        }
        HyperswitchIntentStatus::RequiresPaymentMethod => {
            common_enums::AttemptStatus::PaymentMethodAwaited
        }
        HyperswitchIntentStatus::RequiresConfirmation => {
            common_enums::AttemptStatus::ConfirmationAwaited
        }
        HyperswitchIntentStatus::RequiresCustomerAction => {
            common_enums::AttemptStatus::AuthenticationPending
        }
        HyperswitchIntentStatus::Unknown => common_enums::AttemptStatus::Unknown,
    }
}

fn build_payments_response(
    response: HyperswitchPaymentsResponse,
    http_code: u16,
    status: common_enums::AttemptStatus,
) -> Result<PaymentsResponseData, ErrorResponse> {
    if status == common_enums::AttemptStatus::Failure {
        Err(ErrorResponse {
            code: response
                .error_code
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            message: response
                .error_message
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: response.error_message.clone(),
            attempt_status: None,
            connector_transaction_id: Some(response.payment_id.clone()),
            status_code: http_code,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: response.connector_transaction_id.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(response.payment_id.clone()),
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let is_auto_capture = router_data.request.is_auto_capture();
        let status = map_intent_status(&response.status, is_auto_capture);
        let payment_response = build_payments_response(response, http_code, status);
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let is_auto_capture = !matches!(
            router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Manual)
        );
        let status = map_intent_status(&response.status, is_auto_capture);
        let payment_response = build_payments_response(response, http_code, status);
        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchErrorResponse {
    pub error: HyperswitchErrorDetail,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchErrorDetail {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub message: Option<String>,
    // Hyperswitch always returns an error `code` in its error envelope; keeping
    // it mandatory ensures this struct doesn't match arbitrary JSON objects.
    pub code: String,
}
