use std::fmt::Debug;

use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, FutureUsage, MitCategory, PaymentChannel,
    RefundStatus,
};
use common_utils::types::StringMajorUnit;
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateIds, MandateReference,
        MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RecurringMandatePaymentData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        Card, CardDetailsForNetworkTransactionId, PaymentMethodData, PaymentMethodDataTypes,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    utils::split_full_name as split_domain_full_name,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{super::macros::GetSoapXml, TsysTransitRouterData};
use crate::types::ResponseRouterData;

// =============================================================================
// TSYS XML request and response models
// =============================================================================

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransitCardDataSource {
    Phone,
    Internet,
    Manual,
    Recurring,
    Mail,
}

// =============================================================================
// TerminalData group — XSD-driven enums for the e-commerce cert script.
//
// Most enums use `SCREAMING_SNAKE_CASE`; explicit `rename` stays only where
// the wire value cannot be derived from the Rust variant name.
//
// `Deserialize` is derived on each enum so the connector metadata override
// can parse straight into these types from either:
// - flat metadata fields (`metadata.terminal_capability`)
// - legacy nested fields (`metadata.tsys_transit.terminal_data.terminal_capability`)
// =============================================================================

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
    #[serde(
        rename = "ELECTRONIC_COMMERCE_NO_SECURITY_CHANNEL_ENCRYPTED_SET_WITHOUT_CARDHOLDER_CERTIFICATE"
    )]
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

/// MC/AMEX-only field. PREAUTH for manual capture (delayed funds), FINAL for
/// auto-capture (Sale).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitAuthorizationIndicator {
    Preauth,
    Final,
}

/// `<cardOnFile>` flag — `Y` only for Visa credential-on-file CIT/MIT use.
/// Two-variant enum keeps the wire contract explicit per tech spec § CIT/MIT.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitCardOnFile {
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
/// - `U` — Discover unscheduled card-on-file MIT
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

/// `<isRecurring>` — emitted as `Y` on every recurring/installment Step 5/6
/// row per the cert script. Treated as Option<_> on the wire (skip when absent)
/// because non-recurring flows still use the same XML body.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitIsRecurring {
    Y,
}

/// `<billingType>` — only present on installment rows (cert Step 6).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitBillingType {
    Installment,
}

/// Commercial-card enhanced-data level.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitCommercialCardLevel {
    #[serde(rename = "LEVEL2")]
    Level2,
    #[serde(rename = "LEVEL3")]
    Level3,
}

/// `<citStatusIndicator>` — MasterCard CIT only:
/// `C101` generic card-on-file / `C102` Standing Order intent /
/// `C103` Subscription intent / `C104` Installment intent.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitMcCitStatusIndicator {
    C101,
    C102,
    C103,
    C104,
}

/// `<mit>` wrapper carrying the MIT indicator value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "mit")]
pub struct TsysTransitMit {
    #[serde(rename = "mitIndicator")]
    pub mit_indicator: TsysTransitMitIndicator,
}

/// Vault wallet details — emitted on Path B MIT (and CreateConnectorCustomer
/// response shape). The `<walletDetails><walletID>...</walletID></walletDetails>`
/// structure replaces PAN/expiry/cvv2 on Path B Authorize calls.
#[derive(Debug, Clone, Serialize)]
#[serde(rename = "walletDetails")]
pub struct TsysTransitWalletDetailsRef {
    #[serde(rename = "walletID")]
    pub wallet_id: Secret<String>,
}

/// Order-level tax addendum used by Level 3 Visa/MasterCard requests.
#[derive(Debug, Clone, Serialize)]
pub struct TsysTransitAdditionalTaxDetails {
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
pub struct TsysTransitProductTaxDetails {
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
pub struct TsysTransitProductDiscountDetails {
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
    pub stackable: TsysTransitYesNo,
}

/// Per-line modifier block nested under `<productDetails>`.
#[derive(Debug, Clone, Serialize)]
pub struct TsysTransitProductModifierDetails {
    #[serde(rename = "modifierName")]
    pub modifier_name: String,
    #[serde(rename = "modifierValue", skip_serializing_if = "Option::is_none")]
    pub modifier_value: Option<String>,
    #[serde(rename = "modifierPrice", skip_serializing_if = "Option::is_none")]
    pub modifier_price: Option<StringMajorUnit>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TsysTransitProductDiscountIndicator {
    #[serde(rename = "Y")]
    Y,
    #[serde(rename = "N")]
    N,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitYesNo {
    Yes,
    No,
}

/// Level 3 line-item detail.
#[derive(Debug, Clone, Serialize)]
pub struct TsysTransitProductDetails {
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
    pub product_discount_details: Option<TsysTransitProductDiscountDetails>,
    #[serde(rename = "productTaxDetails", skip_serializing_if = "Option::is_none")]
    pub product_tax_details: Option<TsysTransitProductTaxDetails>,
    #[serde(rename = "productVariation", skip_serializing_if = "Option::is_none")]
    pub product_variation: Option<String>,
    #[serde(
        rename = "productModifierDetails",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_modifier_details: Option<TsysTransitProductModifierDetails>,
    #[serde(rename = "productNotes", skip_serializing_if = "Option::is_none")]
    pub product_notes: Option<String>,
    #[serde(
        rename = "productDiscountIndicator",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_discount_indicator: Option<TsysTransitProductDiscountIndicator>,
    #[serde(
        rename = "productCommodityCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub product_commodity_code: Option<String>,
}

/// Discover/JCB/Diners/CUP-only signal indicating whether the cardholder is a
/// registered user in the merchant's system.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsysTransitRegisteredUserIndicator {
    Yes,
    No,
}

/// XSD `terminalData` group — required by the TransIT e-commerce certification
/// script for every authorization. The 12 inner fields are all required.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
#[serde(rename = "terminalData")]
pub struct TsysTransitTerminalData {
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
}

/// XSD `developerInfo` wrapper. Cert script asks for the developerID to be
/// nested under a `<developerInfo>` element on the Authorize flow.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
#[serde(rename = "developerInfo")]
pub struct TsysTransitDeveloperInfo {
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
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
        raw_request = %xml,
        "tsysTransit raw connector request"
    );
    xml
}

/// TransIT Sale / Auth request.
///
/// Both `<Sale>` and `<Auth>` share the same field schema (tech spec § 1, § 2). We
/// flip the root element via a tagged enum so callers can pick at runtime based on
/// `auto_capture`.
#[derive(Debug, Serialize)]
pub enum TsysTransitAuthorizeRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "Sale")]
    Sale(TsysTransitAuthorizeBody<T>),
    #[serde(rename = "Auth")]
    Auth(TsysTransitAuthorizeBody<T>),
}

