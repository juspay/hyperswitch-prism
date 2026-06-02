use serde::{Deserialize, Serialize};

/// Top-level TransIT status flag (tech spec § Status Mappings).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysXmlStatus {
    Pass,
    Fail,
}

/// Authorize response envelope — covers both `<SaleResponse>` and `<AuthResponse>`.
///
/// quick_xml does not natively project two root names onto a single struct via
/// `#[serde(untagged)]` on a struct; instead we accept either root via an enum
/// wrapper, then merge into the same body shape (their field schemas are
/// identical per tech spec § 2).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum TsysXmlAuthorizeResponse {
    SaleResponse(TsysXmlAuthorizeBody),
    AuthResponse(TsysXmlAuthorizeBody),
}

impl TsysXmlAuthorizeResponse {
    pub fn body(&self) -> &TsysXmlAuthorizeBody {
        match self {
            Self::SaleResponse(b) | Self::AuthResponse(b) => b,
        }
    }
}

/// RepeatPayment response — identical wire shape to `TsysXmlAuthorizeResponse`.
/// The newtype keeps the macro-generated `Templating` registration distinct
/// from the Authorize flow so the same response body can be consumed by both
/// flows without conflicting `BridgeRequestResponse` impls.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum TsysXmlRepeatPaymentResponse {
    SaleResponse(TsysXmlAuthorizeBody),
    AuthResponse(TsysXmlAuthorizeBody),
}

impl TsysXmlRepeatPaymentResponse {
    pub fn body(&self) -> &TsysXmlAuthorizeBody {
        match self {
            Self::SaleResponse(b) | Self::AuthResponse(b) => b,
        }
    }
    pub fn as_authorize(&self) -> TsysXmlAuthorizeResponse {
        match self {
            Self::SaleResponse(b) => TsysXmlAuthorizeResponse::SaleResponse(b.clone()),
            Self::AuthResponse(b) => TsysXmlAuthorizeResponse::AuthResponse(b.clone()),
        }
    }
}

/// Shared body for `<SaleResponse>` / `<AuthResponse>` (tech spec § Sale/Auth response).
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct TsysXmlAuthorizeBody {
    #[serde(rename = "status", default)]
    pub status: Option<TsysXmlStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
    #[serde(rename = "authCode", default)]
    pub auth_code: Option<String>,
    #[serde(rename = "hostReferenceNumber", default)]
    pub host_reference_number: Option<String>,
    #[serde(rename = "hostResponseCode", default)]
    pub host_response_code: Option<String>,
    #[serde(rename = "taskID", default)]
    pub task_id: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "transactionTimestamp", default)]
    pub transaction_timestamp: Option<String>,
    #[serde(rename = "transactionAmount", default)]
    pub transaction_amount: Option<String>,
    #[serde(rename = "processedAmount", default)]
    pub processed_amount: Option<String>,
    #[serde(rename = "totalAmount", default)]
    pub total_amount: Option<String>,
    #[serde(rename = "addressVerificationCode", default)]
    pub address_verification_code: Option<String>,
    #[serde(rename = "cvvVerificationCode", default)]
    pub cvv_verification_code: Option<String>,
    #[serde(rename = "cardType", default)]
    pub card_type: Option<String>,
    #[serde(rename = "maskedCardNumber", default)]
    pub masked_card_number: Option<String>,
}

/// Lifecycle state of a transaction as reported by TransIT (tech spec § PSync).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysXmlTransactionState {
    Authorized,
    Captured,
    Settled,
    Voided,
    Returned,
}

/// TransIT Capture response (tech spec § Capture response).
///
/// Roots at `<CaptureResponse>`. Status mapping per tech spec § Status Mappings
/// is handled in the transformer (`map_capture_status`).
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "CaptureResponse")]
pub struct TsysXmlCaptureResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysXmlStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}

/// TransIT Return (Refund) response (tech spec § Return response).
///
/// Roots at `<ReturnResponse>`. Status mapping per tech spec § Status Mappings
/// is handled in the transformer (`map_refund_status`).
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "ReturnResponse")]
pub struct TsysXmlReturnResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysXmlStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}

/// TransIT Void response (tech spec § Void response).
///
/// Roots at `<VoidResponse>`. Status mapping per tech spec § Status Mappings
/// is handled in the transformer (`map_void_status`).
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "VoidResponse")]
pub struct TsysXmlVoidResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysXmlStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}

/// `<walletDetails>` block on `<AddCustomerResponse>` — exposes the walletID
/// that we stash alongside the customerCode to drive Path B Authorize.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct TsysXmlAddCustomerWalletDetails {
    #[serde(rename = "walletID", default)]
    pub wallet_id: Option<String>,
    #[serde(rename = "maskedCardNumber", default)]
    pub masked_card_number: Option<String>,
    #[serde(rename = "externalWalletReferenceID", default)]
    pub external_wallet_reference_id: Option<String>,
}

/// TransIT `<AddCustomerResponse>` — CreateConnectorCustomer response.
/// We surface `customerCode` as `connector_customer_id` and stash the
/// `walletID` on `PaymentFlowData.connector_response_reference_id` so a
/// downstream Authorize can encode the Path B `cust:CCC:WWW` mandate id.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "AddCustomerResponse")]
pub struct TsysXmlAddCustomerResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysXmlStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
    #[serde(rename = "customerCode", default)]
    pub customer_code: Option<String>,
    #[serde(rename = "walletDetails", default)]
    pub wallet_details: Option<TsysXmlAddCustomerWalletDetails>,
}

/// TransIT `<CardAuthenticationResponse>` — SetupMandate response. Mirrors
/// the Sale/Auth response shape (PASS/FAIL + responseCode + transactionID),
/// plus `cardTransactionIdentifier` which we use as the mandate's
/// network-token id (NTID) for Path A.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "CardAuthenticationResponse")]
pub struct TsysXmlCardAuthenticationResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysXmlStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "cardTransactionIdentifier", default)]
    pub card_transaction_identifier: Option<String>,
    #[serde(rename = "authCode", default)]
    pub auth_code: Option<String>,
}

/// RSync response — reuses the PSync inquiry response shape via a type alias.
/// TransIT's `<TransactionInquiry>` endpoint serves both payment and refund
/// status lookups; the alias keeps the macro layer's Templating types
/// distinct from PSync while sharing the same on-wire schema. The transformer
/// (`map_rsync_status`) interprets the same `<transactionState>` differently
/// for refunds.
pub type TsysXmlRSyncResponse = TsysXmlTransactionInquiryResponse;

/// PSync response envelope.
///
/// TODO(tsys_xml): UNDECIDED - confirm element name with TSYS once API
/// behaviour is validated end-to-end.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "TransactionInquiryResponse")]
pub struct TsysXmlTransactionInquiryResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysXmlStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "transactionState", default)]
    pub transaction_state: Option<TsysXmlTransactionState>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}
