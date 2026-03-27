use common_enums::CountryAlpha2;
use common_utils::{pii, types::StringMajorUnit};
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::Serialize;

use crate::utils::MerchantDefinedInformation;

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