/// RepeatPayment (MIT) request — TransIT does not expose a separate recurring
/// endpoint, so we replay the same `<Sale>` / `<Auth>` shape. This newtype
/// exists purely so the macro-generated `Templating` registration is distinct
/// from the Authorize flow's; the wire body is identical.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TsysTransitRepeatPaymentRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(pub TsysTransitAuthorizeRequest<T>);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> GetSoapXml
    for TsysTransitRepeatPaymentRequest<T>
{
    fn to_soap_xml(&self) -> String {
        self.0.to_soap_xml()
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> GetSoapXml
    for TsysTransitAuthorizeRequest<T>
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
// (`terminalCapability`, `developerID`, etc.) is a FLAT sibling. Verified
// against the F9901 error pasted into the design doc.
// `partialApprovalCapable` is similarly bogus — the real element is
// `partialAuthSupport`.
#[derive(Debug, Serialize)]
pub struct TsysTransitAuthorizeBody<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "cardDataSource")]
    pub card_data_source: TsysTransitCardDataSource,
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
    pub additional_tax_details: Vec<TsysTransitAdditionalTaxDetails>,
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
    pub wallet_details: Option<TsysTransitWalletDetailsRef>,
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
    pub cit_status_indicator: Option<TsysTransitMcCitStatusIndicator>,
    /// Public recurring samples use `<mitStatusIndicator>` for both
    /// Discover-family (`R` / `S` / `T`) and MasterCard (`M102` / `M103` /
    /// `M104`) MIT flows.
    #[serde(rename = "mitStatusIndicator", skip_serializing_if = "Option::is_none")]
    pub mit_status_indicator: Option<TsysTransitMitIndicator>,
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
    pub product_details: Vec<TsysTransitProductDetails>,
    /// Commercial-card qualifier. Emitted only when the merchant explicitly
    /// opts in via `metadata.tsys_transit.commercial_card`.
    #[serde(
        rename = "commercialCardLevel",
        skip_serializing_if = "Option::is_none"
    )]
    pub commercial_card_level: Option<TsysTransitCommercialCardLevel>,
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
    /// `<cardOnFile>` — emitted as `Y` only for Visa credential-on-file CIT/MIT
    /// use per TSYS cert guidance.
    ///
    /// Position note: keep this before `partialAuthSupport`, the conventional
    /// COF/MIT slot for TSYS card-on-file APIs. CIT (Step 4) verified PASS in
    /// this slot.
    #[serde(rename = "cardOnFile", skip_serializing_if = "Option::is_none")]
    pub card_on_file: Option<TsysTransitCardOnFile>,
    /// Generic keyed Auth/Sale samples emit this after `<cardOnFile>`, but the
    /// recurring keyed samples omit it. Keep it optional so MIT flows can match
    /// the published recurring examples exactly.
    #[serde(rename = "partialAuthSupport", skip_serializing_if = "Option::is_none")]
    pub partial_auth_support: Option<String>,
    // --- terminalData fields (flat per the XSD; dev portal groups them, XSD doesn't) ---
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
    pub is_recurring: Option<TsysTransitIsRecurring>,
    /// `<billingType>` — `INSTALLMENT` for Step 6 rows.
    #[serde(rename = "billingType", skip_serializing_if = "Option::is_none")]
    pub billing_type: Option<TsysTransitBillingType>,
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
    pub registered_user_indicator: Option<TsysTransitRegisteredUserIndicator>,
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
    pub authorization_indicator: Option<TsysTransitAuthorizationIndicator>,
    /// Legacy `<mit>` wrapper. The public recurring samples do not use this
    /// block; it is retained only for older non-recurring stored-credential
    /// paths that still rely on the TransIT field.
    #[serde(rename = "mit", skip_serializing_if = "Option::is_none")]
    pub mit: Option<TsysTransitMit>,
    /// Phantom marker so the generic `T` is preserved on the struct without leaking
    /// into the serialized payload.
    #[serde(skip)]
    pub _marker: std::marker::PhantomData<T>,
}

/// TransIT Transaction Inquiry (PSync) request.
///
/// TODO(tsys_transit): UNDECIDED - confirm element name with TSYS.
/// The spec lists `<TransactionInquiry>` as the most likely candidate with
/// `<GetDetails>` as alternative.
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

/// RSync request — reuses the PSync `<TransactionInquiry>` shape via a type
/// alias. TransIT exposes a single inquiry endpoint for both payment and
/// refund status lookups; the type alias keeps the macro layer's Templating
/// types distinct without duplicating wire-level schema.
pub type TsysTransitRSyncRequest = TsysTransitTransactionInquiryRequest;

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
pub struct TsysTransitCaptureRequest {
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

impl GetSoapXml for TsysTransitCaptureRequest {
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
pub struct TsysTransitReturnRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    /// Origin of card data — only sent for unreferenced refunds.
    #[serde(rename = "cardDataSource", skip_serializing_if = "Option::is_none")]
    pub card_data_source: Option<TsysTransitCardDataSource>,
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

impl GetSoapXml for TsysTransitReturnRequest {
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
pub struct TsysTransitVoidRequest {
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

impl GetSoapXml for TsysTransitVoidRequest {
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
pub struct TsysTransitPersonalDetails {
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
pub struct TsysTransitAddCustomerCardDetails {
    #[serde(rename = "cardNumber")]
    pub card_number: Secret<String>,
    /// `MMYYYY` (6 digits) — see tech spec note.
    #[serde(rename = "expirationDate")]
    pub expiration_date: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename = "walletDetails")]
pub struct TsysTransitAddCustomerWalletDetails {
    #[serde(rename = "cardDetails")]
    pub card_details: TsysTransitAddCustomerCardDetails,
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
pub struct TsysTransitAddCustomerRequest {
    #[serde(rename = "deviceID")]
    pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")]
    pub transaction_key: Secret<String>,
    #[serde(rename = "personalDetails")]
    pub personal_details: TsysTransitPersonalDetails,
    #[serde(rename = "walletDetails")]
    pub wallet_details: TsysTransitAddCustomerWalletDetails,
    #[serde(rename = "developerID")]
    pub developer_id: Secret<String>,
}

impl GetSoapXml for TsysTransitAddCustomerRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "AddCustomer")
    }
}

// =============================================================================
// CardAuthentication — SetupMandate flow (zero-dollar CIT verify)
// =============================================================================

/// TransIT `<CardAuthentication>` request — zero-dollar CIT card verification
/// used by the SetupMandate flow. Mirrors the Sale/Auth terminalData fields
/// and emits `<cardOnFile>Y</cardOnFile>` only for Visa CIT consent.
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
    /// MM/YY (matches Sale/Auth format) — TransIT XSD-aligned per tech spec.
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
    // terminalData (flat per XSD; same flattening as Sale/Auth)
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
    /// `<citStatusIndicator>` — MC CIT only. C102 (Standing Order intent) /
    /// C103 (Subscription intent) / C104 (Installment intent). Driven by the
    /// `recurring.mc_cit_status_indicator` metadata field.
    #[serde(rename = "citStatusIndicator", skip_serializing_if = "Option::is_none")]
    pub cit_status_indicator: Option<TsysTransitMcCitStatusIndicator>,
}

impl GetSoapXml for TsysTransitCardAuthenticationRequest {
    fn to_soap_xml(&self) -> String {
        generate_logged_xml(self, "CardAuthentication")
    }
}

/// Top-level TransIT status flag (tech spec § Status Mappings).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransitStatus {
    Pass,
    Fail,
}

/// Authorize response envelope — covers both `<SaleResponse>` and `<AuthResponse>`.
///
/// quick_xml does not natively project two root names onto a single struct via
/// `#[serde(untagged)]` on a struct; instead we accept either root via an enum
/// wrapper, then merge into the same body shape (their field schemas are
/// identical per tech spec § 2).
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

/// RepeatPayment response — identical wire shape to `TsysTransitAuthorizeResponse`.
/// The newtype keeps the macro-generated `Templating` registration distinct
/// from the Authorize flow so the same response body can be consumed by both
/// flows without conflicting `BridgeRequestResponse` impls.
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

/// Shared body for `<SaleResponse>` / `<AuthResponse>` (tech spec § Sale/Auth response).
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
    pub processed_amount: Option<String>,
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

/// Lifecycle state of a transaction as reported by TransIT (tech spec § PSync).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TsysTransitTransactionState {
    Authorized,
    Captured,
    Settled,
    Voided,
    Returned,
}

/// TransIT Capture response (tech spec § Capture response).
///
/// Roots at `<CaptureResponse>`. Status mapping per tech spec § Status Mappings
/// is handled in the transformer (`map_capture_status`).
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

/// TransIT Return (Refund) response (tech spec § Return response).
///
/// Roots at `<ReturnResponse>`. Status mapping per tech spec § Status Mappings
/// is handled in the transformer (`map_refund_status`).
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

