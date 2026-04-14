use common_utils::types::StringMajorUnit;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

// BlueSnap Transaction Type enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BluesnapTxnType {
    AuthOnly,
    AuthCapture,
    AuthReversal,
    Capture,
    Refund,
}

// Card holder information
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapCardHolderInfo {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub email: common_utils::pii::Email,
}

// Credit card details for BlueSnap API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapCreditCard {
    pub card_number: Secret<String>,
    pub security_code: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
}

// Apple Pay wallet structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapApplePayWallet {
    pub encoded_payment_token: Secret<String>,
}

// Google Pay wallet structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapGooglePayWallet {
    pub encoded_payment_token: Secret<String>,
}

// Wallet container for Apple Pay and Google Pay
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapWallet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apple_pay: Option<BluesnapApplePayWallet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_pay: Option<BluesnapGooglePayWallet>,
    pub wallet_type: String,
}

// ACH bank debit data structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapAchData {
    pub account_number: Secret<String>,
    pub routing_number: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_holder_type: Option<String>,
}

// Payment method details - supports cards, wallets, and ACH
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BluesnapPaymentMethodDetails {
    Card {
        #[serde(rename = "creditCard")]
        credit_card: BluesnapCreditCard,
    },
    Wallet {
        wallet: BluesnapWallet,
    },
    Ach {
        #[serde(rename = "ecp")]
        ach: BluesnapAchData,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionFraudInfo {
    pub fraud_session_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapMetadata {
    pub meta_data: Vec<RequestMetadata>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMetadata {
    pub meta_key: Option<String>,
    pub meta_value: Option<String>,
    pub is_visible: Option<String>,
}

// Main authorize request structure based on tech spec
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapPaymentsRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
    pub card_transaction_type: BluesnapTxnType,
    #[serde(flatten)]
    pub payment_method_details: BluesnapPaymentMethodDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_info: Option<BluesnapCardHolderInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_fraud_info: Option<TransactionFraudInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_meta_data: Option<BluesnapMetadata>,
}

// Capture request structure based on BlueSnap tech spec
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapCaptureRequest {
    pub card_transaction_type: BluesnapTxnType,
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

// Void request structure based on BlueSnap tech spec
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapVoidRequest {
    pub card_transaction_type: BluesnapTxnType,
    pub transaction_id: String,
}

// Refund request structure - supports partial refunds via optional amount
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapRefundRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ===== ACH/ECP PAYMENT STRUCTURES =====

// Payer info for ACH transactions (required by BlueSnap alt-transactions endpoint)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapPayerInfo {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub zip: Secret<String>,
}

// ECP transaction data for ACH (BlueSnap alt-transactions format)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapEcpTransaction {
    pub routing_number: Secret<String>,
    pub account_number: Secret<String>,
    pub account_type: String,
}

// ACH-specific authorize request structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapAchAuthorizeRequest {
    pub ecp_transaction: BluesnapEcpTransaction,
    pub amount: StringMajorUnit,
    pub currency: String,
    pub authorized_by_shopper: bool,
    pub payer_info: BluesnapPayerInfo,
    pub merchant_transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_fraud_info: Option<TransactionFraudInfo>,
}

// ===== SEPA DD PAYMENT STRUCTURES =====

// SEPA DD payer info (required by BlueSnap alt-transactions endpoint)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapSepaPayerInfo {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub country: String,
}

// SEPA Direct Debit transaction data (BlueSnap alt-transactions format)
// BlueSnap requires sepaDirectDebitTransaction with the shopper's IBAN
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapSepaDirectDebitTransaction {
    pub iban: Secret<String>,
}

// SEPA DD-specific authorize request structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapSepaAuthorizeRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
    pub authorized_by_shopper: bool,
    pub payer_info: BluesnapSepaPayerInfo,
    pub sepa_direct_debit_transaction: BluesnapSepaDirectDebitTransaction,
    pub merchant_transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_fraud_info: Option<TransactionFraudInfo>,
}

// Unified authorize request enum to support multiple payment methods
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BluesnapAuthorizeRequest {
    Card(BluesnapPaymentsRequest),
    Ach(BluesnapAchAuthorizeRequest),
    Sepa(BluesnapSepaAuthorizeRequest),
    CardToken(BluesnapCompletePaymentsRequest),
}

// ===== 3DS AUTHENTICATION STRUCTURES =====

// 3D Secure information for complete authorize
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapThreeDSecureInfo {
    pub three_d_secure_reference_id: String,
}

// Token request for 3DS flow (prefill)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapPaymentsTokenRequest {
    pub cc_number: Secret<String>,
    pub exp_date: Secret<String>, // Format: MM/YYYY
}

// Complete payment request after 3DS authentication
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapCompletePaymentsRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
    pub card_transaction_type: BluesnapTxnType,
    pub pf_token: Secret<String>, // Payment fields token from prefill step
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure: Option<BluesnapThreeDSecureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_fraud_info: Option<TransactionFraudInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_info: Option<BluesnapCardHolderInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_meta_data: Option<BluesnapMetadata>,
}

// ===== SETUP MANDATE (VAULTED SHOPPER) STRUCTURES =====

/// Credit card information for vaulted shopper creation
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapVaultedCreditCard {
    pub card_number: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_code: Option<Secret<String>>,
}

/// Credit card info wrapper for vaulted shopper
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapVaultedCreditCardInfo {
    pub credit_card: BluesnapVaultedCreditCard,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_contact_info: Option<BluesnapBillingContactInfo>,
}

/// Billing contact info for vaulted shopper
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapBillingContactInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

/// Payment sources for vaulted shopper (supports credit card)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapPaymentSources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_card_info: Option<Vec<BluesnapVaultedCreditCardInfo>>,
}

/// SetupMandate request - creates a vaulted shopper in BlueSnap
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapSetupMandateRequest {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    pub payment_sources: BluesnapPaymentSources,
}
