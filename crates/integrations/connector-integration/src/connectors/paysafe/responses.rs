use common_utils::types::MinorUnit;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentsResponse {
    pub id: String,
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    pub available_to_settle: Option<MinorUnit>,
    pub currency_code: common_enums::Currency,
    pub status: PaysafePaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_handle_token: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_reconciliation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafePaymentStatus {
    Completed,
    #[default]
    Processing,
    Failed,
    Cancelled,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentHandleResponse {
    pub id: String,
    pub merchant_ref_num: String,
    pub payment_handle_token: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<PaysafeUsage>,
    pub status: PaysafePaymentHandleStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<PaymentLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaysafeUsage {
    SingleUse,
    MultiUse,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafePaymentHandleStatus {
    Initiated,
    Payable,
    #[default]
    Processing,
    Failed,
    Expired,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentLink {
    pub rel: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PaysafeSyncResponse {
    // Single payment response (GET /v1/payments/{id})
    SinglePayment(PaysafePaymentsResponse),
    // Multiple payments response (GET /v1/payments?merchantRefNum={})
    Payments(PaysafePaymentsSyncData),
    // Single payment handle response (GET /v1/paymenthandles/{id})
    SinglePaymentHandle(PaysafePaymentHandleResponse),
    // Multiple payment handles response (GET /v1/paymenthandles?merchantRefNum={})
    PaymentHandle(PaysafePaymentHandleSyncData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentsSyncData {
    pub payments: Vec<PaysafePaymentsResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentHandleSyncData {
    pub payment_handles: Vec<PaysafePaymentHandleResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeSettlementResponse {
    pub id: String,
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    pub status: PaysafeSettlementStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeSettlementStatus {
    #[default]
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeVoidResponse {
    pub id: String,
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    pub status: PaysafeVoidStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeVoidStatus {
    #[default]
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeRefundResponse {
    pub id: String,
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    pub status: PaysafeRefundStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeRefundStatus {
    #[default]
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

// RSync uses the same response structure as Refund
pub type PaysafeRSyncResponse = PaysafeRefundResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<String>>,
    /// Paysafe serializes this as `fieldErrors`; without the camelCase rename
    /// the per-field diagnostics were silently dropped on deserialization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_errors: Option<Vec<FieldError>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaysafeErrorResponse {
    pub error: Error,
}

/// CreateConnectorCustomer response body (`POST v1/customers`).
///
/// `id` is the Paysafe customer id (used as the connector_customer_id and later
/// as the `{customerId}` path segment when minting a reusable payment handle).
/// Mirrors hyperswitch's `PaysafeCustomerResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeCustomerResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_customer_id: Option<String>,
}

/// Authorize-flow response.
///
/// Non-redirect payment methods (card no-3DS, Apple Pay, and settlements of an
/// existing payment handle) POST to `v1/payments` and return a
/// [`PaysafePaymentsResponse`]. Redirect APMs (Skrill, Interac e-Transfer,
/// paysafecard) instead POST to `v1/paymenthandles` and return a
/// [`PaysafePaymentHandleResponse`] carrying the customer redirect `links`.
///
/// The variants are discriminated structurally (`untagged`): only
/// `PaysafePaymentsResponse` has the required `amount` + `currencyCode` fields, so
/// a payment-handle body (which lacks them) cannot match `Payment` and falls
/// through to `PaymentHandle`. `Payment` is listed first so a genuine payments
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PaysafeAuthorizeResponse {
    Payment(PaysafePaymentsResponse),
    PaymentHandle(PaysafePaymentHandleResponse),
}

// Type aliases for flows
pub type PaysafePaymentMethodTokenResponse = PaysafePaymentHandleResponse;
/// PreAuthenticate returns a payment-handle body (same shape as Authorize's handle response),
/// but needs a distinct type name so the connector macro generates a separate response-templating
/// type instead of colliding with the Authorize one.
pub type PaysafePreAuthenticateResponse = PaysafeAuthorizeResponse;
/// Authenticate re-fetches the payment handle by merchantRefNum (same GET + shape as PSync) to
/// recover the paymentHandleToken after the ACS redirect. Distinct name so the macro generates a
/// separate response-templating type instead of colliding with PSync's.
pub type PaysafeAuthenticateResponse = PaysafeSyncResponse;
pub type PaysafeCaptureResponse = PaysafeSettlementResponse;
pub type PaysafeRepeatPaymentResponse = PaysafePaymentsResponse;