/// TransIT Void response (tech spec § Void response).
///
/// Roots at `<VoidResponse>`. Status mapping per tech spec § Status Mappings
/// is handled in the transformer (`map_void_status`).
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

/// `<walletDetails>` block on `<AddCustomerResponse>` — exposes the walletID
/// that we stash alongside the customerCode to drive Path B Authorize.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct TsysTransitAddCustomerResponseWalletDetails {
    #[serde(rename = "walletID", default)]
    pub wallet_id: Option<String>,
    #[serde(rename = "maskedCardNumber", default)]
    pub masked_card_number: Option<String>,
    #[serde(rename = "externalWalletReferenceID", default)]
    pub external_wallet_reference_id: Option<String>,
}

/// TransIT `<AddCustomerResponse>` — CreateConnectorCustomer response.
/// We surface `customerCode` as `connector_customer_id` and stash the
/// `walletID` on `PaymentFlowData.connector_response_reference_id` so a
/// downstream Authorize can encode the Path B `cust:CCC:WWW` mandate id.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename = "AddCustomerResponse")]
pub struct TsysTransitAddCustomerResponse {
    #[serde(rename = "status", default)]
    pub status: Option<TsysTransitStatus>,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default)]
    pub response_message: Option<String>,
    #[serde(rename = "customerCode", default)]
    pub customer_code: Option<String>,
    #[serde(rename = "walletDetails", default)]
    pub wallet_details: Option<TsysTransitAddCustomerResponseWalletDetails>,
}

/// TransIT `<CardAuthenticationResponse>` — SetupMandate response. Mirrors
/// the Sale/Auth response shape (PASS/FAIL + responseCode + transactionID),
/// plus `cardTransactionIdentifier` which we use as the mandate's
/// network-token id (NTID) for Path A.
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

/// RSync response — reuses the PSync inquiry response shape via a type alias.
/// TransIT's `<TransactionInquiry>` endpoint serves both payment and refund
/// status lookups; the alias keeps the macro layer's Templating types
/// distinct from PSync while sharing the same on-wire schema. The transformer
/// (`map_rsync_status`) interprets the same `<transactionState>` differently
/// for refunds.
pub type TsysTransitRSyncResponse = TsysTransitTransactionInquiryResponse;

/// PSync response envelope.
///
/// TODO(tsys_transit): UNDECIDED - confirm element name with TSYS once API
/// behaviour is validated end-to-end.
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

// =============================================================================
// Connector metadata schema (parsed from `PaymentsAuthorizeData.metadata`)
// =============================================================================

/// Connector metadata accepted by TSYS XML.
///
/// Preferred shape for terminal-data overrides is flat:
/// `{ "terminal_capability": "KEYED_ENTRY_ONLY", ... }`.
///
/// The legacy nested shape remains supported:
/// `{ "tsys_transit": { "terminal_data": { ... } } }`.
#[derive(Debug, Default, Deserialize, Clone)]
struct TsysTransitMerchantMetadata {
    #[serde(default)]
    tsys_transit: Option<TsysTransitMerchantMetadataInner>,
    #[serde(default)]
    terminal_data: Option<TsysTransitTerminalDataOverrides>,
    #[serde(default)]
    commercial_card: Option<TsysTransitCommercialCardMetadata>,
    #[serde(default, alias = "sales_tax", alias = "salesTax")]
    order_tax_amount: Option<i64>,
    #[serde(default, flatten)]
    terminal_overrides: TsysTransitTerminalDataOverrides,
}

