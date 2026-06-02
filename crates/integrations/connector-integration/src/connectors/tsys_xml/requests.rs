use std::fmt::Debug;

use common_utils::types::StringMajorUnit;
use domain_types::{errors::IntegrationError, payment_method_data::PaymentMethodDataTypes};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::super::macros::GetSoapXml;

/// Origin of card data — drives how TransIT scores risk / which fields are required.
///
/// PHONE = MOTO, INTERNET = eCommerce, MANUAL = keyed (incremental auth / void),
/// RECURRING = scheduled MIT. Tech spec § Sale/Auth Field Reference.
#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysXmlCardDataSource {
    Phone,
    Internet,
    Manual,
    Recurring,
    Mail,
}

// =============================================================================
// TerminalData group — XSD-driven enums for the e-commerce cert script.
//
// Every variant carries its exact XSD wire string via `#[serde(rename = "...")]`.
// We avoid `rename_all` to keep the wire contract explicit.
//
// `Deserialize` is derived on each enum so the connector metadata override
// (`connector_metadata.tsys_xml.terminal_data.*`) — which arrives as a
// `serde_json::Value` — can parse straight into these types.
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlTerminalCapability {
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "NO_TERMINAL_MANUAL")]
    NoTerminalManual,
    #[serde(rename = "MAGSTRIPE_READ_ONLY")]
    MagstripeReadOnly,
    #[serde(rename = "OCR")]
    Ocr,
    #[serde(rename = "ICC_CHIP_READ_ONLY")]
    IccChipReadOnly,
    #[serde(rename = "KEYED_ENTRY_ONLY")]
    KeyedEntryOnly,
    #[serde(rename = "MAGSTRIPE_CONTACTLESS_ONLY")]
    MagstripeContactlessOnly,
    #[serde(rename = "MAGSTRIPE_KEYED_ENTRY_ONLY")]
    MagstripeKeyedEntryOnly,
    #[serde(rename = "MAGSTRIPE_ICC_KEYED_ENTRY_ONLY")]
    MagstripeIccKeyedEntryOnly,
    #[serde(rename = "MAGSTRIPE_ICC_ONLY")]
    MagstripeIccOnly,
    #[serde(rename = "ICC_KEYED_ENTRY_ONLY")]
    IccKeyedEntryOnly,
    #[serde(rename = "ICC_CHIP_CONTACT_CONTACTLESS")]
    IccChipContactContactless,
    #[serde(rename = "ICC_CONTACTLESS_ONLY")]
    IccContactlessOnly,
    #[serde(rename = "OTHER_CAPABILITY_FOR_MASTERCARD")]
    OtherCapabilityForMastercard,
    #[serde(rename = "MAGSTRIPE_SIGNATURE_FOR_AMEX_ONLY")]
    MagstripeSignatureForAmexOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlTerminalOperatingEnvironment {
    #[serde(rename = "NO_TERMINAL")]
    NoTerminal,
    #[serde(rename = "ON_MERCHANT_PREMISES_ATTENDED")]
    OnMerchantPremisesAttended,
    #[serde(rename = "ON_MERCHANT_PREMISES_UNATTENDED")]
    OnMerchantPremisesUnattended,
    #[serde(rename = "OFF_MERCHANT_PREMISES_ATTENDED")]
    OffMerchantPremisesAttended,
    #[serde(rename = "OFF_MERCHANT_PREMISES_UNATTENDED")]
    OffMerchantPremisesUnattended,
    #[serde(rename = "ON_CUSTOMER_PREMISES_UNATTENDED")]
    OnCustomerPremisesUnattended,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "ELECTRONIC_DELIVERY_AMEX")]
    ElectronicDeliveryAmex,
    #[serde(rename = "PHYSICAL_DELIVERY_AMEX")]
    PhysicalDeliveryAmex,
    #[serde(rename = "OFF_MERCHANT_PREMISES_MPOS")]
    OffMerchantPremisesMpos,
    #[serde(rename = "ON_MERCHANT_PREMISES_MPOS")]
    OnMerchantPremisesMpos,
    #[serde(rename = "OFF_MERCHANT_PREMISES_CUSTOMER_POS")]
    OffMerchantPremisesCustomerPos,
    #[serde(rename = "ON_MERCHANT_PREMISES_CUSTOMER_POS")]
    OnMerchantPremisesCustomerPos,
    #[serde(rename = "OFF_CUSTOMER_PREMISES_UNATTENDED")]
    OffCustomerPremisesUnattended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlCardholderAuthenticationMethod {
    #[serde(rename = "NOT_AUTHENTICATED")]
    NotAuthenticated,
    #[serde(rename = "PIN")]
    Pin,
    #[serde(rename = "ELECTRONIC_SIGNATURE_ANALYSIS")]
    ElectronicSignatureAnalysis,
    #[serde(rename = "MANUAL_SIGNATURE")]
    ManualSignature,
    #[serde(rename = "MANUAL_OTHER")]
    ManualOther,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "SYSTEMATIC_OTHER")]
    SystematicOther,
    #[serde(rename = "E_TICKET_ENV_AMEX")]
    ETicketEnvAmex,
    #[serde(rename = "OFFLINE_PIN")]
    OfflinePin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlTerminalAuthenticationCapability {
    #[serde(rename = "NO_CAPABILITY")]
    NoCapability,
    #[serde(rename = "PIN_ENTRY")]
    PinEntry,
    #[serde(rename = "SIGNATURE_ANALYSIS")]
    SignatureAnalysis,
    #[serde(rename = "MPOS_SOFTWARE_BASED_PIN_ENTRY_CAPABILITY")]
    MposSoftwareBasedPinEntryCapability,
    #[serde(rename = "SIGNATURE_ANALYSIS_INOPERATIVE")]
    SignatureAnalysisInoperative,
    #[serde(rename = "OTHER")]
    Other,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlTerminalOutputCapability {
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "PRINT_ONLY")]
    PrintOnly,
    #[serde(rename = "DISPLAY_ONLY")]
    DisplayOnly,
    #[serde(rename = "PRINT_AND_DISPLAY")]
    PrintAndDisplay,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlMaxPinLength {
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "NOT_SUPPORTED")]
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
pub enum TsysXmlTerminalCardCaptureCapability {
    #[serde(rename = "NO_CAPABILITY")]
    NoCapability,
    #[serde(rename = "CARD_CAPTURE_CAPABILITY")]
    CardCaptureCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlCardholderPresentDetail {
    #[serde(rename = "CLICK_TO_PAY_DISCOVER")]
    ClickToPayDiscover,
    #[serde(rename = "CARDHOLDER_PRESENT")]
    CardholderPresent,
    #[serde(rename = "CARDHOLDER_NOT_PRESENT_UNSPECIFIED_REASON")]
    CardholderNotPresentUnspecifiedReason,
    #[serde(rename = "CARDHOLDER_NOT_PRESENT_MAIL_TRANSACTION")]
    CardholderNotPresentMailTransaction,
    #[serde(rename = "CARDHOLDER_NOT_PRESENT_PHONE_TRANSACTION")]
    CardholderNotPresentPhoneTransaction,
    #[serde(rename = "CARDHOLDER_NOT_PRESENT_RECURRING_TRANSACTION")]
    CardholderNotPresentRecurringTransaction,
    #[serde(rename = "CARDHOLDER_NOT_PRESENT_ELECTRONIC_COMMERCE")]
    CardholderNotPresentElectronicCommerce,
    #[serde(rename = "CARDHOLDER_NOT_PRESENT_INSTALLMENT_TRANSACTION")]
    CardholderNotPresentInstallmentTransaction,
    #[serde(rename = "PARTIAL_SHIPMENT_TRANSACTION_ON_TOKEN_CRYPTOGRAM_TXN")]
    PartialShipmentTransactionOnTokenCryptogramTxn,
    #[serde(rename = "RECURRING_TRANSACTION_ON_TOKEN_CRYPTOGRAM_TXN")]
    RecurringTransactionOnTokenCryptogramTxn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlCardPresentDetail {
    #[serde(rename = "CARD_NOT_PRESENT")]
    CardNotPresent,
    #[serde(rename = "CARD_PRESENT")]
    CardPresent,
    #[serde(rename = "TRANSPONDER_AMEX")]
    TransponderAmex,
    #[serde(rename = "CONTACTLESS_CHIP_TRANSACTIONS")]
    ContactlessChipTransactions,
    #[serde(rename = "DIGITAL_WALLET_AMEX")]
    DigitalWalletAmex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlCardDataInputMode {
    #[serde(rename = "VOICE_AUTH_ARU_ONLY")]
    VoiceAuthAruOnly,
    #[serde(rename = "MAGNETIC_STRIPE_READER_INPUT")]
    MagneticStripeReaderInput,
    #[serde(rename = "BAR_CODE_PAYMENT_CODE")]
    BarCodePaymentCode,
    #[serde(rename = "KEY_ENTERED_INPUT")]
    KeyEnteredInput,
    #[serde(rename = "MERCHANT_INITIATED_TRANSACTION_CARD_CREDENTIAL_STORED_ON_FILE")]
    MerchantInitiatedTransactionCardCredentialStoredOnFile,
    #[serde(rename = "PAN_AUTO_ENTRY_CONTACTLESS_MAGNETIC_STRIPE")]
    PanAutoEntryContactlessMagneticStripe,
    #[serde(rename = "MAGNETIC_STRIPE_READER_INPUT_TRACK_DATA_CAPTURED_PASSED_UNALTERED")]
    MagneticStripeReaderInputTrackDataCapturedPassedUnaltered,
    #[serde(rename = "ONLINE_CHIP")]
    OnlineChip,
    #[serde(rename = "OFFLINE_CHIP")]
    OfflineChip,
    #[serde(rename = "PAN_AUTO_ENTRY_CONTACTLESS_CHIP_CARD")]
    PanAutoEntryContactlessChipCard,
    #[serde(rename = "TRACK_DATA_READ_UNALTERED_CHIP_CAPABLE_TERMINAL_CHIP_DATA_NOT_READ")]
    TrackDataReadUnalteredChipCapableTerminalChipDataNotRead,
    #[serde(rename = "EMPTY_CANDIDATE_LIST_FALLBACK")]
    EmptyCandidateListFallback,
    #[serde(rename = "PAN_ENTRY_ELECTRONIC_COMMERCE_INCLUDING_REMOTE_CHIP")]
    PanEntryElectronicCommerceIncludingRemoteChip,
    #[serde(
        rename = "ELECTRONIC_COMMERCE_NO_SECURITY_CHANNEL_ENCRYPTED_SET_WITHOUT_CARDHOLDER_CERTIFICATE"
    )]
    ElectronicCommerceNoSecurityChannelEncryptedSetWithoutCardholderCertificate,
    #[serde(rename = "MANUALLY_ENTERED_WITH_KEYED_CID_AMEX_JCB")]
    ManuallyEnteredWithKeyedCidAmexJcb,
    #[serde(rename = "SWIPED_TRANSACTION_WITH_KEYED_CID_AMEX_JCB")]
    SwipedTransactionWithKeyedCidAmexJcb,
    #[serde(rename = "CONTACTLESS_TO_CONTACT_CHIP_CARD_SWITCH_TRANSACTION_DISCOVER_ONLY")]
    ContactlessToContactChipCardSwitchTransactionDiscoverOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlCardholderAuthenticationEntity {
    #[serde(rename = "NOT_AUTHENTICATED")]
    NotAuthenticated,
    #[serde(rename = "ICC_OFFLINE_PIN")]
    IccOfflinePin,
    #[serde(rename = "CARD_ACCEPTANCE_DEVICE")]
    CardAcceptanceDevice,
    #[serde(rename = "AUTHORIZING_AGENT_ONLINE_PIN")]
    AuthorizingAgentOnlinePin,
    #[serde(rename = "MERCHANT_CARD_ACCEPTOR_SIGNATURE")]
    MerchantCardAcceptorSignature,
    #[serde(rename = "OTHER")]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TsysXmlCardDataOutputCapability {
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "MAGNETIC_STRIPE_WRITE")]
    MagneticStripeWrite,
    #[serde(rename = "ICC")]
    Icc,
    #[serde(rename = "OTHER")]
    Other,
}

