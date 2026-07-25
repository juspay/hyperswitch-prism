use common_utils::{pii::Email, types::StringMajorUnit};
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::connectors::payload::responses;

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum PayloadPaymentsRequest<T: PaymentMethodDataTypes> {
    PayloadPaymentRequest(Box<PayloadCardsRequestData<T>>),
    PayloadMandateRequest(Box<PayloadMandateRequestData>),
    PayloadCardTokenRequest(Box<PayloadCardTokenRequestData>),
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

#[derive(Debug, Clone, Serialize)]
pub struct PayloadWebhookRegisterRequest {
    pub trigger: PayloadWebhookRegisterEventType,
    pub url: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_secret: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadWebhookRegisterEventType {
    Payment,
    Processed,
    Authorized,
    Credit,
    Refund,
    Reversal,
    Void,
    Decline,
    Deposit,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BillingAddress {
    pub city: Secret<String>,
    pub country_code: common_enums::CountryAlpha2,
    pub postal_code: Secret<String>,
    pub state_province: Secret<String>,
    pub street_address: Secret<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadCardsRequestData<T: PaymentMethodDataTypes> {
    pub amount: StringMajorUnit,
    pub payment_method: PayloadPaymentMethod<T>,
    #[serde(rename = "type")]
    pub transaction_types: TransactionTypes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<responses::PayloadPaymentStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    /// Free-text context about the purchase shown on receipts / transaction histories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Flexible JSON object for structured metadata (order IDs, lease references, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attrs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadPaymentMethod<T: PaymentMethodDataTypes> {
    #[serde(flatten)]
    pub method: PayloadPaymentMethods<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<BillingAddress>,
    pub keep_active: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayloadPaymentMethods<T: PaymentMethodDataTypes> {
    Card(PayloadCard<T>),
    BankAccount(PayloadBank),
}

#[derive(Default, Clone, Debug, Serialize, Eq, PartialEq)]
pub struct PayloadCard<T: PaymentMethodDataTypes> {
    pub card: PayloadCardData<T>,
}

#[derive(Default, Clone, Debug, Serialize, Eq, PartialEq)]
pub struct PayloadCardData<T: PaymentMethodDataTypes> {
    pub card_number: RawCardNumber<T>,
    pub expiry: Secret<String>,
    pub card_code: Secret<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PayloadBank {
    pub bank_account: PayloadBankAccountInner,
    pub account_holder: Secret<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PayloadBankAccountInner {
    pub account_class: Option<PayloadAccClass>,
    pub account_currency: String,
    pub account_number: Secret<String>,
    pub account_type: PayloadBankAccountType,
    pub routing_number: Secret<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PayloadAccClass {
    Personal,
    Business,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PayloadBankAccountType {
    #[default]
    Checking,
    Savings,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadMandateRequestData {
    pub amount: StringMajorUnit,
    #[serde(rename = "type")]
    pub transaction_types: TransactionTypes,
    pub payment_method_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<responses::PayloadPaymentStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadCardTokenRequestData {
    pub amount: StringMajorUnit,
    #[serde(rename = "type")]
    pub transaction_types: TransactionTypes,
    pub payment_method_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<responses::PayloadPaymentStatus>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PayloadVoidRequest {
    pub status: responses::PayloadPaymentStatus,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PayloadCaptureRequest {
    pub status: responses::PayloadPaymentStatus,
}

#[derive(Debug, Serialize)]
pub struct PayloadRefundRequest {
    #[serde(rename = "type")]
    pub transaction_type: TransactionTypes,
    pub amount: StringMajorUnit,
    pub ledger: Vec<PayloadRefundLedgerEntry>,
}

#[derive(Debug, Serialize)]
pub struct PayloadRefundLedgerEntry {
    pub assoc_transaction_id: String,
}

pub type PayloadRepeatPaymentRequest<T> = PayloadPaymentsRequest<T>;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PayloadCustomerRequest {
    pub keep_active: bool,
    pub email: Email,
    pub name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_processing_id: Option<Secret<String>>,
}
