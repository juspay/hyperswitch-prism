use std::fmt::Debug;

use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, FutureUsage, MitCategory, PaymentChannel,
    RefundStatus,
};
use common_utils::types::{MinorUnit, StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void, VoidPC,
        VoidPostRefund,
    },
    connector_types::{
        MandateIds, MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RecurringMandatePaymentData, RefundFlowData,
        RefundSyncData, RefundVoidPostRefundData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
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

use super::{super::macros::GetSoapXml, TsysTransitRouterData};
use crate::types::ResponseRouterData;

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
    pub tax_category: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_discount_percentage: Option<String>,
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
fn generate_xml<T: Serialize>(request: &T) -> Result<String, Report<IntegrationError>> {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_descriptor_2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_descriptor_3: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_descriptor_4: Option<String>,
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
}
#[derive(Debug, Serialize)]
#[serde(rename = "TransactionInquiry")]
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
        generate_logged_xml(self, "TransactionInquiry")
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
    #[serde(rename = "addressLine1")]
    pub address_line1: Secret<String>,
    #[serde(rename = "zip")]
    pub zip: Secret<String>,
    #[serde(rename = "externalReferenceID")]
    pub external_reference_id: String,
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
    #[serde(rename = "mPosAcceptanceDeviceType")]
    pub m_pos_acceptance_device_type: String,
    #[serde(rename = "cardOnFile", skip_serializing_if = "Option::is_none")]
    pub card_on_file: Option<TsysTransitCardOnFile>,
    #[serde(rename = "citStatusIndicator", skip_serializing_if = "Option::is_none")]
    pub cit_status_indicator: Option<TsysTransitMcCitStatusIndicator>,
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
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransitTransactionState {
    Authorized,
    Captured,
    Settled,
    Voided,
    Returned,
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
#[serde(rename = "TransactionInquiryResponse")]
pub struct TsysTransitTransactionInquiryResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "transactionID", default)]
    pub transaction_id: Option<String>,
    #[serde(rename = "transactionState", default)]
    pub transaction_state: Option<TsysTransitTransactionState>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
}
#[derive(Debug, Default, Deserialize, Clone)]
struct TsysTransitMerchantMetadata {
    #[serde(default)]
    tsys_transit: Option<TsysTransitMerchantMetadataInner>,
    #[serde(default)]
    terminal_data: Option<TsysTransitTerminalDataOverrides>,
    #[serde(default)]
    commercial_card: Option<TsysTransitCommercialCardMetadata>,
    #[serde(default, flatten)]
    terminal_overrides: TsysTransitTerminalDataOverrides,
}

impl TsysTransitMerchantMetadata {
    fn into_inner(self) -> TsysTransitMerchantMetadataInner {
        let mut inner = self.tsys_transit.unwrap_or_default();

        if self.commercial_card.is_some() {
            inner.commercial_card = self.commercial_card;
        }

        let mut terminal_data = inner.terminal_data.take().unwrap_or_default();
        if let Some(overrides) = self.terminal_data {
            terminal_data.merge(overrides);
        }
        terminal_data.merge(self.terminal_overrides);
        if terminal_data.has_any() {
            inner.terminal_data = Some(terminal_data);
        }

        inner
    }
}

#[derive(Debug, Default, Deserialize, Clone)]
struct TsysTransitMerchantMetadataInner {
    #[serde(default)]
    terminal_data: Option<TsysTransitTerminalDataOverrides>,
    #[serde(default)]
    commercial_card: Option<TsysTransitCommercialCardMetadata>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct TsysTransitCommercialCardMetadata {
    charge_descriptor_2: Option<String>,
    charge_descriptor_3: Option<String>,
    charge_descriptor_4: Option<String>,
    vat_invoice: Option<String>,
    ship_from_zip: Option<String>,
}
#[derive(Debug, Default, Deserialize, Clone)]
struct TsysTransitMandateMetadata {
    #[serde(default)]
    payment_count: Option<u32>,
    #[serde(default)]
    current_payment_count: Option<u32>,
    #[serde(default)]
    mc_subtype: Option<String>,
    #[serde(default)]
    installment_variant: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
struct TsysTransitTerminalDataOverrides {
    terminal_capability: Option<TsysTransitTerminalCapability>,
    terminal_operating_environment: Option<TsysTransitTerminalOperatingEnvironment>,
    cardholder_authentication_method: Option<TsysTransitCardholderAuthenticationMethod>,
    terminal_authentication_capability: Option<TsysTransitTerminalAuthenticationCapability>,
    terminal_output_capability: Option<TsysTransitTerminalOutputCapability>,
    max_pin_length: Option<TsysTransitMaxPinLength>,
    terminal_card_capture_capability: Option<TsysTransitTerminalCardCaptureCapability>,
    cardholder_present_detail: Option<TsysTransitCardholderPresentDetail>,
    card_present_detail: Option<TsysTransitCardPresentDetail>,
    card_data_input_mode: Option<TsysTransitCardDataInputMode>,
    cardholder_authentication_entity: Option<TsysTransitCardholderAuthenticationEntity>,
    card_data_output_capability: Option<TsysTransitCardDataOutputCapability>,
}

impl TsysTransitTerminalDataOverrides {
    fn has_any(&self) -> bool {
        self.terminal_capability.is_some()
            || self.terminal_operating_environment.is_some()
            || self.cardholder_authentication_method.is_some()
            || self.terminal_authentication_capability.is_some()
            || self.terminal_output_capability.is_some()
            || self.max_pin_length.is_some()
            || self.terminal_card_capture_capability.is_some()
            || self.cardholder_present_detail.is_some()
            || self.card_present_detail.is_some()
            || self.card_data_input_mode.is_some()
            || self.cardholder_authentication_entity.is_some()
            || self.card_data_output_capability.is_some()
    }