/// MC/AMEX-only field. PREAUTH for manual capture (delayed funds), FINAL for
/// auto-capture (Sale).
#[derive(Debug, Clone, Serialize)]
pub enum TsysXmlAuthorizationIndicator {
    #[serde(rename = "PREAUTH")]
    Preauth,
    #[serde(rename = "FINAL")]
    Final,
}

/// `<cardOnFile>` flag — `Y` when a credential is being used / stored on file
/// (CIT / MIT / vault), `N` otherwise. Two-variant enum keeps the wire contract
/// explicit per tech spec § CIT/MIT.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysXmlCardOnFile {
    #[serde(rename = "Y")]
    Y,
    #[serde(rename = "N")]
    N,
}

/// Merchant-initiated-transaction indicator — TransIT XSD enum per § CIT/MIT.
/// Values:
/// - `R` — recurring (cert script "Recurring" rows)
/// - `S` — installment (cert script "Installment" rows) — Discover family alias
/// - `T` — installment (cert script "Installment" rows) — Discover family alt
/// - `M101` — resubmission
/// - `M102` — reauthorization
/// - `M103` — delayed charge
/// - `M104` — no-show
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub enum TsysXmlMitIndicator {
    #[default]
    R,
    M101,
    M102,
    M103,
    M104,
    S,
    T,
}

/// `<isRecurring>` — emitted as `Y` on every recurring/installment Step 5/6
/// row per the cert script. Treated as Option<_> on the wire (skip when absent)
/// because non-recurring flows still use the same XML body.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysXmlIsRecurring {
    Y,
}

/// `<billingType>` — only present on installment rows (cert Step 6).
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysXmlBillingType {
    #[serde(rename = "INSTALLMENT")]
    Installment,
}

/// Commercial-card enhanced-data level.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysXmlCommercialCardLevel {
    #[serde(rename = "LEVEL2")]
    Level2,
    #[serde(rename = "LEVEL3")]
    Level3,
}

/// `<citStatusIndicator>` — MasterCard CIT (Step 4) only:
/// `C102` Standing Order intent / `C103` Subscription intent / `C104` Installment intent.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysXmlMcCitStatusIndicator {
    C102,
    C103,
    C104,
}

/// `<mit>` wrapper carrying the MIT indicator value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "mit")]
pub struct TsysXmlMit {
    #[serde(rename = "mitIndicator")]
    pub mit_indicator: TsysXmlMitIndicator,
}

