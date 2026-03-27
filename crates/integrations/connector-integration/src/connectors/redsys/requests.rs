use common_utils::{pii, StringMinorUnit};
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

pub type RedsysPreAuthenticateRequest = super::transformers::RedsysTransaction;
pub type RedsysAuthenticateRequest = super::transformers::RedsysTransaction;
pub type RedsysAuthorizeRequest = super::transformers::RedsysTransaction;
pub type RedsysCaptureRequest = super::transformers::RedsysTransaction;
pub type RedsysVoidRequest = super::transformers::RedsysTransaction;
pub type RedsysRefundRequest = super::transformers::RedsysTransaction;

/// Main payment request structure for Redsys API
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct RedsysPaymentRequest {
    pub ds_merchant_amount: StringMinorUnit,
    pub ds_merchant_currency: String,
    pub ds_merchant_cvv2: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_emv3ds: Option<RedsysEmvThreeDsRequestData>,
    pub ds_merchant_expirydate: Secret<String>,
    pub ds_merchant_merchantcode: Secret<String>,
    pub ds_merchant_order: String,
    pub ds_merchant_pan: cards::CardNumber,
    pub ds_merchant_terminal: Secret<String>,
    pub ds_merchant_transactiontype: RedsysTransactionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_excep_sca: Option<RedsysStrongCustomerAuthenticationException>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_directpayment: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RedsysStrongCustomerAuthenticationException {
    Lwv,
    Tra,
    Cor,
    Mit,
    Atd,
}

#[derive(Debug)]
pub struct RedsysCardData<T: PaymentMethodDataTypes> {
    pub card_number: RawCardNumber<T>,
    pub cvv2: Secret<String>,
    pub expiry_date: Secret<String>,
}

/// Transaction types supported by Redsys
#[derive(Debug, Serialize, Deserialize)]
pub enum RedsysTransactionType {
    /// Standard payment (auto capture)
    #[serde(rename = "0")]
    Payment,
    /// Preauthorization (manual capture required)
    #[serde(rename = "1")]
    Preauthorization,
    /// Confirmation of preauthorization (capture)
    #[serde(rename = "2")]
    Confirmation,
    /// Refund
    #[serde(rename = "3")]
    Refund,
    /// Cancellation (void)
    #[serde(rename = "9")]
    Cancellation,
}

/// EMV 3DS request data for 3D Secure authentication
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysEmvThreeDsRequestData {
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub billing_data: Option<RedsysBillingData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_accept_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_color_depth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_i_p: Option<Secret<String, pii::IpAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_javascript_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_height: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_t_z: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_user_agent: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cres: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_u_r_l: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub shipping_data: Option<RedsysShippingData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_s_comp_ind: Option<RedsysThreeDSCompInd>,
    pub three_d_s_info: RedsysThreeDsInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_s_server_trans_i_d: Option<String>,
}

/// Billing address data for 3DS
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysBillingData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_line3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_addr_state: Option<Secret<String>>,
}

/// Shipping address data for 3DS
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysShippingData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_line3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_addr_state: Option<Secret<String>>,
}

/// 3DS information type for different stages of authentication
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum RedsysThreeDsInfo {
    /// Initial card data collection
    CardData,
    /// Card configuration check
    CardConfiguration,
    /// Challenge request
    ChallengeRequest,
    /// Challenge response submission
    ChallengeResponse,
    /// Final authentication data
    AuthenticationData,
}

/// Indicates whether the 3DSMethod has been executed
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RedsysThreeDSCompInd {
    /// N = Completed with errors
    N,
    /// U = 3DSMethod not executed
    U,
    /// Y = Completed successfully
    Y,
}

