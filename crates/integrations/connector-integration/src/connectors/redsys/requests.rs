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
pub type RedsysSetupMandateRequest = super::transformers::RedsysTransaction;
pub type RedsysRepeatPaymentRequest = super::transformers::RedsysTransaction;

/// Main payment request structure for Redsys API
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct RedsysPaymentRequest {
    pub ds_merchant_amount: StringMinorUnit,
    pub ds_merchant_currency: String,
    // PAN/expiry/CVV are absent on identifier-based MIT and wallet requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_cvv2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_emv3ds: Option<RedsysEmvThreeDsRequestData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_expirydate: Option<Secret<String>>,
    pub ds_merchant_merchantcode: Secret<String>,
    pub ds_merchant_order: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_pan: Option<cards::CardNumber>,
    pub ds_merchant_terminal: Secret<String>,
    pub ds_merchant_transactiontype: RedsysTransactionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_excep_sca: Option<RedsysStrongCustomerAuthenticationException>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_directpayment: Option<bool>,
    /// External 3DS result (authentication done outside Redsys), spec "External MPI"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_mpiexternal: Option<RedsysMpiExternal>,
    /// Card-on-file reference: "REQUIRED" on a CIT asks Redsys to generate one
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_identifier: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_cof_ini: Option<RedsysCofIni>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_cof_type: Option<RedsysCofType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_cof_txnid: Option<Secret<String>>,
    /// Server-to-server notification URL (max 250)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_merchanturl: Option<String>,
    /// Dynamic statement descriptor (max 25, requires bank activation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_merchant_merchantdescriptor: Option<String>,
    /// Encrypted wallet token, Redsys decrypts (Apple: hex paymentData; Google: base64
    /// paymentMethodData). Mutually exclusive with ds_xpaydecodeddata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_xpaydata: Option<Secret<String>>,
    /// JSON-encoded RedsysXpayDecodedData (wallet decrypted before reaching Redsys)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_xpaydecodeddata: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_xpaytype: Option<RedsysXpayType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_xpayorigen: Option<RedsysXpayOrigen>,
}

/// DS_XPAYTYPE (Apple/Google Pay guides; the parameters page swaps TYPE/ORIGEN, guides win)
#[derive(Debug, Serialize)]
pub enum RedsysXpayType {
    Apple,
    Google,
}

/// DS_XPAYORIGEN
#[derive(Debug, Serialize)]
pub enum RedsysXpayOrigen {
    InApp,
    #[serde(rename = "WEB")]
    Web,
}

/// Google Pay decrypted card type: CARD = PAN_ONLY, TOKENIZED_CARD = CRYPTOGRAM_3DS
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RedsysGooglePayMethod {
    Card,
    TokenizedCard,
}

/// DS_XPAYDECODEDDATA content
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysXpayDecodedData {
    /// DPAN (Apple) / pan (Google)
    pub token: cards::CardNumber,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_ind: Option<String>,
    /// YYMM
    pub expiration_date: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<RedsysGooglePayMethod>,
}

/// Google Pay `paymentMethodData` as Google returns it (camelCase), base64-encoded into
/// DS_XPAYDATA for Redsys-side decryption
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysGooglePayPaymentMethodData {
    #[serde(rename = "type")]
    pub pm_type: String,
    pub description: String,
    pub info: RedsysGooglePayInfo,
    pub tokenization_data: RedsysGooglePayTokenizationData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysGooglePayInfo {
    pub card_network: String,
    pub card_details: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assurance_details: Option<RedsysGooglePayAssuranceDetails>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedsysGooglePayAssuranceDetails {
    pub card_holder_authenticated: bool,
    pub account_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct RedsysGooglePayTokenizationData {
    #[serde(rename = "type")]
    pub token_type: String,
    pub token: Secret<String>,
}

/// External MPI authentication result (DS_MERCHANT_MPIEXTERNAL); wire names per the
/// Redsys sample `{"threeDSServerTransID","dsTransID","authenticationValue","protocolVersion","Eci"}`
#[derive(Debug, Serialize)]
pub struct RedsysMpiExternal {
    #[serde(rename = "threeDSServerTransID")]
    pub three_d_s_server_trans_i_d: String,
    #[serde(rename = "dsTransID")]
    pub ds_trans_i_d: String,
    #[serde(rename = "authenticationValue")]
    pub authentication_value: Secret<String>,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "Eci")]
    pub eci: String,
}

/// DS_MERCHANT_COF_INI: S = first card-on-file transaction, N = subsequent
#[derive(Debug, Serialize, Deserialize)]
pub enum RedsysCofIni {
    S,
    N,
}

/// DS_MERCHANT_COF_TYPE
#[derive(Debug, Serialize, Deserialize)]
pub enum RedsysCofType {
    /// Installments
    I,
    /// Recurring
    R,
    /// Reauthorization
    H,
    /// Resubmission
    E,
    /// Delayed
    D,
    /// Incremental
    M,
    /// No-show
    N,
    /// Other (documented default)
    C,
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
    /// Card validation (no charge); with DS_MERCHANT_IDENTIFIER=REQUIRED it stores the card
    #[serde(rename = "7")]
    Validation,
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