/// Vault wallet details — emitted on Path B MIT (and CreateConnectorCustomer
/// response shape). The `<walletDetails><walletID>...</walletID></walletDetails>`
/// structure replaces PAN/expiry/cvv2 on Path B Authorize calls.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "walletDetails")]
pub struct TsysXmlWalletDetailsRef {
    #[serde(rename = "walletID")]
    pub wallet_id: Secret<String>,
}

/// Order-level tax addendum used by Level 3 Visa/MasterCard requests.
#[derive(Debug, Clone, Serialize)]
pub struct TsysXmlAdditionalTaxDetails {
    #[serde(rename = "taxType")]
    pub tax_type: String,
    #[serde(rename = "taxAmount")]
    pub tax_amount: StringMajorUnit,
    #[serde(rename = "taxRate", skip_serializing_if = "Option::is_none")]
    pub tax_rate: Option<String>,
    #[serde(rename = "taxCategory", skip_serializing_if = "Option::is_none")]
    pub tax_category: Option<String>,
}

/// Per-line tax block nested under `<productDetails>`.
#[derive(Debug, Clone, Serialize)]
pub struct TsysXmlProductTaxDetails {
    #[serde(rename = "productTaxName", skip_serializing_if = "Option::is_none")]
    pub product_tax_name: Option<String>,
    #[serde(rename = "productTaxAmount", skip_serializing_if = "Option::is_none")]
    pub product_tax_amount: Option<StringMajorUnit>,
    #[serde(
        rename = "productTaxPercentage",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_tax_percentage: Option<String>,
    #[serde(rename = "productTaxType", skip_serializing_if = "Option::is_none")]
    pub product_tax_type: Option<String>,
}

/// Per-line discount block nested under `<productDetails>`.
#[derive(Debug, Clone, Serialize)]
pub struct TsysXmlProductDiscountDetails {
    #[serde(rename = "productDiscountName")]
    pub product_discount_name: String,
    #[serde(rename = "productDiscountAmount")]
    pub product_discount_amount: StringMajorUnit,
    #[serde(
        rename = "productDiscountPercentage",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_discount_percentage: Option<String>,
    #[serde(rename = "productDiscountType")]
    pub product_discount_type: String,
    #[serde(rename = "priority")]
    pub priority: u16,
    #[serde(rename = "stackable")]
    pub stackable: TsysXmlYesNo,
}