    fn merge(&mut self, other: Self) {
        if other.terminal_capability.is_some() {
            self.terminal_capability = other.terminal_capability;
        }
        if other.terminal_operating_environment.is_some() {
            self.terminal_operating_environment = other.terminal_operating_environment;
        }
        if other.cardholder_authentication_method.is_some() {
            self.cardholder_authentication_method = other.cardholder_authentication_method;
        }
        if other.terminal_authentication_capability.is_some() {
            self.terminal_authentication_capability = other.terminal_authentication_capability;
        }
        if other.terminal_output_capability.is_some() {
            self.terminal_output_capability = other.terminal_output_capability;
        }
        if other.max_pin_length.is_some() {
            self.max_pin_length = other.max_pin_length;
        }
        if other.terminal_card_capture_capability.is_some() {
            self.terminal_card_capture_capability = other.terminal_card_capture_capability;
        }
        if other.cardholder_present_detail.is_some() {
            self.cardholder_present_detail = other.cardholder_present_detail;
        }
        if other.card_present_detail.is_some() {
            self.card_present_detail = other.card_present_detail;
        }
        if other.card_data_input_mode.is_some() {
            self.card_data_input_mode = other.card_data_input_mode;
        }
        if other.cardholder_authentication_entity.is_some() {
            self.cardholder_authentication_entity = other.cardholder_authentication_entity;
        }
        if other.card_data_output_capability.is_some() {
            self.card_data_output_capability = other.card_data_output_capability;
        }
    }
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
    additional_tax_details: Vec<TsysTransitAdditionalTaxDetails>,
    shipping_charges: Option<StringMajorUnit>,
    duty_charges: Option<StringMajorUnit>,
    product_details: Vec<TsysTransitProductDetails>,
    commercial_card_level: Option<TsysTransitCommercialCardLevel>,
    purchase_order: Option<String>,
    charge_descriptor: Option<String>,
    charge_descriptor_2: Option<String>,
    charge_descriptor_3: Option<String>,
    charge_descriptor_4: Option<String>,
    customer_vat_number: Option<String>,
    customer_ref_id: Option<String>,
    supplier_reference_number: Option<String>,
    order_date: Option<String>,
    summary_commodity_code: Option<String>,
    vat_invoice: Option<String>,
    ship_from_zip: Option<String>,
    ship_to_zip: Option<String>,
    destination_country_code: Option<String>,
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
    let mm = match recurring_data.and_then(|d| d.mandate_metadata.as_ref()) {
        Some(raw) => serde_json::from_value::<TsysTransitMandateMetadata>(raw.peek().clone())
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "recurring_mandate_payment_data.mandate_metadata",
                context: Default::default(),
            })?,
        None => TsysTransitMandateMetadata::default(),
    };
    if matches!(mit_category.as_ref(), Some(MitCategory::Installment))
        && (mm.payment_count.is_none() || mm.current_payment_count.is_none())
    {
        return Err(error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "recurring_mandate_payment_data.mandate_metadata.{payment_count,current_payment_count}",
            context: Default::default(),
        })
        .attach_printable(
            "tsys_transit: installment MIT requires both `paymentCount` (total number of \
             scheduled installments, e.g. 12) and `currentPaymentCount` (1-indexed position \
             of this charge in the schedule, e.g. 3) on the TransIT Sale/Auth request — \
             without them TSYS rejects the transaction at the gateway. Populate \
             `recurring_mandate_payment_data.mandate_metadata.payment_count` and \
             `.current_payment_count` upstream, or pick a non-Installment `mit_category`. \
             See: https://developer.tsys.com/tsys-transit/api/installment-payments",
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
        ) => Some(
            if mm
                .installment_variant
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("t"))
            {
                TsysTransitMitIndicator::T
            } else {
                TsysTransitMitIndicator::S
            },
        ),
        _ => None,
    };
    let (mc_cit_status_indicator, mc_mit_status_indicator) =
        match (mit_category.as_ref(), card_network) {
            (Some(MitCategory::Recurring), Some(CardNetwork::Mastercard)) => {
                let is_subscription = mm
                    .mc_subtype
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case("subscription"));
                if is_subscription {
                    (
                        Some(TsysTransitMcCitStatusIndicator::C103),
                        Some(TsysTransitMitIndicator::M103),
                    )
                } else {
                    (
                        Some(TsysTransitMcCitStatusIndicator::C102),
                        Some(TsysTransitMitIndicator::M102),
                    )
                }
            }
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
        payment_count: mm.payment_count,
        current_payment_count: mm.current_payment_count,
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
}

