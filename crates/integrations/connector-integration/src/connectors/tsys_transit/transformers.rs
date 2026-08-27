use common_enums::{
    AttemptStatus, CardNetwork, FutureUsage, MitCategory, PaymentChannel, RefundStatus,
};
use common_utils::{
    collect_missing_value_keys,
    types::{MinorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void, VoidPC,
        VoidPostRefund,
    },
    connector_types::{
        MandateIds, MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RecurringMandatePaymentData, RefundFlowData, RefundSyncData, RefundVoidPostRefundData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::{
        Card, CardDetailsForNetworkTransactionId, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils::split_full_name as split_domain_full_name,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{super::macros::GetSoapXml, profile::TxProfile, rules, TsysTransitRouterData};
use crate::types::ResponseRouterData;

const DEFAULT_CANCELLATION_REASON: &str = "POST_AUTH_USER_DECLINE";

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransitCardDataSource {
    Phone,
    Internet,
    Manual,
    Recurring,
    Mail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitTerminalCapability {
    Unknown,
    NoTerminalManual,
    MagstripeReadOnly,
    Ocr,
    IccChipReadOnly,
    KeyedEntryOnly,
    MagstripeContactlessOnly,
    MagstripeKeyedEntryOnly,
    MagstripeIccKeyedEntryOnly,
    MagstripeIccOnly,
    IccKeyedEntryOnly,
    IccChipContactContactless,
    IccContactlessOnly,
    OtherCapabilityForMastercard,
    MagstripeSignatureForAmexOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitTerminalOperatingEnvironment {
    NoTerminal,
    OnMerchantPremisesAttended,
    OnMerchantPremisesUnattended,
    OffMerchantPremisesAttended,
    OffMerchantPremisesUnattended,
    OnCustomerPremisesUnattended,
    Unknown,
    ElectronicDeliveryAmex,
    PhysicalDeliveryAmex,
    OffMerchantPremisesMpos,
    OnMerchantPremisesMpos,
    OffMerchantPremisesCustomerPos,
    OnMerchantPremisesCustomerPos,
    OffCustomerPremisesUnattended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitCardholderAuthenticationMethod {
    NotAuthenticated,
    Pin,
    ElectronicSignatureAnalysis,
    ManualSignature,
    ManualOther,
    Unknown,
    SystematicOther,
    ETicketEnvAmex,
    OfflinePin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitTerminalAuthenticationCapability {
    NoCapability,
    PinEntry,
    SignatureAnalysis,
    MposSoftwareBasedPinEntryCapability,
    SignatureAnalysisInoperative,
    Other,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitTerminalOutputCapability {
    None,
    PrintOnly,
    DisplayOnly,
    PrintAndDisplay,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitMaxPinLength {
    Unknown,
    NotSupported,
    #[serde(rename = "4")]
    Four,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "6")]
    Six,
    #[serde(rename = "7")]
    Seven,
    #[serde(rename = "8")]
    Eight,
    #[serde(rename = "9")]
    Nine,
    #[serde(rename = "10")]
    Ten,
    #[serde(rename = "11")]
    Eleven,
    #[serde(rename = "12")]
    Twelve,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitTerminalCardCaptureCapability {
    NoCapability,
    CardCaptureCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitCardholderPresentDetail {
    ClickToPayDiscover,
    CardholderPresent,
    CardholderNotPresentUnspecifiedReason,
    CardholderNotPresentMailTransaction,
    CardholderNotPresentPhoneTransaction,
    CardholderNotPresentRecurringTransaction,
    CardholderNotPresentElectronicCommerce,
    CardholderNotPresentInstallmentTransaction,
    PartialShipmentTransactionOnTokenCryptogramTxn,
    RecurringTransactionOnTokenCryptogramTxn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitCardPresentDetail {
    CardNotPresent,
    CardPresent,
    TransponderAmex,
    ContactlessChipTransactions,
    DigitalWalletAmex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitCardDataInputMode {
    VoiceAuthAruOnly,
    MagneticStripeReaderInput,
    BarCodePaymentCode,
    KeyEnteredInput,
    MerchantInitiatedTransactionCardCredentialStoredOnFile,
    PanAutoEntryContactlessMagneticStripe,
    MagneticStripeReaderInputTrackDataCapturedPassedUnaltered,
    OnlineChip,
    OfflineChip,
    PanAutoEntryContactlessChipCard,
    TrackDataReadUnalteredChipCapableTerminalChipDataNotRead,
    EmptyCandidateListFallback,
    PanEntryElectronicCommerceIncludingRemoteChip,
    ElectronicCommerceNoSecurityChannelEncryptedSetWithoutCardholderCertificate,
    ManuallyEnteredWithKeyedCidAmexJcb,
    SwipedTransactionWithKeyedCidAmexJcb,
    ContactlessToContactChipCardSwitchTransactionDiscoverOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitCardholderAuthenticationEntity {
    NotAuthenticated,
    IccOfflinePin,
    CardAcceptanceDevice,
    AuthorizingAgentOnlinePin,
    MerchantCardAcceptorSignature,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitCardDataOutputCapability {
    None,
    MagneticStripeWrite,
    Icc,
    Other,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitAuthorizationIndicator {
    Preauth,
    Final,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitCardOnFile {
    Y,
    N,
}
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub enum TsysTransitMitIndicator {
    #[default]
    R,
    M101,
    M102,
    M103,
    M104,
    S,
    T,
    U,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitIsRecurring {
    Y,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitBillingType {
    Installment,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitCommercialCardLevel {
    Level2,
    Level3,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitMcCitStatusIndicator {
    C101,
    C102,
    C103,
    C104,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitMit {
    pub mit_indicator: TsysTransitMitIndicator,
}
#[derive(Debug, Clone, Serialize)]
pub struct TsysTransitWalletDetailsRef {
    #[serde(rename = "walletID")]
    pub wallet_id: Secret<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitAdditionalTaxDetails {
    pub tax_type: String,
    pub tax_amount: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_rate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_category: Option<TsysTransitTaxCategory>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitProductTaxDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_tax_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_tax_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_tax_percentage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_tax_type: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitProductDiscountDetails {
    pub product_discount_name: String,
    pub product_discount_amount: StringMajorUnit,
    pub product_discount_percentage: f64,
    pub product_discount_type: String,
    pub priority: u16,
    pub stackable: TsysTransitYesNo,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitProductModifierDetails {
    pub modifier_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier_price: Option<StringMajorUnit>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitProductDiscountIndicator {
    Y,
    N,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitYesNo {
    Yes,
    No,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitProductDetails {
    pub product_code: String,
    pub product_name: String,
    pub price: StringMajorUnit,
    pub quantity: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_discount_details: Option<TsysTransitProductDiscountDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_tax_details: Option<TsysTransitProductTaxDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_variation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_modifier_details: Option<TsysTransitProductModifierDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_discount_indicator: Option<TsysTransitProductDiscountIndicator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_commodity_code: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitRegisteredUserIndicator {
    Yes,
    No,
}
pub(super) fn generate_xml<T: Serialize>(request: &T) -> Result<String, Report<IntegrationError>> {
    let body = quick_xml::se::to_string(request).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}",
        body
    ))
}

fn generate_logged_xml<T: Serialize>(request: &T, fallback_root: &str) -> String {
    let fallback = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<{fallback_root}/>");
    let xml = generate_xml(request).unwrap_or(fallback);
    tracing::info!(
        connector = "tsysTransit",
        root = fallback_root,
        raw_request_xml = %xml,
        "tsysTransit raw connector request"
    );
    xml
}

fn log_tsys_transit_response<T: Debug>(flow: &str, status_code: u16, response: &T) {
    tracing::info!(
        connector = "tsysTransit",
        flow,
        http_status = status_code,
        connector_response = ?response,
        "tsysTransit connector response"
    );
}
#[derive(Debug, Serialize)]
pub enum TsysTransitAuthorizeRequest {
    #[serde(rename = "Sale")]
    Sale(TsysTransitAuthorizeBody),
    #[serde(rename = "Auth")]
    Auth(TsysTransitAuthorizeBody),
}
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TsysTransitRepeatPaymentRequest(pub TsysTransitAuthorizeRequest);

impl GetSoapXml for TsysTransitRepeatPaymentRequest {
    fn to_soap_xml(&self) -> String {
        self.0.to_soap_xml()
    }
}

impl GetSoapXml for TsysTransitAuthorizeRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Sale")
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitAuthorizeBody {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    pub transaction_key: Secret<String>,
    pub card_data_source: TsysTransitCardDataSource,
    pub transaction_amount: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sales_tax: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surcharge: Option<StringMajorUnit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_tax_details: Vec<TsysTransitAdditionalTaxDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_charges: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duty_charges: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ucaf_collection_indicator: Option<String>,
    #[serde(
        rename = "directoryServerTransactionID",
        skip_serializing_if = "Option::is_none"
    )]
    pub directory_server_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_details: Option<TsysTransitWalletDetailsRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_on_file_transaction_identifier: Option<String>,
    #[serde(
        rename = "previousNetworkTransactionID",
        skip_serializing_if = "Option::is_none"
    )]
    pub previous_network_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cit_status_indicator: Option<TsysTransitMcCitStatusIndicator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mit_status_indicator: Option<TsysTransitMitIndicator>,
    pub address_line1: Secret<String>,
    pub zip: Secret<String>,
    #[serde(rename = "externalReferenceID")]
    pub external_reference_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub product_details: Vec<TsysTransitProductDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commercial_card_level: Option<TsysTransitCommercialCardLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_descriptor: Option<String>,
    #[serde(rename = "customerVATNumber", skip_serializing_if = "Option::is_none")]
    pub customer_vat_number: Option<String>,
    #[serde(rename = "customerRefID", skip_serializing_if = "Option::is_none")]
    pub customer_ref_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier_reference_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_commodity_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vat_invoice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_from_zip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_to_zip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_on_file: Option<TsysTransitCardOnFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_auth_support: Option<String>,
    pub terminal_capability: TsysTransitTerminalCapability,
    pub terminal_operating_environment: TsysTransitTerminalOperatingEnvironment,
    pub cardholder_authentication_method: TsysTransitCardholderAuthenticationMethod,
    pub terminal_authentication_capability: TsysTransitTerminalAuthenticationCapability,
    pub terminal_output_capability: TsysTransitTerminalOutputCapability,
    pub max_pin_length: TsysTransitMaxPinLength,
    pub terminal_card_capture_capability: TsysTransitTerminalCardCaptureCapability,
    pub cardholder_present_detail: TsysTransitCardholderPresentDetail,
    pub card_present_detail: TsysTransitCardPresentDetail,
    pub card_data_input_mode: TsysTransitCardDataInputMode,
    pub cardholder_authentication_entity: TsysTransitCardholderAuthenticationEntity,
    pub card_data_output_capability: TsysTransitCardDataOutputCapability,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_recurring: Option<TsysTransitIsRecurring>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_type: Option<TsysTransitBillingType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_payment_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_recurring_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_user_indicator: Option<TsysTransitRegisteredUserIndicator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_registered_change_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_indicator: Option<TsysTransitAuthorizationIndicator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mit: Option<TsysTransitMit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptor_street_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptor_customer_service_phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptor_phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptor_u_r_l_address: Option<url::Url>,
}

#[derive(Debug, Serialize)]
#[serde(rename = "SearchTransaction")]
pub struct TsysTransitTransactionInquiryRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysTransitTransactionInquiryRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "SearchTransaction")
    }
}
pub type TsysTransitRSyncRequest = TsysTransitTransactionInquiryRequest;
#[derive(Debug, Serialize)]
#[serde(rename = "Capture")]
pub struct TsysTransitCaptureRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "transactionAmount")]
    pub transaction_amount: StringMajorUnit,
    #[serde(rename = "salesTax", skip_serializing_if = "Option::is_none")]
    pub sales_tax: Option<StringMajorUnit>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "seqNumber", skip_serializing_if = "Option::is_none")]
    pub seq_number: Option<u32>,
    #[serde(rename = "paymentCount", skip_serializing_if = "Option::is_none")]
    pub payment_count: Option<u32>,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysTransitCaptureRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Capture")
    }
}
#[derive(Debug, Serialize)]
#[serde(rename = "Return")]
pub struct TsysTransitReturnRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "cardDataSource", skip_serializing_if = "Option::is_none")]
    pub card_data_source: Option<TsysTransitCardDataSource>,
    #[serde(rename = "transactionAmount", skip_serializing_if = "Option::is_none")]
    pub transaction_amount: Option<StringMajorUnit>,
    #[serde(rename = "transactionID", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(rename = "cardNumber", skip_serializing_if = "Option::is_none")]
    pub card_number: Option<Secret<String>>,
    #[serde(rename = "expirationDate", skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
    #[serde(rename = "cvv2", skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysTransitReturnRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Return")
    }
}
#[derive(Debug, Serialize)]
#[serde(rename = "Void")]
pub struct TsysTransitVoidRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "transactionAmount", skip_serializing_if = "Option::is_none")]
    pub transaction_amount: Option<StringMajorUnit>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    #[serde(rename = "voidReason")]
    pub void_reason: String,
}

impl GetSoapXml for TsysTransitVoidRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Void")
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "Void")]
pub struct TsysTransitVoidPCRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "transactionAmount", skip_serializing_if = "Option::is_none")]
    pub transaction_amount: Option<StringMajorUnit>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    #[serde(rename = "voidReason")]
    pub void_reason: String,
}