impl TsysTransitMerchantMetadata {
    fn into_inner(self) -> TsysTransitMerchantMetadataInner {
        let mut inner = self.tsys_transit.unwrap_or_default();

        if self.commercial_card.is_some() {
            inner.commercial_card = self.commercial_card;
        }
        if self.order_tax_amount.is_some() {
            inner.order_tax_amount = self.order_tax_amount;
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
    #[serde(default, alias = "sales_tax", alias = "salesTax")]
    order_tax_amount: Option<i64>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct TsysTransitCommercialCardMetadata {
    charge_descriptor_2: Option<String>,
    charge_descriptor_3: Option<String>,
    charge_descriptor_4: Option<String>,
    vat_invoice: Option<String>,
    ship_from_zip: Option<String>,
}

/// Mandate-level metadata carried via `RecurringMandatePaymentData.mandate_metadata`.
///
/// Everything that cert needs but HS has no native field for. Only consulted
/// inside `compute_recurring_context()`.
#[derive(Debug, Default, Deserialize, Clone)]
struct TsysTransitMandateMetadata {
    /// Total installment payments. Required when `mit_category == Installment`.
    #[serde(default)]
    payment_count: Option<u32>,
    /// Which payment in the installment series.
    /// Required when `mit_category == Installment`.
    #[serde(default)]
    current_payment_count: Option<u32>,
    /// MC Recurring sub-discriminator: `"standing"` (default → C102 / M102) or
    /// `"subscription"` (→ C103 / M103). HS's `MitCategory::Recurring`
    /// collapses both; this lets cert tests pick the right MC intent code.
    #[serde(default)]
    mc_subtype: Option<String>,
    /// Disc/JCB/Diners/CUP Installment `<mitIndicator>` override: `"s"`
    /// (default) or `"t"`.
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

/// Resolved Recurring/Installment context for a single Authorize / Setup call.
///
/// Built by `compute_recurring_context()` from `metadata.tsys_transit.recurring`.
/// Carries everything the downstream body-builders need — string-typed
/// metadata values are parsed into the strongly-typed enums here so the
/// transformer body sites stay free of `match` / `parse` plumbing.
#[derive(Debug, Default, Clone)]
struct RecurringContext {
    /// True when the merchant supplied `metadata.tsys_transit.recurring`.
    /// Drives terminalData preset switching and `<cvv2>` suppression.
    enabled: bool,
    /// `Some(Y)` when we should emit `<isRecurring>Y</isRecurring>`. Defaults
    /// to `Some(Y)` when `enabled` is true unless the merchant explicitly set
    /// `is_recurring=false`.
    is_recurring_flag: Option<TsysTransitIsRecurring>,
    /// Resolved `<billingType>`.
    billing_type: Option<TsysTransitBillingType>,
    payment_count: Option<u32>,
    current_payment_count: Option<u32>,
    /// MC CIT only (Step 4). Parsed from `recurring.mc_cit_status_indicator`.
    mc_cit_status_indicator: Option<TsysTransitMcCitStatusIndicator>,
    /// Public recurring samples emit `<mitStatusIndicator>` for MasterCard
    /// (`M102` / `M103` / `M104`) and Discover-family (`R` / `S` / `T`) MITs.
    mit_status_indicator: Option<TsysTransitMitIndicator>,
    /// Discover/JCB/Diners/CUP MIT only. Minor units — emit conversion happens
    /// at the body-build site using the connector's `StringMajorUnit` helper.
    original_recurring_amount_minor: Option<i64>,
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

/// Build a `RecurringContext` from HS-native inputs.
///
/// Drives the transformer's recurring/installment branch entirely off
/// `mit_category` (+ `recurring_mandate_payment_data` for amount/counters and
/// brand-specific cert quirks). Returns an empty (`enabled=false`) context for
/// `None | Some(Unscheduled) | Some(Resubmission)` so non-recurring callers
/// short-circuit cleanly.
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
        // Unscheduled / Resubmission / None → recurring presets do not apply.
        Some(MitCategory::Unscheduled) | Some(MitCategory::Resubmission) | None => {
            return Ok(RecurringContext::default())
        }
    };

    // `mandate_metadata` carries everything HS native fields can't express
    // (installment counters + MC standing-vs-subscription).
    let mm = match recurring_data.and_then(|d| d.mandate_metadata.as_ref()) {
        Some(raw) => serde_json::from_value::<TsysTransitMandateMetadata>(raw.peek().clone())
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "recurring_mandate_payment_data.mandate_metadata",
                context: Default::default(),
            })?,
        None => TsysTransitMandateMetadata::default(),
    };

    // Cert script: Installment Sale/Auth must carry both <paymentCount> and
    // <currentPaymentCount>. Fail closed if either is missing.
    if matches!(mit_category.as_ref(), Some(MitCategory::Installment))
        && (mm.payment_count.is_none() || mm.current_payment_count.is_none())
    {
        return Err(IntegrationError::MissingRequiredField {
            field_name:
                "recurring_mandate_payment_data.mandate_metadata.{payment_count,current_payment_count} required when mit_category=Installment",
            context: Default::default(),
        }
        .into());
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

    // MC C102/C103/C104 (CIT) and M102/M103/M104 (MIT) per cert intent codes.
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

    // <originalRecurringAmount> comes from HS-native original_payment_authorized_amount.
    let original_recurring_amount_minor = recurring_data
        .and_then(|d| d.original_payment_authorized_amount.as_ref())
        .map(|m| m.get_amount_as_i64());

    Ok(RecurringContext {
        enabled: true,
        is_recurring_flag,
        billing_type,
        payment_count: mm.payment_count,
        current_payment_count: mm.current_payment_count,
        mc_cit_status_indicator,
        mit_status_indicator: mc_mit_status_indicator.or(discover_family_mit_indicator),
        original_recurring_amount_minor,
    })
}

/// Auth bundle for TsysTransit (TransIT) — flattened into the XML request body.
///
/// TransIT does not use HTTP auth headers; instead each request carries the
/// `deviceID`, `transactionKey`, and `developerID` inline in the XML payload.
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

/// Minimal error envelope for TsysTransit.
///
/// TransIT signals failure with `<status>FAIL</status>` and supplies a
/// `<responseCode>` / `<responseMessage>` pair. The exact element layout will be
/// hardened further per-flow; this scaffold provides only what
/// `build_error_response` needs.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TsysTransitErrorResponse {
    #[serde(rename = "status", default, alias = "Status")]
    pub status: Option<String>,
    #[serde(rename = "responseCode", default, alias = "ResponseCode")]
    pub response_code: Option<String>,
    #[serde(rename = "responseMessage", default, alias = "ResponseMessage")]
    pub response_message: Option<String>,
}

// =============================================================================
// AUTHORIZE — request transformer
// =============================================================================

fn format_expiration_date(card: &Card<impl PaymentMethodDataTypes>) -> Secret<String> {
    // TransIT documents `MM/YY` (tech spec § Sale/Auth Field Reference). Normalize
    // 4-digit years down to 2 digits.
    let month = card.card_exp_month.peek().clone();
    let year_full = card.card_exp_year.peek().clone();
    let year_short = if year_full.len() == 4 {
        year_full[2..].to_string()
    } else {
        year_full
    };
    Secret::new(format!("{}/{}", month, year_short))
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
    let discount_percentage = detail.unit_discount_amount.and_then(|discount| {
        let line_amount = detail.amount.get_amount_as_i64();
        (line_amount > 0).then(|| {
            format_decimal((discount.get_amount_as_i64() as f64 / line_amount as f64) * 100.0)
        })
    });
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
            product_discount_percentage: discount_percentage,
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
        common_utils::types::MinorUnit::new(0),
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
        .or_else(|| {
            let transaction_amount = router_data.request.minor_amount.get_amount_as_i64();
            let sales_tax_amount = order_tax_amount.map(|amount| amount.get_amount_as_i64())?;
            if transaction_amount == 0 || sales_tax_amount == 0 {
                None
            } else {
                Some(format_decimal(
                    (sales_tax_amount as f64 / transaction_amount as f64) * 100.0,
                ))
            }
        })
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
    > for TsysTransitAuthorizeRequest<T>
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

        // Mandate-driven dispatch: when the upstream HS request supplies a
        // `connector_mandate_id` we recognize one of:
        //   - `cust:CCC:WWW`  → Path B (vault token MIT). Omit PAN/expiry/cvv2;
        //                       emit customerCode + walletDetails.
        //   - `ntid:XXX`      → Path A (network-token MIT). Keep PAN, emit
        //                       Visa cardOnFile + previousNetworkTransactionID + mit.
        //   - everything else → fall through to CIT / one-shot logic (PAN-bearing).
        // We split on the FIRST ':' to find the prefix so that walletIDs / NTIDs
        // containing colons still round-trip correctly.
        let mandate_dispatch = decode_mandate_dispatch(router_data.request.mandate_id.as_ref());

        // CIT signal (no prior mandate but caller intends to store creds).
        let is_cit_setup = matches!(mandate_dispatch, MandateDispatch::None)
            && (router_data.request.setup_future_usage == Some(FutureUsage::OffSession)
                || router_data.request.off_session == Some(true));

        // Path B (vault) does NOT need card data — we emit customerCode + walletID
        // instead. Every other branch (Path A / CIT / one-shot) needs card-bearing
        // data. CIT and one-shot arrive as `PaymentMethodData::Card`; Path A MIT
        // replays from HS arrive as `PaymentMethodData::CardDetailsForNetworkTransactionId`
        // (no CVV — cert forbids `<cvv2>` on recurring/installment anyway).
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
            // Vault path doesn't read card_opt / nti_card_opt — keep them as-is and
            // the downstream branch handles the customerCode/walletID emission.
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

        // Billing address fields used by AVS (addressLine1 + zip). Both REQUIRED
        // by the e-commerce certification script.
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
        // Card network drives several MC/AMEX/Discover-only fields AND the
        // brand-specific MIT/CIT indicator derivation. On Path B (vault MIT)
        // no card object is available — we skip the network-driven optional
        // fields entirely. Path A MIT (NTID) carries network via the
        // CardDetailsForNetworkTransactionId variant.
        let card_network = card
            .and_then(|c| c.card_network.clone())
            .or_else(|| nti_card_opt.and_then(|n| n.card_network.clone()));

        // Parse connector metadata. Recurring is driven natively from
        // `mit_category`; the metadata layer only carries acceptor, terminal,
        // and explicit commercial-card opt-in details.
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

        // Build recurring context from HS-native fields. Returns enabled=false
        // for non-recurring flows, so all downstream branches degrade cleanly.
        // `recurring_mandate_payment_data` lives on `PaymentFlowData` (the
        // resource_common_data side) — not on `PaymentsAuthorizeData`.
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

        // Channel-driven cardDataSource selection — replaces the previous
        // hardcoded Internet default. In recurring/installment context the
        // cert script forbids INTERNET and requires MAIL or PHONE; we default
        // to MAIL when no explicit channel is supplied.
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

        // Capture method drives MC/AMEX authorizationIndicator.
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

        // The public recurring/installment keyed samples for Discover/JCB/Diners
        // do not emit these fields on the MIT path. Keep them for non-recurring
        // flows only until TSYS cert/XSD requires otherwise.
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

        // terminalData fields — flat in the XSD. Each field is resolved as:
        //   1. explicit merchant override (`metadata.tsys_transit.terminal_data.*`)
        //   2. recurring/installment preset (cert script § Authorization Requirements
        //      for Recurring/Installments) — only when `recurring_context.enabled`
        //   3. channel-driven preset (e-commerce / MOTO)
        //   4. baseline default
        let terminal_capability = terminal_overrides
            .terminal_capability
            .unwrap_or(TsysTransitTerminalCapability::KeyedEntryOnly);
        // Recurring/installment terminalOperatingEnvironment per cert:
        //   - MC: NO_TERMINAL
        //   - all other brands: OFF_MERCHANT_PREMISES_UNATTENDED
        let terminal_operating_environment = terminal_overrides
            .terminal_operating_environment
            .unwrap_or_else(|| {
                if recurring_context.enabled {
                    match card_network {
                        Some(CardNetwork::Mastercard) => {
                            TsysTransitTerminalOperatingEnvironment::NoTerminal
                        }
                        _ => TsysTransitTerminalOperatingEnvironment::OffMerchantPremisesUnattended,
                    }
                } else {
                    TsysTransitTerminalOperatingEnvironment::NoTerminal
                }
            });
        let cardholder_authentication_method = terminal_overrides
            .cardholder_authentication_method
            .unwrap_or(TsysTransitCardholderAuthenticationMethod::NotAuthenticated);
        let terminal_authentication_capability = terminal_overrides
            .terminal_authentication_capability
            .unwrap_or(TsysTransitTerminalAuthenticationCapability::NoCapability);
        // Recurring cert requires DISPLAY_ONLY; e-com path keeps the existing
        // `None` baseline.
        let terminal_output_capability = terminal_overrides
            .terminal_output_capability
            .unwrap_or_else(|| {
                if recurring_context.enabled {
                    TsysTransitTerminalOutputCapability::DisplayOnly
                } else {
                    TsysTransitTerminalOutputCapability::None
                }
            });
        let max_pin_length = terminal_overrides
            .max_pin_length
            .unwrap_or(TsysTransitMaxPinLength::NotSupported);
        let terminal_card_capture_capability = terminal_overrides
            .terminal_card_capture_capability
            .unwrap_or(TsysTransitTerminalCardCaptureCapability::NoCapability);
        // Recurring/installment cardholderPresentDetail:
        //   - installment → CARDHOLDER_NOT_PRESENT_INSTALLMENT_TRANSACTION
        //   - recurring   → CARDHOLDER_NOT_PRESENT_RECURRING_TRANSACTION
        // MC requires the RECURRING variant on both CIT and MIT of a recurring
        // series — which falls out naturally because the merchant flips
        // `billing_type=INSTALLMENT` only for installment rows.
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

        // Stored-card CIT/MIT requires MIT_STORED_ON_FILE — overrides the
        // channel-driven default. Recurring/installment remains covered by the
        // same condition because those rows carry a decoded mandate id on MIT
        // and `is_cit_setup` on the initial CIT.
        let card_data_input_mode = terminal_overrides.card_data_input_mode.unwrap_or_else(|| {
            if recurring_context.enabled || is_stored_credential_flow {
                TsysTransitCardDataInputMode::MerchantInitiatedTransactionCardCredentialStoredOnFile
            } else {
                match channel {
                    Some(PaymentChannel::Ecommerce) | None => {
                        TsysTransitCardDataInputMode::PanEntryElectronicCommerceIncludingRemoteChip
                    }
                    _ => TsysTransitCardDataInputMode::KeyEnteredInput,
                }
            }
        });
        let cardholder_authentication_entity = terminal_overrides
            .cardholder_authentication_entity
            .unwrap_or(TsysTransitCardholderAuthenticationEntity::NotAuthenticated);
        let card_data_output_capability = terminal_overrides
            .card_data_output_capability
            .unwrap_or(TsysTransitCardDataOutputCapability::None);

        // Path-specific card-source fields: Path A / CIT / one-shot carry PAN;
        // Path B carries customerCode + walletDetails instead.
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
                // Cert recurring/installment MIT paths must not send CVV. For
                // normal authorizations, TSYS requires `<cvv2>`, so do not
                // silently produce an invalid request if an empty CVC slips
                // past HS validation.
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
                // Path A MIT replay: CardDetailsForNetworkTransactionId has card_number
                // + expiry but NO CVV (cert forbids cvv2 on recurring/installment).
                // Normalize expiry to MM/YY identically to format_expiration_date.
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
                // Unreachable — guarded above; fail closed if reached.
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "tsysTransit",
                    context: Default::default(),
                }
                .into());
            };

        // Visa cardOnFile + MIT block + COFTI / previousNetworkTransactionID —
        // driven jointly by `mandate_dispatch` and `recurring_context`.
        //
        // Field routing in recurring/MIT mode:
        //   - NTID dispatch → emit `<cardOnFileTransactionIdentifier>` for the
        //     brands whose published recurring samples carry it (Visa +
        //     Discover/JCB/Diners/CUP). MasterCard recurring uses
        //     `mitStatusIndicator` without COFTI on the public sample page.
        //   - Vault dispatch → no NTID-style field.
        //
        // TransIT Sale accepts network-transaction-id replay through
        // `<cardOnFileTransactionIdentifier>` for both recurring and
        // unscheduled COF flows. `<previousNetworkTransactionID>` is not valid
        // at this point in the Sale element order.
        let visa_card_on_file = || {
            matches!(card_network.as_ref(), Some(CardNetwork::Visa))
                .then_some(TsysTransitCardOnFile::Y)
        };

        let (
            card_on_file,
            mit_block,
            previous_network_transaction_id,
            card_on_file_transaction_identifier,
        ) = match (
            &mandate_dispatch,
            recurring_context.enabled,
            card_network.as_ref(),
        ) {
            (MandateDispatch::Ntid { .. }, true, Some(CardNetwork::Mastercard))
            | (MandateDispatch::Ntid { .. }, true, Some(CardNetwork::AmericanExpress)) => {
                (None, None, None, None)
            }
            (MandateDispatch::Ntid { ntid }, true, _) => {
                (visa_card_on_file(), None, None, Some(ntid.clone()))
            }
            (MandateDispatch::Ntid { ntid }, false, _) => {
                (visa_card_on_file(), None, None, Some(ntid.clone()))
            }
            (MandateDispatch::Vault { .. }, true, _) => (visa_card_on_file(), None, None, None),
            (MandateDispatch::Vault { .. }, false, _) => (
                visa_card_on_file(),
                Some(TsysTransitMit {
                    mit_indicator: TsysTransitMitIndicator::R,
                }),
                None,
                None,
            ),
            (MandateDispatch::None, _, _) if is_cit_setup => (
                // Visa CIT (storing the credential for future MIT) requires
                // cardOnFile=Y. Other brands omit this tag.
                visa_card_on_file(),
                None,
                None,
                None,
            ),
            (MandateDispatch::None, _, _) => (None, None, None, None),
        };

        // `<originalRecurringAmount>` — Discover/JCB/Diners/CUP MIT requirement.
        // Convert merchant-supplied minor units through the connector's amount
        // converter for wire consistency with `<transactionAmount>`.
        let original_recurring_amount = match (
            recurring_context.original_recurring_amount_minor,
            card_network.as_ref(),
            &mandate_dispatch,
        ) {
            (
                Some(minor),
                Some(CardNetwork::Discover)
                | Some(CardNetwork::JCB)
                | Some(CardNetwork::DinersClub)
                | Some(CardNetwork::UnionPay),
                MandateDispatch::Ntid { .. } | MandateDispatch::Vault { .. },
            ) => {
                use common_utils::types::MinorUnit;
                let minor_unit = MinorUnit::new(minor);
                Some(super::TsysTransitAmountConvertor::convert(
                    minor_unit,
                    router_data.request.currency,
                )?)
            }
            _ => None,
        };

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
            additional_tax_details: commercial_card_context.additional_tax_details,
            shipping_charges: commercial_card_context.shipping_charges,
            duty_charges: commercial_card_context.duty_charges,
            card_number,
            expiration_date,
            // TransIT cert "Do Not Send" CVV scenario: emit no `<cvv2>` when empty
            // (cert script row 113 — AMEX with absent CVV is still approved).
            cvv2: cvv2_opt,
            secure_code: three_ds_context.secure_code,
            security_protocol: None,
            ucaf_collection_indicator: three_ds_context.ucaf_collection_indicator,
            digital_payment_cryptogram: None,
            program_protocol: None,
            directory_server_transaction_id: three_ds_context.directory_server_transaction_id,
            eci_indicator: three_ds_context.eci_indicator,
            customer_code: customer_code_opt,
            wallet_details: wallet_details_opt,
            card_on_file_transaction_identifier,
            previous_network_transaction_id,
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
            card_on_file,
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
            mit: mit_block,
            _marker: std::marker::PhantomData,
        };

        Ok(if is_manual_capture {
            Self::Auth(body)
        } else {
            Self::Sale(body)
        })
    }
}

