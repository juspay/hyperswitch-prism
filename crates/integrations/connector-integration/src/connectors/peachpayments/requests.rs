use common_enums::Currency;
use common_utils::MinorUnit;
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::Serialize;
use std::fmt::Debug;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DccMode {
    NoDcc,
    OptInDcc,
    OptOutDcc,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum MerchantType {
    Standard,
    Sub,
    Iso,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CardNetworkLowercase {
    Visa,
    Mastercard,
    Amex,
    Discover,
    Jcb,
    Diners,
    CartesBancaires,
    UnionPay,
    Interac,
    #[serde(rename = "rupay")]
    RuPay,
    Maestro,
    Star,
    Pulse,
    Accel,
    Nyce,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreAuthIncExtCaptureFlow {
    pub dcc_mode: DccMode,
    pub txn_ref_nr: String,
}

#[derive(Debug, Serialize)]
pub struct PosData {
    pub referral: String,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CofType {
    Adhoc,
    Recurring,
    Instalment,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum CofSource {
    Cit,
    Mit,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum CofMode {
    Initial,
    Subsequent,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CardOnFileData {
    #[serde(rename = "type")]
    pub _type: CofType,
    pub source: CofSource,
    pub mode: CofMode,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsAmount {
    pub amount: MinorUnit,
    pub currency_code: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_amount: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PeachpaymentsCaptureRequest {
    pub amount: PeachpaymentsAmount,
}

#[derive(Debug, Serialize)]
pub struct PeachpaymentsVoidRequest {
    pub amount: PeachpaymentsAmount,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsRefundRequest {
    pub reference_id: String,
    pub ecommerce_card_payment_only_transaction_data: PeachpaymentsRefundTransactionData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos_data: Option<PosData>,
}

#[derive(Debug, Serialize)]
pub struct PeachpaymentsRefundTransactionData {
    pub amount: PeachpaymentsAmount,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PeachpaymentsPaymentMethod {
    Card,
    EcommerceCard,
    EcommerceCardPaymentOnly,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsAuthorizeRequest<T: PaymentMethodDataTypes> {
    pub payment_method: PeachpaymentsPaymentMethod,
    pub reference_id: String,
    pub ecommerce_card_payment_only_transaction_data: PeachpaymentsTransactionData<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos_data: Option<serde_json::Value>,
    pub send_date_time: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PeachpaymentsTransactionData<T: PaymentMethodDataTypes> {
    Card(PeachpaymentsCardData<T>),
    NetworkToken(PeachpaymentsNetworkTokenData),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsCardData<T: PaymentMethodDataTypes> {
    pub merchant_information: PeachpaymentsMerchantInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_reference: Option<PeachpaymentsRoutingReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<PeachpaymentsRouting>,
    pub card: PeachpaymentsCardDetails<T>,
    pub amount: PeachpaymentsAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_auth_inc_ext_capture_flow: Option<PeachpaymentsPreAuthFlow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cof_data: Option<PeachpaymentsCofData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsNetworkTokenData {
    pub merchant_information: PeachpaymentsMerchantInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_reference: Option<PeachpaymentsRoutingReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<PeachpaymentsRouting>,
    pub network_token_data: PeachpaymentsNetworkTokenDetails,
    pub amount: PeachpaymentsAmount,
    pub cof_data: PeachpaymentsCofData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_auth_inc_ext_capture_flow: Option<PeachpaymentsPreAuthFlow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsRoutingReference {
    pub merchant_payment_method_route_id: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct PeachpaymentsRouting {
    pub route: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsPreAuthFlow {
    pub dcc_mode: DccMode,
    pub txn_ref_nr: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsCardDetails<T: PaymentMethodDataTypes> {
    pub pan: RawCardNumber<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_year: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_month: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsNetworkToken {
    pub payment_method: String,
    pub routing: PeachpaymentsRoutingInfo,
    pub network_token: PeachpaymentsNetworkTokenDetails,
    pub cof_data: PeachpaymentsCofData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsRoutingInfo {
    pub merchant_payment_method_route_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsNetworkTokenDetails {
    pub token: Secret<String>,
    pub expiry_year: Secret<String>,
    pub expiry_month: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<CardNetworkLowercase>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PeachpaymentsCofData {
    #[serde(rename = "type")]
    pub cof_type: CofType,
    pub source: CofSource,
    pub mode: CofMode,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsMerchantInformation {
    pub client_merchant_reference_id: Secret<String>,
}

// SetupMandate request reuses the same structure as Authorize
// but with cof_data set to initial CIT
pub type PeachpaymentsSetupMandateRequest<T> = PeachpaymentsAuthorizeRequest<T>;

// RepeatPayment request reuses the same structure as Authorize
// but with cof_data set to subsequent MIT
pub type PeachpaymentsRepeatPaymentRequest<T> = PeachpaymentsAuthorizeRequest<T>;