/// Per-line modifier block nested under `<productDetails>`.
#[derive(Debug, Clone, Serialize)]
pub struct TsysXmlProductModifierDetails {
    #[serde(rename = "modifierName")]
    pub modifier_name: String,
    #[serde(rename = "modifierValue", skip_serializing_if = "Option::is_none")]
    pub modifier_value: Option<String>,
    #[serde(rename = "modifierPrice", skip_serializing_if = "Option::is_none")]
    pub modifier_price: Option<StringMajorUnit>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysXmlProductDiscountIndicator {
    #[serde(rename = "Y")]
    Y,
    #[serde(rename = "N")]
    N,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysXmlYesNo {
    #[serde(rename = "YES")]
    Yes,
    #[serde(rename = "NO")]
    No,
}

/// Level 3 line-item detail.
#[derive(Debug, Clone, Serialize)]
pub struct TsysXmlProductDetails {
    #[serde(rename = "productCode")]
    pub product_code: String,
    #[serde(rename = "productName")]
    pub product_name: String,
    #[serde(rename = "price")]
    pub price: StringMajorUnit,
    #[serde(rename = "quantity")]
    pub quantity: u32,
    #[serde(rename = "measurementUnit", skip_serializing_if = "Option::is_none")]
    pub measurement_unit: Option<String>,
    #[serde(
        rename = "productDiscountDetails",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_discount_details: Option<TsysXmlProductDiscountDetails>,
    #[serde(rename = "productTaxDetails", skip_serializing_if = "Option::is_none")]
    pub product_tax_details: Option<TsysXmlProductTaxDetails>,
    #[serde(rename = "productVariation", skip_serializing_if = "Option::is_none")]
    pub product_variation: Option<String>,
    #[serde(
        rename = "productModifierDetails",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_modifier_details: Option<TsysXmlProductModifierDetails>,
    #[serde(rename = "productNotes", skip_serializing_if = "Option::is_none")]
    pub product_notes: Option<String>,
    #[serde(
        rename = "productDiscountIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_discount_indicator: Option<TsysXmlProductDiscountIndicator>,
    #[serde(
        rename = "productCommodityCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_commodity_code: Option<String>,
}

/// Discover/JCB/Diners/CUP-only signal indicating whether the cardholder is a
/// registered user in the merchant's system.
#[derive(Debug, Clone, Serialize)]
pub enum TsysXmlRegisteredUserIndicator {
    #[serde(rename = "YES")]
    Yes,
    #[serde(rename = "NO")]
    No,
}

/// XSD `terminalData` group — required by the TransIT e-commerce certification
/// script for every authorization. The 12 inner fields are all required.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
#[serde(rename = "terminalData")]
pub struct TsysXmlTerminalData {
    #[serde(rename = "terminalCapability")]
    pub terminal_capability: TsysXmlTerminalCapability,
    #[serde(rename = "terminalOperatingEnvironment")]
    pub terminal_operating_environment: TsysXmlTerminalOperatingEnvironment,
    #[serde(rename = "cardholderAuthenticationMethod")]
    pub cardholder_authentication_method: TsysXmlCardholderAuthenticationMethod,
    #[serde(rename = "terminalAuthenticationCapability")]
    pub terminal_authentication_capability: TsysXmlTerminalAuthenticationCapability,
    #[serde(rename = "terminalOutputCapability")]
    pub terminal_output_capability: TsysXmlTerminalOutputCapability,
    #[serde(rename = "maxPinLength")]
    pub max_pin_length: TsysXmlMaxPinLength,
    #[serde(rename = "terminalCardCaptureCapability")]
    pub terminal_card_capture_capability: TsysXmlTerminalCardCaptureCapability,
    #[serde(rename = "cardholderPresentDetail")]
    pub cardholder_present_detail: TsysXmlCardholderPresentDetail,
    #[serde(rename = "cardPresentDetail")]
    pub card_present_detail: TsysXmlCardPresentDetail,
    #[serde(rename = "cardDataInputMode")]
    pub card_data_input_mode: TsysXmlCardDataInputMode,
    #[serde(rename = "cardholderAuthenticationEntity")]
    pub cardholder_authentication_entity: TsysXmlCardholderAuthenticationEntity,
    #[serde(rename = "cardDataOutputCapability")]
    pub card_data_output_capability: TsysXmlCardDataOutputCapability,
}

/// XSD `developerInfo` wrapper. Cert script asks for the developerID to be
/// nested under a `<developerInfo>` element on the Authorize flow.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
#[serde(rename = "developerInfo")]
pub struct TsysXmlDeveloperInfo {
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

fn generate_xml<T: Serialize>(
    request: &T,
) -> Result<String, error_stack::Report<IntegrationError>> {
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

fn mask_xml_tag(xml: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut masked = String::with_capacity(xml.len());
    let mut remaining = xml;

    while let Some(start) = remaining.find(&open) {
        let value_start = start + open.len();
        masked.push_str(&remaining[..value_start]);

        let Some(relative_end) = remaining[value_start..].find(&close) else {
            masked.push_str("***");
            return masked;
        };

        let value = &remaining[value_start..value_start + relative_end];
        if tag == "cardNumber" && value.len() >= 4 {
            masked.push_str(&format!("************{}", &value[value.len() - 4..]));
        } else {
            masked.push_str("***");
        }
        masked.push_str(&close);
        remaining = &remaining[value_start + relative_end + close.len()..];
    }

    masked.push_str(remaining);
    masked
}

fn mask_tsys_xml_for_logs(xml: &str) -> String {
    [
        "cardNumber",
        "expirationDate",
        "cvv2",
        "track2Data",
        "transactionKey",
        "developerID",
        "token",
        "customerCode",
        "walletID",
    ]
    .into_iter()
    .fold(xml.to_string(), |masked, tag| mask_xml_tag(&masked, tag))
}

fn generate_logged_xml<T: Serialize>(request: &T, fallback_root: &str) -> String {
    let fallback = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<{fallback_root}/>");
    let xml = generate_xml(request).unwrap_or(fallback);
    tracing::info!(
        connector = "tsys_xml",
        request_xml = %mask_tsys_xml_for_logs(&xml),
        "tsys_xml_connector_xml"
    );
    xml
}

/// TransIT Sale / Auth request.
///
/// Both `<Sale>` and `<Auth>` share the same field schema (tech spec § 1, § 2). We
/// flip the root element via a tagged enum so callers can pick at runtime based on
/// `auto_capture`.
#[derive(Debug, Serialize)]
pub enum TsysXmlAuthorizeRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "Sale")]
    Sale(TsysXmlAuthorizeBody<T>),
    #[serde(rename = "Auth")]
    Auth(TsysXmlAuthorizeBody<T>),
}

/// RepeatPayment (MIT) request — TransIT does not expose a separate recurring
/// endpoint, so we replay the same `<Sale>` / `<Auth>` shape. This newtype
/// exists purely so the macro-generated `Templating` registration is distinct
/// from the Authorize flow's; the wire body is identical.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TsysXmlRepeatPaymentRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(pub TsysXmlAuthorizeRequest<T>);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> GetSoapXml
    for TsysXmlRepeatPaymentRequest<T>
{
    fn to_soap_xml(&self) -> String {
        self.0.to_soap_xml()
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> GetSoapXml
    for TsysXmlAuthorizeRequest<T>
{
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Sale")
    }
}

// Field order MATTERS: TransIT XSD is sequence-validated. Order verified against
// the dev portal MOTO sample and live `<SaleResponse><responseCode>F9901`
// rejections that leaked the allowed-next sets.
//
// CRITICAL DEV-PORTAL DOC MISMATCH:
// The dev portal labels `terminalData` and `developerInfo` as XSD groups with
// child nodes. The live XSD does NOT have those groups — every child element
// (`terminalCapability`, `developerID`, `acceptorStreetAddress`, etc.) is a
// FLAT sibling. Verified against the F9901 error pasted into the design doc.
// `partialApprovalCapable` is similarly bogus — the real element is
// `partialAuthSupport`.
#[derive(Debug, Serialize)]
pub struct TsysXmlAuthorizeBody<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "cardDataSource")]
    pub card_data_source: TsysXmlCardDataSource,
    #[serde(rename = "transactionAmount")]
    pub transaction_amount: StringMajorUnit,
    /// Commercial-card sales tax. Public Auth/Sale samples position this
    /// directly after `transactionAmount`.
    #[serde(rename = "salesTax", skip_serializing_if = "Option::is_none")]
    pub sales_tax: Option<StringMajorUnit>,
    /// Level 3 Visa/MasterCard addendum. Repeated tag in the XSD.
    #[serde(
        rename = "additionalTaxDetails",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additional_tax_details: Vec<TsysXmlAdditionalTaxDetails>,
    #[serde(rename = "shippingCharges", skip_serializing_if = "Option::is_none")]
    pub shipping_charges: Option<StringMajorUnit>,
    #[serde(rename = "dutyCharges", skip_serializing_if = "Option::is_none")]
    pub duty_charges: Option<StringMajorUnit>,
    /// Path A (PAN / network-token MIT / CIT) — emit `<cardNumber>` /
    /// `<expirationDate>` / `<cvv2>`. Mutually exclusive with the
    /// Path B (`customerCode` + `walletDetails`) block below.
    #[serde(rename = "cardNumber", skip_serializing_if = "Option::is_none")]
    pub card_number: Option<Secret<String>>,
    /// MM/YY — TransIT explicitly documents this format (tech spec § Field Reference).
    /// Skipped on Path B (vault token MIT).
    #[serde(rename = "expirationDate", skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
    #[serde(rename = "cvv2", skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    /// 3DS CAVV/AAV value. TransIT calls this the 3D Secure Code.
    #[serde(rename = "secureCode", skip_serializing_if = "Option::is_none")]
    pub secure_code: Option<Secret<String>>,
    /// Mastercard SecureCode security protocol. `21` is channel encryption.
    #[serde(rename = "securityProtocol", skip_serializing_if = "Option::is_none")]
    pub security_protocol: Option<String>,
    #[serde(
        rename = "ucafCollectionIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub ucaf_collection_indicator: Option<String>,
    /// Mastercard DSRP cryptogram.
    #[serde(
        rename = "digitalPaymentCryptogram",
        skip_serializing_if = "Option::is_none"
    )]
    pub digital_payment_cryptogram: Option<String>,
    /// EMV 3DS program protocol version (`1` = 2.1, `2` = 2.2, etc.).
    #[serde(rename = "programProtocol", skip_serializing_if = "Option::is_none")]
    pub program_protocol: Option<String>,
    #[serde(
        rename = "directoryServerTransactionID",
        skip_serializing_if = "Option::is_none"
    )]
    pub directory_server_transaction_id: Option<String>,
    #[serde(rename = "eciIndicator", skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
    /// Path B vault dispatch (`customerCode` + `walletDetails`). When present,
    /// `card_number` / `expiration_date` / `cvv2` MUST be `None`.
    /// Sequence-positioned near the other card-source fields per tech spec §
    /// CIT/MIT; final wire order is the same as TSYS doc examples (we iterate
    /// against F9901 if needed).
    #[serde(rename = "customerCode", skip_serializing_if = "Option::is_none")]
    pub customer_code: Option<Secret<String>>,
    #[serde(rename = "walletDetails", skip_serializing_if = "Option::is_none")]
    pub wallet_details: Option<TsysXmlWalletDetailsRef>,
    /// Stored-credential reference from the originating CIT.
    ///
    /// Public keyed recurring + MOTO samples place this immediately after the
    /// PAN/expiry block and before AVS/address data.
    #[serde(
        rename = "cardOnFileTransactionIdentifier",
        skip_serializing_if = "Option::is_none"
    )]
    pub card_on_file_transaction_identifier: Option<String>,
    /// Legacy Path A NTID one-shot field. Not present in the recurring public
    /// samples, so recurring MIT flows should omit it.
    #[serde(
        rename = "previousNetworkTransactionID",
        skip_serializing_if = "Option::is_none"
    )]
    pub previous_network_transaction_id: Option<String>,
    /// `<citStatusIndicator>` appears immediately after the credential
    /// reference/expiry block in the keyed card-on-file samples.
    #[serde(rename = "citStatusIndicator", skip_serializing_if = "Option::is_none")]
    pub cit_status_indicator: Option<TsysXmlMcCitStatusIndicator>,
    /// Public recurring samples use `<mitStatusIndicator>` for both
    /// Discover-family (`R` / `S` / `T`) and MasterCard (`M102` / `M103` /
    /// `M104`) MIT flows.
    #[serde(rename = "mitStatusIndicator", skip_serializing_if = "Option::is_none")]
    pub mit_status_indicator: Option<TsysXmlMitIndicator>,
    /// Required by the cert script (AVS).
    #[serde(rename = "addressLine1")]
    pub address_line1: Secret<String>,
    /// Required by the cert script (AVS).
    #[serde(rename = "zip")]
    pub zip: Secret<String>,
    /// Required by the cert script (merchant's reference id, echoed in the response).
    #[serde(rename = "externalReferenceID")]
    pub external_reference_id: String,
    /// Level 3 line-item data.
    #[serde(
        rename = "productDetails",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub product_details: Vec<TsysXmlProductDetails>,
    /// Commercial-card qualifier. Emitted only when the merchant explicitly
    /// opts in via `metadata.tsys_xml.commercial_card`.
    #[serde(
        rename = "commercialCardLevel",
        skip_serializing_if = "Option::is_none"
    )]
    pub commercial_card_level: Option<TsysXmlCommercialCardLevel>,
    #[serde(rename = "purchaseOrder", skip_serializing_if = "Option::is_none")]
    pub purchase_order: Option<String>,
    #[serde(rename = "chargeDescriptor", skip_serializing_if = "Option::is_none")]
    pub charge_descriptor: Option<String>,
    #[serde(rename = "chargeDescriptor2", skip_serializing_if = "Option::is_none")]
    pub charge_descriptor_2: Option<String>,
    #[serde(rename = "chargeDescriptor3", skip_serializing_if = "Option::is_none")]
    pub charge_descriptor_3: Option<String>,
    #[serde(rename = "chargeDescriptor4", skip_serializing_if = "Option::is_none")]
    pub charge_descriptor_4: Option<String>,
    #[serde(rename = "customerVATNumber", skip_serializing_if = "Option::is_none")]
    pub customer_vat_number: Option<String>,
    #[serde(rename = "customerRefID", skip_serializing_if = "Option::is_none")]
    pub customer_ref_id: Option<String>,
    #[serde(
        rename = "supplierReferenceNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub supplier_reference_number: Option<String>,
    #[serde(rename = "orderDate", skip_serializing_if = "Option::is_none")]
    pub order_date: Option<String>,
    #[serde(
        rename = "summaryCommodityCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub summary_commodity_code: Option<String>,
    #[serde(rename = "vatInvoice", skip_serializing_if = "Option::is_none")]
    pub vat_invoice: Option<String>,
    #[serde(rename = "shipFromZip", skip_serializing_if = "Option::is_none")]
    pub ship_from_zip: Option<String>,
    #[serde(rename = "shipToZip", skip_serializing_if = "Option::is_none")]
    pub ship_to_zip: Option<String>,
    #[serde(
        rename = "destinationCountryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub destination_country_code: Option<String>,
    /// `<cardOnFile>` — emitted as `Y` on CIT (stored credential consent) and
    /// MIT (subsequent use) per tech spec § CIT/MIT.
    ///
    /// Position note (Bug 2 fix): TSYS F9901 rejected `cardOnFile` after
    /// `acceptorURLAddress`; the allowed-next set at that XSD slot omitted COF
    /// indicators. Moved to BEFORE `partialAuthSupport` (the conventional COF/MIT
    /// slot for TSYS card-on-file APIs). CIT (Step 4) verified PASS in this slot.
    #[serde(rename = "cardOnFile", skip_serializing_if = "Option::is_none")]
    pub card_on_file: Option<TsysXmlCardOnFile>,
    /// Generic keyed Auth/Sale samples emit this after `<cardOnFile>`, but the
    /// recurring keyed samples omit it. Keep it optional so MIT flows can match
    /// the published recurring examples exactly.
    #[serde(rename = "partialAuthSupport", skip_serializing_if = "Option::is_none")]
    pub partial_auth_support: Option<String>,
    // --- terminalData fields (flat per the XSD; dev portal groups them, XSD doesn't) ---
    #[serde(rename = "terminalCapability")]
    pub terminal_capability: TsysXmlTerminalCapability,
    #[serde(rename = "terminalOperatingEnvironment")]
    pub terminal_operating_environment: TsysXmlTerminalOperatingEnvironment,
    #[serde(rename = "cardholderAuthenticationMethod")]
    pub cardholder_authentication_method: TsysXmlCardholderAuthenticationMethod,
    #[serde(rename = "terminalAuthenticationCapability")]
    pub terminal_authentication_capability: TsysXmlTerminalAuthenticationCapability,
    #[serde(rename = "terminalOutputCapability")]
    pub terminal_output_capability: TsysXmlTerminalOutputCapability,
    #[serde(rename = "maxPinLength")]
    pub max_pin_length: TsysXmlMaxPinLength,
    #[serde(rename = "terminalCardCaptureCapability")]
    pub terminal_card_capture_capability: TsysXmlTerminalCardCaptureCapability,
    #[serde(rename = "cardholderPresentDetail")]
    pub cardholder_present_detail: TsysXmlCardholderPresentDetail,
    #[serde(rename = "cardPresentDetail")]
    pub card_present_detail: TsysXmlCardPresentDetail,
    #[serde(rename = "cardDataInputMode")]
    pub card_data_input_mode: TsysXmlCardDataInputMode,
    #[serde(rename = "cardholderAuthenticationEntity")]
    pub cardholder_authentication_entity: TsysXmlCardholderAuthenticationEntity,
    #[serde(rename = "cardDataOutputCapability")]
    pub card_data_output_capability: TsysXmlCardDataOutputCapability,
    /// developerID is a FLAT element, NOT inside a `<developerInfo>` wrapper.
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    // --- Recurring/Installment metadata cluster — XSD slot probe (F9901 at
    // `partialAuthSupport` listed the allowed-next set as `{billingType,
    // paymentCount, currentPaymentCount, originalRecurringAmount, isoIdentifier,
    // registeredUserIndicator, ..., merchantTokenRequesterID}`). These all live
    // AFTER `developerID` and BEFORE `registeredUserIndicator`. ---
    /// `<isRecurring>` — required = `Y` on every Step 5/6 row per cert.
    #[serde(rename = "isRecurring", skip_serializing_if = "Option::is_none")]
    pub is_recurring: Option<TsysXmlIsRecurring>,
    /// `<billingType>` — `INSTALLMENT` for Step 6 rows.
    #[serde(rename = "billingType", skip_serializing_if = "Option::is_none")]
    pub billing_type: Option<TsysXmlBillingType>,
    /// `<paymentCount>` — total number of installment payments (Step 6 only).
    #[serde(rename = "paymentCount", skip_serializing_if = "Option::is_none")]
    pub payment_count: Option<u32>,
    /// `<currentPaymentCount>` — which installment in the series (Step 6 only).
    #[serde(
        rename = "currentPaymentCount",
        skip_serializing_if = "Option::is_none"
    )]
    pub current_payment_count: Option<u32>,
    /// `<originalRecurringAmount>` — Discover/JCB/Diners/CUP MIT requirement.
    #[serde(
        rename = "originalRecurringAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub original_recurring_amount: Option<StringMajorUnit>,
    /// Discover/JCB/Diners/CUP only.
    #[serde(
        rename = "registeredUserIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub registered_user_indicator: Option<TsysXmlRegisteredUserIndicator>,
    /// Discover/JCB/Diners/CUP only.
    #[serde(
        rename = "lastRegisteredChangeDate",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_registered_change_date: Option<String>,
    /// MC/AMEX MOTO and keyed recurring samples place auth intent after
    /// `developerID` and any recurring/installment metadata.
    #[serde(
        rename = "authorizationIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub authorization_indicator: Option<TsysXmlAuthorizationIndicator>,
    /// MC keyed samples place acceptor data after `authorizationIndicator`.
    #[serde(
        rename = "acceptorStreetAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub acceptor_street_address: Option<String>,
    #[serde(
        rename = "acceptorCustomerServicePhoneNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub acceptor_customer_service_phone_number: Option<String>,
    #[serde(
        rename = "acceptorPhoneNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub acceptor_phone_number: Option<String>,
    #[serde(rename = "acceptorURLAddress", skip_serializing_if = "Option::is_none")]
    pub acceptor_url_address: Option<String>,
    /// Legacy `<mit>` wrapper. The public recurring samples do not use this
    /// block; it is retained only for older non-recurring stored-credential
    /// paths that still rely on the TransIT field.
    #[serde(rename = "mit", skip_serializing_if = "Option::is_none")]
    pub mit: Option<TsysXmlMit>,
    /// Phantom marker so the generic `T` is preserved on the struct without leaking
    /// into the serialized payload.
    #[serde(skip)]
    pub _marker: std::marker::PhantomData<T>,
}

