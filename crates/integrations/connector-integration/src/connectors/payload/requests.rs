use common_utils::types::FloatMajorUnit;
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::connectors::payload::responses;

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum PayloadPaymentsRequest<T: PaymentMethodDataTypes> {
    PayloadCardsRequest(Box<PayloadCardsRequestData<T>>),
    PayloadBankAccountRequest(Box<PayloadBankAccountRequestData>),
    PayloadMandateRequest(Box<PayloadMandateRequestData>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionTypes {
    Credit,
    Chargeback,
    ChargebackReversal,
    Deposit,
    Payment,
    Refund,
    Reversal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BillingAddress {
    #[serde(rename = "payment_method[billing_address][city]")]
    pub city: Secret<String>,
    #[serde(rename = "payment_method[billing_address][country_code]")]
    pub country: common_enums::CountryAlpha2,
    #[serde(rename = "payment_method[billing_address][postal_code]")]
    pub postal_code: Secret<String>,
    #[serde(rename = "payment_method[billing_address][state_province]")]
    pub state_province: Secret<String>,
    #[serde(rename = "payment_method[billing_address][street_address]")]
    pub street_address: Secret<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadCardsRequestData<T: PaymentMethodDataTypes> {
    pub amount: FloatMajorUnit,
    #[serde(flatten)]
    pub card: PayloadCard<T>,
    #[serde(rename = "type")]
    pub transaction_types: TransactionTypes,
    // For manual capture, set status to "authorized", otherwise omit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<responses::PayloadPaymentStatus>,
    #[serde(rename = "payment_method[type]")]
    pub payment_method_type: String,
    // Billing address fields are for AVS validation
    #[serde(flatten)]
    pub billing_address: BillingAddress,
    pub processing_id: Option<Secret<String>>,
    /// Allows one-time payment by customer without saving their payment method
    /// This is true by default
    #[serde(rename = "payment_method[keep_active]")]
    pub keep_active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadMandateRequestData {
    pub amount: FloatMajorUnit,
    #[serde(rename = "type")]
    pub transaction_types: TransactionTypes,
    // Based on the connectors' response, we can do recurring payment either based on a default payment method id saved in the customer profile or a specific payment method id
    // Connector by default, saves every payment method
    pub payment_method_id: Secret<String>,
    // For manual capture, set status to "authorized", otherwise omit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<responses::PayloadPaymentStatus>,
}

#[derive(Default, Clone, Debug, Serialize, Eq, PartialEq)]
pub struct PayloadCard<T: PaymentMethodDataTypes> {
    #[serde(rename = "payment_method[card][card_number]")]
    pub number: RawCardNumber<T>,
    #[serde(rename = "payment_method[card][expiry]")]
    pub expiry: Secret<String>,
    #[serde(rename = "payment_method[card][card_code]")]
    pub cvc: Secret<String>,
}

/// Bank account payment method type for ACH bank debit payments
pub const PAYMENT_METHOD_TYPE_BANK_ACCOUNT: &str = "bank_account";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadBankAccountRequestData {
    pub amount: FloatMajorUnit,
    #[serde(flatten)]
    pub bank_account: PayloadBankAccount,
    #[serde(rename = "type")]
    pub transaction_types: TransactionTypes,
    #[serde(rename = "payment_method[type]")]
    pub payment_method_type: String,
    /// Account holder name is required by Payload for bank account payments
    #[serde(rename = "payment_method[account_holder]")]
    pub account_holder: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<responses::PayloadPaymentStatus>,
    pub processing_id: Option<Secret<String>>,
    /// For one-time payments, set to false
    #[serde(rename = "payment_method[keep_active]")]
    pub keep_active: bool,
}

#[derive(Default, Clone, Debug, Serialize, PartialEq)]
pub struct PayloadBankAccount {
    #[serde(rename = "payment_method[bank_account][account_number]")]
    pub account_number: Secret<String>,
    #[serde(rename = "payment_method[bank_account][routing_number]")]
    pub routing_number: Secret<String>,
    #[serde(rename = "payment_method[bank_account][account_type]")]
    pub account_type: PayloadBankAccountType,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PayloadBankAccountType {
    #[default]
    Checking,
    Savings,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PayloadVoidRequest {
    pub status: responses::PayloadPaymentStatus,
}

// Type definition for CaptureRequest
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PayloadCaptureRequest {
    pub status: responses::PayloadPaymentStatus,
}

// Type definition for RefundRequest
#[derive(Debug, Serialize)]
pub struct PayloadRefundRequest {
    #[serde(rename = "type")]
    pub transaction_type: TransactionTypes,
    pub amount: FloatMajorUnit,
    #[serde(rename = "ledger[0][assoc_transaction_id]")]
    pub ledger_assoc_transaction_id: String,
}

// Type alias for RepeatPayment request (same structure as PayloadPaymentsRequest)
pub type PayloadRepeatPaymentRequest<T> = PayloadPaymentsRequest<T>;
