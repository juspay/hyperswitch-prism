use common_utils::StringMinorUnit;
use domain_types::errors::ConnectorError;
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::Serialize;

use super::super::macros::GetSoapXml;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorldpayxmlAction {
    Authorise,
    Sale,
    Cancel,
}
fn generate_soap_xml<T: Serialize>(
    request: &T,
) -> Result<String, error_stack::Report<ConnectorError>> {
    let xml_body =
        quick_xml::se::to_string(request).change_context(ConnectorError::RequestEncodingFailed)?;

    Ok(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE paymentService PUBLIC \"-//Worldpay//DTD Worldpay PaymentService v1//EN\" \"http://dtd.worldpay.com/paymentService_v1.dtd\">\n{}", xml_body))
}

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPaymentsRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub submit: WorldpayxmlSubmit,
}

impl GetSoapXml for WorldpayxmlPaymentsRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlSubmit {
    pub order: WorldpayxmlOrder,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlOrder {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    #[serde(rename = "@captureDelay")]
    pub capture_delay: String,
    pub description: String,
    pub amount: WorldpayxmlAmount,
    #[serde(rename = "paymentDetails")]
    pub payment_details: WorldpayxmlPaymentDetails,
    pub shopper: WorldpayxmlShopper,
    #[serde(rename = "billingAddress", skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<WorldpayxmlBillingAddress>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlAmount {
    #[serde(rename = "@value")]
    pub value: StringMinorUnit,
    #[serde(rename = "@currencyCode")]
    pub currency_code: common_enums::Currency,
    #[serde(rename = "@exponent")]
    pub exponent: String,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlPaymentDetails {
    #[serde(rename = "@action")]
    pub action: WorldpayxmlAction,
    #[serde(rename = "$value")]
    pub payment_method: WorldpayxmlPaymentMethod,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum WorldpayxmlPaymentMethod {
    #[serde(rename = "CARD-SSL")]
    Card(WorldpayxmlCard),
    #[serde(rename = "VISA-SSL")]
    Visa(WorldpayxmlCard),
    #[serde(rename = "ECMC-SSL")]
    Ecmc(WorldpayxmlCard),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlCard {
    pub card_number: Secret<String>,
    pub expiry_date: WorldpayxmlExpiryDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    pub cvc: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlExpiryDate {
    pub date: WorldpayxmlDate,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlDate {
    #[serde(rename = "@month")]
    pub month: Secret<String>,
    #[serde(rename = "@year")]
    pub year: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlShopper {
    #[serde(
        rename = "shopperEmailAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub shopper_email_address: Option<common_utils::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<WorldpayxmlBrowser>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlBrowser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_accept_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_java_script_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_colour_depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_screen_width: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlBillingAddress {
    pub address: WorldpayxmlAddress,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorldpayxmlAddress {
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(rename = "address1", skip_serializing_if = "Option::is_none")]
    pub address1: Option<Secret<String>>,
    #[serde(rename = "address2", skip_serializing_if = "Option::is_none")]
    pub address2: Option<Secret<String>>,
    #[serde(rename = "address3", skip_serializing_if = "Option::is_none")]
    pub address3: Option<Secret<String>>,
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(rename = "city", skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(rename = "state", skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(rename = "countryCode", skip_serializing_if = "Option::is_none")]
    pub country_code: Option<common_enums::CountryAlpha2>,
}

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlCaptureRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub modify: WorldpayxmlModify,
}

impl GetSoapXml for WorldpayxmlCaptureRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlModify {
    #[serde(rename = "orderModification")]
    pub order_modification: WorldpayxmlOrderModification,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlOrderModification {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    pub capture: WorldpayxmlCapture,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlCapture {
    pub amount: WorldpayxmlAmount,
}

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlVoidRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub modify: WorldpayxmlVoidModify,
}

impl GetSoapXml for WorldpayxmlVoidRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlVoidModify {
    #[serde(rename = "orderModification")]
    pub order_modification: WorldpayxmlVoidOrderModification,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlVoidOrderModification {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    pub cancel: WorldpayxmlCancel,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlCancel {
    // Empty struct - generates <cancel/> element
}

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlRefundRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub modify: WorldpayxmlRefundModify,
}

impl GetSoapXml for WorldpayxmlRefundRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlRefundModify {
    #[serde(rename = "orderModification")]
    pub order_modification: WorldpayxmlRefundOrderModification,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlRefundOrderModification {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    pub refund: WorldpayxmlRefund,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlRefund {
    pub amount: WorldpayxmlAmount,
}

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPSyncRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub inquiry: WorldpayxmlInquiry,
}

impl GetSoapXml for WorldpayxmlPSyncRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlInquiry {
    #[serde(rename = "orderInquiry")]
    pub order_inquiry: WorldpayxmlOrderInquiry,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlOrderInquiry {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
}

// Type alias for RSync - reuses PSync request structure
pub type WorldpayxmlRSyncRequest = WorldpayxmlPSyncRequest;
