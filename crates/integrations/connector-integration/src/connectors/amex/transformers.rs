use crate::types::ResponseRouterData;
use common_enums::AttemptStatus;
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AmexAuthType {
    pub merchant_id: Secret<String>,
    pub shared_secret: Secret<String>,
}

// Auth is read from `ConnectorSpecificConfig`, NOT from a `connector_auth_type`
// field on RouterDataV2 - that field was deleted from RouterDataV2. The variant
// below is the one add_connector.sh appends to
// `domain_types::router_data::ConnectorSpecificConfig`.
impl TryFrom<&ConnectorSpecificConfig> for AmexAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Amex {
                merchant_id,
                shared_secret,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                shared_secret: shared_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// Every field is `Option` on purpose: a vendor error body that omits `code` or
// `message` must still deserialize. The NO_ERROR_CODE / NO_ERROR_MESSAGE
// sentinels are applied at the call site in the connector's
// `build_error_response` - never `.unwrap_or_default()`, which turns a missing
// code into an empty string indistinguishable from a real empty code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmexErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AmexPaymentsRequest {
    pub amount: i64,
    pub currency: String,
    pub reference: String,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for AmexPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.request.money.amount().get_amount_as_i64(),
            currency: item.request.currency.to_string(),
            reference: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

// ===== DESERIALIZATION LAYER =====
// Model the vendor's status as a typed enum, not a bare `String`.
// `#[serde(other)] Unknown` is REQUIRED: without it, the first status string the
// vendor adds becomes a hard deserialization failure and the payment is lost.
// Reviewers demand BOTH halves - this arm here, AND an exhaustive match (no
// `_ =>`) at the status-mapping layer below.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmexPaymentStatus {
    Succeeded,
    Processing,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

// ===== STATUS-MAPPING LAYER =====
// Deliberately exhaustive: NO `_ =>` catch-all. When the vendor adds a status
// and you add it to the enum above, this match must fail to compile so a human
// classifies it, instead of a new terminal state silently collapsing to Pending.
impl From<AmexPaymentStatus> for AttemptStatus {
    fn from(status: AmexPaymentStatus) -> Self {
        match status {
            AmexPaymentStatus::Succeeded => Self::Charged,
            AmexPaymentStatus::Processing => Self::Pending,
            AmexPaymentStatus::Failed => Self::Failure,
            AmexPaymentStatus::Cancelled => Self::Voided,
            // Unknown is NOT terminal - keep it pollable rather than declaring
            // a payment dead on a status string nobody has classified yet.
            AmexPaymentStatus::Unknown => Self::Pending,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AmexPaymentsResponse {
    pub id: String,
    pub status: AmexPaymentStatus,
    pub amount: i64,
    pub currency: String,
    // In-band decline detail, if the vendor puts it on the 200 body.
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        ResponseRouterData<
            AmexPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            AmexPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        // IN-BAND 2xx FAILURE.
        // Many gateways return HTTP 200 with a declined status in the body.
        // `build_error_response` never sees those, so the decline must be turned
        // into `Err(ErrorResponse { .. })` HERE or a hard decline is reported as
        // a successful call. Predicate:
        // `domain_types::utils::is_payment_failure`
        // (crates/types-traits/domain_types/src/utils.rs).
        let response = if domain_types::utils::is_payment_failure(status) {
            Err(ErrorResponse {
                code: item
                    .response
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .error_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.error_message.clone(),
                status_code: item.http_code,
                // Terminal here is correct: `status` came from the connector's
                // OWN response and `is_payment_failure` already proved it is a
                // failure state. Do NOT copy this into `build_error_response`,
                // which is shared by every flow.
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id.clone()),
                // ErrorResponse has 13 fields and an `impl Default`
                // (crates/types-traits/domain_types/src/router_data.rs).
                ..Default::default()
            })
        } else {
            // `PaymentsResponseData::TransactionResponse` is an enum
            // struct-variant, so `..Default::default()` is NOT allowed - every
            // one of its 11 fields must be listed or it is E0063. Field list:
            // crates/types-traits/domain_types/src/connector_types.rs.
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