/// Operation request for capture, void, and refund operations
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct RedsysOperationRequest {
    pub ds_merchant_amount: StringMinorUnit,
    // Redsys uses numeric ISO 4217 currency codes (e.g., "978" for EUR)
    // not 3-letter codes, so we use String here
    pub ds_merchant_currency: String,
    pub ds_merchant_merchantcode: Secret<String>,
    pub ds_merchant_order: String,
    pub ds_merchant_terminal: Secret<String>,
    pub ds_merchant_transactiontype: RedsysTransactionType,
}

/// SOAP XML messages container for sync operations
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Messages {
    pub version: RedsysVersionData,
    pub signature: String,
    pub signature_version: String,
}

/// Version wrapper for SOAP sync request
#[derive(Debug, Serialize)]
#[serde(rename = "Version")]
pub struct RedsysVersionData {
    #[serde(rename = "@Ds_Version")]
    pub ds_version: String,
    #[serde(rename = "Message")]
    pub message: Message,
}

/// Message wrapper containing the actual message type
///
/// Note: Uses Transaction or Monitor based on transaction_type parameter in construct_sync_request()
/// - Transaction: Filters by Ds_TransactionType
/// - Monitor: Returns all transaction types
///
/// Both use same field ordering as simple variants (not Masiva).
#[derive(Debug, Serialize)]
#[serde(rename = "Message")]
pub struct Message {
    #[serde(flatten)]
    pub content: MessageContent,
}

/// The actual message content (Transaction or Monitor)
#[derive(Debug, Serialize)]
pub enum MessageContent {
    #[serde(rename = "Transaction")]
    Transaction(RedsysTransactionRequest),
    #[serde(rename = "Monitor")]
    Monitor(RedsysMonitorRequest),
}

/// SOAP XML Transaction request for querying transaction status
///
/// CRITICAL: Field ordering must match Redsys DTD exactly.
/// Alphabetical sorting will cause XML0001 error (DTD validation failure).
///
/// Required DTD order: Ds_MerchantCode → Ds_Terminal → Ds_Order → Ds_TransactionType
///
/// Ref: RS.TE.CEL.MAN.0021 v1.4, Section 3.2.1 (Transaction simple)
#[derive(Debug, Serialize)]
pub struct RedsysTransactionRequest {
    #[serde(rename = "Ds_MerchantCode")]
    pub ds_merchant_code: Secret<String>,
    #[serde(rename = "Ds_Terminal")]
    pub ds_terminal: Secret<String>,
    #[serde(rename = "Ds_Order")]
    pub ds_order: String,
    #[serde(rename = "Ds_TransactionType")]
    pub ds_transaction_type: String,
}

/// SOAP XML Monitor request for querying all transaction types
///
/// CRITICAL: Field ordering must match Redsys DTD exactly.
/// Alphabetical sorting will cause XML0001 error (DTD validation failure).
///
/// Required DTD order: Ds_MerchantCode → Ds_Terminal → Ds_Order
///
/// Note: Monitor (simple) does NOT include Ds_TransactionType
///
/// Ref: RS.TE.CEL.MAN.0021 v1.4, Section 3.2.1 (Monitor simple)
#[derive(Debug, Serialize)]
pub struct RedsysMonitorRequest {
    #[serde(rename = "Ds_MerchantCode")]
    pub ds_merchant_code: Secret<String>,
    #[serde(rename = "Ds_Terminal")]
    pub ds_terminal: Secret<String>,
    #[serde(rename = "Ds_Order")]
    pub ds_order: String,
}

/// Request for invoking 3DS method redirect
/// Used to build the Base64-encoded threeDSMethodData POST body
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysThreedsInvokeRequest {
    pub three_d_s_method_notification_u_r_l: String,
    pub three_d_s_server_trans_i_d: String,
}

// serialize to camel case and try to convert it to hashmap<string, string>
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysThreeDsInvokeData {
    pub message_version: common_utils::types::SemanticVersion,
    pub three_ds_method_data: String,
    pub three_ds_method_url: String,
    pub three_ds_method_data_submission: String,
    pub three_d_s_server_trans_i_d: String,
}