/// TransIT Transaction Inquiry (PSync) request.
///
/// TODO(tsys_xml): UNDECIDED - confirm element name with TSYS.
/// The spec lists `<TransactionInquiry>` as the most likely candidate with
/// `<GetDetails>` as alternative.
#[derive(Debug, Serialize)]
#[serde(rename = "TransactionInquiry")]
pub struct TsysXmlTransactionInquiryRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysXmlTransactionInquiryRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "TransactionInquiry")
    }
}

/// RSync request — reuses the PSync `<TransactionInquiry>` shape via a type
/// alias. TransIT exposes a single inquiry endpoint for both payment and
/// refund status lookups; the type alias keeps the macro layer's Templating
/// types distinct without duplicating wire-level schema.
pub type TsysXmlRSyncRequest = TsysXmlTransactionInquiryRequest;

/// TransIT Capture request (tech spec § Capture / Field Reference for Capture).
///
/// Roots at `<Capture>`. The auth triple (`deviceID` / `transactionKey` /
/// `developerID`) is flattened into the body just like the other flows.
/// `transactionID` references the prior Auth's `<transactionID>`.
///
/// `seqNumber` / `paymentCount` are reserved for multi-clearing
/// (split-shipment / partial captures against a single auth). PR-1 leaves them
/// as `None`; a follow-up via `add-connector-flow` will wire them up.
#[derive(Debug, Serialize)]
#[serde(rename = "Capture")]
pub struct TsysXmlCaptureRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    // TransIT XSD: transactionAmount before transactionID for Capture/Void/Return.
    // Verified live against responseCode F9901.
    #[serde(rename = "transactionAmount")]
    pub transaction_amount: StringMajorUnit,
    #[serde(rename = "salesTax", skip_serializing_if = "Option::is_none")]
    pub sales_tax: Option<StringMajorUnit>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    /// Multi-clearing sequence number (1-based). Stubbed `None` for PR-1.
    #[serde(rename = "seqNumber", skip_serializing_if = "Option::is_none")]
    pub seq_number: Option<u32>,
    /// Total expected capture count for this auth. Stubbed `None` for PR-1.
    #[serde(rename = "paymentCount", skip_serializing_if = "Option::is_none")]
    pub payment_count: Option<u32>,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysXmlCaptureRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Capture")
    }
}