pub type TsysTransitVoidPCResponse = TsysTransitVoidResponse;

impl GetSoapXml for TsysTransitVoidPCRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Void")
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "Void")]
pub struct TsysTransitVoidPostRefundRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "transactionAmount", skip_serializing_if = "Option::is_none")]
    pub transaction_amount: Option<StringMajorUnit>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    #[serde(rename = "voidReason")]
    pub void_reason: String,
}

impl GetSoapXml for TsysTransitVoidPostRefundRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Void")
    }
}
#[derive(Debug, Serialize)]
#[serde(rename = "CardAuthentication")]
pub struct TsysTransitCardAuthenticationRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "cardDataSource")]
    pub card_data_source: TsysTransitCardDataSource,
    #[serde(rename = "cardNumber")]
    pub card_number: Secret<String>,
    #[serde(rename = "expirationDate")]
    pub expiration_date: Secret<String>,
    // TSYS cert: cvv2 must be sent on card authentication when the
    // merchant collected one. The XSD requires it adjacent to
    // expirationDate (early in the body, like Sale/Auth).
    #[serde(rename = "cvv2", skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    // TSYS cert: citStatusIndicator (Mastercard sCIT) was displaced —
    // it sits before addressLine1/zip/externalReferenceID on Sale, not
    // after externalReferenceID. Match that slot here.
    #[serde(rename = "citStatusIndicator", skip_serializing_if = "Option::is_none")]
    pub cit_status_indicator: Option<TsysTransitMcCitStatusIndicator>,
    #[serde(rename = "addressLine1")]
    pub address_line1: Secret<String>,
    #[serde(rename = "zip")]
    pub zip: Secret<String>,
    #[serde(rename = "externalReferenceID")]
    pub external_reference_id: String,
    // TSYS cert: cardOnFile must be sent on Visa CIT-setup card auth
    // (storing credential for future MIT). Schema slot matches Sale —
    // sits between externalReferenceID and terminalCapability.
    #[serde(rename = "cardOnFile", skip_serializing_if = "Option::is_none")]
    pub card_on_file: Option<TsysTransitCardOnFile>,
    // TSYS cert: authorizationIndicator missing on MC card auth.
    #[serde(
        rename = "authorizationIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub authorization_indicator: Option<TsysTransitAuthorizationIndicator>,
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(rename = "middleName", skip_serializing_if = "Option::is_none")]
    pub middle_name: Option<Secret<String>>,
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    #[serde(rename = "terminalCapability")]
    pub terminal_capability: TsysTransitTerminalCapability,
    #[serde(rename = "terminalOperatingEnvironment")]
    pub terminal_operating_environment: TsysTransitTerminalOperatingEnvironment,
    #[serde(rename = "cardholderAuthenticationMethod")]
    pub cardholder_authentication_method: TsysTransitCardholderAuthenticationMethod,
    #[serde(rename = "terminalAuthenticationCapability")]
    pub terminal_authentication_capability: TsysTransitTerminalAuthenticationCapability,
    #[serde(rename = "terminalOutputCapability")]
    pub terminal_output_capability: TsysTransitTerminalOutputCapability,
    #[serde(rename = "maxPinLength")]
    pub max_pin_length: TsysTransitMaxPinLength,
    #[serde(rename = "terminalCardCaptureCapability")]
    pub terminal_card_capture_capability: TsysTransitTerminalCardCaptureCapability,
    #[serde(rename = "cardholderPresentDetail")]
    pub cardholder_present_detail: TsysTransitCardholderPresentDetail,
    #[serde(rename = "cardPresentDetail")]
    pub card_present_detail: TsysTransitCardPresentDetail,
    #[serde(rename = "cardDataInputMode")]
    pub card_data_input_mode: TsysTransitCardDataInputMode,
    #[serde(rename = "cardholderAuthenticationEntity")]
    pub cardholder_authentication_entity: TsysTransitCardholderAuthenticationEntity,
    #[serde(rename = "cardDataOutputCapability")]
    pub card_data_output_capability: TsysTransitCardDataOutputCapability,
    #[serde(
        rename = "acceptorStreetAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub acceptor_street_address: Option<Secret<String>>,
    #[serde(
        rename = "acceptorCustomerServicePhoneNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub acceptor_customer_service_phone_number: Option<Secret<String>>,
    #[serde(
        rename = "acceptorPhoneNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub acceptor_phone_number: Option<Secret<String>>,
    #[serde(rename = "acceptorURLAddress", skip_serializing_if = "Option::is_none")]
    pub acceptor_u_r_l_address: Option<url::Url>,
}