impl TryFrom<&ConnectorSpecificConfig> for TsysTransitAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::TsysTransit {
                device_id,
                transaction_key,
                developer_id,
                ..
            } => Ok(Self {
                device_id: device_id.to_owned(),
                transaction_key: transaction_key.to_owned(),
                developer_id: developer_id.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
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

fn format_tsys_order_date(value: time::PrimitiveDateTime) -> String {
    let date = value.date();
    format!(
        "{:02}/{:02}/{:04}",
        u8::from(date.month()),
        date.day(),
        date.year()
    )
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
    zero_amount: &StringMajorUnit,
    derived_tax_rate: Option<&String>,
    require_commodity_code: bool,
) -> Result<TsysTransitProductDetails, Report<IntegrationError>> {
    let price = super::TsysTransitAmountConvertor::convert(detail.amount, currency)?;
    let unit_discount_amount = detail
        .unit_discount_amount
        .map(|amount| super::TsysTransitAmountConvertor::convert(amount, currency))
        .transpose()?
        .unwrap_or_else(|| zero_amount.clone());
    let has_discount = detail
        .unit_discount_amount
        .map(|amount| amount.get_amount_as_i64() > 0)
        .unwrap_or(false);
    let product_tax_amount = detail
        .total_tax_amount
        .map(|amount| super::TsysTransitAmountConvertor::convert(amount, currency))
        .transpose()?
        .unwrap_or_else(|| zero_amount.clone());
    let product_commodity_code = detail
        .commodity_code
        .clone()
        .or_else(|| detail.upc.clone())
        .or_else(|| detail.product_id.clone())
        .or_else(|| detail.sku.clone())
        .map(|code| sanitize_alphanumeric_space(&code, 12));

    if require_commodity_code && product_commodity_code.is_none() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "productCommodityCode required for Visa/Mastercard Level III",
            context: Default::default(),
        }
        .into());
    }

    Ok(TsysTransitProductDetails {
        product_code: detail
            .product_id
            .clone()
            .or_else(|| detail.sku.clone())
            .or_else(|| detail.upc.clone())
            .map(|code| sanitize_alphanumeric_space(&code, 20))
            .filter(|code| !code.is_empty())
            .unwrap_or_else(|| sanitize_alphanumeric_space(&detail.product_name, 20)),
        product_name: truncate_chars(&detail.product_name, 50),
        price,
        quantity: u32::from(detail.quantity),
        measurement_unit: detail
            .unit_of_measure
            .clone()
            .or_else(|| Some("EA".to_string())),
        product_discount_details: Some(TsysTransitProductDiscountDetails {
            product_discount_name: "Line Item Discount".to_string(),
            product_discount_amount: unit_discount_amount,
            product_discount_percentage: Some("0.01".to_string()),
            product_discount_type: "DISCOUNT".to_string(),
            priority: 1,
            stackable: if has_discount {
                TsysTransitYesNo::Yes
            } else {
                TsysTransitYesNo::No
            },
        }),
        product_tax_details: Some(TsysTransitProductTaxDetails {
            product_tax_name: detail
                .product_tax_code
                .clone()
                .or_else(|| Some("TAX".to_string())),
            product_tax_amount: Some(product_tax_amount),
            product_tax_percentage: Some(
                detail
                    .tax_rate
                    .map(format_decimal)
                    .or_else(|| derived_tax_rate.cloned())
                    .unwrap_or_else(|| "0".to_string()),
            ),
            product_tax_type: detail
                .product_tax_code
                .clone()
                .map(|tax_code| truncate_chars(&tax_code, 4)),
        }),
        product_variation: detail
            .sub_category
            .clone()
            .or_else(|| detail.category.clone()),
        product_modifier_details: detail
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
            }),
        product_notes: detail
            .description
            .clone()
            .map(|description| truncate_chars(&description, 100)),
        product_discount_indicator: Some(if has_discount {
            TsysTransitProductDiscountIndicator::Y
        } else {
            TsysTransitProductDiscountIndicator::N
        }),
        product_commodity_code,
    })
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
    commercial_meta: Option<&TsysTransitCommercialCardMetadata>,
    card_network: Option<&CardNetwork>,
) -> Result<CommercialCardContext, Report<IntegrationError>> {
    let empty_commercial_meta = TsysTransitCommercialCardMetadata::default();
    let commercial_meta = commercial_meta.unwrap_or(&empty_commercial_meta);

    let l2_l3_data = router_data.resource_common_data.l2_l3_data.as_deref();
    let shipping_address = router_data.resource_common_data.get_shipping_address().ok();
    let billing_address = router_data.resource_common_data.get_billing_address().ok();
    let billing_descriptor = router_data.request.billing_descriptor.as_ref();
    let connector_request_reference_id = router_data
        .resource_common_data
        .connector_request_reference_id
        .clone();
    let order_details = l2_l3_data
        .and_then(|data| data.get_order_details())
        .or_else(|| router_data.resource_common_data.order_details.clone())
        .unwrap_or_default();
    let order_tax_amount = l2_l3_data
        .and_then(|data| data.get_order_tax_amount())
        .or(router_data.request.order_tax_amount);
    let order_reference = l2_l3_data
        .and_then(|data| data.get_merchant_order_reference_id())
        .or_else(|| router_data.request.merchant_order_id.clone());

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

    if order_details.is_empty()
        && order_tax_amount.is_none()
        && shipping_charges.is_none()
        && duty_charges.is_none()
    {
        return Ok(CommercialCardContext::default());
    }

    let commercial_card_level = if order_details.is_empty() {
        TsysTransitCommercialCardLevel::Level2
    } else {
        TsysTransitCommercialCardLevel::Level3
    };
    let is_level3 = matches!(
        commercial_card_level,
        TsysTransitCommercialCardLevel::Level3
    );
    let is_visa_or_mastercard = matches!(
        card_network,
        Some(CardNetwork::Visa) | Some(CardNetwork::Mastercard)
    );
    let is_mastercard = matches!(card_network, Some(CardNetwork::Mastercard));
    let is_amex = matches!(card_network, Some(CardNetwork::AmericanExpress));
    let zero_amount = super::TsysTransitAmountConvertor::convert(
        MinorUnit::new(0),
        router_data.request.currency,
    )?;

    let sales_tax = order_tax_amount
        .map(|amount| {
            super::TsysTransitAmountConvertor::convert(amount, router_data.request.currency)
        })
        .transpose()?;

    let derived_tax_rate = order_details
        .iter()
        .find_map(|detail| detail.tax_rate.map(format_decimal))
        .or_else(|| is_level3.then_some("0".to_string()));
    let derived_tax_type = order_details
        .iter()
        .find_map(|detail| detail.product_tax_code.clone())
        .filter(|value| !value.is_empty());

    let additional_tax_details = if is_level3 && is_visa_or_mastercard {
        let tax_amount =
            sales_tax
                .clone()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "salesTax required for commercial_card_level LEVEL3",
                    context: Default::default(),
                })?;
        let tax_type = derived_tax_type.clone().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name:
                    "taxType required for additionalTaxDetails (order_details[0].product_tax_code missing)",
                context: Default::default(),
            }
        })?;

        vec![TsysTransitAdditionalTaxDetails {
            tax_type: tax_type.clone(),
            tax_amount,
            tax_rate: Some(derived_tax_rate.clone().unwrap_or_else(|| "0".to_string())),
            tax_category: Some(tax_type),
        }]
    } else {
        Vec::new()
    };

    let product_details = if is_level3 {
        if order_details.is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "order_details required for commercial_card_level LEVEL3",
                context: Default::default(),
            }
            .into());
        }

        order_details
            .iter()
            .map(|detail| {
                build_tsys_product_details(
                    detail,
                    router_data.request.currency,
                    &zero_amount,
                    derived_tax_rate.as_ref(),
                    is_visa_or_mastercard,
                )
            })
            .collect::<Result<Vec<_>, Report<IntegrationError>>>()?
    } else {
        Vec::new()
    };

    let purchase_order = sanitize_optional_alphanumeric_space(
        order_reference
            .clone()
            .or_else(|| Some(connector_request_reference_id.clone())),
        25,
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
    let supplier_reference_number = (!is_level3 || is_amex)
        .then(|| {
            sanitize_optional_alphanumeric_space(
                order_reference
                    .clone()
                    .or_else(|| Some(connector_request_reference_id.clone())),
                9,
            )
        })
        .flatten();
    let customer_vat_number = l2_l3_data
        .and_then(|data| data.get_customer_tax_registration_id())
        .map(|tax_id| tax_id.expose())
        .and_then(|tax_id| sanitize_optional_alphanumeric_space(Some(tax_id), 13));
    let customer_ref_id = (!is_level3 || is_amex)
        .then(|| {
            sanitize_optional_alphanumeric_space(
                order_reference
                    .clone()
                    .or_else(|| Some(connector_request_reference_id.clone())),
                17,
            )
        })
        .flatten();
    let order_date = l2_l3_data
        .and_then(|data| data.get_order_date())
        .map(format_tsys_order_date);
    let summary_commodity_code = order_details
        .iter()
        .find_map(|detail| {
            detail
                .commodity_code
                .clone()
                .or_else(|| detail.upc.clone())
                .or_else(|| detail.product_id.clone())
                .or_else(|| detail.sku.clone())
        })
        .and_then(|code| sanitize_optional_alphanumeric_space(Some(code), 4));
    let vat_invoice = sanitize_optional_alphanumeric_space(commercial_meta.vat_invoice.clone(), 15);
    let ship_from_zip = l2_l3_data
        .and_then(|data| data.get_shipping_origin_zip())
        .map(|zip| zip.expose())
        .or_else(|| commercial_meta.ship_from_zip.clone());
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

    if is_level3 && is_visa_or_mastercard {
        if sales_tax.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name:
                    "salesTax required for TSYS commercial-card Level III (Visa/Mastercard)",
                context: Default::default(),
            }
            .into());
        }
        if purchase_order.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "purchaseOrder required for Visa/Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
        if shipping_charges.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "shippingCharges required for Visa/Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
        if duty_charges.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "dutyCharges required for Visa/Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
        if is_mastercard
            && destination_country_code
                .as_ref()
                .is_none_or(|code| code.len() != 3)
        {
            return Err(IntegrationError::MissingRequiredField {
                field_name:
                    "destinationCountryCode required and must be 3-digit for Mastercard Level III",
                context: Default::default(),
            }
            .into());
        }
    }

    if matches!(
        commercial_card_level,
        TsysTransitCommercialCardLevel::Level2
    ) {
        if is_visa_or_mastercard && purchase_order.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "purchaseOrder required for Visa/Mastercard Level II",
                context: Default::default(),
            }
            .into());
        }
        if sales_tax.is_none() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "salesTax required for TSYS commercial-card Level II",
                context: Default::default(),
            }
            .into());
        }
    }

    if matches!(
        commercial_card_level,
        TsysTransitCommercialCardLevel::Level2
    ) && is_amex
    {
        for (field_name, is_missing) in [
            (
                "supplierReferenceNumber",
                supplier_reference_number.is_none(),
            ),
            ("customerRefID", customer_ref_id.is_none()),
            ("shipToZip", ship_to_zip.is_none()),
            ("chargeDescriptor", charge_descriptor.is_none()),
        ] {
            if is_missing {
                return Err(IntegrationError::MissingRequiredField {
                    field_name,
                    context: Default::default(),
                }
                .into());
            }
        }
    }

    if is_level3 && is_visa_or_mastercard {
        for (field_name, is_missing) in [
            ("purchaseOrder", purchase_order.is_none()),
            ("orderDate", order_date.is_none()),
            ("summaryCommodityCode", summary_commodity_code.is_none()),
            ("vatInvoice", vat_invoice.is_none()),
            ("shipFromZip", ship_from_zip.is_none()),
            ("shipToZip", ship_to_zip.is_none()),
            ("destinationCountryCode", destination_country_code.is_none()),
        ] {
            if is_missing {
                return Err(IntegrationError::MissingRequiredField {
                    field_name,
                    context: Default::default(),
                }
                .into());
            }
        }
    }

    if is_level3 && matches!(card_network, Some(CardNetwork::Visa)) && customer_vat_number.is_none()
    {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "customerVATNumber required for Visa Level 3",
            context: Default::default(),
        }
        .into());
    }

    if is_level3 && is_visa_or_mastercard && additional_tax_details.is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "additionalTaxDetails required for Visa/Mastercard Level III",
            context: Default::default(),
        }
        .into());
    }

    Ok(CommercialCardContext {
        sales_tax,
        additional_tax_details,
        shipping_charges,
        duty_charges,
        product_details,
        commercial_card_level: Some(commercial_card_level),
        purchase_order: purchase_order.clone(),
        charge_descriptor,
        charge_descriptor_2: commercial_meta.charge_descriptor_2.clone(),
        charge_descriptor_3: commercial_meta.charge_descriptor_3.clone(),
        charge_descriptor_4: commercial_meta.charge_descriptor_4.clone(),
        customer_vat_number,
        customer_ref_id,
        supplier_reference_number,
        order_date,
        summary_commodity_code,
        vat_invoice,
        ship_from_zip,
        ship_to_zip,
        destination_country_code,
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
            .map(|amount| {
                super::TsysTransitAmountConvertor::convert(amount.amount, amount.currency)
            })
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
        let merchant_metadata_early = match router_data.request.metadata.as_ref() {
            Some(meta) => {
                serde_json::from_value::<TsysTransitMerchantMetadata>(meta.clone().expose())
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "connector_metadata.tsys_transit",
                        context: Default::default(),
                    })?
            }
            None => TsysTransitMerchantMetadata::default(),
        };
        let merchant_inner_early = merchant_metadata_early.into_inner();
        let commercial_meta = merchant_inner_early.commercial_card.clone();
        let terminal_overrides = merchant_inner_early.terminal_data.unwrap_or_default();
        let recurring_context = compute_recurring_context(
            router_data.request.mit_category.clone(),
            router_data
                .resource_common_data
                .recurring_mandate_payment_data
                .as_ref(),
            card_network.as_ref(),
        )?;
        let commercial_card_context = compute_commercial_card_context(
            router_data,
            commercial_meta.as_ref(),
            card_network.as_ref(),
        )?;
        let three_ds_context = compute_three_ds_context(router_data, card_network.as_ref());
        let channel = router_data.request.payment_channel.clone();
        let card_data_source = match channel {
            Some(PaymentChannel::TelephoneOrder) => TsysTransitCardDataSource::Phone,
            Some(PaymentChannel::MailOrder) => TsysTransitCardDataSource::Mail,
            Some(PaymentChannel::Ecommerce) | None => {
                if recurring_context.enabled {
                    TsysTransitCardDataSource::Mail
                } else {
                    TsysTransitCardDataSource::Internet
                }
            }
        };
        let is_manual_capture = matches!(
            router_data.request.capture_method,
            Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple)
        );

        let authorization_indicator = match card_network {
            Some(CardNetwork::Mastercard) => {
                if recurring_context.enabled && is_manual_capture {
                    None
                } else {
                    Some(if is_manual_capture {
                        TsysTransitAuthorizationIndicator::Preauth
                    } else {
                        TsysTransitAuthorizationIndicator::Final
                    })
                }
            }
            Some(CardNetwork::AmericanExpress) => Some(if is_manual_capture {
                TsysTransitAuthorizationIndicator::Preauth
            } else {
                TsysTransitAuthorizationIndicator::Final
            }),
            _ => None,
        };
        let (registered_user_indicator, last_registered_change_date) = if recurring_context.enabled
        {
            (None, None)
        } else {
            match card_network {
                Some(CardNetwork::Discover)
                | Some(CardNetwork::JCB)
                | Some(CardNetwork::DinersClub)
                | Some(CardNetwork::UnionPay) => (
                    Some(TsysTransitRegisteredUserIndicator::No),
                    Some("00/00/0000".to_string()),
                ),
                _ => (None, None),
            }
        };
        let terminal_capability = terminal_overrides
            .terminal_capability
            .unwrap_or(TsysTransitTerminalCapability::KeyedEntryOnly);
        let default_terminal_operating_environment = if recurring_context.enabled {
            match card_network {
                Some(CardNetwork::Mastercard) => {
                    TsysTransitTerminalOperatingEnvironment::NoTerminal
                }
                _ => TsysTransitTerminalOperatingEnvironment::OffMerchantPremisesUnattended,
            }
        } else {
            TsysTransitTerminalOperatingEnvironment::NoTerminal
        };
        let terminal_operating_environment = terminal_overrides
            .terminal_operating_environment
            .unwrap_or(default_terminal_operating_environment);
        let cardholder_authentication_method = terminal_overrides
            .cardholder_authentication_method
            .unwrap_or(TsysTransitCardholderAuthenticationMethod::NotAuthenticated);
        let terminal_authentication_capability = terminal_overrides
            .terminal_authentication_capability
            .unwrap_or(TsysTransitTerminalAuthenticationCapability::NoCapability);
        let default_terminal_output_capability = if recurring_context.enabled {
            TsysTransitTerminalOutputCapability::DisplayOnly
        } else {
            TsysTransitTerminalOutputCapability::None
        };
        let terminal_output_capability = terminal_overrides
            .terminal_output_capability
            .unwrap_or(default_terminal_output_capability);
        let max_pin_length = terminal_overrides
            .max_pin_length
            .unwrap_or(TsysTransitMaxPinLength::NotSupported);
        let terminal_card_capture_capability = terminal_overrides
            .terminal_card_capture_capability
            .unwrap_or(TsysTransitTerminalCardCaptureCapability::NoCapability);
        let cardholder_present_detail = terminal_overrides
            .cardholder_present_detail
            .unwrap_or_else(|| {
                if recurring_context.enabled {
                    if recurring_context.billing_type.is_some() {
                        TsysTransitCardholderPresentDetail::CardholderNotPresentInstallmentTransaction
                    } else {
                        TsysTransitCardholderPresentDetail::CardholderNotPresentRecurringTransaction
                    }
                } else {
                    match channel {
                        Some(PaymentChannel::TelephoneOrder) => {
                            TsysTransitCardholderPresentDetail::CardholderNotPresentPhoneTransaction
                        }
                        Some(PaymentChannel::MailOrder) => {
                            TsysTransitCardholderPresentDetail::CardholderNotPresentMailTransaction
                        }
                        _ => TsysTransitCardholderPresentDetail::CardholderNotPresentElectronicCommerce,
                    }
                }
            });
        let card_present_detail = terminal_overrides
            .card_present_detail
            .unwrap_or(TsysTransitCardPresentDetail::CardNotPresent);
        let is_stored_credential_flow =
            !matches!(mandate_dispatch, MandateDispatch::None) || is_cit_setup;
        let default_card_data_input_mode = if recurring_context.enabled || is_stored_credential_flow
        {
            TsysTransitCardDataInputMode::MerchantInitiatedTransactionCardCredentialStoredOnFile
        } else {
            match channel {
                Some(PaymentChannel::Ecommerce) | None => {
                    TsysTransitCardDataInputMode::PanEntryElectronicCommerceIncludingRemoteChip
                }
                _ => TsysTransitCardDataInputMode::KeyEnteredInput,
            }
        };
        let card_data_input_mode = terminal_overrides
            .card_data_input_mode
            .unwrap_or(default_card_data_input_mode);
        let cardholder_authentication_entity = terminal_overrides
            .cardholder_authentication_entity
            .unwrap_or(TsysTransitCardholderAuthenticationEntity::NotAuthenticated);
        let card_data_output_capability = terminal_overrides
            .card_data_output_capability
            .unwrap_or(TsysTransitCardDataOutputCapability::None);
        let (card_number, expiration_date, cvv2_opt, customer_code_opt, wallet_details_opt) =
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

        let cof_mit_status_indicator = match (
            card_network.as_ref(),
            router_data.request.mit_category.as_ref(),
            &mandate_dispatch,
        ) {
            (
                Some(CardNetwork::Mastercard),
                Some(MitCategory::Unscheduled) | Some(MitCategory::Resubmission) | None,
                MandateDispatch::Ntid { .. } | MandateDispatch::Vault { .. },
            ) => Some(TsysTransitMitIndicator::M101),
            (
                Some(CardNetwork::Discover),
                Some(MitCategory::Unscheduled) | Some(MitCategory::Resubmission),
                MandateDispatch::Ntid { .. } | MandateDispatch::Vault { .. },
            ) => Some(TsysTransitMitIndicator::U),
            _ => None,
        };

        let (cit_status_indicator, mit_status_indicator) = match &mandate_dispatch {
            MandateDispatch::Ntid { .. } | MandateDispatch::Vault { .. } => (
                None,
                recurring_context
                    .mit_status_indicator
                    .or(cof_mit_status_indicator),
            ),
            MandateDispatch::None if is_cit_setup => (
                recurring_context.mc_cit_status_indicator.or_else(|| {
                    matches!(card_network.as_ref(), Some(CardNetwork::Mastercard))
                        .then_some(TsysTransitMcCitStatusIndicator::C101)
                }),
                None,
            ),
            MandateDispatch::None => (None, None),
        };

        let partial_auth_support = if recurring_context.enabled
            || !matches!(mandate_dispatch, MandateDispatch::None)
            || commercial_card_context.commercial_card_level.is_some()
        {
            None
        } else {
            Some("YES".to_string())
        };

        let body = TsysTransitAuthorizeBody {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            card_data_source,
            transaction_amount,
            sales_tax: commercial_card_context.sales_tax,
            surcharge,
            additional_tax_details: commercial_card_context.additional_tax_details,
            shipping_charges: commercial_card_context.shipping_charges,
            duty_charges: commercial_card_context.duty_charges,
            card_number,
            expiration_date,
            cvv2: cvv2_opt,
            secure_code: three_ds_context.secure_code,
            ucaf_collection_indicator: three_ds_context.ucaf_collection_indicator,
            directory_server_transaction_id: three_ds_context.directory_server_transaction_id,
            eci_indicator: three_ds_context.eci_indicator,
            customer_code: customer_code_opt,
            wallet_details: wallet_details_opt,
            card_on_file_transaction_identifier: card_on_file_context
                .card_on_file_transaction_identifier,
            previous_network_transaction_id: card_on_file_context.previous_network_transaction_id,
            cit_status_indicator,
            mit_status_indicator,
            address_line1,
            zip,
            external_reference_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            product_details: commercial_card_context.product_details,
            commercial_card_level: commercial_card_context.commercial_card_level,
            purchase_order: commercial_card_context.purchase_order,
            charge_descriptor: commercial_card_context.charge_descriptor,
            charge_descriptor_2: commercial_card_context.charge_descriptor_2,
            charge_descriptor_3: commercial_card_context.charge_descriptor_3,
            charge_descriptor_4: commercial_card_context.charge_descriptor_4,
            customer_vat_number: commercial_card_context.customer_vat_number,
            customer_ref_id: commercial_card_context.customer_ref_id,
            supplier_reference_number: commercial_card_context.supplier_reference_number,
            order_date: commercial_card_context.order_date,
            summary_commodity_code: commercial_card_context.summary_commodity_code,
            vat_invoice: commercial_card_context.vat_invoice,
            ship_from_zip: commercial_card_context.ship_from_zip,
            ship_to_zip: commercial_card_context.ship_to_zip,
            destination_country_code: commercial_card_context.destination_country_code,
            card_on_file: card_on_file_context.card_on_file,
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
            mit: card_on_file_context.mit_block,
        };

        Ok(if is_manual_capture {
            Self::Auth(body)
        } else {
            Self::Sale(body)
        })
    }
}
#[derive(Debug, Clone)]
enum MandateDispatch {
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
        return MandateDispatch::Ntid { ntid: ntid.clone() };
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
    if let Some(ntid) = raw.strip_prefix("ntid:") {
        if !ntid.is_empty() {
            return MandateDispatch::Ntid {
                ntid: ntid.to_string(),
            };
        }
    }
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