/// TransIT Return (Refund) request (tech spec § Return / Field Reference for Return).
///
/// Roots at `<Return>`. TransIT supports three modes from the same element shape:
///
/// 1. **Referenced full**: `transactionID` populated, no `transactionAmount` →
///    refunds the full captured amount. (PR-1 still emits `transactionAmount`
///    for explicitness; "omit for full" is a follow-up TODO.)
/// 2. **Referenced partial**: `transactionID` + `transactionAmount` (less than
///    the original).
/// 3. **Unreferenced** ("Return WITHOUT Reference"): NO `transactionID`; raw
///    card data (`cardNumber`, `expirationDate`, `cardDataSource`) +
///    `transactionAmount` instead.
///
/// All discriminator fields are `Option<>` and `skip_serializing_if`-gated so a
/// single struct can serialize any of the three layouts.
#[derive(Debug, Serialize)]
#[serde(rename = "Return")]
pub struct TsysXmlReturnRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    /// Origin of card data — only sent for unreferenced refunds.
    #[serde(rename = "cardDataSource", skip_serializing_if = "Option::is_none")]
    pub card_data_source: Option<TsysXmlCardDataSource>,
    /// Refund amount in major units. Always emitted in PR-1; "omit for full
    /// referenced refunds" is a TODO follow-up.
    /// TransIT XSD requires transactionAmount BEFORE transactionID.
    #[serde(rename = "transactionAmount", skip_serializing_if = "Option::is_none")]
    pub transaction_amount: Option<StringMajorUnit>,
    /// Reference to the original capture's `<transactionID>`. Present for
    /// referenced refunds; absent for unreferenced refunds.
    #[serde(rename = "transactionID", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    /// PAN — only present for unreferenced refunds.
    #[serde(rename = "cardNumber", skip_serializing_if = "Option::is_none")]
    pub card_number: Option<Secret<String>>,
    /// MM/YY — only present for unreferenced refunds.
    #[serde(rename = "expirationDate", skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<Secret<String>>,
    /// CVV — optional even within the unreferenced mode (not all card types
    /// require it).
    #[serde(rename = "cvv2", skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysXmlReturnRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Return")
    }
}

/// TransIT Void request (tech spec § Void / Field Reference for Void).
///
/// Roots at `<Void>`. The auth triple (`deviceID` / `transactionKey` /
/// `developerID`) is flattened into the body just like the other flows.
/// `transactionID` references the prior Auth/Capture's `<transactionID>`.
///
/// `transactionAmount` is OPTIONAL — omit for a full void; include for a
/// partial void (cert script Step 7).
#[derive(Debug, Serialize)]
#[serde(rename = "Void")]
pub struct TsysXmlVoidRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    /// Optional — present for a partial void, omitted for a full void.
    /// TransIT XSD requires transactionAmount BEFORE transactionID.
    #[serde(rename = "transactionAmount", skip_serializing_if = "Option::is_none")]
    pub transaction_amount: Option<StringMajorUnit>,
    #[serde(rename = "transactionID")]
    pub transaction_id: String,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    /// MUST come AFTER developerID (TransIT XSD verified live — voidReason is the
    /// last element in the Void sequence). Derived from `cancellation_reason`,
    /// capped at 80 chars. Defaults to `POST_AUTH_USER_DECLINE` — the only enum
    /// value we've found accepted by TSYS' XSD validator.
    #[serde(rename = "voidReason")]
    pub void_reason: String,
}