impl GetSoapXml for TsysTransitCardAuthenticationRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "CardAuthentication")
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransitStatus {
    Pass,
    Fail,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum TsysTransitAuthorizeResponse {
    SaleResponse(TsysTransitAuthorizeResponseBody),
    AuthResponse(TsysTransitAuthorizeResponseBody),
}

impl TsysTransitAuthorizeResponse {
    pub fn body(&self) -> &TsysTransitAuthorizeResponseBody {
        match self {
            Self::SaleResponse(b) | Self::AuthResponse(b) => b,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum TsysTransitRepeatPaymentResponse {
    SaleResponse(TsysTransitAuthorizeResponseBody),
    AuthResponse(TsysTransitAuthorizeResponseBody),
}

impl TsysTransitRepeatPaymentResponse {
    pub fn body(&self) -> &TsysTransitAuthorizeResponseBody {
        match self {
            Self::SaleResponse(b) | Self::AuthResponse(b) => b,
        }
    }
    pub fn as_authorize(&self) -> TsysTransitAuthorizeResponse {
        match self {
            Self::SaleResponse(b) => TsysTransitAuthorizeResponse::SaleResponse(b.clone()),
            Self::AuthResponse(b) => TsysTransitAuthorizeResponse::AuthResponse(b.clone()),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct TsysTransitAuthorizeResponseBody {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
    #[serde(rename = "authCode", default)]
    pub auth_code: Option<String>,
    #[serde(rename = "hostReferenceNumber", default)]
    pub host_reference_number: Option<String>,
    #[serde(rename = "hostResponseCode", default)]
    pub host_response_code: Option<String>,
    #[serde(rename = "taskID", default)]
    pub task_id: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "transactionTimestamp", default)]
    pub transaction_timestamp: Option<String>,
    #[serde(rename = "transactionAmount", default)]
    pub transaction_amount: Option<String>,
    #[serde(rename = "processedAmount", default)]
    pub processed_amount: Option<StringMajorUnit>,
    #[serde(rename = "totalAmount", default)]
    pub total_amount: Option<String>,
    #[serde(rename = "addressVerificationCode", default)]
    pub address_verification_code: Option<String>,
    #[serde(rename = "cvvVerificationCode", default)]
    pub cvv_verification_code: Option<String>,
    #[serde(rename = "cardType", default)]
    pub card_type: Option<String>,
    #[serde(rename = "maskedCardNumber", default)]
    pub masked_card_number: Option<String>,
    /// Network transaction identifier returned by TSYS. This is the value to
    /// store as the stored-credential / network-transaction-id for later MITs
    /// — NOT the `authCode` (which is a per-transaction approval code).
    #[serde(rename = "cardTransactionIdentifier", default)]
    pub card_transaction_identifier: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransitTransactionStatus {
    Approved,
    Decline,
    Cancel,
    Void,
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "CaptureResponse")]
pub struct TsysTransitCaptureResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "ReturnResponse")]
pub struct TsysTransitReturnResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "VoidResponse")]
pub struct TsysTransitVoidResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "VoidResponse")]
pub struct TsysTransitVoidPostRefundResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "CardAuthenticationResponse")]
pub struct TsysTransitCardAuthenticationResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "cardTransactionIdentifier", default)]
    pub card_transaction_identifier: Option<String>,
    #[serde(rename = "authCode", default)]
    pub auth_code: Option<String>,
}
pub type TsysTransitRSyncResponse = TsysTransitTransactionInquiryResponse;
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "SearchTransactionResponse")]
pub struct TsysTransitTransactionInquiryResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
    #[serde(rename = "transactionDetails", default)]
    pub transaction_details: Option<TsysTransitTransactionDetails>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TsysTransitTransactionDetails {
    pub transaction_i_d: String,
    pub transaction_type: String,
    pub transaction_status: Option<TsysTransitTransactionStatus>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct TsysTransitPaymentRequestMetadata {
    vat_invoice_number: Option<String>,
    ship_from_zip: Option<String>,
    customer_vat_number: Option<String>,
    summary_commodity_code: Option<String>,
    /// Channel override for the RepeatPayment / MIT flow. The
    /// `RecurringPaymentServiceChargeRequest` proto does not carry
    /// `payment_channel`, so this lets the caller re-inject the MOTO-vs-Ecom
    /// signal. Accepts `"telephone_order"` / `"mail_order"` / `"ecommerce"`.
    #[serde(default)]
    payment_channel: Option<String>,
    /// Installment schedule. TSYS needs both the total number of installments
    /// (`paymentCount`) and the 1-indexed position of this charge
    /// (`currentPaymentCount`); HS' `installment_data` only carries the total
    /// and is not forwarded on the RepeatPayment/MIT flow, so both come from
    /// here.
    #[serde(default)]
    payment_count: Option<u32>,
    #[serde(default)]
    current_payment_count: Option<u32>,
}

#[derive(Debug, Default, Clone)]
struct RecurringContext {
    enabled: bool,
    is_recurring_flag: Option<TsysTransitIsRecurring>,
    billing_type: Option<TsysTransitBillingType>,
    payment_count: Option<u32>,
    current_payment_count: Option<u32>,
    mc_cit_status_indicator: Option<TsysTransitMcCitStatusIndicator>,
    mit_status_indicator: Option<TsysTransitMitIndicator>,
    original_recurring_amount: Option<MinorUnit>,
}

#[derive(Debug, Default, Clone)]
struct CommercialCardContext {
    sales_tax: Option<StringMajorUnit>,
    tax_type: Option<String>,
    tax_amount: Option<StringMajorUnit>,
    tax_rate: Option<String>,
    tax_category: Option<TsysTransitTaxCategory>,
    shipping_charges: Option<StringMajorUnit>,
    duty_charges: Option<StringMajorUnit>,
    product_details: Option<Vec<TsysTransitProductDetails>>,
    commercial_card_level: Option<TsysTransitCommercialCardLevel>,
    purchase_order: Option<String>,
    charge_descriptor: Option<String>,
    customer_vat_number: Option<String>,
    customer_ref_id: Option<String>,
    supplier_reference_number: Option<String>,
    order_date: Option<String>,
    summary_commodity_code: Option<String>,
    vat_invoice: Option<String>,
    ship_from_zip: Option<String>,
    ship_to_zip: Option<String>,
    destination_country_code: Option<String>,
    acceptor_street_address: Option<Secret<String>>,
    acceptor_customer_service_phone_number: Option<Secret<String>>,
    acceptor_phone_number: Option<Secret<String>>,
    acceptor_url: Option<url::Url>,
}

#[derive(Debug, Default, Clone)]
struct ThreeDsContext {
    secure_code: Option<Secret<String>>,
    ucaf_collection_indicator: Option<String>,
    directory_server_transaction_id: Option<String>,
    eci_indicator: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct CardOnFileContext {
    // `card_on_file` is now decided by `rules::cof_mit::card_on_file`
    // based on TxProfile; this struct only carries the raw mandate-
    // derived values that the assembler still needs.
    #[allow(dead_code)]
    card_on_file: Option<TsysTransitCardOnFile>,
    mit_block: Option<TsysTransitMit>,
    previous_network_transaction_id: Option<String>,
    card_on_file_transaction_identifier: Option<String>,
}

fn compute_three_ds_context<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    _card_network: Option<&CardNetwork>,
) -> ThreeDsContext {
    let authentication_data = router_data.request.authentication_data.as_ref();
    let secure_code = authentication_data.and_then(|data| data.cavv.clone());
    let ucaf_collection_indicator =
        authentication_data.and_then(|data| data.ucaf_collection_indicator.clone());
    let directory_server_transaction_id =
        authentication_data.and_then(|data| data.ds_trans_id.clone());
    let eci_indicator = authentication_data.and_then(|data| data.eci.clone());

    ThreeDsContext {
        secure_code,
        ucaf_collection_indicator,
        directory_server_transaction_id,
        eci_indicator,
    }
}
fn compute_recurring_context(
    mit_category: Option<MitCategory>,
    recurring_data: Option<&RecurringMandatePaymentData>,
    card_network: Option<&CardNetwork>,
    payment_meta: Option<&TsysTransitPaymentRequestMetadata>,
) -> Result<RecurringContext, Report<IntegrationError>> {
    let (is_recurring_flag, billing_type) = match mit_category.as_ref() {
        Some(MitCategory::Recurring) => (Some(TsysTransitIsRecurring::Y), None),
        Some(MitCategory::Installment) => (
            Some(TsysTransitIsRecurring::Y),
            Some(TsysTransitBillingType::Installment),
        ),
        Some(MitCategory::Unscheduled) | Some(MitCategory::Resubmission) | None => {
            return Ok(RecurringContext::default())
        }
    };
    // The installment schedule (paymentCount / currentPaymentCount), MC
    // recurring subtype and Discover installment variant come from the payment
    // metadata (`connector_metadata.tsys_transit`). The RepeatPayment / MIT flow
    // does not reliably carry `recurring_mandate_payment_data.mandate_metadata`,
    // so that source is intentionally not consulted here.
    let payment_count = payment_meta.and_then(|m| m.payment_count);
    let current_payment_count = payment_meta.and_then(|m| m.current_payment_count);
    if matches!(mit_category.as_ref(), Some(MitCategory::Installment))
        && (payment_count.is_none() || current_payment_count.is_none())
    {
        return Err(error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "connector_metadata.tsys_transit.{payment_count,current_payment_count}",
            context: Default::default(),
        })
        .attach_printable(
            "tsys_transit: installment MIT requires both `paymentCount` (total number of \
             scheduled installments, e.g. 8) and `currentPaymentCount` (1-indexed position \
             of this charge in the schedule, e.g. 3) on the TransIT Sale/Auth request — \
             without them TSYS rejects the transaction at the gateway. Populate \
             `connector_metadata.tsys_transit.payment_count` and `.current_payment_count` \
             in the payment metadata, or pick a non-Installment `mit_category`. \
             See: https://developerportal.transit-pass.com/developerportal/resources/dist/#/api-specs/./assets/build/api/API3.0/UseCases/index.html",
        ));
    }

    let discover_family_mit_indicator = match (mit_category.as_ref(), card_network) {
        (
            Some(MitCategory::Recurring),
            Some(CardNetwork::Discover)
            | Some(CardNetwork::JCB)
            | Some(CardNetwork::DinersClub)
            | Some(CardNetwork::UnionPay),
        ) => Some(TsysTransitMitIndicator::R),
        (
            Some(MitCategory::Installment),
            Some(CardNetwork::Discover)
            | Some(CardNetwork::JCB)
            | Some(CardNetwork::DinersClub)
            | Some(CardNetwork::UnionPay),
        ) => Some(TsysTransitMitIndicator::S),
        _ => None,
    };
    let (mc_cit_status_indicator, mc_mit_status_indicator) =
        match (mit_category.as_ref(), card_network) {
            (Some(MitCategory::Recurring), Some(CardNetwork::Mastercard)) => (
                Some(TsysTransitMcCitStatusIndicator::C102),
                Some(TsysTransitMitIndicator::M102),
            ),
            (Some(MitCategory::Installment), Some(CardNetwork::Mastercard)) => (
                Some(TsysTransitMcCitStatusIndicator::C104),
                Some(TsysTransitMitIndicator::M104),
            ),
            _ => (None, None),
        };
    let original_recurring_amount = recurring_data
        .and_then(|d| d.original_payment_authorized_amount.as_ref())
        .copied();

    Ok(RecurringContext {
        enabled: true,
        is_recurring_flag,
        billing_type,
        payment_count,
        current_payment_count,
        mc_cit_status_indicator,
        mit_status_indicator: mc_mit_status_indicator.or(discover_family_mit_indicator),
        original_recurring_amount,
    })
}
#[derive(Debug, Clone)]
pub struct TsysTransitAuthType {
    pub device_id: Secret<String>,
    pub transaction_key: Secret<String>,
    pub developer_id: Secret<String>,
    pub merchant_street_address: Option<Secret<String>>,
    pub customer_service_phone_number: Option<Secret<String>>,
    pub merchant_url: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TsysTransitAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::TsysTransit {
                device_id,
                transaction_key,
                developer_id,
                merchant_street_address,
                customer_service_phone_number,
                merchant_url,
                ..
            } => Ok(Self {
                device_id: device_id.to_owned(),
                transaction_key: transaction_key.to_owned(),
                developer_id: developer_id.to_owned(),
                merchant_street_address: merchant_street_address.to_owned(),
                customer_service_phone_number: customer_service_phone_number.to_owned(),
                merchant_url: merchant_url.to_owned(),
            }),
            _ => Err(error_stack::report!(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })
            .attach_printable(
                "tsys_transit: expected `ConnectorSpecificConfig::TsysTransit` with \
                 `device_id`, `transaction_key` and `developer_id` populated on the \
                 merchant connector account. Confirm the MCA is provisioned with the \
                 TSYS TransIT SignatureKey auth type (deviceID + transactionKey from \
                 TSYS' GenKey flow, developerID from your TSYS integration credentials). \
                 See: https://developerportal.transit-pass.com/developerportal/resources/dist/#/api-specs/./assets/build/api/API3.0/UseCases/index.html",
            )),
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TsysTransitErrorResponse {
    #[serde(rename = "status", default, alias = "Status")]
    pub status: Option<String>,
    #[serde(rename = "responseCode", default, alias = "ResponseCode")]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default, alias = "ResponseMessage")]
    pub response_message: Option<String>,
}

fn format_expiration_date(card: &Card<impl PaymentMethodDataTypes>) -> Secret<String> {
    // TSYS TransIT expects `MM/YY`. Zero-pad both halves so a single-digit
    // month or a 2-digit year delivered by the upstream caller still
    // produce a network-valid expiration string (e.g. "3"/"25" -> "03/25",
    // "12"/"2028" -> "12/28").
    let month_raw = card.card_exp_month.peek();
    let year_raw = card.card_exp_year.peek();
    let month = format!("{month_raw:0>2}");
    let year_short = if year_raw.len() >= 4 {
        year_raw[year_raw.len() - 2..].to_string()
    } else {
        format!("{year_raw:0>2}")
    };
    Secret::new(format!("{month}/{year_short}"))
}

fn format_decimal(value: f64) -> String {
    let mut rendered = format!("{value:.4}");
    while rendered.contains('.') && rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.push('0');
    }
    rendered
}

fn truncate_chars(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn sanitize_alphanumeric_space(value: &str, max_len: usize) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace())
        .take(max_len)
        .collect()
}

fn sanitize_optional_alphanumeric_space(value: Option<String>, max_len: usize) -> Option<String> {
    value
        .map(|value| sanitize_alphanumeric_space(&value, max_len))
        .filter(|value| !value.is_empty())
}

fn normalize_tsys_country_code(value: Option<String>) -> Option<String> {
    value.map(|code| match code.as_str() {
        "840" => "USA".to_string(),
        _ => code,
    })
}

fn format_country_alpha3(country: common_enums::CountryAlpha2) -> String {
    common_enums::CountryAlpha2::from_alpha2_to_alpha3(country).to_string()
}

