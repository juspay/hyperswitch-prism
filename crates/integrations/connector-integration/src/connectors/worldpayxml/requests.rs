use common_utils::StringMinorUnit;
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::super::macros::GetSoapXml;
use domain_types::errors::IntegrationError;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorldpayxmlAction {
    Authorise,
    Sale,
    Cancel,
}
fn generate_soap_xml<T: Serialize>(
    request: &T,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let xml_body = quick_xml::se::to_string(request).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;

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
    #[serde(rename = "@captureDelay", skip_serializing_if = "Option::is_none")]
    pub capture_delay: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<WorldpayxmlAmount>,
    #[serde(rename = "paymentDetails", skip_serializing_if = "Option::is_none")]
    pub payment_details: Option<WorldpayxmlPaymentDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shopper: Option<WorldpayxmlShopper>,
    #[serde(rename = "billingAddress", skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<WorldpayxmlBillingAddress>,
    // NOTE: field order below is wire order — quick-xml emits elements in declaration
    // order and the WPG DTD expects info3DSecure, session, createToken, additional3DSData
    // after <billingAddress>.
    #[serde(rename = "info3DSecure", skip_serializing_if = "Option::is_none")]
    pub info_threed_secure: Option<WorldpayxmlInfo3DSecure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<WorldpayxmlSession>,
    #[serde(rename = "createToken", skip_serializing_if = "Option::is_none")]
    pub create_token: Option<WorldpayxmlCreateToken>,
    #[serde(rename = "additional3DSData", skip_serializing_if = "Option::is_none")]
    pub additional_threeds_data: Option<WorldpayxmlAdditionalThreeDSData>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlInfo3DSecure {
    #[serde(rename = "completedAuthentication")]
    pub completed_authentication: WorldpayxmlCompletedAuthentication,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlCompletedAuthentication {}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlSession {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@shopperIPAddress")]
    pub shopper_ip_address: Secret<String, common_utils::pii::IpAddress>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlAdditionalThreeDSData {
    #[serde(rename = "@dfReferenceId", skip_serializing_if = "Option::is_none")]
    pub df_reference_id: Option<Secret<String>>,
    #[serde(rename = "@javaScriptEnabled")]
    pub javascript_enabled: bool,
    #[serde(rename = "@deviceChannel")]
    pub device_channel: String,
    #[serde(rename = "@challengePreference")]
    pub challenge_preference: WorldpayxmlChallengePreference,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldpayxmlChallengePreference {
    ChallengeMandated,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlCreateToken {
    #[serde(rename = "@tokenScope")]
    pub token_scope: WorldpayxmlTokenScope,
    #[serde(rename = "tokenEventReference")]
    pub token_event_reference: String,
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
    /// Omitted for merchant-initiated payments, where Worldpay derives the action from the
    /// order's capture delay instead.
    #[serde(rename = "@action", skip_serializing_if = "Option::is_none")]
    pub action: Option<WorldpayxmlAction>,
    #[serde(rename = "$value")]
    pub payment_method: WorldpayxmlPaymentMethod,
    #[serde(rename = "storedCredentials", skip_serializing_if = "Option::is_none")]
    pub stored_credentials: Option<WorldpayxmlStoredCredentials>,
}

/// Flags the authorisation as part of a stored-credential agreement.
#[derive(Debug, Serialize)]
pub struct WorldpayxmlStoredCredentials {
    #[serde(rename = "@usage")]
    pub usage: WorldpayxmlUsageType,
    #[serde(
        rename = "@customerInitiatedReason",
        skip_serializing_if = "Option::is_none"
    )]
    pub customer_initiated_reason: Option<WorldpayxmlMandateType>,
    #[serde(
        rename = "@merchantInitiatedReason",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_initiated_reason: Option<WorldpayxmlMandateType>,
    #[serde(
        rename = "schemeTransactionIdentifier",
        skip_serializing_if = "Option::is_none"
    )]
    pub scheme_transaction_identifier: Option<Secret<String>>,
}

/// Scope a Worldpay payment token is issued under. Only `shopper` is used.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WorldpayxmlTokenScope {
    Shopper,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorldpayxmlUsageType {
    First,
    Used,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorldpayxmlMandateType {
    Recurring,
    Unscheduled,
    Instalment,
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
    #[serde(rename = "PAYWITHGOOGLE-SSL")]
    PayWithGoogle(WorldpayxmlGooglePayData),
    #[serde(rename = "APPLEPAY-SSL")]
    ApplePay(WorldpayxmlApplePayData),
    /// Carries an already-decrypted wallet token as a network token.
    #[serde(rename = "EMVCO_TOKEN-SSL")]
    EmvcoToken(WorldpayxmlEmvcoTokenData),
    #[serde(rename = "TOKEN-SSL")]
    TokenSsl(WorldpayxmlTokenData),
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlTokenData {
    #[serde(rename = "@tokenScope")]
    pub token_scope: WorldpayxmlTokenScope,
    #[serde(rename = "paymentTokenID")]
    pub payment_token_id: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlGooglePayData {
    pub protocol_version: Secret<String>,
    pub signature: Secret<String>,
    pub signed_message: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlApplePayData {
    pub header: WorldpayxmlApplePayHeader,
    pub signature: Secret<String>,
    pub version: Secret<String>,
    pub data: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlApplePayHeader {
    pub ephemeral_public_key: Secret<String>,
    pub public_key_hash: Secret<String>,
    pub transaction_id: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorldpayxmlEmvcoTokenType {
    Applepay,
    Googlepay,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlEmvcoTokenData {
    #[serde(rename = "@type")]
    pub token_type: WorldpayxmlEmvcoTokenType,
    pub token_number: cards::CardNumber,
    pub expiry_date: WorldpayxmlExpiryDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlCard {
    pub card_number: Secret<String>,
    pub expiry_date: WorldpayxmlExpiryDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    /// Absent when the PAN came from a decrypted wallet token, which carries no CVC.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<Secret<String>>,
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
    /// Worldpay scopes shopper tokens to this identifier, so it must be present on every
    /// request that creates or spends one.
    #[serde(
        rename = "authenticatedShopperID",
        skip_serializing_if = "Option::is_none"
    )]
    pub authenticated_shopper_id: Option<Secret<String>>,
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
    // NOTE: must stay the LAST field — quick-xml emits elements in declaration
    // order and the WPG DTD expects <telephoneNumber> after <countryCode>
    #[serde(rename = "telephoneNumber", skip_serializing_if = "Option::is_none")]
    pub telephone_number: Option<Secret<String>>,
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

// SetupMandate and RepeatPayment submit the same `<submit><order>` envelope as Authorize.
// They are aliased (rather than reused directly) because the connector macros key their
// per-flow bridge types off the request/response type name.
pub type WorldpayxmlSetupMandateRequest = WorldpayxmlPaymentsRequest;
pub type WorldpayxmlRepeatPaymentRequest = WorldpayxmlPaymentsRequest;

// ===== PAYOUT REQUESTS =====

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPayoutTransferRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub submit: WorldpayxmlPayoutSubmit,
}

impl GetSoapXml for WorldpayxmlPayoutTransferRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlPayoutSubmit {
    pub order: WorldpayxmlPayoutOrder,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlPayoutOrder {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    pub description: String,
    pub amount: WorldpayxmlAmount,
    #[serde(rename = "paymentDetails")]
    pub payment_details: WorldpayxmlPayoutPaymentDetails,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlPayoutPaymentDetails {
    #[serde(rename = "$value")]
    pub payment_method: WorldpayxmlPayoutPaymentMethod,
}

#[derive(Debug, Serialize)]
pub enum WorldpayxmlPayoutPaymentMethod {
    #[serde(rename = "FF_DISBURSE-SSL")]
    FastAccessSsl(WorldpayxmlFastAccess),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlFastAccess {
    pub recipient: WorldpayxmlPayoutRecipient,
    #[serde(rename = "purposeOfPayment", skip_serializing_if = "Option::is_none")]
    pub purpose_of_payment: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlPayoutRecipient {
    pub payment_instrument: WorldpayxmlPayoutPaymentInstrument,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<WorldpayxmlAddress>,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlPayoutPaymentInstrument {
    #[serde(rename = "cardDetails")]
    pub card_details: WorldpayxmlPayoutCardDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldpayxmlPayoutCardDetails {
    pub card_number: Secret<String>,
    pub expiry_date: WorldpayxmlExpiryDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPayoutGetRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub inquiry: WorldpayxmlInquiry,
}

impl GetSoapXml for WorldpayxmlPayoutGetRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlPayoutVoidRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub modify: WorldpayxmlPayoutVoidModify,
}

impl GetSoapXml for WorldpayxmlPayoutVoidRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlPayoutVoidModify {
    #[serde(rename = "orderModification")]
    pub order_modification: WorldpayxmlPayoutCancelOrderModification,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlPayoutCancelOrderModification {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    #[serde(rename = "cancelRefund")]
    pub cancel_refund: WorldpayxmlCancelRefund,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlCancelRefund {}

// ===== VOID PC REQUESTS =====

#[derive(Debug, Serialize)]
#[serde(rename = "paymentService")]
pub struct WorldpayxmlVoidPCRequest {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@merchantCode")]
    pub merchant_code: Secret<String>,
    pub modify: WorldpayxmlVoidPCModify,
}

impl GetSoapXml for WorldpayxmlVoidPCRequest {
    fn to_soap_xml(&self) -> String {
        generate_soap_xml(self).unwrap_or_else(|_| {
            String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><paymentService/>")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlVoidPCModify {
    #[serde(rename = "orderModification")]
    pub order_modification: WorldpayxmlVoidPCOrderModification,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlVoidPCOrderModification {
    #[serde(rename = "@orderCode")]
    pub order_code: String,
    #[serde(rename = "cancelOrRefund")]
    pub cancel_or_refund: WorldpayxmlCancelOrRefund,
}

#[derive(Debug, Serialize)]
pub struct WorldpayxmlCancelOrRefund {
    // Empty struct - generates <cancelOrRefund/> element
}