impl GetSoapXml for TsysXmlVoidRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "Void")
    }
}

// =============================================================================
// AddCustomer — CreateConnectorCustomer flow
// =============================================================================

/// `<personalDetails>` block for `<AddCustomer>`. TransIT requires firstName +
/// lastName (we split on first whitespace; if no whitespace, lastName is `"-"`).
#[derive(Debug, Serialize)]
#[serde(rename = "personalDetails")]
pub struct TsysXmlPersonalDetails {
    #[serde(rename = "firstName")]
    pub first_name: Secret<String>,
    #[serde(rename = "lastName")]
    pub last_name: Secret<String>,
    #[serde(rename = "addressLine1")]
    pub address_line1: Secret<String>,
    #[serde(rename = "zip")]
    pub zip: Secret<String>,
}

/// Card data inside `<walletDetails>` of `<AddCustomer>`. Note the
/// `expirationDate` format here is `MMYYYY` (6 digits) — different from
/// Sale/Auth which uses `MMYY`.
#[derive(Debug, Serialize)]
#[serde(rename = "cardDetails")]
pub struct TsysXmlAddCustomerCardDetails {
    #[serde(rename = "cardNumber")]
    pub card_number: Secret<String>,
    /// `MMYYYY` (6 digits) — see tech spec note.
    #[serde(rename = "expirationDate")]
    pub expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename = "walletDetails")]
pub struct TsysXmlAddCustomerWalletDetails {
    #[serde(rename = "cardDetails")]
    pub card_details: TsysXmlAddCustomerCardDetails,
    #[serde(rename = "addressLine1")]
    pub address_line1: Secret<String>,
    #[serde(rename = "zip")]
    pub zip: Secret<String>,
    /// `1` for the primary card on the new customer wallet.
    #[serde(rename = "paymentSequence")]
    pub payment_sequence: String,
}

/// TransIT `<AddCustomer>` request (CreateConnectorCustomer flow). The wallet
/// block holds the first card we want to associate with the new customer.
#[derive(Debug, Serialize)]
#[serde(rename = "AddCustomer")]
pub struct TsysXmlAddCustomerRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "personalDetails")]
    pub personal_details: TsysXmlPersonalDetails,
    #[serde(rename = "walletDetails")]
    pub wallet_details: TsysXmlAddCustomerWalletDetails,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysXmlAddCustomerRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "AddCustomer")
    }
}

// =============================================================================
// CardAuthentication — SetupMandate flow (zero-dollar CIT verify)
// =============================================================================

/// TransIT `<CardAuthentication>` request — zero-dollar CIT card verification
/// used by the SetupMandate flow. Mirrors the Sale/Auth terminalData fields
/// plus `<cardOnFile>Y</cardOnFile>` to flag CIT consent.
#[derive(Debug, Serialize)]
#[serde(rename = "CardAuthentication")]
pub struct TsysXmlCardAuthenticationRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "cardDataSource")]
    pub card_data_source: TsysXmlCardDataSource,
    #[serde(rename = "cardNumber")]
    pub card_number: Secret<String>,
    /// MM/YY (matches Sale/Auth format) — TransIT XSD-aligned per tech spec.
    #[serde(rename = "expirationDate")]
    pub expiration_date: Secret<String>,
    #[serde(rename = "addressLine1")]
    pub address_line1: Secret<String>,
    #[serde(rename = "zip")]
    pub zip: Secret<String>,
    #[serde(rename = "externalReferenceID")]
    pub external_reference_id: String,
    #[serde(rename = "cardOnFile")]
    pub card_on_file: TsysXmlCardOnFile,
    /// `<citStatusIndicator>` — MC CIT only. C102 (Standing Order intent) /
    /// C103 (Subscription intent) / C104 (Installment intent). Driven by the
    /// `recurring.mc_cit_status_indicator` metadata field.
    #[serde(rename = "citStatusIndicator", skip_serializing_if = "Option::is_none")]
    pub cit_status_indicator: Option<TsysXmlMcCitStatusIndicator>,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
    // terminalData (flat per XSD; same flattening as Sale/Auth)
    #[serde(rename = "terminalCapability")]
    pub terminal_capability: TsysXmlTerminalCapability,
    #[serde(rename = "terminalOperatingEnvironment")]
    pub terminal_operating_environment: TsysXmlTerminalOperatingEnvironment,
    #[serde(rename = "cardholderAuthenticationMethod")]
    pub cardholder_authentication_method: TsysXmlCardholderAuthenticationMethod,
    #[serde(rename = "terminalAuthenticationCapability")]
    pub terminal_authentication_capability: TsysXmlTerminalAuthenticationCapability,
    #[serde(rename = "terminalOutputCapability")]
    pub terminal_output_capability: TsysXmlTerminalOutputCapability,
    #[serde(rename = "maxPinLength")]
    pub max_pin_length: TsysXmlMaxPinLength,
    #[serde(rename = "terminalCardCaptureCapability")]
    pub terminal_card_capture_capability: TsysXmlTerminalCardCaptureCapability,
    #[serde(rename = "cardholderPresentDetail")]
    pub cardholder_present_detail: TsysXmlCardholderPresentDetail,
    #[serde(rename = "cardPresentDetail")]
    pub card_present_detail: TsysXmlCardPresentDetail,
    #[serde(rename = "cardDataInputMode")]
    pub card_data_input_mode: TsysXmlCardDataInputMode,
    #[serde(rename = "cardholderAuthenticationEntity")]
    pub cardholder_authentication_entity: TsysXmlCardholderAuthenticationEntity,
    #[serde(rename = "cardDataOutputCapability")]
    pub card_data_output_capability: TsysXmlCardDataOutputCapability,
}