// =============================================================================
// Mandate dispatch helper
// =============================================================================

/// Result of decoding an upstream `connector_mandate_id` ("cust:CCC:WWW" or
/// "ntid:XXX") into a Path A / Path B / fall-through directive.
#[derive(Debug, Clone)]
enum MandateDispatch {
    /// Path B — vault token MIT. Emit customerCode + walletDetails.
    Vault {
        customer_code: String,
        wallet_id: String,
    },
    /// Path A — network-token MIT. Emit Visa cardOnFile + MIT + previousNetworkTransactionID.
    Ntid { ntid: String },
    /// No mandate id (or a mandate id we couldn't decode) — caller decides
    /// whether to treat the request as a CIT or a one-shot.
    None,
}

/// Decode `MandateIds.mandate_reference_id` into a `MandateDispatch`.
///
/// We look at the `ConnectorMandateId` variant first (this is where prior
/// CreateConnectorCustomer / SetupMandate responses encode the mandate id).
/// Falls back to `NetworkMandateId` so plain NTIDs surfaced by HS are still
/// treated as Path A.
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

    // NetworkMandateId — treat as a raw NTID (Path A) so HS-stored network
    // transaction ids still drive the MIT path.
    if let Some(MandateReferenceId::NetworkMandateId(ntid)) =
        mandate_id.mandate_reference_id.as_ref()
    {
        return MandateDispatch::Ntid { ntid: ntid.clone() };
    }

    MandateDispatch::None
}

