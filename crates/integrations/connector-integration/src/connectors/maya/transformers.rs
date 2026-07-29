use common_utils::{types::StringMajorUnit, Method};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    errors::{self, ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{connectors::maya::MayaRouterData, types::ResponseRouterData};

#[derive(Debug, Clone)]
pub struct MayaAuthType {
    pub public_key: Secret<String>,
    pub secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MayaAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Maya {
                public_key,
                secret_key,
                ..
            } => Ok(Self {
                public_key: public_key.to_owned(),
                secret_key: secret_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MayaErrorParameter {
    pub description: String,
    pub field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MayaErrorResponse {
    pub code: String,
    /// Maya uses both `message` (payment errors) and `error` (auth/endpoint errors)
    /// for the human-readable description.
    #[serde(alias = "error")]
    pub message: String,
    #[serde(default)]
    pub parameters: Option<Vec<MayaErrorParameter>>,
    /// Optional correlation/reference id returned by some Maya error responses.
    #[serde(default)]
    pub reference: Option<String>,
}

// =============================================================================
// AUTHORIZE FLOW TYPES AND TRANSFORMERS
// =============================================================================

/// Maya "Create Single Payment" request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaPaymentsRequest {
    pub total_amount: MayaTotalAmount,
    pub redirect_url: MayaRedirectUrl,
    pub request_reference_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct MayaTotalAmount {
    pub value: StringMajorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
pub struct MayaRedirectUrl {
    pub success: String,
    pub failure: String,
    pub cancel: String,
}

/// Maya "Create Single Payment" response body.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaPaymentsResponse {
    pub payment_id: String,
    pub redirect_url: String,
}

/// Maya payment / webhook status values (used by PSync / webhooks).
///
/// Includes both the documented payment-object statuses and the webhook event
/// names returned by Maya (e.g. `CHECKOUT_DROPOUT`, `3DS_PAYMENT_SUCCESS`).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MayaPaymentStatus {
    PendingToken,
    PendingPayment,
    ForAuthentication,
    Authenticating,
    AuthSuccess,
    AuthFailed,
    PaymentProcessing,
    PaymentSuccess,
    PaymentFailed,
    PaymentExpired,
    PaymentCancelled,
    Voided,
    Refunded,
    // Webhook-specific event names. Explicit renames are required for values
    // that contain numbers (3DS) or do not round-trip cleanly through serde's
    // SCREAMING_SNAKE_CASE conversion.
    #[serde(rename = "CHECKOUT_DROPOUT")]
    CheckOutDropout,
    #[serde(rename = "3DS_PAYMENT_FAILURE")]
    ThreeDsPaymentFailure,
    #[serde(rename = "3DS_PAYMENT_DROPOUT")]
    ThreeDsPaymentDropout,
    #[serde(rename = "CHECKOUT_SUCCESS")]
    CheckOutSuccess,
    #[serde(rename = "CHECKOUT_FAILURE")]
    CheckOutFailure,
    #[serde(rename = "3DS_PAYMENT_SUCCESS")]
    ThreeDsPaymentSuccess,
}

impl From<MayaPaymentStatus> for common_enums::AttemptStatus {
    fn from(status: MayaPaymentStatus) -> Self {
        match status {
            MayaPaymentStatus::PendingToken | MayaPaymentStatus::PendingPayment => {
                Self::PaymentMethodAwaited
            }
            MayaPaymentStatus::ForAuthentication | MayaPaymentStatus::Authenticating => {
                Self::AuthenticationPending
            }
            MayaPaymentStatus::AuthSuccess | MayaPaymentStatus::ThreeDsPaymentSuccess => {
                Self::AuthenticationSuccessful
            }
            MayaPaymentStatus::AuthFailed
            | MayaPaymentStatus::ThreeDsPaymentFailure
            | MayaPaymentStatus::ThreeDsPaymentDropout
            | MayaPaymentStatus::CheckOutDropout => Self::AuthenticationFailed,
            MayaPaymentStatus::PaymentProcessing | MayaPaymentStatus::CheckOutSuccess => {
                Self::Authorizing
            }
            MayaPaymentStatus::PaymentSuccess => Self::Charged,
            MayaPaymentStatus::PaymentFailed | MayaPaymentStatus::CheckOutFailure => {
                Self::AuthorizationFailed
            }
            MayaPaymentStatus::PaymentCancelled | MayaPaymentStatus::Voided => Self::Voided,
            MayaPaymentStatus::PaymentExpired => Self::Expired,
            MayaPaymentStatus::Refunded => Self::AutoRefunded,
        }
    }
}

impl From<MayaPaymentStatus> for EventType {
    fn from(status: MayaPaymentStatus) -> Self {
        match status {
            MayaPaymentStatus::PaymentSuccess => Self::PaymentIntentSuccess,
            MayaPaymentStatus::AuthSuccess | MayaPaymentStatus::ThreeDsPaymentSuccess => {
                Self::PaymentIntentAuthorizationSuccess
            }
            MayaPaymentStatus::PaymentCancelled | MayaPaymentStatus::Voided => {
                Self::PaymentIntentCancelled
            }
            MayaPaymentStatus::PaymentExpired => Self::PaymentIntentExpired,
            MayaPaymentStatus::PaymentProcessing
            | MayaPaymentStatus::CheckOutSuccess
            | MayaPaymentStatus::PendingToken
            | MayaPaymentStatus::PendingPayment
            | MayaPaymentStatus::ForAuthentication
            | MayaPaymentStatus::Authenticating => Self::PaymentIntentProcessing,
            MayaPaymentStatus::PaymentFailed
            | MayaPaymentStatus::CheckOutFailure
            | MayaPaymentStatus::AuthFailed
            | MayaPaymentStatus::ThreeDsPaymentFailure
            | MayaPaymentStatus::CheckOutDropout
            | MayaPaymentStatus::ThreeDsPaymentDropout => Self::PaymentIntentFailure,
            MayaPaymentStatus::Refunded => Self::RefundSuccess,
        }
    }
}

// =============================================================================
// WEBHOOK TYPES AND HELPERS
// =============================================================================

/// Pay-with-Maya payment object.
///
/// Used for both incoming webhooks and the response from the payment-by-ID
/// endpoint (`GET /payments/v1/payments/{paymentId}`).
///
/// Only the fields relevant to the Pay-with-Maya redirect flow are modeled;
/// Checkout / Payment-Facilitator / Vault fields are intentionally omitted.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MayaWebhookBody {
    pub id: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub payment_status: Option<String>,
    #[serde(default)]
    pub request_reference_number: Option<String>,
    #[serde(default)]
    pub transaction_reference_number: Option<String>,
    #[serde(default)]
    pub receipt_number: Option<String>,
    #[serde(default)]
    pub approval_code: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub can_void: Option<bool>,
    #[serde(default)]
    pub can_refund: Option<bool>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

impl MayaWebhookBody {
    /// Effective payment status string.
    ///
    /// Prefers the explicit `paymentStatus` field, falling back to `status`.
    pub fn payment_status(&self) -> Option<&str> {
        self.payment_status.as_deref().or(self.status.as_deref())
    }

    /// Merchant reference echoed by Maya, distinct from Maya's payment `id`.
    pub fn connector_response_reference_id(&self) -> Option<String> {
        self.request_reference_number
            .clone()
            .or_else(|| self.transaction_reference_number.clone())
    }
}

// =============================================================================
// VOID FLOW TYPES AND TRANSFORMERS
// =============================================================================

/// Maya "Void Payment" request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaVoidRequest {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_reference_number: Option<String>,
}

/// Maya void status values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MayaVoidStatus {
    Success,
    Failed,
    Pending,
}

impl From<MayaVoidStatus> for common_enums::AttemptStatus {
    fn from(status: MayaVoidStatus) -> Self {
        match status {
            MayaVoidStatus::Success => Self::Voided,
            MayaVoidStatus::Failed => Self::Failure,
            MayaVoidStatus::Pending => Self::Pending,
        }
    }
}

/// Maya "Void Payment" response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaVoidResponse {
    pub id: String,
    #[serde(default)]
    pub status: Option<MayaVoidStatus>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub request_reference_number: Option<String>,
    #[serde(default)]
    pub payment: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MayaRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for MayaVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MayaRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        Ok(Self {
            reason: "Customer request".to_string(),
            request_reference_number: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MayaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MayaPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MayaRouterData<
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

        let value = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount to Maya major unit")?;

        let return_url = router_data.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            },
        )?;

        let user_id = router_data
            .request
            .email
            .clone()
            .map(|email| email.peek().to_string());

        Ok(Self {
            total_amount: MayaTotalAmount {
                value,
                currency: router_data.request.currency,
            },
            redirect_url: MayaRedirectUrl {
                success: return_url.clone(),
                failure: return_url.clone(),
                cancel: return_url,
            },
            request_reference_number: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            user_id,
            metadata: router_data.request.metadata.clone().expose_option(),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<MayaPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let redirect_url = Url::parse(&item.response.redirect_url).change_context(
            ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            },
        )?;

        let status = common_enums::AttemptStatus::Pending;

        // Maya returns only `paymentId` and `redirectUrl` in the create-payment response.
        // The merchant-facing reference is the `requestReferenceNumber` we sent in the
        // request (`connector_request_reference_id`). Surface it as the connector
        // response reference id so it stays consistent across Authorize / PSync / Webhook.
        let connector_response_reference_id = Some(
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: Some(Box::new(RedirectForm::from((redirect_url, Method::Get)))),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id,
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

impl TryFrom<ResponseRouterData<MayaWebhookBody, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaWebhookBody, Self>) -> Result<Self, Self::Error> {
        // The Maya paymentId from the request, used as a fallback for `resource_id`
        // when the response does not carry one.
        let request_txn_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .ok();
        let connector_request_reference_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let payment = item.response;

        // Effective status: prefer `paymentStatus`, fall back to `status`
        // (same logic as the webhook body).
        let effective_status = payment.payment_status().unwrap_or("PAYMENT_FAILED");
        let maya_payment_status: MayaPaymentStatus =
            serde_json::from_str(&format!("\"{effective_status}\"")).change_context(
                ConnectorError::response_deserialization_failed(item.http_code),
            )?;
        let status = common_enums::AttemptStatus::from(maya_payment_status);

        // `resource_id`: Maya paymentId (`id`) from the response if available, falling
        // back to the request's `connector_transaction_id`.
        let resource_id_value = if !payment.id.is_empty() {
            payment.id.clone()
        } else {
            request_txn_id.unwrap_or_default()
        };

        // `connector_response_reference_id`: the merchant reference number echoed back
        // by Maya (`requestReferenceNumber`). This keeps the reference consistent with
        // what Euler/Juspay uses as `txn_id`.
        let connector_response_reference_id = payment
            .connector_response_reference_id()
            .unwrap_or(connector_request_reference_id);

        // Expose Maya's `canVoid` / `canRefund` flags so downstream services can
        // decide whether void/refund operations are currently allowed.
        let connector_metadata = {
            let mut map = serde_json::Map::new();
            if let Some(can_void) = payment.can_void {
                map.insert("canVoid".to_string(), serde_json::json!(can_void));
            }
            if let Some(can_refund) = payment.can_refund {
                map.insert("canRefund".to_string(), serde_json::json!(can_refund));
            }
            if map.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(map))
            }
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(resource_id_value),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: Some(connector_response_reference_id),
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

impl TryFrom<ResponseRouterData<MayaVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = item
            .response
            .status
            .map(common_enums::AttemptStatus::from)
            .unwrap_or(common_enums::AttemptStatus::Voided);
        let connector_transaction_id = item.router_data.request.connector_transaction_id.clone();
        let void_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(void_id),
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

// =============================================================================
// REFUND FLOW TYPES AND TRANSFORMERS
// =============================================================================

/// Maya refund status values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MayaRefundStatus {
    Success,
    Failed,
    Pending,
}

impl From<MayaRefundStatus> for common_enums::RefundStatus {
    fn from(status: MayaRefundStatus) -> Self {
        match status {
            MayaRefundStatus::Success => Self::Success,
            MayaRefundStatus::Failed => Self::Failure,
            MayaRefundStatus::Pending => Self::Pending,
        }
    }
}

/// Maya "Refund Payment" request body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaRefundRequest {
    pub total_amount: MayaTotalAmount,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_reference_number: Option<String>,
}

/// Maya "Refund Payment" response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MayaRefundResponse {
    pub id: String,
    #[serde(default)]
    pub status: Option<MayaRefundStatus>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub request_reference_number: Option<String>,
    #[serde(default)]
    pub payment: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Type alias for the refund-sync flow.
///
/// `create_all_prerequisites!` generates a unique templating struct per
/// response type, so reusing the same type for both `Refund` and `RSync`
/// causes a duplicate-definition error. This alias points to the same
/// underlying deserialization logic while satisfying the macro.
pub type MayaRefundSyncResponse = MayaRefundResponse;

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<MayaRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for MayaRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MayaRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let value = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert refund amount to Maya major unit")?;

        let reason = router_data
            .request
            .reason
            .unwrap_or_else(|| "Customer request".to_string());

        Ok(Self {
            total_amount: MayaTotalAmount {
                value,
                currency: router_data.request.currency,
            },
            reason,
            request_reference_number: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
        })
    }
}