impl GetSoapXml for TsysXmlCardAuthenticationRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "CardAuthentication")
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use domain_types::payment_method_data::DefaultPCIHolder;

    use super::*;

    fn major(value: &str) -> StringMajorUnit {
        serde_json::from_str(&format!("\"{value}\"")).expect("valid string major unit")
    }

    fn sample_level3_sale_body() -> TsysXmlAuthorizeBody<DefaultPCIHolder> {
        TsysXmlAuthorizeBody {
            device_id: Secret::new("device_123".to_string()),
            transaction_key: Secret::new("txn_key_123".to_string()),
            card_data_source: TsysXmlCardDataSource::Internet,
            transaction_amount: major("109.00"),
            sales_tax: Some(major("9.00")),
            additional_tax_details: vec![TsysXmlAdditionalTaxDetails {
                tax_type: "VAT".to_string(),
                tax_amount: major("9.00"),
                tax_rate: Some("8.25".to_string()),
                tax_category: Some("VAT".to_string()),
            }],
            shipping_charges: Some(major("5.00")),
            duty_charges: Some(major("0.00")),
            card_number: Some(Secret::new("4111111111111111".to_string())),
            expiration_date: Some(Secret::new("12/30".to_string())),
            cvv2: Some(Secret::new("123".to_string())),
            customer_code: None,
            wallet_details: None,
            card_on_file_transaction_identifier: None,
            previous_network_transaction_id: None,
            cit_status_indicator: None,
            mit_status_indicator: None,
            address_line1: Secret::new("123 Test St".to_string()),
            zip: Secret::new("30301".to_string()),
            external_reference_id: "ext_ref_123".to_string(),
            product_details: vec![TsysXmlProductDetails {
                product_code: "SKU-123".to_string(),
                product_name: "Widget".to_string(),
                price: major("100.00"),
                quantity: 1,
                measurement_unit: Some("EA".to_string()),
                product_discount_details: Some(TsysXmlProductDiscountDetails {
                    product_discount_name: "Line Item Discount".to_string(),
                    product_discount_amount: major("0.00"),
                    product_discount_percentage: None,
                    product_discount_type: "DISCOUNT".to_string(),
                    priority: 1,
                    stackable: TsysXmlYesNo::No,
                }),
                product_tax_details: Some(TsysXmlProductTaxDetails {
                    product_tax_name: Some("TAX".to_string()),
                    product_tax_amount: Some(major("9.00")),
                    product_tax_percentage: Some("8.25".to_string()),
                    product_tax_type: Some("VAT".to_string()),
                }),
                product_variation: Some("Standard".to_string()),
                product_modifier_details: Some(TsysXmlProductModifierDetails {
                    modifier_name: "Brand".to_string(),
                    modifier_value: Some("Acme".to_string()),
                    modifier_price: None,
                }),
                product_notes: Some("Level III product detail".to_string()),
                product_discount_indicator: Some(TsysXmlProductDiscountIndicator::N),
                product_commodity_code: Some("123456789012".to_string()),
            }],
            commercial_card_level: Some(TsysXmlCommercialCardLevel::Level3),
            purchase_order: Some("PO-12345".to_string()),
            charge_descriptor: Some("Descriptor".to_string()),
            charge_descriptor_2: Some("Descriptor-2".to_string()),
            charge_descriptor_3: Some("Descriptor-3".to_string()),
            charge_descriptor_4: Some("Descriptor-4".to_string()),
            customer_vat_number: Some("VAT-123".to_string()),
            customer_ref_id: Some("CUSTOMER-REF-1".to_string()),
            supplier_reference_number: Some("SUP-REF-1".to_string()),
            order_date: Some("05/27/2026".to_string()),
            summary_commodity_code: Some("123456789012".to_string()),
            vat_invoice: Some("INV-123".to_string()),
            ship_from_zip: Some("30301".to_string()),
            ship_to_zip: Some("94105".to_string()),
            destination_country_code: Some("USA".to_string()),
            card_on_file: None,
            partial_auth_support: Some("YES".to_string()),
            terminal_capability: TsysXmlTerminalCapability::KeyedEntryOnly,
            terminal_operating_environment: TsysXmlTerminalOperatingEnvironment::NoTerminal,
            cardholder_authentication_method:
                TsysXmlCardholderAuthenticationMethod::NotAuthenticated,
            terminal_authentication_capability:
                TsysXmlTerminalAuthenticationCapability::NoCapability,
            terminal_output_capability: TsysXmlTerminalOutputCapability::None,
            max_pin_length: TsysXmlMaxPinLength::NotSupported,
            terminal_card_capture_capability: TsysXmlTerminalCardCaptureCapability::NoCapability,
            cardholder_present_detail:
                TsysXmlCardholderPresentDetail::CardholderNotPresentElectronicCommerce,
            card_present_detail: TsysXmlCardPresentDetail::CardNotPresent,
            card_data_input_mode:
                TsysXmlCardDataInputMode::PanEntryElectronicCommerceIncludingRemoteChip,
            cardholder_authentication_entity:
                TsysXmlCardholderAuthenticationEntity::NotAuthenticated,
            card_data_output_capability: TsysXmlCardDataOutputCapability::None,
            developer_id: Secret::new("developer_123".to_string()),
            is_recurring: None,
            billing_type: None,
            payment_count: None,
            current_payment_count: None,
            original_recurring_amount: None,
            registered_user_indicator: None,
            last_registered_change_date: None,
            authorization_indicator: None,
            acceptor_street_address: None,
            acceptor_customer_service_phone_number: None,
            acceptor_phone_number: None,
            acceptor_url_address: None,
            mit: None,
            _marker: PhantomData,
        }
    }

    #[test]
    fn level3_sale_serializes_required_commercial_card_nodes() {
        let xml = TsysXmlAuthorizeRequest::Sale(sample_level3_sale_body()).to_soap_xml();

        for required_tag in [
            "<commercialCardLevel>LEVEL3</commercialCardLevel>",
            "<salesTax>9.00</salesTax>",
            "<purchaseOrder>PO-12345</purchaseOrder>",
            "<customerVATNumber>VAT-123</customerVATNumber>",
            "<additionalTaxDetails><taxType>VAT</taxType><taxAmount>9.00</taxAmount><taxRate>8.25</taxRate><taxCategory>VAT</taxCategory></additionalTaxDetails>",
            "<shippingCharges>5.00</shippingCharges>",
            "<dutyCharges>0.00</dutyCharges>",
            "<productCode>SKU-123</productCode>",
            "<productName>Widget</productName>",
            "<price>100.00</price>",
            "<quantity>1</quantity>",
            "<measurementUnit>EA</measurementUnit>",
            "<productDiscountDetails><productDiscountName>Line Item Discount</productDiscountName><productDiscountAmount>0.00</productDiscountAmount><productDiscountPercentage>0</productDiscountPercentage><productDiscountType>DISCOUNT</productDiscountType><priority>1</priority><stackable>NO</stackable></productDiscountDetails>",
            "<productTaxDetails><productTaxName>TAX</productTaxName><productTaxAmount>9.00</productTaxAmount><productTaxPercentage>8.25</productTaxPercentage><productTaxType>VAT</productTaxType></productTaxDetails>",
            "<productCommodityCode>123456789012</productCommodityCode>",
            "<orderDate>05/27/2026</orderDate>",
            "<summaryCommodityCode>123456789012</summaryCommodityCode>",
            "<vatInvoice>INV-123</vatInvoice>",
            "<shipFromZip>30301</shipFromZip>",
            "<shipToZip>94105</shipToZip>",
            "<destinationCountryCode>USA</destinationCountryCode>",
        ] {
            assert!(
                xml.contains(required_tag),
                "expected XML to contain `{required_tag}`, got: {xml}"
            );
        }
    }

    #[test]
    fn capture_serializes_sales_tax_between_amount_and_transaction_id() {
        let xml = TsysXmlCaptureRequest {
            device_id: Secret::new("device_123".to_string()),
            transaction_key: Secret::new("txn_key_123".to_string()),
            transaction_amount: major("109.00"),
            sales_tax: Some(major("9.00")),
            transaction_id: "txn_123".to_string(),
            seq_number: None,
            payment_count: None,
            developer_id: Secret::new("developer_123".to_string()),
        }
        .to_soap_xml();

        let amount_idx = xml
            .find("<transactionAmount>109.00</transactionAmount>")
            .expect("transaction amount tag");
        let tax_idx = xml
            .find("<salesTax>9.00</salesTax>")
            .expect("sales tax tag");
        let transaction_id_idx = xml
            .find("<transactionID>txn_123</transactionID>")
            .expect("transaction id tag");

        assert!(
            amount_idx < tax_idx && tax_idx < transaction_id_idx,
            "{xml}"
        );
    }
}