fn build_tsys_product_details(
    detail: &domain_types::payment_address::OrderDetailsWithAmount,
    currency: common_enums::Currency,
    card_network: Option<&CardNetwork>,
    payment_channel: &Option<PaymentChannel>,
) -> Result<Option<TsysTransitProductDetails>, Report<IntegrationError>> {
    if matches!(
        card_network,
        Some(CardNetwork::Visa) | Some(CardNetwork::Mastercard)
    ) {
        let is_visa_card = matches!(card_network, Some(CardNetwork::Visa));

        let missing_field = |field_name: &'static str| {
            IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(format!("Provide {field_name} in the request")),
            doc_url: None,
            additional_context: Some(format!(
                "{field_name} is required for Level III order_details on card_network={card_network:?}"
            )),
        },
    }
        };
        let is_ecommerce_payment =
            matches!(payment_channel, Some(PaymentChannel::Ecommerce) | None);

        let product_code = detail
            .product_id
            .clone()
            .or_else(|| detail.sku.clone())
            .or_else(|| detail.upc.clone())
            .map(|code| sanitize_alphanumeric_space(&code, 20))
            .filter(|code| !code.is_empty())
            .unwrap_or_else(|| sanitize_alphanumeric_space(&detail.product_name, 20));

        let product_name = truncate_chars(&detail.product_name, 50);
        let price = super::TsysTransitAmountConvertor::convert(detail.amount, currency)?;
        let quantity = u32::from(detail.quantity);
        let measurement_unit = detail.unit_of_measure.clone();
        let unit_discount_amount = detail
            .unit_discount_amount
            .map(|amount| super::TsysTransitAmountConvertor::convert(amount, currency))
            .transpose()?;
        let product_tax_name = detail.product_tax_code.clone();

        let product_tax_amount = detail
            .total_tax_amount
            .map(|amount| super::TsysTransitAmountConvertor::convert(amount, currency))
            .transpose()?;

        let product_tax_percentage = detail.tax_rate.map(format_decimal);

        let product_tax_type = detail
            .product_tax_code
            .clone()
            .map(|tax_code| truncate_chars(&tax_code, 4));

        let commodity_code = detail
            .commodity_code
            .clone()
            .or_else(|| detail.upc.clone())
            .or_else(|| detail.product_id.clone())
            .or_else(|| detail.sku.clone())
            .map(|code| sanitize_alphanumeric_space(&code, 12));

        let product_discount_name = detail
            .discount_name
            .clone()
            .map(|discount_type| sanitize_alphanumeric_space(&discount_type, 50));
        let product_discount_percentage = detail.discount_percentage;
        let product_discount_type = detail
            .discount_type
            .clone()
            .map(|discount_type| sanitize_alphanumeric_space(&discount_type, 20));
        let priority = 1;
        let has_discount = detail
            .unit_discount_amount
            .map(|amount| amount.get_amount_as_i64() > 0)
            .unwrap_or(false);
        let stackable = if has_discount {
            TsysTransitYesNo::Yes
        } else {
            TsysTransitYesNo::No
        };
        let product_variation = (!is_ecommerce_payment)
            .then_some({
                detail
                    .sub_category
                    .clone()
                    .or_else(|| detail.category.clone())
            })
            .flatten();
        let product_modifier_details = (!is_ecommerce_payment)
            .then_some({
                detail
                    .brand
                    .clone()
                    .or_else(|| detail.category.clone())
                    .map(|modifier_name| TsysTransitProductModifierDetails {
                        modifier_name: truncate_chars(&modifier_name, 50),
                        modifier_value: detail
                            .sub_category
                            .clone()
                            .or_else(|| detail.description.clone())
                            .map(|value| truncate_chars(&value, 25)),
                        modifier_price: None,
                    })
            })
            .flatten();
        let product_notes = (!is_ecommerce_payment)
            .then_some({
                detail
                    .description
                    .clone()
                    .map(|description| truncate_chars(&description, 100))
            })
            .flatten();

        let product_discount_indicator = (!is_ecommerce_payment).then_some({
            if has_discount {
                TsysTransitProductDiscountIndicator::Y
            } else {
                TsysTransitProductDiscountIndicator::N
            }
        });

        let missing_fields_error = |missing_fields: Vec<&'static str>| {
            IntegrationError::MissingRequiredFields {
            context: IntegrationErrorContext {
                suggested_action: Some(format!(
                    "Provide the following order_details fields: {missing_fields:?}"
                )),
                doc_url: None,
                additional_context: Some(format!(
                    "these fields are required for Level III order_details on card_network={card_network:?}"
                )),
            },
            field_names: missing_fields,
        }
        };

        if matches!(card_network, Some(CardNetwork::Visa)) {
            let missing_fields = collect_missing_value_keys!(
                ("order_details.commodity_code", commodity_code),
                ("order_details.unit_of_measure", measurement_unit),
                ("order_details.unit_discount_amount", unit_discount_amount),
                ("order_details.product_tax_name", product_tax_name),
                ("order_details.product_tax_amount", product_tax_amount),
                (
                    "order_details.product_tax_percentage",
                    product_tax_percentage
                )
            );

            if !missing_fields.is_empty() {
                Err::<(), Report<IntegrationError>>(missing_fields_error(missing_fields).into())?;
            };
        };

        if matches!(card_network, Some(CardNetwork::Mastercard)) {
            let missing_fields = collect_missing_value_keys!(
                ("order_details.unit_of_measure", measurement_unit),
                ("order_details.unit_discount_amount", unit_discount_amount),
                ("order_details.product_tax_name", product_tax_name),
                ("order_details.product_tax_amount", product_tax_amount),
                (
                    "order_details.product_tax_percentage",
                    product_tax_percentage
                ),
                ("order_details.unit_discount_amount", unit_discount_amount)
            );

            if !missing_fields.is_empty() {
                Err::<(), Report<IntegrationError>>(missing_fields_error(missing_fields).into())?;
            };
        };

        Ok(Some(TsysTransitProductDetails {
            product_code,
            product_name,
            price,
            quantity,
            measurement_unit,
            product_discount_details: Some(TsysTransitProductDiscountDetails {
                product_discount_name: product_discount_name
                    .ok_or(missing_field("order_details.discount_name"))?,
                product_discount_amount: unit_discount_amount
                    .ok_or(missing_field("order_details.unit_discount_amount"))?,
                product_discount_percentage: product_discount_percentage
                    .ok_or(missing_field("order_details.discount_percentage"))?,
                product_discount_type: product_discount_type
                    .ok_or(missing_field("order_details.discount_type"))?,
                priority,
                stackable,
            }),
            product_tax_details: Some(TsysTransitProductTaxDetails {
                product_tax_name,
                product_tax_amount,
                product_tax_percentage,
                product_tax_type,
            }),
            product_variation,
            product_modifier_details,
            product_notes,
            product_discount_indicator,
            product_commodity_code: is_visa_card
                .then_some(commodity_code.ok_or(missing_field("order_details.commodity_code")))
                .transpose()?,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
struct MerchantAcceptorInfo {
    street_address: Secret<String>,
    customer_service_phone_number: Secret<String>,
    phone_number: Secret<String>,
    url: url::Url,
}

#[derive(Debug, Clone, Serialize, strum::EnumString)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "snake_case")]
pub enum TsysTransitTaxCategory {
    Service,
    Duty,
    VAT,
    Alternate,
    National,
    #[serde(rename = "TAX_EXEMPT")]
    TaxExempt,
}

fn build_merchant_acceptor_info(
    auth_data: &TsysTransitAuthType,
    card_network: Option<&CardNetwork>,
    payment_channel: Option<&PaymentChannel>,
) -> Result<Option<MerchantAcceptorInfo>, Report<IntegrationError>> {
    if matches!(card_network, Some(CardNetwork::Mastercard))
        && matches!(payment_channel, Some(PaymentChannel::Ecommerce) | None)
    {
        let missing_field = |field_name: &'static str| IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(format!("Provide {field_name} in connector_metadata")),
                doc_url: None,
                additional_context: Some(format!(
                    "{field_name} is required for Mastercard on payment_channel={payment_channel:?}"
                )),
            },
        };

        let street_address = auth_data
            .merchant_street_address
            .clone()
            .ok_or(missing_field(
                "connector_metadata.tsys_transit.merchant_street_address",
            ))?;
        let customer_service_phone_number =
            auth_data
                .customer_service_phone_number
                .clone()
                .ok_or(missing_field(
                    "connector_metadata.tsys_transit.customer_service_phone_number",
                ))?;
        let phone_number = customer_service_phone_number.clone();
        let merchant_url = auth_data.merchant_url.clone().ok_or(missing_field(
            "connector_metadata.tsys_transit.merchant_url",
        ))?;

        let url =
            url::Url::parse(&merchant_url).change_context(IntegrationError::InvalidDataFormat {
                field_name: "connector_metadata.tsys_transit.merchant_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure connector_metadata.tsys_transit.merchant_url is a valid URL"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "failed to parse merchant_url {merchant_url:?} as a URL"
                    )),
                },
            })?;

        Ok(Some(MerchantAcceptorInfo {
            street_address,
            customer_service_phone_number,
            phone_number,
            url,
        }))
    } else {
        Ok(None)
    }
}

fn compute_commercial_card_context<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    card_network: Option<&CardNetwork>,
    auth_data: &TsysTransitAuthType,
) -> Result<CommercialCardContext, Report<IntegrationError>> {
    let merchant_acceptor_info = build_merchant_acceptor_info(
        auth_data,
        card_network,
        router_data.request.payment_channel.as_ref(),
    )?;
    let request_metadata = router_data
        .request
        .metadata
        .as_ref()
        .map(|meta| {
            serde_json::from_value::<TsysTransitPaymentRequestMetadata>(meta.clone().expose())
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "connector_metadata.tsys_transit",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Ensure connector_metadata.tsys_transit matches the expected schema"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "failed to deserialize TsysTransitPaymentRequestMetadata from connector_metadata"
                                .to_string(),
                        ),
                    },
                })
        })
        .transpose()?
        .unwrap_or_default();

    let vat_invoice =
        sanitize_optional_alphanumeric_space(request_metadata.vat_invoice_number.clone(), 15);
    let customer_vat_number = request_metadata.customer_vat_number.clone();
    let ship_from_zip = request_metadata.ship_from_zip.clone();
    let l2_l3_data = router_data.resource_common_data.l2_l3_data.as_deref();
    let order_date = l2_l3_data
        .map(domain_types::connector_types::L2L3Data::get_order_date_mmddyyyy)
        .transpose()?
        .flatten();
    let summary_commodity_code =
        sanitize_optional_alphanumeric_space(request_metadata.summary_commodity_code, 25);
    let acceptor_street_address = merchant_acceptor_info
        .as_ref()
        .map(|info| info.street_address.clone());
    let acceptor_customer_service_phone_number = merchant_acceptor_info
        .as_ref()
        .map(|info| info.customer_service_phone_number.clone());
    let acceptor_phone_number = merchant_acceptor_info
        .as_ref()
        .map(|info| info.phone_number.clone());
    let acceptor_url = merchant_acceptor_info.as_ref().map(|info| info.url.clone());
    let order_details = l2_l3_data
        .and_then(|data| data.get_order_details())
        .or_else(|| router_data.resource_common_data.order_details.clone())
        .unwrap_or_default();
    let product_details: Option<Vec<TsysTransitProductDetails>> = order_details
        .iter()
        .map(|detail| {
            build_tsys_product_details(
                detail,
                router_data.request.currency,
                card_network,
                &router_data.request.payment_channel,
            )
        })
        .collect::<Result<Vec<Option<TsysTransitProductDetails>>, Report<IntegrationError>>>()
        .ok()
        .and_then(|items| items.into_iter().collect());
    let shipping_charges = l2_l3_data
        .and_then(|data| data.get_shipping_cost())
        .or(router_data.request.shipping_cost)
        .map(|amount| {
            super::TsysTransitAmountConvertor::convert(amount, router_data.request.currency)
        })
        .transpose()?;
    let duty_charges = l2_l3_data
        .and_then(|data| data.get_duty_amount())
        .map(|amount| {
            super::TsysTransitAmountConvertor::convert(amount, router_data.request.currency)
        })
        .transpose()?;
    let derived_tax_rate = order_details
        .iter()
        .find_map(|detail| detail.tax_rate.map(format_decimal));
    let derived_tax_type = order_details
        .iter()
        .find_map(|detail| detail.product_tax_code.clone())
        .filter(|value| !value.is_empty());
    let order_tax_amount = l2_l3_data
        .and_then(|data| data.get_order_tax_amount())
        .or(router_data.request.order_tax_amount);
    let sales_tax = order_tax_amount
        .map(|amount| {
            super::TsysTransitAmountConvertor::convert(amount, router_data.request.currency)
        })
        .transpose()?;
    let tax_amount = sales_tax.clone();
    let tax_category = derived_tax_type.clone().map(|value| value.parse::<TsysTransitTaxCategory>()).transpose().change_context(
        IntegrationError::InvalidDataFormat {
            field_name: "order_details.product_tax_code",
            context: IntegrationErrorContext {
                suggested_action: Some("Ensure that the product_tax_code is one of the valid TSYS TransIT tax categories: service, duty, vat, alternate, national and tax_exempt".to_string()),
                doc_url: None,
                additional_context: Some(format!(
                    "failed to parse product_tax_code {derived_tax_type:?} as a TsysTransitTaxCategory"
                )),
            },
        },
    )?;
    let order_reference = l2_l3_data
        .and_then(|data| data.get_merchant_order_reference_id())
        .or_else(|| router_data.request.merchant_order_id.clone());
    let connector_request_reference_id = router_data
        .resource_common_data
        .connector_request_reference_id
        .clone();
    let purchase_order = sanitize_optional_alphanumeric_space(
        order_reference
            .clone()
            .or_else(|| Some(connector_request_reference_id.clone())),
        25,
    );
    let shipping_address = router_data.resource_common_data.get_shipping_address().ok();
    let billing_address = router_data.resource_common_data.get_billing_address().ok();
    let billing_descriptor = router_data.request.billing_descriptor.as_ref();

    let ship_to_zip = l2_l3_data
        .and_then(|data| data.get_shipping_zip())
        .map(|zip| zip.expose())
        .or_else(|| {
            shipping_address
                .and_then(|address| address.zip.clone())
                .map(|zip| zip.expose())
        })
        .or_else(|| {
            billing_address
                .and_then(|address| address.zip.clone())
                .map(|zip| zip.expose())
        });

    let destination_country_code = normalize_tsys_country_code(
        l2_l3_data
            .and_then(|data| data.get_shipping_country())
            .map(format_country_alpha3)
            .or_else(|| {
                shipping_address
                    .and_then(|address| address.country)
                    .map(format_country_alpha3)
            })
            .or_else(|| {
                billing_address
                    .and_then(|address| address.country)
                    .map(format_country_alpha3)
            }),
    );

    let supplier_reference_number = sanitize_optional_alphanumeric_space(
        order_reference
            .clone()
            .or_else(|| Some(connector_request_reference_id.clone())),
        9,
    );

    let customer_ref_id = sanitize_optional_alphanumeric_space(
        order_reference
            .clone()
            .or_else(|| Some(connector_request_reference_id.clone())),
        17,
    );

    let charge_descriptor = billing_descriptor.and_then(|descriptor| {
        sanitize_optional_alphanumeric_space(
            descriptor
                .statement_descriptor
                .clone()
                .or_else(|| descriptor.reference.clone())
                .or_else(|| descriptor.name.as_ref().map(|name| name.clone().expose())),
            25,
        )
    });

    let is_visa_and_mastercard_level3_common_field_present = tax_amount.is_some()
        && tax_category.is_some()
        && derived_tax_type.is_some()
        && derived_tax_rate.is_some()
        && shipping_charges.is_some()
        && duty_charges.is_some()
        && product_details.is_some()
        && purchase_order.is_some()
        && order_date.is_some()
        && summary_commodity_code.is_some()
        && vat_invoice.is_some()
        && ship_from_zip.is_some()
        && ship_to_zip.is_some()
        && destination_country_code.is_some();

    let is_level3 = match card_network {
        Some(CardNetwork::Visa) => {
            is_visa_and_mastercard_level3_common_field_present
                && customer_vat_number.is_some()
                && sales_tax.is_some()
        }
        Some(CardNetwork::Mastercard) => is_visa_and_mastercard_level3_common_field_present,
        _ => false,
    };

    let is_level2 = match card_network {
        Some(CardNetwork::AmericanExpress) => {
            supplier_reference_number.is_some()
                && sales_tax.is_some()
                && ship_to_zip.is_some()
                && charge_descriptor.is_some()
                && customer_ref_id.is_some()
        }
        Some(CardNetwork::Visa) | Some(CardNetwork::Mastercard) => {
            sales_tax.is_some() && purchase_order.is_some()
        }
        _ => false,
    };

    let commercial_card_level = if is_level3 {
        Some(TsysTransitCommercialCardLevel::Level3)
    } else if is_level2 {
        Some(TsysTransitCommercialCardLevel::Level2)
    } else {
        None
    };

    Ok(CommercialCardContext {
        sales_tax,
        tax_type: derived_tax_type,
        tax_amount,
        tax_rate: derived_tax_rate,
        tax_category,
        shipping_charges,
        duty_charges,
        product_details,
        commercial_card_level,
        purchase_order,
        charge_descriptor,
        customer_vat_number,
        customer_ref_id,
        supplier_reference_number,
        order_date,
        summary_commodity_code,
        vat_invoice,
        ship_from_zip,
        ship_to_zip,
        destination_country_code,
        acceptor_customer_service_phone_number,
        acceptor_street_address,
        acceptor_url,
        acceptor_phone_number,
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysTransitAuthorizeRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        validate_authorize_request(&item.router_data)?;

        let assembly = extract_for_authorize(&item)?;
        let is_manual_capture = assembly.profile.capture.is_manual();
        let body = assemble_authorize_body(assembly)?;
        Ok(if is_manual_capture {
            Self::Auth(body)
        } else {
            Self::Sale(body)
        })
    }
}