        // On A0002 (partial approval) TSYS returns the actually approved
        // value in <processedAmount>. Forward it as MinorUnit so the core
        // can reconcile the partial capture/authorization against the
        // requested amount; for the fully-approved A0000 case this is also
        // harmless to populate.
        let minor_amount_captured = body.processed_amount.as_ref().and_then(|amount| {
            crate::connectors::tsys_transit::TsysTransitAmountConvertor::convert_back(
                amount.clone(),
                router_data.request.currency,
            )
            .ok()
        });
        let amount_captured = minor_amount_captured.map(|m| m.get_amount_as_i64());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: body.auth_code.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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
fn map_sync_status(response: &TsysTransitTransactionInquiryResponse) -> AttemptStatus {
    match (
        response.status.as_ref(),
        response.transaction_state.as_ref(),
    ) {
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Authorized)) => {
            AttemptStatus::Authorized
        }
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Captured)) => {
            AttemptStatus::Charged
        }
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Settled)) => {
            AttemptStatus::Charged
        }
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Voided)) => {
            AttemptStatus::Voided
        }
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Returned)) => {
            AttemptStatus::AutoRefunded
        }
        (Some(TsysTransitStatus::Fail), _) => AttemptStatus::Failure,
        _ => {
            tracing::warn!(
                "tsysTransit: PSync response missing or unrecognized transactionState; defaulting to Pending"
            );
            AttemptStatus::Pending
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

        let status = map_sync_status(response);

        if matches!(status, AttemptStatus::Failure) {
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
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
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
                        "tsysTransit: PSync response and request both missing transactionID.",
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
fn map_rsync_status(response: &TsysTransitTransactionInquiryResponse) -> RefundStatus {
    match (
        response.status.as_ref(),
        response.transaction_state.as_ref(),
    ) {
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Returned)) => {
            RefundStatus::Success
        }
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Settled)) => {
            RefundStatus::Success
        }
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Voided)) => {
            RefundStatus::Failure
        }
        (Some(TsysTransitStatus::Fail), _) => RefundStatus::Failure,
        _ => {
            tracing::warn!(
                "tsysTransit: RSync response missing or unrecognized transactionState; defaulting to Pending"
            );
            RefundStatus::Pending
        }
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

        let refund_status = map_rsync_status(response);

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
                }),
                ..router_data.clone()
            });
        }
        let connector_refund_id = match response.transaction_id.clone() {
            Some(id) => id,
            None => {
                if !router_data.request.connector_refund_id.is_empty() {
                    router_data.request.connector_refund_id.clone()
                } else if !router_data.request.connector_transaction_id.is_empty() {
                    router_data.request.connector_transaction_id.clone()
                } else {
                    return Err(crate::utils::response_deserialization_fail(
                        item.http_code,
                        "tsysTransit: RSync response and request both missing transactionID.",
                    )
                    .into());
                }
            }
        };

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
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
        let void_reason = {
            let raw = router_data
                .request
                .cancellation_reason
                .clone()
                .unwrap_or_else(|| "RETURN_REVERSAL".to_string());
            if raw.len() > 80 {
                raw.chars().take(80).collect()
            } else {
                raw
            }
        };

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
        let void_reason = {
            let raw = router_data
                .request
                .cancellation_reason
                .clone()
                .unwrap_or_else(|| "POST_AUTH_USER_DECLINE".to_string());
            if raw.len() > 80 {
                raw.chars().take(80).collect()
            } else {
                raw
            }
        };

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
        let (cardholder_first_name, cardholder_last_name) =
            split_domain_full_name(card.card_holder_name.clone());
        let first_name = billing
            .and_then(|a| a.first_name.clone())
            .or(cardholder_first_name)
            .map(|name| Secret::new(sanitize_alphanumeric_space(name.peek(), 25)));
        let derived_last_name = billing
            .and_then(|a| a.last_name.clone())
            .or(cardholder_last_name)
            .map(|name| Secret::new(sanitize_alphanumeric_space(name.peek(), 25)));
        let last_name = if matches!(card.card_network, Some(CardNetwork::Visa)) {
            Some(derived_last_name.ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "billing.address.last_name required for Visa CardAuthentication Account Name Inquiry",
                    context: Default::default(),
                })
            })?)
        } else {
            derived_last_name
        };

        let channel = router_data.request.payment_channel.clone();
        let card_data_source = match channel {
            Some(PaymentChannel::TelephoneOrder) => TsysTransitCardDataSource::Phone,
            Some(PaymentChannel::MailOrder) => TsysTransitCardDataSource::Mail,
            Some(PaymentChannel::Ecommerce) | None => TsysTransitCardDataSource::Internet,
        };
        let merchant_metadata = match router_data.request.metadata.as_ref() {
            Some(meta) => {
                serde_json::from_value::<TsysTransitMerchantMetadata>(meta.clone().expose())
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "connector_metadata.tsys_transit",
                        context: Default::default(),
                    })?
            }
            None => TsysTransitMerchantMetadata::default(),
        };
        let merchant_inner = merchant_metadata.into_inner();
        let terminal_overrides = merchant_inner.terminal_data.unwrap_or_default();
        let card_network = card.card_network.clone();
        let recurring_context = compute_recurring_context(
            router_data.request.mit_category.clone(),
            None,
            card_network.as_ref(),
        )?;
        let cit_status_indicator = if matches!(card_network, Some(CardNetwork::Mastercard)) {
            recurring_context
                .mc_cit_status_indicator
                .or(Some(TsysTransitMcCitStatusIndicator::C101))
        } else {
            None
        };

        let terminal_capability = terminal_overrides
            .terminal_capability
            .unwrap_or(TsysTransitTerminalCapability::KeyedEntryOnly);
        let terminal_operating_environment = terminal_overrides
            .terminal_operating_environment
            .unwrap_or(TsysTransitTerminalOperatingEnvironment::NoTerminal);
        let cardholder_authentication_method = terminal_overrides
            .cardholder_authentication_method
            .unwrap_or(TsysTransitCardholderAuthenticationMethod::NotAuthenticated);
        let terminal_authentication_capability = terminal_overrides
            .terminal_authentication_capability
            .unwrap_or(TsysTransitTerminalAuthenticationCapability::NoCapability);
        let terminal_output_capability = terminal_overrides
            .terminal_output_capability
            .unwrap_or(TsysTransitTerminalOutputCapability::None);
        let max_pin_length = terminal_overrides
            .max_pin_length
            .unwrap_or(TsysTransitMaxPinLength::NotSupported);
        let terminal_card_capture_capability = terminal_overrides
            .terminal_card_capture_capability
            .unwrap_or(TsysTransitTerminalCardCaptureCapability::NoCapability);
        let cardholder_present_detail = terminal_overrides
            .cardholder_present_detail
            .unwrap_or_else(|| {
                if recurring_context.enabled
                    && matches!(card_network, Some(CardNetwork::Mastercard))
                {
                    return TsysTransitCardholderPresentDetail::CardholderNotPresentRecurringTransaction;
                }
                match channel {
                    Some(PaymentChannel::TelephoneOrder) => {
                        TsysTransitCardholderPresentDetail::CardholderNotPresentPhoneTransaction
                    }
                    Some(PaymentChannel::MailOrder) => {
                        TsysTransitCardholderPresentDetail::CardholderNotPresentMailTransaction
                    }
                    _ => TsysTransitCardholderPresentDetail::CardholderNotPresentElectronicCommerce,
                }
            });
        let card_present_detail = terminal_overrides
            .card_present_detail
            .unwrap_or(TsysTransitCardPresentDetail::CardNotPresent);
        let is_cit_setup = router_data.request.setup_future_usage == Some(FutureUsage::OffSession)
            || router_data.request.off_session == Some(true);
        let default_card_data_input_mode = if is_cit_setup {
            TsysTransitCardDataInputMode::MerchantInitiatedTransactionCardCredentialStoredOnFile
        } else {
            match channel {
                Some(PaymentChannel::Ecommerce) | None => {
                    TsysTransitCardDataInputMode::PanEntryElectronicCommerceIncludingRemoteChip
                }
                _ => TsysTransitCardDataInputMode::KeyEnteredInput,
            }
        };
        let card_data_input_mode = terminal_overrides
            .card_data_input_mode
            .unwrap_or(default_card_data_input_mode);
        let cardholder_authentication_entity = terminal_overrides
            .cardholder_authentication_entity
            .unwrap_or(TsysTransitCardholderAuthenticationEntity::NotAuthenticated);
        let card_data_output_capability = terminal_overrides
            .card_data_output_capability
            .unwrap_or(TsysTransitCardDataOutputCapability::None);

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            card_data_source,
            card_number: Secret::new(card.card_number.peek().to_string()),
            expiration_date: format_expiration_date(card),
            address_line1,
            zip,
            external_reference_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
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
            m_pos_acceptance_device_type: "0".to_string(),
            card_on_file: None,
            cit_status_indicator,
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

        let path_a_mandate_id = format!("ntid:{ntid_source}");
        let mandate_reference = Box::new(MandateReference {
            connector_mandate_id: Some(path_a_mandate_id),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        });

        let connector_txn_id = response
            .transaction_id
            .clone()
            .unwrap_or_else(|| ntid_source.clone());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_txn_id.clone()),
            redirection_data: None,
            mandate_reference: Some(mandate_reference),
            connector_metadata: None,
            network_txn_id: response.auth_code.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Authorized,
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
        payment_channel: None,
        enable_partial_authorization: req.enable_partial_authorization,
        locale: req.locale.clone(),
        redirect_response: None,
        threeds_method_comp_ind: None,
        continue_redirection_url: None,
        tokenization: None,
        mit_category: req.mit_category.clone(),
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
            network_txn_id: body.auth_code.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(transaction_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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