/// Parse the prefix-encoded mandate id our CreateConnectorCustomer /
/// SetupMandate flows emit:
/// - `cust:<customerCode>:<walletID>` → Path B
/// - `ntid:<cardTransactionIdentifier>` → Path A
/// Anything else → `None` (fall through to CIT / one-shot decision).
fn decode_mandate_id_string(raw: &str) -> MandateDispatch {
    if let Some(rest) = raw.strip_prefix("cust:") {
        // splitn(2, ':') so wallet IDs containing additional colons survive.
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

// =============================================================================
// AUTHORIZE — response transformer
// =============================================================================

/// Successful response codes per tech spec § Status Mappings.
///
/// `A0000` = full approval, `A0002` = partial approval. Anything else combined
/// with `status=PASS` is treated as an unexpected success surface (fail closed)
/// to surface upstream.
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
        (Some(TsysTransitStatus::Pass), Some("A0002"), _) => AttemptStatus::PartialCharged,
        (Some(TsysTransitStatus::Fail), _, _) => AttemptStatus::Failure,
        // Unknown / missing — fail closed.
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
        let body = response.body();

        let status = map_authorize_status(response);

        // Failure surface: surface code/message but keep transactionID if TransIT
        // gave us one (tech spec § Error Codes — decline envelopes still carry
        // <transactionID>).
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
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: body.transaction_id.clone(),
                    network_decline_code: body.host_response_code.clone(),
                    network_advice_code: None,
                    network_error_message: body.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success path requires a transactionID — without one we cannot drive
        // subsequent Capture/Void/Refund flows, so reject as a deserialization
        // problem.
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

// =============================================================================
// PSYNC — request transformer
// =============================================================================
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

// =============================================================================
// PSYNC — response transformer
// =============================================================================

/// Map TransIT PSync (`<status>` + `<transactionState>`) to `AttemptStatus`
/// per tech spec § Status Mappings.
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
        // Unknown / missing transactionState — keep Pending and log a warning
        // rather than panicking. UCS callers will retry the sync.
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
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // For success / pending: prefer the response's transactionID when
        // present; otherwise fall back to what we asked about so the caller
        // never loses the reference.
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

// =============================================================================
// CAPTURE — request transformer
// =============================================================================
fn compute_capture_sales_tax<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> Result<Option<StringMajorUnit>, Report<IntegrationError>> {
    let merchant_metadata = match router_data.request.metadata.as_ref() {
        Some(meta) => serde_json::from_value::<TsysTransitMerchantMetadata>(meta.clone().expose())
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "connector_metadata.tsys_transit",
                context: Default::default(),
            })?,
        None => TsysTransitMerchantMetadata::default(),
    };
    let merchant_inner = merchant_metadata.into_inner();
    let metadata_order_tax_amount = merchant_inner
        .order_tax_amount
        .map(common_utils::types::MinorUnit::new);
    if merchant_inner.commercial_card.is_none() && metadata_order_tax_amount.is_none() {
        return Ok(None);
    }

    router_data
        .request
        .order_tax_amount
        .or_else(|| {
            router_data
                .resource_common_data
                .l2_l3_data
                .as_deref()
                .and_then(|data| data.get_order_tax_amount())
        })
        .or(metadata_order_tax_amount)
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

        // The auth's <transactionID> drives the capture — it is required.
        let transaction_id = router_data.request.get_connector_transaction_id()?;

        let transaction_amount = super::TsysTransitAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;
        let sales_tax = compute_capture_sales_tax::<T>(router_data)?;

        // TODO(tsys_transit): wire seq_number / payment_count for multi-clearing
        // (split-shipment) via add-connector-flow. PR-1 ships single-capture only.
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

// =============================================================================
// CAPTURE — response transformer
// =============================================================================

/// Map TransIT Capture (`<status>` + `<responseCode>`) to `AttemptStatus` per
/// tech spec § Status Mappings.
///
/// - `PASS` + `A0000` → `Charged`
/// - `PASS` + `A0002` → `PartialCharged`
/// - `FAIL` (any code) → `CaptureFailed`
/// - Anything else → `CaptureFailed` (fail closed)
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
                    attempt_status: Some(AttemptStatus::CaptureFailed),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success path: prefer response's transactionID; fall back to the auth
        // txn id we sent (TransIT's capture echoes the same id).
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

// =============================================================================
// REFUND — request transformer
// =============================================================================
//
// TransIT Return supports three modes from the same `<Return>` element shape:
//
//   1. Referenced full    — `transactionID` only (no `transactionAmount`).
//   2. Referenced partial — `transactionID` + `transactionAmount`.
//   3. Unreferenced       — NO `transactionID`; raw card data + `transactionAmount`.
//
// Mode selection happens here based on `RefundsData`:
//   * non-empty `connector_transaction_id` → referenced (we always emit
//     `transactionAmount` in PR-1; "omit for full" is a TODO follow-up so the
//     gateway recognises the partial vs. full distinction without us guessing
//     the original amount).
//   * empty `connector_transaction_id` → unreferenced; raw card data is
//     required. `RefundsData` does not surface `payment_method_data` today, so
//     this path returns `MissingRequiredField` until upstream wires card data
//     through for refunds.
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
            // Referenced mode (full or partial). PR-1 always emits
            // `transactionAmount` so the gateway sees the explicit value; a
            // follow-up TODO will compare `refund_amount` to the original
            // captured amount and omit `transactionAmount` for full refunds.
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
            // Unreferenced mode: full card data must be supplied. `RefundsData`
            // does not carry `payment_method_data` today, so PR-1 surfaces this
            // as a missing-field error rather than silently producing an
            // invalid request.
            Err(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data for unreferenced refund",
                context: Default::default(),
            }
            .into())
        }
    }
}

// =============================================================================
// REFUND — response transformer
// =============================================================================