fn validate_off_session_ecommerce_support(
    setup_future_usage: Option<FutureUsage>,
    payment_channel: Option<&PaymentChannel>,
) -> Result<(), Report<IntegrationError>> {
    let is_off_session = setup_future_usage == Some(FutureUsage::OffSession);

    let is_ecommerce_payment = matches!(payment_channel, Some(PaymentChannel::Ecommerce) | None);

    if is_off_session && is_ecommerce_payment {
        Err(IntegrationError::NotSupported {
            message: "Off-session e-commerce payments".to_string(),
            connector: "tsysTransit",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Use a different connector or payment channel that supports off-session payments"
                        .to_string(),
                ),
                doc_url: None,
                additional_context: None
            },
        }
        .into())
    } else {
        Ok(())
    }
}

fn validate_authorize_request<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<(), Report<IntegrationError>> {
    validate_off_session_ecommerce_support(
        item.request.setup_future_usage,
        item.request.payment_channel.as_ref(),
    )
}

struct AuthorizeAssembly {
    profile: TxProfile,
    auth: TsysTransitAuthType,
    transaction_amount: StringMajorUnit,
    surcharge: Option<StringMajorUnit>,
    address_line1: Secret<String>,
    zip: Secret<String>,
    external_reference_id: String,
    card_number: Option<Secret<String>>,
    expiration_date: Option<Secret<String>>,
    cvv2: Option<Secret<String>>,
    customer_code: Option<Secret<String>>,
    wallet_details: Option<TsysTransitWalletDetailsRef>,
    cvv_present_for_authorize: bool,
    recurring_context: RecurringContext,
    commercial_card_context: CommercialCardContext,
    three_ds_context: ThreeDsContext,
    card_on_file_context: CardOnFileContext,
    original_recurring_amount: Option<StringMajorUnit>,
}

fn extract_for_authorize<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>(
    item: &TsysTransitRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Result<AuthorizeAssembly, Report<IntegrationError>> {
    let router_data = &item.router_data;
    let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;
    let mandate_dispatch = decode_mandate_dispatch(router_data.request.mandate_id.as_ref());
    let is_cit_setup = matches!(mandate_dispatch, MandateDispatch::None)
        && (router_data.request.setup_future_usage == Some(FutureUsage::OffSession)
            || router_data.request.off_session == Some(true));
    let card_opt = match &router_data.request.payment_method_data {
        PaymentMethodData::Card(card) => Some(card),
        _ => None,
    };
    let nti_card_opt: Option<&CardDetailsForNetworkTransactionId> =
        match &router_data.request.payment_method_data {
            PaymentMethodData::CardDetailsForNetworkTransactionId(nti) => Some(nti),
            _ => None,
        };
    if matches!(mandate_dispatch, MandateDispatch::Vault { .. }) {
    } else if card_opt.is_none() && nti_card_opt.is_none() {
        return Err(IntegrationError::NotSupported {
            message: "Selected payment method".to_string(),
            connector: "tsysTransit",
            context: Default::default(),
        }
        .into());
    }
    let card = card_opt;

    let transaction_amount = super::TsysTransitAmountConvertor::convert(
        router_data.request.minor_amount,
        router_data.request.currency,
    )?;
    let surcharge = router_data
        .request
        .surcharge_amount
        .as_ref()
        .map(|amount| super::TsysTransitAmountConvertor::convert(amount.amount, amount.currency))
        .transpose()?;
    let billing = router_data
        .resource_common_data
        .address
        .get_payment_billing()
        .and_then(|b| b.address.as_ref());
    let address_line1 = billing.and_then(|a| a.line1.clone()).ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "billing.address.line1",
            context: Default::default(),
        })
    })?;
    let zip = billing.and_then(|a| a.zip.clone()).ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "billing.address.zip",
            context: Default::default(),
        })
    })?;
    let card_network = card
        .and_then(|c| c.card_network.clone())
        .or_else(|| nti_card_opt.and_then(|n| n.card_network.clone()));
    // Recurring / installment scheduling (payment_channel, paymentCount,
    // currentPaymentCount) is read from `connector_metadata.tsys_transit`.
    let request_metadata = router_data
        .request
        .metadata
        .as_ref()
        .and_then(|m| {
            serde_json::from_value::<TsysTransitPaymentRequestMetadata>(m.clone().expose()).ok()
        })
        .unwrap_or_default();
    let recurring_context = compute_recurring_context(
        router_data.request.mit_category.clone(),
        router_data
            .resource_common_data
            .recurring_mandate_payment_data
            .as_ref(),
        card_network.as_ref(),
        Some(&request_metadata),
    )?;
    let commercial_card_context =
        compute_commercial_card_context(router_data, card_network.as_ref(), &auth)?;
    let three_ds_context = compute_three_ds_context(router_data, card_network.as_ref());

    let profile =
        TxProfile::derive_for_authorize(router_data, commercial_card_context.commercial_card_level);
    let cvv_present_for_authorize = card.map(|c| !c.card_cvc.peek().is_empty()).unwrap_or(false);

    let (card_number, expiration_date, cvv2, customer_code, wallet_details) =
        if let MandateDispatch::Vault {
            customer_code,
            wallet_id,
        } = &mandate_dispatch
        {
            (
                None,
                None,
                None,
                Some(Secret::new(customer_code.clone())),
                Some(TsysTransitWalletDetailsRef {
                    wallet_id: Secret::new(wallet_id.clone()),
                }),
            )
        } else if let Some(card) = card {
            let cvv = if recurring_context.enabled {
                None
            } else if card.card_cvc.peek().is_empty() {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "card_cvc required for TSYS XML authorization cvv2",
                    context: Default::default(),
                }
                .into());
            } else {
                Some(card.card_cvc.clone())
            };
            (
                Some(Secret::new(card.card_number.peek().to_string())),
                Some(format_expiration_date(card)),
                cvv,
                None,
                None,
            )
        } else if let Some(nti) = nti_card_opt {
            let month = nti.card_exp_month.peek().clone();
            let year_full = nti.card_exp_year.peek().clone();
            let year_short = if year_full.len() == 4 {
                year_full[2..].to_string()
            } else {
                year_full
            };
            (
                Some(Secret::new(nti.card_number.peek().to_string())),
                Some(Secret::new(format!("{}/{}", month, year_short))),
                None,
                None,
                None,
            )
        } else {
            return Err(IntegrationError::NotSupported {
                message: "Selected payment method".to_string(),
                connector: "tsysTransit",
                context: Default::default(),
            }
            .into());
        };

    let card_on_file_context = build_card_on_file_context(
        &mandate_dispatch,
        recurring_context.enabled,
        card_network.as_ref(),
        is_cit_setup,
    );
    let original_recurring_amount = compute_original_recurring_amount(
        &recurring_context,
        card_network.as_ref(),
        &mandate_dispatch,
        router_data.request.currency,
    )?;
    let external_reference_id = sanitize_alphanumeric_space(
        &router_data
            .resource_common_data
            .connector_request_reference_id,
        40,
    );

    Ok(AuthorizeAssembly {
        profile,
        auth,
        transaction_amount,
        surcharge,
        address_line1,
        zip,
        external_reference_id,
        card_number,
        expiration_date,
        cvv2,
        customer_code,
        wallet_details,
        cvv_present_for_authorize,
        recurring_context,
        commercial_card_context,
        three_ds_context,
        card_on_file_context,
        original_recurring_amount,
    })
}