impl TryFrom<ResponseRouterData<MayaRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<MayaRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = item
            .response
            .status
            .map(common_enums::RefundStatus::from)
            .unwrap_or(common_enums::RefundStatus::Success);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<MayaRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MayaRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item
            .response
            .status
            .map(common_enums::RefundStatus::from)
            .unwrap_or(common_enums::RefundStatus::Pending);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[cfg(test)]
mod tests {
    use super::MayaWebhookBody;

    #[test]
    fn payment_id_and_merchant_reference_remain_distinct() {
        let payment: MayaWebhookBody = serde_json::from_value(serde_json::json!({
            "id": "c0ea08ca-9344-45a4-be41-ee03665bbe13",
            "status": "PAYMENT_SUCCESS",
            "requestReferenceNumber": "azharamin-1785229582-1"
        }))
        .expect("Maya payment response must deserialize");

        assert_eq!(payment.id, "c0ea08ca-9344-45a4-be41-ee03665bbe13");
        assert_eq!(
            payment.connector_response_reference_id().as_deref(),
            Some("azharamin-1785229582-1")
        );
    }

    #[test]
    fn transaction_reference_number_is_used_as_merchant_reference_fallback() {
        let payment: MayaWebhookBody = serde_json::from_value(serde_json::json!({
            "id": "maya-payment-id",
            "status": "PAYMENT_SUCCESS",
            "transactionReferenceNumber": "merchant-transaction-id"
        }))
        .expect("Maya payment response must deserialize");

        assert_eq!(
            payment.connector_response_reference_id().as_deref(),
            Some("merchant-transaction-id")
        );
    }
}