/// Map TransIT Return (`<status>` + `<responseCode>`) to `RefundStatus` per
/// tech spec § Status Mappings.
///
/// - `PASS` + `A0000` → `Success` — full referenced refund completed.
/// - `PASS` + `A0002` → `Success` — partial approval (refundedAmount in the
///   response reflects the actual amount processed).
/// - `PASS` + `A0014` → `Success` — Return requested against an unsettled
///   transaction; TSYS converts it to a pre-settlement Void. Effective refund
///   from the merchant's perspective. Verified live (`<ReturnResponse>` with
///   `responseMessage: "Return requested, Void successful"`).
/// - `FAIL` (any code) → `Failure`
/// - Anything else → `Failure` (fail closed)
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

        // Success path: TransIT echoes the original capture's transactionID for
        // referenced returns; we treat that as the refund identifier for PR-1.
        // RSync will refine this once we know the on-wire id semantics.
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

// =============================================================================
// RSYNC — request transformer (REUSES TsysTransitTransactionInquiryRequest)
// =============================================================================
//
// TransIT refunds are sync-final on `<ReturnResponse>`; there is no dedicated
// refund-status-poll endpoint. HS still dispatches RSync though, so we
// re-issue a `<TransactionInquiry>` against the original refund's
// `transactionID` (echoed back by TransIT as `connector_refund_id` in our
// Return response transformer). If upstream lacks a refund id we fall back to
// the original payment transactionID — both are valid keys for TransIT's
// inquiry endpoint.
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

        // Prefer `connector_refund_id` (TransIT's echoed `<transactionID>` from
        // the original `<ReturnResponse>`); fall back to the original payment's
        // `connector_transaction_id` if the refund id wasn't recorded.
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

// =============================================================================
// RSYNC — response transformer (REUSES TsysTransitTransactionInquiryResponse)
// =============================================================================

/// Map TransIT TransactionInquiry (`<status>` + `<transactionState>`) to
/// `RefundStatus` per tech spec § Status Mappings.
///
/// - `PASS` + `RETURNED` → `Success` (refund applied, awaiting batch settle)
/// - `PASS` + `SETTLED`  → `Success` (refund batch settled — terminal success)
/// - `PASS` + `VOIDED`   → `Failure` (the return itself was reversed; refund
///   didn't actually go through).
///   TODO(tsys_transit): VOIDED-on-RSync semantics depend on whether TransIT
///   distinguishes "return reversed before settle" vs "original auth voided";
///   confirm with TSYS whether `Failure` is the correct terminal mapping.
/// - `FAIL`              → `Failure`
/// - Unknown / missing   → `Pending` (do NOT fail; let HS poll again).
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
        // TODO(tsys_transit): confirm VOIDED semantics with TSYS — currently treated
        // as terminal Failure because a voided return means the refund didn't
        // settle to the cardholder.
        (Some(TsysTransitStatus::Pass), Some(TsysTransitTransactionState::Voided)) => {
            RefundStatus::Failure
        }
        (Some(TsysTransitStatus::Fail), _) => RefundStatus::Failure,
        // Unknown / missing transactionState (including Authorized/Captured
        // pre-return states) — stay Pending so HS keeps polling.
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

        // Success / Pending: prefer the response's transactionID; fall back to
        // whichever id we sent so the caller never loses the reference.
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

// =============================================================================
// VOID — request transformer
// =============================================================================
//
// TransIT `<Void>` accepts an optional `<transactionAmount>`:
//   * Omitted   → full void of the prior auth.
//   * Provided  → partial void (cert script Step 7) — the prior auth is reduced
//     by that amount.
//
// `PaymentVoidData` carries an `Option<MinorUnit>` `amount` field. When set
// alongside `currency`, we convert via the StringMajorUnit converter and emit
// it; otherwise we omit `<transactionAmount>` so TransIT treats this as a full
// void.
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

        // Partial-void support: if both `amount` and `currency` are present on
        // PaymentVoidData, convert to a major-unit string and emit
        // `<transactionAmount>`; otherwise omit so TransIT performs a full
        // void.
        let transaction_amount = match (router_data.request.amount, router_data.request.currency) {
            (Some(amount), Some(currency)) => Some(super::TsysTransitAmountConvertor::convert(
                amount, currency,
            )?),
            _ => None,
        };

        // Cert script Step 7: voidReason is required. Derive from
        // `cancellation_reason`, fall back to a sensible default, cap at 80
        // chars to stay within TSYS' field bounds.
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

// =============================================================================
// VOID — response transformer
// =============================================================================

/// Map TransIT Void (`<status>` + `<responseCode>`) to `AttemptStatus` per
/// tech spec § Status Mappings.
///
/// - `PASS` + `A0000` → `Voided` (full void)
/// - `PASS` + `A0002` → `Voided` (partial void — the auth is reduced; at the
///   auth lifecycle level the state is still "voided" from UCS's perspective)
/// - `FAIL` (any code) → `VoidFailed`
/// - Anything else → `VoidFailed` (fail closed)
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
                    attempt_status: Some(AttemptStatus::VoidFailed),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Success path: prefer response's transactionID; fall back to the auth
        // txn id we sent (TransIT echoes the same id).
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

// =============================================================================
// CreateConnectorCustomer — request transformer (`<AddCustomer>`)
// =============================================================================
//
// Sources:
//   - first/last name: split `ConnectorCustomerData.name` on first whitespace.
//     No whitespace -> entire string goes to firstName, lastName defaults to
//     "-" (TSYS' XSD requires both fields).
//   - addressLine1 / zip: PaymentFlowData.address.billing_address.
//   - card data: `ConnectorCustomerData` does NOT carry payment_method_data in
//     this repo. PR-1 fails closed via `MissingRequiredField` so the live-test
//     phase identifies the right HS-side bridge before iterating.
//
// `expirationDate` in `<AddCustomer>` is MMYYYY (6 digits) — different from
// Sale/Auth's MMYY.

fn split_full_name(full: &str) -> (String, String) {
    let trimmed = full.trim();
    if trimmed.is_empty() {
        return ("-".to_string(), "-".to_string());
    }
    match trimmed.split_once(char::is_whitespace) {
        Some((first, rest)) => {
            let last = rest.trim();
            (
                first.to_string(),
                if last.is_empty() {
                    "-".to_string()
                } else {
                    last.to_string()
                },
            )
        }
        None => (trimmed.to_string(), "-".to_string()),
    }
}

#[allow(dead_code)]
fn format_add_customer_expiration(card: &Card<impl PaymentMethodDataTypes>) -> Secret<String> {
    // AddCustomer wants MMYYYY (6 digits). Normalize 2-digit years up to 4-digit
    // by prefixing "20" (TransIT only supports cards expiring this century).
    let month_raw = card.card_exp_month.peek().clone();
    let year_raw = card.card_exp_year.peek().clone();
    let month = if month_raw.len() == 1 {
        format!("0{month_raw}")
    } else {
        month_raw
    };
    let year_full = if year_raw.len() == 2 {
        format!("20{year_raw}")
    } else {
        year_raw
    };
    Secret::new(format!("{month}{year_full}"))
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TsysTransitRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for TsysTransitAddCustomerRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: TsysTransitRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TsysTransitAuthType::try_from(&router_data.connector_config)?;

        // Name — required by AddCustomer XSD. Split on the first whitespace; if
        // no whitespace at all, lastName defaults to "-".
        let name_secret = router_data.request.name.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "ConnectorCustomerData.name",
                context: Default::default(),
            })
        })?;
        let (first_name, last_name) = split_full_name(name_secret.peek().as_str());

        // Billing address — supplies addressLine1 + zip in both personalDetails
        // and walletDetails per the AddCustomer body shape.
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

        // `ConnectorCustomerData` is non-generic and lacks `payment_method_data`
        // in this repo; we cannot populate the mandatory <walletDetails>
        // <cardDetails> block without it. Fail closed with the precise field
        // name so the live-test phase identifies the right HS-side bridge.
        let (card_number, expiration_date) = extract_add_customer_card::<T>(router_data)?;

        Ok(Self {
            device_id: auth.device_id,
            transaction_key: auth.transaction_key,
            personal_details: TsysTransitPersonalDetails {
                first_name: Secret::new(first_name),
                last_name: Secret::new(last_name),
                address_line1: address_line1.clone(),
                zip: zip.clone(),
            },
            wallet_details: TsysTransitAddCustomerWalletDetails {
                card_details: TsysTransitAddCustomerCardDetails {
                    card_number,
                    expiration_date,
                },
                address_line1,
                zip,
                payment_sequence: "1".to_string(),
            },
            developer_id: auth.developer_id,
        })
    }
}

