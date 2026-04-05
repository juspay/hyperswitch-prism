use common_utils::types::MinorUnit;
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganTokenRequest {
    pub grant_type: String,
    pub scope: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganPaymentsRequest<T: PaymentMethodDataTypes> {
    pub capture_method: CapMethod,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub merchant: JpmorganMerchant,
    pub payment_method_type: JpmorganPaymentMethodType<T>,
    pub account_holder: JpmorganAccountHolder,
    pub statement_descriptor: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganPaymentMethodType<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<JpmorganCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ach: Option<JpmorganAch>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganCard<T: PaymentMethodDataTypes> {
    pub account_number: RawCardNumber<T>,
    pub expiry: Expiry,
}

/// ACH Bank Debit payment method structure for JPMorgan
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAch {
    pub account_number: Secret<String>,
    pub financial_institution_routing_number: Secret<String>,
    pub account_type: JpmorganAchAccountType,
}

/// ACH Account Holder structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAccountHolder {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
}

/// ACH Account Type enum
#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganAchAccountType {
    Checking,
    Savings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Expiry {
    pub month: Secret<i32>,
    pub year: Secret<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMerchant {
    pub merchant_software: JpmorganMerchantSoftware,
    pub soft_merchant: JpmorganSoftMerchant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMerchantSoftware {
    pub company_name: Secret<String>,
    pub product_name: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganSoftMerchant {
    pub merchant_purchase_description: Secret<String>,
}

#[derive(Debug, Default, Copy, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CapMethod {
    #[default]
    Now,
    Delayed,
    Manual,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganCaptureRequest {
    pub capture_method: CapMethod,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganVoidRequest {
    // As per the docs, this is not a required field
    // Since we always pass `true` in `isVoid` only during the void call, it makes more sense to have it required field
    pub is_void: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganRefundRequest {
    pub merchant: JpmorganMerchantRefund,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMerchantRefund {
    pub merchant_software: JpmorganMerchantSoftware,
}

/// JPMorgan initiator type for stored credentials / MIT
#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganInitiatorType {
    Cardholder,
    Merchant,
}

/// JPMorgan account on file status
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganAccountOnFile {
    ToBeStored,
    Stored,
}

/// JPMorgan recurring sequence
#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganRecurringSequence {
    First,
    Subsequent,
}

/// JPMorgan recurring object for MIT transactions
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganRecurring {
    pub recurring_sequence: JpmorganRecurringSequence,
    pub agreement_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_variable_amount: Option<bool>,
}

/// JPMorgan card with optional originalNetworkTransactionId for MIT
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMitCard<T: PaymentMethodDataTypes> {
    pub account_number: RawCardNumber<T>,
    pub expiry: Expiry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_network_transaction_id: Option<String>,
}

/// JPMorgan payment method type for MIT (with optional network transaction ID)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMitPaymentMethodType<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<JpmorganMitCard<T>>,
}

/// SetupMandate request (initial CIT with credential storage)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganSetupMandateRequest<T: PaymentMethodDataTypes> {
    pub capture_method: CapMethod,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub merchant: JpmorganMerchant,
    pub payment_method_type: JpmorganMitPaymentMethodType<T>,
    pub recurring: JpmorganRecurring,
    pub initiator_type: JpmorganInitiatorType,
    pub account_on_file: JpmorganAccountOnFile,
    pub is_amount_final: bool,
}

/// RepeatPayment request (subsequent MIT)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganRepeatPaymentRequest<T: PaymentMethodDataTypes> {
    pub capture_method: CapMethod,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub merchant: JpmorganMerchant,
    pub payment_method_type: JpmorganMitPaymentMethodType<T>,
    pub recurring: JpmorganRecurring,
    pub initiator_type: JpmorganInitiatorType,
    pub account_on_file: JpmorganAccountOnFile,
    pub is_amount_final: bool,
}