fn assemble_authorize_body(
    assembly: AuthorizeAssembly,
) -> Result<TsysTransitAuthorizeBody, Report<IntegrationError>> {
    let AuthorizeAssembly {
        profile,
        auth,
        transaction_amount,
        surcharge,
        address_line1,
        zip,
        external_reference_id,
        card_number,
        expiration_date,
        cvv2,
        customer_code,
        wallet_details,
        cvv_present_for_authorize,
        recurring_context,
        commercial_card_context,
        three_ds_context,
        card_on_file_context,
        original_recurring_amount,
    } = assembly;
    let terminal_data = rules::terminal_data::terminal_data(&profile);

    // ── terminalData fields (merchant overrides win) ─────────────
    let rules::terminal_data::ResolvedTerminalData {
        card_data_source,
        terminal_capability,
        terminal_operating_environment,
        cardholder_authentication_method,
        terminal_authentication_capability,
        terminal_output_capability,
        max_pin_length,
        terminal_card_capture_capability,
        cardholder_present_detail,
        card_present_detail,
        card_data_input_mode,
        cardholder_authentication_entity,
        card_data_output_capability,
    } = rules::terminal_data::resolve(&profile, &terminal_data, cvv_present_for_authorize);

    // ── Network indicators via rules ─────────────────────────────
    let authorization_indicator = rules::network_indicators::authorization_indicator(&profile);
    let (registered_user_indicator, last_registered_change_date) =
        match rules::network_indicators::registered_user(&profile) {
            Some((ind, date)) => (Some(ind), Some(date)),
            None => (None, None),
        };

    // ── COF / MIT signaling via rules ────────────────────────────
    let card_on_file_from_rule = rules::cof_mit::card_on_file(&profile);
    // citStatusIndicator and mitStatusIndicator are MUTUALLY EXCLUSIVE
    // (TSYS rejects both on one transaction). The cof_phase decides which:
    // MIT → mitStatusIndicator only; CIT-setup / CIT-using-stored → cit only.
    // Network-specific MC values (M102/M103/M104, C102/C103) come from the
    // recurring metadata subtype when present, else the generic rule default.
    let (mit_status_indicator, cit_status_indicator) = if profile.cof_phase.is_mit() {
        let mit = recurring_context
            .mit_status_indicator
            .or_else(|| rules::cof_mit::mit_status_indicator(&profile));
        (mit, None)
    } else {
        let cit = recurring_context
            .mc_cit_status_indicator
            .or_else(|| rules::cof_mit::cit_status_indicator(&profile));
        (None, cit)
    };
    // CIT-using-stored must NOT carry cardOnFileTransactionIdentifier
    // (MIT-only tag).
    let card_on_file_transaction_identifier =
        if rules::cof_mit::should_send_card_on_file_transaction_identifier(&profile) {
            card_on_file_context
                .card_on_file_transaction_identifier
                .clone()
        } else {
            None
        };
    // The `mit` block on the body comes from build_card_on_file_context
    // for vault flows; rules decide whether to send it.
    let mit_block_for_body = if profile.cof_phase.is_mit() {
        card_on_file_context.mit_block.clone()
    } else {
        None
    };

    let partial_auth_support = rules::network_indicators::partial_auth_support(&profile);

    let additional_tax_details = rules::commercial::additional_tax_details(
        &profile,
        commercial_card_context.tax_type,
        commercial_card_context.tax_amount,
        commercial_card_context.tax_rate,
        commercial_card_context.tax_category,
    )?;
    let sales_tax = rules::commercial::sales_tax(&profile, commercial_card_context.sales_tax)?;
    let customer_vat_number = rules::commercial::customer_vat_number(
        &profile,
        commercial_card_context.customer_vat_number,
    )?;
    let shipping_charges =
        rules::commercial::shipping_charges(&profile, commercial_card_context.shipping_charges)?;
    let duty_charges =
        rules::commercial::duty_charges(&profile, commercial_card_context.duty_charges)?;
    let product_details =
        rules::commercial::product_details(&profile, commercial_card_context.product_details)?;
    let order_date = rules::commercial::order_date(&profile, commercial_card_context.order_date)?;
    let summary_commodity_code = rules::commercial::summary_commodity_code(
        &profile,
        commercial_card_context.summary_commodity_code,
    )?;
    let vat_invoice =
        rules::commercial::vat_invoice(&profile, commercial_card_context.vat_invoice)?;
    let ship_from_zip =
        rules::commercial::ship_from_zip(&profile, commercial_card_context.ship_from_zip)?;
    let destination_country_code = rules::commercial::destination_country_code(
        &profile,
        commercial_card_context.destination_country_code,
    )?;
    let purchase_order =
        rules::commercial::purchase_order(&profile, commercial_card_context.purchase_order)?;
    let charge_descriptor =
        rules::commercial::charge_descriptor(&profile, commercial_card_context.charge_descriptor)?;
    let customer_ref_id =
        rules::commercial::customer_ref_id(&profile, commercial_card_context.customer_ref_id)?;
    let supplier_reference_number = rules::commercial::supplier_reference_number(
        &profile,
        commercial_card_context.supplier_reference_number,
    )?;
    let ship_to_zip =
        rules::commercial::ship_to_zip(&profile, commercial_card_context.ship_to_zip)?;

    Ok(TsysTransitAuthorizeBody {
        device_id: auth.device_id,
        transaction_key: auth.transaction_key,
        card_data_source,
        transaction_amount,
        sales_tax,
        surcharge,
        additional_tax_details,
        shipping_charges,
        duty_charges,
        card_number,
        expiration_date,
        cvv2,
        secure_code: three_ds_context.secure_code,
        ucaf_collection_indicator: three_ds_context.ucaf_collection_indicator,
        directory_server_transaction_id: three_ds_context.directory_server_transaction_id,
        eci_indicator: three_ds_context.eci_indicator,
        customer_code,
        wallet_details,
        card_on_file_transaction_identifier,
        previous_network_transaction_id: card_on_file_context.previous_network_transaction_id,
        cit_status_indicator,
        mit_status_indicator,
        address_line1,
        zip,
        external_reference_id,
        product_details,
        commercial_card_level: commercial_card_context.commercial_card_level,
        // Per-field commercial gating from rules::commercial.
        purchase_order,
        charge_descriptor,
        customer_vat_number,
        customer_ref_id,
        supplier_reference_number,
        order_date,
        summary_commodity_code,
        vat_invoice,
        ship_from_zip,
        ship_to_zip,
        destination_country_code,
        card_on_file: card_on_file_from_rule,
        partial_auth_support,
        terminal_capability,
        terminal_operating_environment,
        cardholder_authentication_method,
        terminal_authentication_capability,
        terminal_output_capability,
        max_pin_length,
        terminal_card_capture_capability,
        cardholder_present_detail,
        card_present_detail,
        card_data_input_mode,
        cardholder_authentication_entity,
        card_data_output_capability,
        developer_id: auth.developer_id,
        is_recurring: recurring_context.is_recurring_flag,
        billing_type: recurring_context.billing_type,
        payment_count: recurring_context.payment_count,
        current_payment_count: recurring_context.current_payment_count,
        original_recurring_amount,
        registered_user_indicator,
        last_registered_change_date,
        authorization_indicator,
        mit: mit_block_for_body,
        acceptor_street_address: commercial_card_context.acceptor_street_address,
        acceptor_customer_service_phone_number: commercial_card_context
            .acceptor_customer_service_phone_number,
        acceptor_phone_number: commercial_card_context.acceptor_phone_number,
        acceptor_u_r_l_address: commercial_card_context.acceptor_url,
    })
}
#[derive(Debug, Clone)]
pub enum MandateDispatch {
    Vault {
        customer_code: String,
        wallet_id: String,
    },
    Ntid {
        ntid: String,
    },
    None,
}
fn decode_mandate_dispatch(mandate_id: Option<&MandateIds>) -> MandateDispatch {
    let Some(mandate_id) = mandate_id else {
        return MandateDispatch::None;
    };

    if let Some(MandateReferenceId::ConnectorMandateId(connector_mandate_ids)) =
        mandate_id.mandate_reference_id.as_ref()
    {
        if let Some(raw) = connector_mandate_ids.get_connector_mandate_id() {
            return decode_mandate_id_string(&raw);
        }
    }
    if let Some(MandateReferenceId::NetworkMandateId(ntid)) =
        mandate_id.mandate_reference_id.as_ref()
    {
        return MandateDispatch::Ntid {
            ntid: ntid.network_transaction_id.clone(),
        };
    }

    MandateDispatch::None
}

fn visa_card_on_file(card_network: Option<&CardNetwork>) -> Option<TsysTransitCardOnFile> {
    matches!(card_network, Some(CardNetwork::Visa)).then_some(TsysTransitCardOnFile::Y)
}

fn build_card_on_file_context(
    mandate_dispatch: &MandateDispatch,
    is_recurring: bool,
    card_network: Option<&CardNetwork>,
    is_cit_setup: bool,
) -> CardOnFileContext {
    match (mandate_dispatch, is_recurring, card_network) {
        (MandateDispatch::Ntid { .. }, true, Some(CardNetwork::Mastercard))
        | (MandateDispatch::Ntid { .. }, true, Some(CardNetwork::AmericanExpress)) => {
            CardOnFileContext::default()
        }
        (MandateDispatch::Ntid { ntid }, _, _) => CardOnFileContext {
            card_on_file: visa_card_on_file(card_network),
            card_on_file_transaction_identifier: Some(ntid.clone()),
            ..Default::default()
        },
        (MandateDispatch::Vault { .. }, true, _) => CardOnFileContext {
            card_on_file: visa_card_on_file(card_network),
            ..Default::default()
        },
        (MandateDispatch::Vault { .. }, false, _) => CardOnFileContext {
            card_on_file: visa_card_on_file(card_network),
            mit_block: Some(TsysTransitMit {
                mit_indicator: TsysTransitMitIndicator::R,
            }),
            ..Default::default()
        },
        (MandateDispatch::None, _, _) if is_cit_setup => CardOnFileContext {
            card_on_file: visa_card_on_file(card_network),
            ..Default::default()
        },
        (MandateDispatch::None, _, _) => CardOnFileContext::default(),
    }
}

fn compute_original_recurring_amount(
    recurring_context: &RecurringContext,
    card_network: Option<&CardNetwork>,
    mandate_dispatch: &MandateDispatch,
    currency: common_enums::Currency,
) -> Result<Option<StringMajorUnit>, Report<IntegrationError>> {
    match (
        recurring_context.original_recurring_amount,
        card_network,
        mandate_dispatch,
    ) {
        (
            Some(amount),
            Some(CardNetwork::Discover)
            | Some(CardNetwork::JCB)
            | Some(CardNetwork::DinersClub)
            | Some(CardNetwork::UnionPay),
            MandateDispatch::Ntid { .. } | MandateDispatch::Vault { .. },
        ) => Ok(Some(super::TsysTransitAmountConvertor::convert(
            amount, currency,
        )?)),
        _ => Ok(None),
    }
}

fn decode_mandate_id_string(raw: &str) -> MandateDispatch {
    if let Some(rest) = raw.strip_prefix("cust:") {
        let mut parts = rest.splitn(2, ':');
        match (parts.next(), parts.next()) {
            (Some(customer_code), Some(wallet_id))
                if !customer_code.is_empty() && !wallet_id.is_empty() =>
            {
                return MandateDispatch::Vault {
                    customer_code: customer_code.to_string(),
                    wallet_id: wallet_id.to_string(),
                };
            }
            _ => {}
        }
    }
    // Network transaction id is never stored as a connector_mandate_id (PSP
    // token) for tsys_transit — it arrives via `NetworkMandateId` and is
    // decoded in `decode_mandate_dispatch`. Nothing writes an `ntid:` prefixed
    // connector_mandate_id, so there is nothing to strip here.
    MandateDispatch::None
}
fn map_authorize_status(response: &TsysTransitAuthorizeResponse) -> AttemptStatus {
    let body = response.body();
    match (
        body.status.as_ref(),
        body.response_code.as_deref(),
        response,
    ) {
        (
            Some(TsysTransitStatus::Pass),
            Some("A0000"),
            TsysTransitAuthorizeResponse::SaleResponse(_),
        ) => AttemptStatus::Charged,
        (
            Some(TsysTransitStatus::Pass),
            Some("A0000"),
            TsysTransitAuthorizeResponse::AuthResponse(_),
        ) => AttemptStatus::Authorized,
        // A0002 — partially approved. For Sale this is a partial capture
        // (money moved), for Auth this is a partial authorization
        // (no capture yet; the capture flow will move the approved amount).
        (
            Some(TsysTransitStatus::Pass),
            Some("A0002"),
            TsysTransitAuthorizeResponse::SaleResponse(_),
        ) => AttemptStatus::PartialCharged,
        (
            Some(TsysTransitStatus::Pass),
            Some("A0002"),
            TsysTransitAuthorizeResponse::AuthResponse(_),
        ) => AttemptStatus::PartiallyAuthorized,
        (Some(TsysTransitStatus::Fail), _, _) => AttemptStatus::Failure,
        // TODO(tsys-cert): confirm with TSYS whether any other Pass codes
        // (soft / referral / "call issuer") should be mapped to Pending
        // rather than Failure. Until then we conservatively fail.
        _ => AttemptStatus::Failure,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TsysTransitAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("Authorize", item.http_code, response);
        let body = response.body();

        let status = map_authorize_status(response);
        if matches!(status, AttemptStatus::Failure) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: body
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: body
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: body.response_message.clone(),
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: body.transaction_id.clone(),
                    network_decline_code: body.host_response_code.clone(),
                    network_advice_code: None,
                    network_error_message: body.response_message.clone(),
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }
        let transaction_id = body.transaction_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsysTransit: success response missing <transactionID>; confirm API contract.",
            )
        })?;

        // <processedAmount> reflects settled funds, so amount_captured must be
        // populated ONLY when the transaction actually captured: a Sale (Charged)
        // or a partial approval (PartialCharged). For an auth-only (Authorized)
        // manual-capture flow nothing is captured yet — populating it there made
        // HS record amount_received == amount at authorization, pushing later
        // voids/captures into a wrong partially-captured state.
        let is_settled = matches!(
            status,
            AttemptStatus::Charged | AttemptStatus::PartialCharged
        );
        let minor_amount_captured = is_settled
            .then(|| {
                body.processed_amount.as_ref().and_then(|amount| {
                    crate::connectors::tsys_transit::TsysTransitAmountConvertor::convert_back(
                        amount.clone(),
                        router_data.request.currency,
                    )
                    .ok()
                })
            })
            .flatten();
        let amount_captured = minor_amount_captured.map(|m| m.get_amount_as_i64());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            // Store the network transaction identifier (NOT the per-transaction
            // authCode) so later Mastercard/Visa MITs replay the correct
            // stored-credential reference. See cert MOTO rows 161/162 — the
            // authCode (e.g. "VTLMC1") must never surface as
            // cardOnFileTransactionIdentifier.
            network_txn_id: body.card_transaction_identifier.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                minor_amount_captured,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for TsysTransitTransactionInquiryRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;

        let transaction_id = router_data.request.get_connector_transaction_id()?;

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
        })
    }
}