/// Pull card data for `<AddCustomer>` from any HS-side surface we recognize.
///
/// `ConnectorCustomerData` does not carry `payment_method_data` in this repo
/// today, so we surface `MissingRequiredField` explicitly. The live-test phase
/// will identify the right HS-side bridge (likely a generic variant of
/// `ConnectorCustomerData` or a `connector_feature_data` payload).
fn extract_add_customer_card<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    _router_data: &RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >,
) -> Result<(Secret<String>, Secret<String>), Report<IntegrationError>> {
    Err(IntegrationError::MissingRequiredField {
        field_name: "ConnectorCustomerData.payment_method_data (card)",
        context: Default::default(),
    }
    .into())
}

// =============================================================================
// CreateConnectorCustomer — response transformer
// =============================================================================

impl TryFrom<ResponseRouterData<TsysTransitAddCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TsysTransitAddCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let response = &item.response;

        let is_success = matches!(response.status, Some(TsysTransitStatus::Pass))
            && response.response_code.as_deref() == Some("A0000");

        if !is_success {
            return Ok(Self {
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
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        let customer_code = response.customer_code.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "tsysTransit: AddCustomerResponse missing <customerCode>; confirm API contract.",
            )
        })?;
        let wallet_id = response
            .wallet_details
            .as_ref()
            .and_then(|w| w.wallet_id.clone())
            .ok_or_else(|| {
                crate::utils::response_deserialization_fail(
                    item.http_code,
                    "tsysTransit: AddCustomerResponse missing <walletDetails><walletID>; confirm API contract.",
                )
            })?;

        // Stash the Path B mandate id (`cust:CCC:WWW`) on
        // `PaymentFlowData.reference_id` so the next Authorize call can pick it
        // up. `ConnectorCustomerResponse` only carries `connector_customer_id`,
        // so we use the generic reference_id slot to surface walletID.
        let path_b_mandate_id = format!("cust:{customer_code}:{wallet_id}");

        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: customer_code,
            }),
            resource_common_data: PaymentFlowData {
                reference_id: Some(path_b_mandate_id),
                ..router_data.resource_common_data.clone()
            },
            ..router_data.clone()
        })
    }
}

// =============================================================================
// SetupMandate — request transformer (`<CardAuthentication>`, zero-dollar CIT)
// =============================================================================

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

        // Reuse the Authorize metadata overrides so terminalData is consistent
        // across CIT verify and the subsequent MIT call.
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
        // CardAuthentication uses the e-commerce terminalData baseline per cert
        // (the recurring presets explicitly do NOT apply to Card Authentications).
        // But: MC CIT requires `cardholderPresentDetail=CARDHOLDER_NOT_PRESENT_
        // RECURRING_TRANSACTION` on the CIT in a recurring/subscription series,
        // and the `citStatusIndicator` (C102/C103/C104) when present.
        //
        // SetupMandate has no `recurring_mandate_payment_data` (no prior MIT)
        // — pass None so installment-counter guards never fire on CIT setup.
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
                // MC CIT in a recurring series: force RECURRING_TRANSACTION on
                // the CIT (cert: "MasterCard requires you to set
                // cardholderPresentDetail as CARDHOLDER_NOT_PRESENT_RECURRING_
                // TRANSACTION in both the CIT … and the subsequent MIT").
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
        let card_data_input_mode = terminal_overrides.card_data_input_mode.unwrap_or_else(|| {
            if is_cit_setup {
                TsysTransitCardDataInputMode::MerchantInitiatedTransactionCardCredentialStoredOnFile
            } else {
                match channel {
                    Some(PaymentChannel::Ecommerce) | None => {
                        TsysTransitCardDataInputMode::PanEntryElectronicCommerceIncludingRemoteChip
                    }
                    _ => TsysTransitCardDataInputMode::KeyEnteredInput,
                }
            }
        });
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

// =============================================================================
// SetupMandate — response transformer
// =============================================================================

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
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: response.response_message.clone(),
                }),
                ..router_data.clone()
            });
        }

        // Prefer cardTransactionIdentifier (the actual NTID); fall back to
        // transactionID if the cert sandbox forgets to emit it.
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
            connector_response_reference_id: Some(connector_txn_id),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                // Card verified — Authorized is the closest non-charged status.
                status: AttemptStatus::Authorized,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// =============================================================================
// REPEAT PAYMENT — request transformer
// =============================================================================
//
// TransIT does not expose a separate "RecurringCharge" endpoint; MIT replays
// fire the same `<Sale>` (auto-capture) or `<Auth>` (manual capture) XML body
// against the same POST `/` endpoint. We translate `RepeatPaymentData` into a
// synthetic `PaymentsAuthorizeData` so the existing Authorize TryFrom (and its
// `decode_mandate_dispatch` logic) handles Path A (NTID) and Path B (vault)
// without duplication.
fn repeat_payment_data_to_authorize<T: PaymentMethodDataTypes>(
    req: &RepeatPaymentData<T>,
) -> PaymentsAuthorizeData<T> {
    // RepeatPaymentData carries `mandate_reference: MandateReferenceId` directly;
    // wrap it into the `MandateIds` shape Authorize expects.
    let mandate_ids = MandateIds {
        mandate_id: None,
        mandate_reference_id: Some(req.mandate_reference.clone()),
    };

    PaymentsAuthorizeData {
        payment_method_data: req.payment_method_data.clone(),
        amount: req.minor_amount,
        order_tax_amount: None,
        email: req.email.clone(),
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
        // MIT — explicitly off-session per the spec.
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
        // MIT replay — channel inferred from the original CIT; default to
        // Ecommerce so terminalData defaults match the typical recurring case.
        payment_channel: None,
        enable_partial_authorization: req.enable_partial_authorization,
        locale: req.locale.clone(),
        redirect_response: None,
        threeds_method_comp_ind: None,
        continue_redirection_url: None,
        tokenization: None,
        // Pipe HS-native MIT fields through so the synthesized Authorize body
        // engages recurring/installment mode without any metadata shim.
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
    > for TsysTransitRepeatPaymentRequest<T>
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

        // Project the RepeatPayment RouterDataV2 onto an Authorize-shaped one so
        // the existing TryFrom (which encodes all of Path A / Path B / CIT logic)
        // can build the wire body unchanged.
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

        let inner = TsysTransitAuthorizeRequest::<T>::try_from(synthetic_wrapper)?;
        Ok(Self(inner))
    }
}

// =============================================================================
// REPEAT PAYMENT — response transformer
// =============================================================================
//
// Response shape is identical to Authorize (Sale / Auth response). We reuse
// `map_authorize_status` and the same success/failure surface; only the
// `RouterDataV2` flow phantom differs.
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
        let body = response.body();

        // Reuse the Authorize status mapper by projecting onto the Authorize
        // response enum (wire shape is identical per tech spec).
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
                    attempt_status: Some(AttemptStatus::Failure),
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
