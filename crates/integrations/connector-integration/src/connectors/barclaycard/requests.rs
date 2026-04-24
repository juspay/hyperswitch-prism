use common_enums::CountryAlpha2;
use common_utils::{pii, types::StringMajorUnit};
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::Serialize;

use crate::utils::MerchantDefinedInformation;
use cards;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardPaymentsRequest<T: PaymentMethodDataTypes + Sync + Send + 'static + Serialize>
{
    pub processing_information: ProcessingInformation,
    pub payment_information: PaymentInformation<T>,
    pub order_information: OrderInformationWithBill,
    pub client_reference_information: ClientReferenceInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_defined_information: Option<Vec<MerchantDefinedInformation>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingInformation {
    pub commerce_indicator: String,
    pub capture: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_solution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv_algorithm: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardPaymentInformation<T: PaymentMethodDataTypes + Sync + Send + 'static + Serialize> {
    pub card: Card<T>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaymentInformation<T: PaymentMethodDataTypes + Sync + Send + 'static + Serialize> {
    Cards(Box<CardPaymentInformation<T>>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card<T: PaymentMethodDataTypes + Sync + Send + 'static + Serialize> {
    pub number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub security_code: Secret<String>,
    #[serde(rename = "type")]
    pub card_type: Option<String>,
    pub type_selection_indicator: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInformationWithBill {
    pub amount_details: Amount,
    pub bill_to: Option<BillTo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    pub total_amount: StringMajorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillTo {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub address1: Secret<String>,
    pub locality: String,
    pub administrative_area: Secret<String>,
    pub postal_code: Secret<String>,
    pub country: CountryAlpha2,
    pub email: pii::Email,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientReferenceInformation {
    pub code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInformation {
    pub amount_details: Amount,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardCaptureRequest {
    pub order_information: OrderInformation,
    pub client_reference_information: ClientReferenceInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_defined_information: Option<Vec<MerchantDefinedInformation>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardVoidRequest {
    pub client_reference_information: ClientReferenceInformation,
    pub reversal_information: ReversalInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_defined_information: Option<Vec<MerchantDefinedInformation>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReversalInformation {
    pub amount_details: Amount,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardRefundRequest {
    pub order_information: OrderInformation,
    pub client_reference_information: ClientReferenceInformation,
}

// --- SetupMandate (Zero-dollar auth for TMS token creation) types ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardSetupMandateRequest<
    T: PaymentMethodDataTypes + Sync + Send + 'static + Serialize,
> {
    pub processing_information: SetupMandateProcessingInformation,
    pub payment_information: PaymentInformation<T>,
    pub order_information: OrderInformationWithBill,
    pub client_reference_information: ClientReferenceInformation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupMandateProcessingInformation {
    pub commerce_indicator: String,
    pub capture: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_list: Option<Vec<BarclaycardActionsList>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_token_types: Option<Vec<BarclaycardActionsTokenType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_options: Option<SetupMandateAuthorizationOptions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BarclaycardActionsList {
    TokenCreate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BarclaycardActionsTokenType {
    PaymentInstrument,
    Customer,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupMandateAuthorizationOptions {
    pub initiator: Option<SetupMandateInitiator>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupMandateInitiator {
    #[serde(rename = "type")]
    pub initiator_type: Option<BarclaycardPaymentInitiatorTypes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_stored_on_file: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BarclaycardPaymentInitiatorTypes {
    Customer,
    Merchant,
}

// --- RepeatPayment (MIT) types ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardRepeatPaymentRequest {
    pub processing_information: RepeatPaymentProcessingInformation,
    pub payment_information: RepeatPaymentInformation,
    pub order_information: OrderInformationWithBill,
    pub client_reference_information: ClientReferenceInformation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_defined_information: Option<Vec<MerchantDefinedInformation>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepeatPaymentProcessingInformation {
    pub commerce_indicator: String,
    pub capture: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_options: Option<AuthorizationOptions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiator: Option<PaymentInitiator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_initiated_transaction: Option<MerchantInitiatedTransaction>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInitiator {
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiator_type: Option<BarclaycardPaymentInitiatorTypes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credential_used: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantInitiatedTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_transaction_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_authorized_amount: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RepeatPaymentInformation {
    MandatePayment(Box<MandatePaymentInformation>),
    Cards(Box<CardWithNtiPaymentInformation>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardWithNtiPaymentInformation {
    pub card: CardWithNti,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardWithNti {
    pub number: cards::CardNumber,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_code: Option<Secret<String>>,
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_selection_indicator: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MandatePaymentInformation {
    pub payment_instrument: PaymentInstrument,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<MandateCard>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInstrument {
    pub id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MandateCard {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_selection_indicator: Option<String>,
}