impl From<&TsysTransitTransactionDetails> for AttemptStatus {
    fn from(item: &TsysTransitTransactionDetails) -> Self {
        let transaction_type = item.transaction_type.to_lowercase();
        if transaction_type.contains("auth") && transaction_type.contains("void") {
            match item.transaction_status {
                Some(TsysTransitTransactionStatus::Approved) => Self::Voided,
                Some(TsysTransitTransactionStatus::Decline)
                | Some(TsysTransitTransactionStatus::Cancel)
                | Some(TsysTransitTransactionStatus::Void) => Self::VoidFailed,
                None => Self::Unspecified,
            }
        } else if transaction_type.contains("sale") {
            match item.transaction_status {
                Some(TsysTransitTransactionStatus::Approved) => Self::Charged,
                Some(TsysTransitTransactionStatus::Decline)
                | Some(TsysTransitTransactionStatus::Cancel)
                | Some(TsysTransitTransactionStatus::Void) => Self::Failure,
                None => Self::Unspecified,
            }
        } else if transaction_type.contains("auth") {
            match item.transaction_status {
                Some(TsysTransitTransactionStatus::Approved) => Self::Authorized,
                Some(TsysTransitTransactionStatus::Decline)
                | Some(TsysTransitTransactionStatus::Cancel)
                | Some(TsysTransitTransactionStatus::Void) => Self::AuthorizationFailed,
                None => Self::Unspecified,
            }
        } else {
            Self::Unspecified
        }
    }
}

impl TryFrom<ResponseRouterData<TsysTransitTransactionInquiryResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitTransactionInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("PSync", item.http_code, response);

        if let Some(transaction_details) = response.transaction_details.as_ref() {
            // Incase of failure error message is not returned in sync call
            let connector_transaction_id = transaction_details.transaction_i_d.clone();
            let status = AttemptStatus::from(transaction_details);
            let payments_response_data = PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Ok(payments_response_data),
                ..router_data.clone()
            })
        } else {
            let payments_response_data = ErrorResponse {
                status_code: item.http_code,
                code: response
                    .response_code
                    .clone()
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                message: response
                    .response_message
                    .clone()
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                reason: response.response_message.clone(),
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: response.response_message.clone(),
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            };

            Ok(Self {
                response: Err(payments_response_data),
                ..router_data.clone()
            })
        }
    }
}

fn compute_capture_sales_tax(
    router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> Result<Option<StringMajorUnit>, Report<IntegrationError>> {
    router_data
        .request
        .order_tax_amount
        .map(|amount| {
            super::TsysTransitAmountConvertor::convert(amount, router_data.request.currency)
        })
        .transpose()
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TsysTransitCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = router_data.request.get_connector_transaction_id()?;

        let transaction_amount = super::TsysTransitAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;
        let sales_tax = compute_capture_sales_tax(router_data)?;
        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
            transaction_amount,
            sales_tax,
            seq_number: None,
            payment_count: None,
        })
    }
}
fn map_capture_status(response: &TsysTransitCaptureResponse) -> AttemptStatus {
    match (response.status.as_ref(), response.response_code.as_deref()) {
        (Some(TsysTransitStatus::Pass), Some("A0000")) => AttemptStatus::Charged,
        (Some(TsysTransitStatus::Pass), Some("A0002")) => AttemptStatus::PartialCharged,
        (Some(TsysTransitStatus::Fail), _) => AttemptStatus::CaptureFailed,
        _ => AttemptStatus::CaptureFailed,
    }
}

impl TryFrom<ResponseRouterData<TsysTransitCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("Capture", item.http_code, response);

        let status = map_capture_status(response);

        if matches!(status, AttemptStatus::CaptureFailed) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }
        let connector_txn_id = match response.transaction_id.clone() {
            Some(id) => id,
            None => router_data
                .request
                .get_connector_transaction_id()
                .map_err(|_| {
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                        "tsysTransit: Capture response missing <transactionID> and request had none.",
                    )
                })?,
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TsysTransitReturnRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;

        let transaction_amount = super::TsysTransitAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        if !connector_transaction_id.is_empty() {
            Ok(Self {
                device_id: auth.device_id,
                transaction_key: auth.transaction_key,
                developer_id: auth.developer_id,
                transaction_id: Some(connector_transaction_id),
                card_data_source: None,
                card_number: None,
                expiration_date: None,
                cvv2: None,
                transaction_amount: Some(transaction_amount),
            })
        } else {
            Err(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data for unreferenced refund",
                context: Default::default(),
            }
            .into())
        }
    }
}

fn map_refund_status(response: &TsysTransitReturnResponse) -> RefundStatus {
    match (response.status.as_ref(), response.response_code.as_deref()) {
        (Some(TsysTransitStatus::Pass), Some("A0000" | "A0002" | "A0014")) => RefundStatus::Success,
        (Some(TsysTransitStatus::Fail), _) => RefundStatus::Failure,
        _ => RefundStatus::Failure,
    }
}

impl TryFrom<ResponseRouterData<TsysTransitReturnResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitReturnResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("Refund", item.http_code, response);

        let refund_status = map_refund_status(response);

        if matches!(refund_status, RefundStatus::Failure) {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }
        let connector_refund_id = response.transaction_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsysTransit: Return response missing <transactionID>; confirm API contract.",
            )
        })?;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for TsysTransitTransactionInquiryRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;
        let transaction_id = if !router_data.request.connector_refund_id.is_empty() {
            router_data.request.connector_refund_id.clone()
        } else if !router_data.request.connector_transaction_id.is_empty() {
            router_data.request.connector_transaction_id.clone()
        } else {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "connector_refund_id or connector_transaction_id",
                context: Default::default(),
            }
            .into());
        };

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
        })
    }
}

fn get_refund_status(item: &TsysTransitTransactionDetails) -> Option<RefundStatus> {
    let transaction_type = item.transaction_type.to_lowercase();
    if transaction_type.contains("return")
        || (transaction_type.contains("sale") && transaction_type.contains("void"))
    {
        match item.transaction_status {
            Some(TsysTransitTransactionStatus::Approved) => Some(RefundStatus::Success),
            Some(TsysTransitTransactionStatus::Decline)
            | Some(TsysTransitTransactionStatus::Cancel)
            | Some(TsysTransitTransactionStatus::Void) => Some(RefundStatus::Failure),
            None => None,
        }
    } else {
        None
    }
}

impl TryFrom<ResponseRouterData<TsysTransitTransactionInquiryResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitTransactionInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        log_tsys_transit_response("RSync", item.http_code, response);

        match response
            .transaction_details
            .as_ref()
            .and_then(get_refund_status)
        {
            Some(refund_status) => {
                let transaction_details = response.transaction_details.as_ref().ok_or(
                    ConnectorError::ResponseHandlingFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: Some(item.http_code),
                            additional_context: Some(
                                "tsysTransit: RSync response missing transactionDetails"
                                    .to_string(),
                            ),
                        },
                    },
                )?;
                // In rsync response error reason is not returned
                Ok(Self {
                    resource_common_data: RefundFlowData {
                        status: refund_status,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Ok(RefundsResponseData {
                        connector_refund_id: transaction_details.transaction_i_d.clone(),
                        refund_status,
                        status_code: item.http_code,
                        acquirer_reference_number: None,
                    }),
                    ..router_data.clone()
                })
            }
            None => {
                // In case of rsync failure, returning an error response would fail the refund, hence we are constructing the old state of the refund and returning it
                let refund_status = RefundStatus::Unknown;
                let refund_response = RefundsResponseData {
                    connector_refund_id: router_data.request.connector_refund_id.clone(),
                    refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                };

                Ok(Self {
                    resource_common_data: RefundFlowData {
                        status: refund_status,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Ok(refund_response),
                    ..router_data.clone()
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<
                VoidPostRefund,
                RefundFlowData,
                RefundVoidPostRefundData,
                RefundsResponseData,
            >,
            T,
        >,
    > for TsysTransitVoidPostRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<
                VoidPostRefund,
                RefundFlowData,
                RefundVoidPostRefundData,
                RefundsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;
        let transaction_amount = router_data
            .request
            .refund_money
            .as_ref()
            .map(|amount| {
                super::TsysTransitAmountConvertor::convert(amount.amount, amount.currency)
            })
            .transpose()?;
        // TSYS <voidReason> only accepts a fixed set of enum values, so an
        // arbitrary caller-supplied cancellation_reason (free text) is rejected
        // with "The value of element 'voidReason' is not valid." Ignore the
        // request value and always send a valid connector default.
        let void_reason = router_data
            .request
            .cancellation_reason
            .clone()
            .unwrap_or_else(|| DEFAULT_CANCELLATION_REASON.to_string());

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id: router_data.request.connector_refund_id.clone(),
            transaction_amount,
            void_reason,
        })
    }
}

fn map_void_post_refund_status(response: &TsysTransitVoidPostRefundResponse) -> RefundStatus {
    match (response.status.as_ref(), response.response_code.as_deref()) {
        (Some(TsysTransitStatus::Pass), Some("A0000" | "A0002")) => RefundStatus::Success,
        (Some(TsysTransitStatus::Fail), _) => RefundStatus::Failure,
        _ => RefundStatus::Failure,
    }
}

impl TryFrom<ResponseRouterData<TsysTransitVoidPostRefundResponse, Self>>
    for RouterDataV2<VoidPostRefund, RefundFlowData, RefundVoidPostRefundData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitVoidPostRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("VoidPostRefund", item.http_code, response);

        let void_post_refund_status = map_void_post_refund_status(response);

        if matches!(void_post_refund_status, RefundStatus::Failure) {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: RefundStatus::Success,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let connector_refund_id = response
            .transaction_id
            .clone()
            .unwrap_or_else(|| router_data.request.connector_refund_id.clone());

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: RefundStatus::Success,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status: void_post_refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..router_data.clone()
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TsysTransitVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;

        let transaction_id = router_data.request.connector_transaction_id.clone();
        let transaction_amount = match (router_data.request.amount, router_data.request.currency) {
            (Some(amount), Some(currency)) => Some(super::TsysTransitAmountConvertor::convert(
                amount, currency,
            )?),
            _ => None,
        };
        // TSYS <voidReason> only accepts a fixed set of enum values, so an
        // arbitrary caller-supplied cancellation_reason (free text) is rejected
        // with "The value of element 'voidReason' is not valid." Ignore the
        // request value and always send a valid connector default.
        let void_reason = "POST_AUTH_USER_DECLINE".to_string();

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id,
            transaction_amount,
            void_reason,
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysTransitVoidPCRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            developer_id: auth.developer_id,
            transaction_id: router_data.request.connector_transaction_id.clone(),
            transaction_amount: None,
            void_reason: "PARTIAL_REVERSAL".to_string(),
        })
    }
}
fn map_void_status(response: &TsysTransitVoidResponse) -> AttemptStatus {
    match (response.status.as_ref(), response.response_code.as_deref()) {
        (Some(TsysTransitStatus::Pass), Some("A0000")) => AttemptStatus::Voided,
        (Some(TsysTransitStatus::Pass), Some("A0002")) => AttemptStatus::Voided,
        (Some(TsysTransitStatus::Fail), _) => AttemptStatus::VoidFailed,
        _ => AttemptStatus::VoidFailed,
    }
}

impl TryFrom<ResponseRouterData<TsysTransitVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("Void", item.http_code, response);

        let status = map_void_status(response);

        if matches!(status, AttemptStatus::VoidFailed) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }
        let connector_txn_id = match response.transaction_id.clone() {
            Some(id) => id,
            None => {
                let id = router_data.request.connector_transaction_id.clone();
                if id.is_empty() {
                    return Err(crate::utils::response_deserialization_fail(
                        item.http_code,
                        "tsysTransit: Void response missing <transactionID> and request had none.",
                    )
                    .into());
                }
                id
            }
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

impl TryFrom<ResponseRouterData<TsysTransitVoidResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("VoidPC", item.http_code, response);

        let post_capture_void_status = match map_void_status(response) {
            AttemptStatus::Voided => common_enums::PostCaptureVoidStatus::Succeeded,
            AttemptStatus::Pending => common_enums::PostCaptureVoidStatus::Pending,
            _ => common_enums::PostCaptureVoidStatus::Failed,
        };
        let connector_reference_id = response
            .transaction_id
            .clone()
            .or_else(|| Some(router_data.request.connector_transaction_id.clone()));
        let description = post_capture_void_status
            .is_post_capture_void_failure()
            .then(|| response.response_message.clone())
            .flatten();

        Ok(Self {
            response: Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id,
                description,
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysTransitCardAuthenticationRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;
        validate_off_session_ecommerce_support(
            router_data.request.setup_future_usage,
            router_data.request.payment_channel.as_ref(),
        )?;
        let is_ecommerce_payment = matches!(
            router_data.request.payment_channel,
            Some(PaymentChannel::Ecommerce)
        );

        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "tsysTransit",
                    context: Default::default(),
                }
                .into());
            }
        };

        let billing = router_data
            .resource_common_data
            .address
            .get_payment_billing()
            .and_then(|b| b.address.as_ref());
        let address_line1 = billing.and_then(|a| a.line1.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.line1",
                context: Default::default(),
            })
        })?;
        let zip = billing.and_then(|a| a.zip.clone()).ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.zip",
                context: Default::default(),
            })
        })?;
        // TSYS cert: firstName / lastName are Visa-only on card
        // authentication ("firstName and lastName tags must not be sent
        // on the 0.00 Mastercard / AMEX card authentication in step 3 as
        // these are Visa card authentication only tags").
        let (cardholder_first_name, cardholder_last_name) =
            split_domain_full_name(card.card_holder_name.clone());
        let is_visa_card_auth = matches!(card.card_network, Some(CardNetwork::Visa));
        let first_name = if is_visa_card_auth && !is_ecommerce_payment {
            billing
                .and_then(|a| a.first_name.clone())
                .or(cardholder_first_name)
                .map(|name| Secret::new(sanitize_alphanumeric_space(name.peek(), 25)))
        } else {
            None
        };
        let derived_last_name = billing
            .and_then(|a| a.last_name.clone())
            .or(cardholder_last_name)
            .map(|name| Secret::new(sanitize_alphanumeric_space(name.peek(), 25)));

        let last_name = if is_visa_card_auth && !is_ecommerce_payment {
            Some(derived_last_name.ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "billing.address.last_name required for Visa CardAuthentication Account Name Inquiry",
                    context: Default::default(),
                })
            })?)
        } else {
            None
        };

        // ── Profile + terminalData via rules ─────────────────────────
        let profile = TxProfile::derive_for_card_authentication(router_data);
        let terminal_data = rules::terminal_data::terminal_data(&profile);
        let cvv_present = !card.card_cvc.peek().is_empty();

        // ── terminalData fields (profile/rules only, no merchant override) ─
        let rules::terminal_data::ResolvedTerminalData {
            card_data_source,
            terminal_capability,
            terminal_operating_environment,
            cardholder_authentication_method,
            terminal_authentication_capability,
            terminal_output_capability,
            max_pin_length,
            terminal_card_capture_capability,
            cardholder_present_detail,
            card_present_detail,
            card_data_input_mode,
            cardholder_authentication_entity,
            card_data_output_capability,
        } = rules::terminal_data::resolve(&profile, &terminal_data, cvv_present);

        // ── Per-tx fields via rules ──────────────────────────────────
        // TSYS cert: cvv2 must be sent on card authentication when the
        // merchant collected one.
        let cvv2 = cvv_present.then(|| card.card_cvc.clone());
        // TSYS cert: authorizationIndicator must be sent on Mastercard
        // card authentications in step 3 (Final since card auth is a
        // self-contained 0.00 probe).
        let authorization_indicator =
            rules::network_indicators::authorization_indicator_for_card_auth(&profile);
        // TSYS cert (MOTO step 5): cardOnFile=Y on Visa CIT-setup card
        // authentications used to store credentials for future payments.
        let card_on_file = rules::cof_mit::card_on_file(&profile);
        let cit_status_indicator = rules::cof_mit::cit_status_indicator(&profile);

        let merchant_acceptor_info = build_merchant_acceptor_info(
            &auth,
            card.card_network.as_ref(),
            router_data.request.payment_channel.as_ref(),
        )?;

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            card_data_source,
            card_number: Secret::new(card.card_number.peek().to_string()),
            expiration_date: format_expiration_date(card),
            cvv2,
            address_line1: address_line1.clone(),
            zip,
            // TSYS cert: externalReferenceID is alphanumeric only; strip
            // underscores and any other non-alphanumeric/space chars.
            external_reference_id: sanitize_alphanumeric_space(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
                40,
            ),
            first_name,
            middle_name: None,
            last_name,
            developer_id: auth.developer_id,
            terminal_capability,
            terminal_operating_environment,
            cardholder_authentication_method,
            terminal_authentication_capability,
            terminal_output_capability,
            max_pin_length,
            terminal_card_capture_capability,
            cardholder_present_detail,
            card_present_detail,
            card_data_input_mode,
            cardholder_authentication_entity,
            card_data_output_capability,
            authorization_indicator,
            card_on_file,
            cit_status_indicator,
            acceptor_street_address: merchant_acceptor_info
                .as_ref()
                .map(|info| info.street_address.clone()),
            acceptor_customer_service_phone_number: merchant_acceptor_info
                .as_ref()
                .map(|info| info.customer_service_phone_number.clone()),
            acceptor_phone_number: merchant_acceptor_info
                .as_ref()
                .map(|info| info.phone_number.clone()),
            acceptor_u_r_l_address: merchant_acceptor_info.as_ref().map(|info| info.url.clone()),
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TsysTransitCardAuthenticationResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitCardAuthenticationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("SetupMandate", item.http_code, response);

        let is_success = matches!(response.status, Some(TsysTransitStatus::Pass))
            && response.response_code.as_deref() == Some("A0000");

        if !is_success {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: response
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.response_message.clone(),
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }
        let ntid_source = response
            .card_transaction_identifier
            .clone()
            .or_else(|| response.transaction_id.clone())
            .ok_or_else(|| {
                crate::utils::response_deserialization_fail(
                    item.http_code,
                    "tsysTransit: CardAuthenticationResponse missing both <cardTransactionIdentifier> and <transactionID>; confirm API contract.",
                )
            })?;

        let connector_txn_id = response
            .transaction_id
            .clone()
            .unwrap_or_else(|| ntid_source.clone());

        // Do NOT fake the network transaction id as a connector_mandate_id (PSP
        // token). The NTI is stored as network_txn_id; a connector-agnostic MIT
        // replays it from the payment method's network_transaction_id together with
        // the locker card, so no connector mandate / PSP token is needed.
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: Some(ntid_source.clone()),
            network_txn_link_id: None,
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
fn repeat_payment_data_to_authorize<T: PaymentMethodDataTypes>(
    req: &RepeatPaymentData<T>,
) -> PaymentsAuthorizeData<T> {
    let mandate_ids = MandateIds {
        mandate_id: None,
        mandate_reference_id: Some(req.mandate_reference.clone()),
    };

    // The RecurringPaymentServiceChargeRequest proto doesn't carry
    // payment_channel — recover it from the TSYS merchant metadata's optional
    // `payment_channel` override. When it is absent the transaction defaults to
    // MOTO/PHONE (tsys_transit is a MOTO connector; legacy stored cards have no
    // channel on file), so MOTO merchant-initiated transactions produce
    // CARDHOLDER_NOT_PRESENT_PHONE_TRANSACTION instead of the e-com default.
    let payment_channel_from_metadata = req
        .metadata
        .as_ref()
        .and_then(|m| {
            serde_json::from_value::<TsysTransitPaymentRequestMetadata>(m.clone().expose()).ok()
        })
        .and_then(|m| m.payment_channel)
        .and_then(|s| match s.to_ascii_lowercase().as_str() {
            "telephone_order" | "phone" => Some(PaymentChannel::TelephoneOrder),
            "mail_order" | "mail" => Some(PaymentChannel::MailOrder),
            "ecommerce" | "internet" => Some(PaymentChannel::Ecommerce),
            _ => None,
        });

    PaymentsAuthorizeData {
        payment_method_data: req.payment_method_data.clone(),
        amount: req.minor_amount,
        order_tax_amount: None,
        surcharge_amount: None,
        email: req.email.clone(),
        customer_document_details: None,
        customer_name: None,
        currency: req.currency,
        confirm: true,
        billing_descriptor: req.billing_descriptor.clone(),
        capture_method: req.capture_method,
        router_return_url: req.router_return_url.clone(),
        webhook_url: req.webhook_url.clone(),
        complete_authorize_url: None,
        mandate_id: Some(mandate_ids),
        setup_future_usage: None,
        off_session: Some(true),
        browser_info: req.browser_info.clone(),
        order_category: None,
        session_token: None,
        access_token: None,
        customer_acceptance: None,
        enrolled_for_3ds: None,
        related_transaction_id: None,
        payment_experience: None,
        payment_method_type: req.payment_method_type,
        customer_id: None,
        request_incremental_authorization: None,
        metadata: req.metadata.clone(),
        authentication_data: req.authentication_data.clone(),
        split_payments: req.split_payments.clone(),
        split_settlement: req.split_settlement.clone(),
        minor_amount: req.minor_amount,
        merchant_order_id: req.merchant_order_id.clone(),
        shipping_cost: req.shipping_cost,
        merchant_account_id: req.merchant_account_id.as_ref().map(|s| s.peek().clone()),
        integrity_object: None,
        merchant_config_currency: req.merchant_configured_currency,
        all_keys_required: None,
        request_extended_authorization: None,
        enable_overcapture: None,
        setup_mandate_details: None,
        connector_feature_data: req.connector_feature_data.clone(),
        connector_testing_data: req.connector_testing_data.clone(),
        // Precedence: the request's own `payment_channel` (now carried on the
        // charge), then the metadata override, then the MOTO/PHONE default
        // (tsys_transit is a MOTO connector; stored cards carry no channel).
        payment_channel: req
            .payment_channel
            .clone()
            .or(payment_channel_from_metadata),
        enable_partial_authorization: req.enable_partial_authorization,
        locale: req.locale.clone(),
        redirect_response: None,
        threeds_method_comp_ind: None,
        continue_redirection_url: None,
        tokenization: None,
        mit_category: req.mit_category.clone(),
        domain_data: None,
        partner_merchant_identifier_details: None,
        currency_conversion_data: None,
        is_account_funded_transaction: None,
        recipient_details: None,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TsysTransitRepeatPaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let TsysTransitRouterData {
            connector,
            router_data,
        } = item;
        let synthetic_request = repeat_payment_data_to_authorize(&router_data.request);

        let synthetic_router_data: RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        > = RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: router_data.resource_common_data.clone(),
            connector_config: router_data.connector_config.clone(),
            request: synthetic_request,
            response: Err(ErrorResponse::default()),
        };

        let synthetic_wrapper = TsysTransitRouterData {
            connector,
            router_data: synthetic_router_data,
        };

        let inner = TsysTransitAuthorizeRequest::try_from(synthetic_wrapper)?;
        Ok(Self(inner))
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<TsysTransitRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;
        log_tsys_transit_response("RepeatPayment", item.http_code, response);
        let body = response.body();
        let authorize_view = response.as_authorize();
        let status = map_authorize_status(&authorize_view);

        if matches!(status, AttemptStatus::Failure) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: body
                        .response_code
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: body
                        .response_message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: body.response_message.clone(),
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: body.transaction_id.clone(),
                    network_decline_code: body.host_response_code.clone(),
                    network_advice_code: None,
                    network_error_message: body.response_message.clone(),
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let transaction_id = body.transaction_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsysTransit: success response missing <transactionID>; confirm API contract.",
            )
        })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            // Network transaction identifier, not the authCode (see Authorize).
            network_txn_id: body.card_transaction_identifier.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
