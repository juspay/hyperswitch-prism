use base64::{engine::general_purpose::STANDARD, Engine};
use cards::{CardNumber, NetworkToken};
use common_enums::{self, AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    ext_traits::{ByteSliceExt, Encode, OptionExt, ValueExt},
    request::Method,
    types::{MinorUnit, SemanticVersion},
    SecretSerdeValue,
};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, DefendDispute, PSync, Refund, RepeatPayment, SetupMandate,
        SubmitEvidence, Void,
    },
    connector_types::{
        AcceptDisputeData, CardDetailUpdate, DisputeDefendData, DisputeFlowData,
        DisputeResponseData, EventType, MandateReference, MandateReferenceId, PaymentFlowData,
        PaymentMethodUpdate, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData, SubmitEvidenceData,
    },
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, BankRedirectData, BankTransferData, Card,
        CardRedirectData, DefaultPCIHolder, GiftCardData, GpayTokenizationData, NetworkTokenData,
        PayLaterData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, VoucherData,
        VoucherNextStepData, WalletData,
    },
    router_data::{
        ConnectorResponseData, ConnectorSpecificConfig, ErrorResponse,
        ExtendedAuthorizationResponseData,
    },
    router_data_v2::RouterDataV2,
    router_request_types::SyncRequestType,
    router_response_types::RedirectForm,
    utils as domain_utils,
    utils::get_timestamp_in_milliseconds,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, PrimitiveDateTime};
use url::Url;

use super::AdyenRouterData;
use crate::{
    types::ResponseRouterData,
    utils::{
        self, is_manual_capture,
        qr_code::{QrCodeInformation, QrImage},
        to_connector_meta_from_secret,
    },
};
use domain_types::errors::ConnectorResponseTransformationError;
use domain_types::errors::{IntegrationError, WebhookError};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum Currency {
    #[default]
    USD,
    EUR,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    pub currency: common_enums::Currency,
    pub value: MinorUnit,
}

type Error = error_stack::Report<IntegrationError>;

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenApplePayDecryptData {
    number: CardNumber,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    brand: String,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenGooglePayDecryptData {
    number: CardNumber,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    brand: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardBrand {
    Visa,
    MC,
    Amex,
    Jcb,
    Diners,
    Discover,
    Cartebancaire,
    Cup,
    Maestro,
    Rupay,
    Star,
    Accel,
    Pulse,
    Nyce,
    Bcmc,
}

const GOOGLE_PAY_BRAND: &str = "googlepay";

impl TryFrom<&domain_utils::CardIssuer> for CardBrand {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(card_issuer: &domain_utils::CardIssuer) -> Result<Self, Self::Error> {
        match card_issuer {
            domain_utils::CardIssuer::AmericanExpress => Ok(Self::Amex),
            domain_utils::CardIssuer::Master => Ok(Self::MC),
            domain_utils::CardIssuer::Visa => Ok(Self::Visa),
            domain_utils::CardIssuer::Maestro => Ok(Self::Maestro),
            domain_utils::CardIssuer::Discover => Ok(Self::Discover),
            domain_utils::CardIssuer::DinersClub => Ok(Self::Diners),
            domain_utils::CardIssuer::JCB => Ok(Self::Jcb),
            domain_utils::CardIssuer::CarteBlanche => Ok(Self::Cartebancaire),
            domain_utils::CardIssuer::CartesBancaires => Ok(Self::Cartebancaire),
            domain_utils::CardIssuer::UnionPay => Ok(Self::Cup),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GiftCardBrand {
    Givex,
    Auriga,
    Babygiftcard,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenGiftCardData {
    brand: GiftCardBrand,
    number: Secret<String>,
    cvc: Secret<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub enum AdyenConnectorError {
    ParsingFailed,
    NotImplemented,
    FailedToObtainAuthType,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    number: RawCardNumber<T>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    cvc: Option<Secret<String>>,
    holder_name: Option<Secret<String>>,
    brand: Option<CardBrand>, //Mandatory for mandate using network_txns_id
    network_payment_reference: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenNetworkTokenData {
    number: NetworkToken,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    holder_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brand: Option<CardBrand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_payment_reference: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum AdyenPaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "klarna")]
    Klarna,
    #[serde(rename = "affirm")]
    AdyenAffirm,
    #[serde(rename = "afterpaytouch")]
    AfterPay,
    #[serde(rename = "clearpay")]
    ClearPay,
    #[serde(rename = "paybright")]
    PayBright,
    #[serde(rename = "walley")]
    Walley,
    #[serde(rename = "alma")]
    AlmaPayLater,
    #[serde(rename = "atome")]
    Atome,
    #[serde(rename = "scheme")]
    AdyenCard(Box<AdyenCard<T>>),
    #[serde(rename = "networkToken")]
    NetworkToken(Box<AdyenNetworkTokenData>),
    #[serde(rename = "googlepay")]
    Gpay(Box<AdyenGPay>),
    #[serde(rename = "scheme")]
    GooglePayDecrypt(Box<AdyenGooglePayDecryptData>),
    ApplePay(Box<AdyenApplePay>),
    #[serde(rename = "scheme")]
    ApplePayDecrypt(Box<AdyenApplePayDecryptData>),
    #[serde(rename = "scheme")]
    BancontactCard(Box<AdyenCard<DefaultPCIHolder>>),
    Bizum,
    Blik(Box<BlikRedirectionData>),
    Eps(Box<BankRedirectionWithIssuer>),
    Ideal,
    #[serde(rename = "onlineBanking_CZ")]
    OnlineBankingCzechRepublic(Box<OnlineBankingCzechRepublicData>),
    #[serde(rename = "ebanking_FI")]
    OnlineBankingFinland,
    #[serde(rename = "onlineBanking_PL")]
    OnlineBankingPoland(Box<OnlineBankingPolandData>),
    #[serde(rename = "onlineBanking_SK")]
    OnlineBankingSlovakia(Box<OnlineBankingSlovakiaData>),
    #[serde(rename = "molpay_ebanking_fpx_MY")]
    OnlineBankingFpx(Box<OnlineBankingFpxData>),
    #[serde(rename = "molpay_ebanking_TH")]
    OnlineBankingThailand(Box<OnlineBankingThailandData>),
    #[serde(rename = "paybybank")]
    OpenBankingUK(Box<OpenBankingUKData>),
    #[serde(rename = "giftcard")]
    AdyenGiftCard(Box<AdyenGiftCardData>),
    #[serde(rename = "paysafecard")]
    PaySafeCard,
    #[serde(rename = "trustly")]
    Trustly,
    // Bank transfer payment methods (Indonesian banks via Doku)
    #[serde(rename = "doku_permata_lite_atm")]
    PermataBankTransfer(Box<DokuBankData>),
    #[serde(rename = "doku_bca_va")]
    BcaBankTransfer(Box<DokuBankData>),
    #[serde(rename = "doku_bni_va")]
    BniVa(Box<DokuBankData>),
    #[serde(rename = "doku_bri_va")]
    BriVa(Box<DokuBankData>),
    #[serde(rename = "doku_cimb_va")]
    CimbVa(Box<DokuBankData>),
    #[serde(rename = "doku_danamon_va")]
    DanamonVa(Box<DokuBankData>),
    #[serde(rename = "doku_mandiri_va")]
    MandiriVa(Box<DokuBankData>),
    // Brazilian instant payment
    Pix,
    #[serde(rename = "ach")]
    AchDirectDebit(Box<AchDirectDebitData>),
    #[serde(rename = "sepadirectdebit")]
    SepaDirectDebit(Box<SepaDirectDebitData>),
    #[serde(rename = "directdebit_GB")]
    BacsDirectDebit(Box<BacsDirectDebitData>),
    Knet,
    Benefit,
    #[serde(rename = "momo_atm")]
    MomoAtm,
    // Voucher payment methods
    #[serde(rename = "boletobancario")]
    BoletoBancario,
    #[serde(rename = "doku_alfamart")]
    Alfamart(Box<DokuBankData>),
    #[serde(rename = "doku_indomaret")]
    Indomaret(Box<DokuBankData>),
    #[serde(rename = "oxxo")]
    Oxxo,
    #[serde(rename = "econtext_seven_eleven")]
    SevenEleven(Box<JCSVoucherData>),
    #[serde(rename = "econtext_stores")]
    Lawson(Box<JCSVoucherData>),
    #[serde(rename = "econtext_stores")]
    MiniStop(Box<JCSVoucherData>),
    #[serde(rename = "econtext_stores")]
    FamilyMart(Box<JCSVoucherData>),
    #[serde(rename = "econtext_stores")]
    Seicomart(Box<JCSVoucherData>),
    #[serde(rename = "econtext_stores")]
    PayEasy(Box<JCSVoucherData>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlikRedirectionData {
    blik_code: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JCSVoucherData {
    first_name: Secret<String>,
    last_name: Option<Secret<String>>,
    shopper_email: common_utils::pii::Email,
    telephone_number: Secret<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankRedirectionWithIssuer {
    issuer: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OnlineBankingCzechRepublicData {
    issuer: OnlineBankingCzechRepublicBanks,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnlineBankingCzechRepublicBanks {
    KB,
    CS,
}

#[derive(Debug, Clone, Serialize)]
pub struct OnlineBankingPolandData {
    issuer: OnlineBankingPolandBanks,
}

#[derive(Debug, Clone, Serialize)]
pub enum OnlineBankingPolandBanks {
    #[serde(rename = "154")]
    BlikPSP,
    #[serde(rename = "31")]
    PlaceZIPKO,
    #[serde(rename = "243")]
    MBank,
    #[serde(rename = "112")]
    PayWithING,
    #[serde(rename = "20")]
    SantanderPrzelew24,
    #[serde(rename = "65")]
    BankPEKAOSA,
    #[serde(rename = "85")]
    BankMillennium,
    #[serde(rename = "88")]
    PayWithAliorBank,
    #[serde(rename = "143")]
    BankiSpoldzielcze,
    #[serde(rename = "26")]
    PayWithInteligo,
    #[serde(rename = "33")]
    BNPParibasPoland,
    #[serde(rename = "144")]
    BankNowySA,
    #[serde(rename = "45")]
    CreditAgricole,
    #[serde(rename = "99")]
    PayWithBOS,
    #[serde(rename = "119")]
    PayWithCitiHandlowy,
    #[serde(rename = "131")]
    PayWithPlusBank,
    #[serde(rename = "64")]
    ToyotaBank,
    #[serde(rename = "153")]
    VeloBank,
    #[serde(rename = "141")]
    ETransferPocztowy24,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineBankingSlovakiaData {
    issuer: OnlineBankingSlovakiaBanks,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnlineBankingSlovakiaBanks {
    Vub,
    Posto,
    Sporo,
    Tatra,
    Viamo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineBankingFpxData {
    issuer: OnlineBankingFpxIssuer,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OnlineBankingFpxIssuer {
    FpxAbb,
    FpxAgrobank,
    FpxAbmb,
    FpxAmb,
    FpxBimb,
    FpxBmmb,
    FpxBkrm,
    FpxBsn,
    FpxCimbclicks,
    FpxHlb,
    FpxHsbc,
    FpxKfh,
    FpxMb2u,
    FpxOcbc,
    FpxPbb,
    FpxRhb,
    FpxScb,
    FpxUob,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineBankingThailandData {
    issuer: OnlineBankingThailandIssuer,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub enum OnlineBankingThailandIssuer {
    #[serde(rename = "molpay_bangkokbank")]
    Bangkokbank,
    #[serde(rename = "molpay_krungsribank")]
    Krungsribank,
    #[serde(rename = "molpay_krungthaibank")]
    Krungthaibank,
    #[serde(rename = "molpay_siamcommercialbank")]
    Siamcommercialbank,
    #[serde(rename = "molpay_kbank")]
    Kbank,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenBankingUKData {
    issuer: Option<OpenBankingUKIssuer>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
pub enum OpenBankingUKIssuer {
    #[serde(rename = "uk-test-open-banking-redirect")]
    RedirectSuccess,
    #[serde(rename = "uk-test-open-banking-redirect-failed")]
    RedirectFailure,
    #[serde(rename = "uk-test-open-banking-redirect-cancelled")]
    RedirectCancelled,
    #[serde(rename = "uk-aib-oauth2")]
    Aib,
    #[serde(rename = "uk-bankofscotland-oauth2")]
    BankOfScotland,
    #[serde(rename = "uk-barclays-oauth2")]
    Barclays,
    #[serde(rename = "uk-danskebank-oauth2")]
    DanskeBank,
    #[serde(rename = "uk-firstdirect-oauth2")]
    FirstDirect,
    #[serde(rename = "uk-firsttrust-oauth2")]
    FirstTrust,
    #[serde(rename = "uk-hsbc-oauth2")]
    HsbcBank,
    #[serde(rename = "uk-halifax-oauth2")]
    Halifax,
    #[serde(rename = "uk-lloyds-oauth2")]
    Lloyds,
    #[serde(rename = "uk-monzo-oauth2")]
    Monzo,
    #[serde(rename = "uk-natwest-oauth2")]
    NatWest,
    #[serde(rename = "uk-nationwide-oauth2")]
    NationwideBank,
    #[serde(rename = "uk-revolut-oauth2")]
    Revolut,
    #[serde(rename = "uk-rbs-oauth2")]
    RoyalBankOfScotland,
    #[serde(rename = "uk-santander-oauth2")]
    SantanderPrzelew24,
    #[serde(rename = "uk-starling-oauth2")]
    Starling,
    #[serde(rename = "uk-tsb-oauth2")]
    TsbBank,
    #[serde(rename = "uk-tesco-oauth2")]
    TescoBank,
    #[serde(rename = "uk-ulster-oauth2")]
    UlsterBank,
}

/// Data structure for Indonesian bank transfers (Doku integration)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DokuBankData {
    first_name: Secret<String>,
    last_name: Option<Secret<String>>,
    shopper_email: common_utils::pii::Email,
}

pub struct AdyenTestBankNames(String);

impl TryFrom<&common_enums::BankNames> for AdyenTestBankNames {
    type Error = Error;
    fn try_from(bank: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank {
            common_enums::BankNames::AbnAmro => Ok(Self("1121".to_string())),
            common_enums::BankNames::AsnBank => Ok(Self("1151".to_string())),
            common_enums::BankNames::Bunq => Ok(Self("1152".to_string())),
            common_enums::BankNames::Ing => Ok(Self("1154".to_string())),
            common_enums::BankNames::Knab => Ok(Self("1155".to_string())),
            common_enums::BankNames::N26 => Ok(Self("1156".to_string())),
            common_enums::BankNames::NationaleNederlanden => Ok(Self("1157".to_string())),
            common_enums::BankNames::Rabobank => Ok(Self("1157".to_string())),
            common_enums::BankNames::Regiobank => Ok(Self("1158".to_string())),
            common_enums::BankNames::Revolut => Ok(Self("1159".to_string())),
            common_enums::BankNames::SnsBank => Ok(Self("1159".to_string())),
            common_enums::BankNames::TriodosBank => Ok(Self("1159".to_string())),
            common_enums::BankNames::VanLanschot => Ok(Self("1159".to_string())),
            common_enums::BankNames::Yoursafe => Ok(Self("1159".to_string())),
            common_enums::BankNames::BankAustria => {
                Ok(Self("e6819e7a-f663-414b-92ec-cf7c82d2f4e5".to_string()))
            }
            common_enums::BankNames::BawagPskAg => {
                Ok(Self("ba7199cc-f057-42f2-9856-2378abf21638".to_string()))
            }
            common_enums::BankNames::Dolomitenbank => {
                Ok(Self("d5d5b133-1c0d-4c08-b2be-3c9b116dc326".to_string()))
            }
            common_enums::BankNames::EasybankAg => {
                Ok(Self("eff103e6-843d-48b7-a6e6-fbd88f511b11".to_string()))
            }
            common_enums::BankNames::ErsteBankUndSparkassen => {
                Ok(Self("3fdc41fc-3d3d-4ee3-a1fe-cd79cfd58ea3".to_string()))
            }
            common_enums::BankNames::HypoTirolBankAg => {
                Ok(Self("6765e225-a0dc-4481-9666-e26303d4f221".to_string()))
            }
            common_enums::BankNames::PosojilnicaBankEGen => {
                Ok(Self("65ef4682-4944-499f-828f-5d74ad288376".to_string()))
            }
            common_enums::BankNames::RaiffeisenBankengruppeOsterreich => {
                Ok(Self("ee9fc487-ebe0-486c-8101-17dce5141a67".to_string()))
            }
            common_enums::BankNames::SchoellerbankAg => {
                Ok(Self("1190c4d1-b37a-487e-9355-e0a067f54a9f".to_string()))
            }
            common_enums::BankNames::SpardaBankWien => {
                Ok(Self("8b0bfeea-fbb0-4337-b3a1-0e25c0f060fc".to_string()))
            }
            common_enums::BankNames::VolksbankGruppe => {
                Ok(Self("e2e97aaa-de4c-4e18-9431-d99790773433".to_string()))
            }
            common_enums::BankNames::VolkskreditbankAg => {
                Ok(Self("4a0a975b-0594-4b40-9068-39f77b3a91f9".to_string()))
            }
            _ => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
            )
            .into()),
        }
    }
}

impl TryFrom<&common_enums::BankNames> for OnlineBankingCzechRepublicBanks {
    type Error = Error;
    fn try_from(bank_name: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank_name {
            common_enums::BankNames::KomercniBanka => Ok(Self::KB),
            common_enums::BankNames::CeskaSporitelna => Ok(Self::CS),
            _ => Err(IntegrationError::not_implemented("payment method").into()),
        }
    }
}

impl TryFrom<&common_enums::BankNames> for OnlineBankingPolandBanks {
    type Error = Error;
    fn try_from(bank_name: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank_name {
            common_enums::BankNames::BlikPSP => Ok(Self::BlikPSP),
            common_enums::BankNames::PlaceZIPKO => Ok(Self::PlaceZIPKO),
            common_enums::BankNames::MBank => Ok(Self::MBank),
            common_enums::BankNames::PayWithING => Ok(Self::PayWithING),
            common_enums::BankNames::SantanderPrzelew24 => Ok(Self::SantanderPrzelew24),
            common_enums::BankNames::BankPEKAOSA => Ok(Self::BankPEKAOSA),
            common_enums::BankNames::BankMillennium => Ok(Self::BankMillennium),
            common_enums::BankNames::PayWithAliorBank => Ok(Self::PayWithAliorBank),
            common_enums::BankNames::BankiSpoldzielcze => Ok(Self::BankiSpoldzielcze),
            common_enums::BankNames::PayWithInteligo => Ok(Self::PayWithInteligo),
            common_enums::BankNames::BNPParibasPoland => Ok(Self::BNPParibasPoland),
            common_enums::BankNames::BankNowySA => Ok(Self::BankNowySA),
            common_enums::BankNames::CreditAgricole => Ok(Self::CreditAgricole),
            common_enums::BankNames::PayWithBOS => Ok(Self::PayWithBOS),
            common_enums::BankNames::PayWithCitiHandlowy => Ok(Self::PayWithCitiHandlowy),
            common_enums::BankNames::PayWithPlusBank => Ok(Self::PayWithPlusBank),
            common_enums::BankNames::ToyotaBank => Ok(Self::ToyotaBank),
            common_enums::BankNames::VeloBank => Ok(Self::VeloBank),
            common_enums::BankNames::ETransferPocztowy24 => Ok(Self::ETransferPocztowy24),
            _ => Err(IntegrationError::not_implemented("payment method").into()),
        }
    }
}

impl TryFrom<&common_enums::BankNames> for OnlineBankingSlovakiaBanks {
    type Error = Error;
    fn try_from(bank_name: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank_name {
            common_enums::BankNames::EPlatbyVUB => Ok(Self::Vub),
            common_enums::BankNames::PostovaBanka => Ok(Self::Posto),
            common_enums::BankNames::SporoPay => Ok(Self::Sporo),
            common_enums::BankNames::TatraPay => Ok(Self::Tatra),
            common_enums::BankNames::Viamo => Ok(Self::Viamo),
            _ => Err(IntegrationError::not_implemented("payment method").into()),
        }
    }
}

impl TryFrom<&common_enums::BankNames> for OnlineBankingFpxIssuer {
    type Error = Error;
    fn try_from(bank_name: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank_name {
            common_enums::BankNames::AffinBank => Ok(Self::FpxAbb),
            common_enums::BankNames::AgroBank => Ok(Self::FpxAgrobank),
            common_enums::BankNames::AllianceBank => Ok(Self::FpxAbmb),
            common_enums::BankNames::AmBank => Ok(Self::FpxAmb),
            common_enums::BankNames::BankIslam => Ok(Self::FpxBimb),
            common_enums::BankNames::BankMuamalat => Ok(Self::FpxBmmb),
            common_enums::BankNames::BankRakyat => Ok(Self::FpxBkrm),
            common_enums::BankNames::BankSimpananNasional => Ok(Self::FpxBsn),
            common_enums::BankNames::CimbBank => Ok(Self::FpxCimbclicks),
            common_enums::BankNames::HongLeongBank => Ok(Self::FpxHlb),
            common_enums::BankNames::HsbcBank => Ok(Self::FpxHsbc),
            common_enums::BankNames::KuwaitFinanceHouse => Ok(Self::FpxKfh),
            common_enums::BankNames::Maybank => Ok(Self::FpxMb2u),
            common_enums::BankNames::OcbcBank => Ok(Self::FpxOcbc),
            common_enums::BankNames::PublicBank => Ok(Self::FpxPbb),
            common_enums::BankNames::RhbBank => Ok(Self::FpxRhb),
            common_enums::BankNames::StandardCharteredBank => Ok(Self::FpxScb),
            common_enums::BankNames::UobBank => Ok(Self::FpxUob),
            _ => Err(IntegrationError::not_implemented("payment method").into()),
        }
    }
}

impl TryFrom<&common_enums::BankNames> for OnlineBankingThailandIssuer {
    type Error = Error;
    fn try_from(bank_name: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank_name {
            common_enums::BankNames::BangkokBank => Ok(Self::Bangkokbank),
            common_enums::BankNames::KrungsriBank => Ok(Self::Krungsribank),
            common_enums::BankNames::KrungThaiBank => Ok(Self::Krungthaibank),
            common_enums::BankNames::TheSiamCommercialBank => Ok(Self::Siamcommercialbank),
            common_enums::BankNames::KasikornBank => Ok(Self::Kbank),
            _ => Err(IntegrationError::not_implemented("payment method").into()),
        }
    }
}

impl TryFrom<&common_enums::BankNames> for OpenBankingUKIssuer {
    type Error = Error;
    fn try_from(bank_name: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank_name {
            common_enums::BankNames::OpenBankSuccess => Ok(Self::RedirectSuccess),
            common_enums::BankNames::OpenBankFailure => Ok(Self::RedirectFailure),
            common_enums::BankNames::OpenBankCancelled => Ok(Self::RedirectCancelled),
            common_enums::BankNames::Aib => Ok(Self::Aib),
            common_enums::BankNames::BankOfScotland => Ok(Self::BankOfScotland),
            common_enums::BankNames::Barclays => Ok(Self::Barclays),
            common_enums::BankNames::DanskeBank => Ok(Self::DanskeBank),
            common_enums::BankNames::FirstDirect => Ok(Self::FirstDirect),
            common_enums::BankNames::FirstTrust => Ok(Self::FirstTrust),
            common_enums::BankNames::HsbcBank => Ok(Self::HsbcBank),
            common_enums::BankNames::Halifax => Ok(Self::Halifax),
            common_enums::BankNames::Lloyds => Ok(Self::Lloyds),
            common_enums::BankNames::Monzo => Ok(Self::Monzo),
            common_enums::BankNames::NatWest => Ok(Self::NatWest),
            common_enums::BankNames::NationwideBank => Ok(Self::NationwideBank),
            common_enums::BankNames::Revolut => Ok(Self::Revolut),
            common_enums::BankNames::RoyalBankOfScotland => Ok(Self::RoyalBankOfScotland),
            common_enums::BankNames::SantanderPrzelew24 => Ok(Self::SantanderPrzelew24),
            common_enums::BankNames::Starling => Ok(Self::Starling),
            common_enums::BankNames::TsbBank => Ok(Self::TsbBank),
            common_enums::BankNames::TescoBank => Ok(Self::TescoBank),
            common_enums::BankNames::UlsterBank => Ok(Self::UlsterBank),
            _ => Err(IntegrationError::not_implemented("payment method").into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchDirectDebitData {
    bank_account_number: Secret<String>,
    bank_location_id: Secret<String>,
    owner_name: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SepaDirectDebitData {
    #[serde(rename = "sepa.ownerName")]
    owner_name: Secret<String>,
    #[serde(rename = "sepa.ibanNumber")]
    iban_number: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacsDirectDebitData {
    bank_account_number: Secret<String>,
    bank_location_id: Secret<String>,
    holder_name: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdyenBrowserInfo {
    user_agent: String,
    accept_header: String,
    language: String,
    color_depth: u8,
    screen_height: u32,
    screen_width: u32,
    time_zone_offset: i32,
    java_enabled: bool,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub enum AuthType {
    #[default]
    PreAuth,
}

#[serde_with::skip_serializing_none]
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    city: Secret<String>,
    country: common_enums::CountryAlpha2,
    house_number_or_name: Secret<String>,
    postal_code: Secret<String>,
    state_or_province: Option<Secret<String>>,
    street: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    AdyenPaymentMethod(Box<AdyenPaymentMethod<T>>),
    AdyenMandatePaymentMethod(Box<AdyenMandate>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenMandate {
    #[serde(rename = "type")]
    pub payment_type: PaymentType,
    pub stored_payment_method_id: Secret<String>,
    pub holder_name: Option<Secret<String>>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdyenMpiData {
    directory_response: common_enums::TransactionStatus,
    authentication_response: common_enums::TransactionStatus,
    cavv: Option<Secret<String>>,
    token_authentication_verification_value: Option<Secret<String>>,
    eci: Option<String>,
    #[serde(rename = "dsTransID")]
    ds_trans_id: Option<String>,
    #[serde(rename = "threeDSVersion")]
    three_ds_version: Option<SemanticVersion>,
    challenge_cancel: Option<String>,
    risk_score: Option<String>,
    cavv_algorithm: Option<common_enums::CavvAlgorithm>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub enum AdyenShopperInteraction {
    #[default]
    Ecommerce,
    #[serde(rename = "ContAuth")]
    ContinuedAuthentication,
    Moto,
    #[serde(rename = "POS")]
    Pos,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    From<&RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>
    for AdyenShopperInteraction
{
    fn from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Self {
        match item.request.off_session {
            Some(true) => Self::ContinuedAuthentication,
            _ => Self::Ecommerce,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AdyenRecurringModel {
    UnscheduledCardOnFile,
    CardOnFile,
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalData {
    authorisation_type: Option<AuthType>,
    manual_capture: Option<String>,
    execute_three_d: Option<String>,
    pub recurring_processing_model: Option<AdyenRecurringModel>,
    /// Enable recurring details in dashboard to receive this ID, https://docs.adyen.com/online-payments/tokenization/create-and-use-tokens#test-and-go-live
    #[serde(rename = "recurring.recurringDetailReference")]
    recurring_detail_reference: Option<Secret<String>>,
    #[serde(rename = "recurring.shopperReference")]
    recurring_shopper_reference: Option<String>,
    network_tx_reference: Option<Secret<String>>,
    funds_availability: Option<String>,
    refusal_reason_raw: Option<String>,
    refusal_code_raw: Option<String>,
    merchant_advice_code: Option<String>,
    #[serde(flatten)]
    riskdata: Option<RiskData>,
    sca_exemption: Option<AdyenExemptionValues>,
    pub auth_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdyenExemptionValues {
    LowValue,
    SecureCorporate,
    TrustedBeneficiary,
    TransactionRiskAnalysis,
}

fn to_adyen_exemption(data: &common_enums::ExemptionIndicator) -> Option<AdyenExemptionValues> {
    match data {
        common_enums::ExemptionIndicator::LowValue => Some(AdyenExemptionValues::LowValue),
        common_enums::ExemptionIndicator::SecureCorporatePayment => {
            Some(AdyenExemptionValues::SecureCorporate)
        }
        common_enums::ExemptionIndicator::TrustedListing => {
            Some(AdyenExemptionValues::TrustedBeneficiary)
        }
        common_enums::ExemptionIndicator::TransactionRiskAssessment => {
            Some(AdyenExemptionValues::TransactionRiskAnalysis)
        }
        _ => None,
    }
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskData {
    #[serde(rename = "riskdata.basket.item1.itemID")]
    item_i_d: Option<String>,
    #[serde(rename = "riskdata.basket.item1.productTitle")]
    product_title: Option<String>,
    #[serde(rename = "riskdata.basket.item1.amountPerItem")]
    amount_per_item: Option<String>,
    #[serde(rename = "riskdata.basket.item1.currency")]
    currency: Option<String>,
    #[serde(rename = "riskdata.basket.item1.upc")]
    upc: Option<String>,
    #[serde(rename = "riskdata.basket.item1.brand")]
    brand: Option<String>,
    #[serde(rename = "riskdata.basket.item1.manufacturer")]
    manufacturer: Option<String>,
    #[serde(rename = "riskdata.basket.item1.category")]
    category: Option<String>,
    #[serde(rename = "riskdata.basket.item1.quantity")]
    quantity: Option<String>,
    #[serde(rename = "riskdata.basket.item1.color")]
    color: Option<String>,
    #[serde(rename = "riskdata.basket.item1.size")]
    size: Option<String>,
    #[serde(rename = "riskdata.deviceCountry")]
    device_country: Option<String>,
    #[serde(rename = "riskdata.houseNumberorName")]
    house_numberor_name: Option<String>,
    #[serde(rename = "riskdata.accountCreationDate")]
    account_creation_date: Option<String>,
    #[serde(rename = "riskdata.affiliateChannel")]
    affiliate_channel: Option<String>,
    #[serde(rename = "riskdata.avgOrderValue")]
    avg_order_value: Option<String>,
    #[serde(rename = "riskdata.deliveryMethod")]
    delivery_method: Option<String>,
    #[serde(rename = "riskdata.emailName")]
    email_name: Option<String>,
    #[serde(rename = "riskdata.emailDomain")]
    email_domain: Option<String>,
    #[serde(rename = "riskdata.lastOrderDate")]
    last_order_date: Option<String>,
    #[serde(rename = "riskdata.merchantReference")]
    merchant_reference: Option<String>,
    #[serde(rename = "riskdata.paymentMethod")]
    payment_method: Option<String>,
    #[serde(rename = "riskdata.promotionName")]
    promotion_name: Option<String>,
    #[serde(rename = "riskdata.secondaryPhoneNumber")]
    secondary_phone_number: Option<Secret<String>>,
    #[serde(rename = "riskdata.timefromLogintoOrder")]
    timefrom_loginto_order: Option<String>,
    #[serde(rename = "riskdata.totalSessionTime")]
    total_session_time: Option<String>,
    #[serde(rename = "riskdata.totalAuthorizedAmountInLast30Days")]
    total_authorized_amount_in_last30_days: Option<String>,
    #[serde(rename = "riskdata.totalOrderQuantity")]
    total_order_quantity: Option<String>,
    #[serde(rename = "riskdata.totalLifetimeValue")]
    total_lifetime_value: Option<String>,
    #[serde(rename = "riskdata.visitsMonth")]
    visits_month: Option<String>,
    #[serde(rename = "riskdata.visitsWeek")]
    visits_week: Option<String>,
    #[serde(rename = "riskdata.visitsYear")]
    visits_year: Option<String>,
    #[serde(rename = "riskdata.shipToName")]
    ship_to_name: Option<String>,
    #[serde(rename = "riskdata.first8charactersofAddressLine1Zip")]
    first8charactersof_address_line1_zip: Option<String>,
    #[serde(rename = "riskdata.affiliateOrder")]
    affiliate_order: Option<bool>,
}

#[serde_with::skip_serializing_none]
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopperName {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineItem {
    amount_excluding_tax: Option<MinorUnit>,
    amount_including_tax: Option<MinorUnit>,
    description: Option<String>,
    id: Option<String>,
    tax_amount: Option<MinorUnit>,
    quantity: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub enum Channel {
    Web,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdyenSplitData {
    amount: Option<Amount>,
    #[serde(rename = "type")]
    split_type: AdyenSplitType,
    account: Option<String>,
    reference: String,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdyenGPay {
    #[serde(rename = "googlePayToken")]
    google_pay_token: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdyenApplePay {
    #[serde(rename = "applePayToken")]
    apple_pay_token: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentType {
    Affirm,
    Afterpaytouch,
    Alipay,
    #[serde(rename = "alipay_hk")]
    AlipayHk,
    #[serde(rename = "doku_alfamart")]
    Alfamart,
    Alma,
    Applepay,
    Bizum,
    Atome,
    Blik,
    #[serde(rename = "boletobancario")]
    BoletoBancario,
    ClearPay,
    Dana,
    Eps,
    Gcash,
    Googlepay,
    #[serde(rename = "gopay_wallet")]
    GoPay,
    Ideal,
    #[serde(rename = "doku_indomaret")]
    Indomaret,
    Klarna,
    Kakaopay,
    Mbway,
    MobilePay,
    #[serde(rename = "momo_wallet")]
    Momo,
    #[serde(rename = "momo_atm")]
    MomoAtm,
    #[serde(rename = "onlineBanking_CZ")]
    OnlineBankingCzechRepublic,
    #[serde(rename = "ebanking_FI")]
    OnlineBankingFinland,
    #[serde(rename = "onlineBanking_PL")]
    OnlineBankingPoland,
    #[serde(rename = "onlineBanking_SK")]
    OnlineBankingSlovakia,
    #[serde(rename = "molpay_ebanking_fpx_MY")]
    OnlineBankingFpx,
    #[serde(rename = "molpay_ebanking_TH")]
    OnlineBankingThailand,
    #[serde(rename = "paybybank")]
    OpenBankingUK,
    #[serde(rename = "oxxo")]
    Oxxo,
    #[serde(rename = "paysafecard")]
    PaySafeCard,
    PayBright,
    Paypal,
    Scheme,
    #[serde(rename = "networkToken")]
    NetworkToken,
    #[serde(rename = "trustly")]
    Trustly,
    #[serde(rename = "touchngo")]
    TouchNGo,
    Walley,
    #[serde(rename = "wechatpayWeb")]
    WeChatPayWeb,
    #[serde(rename = "ach")]
    AchDirectDebit,
    SepaDirectDebit,
    #[serde(rename = "directdebit_GB")]
    BacsDirectDebit,
    Samsungpay,
    Twint,
    Vipps,
    Giftcard,
    Knet,
    Benefit,
    Swish,
    #[serde(rename = "doku_permata_lite_atm")]
    PermataBankTransfer,
    #[serde(rename = "doku_bca_va")]
    BcaBankTransfer,
    #[serde(rename = "doku_bni_va")]
    BniVa,
    #[serde(rename = "doku_bri_va")]
    BriVa,
    #[serde(rename = "doku_cimb_va")]
    CimbVa,
    #[serde(rename = "doku_danamon_va")]
    DanamonVa,
    #[serde(rename = "doku_mandiri_va")]
    MandiriVa,
    #[serde(rename = "econtext_seven_eleven")]
    SevenEleven,
    #[serde(rename = "econtext_stores")]
    Lawson,
    #[serde(rename = "pix")]
    Pix,
}

impl TryFrom<&common_enums::PaymentMethodType> for PaymentType {
    type Error = Error;
    fn try_from(item: &common_enums::PaymentMethodType) -> Result<Self, Self::Error> {
        match item {
            common_enums::PaymentMethodType::Card
            | common_enums::PaymentMethodType::Klarna
            | common_enums::PaymentMethodType::BancontactCard
            | common_enums::PaymentMethodType::Blik
            | common_enums::PaymentMethodType::Eps
            | common_enums::PaymentMethodType::Ideal
            | common_enums::PaymentMethodType::OnlineBankingCzechRepublic
            | common_enums::PaymentMethodType::OnlineBankingFinland
            | common_enums::PaymentMethodType::OnlineBankingPoland
            | common_enums::PaymentMethodType::OnlineBankingSlovakia
            | common_enums::PaymentMethodType::Trustly
            | common_enums::PaymentMethodType::GooglePay
            | common_enums::PaymentMethodType::AliPay
            | common_enums::PaymentMethodType::ApplePay
            | common_enums::PaymentMethodType::AliPayHk
            | common_enums::PaymentMethodType::MbWay
            | common_enums::PaymentMethodType::MobilePay
            | common_enums::PaymentMethodType::WeChatPay
            | common_enums::PaymentMethodType::SamsungPay
            | common_enums::PaymentMethodType::Affirm
            | common_enums::PaymentMethodType::AfterpayClearpay
            | common_enums::PaymentMethodType::PayBright
            | common_enums::PaymentMethodType::Walley => Ok(Self::Scheme),
            common_enums::PaymentMethodType::Sepa => Ok(Self::SepaDirectDebit),
            common_enums::PaymentMethodType::Bacs => Ok(Self::BacsDirectDebit),
            common_enums::PaymentMethodType::Ach => Ok(Self::AchDirectDebit),

            common_enums::PaymentMethodType::Paypal => Ok(Self::Paypal),
            common_enums::PaymentMethodType::Pix => Ok(Self::Pix),
            common_enums::PaymentMethodType::Givex => Ok(Self::Giftcard),
            common_enums::PaymentMethodType::PaySafeCard => Ok(Self::PaySafeCard),
            _ => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
            )
            .into()),
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
)]
#[strum(serialize_all = "PascalCase")]
#[serde(rename_all = "PascalCase")]
pub enum AdyenSplitType {
    /// Books split amount to the specified account.
    BalanceAccount,
    /// The aggregated amount of the interchange and scheme fees.
    AcquiringFees,
    /// The aggregated amount of all transaction fees.
    PaymentFee,
    /// The aggregated amount of Adyen's commission and markup fees.
    AdyenFees,
    ///  The transaction fees due to Adyen under blended rates.
    AdyenCommission,
    /// The transaction fees due to Adyen under Interchange ++ pricing.
    AdyenMarkup,
    ///  The fees paid to the issuer for each payment made with the card network.
    Interchange,
    ///  The fees paid to the card scheme for using their network.
    SchemeFee,
    /// Your platform's commission on the payment (specified in amount), booked to your liable balance account.
    Commission,
    /// Allows you and your users to top up balance accounts using direct debit, card payments, or other payment methods.
    TopUp,
    /// The value-added tax charged on the payment, booked to your platforms liable balance account.
    Vat,
}

// Wrapper types for RepeatPayment to avoid duplicate templating structs in macro
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct AdyenRepeatPaymentRequest(pub AdyenPaymentRequest<DefaultPCIHolder>);

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AdyenRepeatPaymentResponse(pub AdyenPaymentResponse);

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    amount: Amount,
    merchant_account: Secret<String>,
    payment_method: PaymentMethod<T>,
    mpi_data: Option<AdyenMpiData>,
    reference: String,
    return_url: String,
    browser_info: Option<AdyenBrowserInfo>,
    shopper_interaction: AdyenShopperInteraction,
    recurring_processing_model: Option<AdyenRecurringModel>,
    additional_data: Option<AdditionalData>,
    shopper_reference: Option<String>,
    store_payment_method: Option<bool>,
    shopper_name: Option<ShopperName>,
    #[serde(rename = "shopperIP")]
    shopper_ip: Option<Secret<String, common_utils::pii::IpAddress>>,
    shopper_locale: Option<String>,
    shopper_email: Option<common_utils::pii::Email>,
    shopper_statement: Option<String>,
    social_security_number: Option<Secret<String>>,
    telephone_number: Option<Secret<String>>,
    billing_address: Option<Address>,
    delivery_address: Option<Address>,
    country_code: Option<common_enums::CountryAlpha2>,
    line_items: Option<Vec<LineItem>>,
    channel: Option<Channel>,
    merchant_order_reference: Option<String>,
    splits: Option<Vec<AdyenSplitData>>,
    store: Option<String>,
    device_fingerprint: Option<Secret<String>>,
    metadata: Option<Secret<serde_json::Value>>,
    platform_chargeback_logic: Option<AdyenPlatformChargeBackLogicMetadata>,
    #[serde(with = "common_utils::custom_serde::iso8601::option")]
    session_validity: Option<PrimitiveDateTime>,
}

#[derive(Debug, Serialize)]
pub struct SetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(AdyenPaymentRequest<T>);

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenVoidRequest {
    merchant_account: Secret<String>,
    reference: String,
}

/// Local struct for Adyen split payment data (extracted from metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdyenSplitPaymentRequest {
    pub store: Option<String>,
    pub split_items: Vec<AdyenSplitItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdyenSplitItem {
    pub amount: Option<MinorUnit>,
    pub reference: String,
    pub split_type: AdyenSplitType,
    pub account: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdyenRouterData1<T> {
    pub amount: MinorUnit,
    pub router_data: T,
}

impl<T> TryFrom<(MinorUnit, T)> for AdyenRouterData1<T> {
    type Error = IntegrationError;
    fn try_from((amount, item): (MinorUnit, T)) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            router_data: item,
        })
    }
}

fn get_amount_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &AdyenRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Amount {
    Amount {
        currency: item.router_data.request.currency,
        value: item.router_data.request.minor_amount.to_owned(),
    }
}

pub struct AdyenAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) merchant_account: Secret<String>,
    #[allow(dead_code)]
    pub(super) review_key: Option<Secret<String>>,
    #[allow(dead_code)]
    pub(super) endpoint_prefix: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AdyenAuthType {
    type Error = IntegrationError;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Adyen {
                api_key,
                merchant_account,
                review_key,
                endpoint_prefix,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_account: merchant_account.to_owned(),
                review_key: review_key.to_owned(),
                endpoint_prefix: endpoint_prefix.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }),
        }
    }
}

fn get_adyen_card_network(card_network: common_enums::CardNetwork) -> Option<CardBrand> {
    match card_network {
        common_enums::CardNetwork::Visa => Some(CardBrand::Visa),
        common_enums::CardNetwork::Mastercard => Some(CardBrand::MC),
        common_enums::CardNetwork::AmericanExpress => Some(CardBrand::Amex),
        common_enums::CardNetwork::JCB => Some(CardBrand::Jcb),
        common_enums::CardNetwork::DinersClub => Some(CardBrand::Diners),
        common_enums::CardNetwork::Discover => Some(CardBrand::Discover),
        common_enums::CardNetwork::CartesBancaires => Some(CardBrand::Cartebancaire),
        common_enums::CardNetwork::UnionPay => Some(CardBrand::Cup),
        common_enums::CardNetwork::Maestro => Some(CardBrand::Maestro),
        common_enums::CardNetwork::RuPay => Some(CardBrand::Rupay),
        common_enums::CardNetwork::Star => Some(CardBrand::Star),
        common_enums::CardNetwork::Accel => Some(CardBrand::Accel),
        common_enums::CardNetwork::Pulse => Some(CardBrand::Pulse),
        common_enums::CardNetwork::Nyce => Some(CardBrand::Nyce),
        common_enums::CardNetwork::Interac => None,
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&Card<T>, Option<Secret<String>>)> for AdyenPaymentMethod<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        (card, card_holder_name): (&Card<T>, Option<Secret<String>>),
    ) -> Result<Self, Self::Error> {
        // Only set brand for cobadged cards
        let brand = if card.card_number.is_cobadged_card().change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )? {
            // Use the detected card network from the card data
            card.card_network.clone().and_then(get_adyen_card_network)
        } else {
            None
        };

        let adyen_card = AdyenCard {
            number: card.card_number.clone(),
            expiry_month: card.card_exp_month.clone(),
            expiry_year: card.get_expiry_year_4_digit(),
            cvc: Some(card.card_cvc.clone()),
            holder_name: card_holder_name,
            brand,
            network_payment_reference: None,
        };
        Ok(Self::AdyenCard(Box::new(adyen_card)))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&NetworkTokenData, Option<Secret<String>>)> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (token_data, card_holder_name): (&NetworkTokenData, Option<Secret<String>>),
    ) -> Result<Self, Self::Error> {
        let adyen_network_token = AdyenNetworkTokenData {
            number: token_data.get_network_token(),
            expiry_month: token_data.get_network_token_expiry_month(),
            expiry_year: token_data.get_expiry_year_4_digit(),
            holder_name: card_holder_name,
            brand: None,                     // Only required for NTI mandate payments
            network_payment_reference: None, // Only for mandate payments
        };
        Ok(Self::NetworkToken(Box::new(adyen_network_token)))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &WalletData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &WalletData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        let (wallet_data, _item) = value;
        match wallet_data {
            WalletData::GooglePay(data) => {
                let google_pay_wallet_data = match &data.tokenization_data {
                    GpayTokenizationData::Decrypted(decrypt_data) => {
                        let expiry_year = decrypt_data
                            .get_four_digit_expiry_year()
                            .change_context(IntegrationError::InvalidDataFormat {
                                field_name: "expiry_year",
                                context: Default::default(),
                            })?;

                        let expiry_month = decrypt_data.get_expiry_month().change_context(
                            IntegrationError::InvalidDataFormat {
                                field_name: "expiry_month",
                                context: Default::default(),
                            },
                        )?;

                        let google_pay_decrypt_data = AdyenGooglePayDecryptData {
                            number: decrypt_data.application_primary_account_number.clone(),
                            expiry_month,
                            expiry_year,
                            brand: GOOGLE_PAY_BRAND.to_string(),
                        };

                        Self::GooglePayDecrypt(Box::new(google_pay_decrypt_data))
                    }
                    GpayTokenizationData::Encrypted(_) => {
                        let gpay_data = AdyenGPay {
                            google_pay_token: Secret::new(
                                data.tokenization_data
                                    .get_encrypted_google_pay_token()
                                    .change_context(IntegrationError::InvalidDataFormat {
                                        field_name: "google_pay_token",
                                        context: Default::default(),
                                    })?,
                            ),
                        };

                        Self::Gpay(Box::new(gpay_data))
                    }
                };

                Ok(google_pay_wallet_data)
            }
            WalletData::ApplePay(data) => {
                let apple_pay_wallet_data = match &data.payment_data {
                    ApplePayPaymentData::Decrypted(decrypt_data) => {
                        let expiry_year_4_digit = decrypt_data.get_four_digit_expiry_year();
                        let exp_month = decrypt_data.get_expiry_month();

                        let apple_pay_decrypt_data = AdyenApplePayDecryptData {
                            number: decrypt_data.application_primary_account_number.clone(),
                            expiry_month: exp_month,
                            expiry_year: expiry_year_4_digit,
                            brand: data.payment_method.network.clone(),
                        };

                        Self::ApplePayDecrypt(Box::new(apple_pay_decrypt_data))
                    }
                    ApplePayPaymentData::Encrypted(encrypted_str) => {
                        let apple_pay_data = AdyenApplePay {
                            apple_pay_token: Secret::new(encrypted_str.clone()),
                        };

                        Self::ApplePay(Box::new(apple_pay_data))
                    }
                };

                Ok(apple_pay_wallet_data)
            }

            WalletData::PaypalRedirect(_)
            | WalletData::AmazonPayRedirect(_)
            | WalletData::Paze(_)
            | WalletData::RevolutPay(_)
            | WalletData::AliPayRedirect(_)
            | WalletData::AliPayHkRedirect(_)
            | WalletData::GoPayRedirect(_)
            | WalletData::KakaoPayRedirect(_)
            | WalletData::GcashRedirect(_)
            | WalletData::MomoRedirect(_)
            | WalletData::TouchNGoRedirect(_)
            | WalletData::MbWayRedirect(_)
            | WalletData::MobilePayRedirect(_)
            | WalletData::WeChatPayRedirect(_)
            | WalletData::SamsungPay(_)
            | WalletData::TwintRedirect { .. }
            | WalletData::VippsRedirect { .. }
            | WalletData::DanaRedirect { .. }
            | WalletData::SwishQr(_)
            | WalletData::AliPayQr(_)
            | WalletData::ApplePayRedirect(_)
            | WalletData::ApplePayThirdPartySdk(_)
            | WalletData::GooglePayRedirect(_)
            | WalletData::GooglePayThirdPartySdk(_)
            | WalletData::PaypalSdk(_)
            | WalletData::WeChatPayQr(_)
            | WalletData::CashappQr(_)
            | WalletData::Mifinity(_)
            | WalletData::BluecodeRedirect { .. }
            | WalletData::MbWay(_)
            | WalletData::Satispay(_)
            | WalletData::Wero(_) => {
                Err(IntegrationError::not_implemented("payment_method").into())
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &VoucherData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (voucher_data, item): (
            &VoucherData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        match voucher_data {
            VoucherData::Boleto(_) => Ok(Self::BoletoBancario),
            VoucherData::Alfamart(_) => Ok(Self::Alfamart(Box::new(DokuBankData::try_from(item)?))),
            VoucherData::Indomaret(_) => {
                Ok(Self::Indomaret(Box::new(DokuBankData::try_from(item)?)))
            }
            VoucherData::Oxxo => Ok(Self::Oxxo),
            VoucherData::SevenEleven(_) => {
                Ok(Self::SevenEleven(Box::new(JCSVoucherData::try_from(item)?)))
            }
            VoucherData::Lawson(_) => Ok(Self::Lawson(Box::new(JCSVoucherData::try_from(item)?))),
            VoucherData::MiniStop(_) => {
                Ok(Self::MiniStop(Box::new(JCSVoucherData::try_from(item)?)))
            }
            VoucherData::FamilyMart(_) => {
                Ok(Self::FamilyMart(Box::new(JCSVoucherData::try_from(item)?)))
            }
            VoucherData::Seicomart(_) => {
                Ok(Self::Seicomart(Box::new(JCSVoucherData::try_from(item)?)))
            }
            VoucherData::PayEasy(_) => Ok(Self::PayEasy(Box::new(JCSVoucherData::try_from(item)?))),
            VoucherData::Efecty
            | VoucherData::PagoEfectivo
            | VoucherData::RedCompra
            | VoucherData::RedPagos => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
            )
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for JCSVoucherData
{
    type Error = Error;
    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            first_name: item.resource_common_data.get_billing_first_name()?,
            last_name: item.resource_common_data.get_optional_billing_last_name(),
            shopper_email: item.resource_common_data.get_billing_email()?,
            telephone_number: item.resource_common_data.get_billing_phone_number()?,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&GiftCardData> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(gift_card_data: &GiftCardData) -> Result<Self, Self::Error> {
        match gift_card_data {
            GiftCardData::PaySafeCard {} => Ok(Self::PaySafeCard),
            GiftCardData::Givex(givex_data) => {
                let gift_card_pm = AdyenGiftCardData {
                    brand: GiftCardBrand::Givex,
                    number: givex_data.number.clone(),
                    cvc: givex_data.cvc.clone(),
                };
                Ok(Self::AdyenGiftCard(Box::new(gift_card_pm)))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &BankRedirectData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (bank_redirect_data, item): (
            &BankRedirectData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        match bank_redirect_data {
            BankRedirectData::BancontactCard {
                card_number,
                card_exp_month,
                card_exp_year,
                ..
            } => {
                let card_holder_name = item.resource_common_data.get_optional_billing_full_name();
                let card_num = card_number
                    .as_ref()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "bancontact_card.card_number",
                        context: Default::default(),
                    })?
                    .clone();
                let raw_card_number = RawCardNumber(card_num);
                Ok(Self::BancontactCard(Box::new(AdyenCard {
                    number: raw_card_number,
                    expiry_month: card_exp_month
                        .as_ref()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bancontact_card.card_exp_month",
                            context: Default::default(),
                        })?
                        .clone(),
                    expiry_year: card_exp_year
                        .as_ref()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bancontact_card.card_exp_year",
                            context: Default::default(),
                        })?
                        .clone(),
                    holder_name: card_holder_name,
                    cvc: None,
                    brand: Some(CardBrand::Bcmc),
                    network_payment_reference: None,
                })))
            }
            BankRedirectData::Bizum { .. } => Ok(Self::Bizum),
            BankRedirectData::Blik { blik_code } => Ok(Self::Blik(Box::new(BlikRedirectionData {
                blik_code: Secret::new(blik_code.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "blik_code",
                        context: Default::default(),
                    },
                )?),
            }))),
            BankRedirectData::Eps { bank_name, .. } => {
                Ok(Self::Eps(Box::new(BankRedirectionWithIssuer {
                    issuer: Some(
                        AdyenTestBankNames::try_from(&bank_name.ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "eps.bank_name",
                                context: Default::default(),
                            },
                        )?)?
                        .0,
                    ),
                })))
            }
            BankRedirectData::Ideal { .. } => Ok(Self::Ideal),
            BankRedirectData::OnlineBankingCzechRepublic { issuer } => Ok(
                Self::OnlineBankingCzechRepublic(Box::new(OnlineBankingCzechRepublicData {
                    issuer: OnlineBankingCzechRepublicBanks::try_from(issuer)?,
                })),
            ),
            BankRedirectData::OnlineBankingFinland { .. } => Ok(Self::OnlineBankingFinland),
            BankRedirectData::OnlineBankingPoland { issuer } => Ok(Self::OnlineBankingPoland(
                Box::new(OnlineBankingPolandData {
                    issuer: OnlineBankingPolandBanks::try_from(issuer)?,
                }),
            )),
            BankRedirectData::OnlineBankingSlovakia { issuer } => Ok(Self::OnlineBankingSlovakia(
                Box::new(OnlineBankingSlovakiaData {
                    issuer: OnlineBankingSlovakiaBanks::try_from(issuer)?,
                }),
            )),
            BankRedirectData::OnlineBankingFpx { issuer } => {
                Ok(Self::OnlineBankingFpx(Box::new(OnlineBankingFpxData {
                    issuer: OnlineBankingFpxIssuer::try_from(issuer)?,
                })))
            }
            BankRedirectData::OnlineBankingThailand { issuer } => Ok(Self::OnlineBankingThailand(
                Box::new(OnlineBankingThailandData {
                    issuer: OnlineBankingThailandIssuer::try_from(issuer)?,
                }),
            )),
            BankRedirectData::OpenBankingUk { issuer, .. } => {
                Ok(Self::OpenBankingUK(Box::new(OpenBankingUKData {
                    issuer: match issuer {
                        Some(bank_name) => Some(OpenBankingUKIssuer::try_from(bank_name)?),
                        None => None,
                    },
                })))
            }
            BankRedirectData::Trustly { .. } => Ok(Self::Trustly),
            BankRedirectData::Giropay { .. }
            | BankRedirectData::Eft { .. }
            | BankRedirectData::Interac { .. }
            | BankRedirectData::LocalBankRedirect {}
            | BankRedirectData::Przelewy24 { .. }
            | BankRedirectData::Sofort { .. }
            | BankRedirectData::OpenBanking { .. } => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
            )
            .into()),
        }
    }
}

// TryFrom implementation for extracting DokuBankData from router data
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for DokuBankData
{
    type Error = Error;
    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let first_name = item.resource_common_data.get_billing_first_name()?;
        let last_name = item.resource_common_data.get_optional_billing_last_name();
        let shopper_email = item.resource_common_data.get_billing_email()?;
        Ok(Self {
            first_name,
            last_name,
            shopper_email,
        })
    }
}

// TryFrom implementation for converting BankTransferData to AdyenPaymentMethod
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &BankTransferData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (bank_transfer_data, item): (
            &BankTransferData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        match bank_transfer_data {
            BankTransferData::PermataBankTransfer {} => Ok(Self::PermataBankTransfer(Box::new(
                DokuBankData::try_from(item)?,
            ))),
            BankTransferData::BcaBankTransfer {} => Ok(Self::BcaBankTransfer(Box::new(
                DokuBankData::try_from(item)?,
            ))),
            BankTransferData::BniVaBankTransfer {} => {
                Ok(Self::BniVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::BriVaBankTransfer {} => {
                Ok(Self::BriVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::CimbVaBankTransfer {} => {
                Ok(Self::CimbVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::DanamonVaBankTransfer {} => {
                Ok(Self::DanamonVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::MandiriVaBankTransfer {} => {
                Ok(Self::MandiriVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::Pix { .. } => Ok(Self::Pix),
            BankTransferData::AchBankTransfer {}
            | BankTransferData::SepaBankTransfer {}
            | BankTransferData::BacsBankTransfer {}
            | BankTransferData::MultibancoBankTransfer {}
            | BankTransferData::Pse {}
            | BankTransferData::LocalBankTransfer { .. }
            | BankTransferData::InstantBankTransfer {}
            | BankTransferData::IndonesianBankTransfer { .. }
            | BankTransferData::InstantBankTransferFinland {}
            | BankTransferData::InstantBankTransferPoland {} => {
                Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("Adyen"),
                )
                .into())
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&CardRedirectData> for AdyenPaymentMethod<T>
{
    type Error = Error;

    fn try_from(card_redirect_data: &CardRedirectData) -> Result<Self, Self::Error> {
        match card_redirect_data {
            CardRedirectData::Knet {} => Ok(Self::Knet),
            CardRedirectData::Benefit {} => Ok(Self::Benefit),
            CardRedirectData::MomoAtm {} => Ok(Self::MomoAtm),
            CardRedirectData::CardRedirect {} => {
                Err(IntegrationError::not_implemented("payment_method").into())
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &BankDebitData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (bank_debit_data, item): (
            &BankDebitData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        match bank_debit_data {
            BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                ..
            } => Ok(Self::AchDirectDebit(Box::new(AchDirectDebitData {
                bank_account_number: account_number.clone(),
                bank_location_id: routing_number.clone(),
                owner_name: item.resource_common_data.get_billing_full_name()?,
            }))),
            BankDebitData::SepaBankDebit { iban, .. } => {
                Ok(Self::SepaDirectDebit(Box::new(SepaDirectDebitData {
                    owner_name: item.resource_common_data.get_billing_full_name()?,
                    iban_number: iban.clone(),
                })))
            }
            BankDebitData::BacsBankDebit {
                account_number,
                sort_code,
                ..
            } => {
                let testing_data = item
                    .request
                    .get_connector_testing_data()
                    .map(AdyenTestingData::try_from)
                    .transpose()
                    .map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })?;
                let test_holder_name = testing_data.and_then(|test_data| test_data.holder_name);
                Ok(Self::BacsDirectDebit(Box::new(BacsDirectDebitData {
                    bank_account_number: account_number.clone(),
                    bank_location_id: sort_code.clone(),
                    holder_name: test_holder_name
                        .unwrap_or(item.resource_common_data.get_billing_full_name()?),
                })))
            }
            BankDebitData::BecsBankDebit { .. } | BankDebitData::SepaGuaranteedBankDebit { .. } => {
                Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("Adyen"),
                )
                .into())
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        &PayLaterData,
    )> for AdyenPaymentMethod<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (router_data, pay_later_data): (
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            &PayLaterData,
        ),
    ) -> Result<Self, Self::Error> {
        match pay_later_data {
            PayLaterData::KlarnaRedirect { .. } => {
                router_data
                    .resource_common_data
                    .get_billing_email()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.email",
                        context: Default::default(),
                    })?;
                router_data.resource_common_data.customer_id.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "customer_id",
                        context: Default::default(),
                    },
                )?;
                router_data
                    .resource_common_data
                    .get_billing_country()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.country",
                        context: Default::default(),
                    })?;
                Ok(Self::Klarna)
            }
            PayLaterData::KlarnaSdk { token } => {
                if token.is_empty() {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "token",
                        context: Default::default(),
                    }
                    .into());
                }
                Ok(Self::Klarna)
            }
            PayLaterData::AffirmRedirect { .. } => {
                router_data
                    .resource_common_data
                    .get_billing_email()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.email",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_full_name()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.full_name",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.phone",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.address",
                        context: Default::default(),
                    })?;
                Ok(Self::AdyenAffirm)
            }
            PayLaterData::AfterpayClearpayRedirect { .. } => {
                router_data
                    .resource_common_data
                    .get_billing_email()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.email",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_full_name()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.full_name",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.address",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_shipping_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "shipping.address",
                        context: Default::default(),
                    })?;
                let country = router_data
                    .resource_common_data
                    .get_billing_country()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.country",
                        context: Default::default(),
                    })?;
                match country {
                    common_enums::CountryAlpha2::IT
                    | common_enums::CountryAlpha2::FR
                    | common_enums::CountryAlpha2::ES
                    | common_enums::CountryAlpha2::GB => Ok(Self::ClearPay),
                    _ => Ok(Self::AfterPay),
                }
            }
            PayLaterData::PayBrightRedirect { .. } => {
                router_data
                    .resource_common_data
                    .get_billing_full_name()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.full_name",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.phone",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_email()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.email",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.address",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_shipping_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "shipping.address",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_country()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.country",
                        context: Default::default(),
                    })?;
                Ok(Self::PayBright)
            }
            PayLaterData::WalleyRedirect { .. } => {
                router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.phone",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_email()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.email",
                        context: Default::default(),
                    })?;
                Ok(Self::Walley)
            }
            PayLaterData::AlmaRedirect { .. } => {
                router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.phone",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_email()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.email",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.address",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_shipping_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "shipping.address",
                        context: Default::default(),
                    })?;
                Ok(Self::AlmaPayLater)
            }
            PayLaterData::AtomeRedirect { .. } => {
                router_data
                    .resource_common_data
                    .get_billing_email()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.email",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_full_name()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.full_name",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.phone",
                        context: Default::default(),
                    })?;
                router_data
                    .resource_common_data
                    .get_billing_address()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "billing.address",
                        context: Default::default(),
                    })?;

                Ok(Self::Atome)
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &Card<T>,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &Card<T>,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, card_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;

        let return_url = item.router_data.request.get_router_return_url()?;

        let billing_address =
            get_address_info(item.router_data.resource_common_data.get_optional_billing())
                .and_then(Result::ok);

        let testing_data = item
            .router_data
            .request
            .get_connector_testing_data()
            .map(AdyenTestingData::try_from)
            .transpose()?;
        let test_holder_name = testing_data.and_then(|test_data| test_data.holder_name);
        let card_holder_name = test_holder_name.or(item
            .router_data
            .resource_common_data
            .get_optional_billing_full_name());

        let additional_data = get_additional_data(&item.router_data);

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let store = adyen_metadata.store.clone(); // no split payment support yet
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        let country_code =
            get_country_code(item.router_data.resource_common_data.get_optional_billing());

        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((card_data, card_holder_name))?,
        ));

        let mpi_data =
            if let Some(auth_data) = item.router_data.request.authentication_data.as_ref() {
                let (cavv_algorithm, challenge_cancel, risk_score) =
                    match &item.router_data.request.payment_method_data {
                        PaymentMethodData::Card(card)
                            if matches!(
                                card.card_network,
                                Some(common_enums::CardNetwork::CartesBancaires)
                            ) =>
                        {
                            let cartes_params = auth_data
                                .network_params
                                .as_ref()
                                .and_then(|net| net.cartes_bancaires.as_ref());

                            (
                                cartes_params.as_ref().map(|cb| cb.cavv_algorithm.clone()),
                                cartes_params.as_ref().map(|cb| cb.cb_exemption.clone()),
                                cartes_params.as_ref().map(|cb| cb.cb_score.to_string()),
                            )
                        }
                        _ => (None, None, None),
                    };

                Some(AdyenMpiData {
                    directory_response: auth_data.trans_status.clone().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "three_ds_data.trans_status",
                            context: Default::default(),
                        },
                    )?,
                    authentication_response: auth_data.trans_status.clone().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "three_ds_data.trans_status",
                            context: Default::default(),
                        },
                    )?,
                    cavv: auth_data.cavv.clone(),
                    token_authentication_verification_value: None,
                    eci: auth_data.eci.clone(),
                    ds_trans_id: auth_data.ds_trans_id.clone(),
                    three_ds_version: auth_data.message_version.clone(),
                    cavv_algorithm,
                    challenge_cancel,
                    risk_score,
                })
            } else {
                None
            };

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info: get_browser_info(&item.router_data)?,
            additional_data,
            mpi_data,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            shopper_name: get_shopper_name(
                item.router_data.resource_common_data.get_optional_billing(),
            ),
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address,
            delivery_address: get_address_info(
                item.router_data
                    .resource_common_data
                    .get_optional_shipping(),
            )
            .and_then(Result::ok),
            country_code,
            line_items: None,
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: get_shopper_statement(&item.router_data),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits: None,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &WalletData,
    )> for AdyenPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &WalletData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, wallet_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((wallet_data, &item.router_data))?,
        ));
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;
        let return_url = item.router_data.request.get_router_return_url()?;
        let additional_data = get_additional_data(&item.router_data);

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        let country_code =
            get_country_code(item.router_data.resource_common_data.get_optional_billing());

        let mpi_data = match wallet_data {
            WalletData::ApplePay(apple_data) => {
                if let ApplePayPaymentData::Decrypted(decrypt_data) = &apple_data.payment_data {
                    Some(AdyenMpiData {
                        directory_response: common_enums::TransactionStatus::Success,
                        authentication_response: common_enums::TransactionStatus::Success,
                        cavv: Some(decrypt_data.payment_data.online_payment_cryptogram.clone()),
                        token_authentication_verification_value: None,
                        eci: decrypt_data.payment_data.eci_indicator.clone(),
                        ds_trans_id: None,
                        three_ds_version: None,
                        challenge_cancel: None,
                        risk_score: None,
                        cavv_algorithm: None,
                    })
                } else {
                    None
                }
            }
            WalletData::GooglePay(gpay_data) => {
                if let GpayTokenizationData::Decrypted(decrypt_data) = &gpay_data.tokenization_data
                {
                    match (
                        decrypt_data.cryptogram.clone(),
                        decrypt_data.eci_indicator.clone(),
                    ) {
                        (Some(cryptogram), Some(eci_indicator)) => Some(AdyenMpiData {
                            directory_response: common_enums::TransactionStatus::Success,
                            authentication_response: common_enums::TransactionStatus::Success,
                            cavv: Some(cryptogram),
                            token_authentication_verification_value: None,
                            eci: Some(eci_indicator),
                            ds_trans_id: None,
                            three_ds_version: None,
                            challenge_cancel: None,
                            risk_score: None,
                            cavv_algorithm: None,
                        }),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info: get_browser_info(&item.router_data)?,
            additional_data,
            mpi_data,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            shopper_name: get_shopper_name(
                item.router_data
                    .resource_common_data
                    .address
                    .get_payment_billing(),
            ),
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address: get_address_info(
                item.router_data
                    .resource_common_data
                    .address
                    .get_payment_billing(),
            )
            .and_then(Result::ok),
            delivery_address: get_address_info(
                item.router_data
                    .resource_common_data
                    .get_optional_shipping(),
            )
            .and_then(Result::ok),
            country_code,
            line_items: None,
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: item
                .router_data
                .request
                .billing_descriptor
                .clone()
                .and_then(|descriptor| descriptor.statement_descriptor),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store: None,
            splits: None,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &BankRedirectData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &BankRedirectData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, bank_redirect_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;
        let browser_info = get_browser_info(&item.router_data)?;
        let additional_data = get_additional_data(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((bank_redirect_data, &item.router_data))?,
        ));
        let (shopper_locale, country) = get_redirect_extra_details(&item.router_data)?;
        let billing_address = get_address_info(
            item.router_data
                .resource_common_data
                .address
                .get_payment_billing(),
        )
        .and_then(Result::ok);
        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());

        // Split payments not currently handled for bank transfers
        let store = adyen_metadata.store.clone();
        let splits = None;
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();

        let delivery_address = get_address_info(
            item.router_data
                .resource_common_data
                .get_optional_shipping(),
        )
        .and_then(Result::ok);
        let telephone_number = item
            .router_data
            .resource_common_data
            .get_optional_billing_phone_number();
        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info,
            additional_data,
            mpi_data: None,
            telephone_number,
            shopper_name: None,
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale,
            social_security_number: None,
            billing_address,
            delivery_address,
            country_code: country,
            line_items: Some(get_line_items(&item)),
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: item
                .router_data
                .request
                .billing_descriptor
                .clone()
                .and_then(|descriptor| descriptor.statement_descriptor),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &BankDebitData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &BankDebitData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, bank_debit_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;
        let return_url = item.router_data.request.get_router_return_url()?;

        let billing_address =
            get_address_info(item.router_data.resource_common_data.get_optional_billing())
                .and_then(Result::ok);

        let additional_data = get_additional_data(&item.router_data);

        let adyen_metadata = get_adyen_metadata(
            item.router_data
                .request
                .metadata
                .clone()
                .map(|secret| secret.expose()),
        );
        let store = adyen_metadata.store.clone();
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        let country_code =
            get_country_code(item.router_data.resource_common_data.get_optional_billing());

        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((bank_debit_data, &item.router_data))?,
        ));

        // For ACH bank debit, override state_or_province with state code (e.g., "CA" instead of "California")
        let billing_address = match bank_debit_data {
            BankDebitData::AchBankDebit { .. } => billing_address.map(|mut addr| {
                addr.state_or_province = item
                    .router_data
                    .resource_common_data
                    .get_optional_billing()
                    .and_then(|b| b.address.as_ref())
                    .and_then(|address| address.to_state_code_as_optional().ok().flatten())
                    .or(addr.state_or_province);
                addr
            }),
            BankDebitData::SepaBankDebit { .. }
            | BankDebitData::BacsBankDebit { .. }
            | BankDebitData::SepaGuaranteedBankDebit { .. }
            | BankDebitData::BecsBankDebit { .. } => billing_address,
        };

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info: get_browser_info(&item.router_data)?,
            additional_data,
            mpi_data: None,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            shopper_name: None,
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address,
            delivery_address: get_address_info(
                item.router_data
                    .resource_common_data
                    .get_optional_shipping(),
            )
            .and_then(Result::ok),
            country_code,
            line_items: None,
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: item
                .router_data
                .request
                .billing_descriptor
                .clone()
                .and_then(|descriptor| descriptor.statement_descriptor),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits: None,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

// TryFrom implementation for converting BankTransferData to AdyenPaymentRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &BankTransferData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &BankTransferData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, bank_transfer_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((bank_transfer_data, &item.router_data))?,
        ));
        let billing_address = get_address_info(
            item.router_data
                .resource_common_data
                .address
                .get_payment_billing(),
        )
        .and_then(Result::ok);
        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());

        // Split payments not currently handled for bank transfers
        let store = adyen_metadata.store.clone();
        let splits = None;
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();

        let delivery_address = get_address_info(
            item.router_data
                .resource_common_data
                .get_optional_shipping(),
        )
        .and_then(Result::ok);
        let telephone_number = item
            .router_data
            .resource_common_data
            .get_optional_billing_phone_number();

        // Extract Pix-specific fields (session_validity and social_security_number)
        // This aligns with Hyperswitch implementation for Adyen Pix payments
        let (session_validity, social_security_number) = match bank_transfer_data {
            BankTransferData::Pix {
                cpf,
                cnpj,
                expiry_date,
                ..
            } => {
                // Validate expiry_date doesn't exceed 5 days from now (Adyen requirement)
                expiry_date
                    .map(|expiry| -> CustomResult<(), IntegrationError> {
                        let now = OffsetDateTime::now_utc();
                        let max_expiry = now + Duration::days(5);
                        let max_expiry_primitive =
                            PrimitiveDateTime::new(max_expiry.date(), max_expiry.time());

                        if expiry > max_expiry_primitive {
                            Err(IntegrationError::InvalidDataFormat {
                                field_name: "expiry_date cannot be more than 5 days from now",
                                context: Default::default(),
                            }
                            .into())
                        } else {
                            Ok(())
                        }
                    })
                    .transpose()?;

                // Use CPF or CNPJ as social security number (Brazilian tax ID)
                (*expiry_date, cpf.clone().or_else(|| cnpj.clone()))
            }
            _ => (None, None),
        };

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model: None,
            browser_info: None,
            additional_data: None,
            mpi_data: None,
            telephone_number,
            shopper_name: None,
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number,
            billing_address,
            delivery_address,
            country_code: None,
            line_items: None,
            shopper_reference: None,
            store_payment_method: None,
            channel: None,
            shopper_statement: item
                .router_data
                .request
                .billing_descriptor
                .clone()
                .and_then(|descriptor| descriptor.statement_descriptor),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity,
        })
    }
}

// TryFrom implementation for converting CardRedirectData to AdyenPaymentRequest
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &CardRedirectData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &CardRedirectData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, card_redirect_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from(card_redirect_data)?,
        ));
        let billing_address =
            get_address_info(item.router_data.resource_common_data.get_optional_billing())
                .and_then(Result::ok);
        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());

        let (store, splits) = get_adyen_split_request(
            &item.router_data.request.metadata,
            &adyen_metadata.store,
            item.router_data.request.currency,
        );
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();

        let delivery_address = get_address_info(
            item.router_data
                .resource_common_data
                .get_optional_shipping(),
        )
        .and_then(Result::ok);
        let telephone_number = item
            .router_data
            .resource_common_data
            .get_optional_billing_phone_number();
        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model: None,
            browser_info: None,
            additional_data: None,
            mpi_data: None,
            telephone_number,
            shopper_name: get_shopper_name(
                item.router_data.resource_common_data.get_optional_billing(),
            ),
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address,
            delivery_address,
            country_code: None,
            line_items: None,
            shopper_reference: None,
            store_payment_method: None,
            channel: None,
            shopper_statement: get_shopper_statement(&item.router_data),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .expose_option()
                .map(|value| Secret::new(filter_adyen_metadata(value))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &GiftCardData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &GiftCardData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, gift_card_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from(gift_card_data)?,
        ));
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        let billing_address = get_address_info(
            item.router_data
                .resource_common_data
                .address
                .get_payment_billing(),
        )
        .and_then(Result::ok);
        let delivery_address = get_address_info(
            item.router_data
                .resource_common_data
                .get_optional_shipping(),
        )
        .and_then(Result::ok);

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model: None,
            browser_info: None,
            additional_data: None,
            mpi_data: None,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            shopper_name: None,
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address,
            delivery_address,
            country_code: None,
            line_items: None,
            shopper_reference: None,
            store_payment_method: None,
            channel: None,
            shopper_statement: get_shopper_statement(&item.router_data),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store: None,
            splits: None,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &NetworkTokenData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &NetworkTokenData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, token_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;

        let return_url = item.router_data.request.get_router_return_url()?;

        let billing_address =
            get_address_info(item.router_data.resource_common_data.get_optional_billing())
                .and_then(Result::ok);

        let card_holder_name = item
            .router_data
            .resource_common_data
            .get_optional_billing_full_name();

        let additional_data = get_additional_data(&item.router_data);

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let store = adyen_metadata.store.clone();
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        let country_code =
            get_country_code(item.router_data.resource_common_data.get_optional_billing());

        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((token_data, card_holder_name))?,
        ));

        // Cryptogram is REQUIRED for network token payments
        let cryptogram =
            token_data
                .get_cryptogram()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "network_token_data.token_cryptogram",
                    context: Default::default(),
                })?;

        let mpi_data = Some(AdyenMpiData {
            directory_response: common_enums::TransactionStatus::Success,
            authentication_response: common_enums::TransactionStatus::Success,
            cavv: None,
            token_authentication_verification_value: Some(cryptogram),
            eci: Some("02".to_string()),
            ds_trans_id: None,
            three_ds_version: None,
            challenge_cancel: None,
            risk_score: None,
            cavv_algorithm: None,
        });

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info: get_browser_info(&item.router_data)?,
            additional_data,
            mpi_data,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            shopper_name: get_shopper_name(
                item.router_data.resource_common_data.get_optional_billing(),
            ),
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address,
            delivery_address: get_address_info(
                item.router_data
                    .resource_common_data
                    .get_optional_shipping(),
            )
            .and_then(Result::ok),
            country_code,
            line_items: None,
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: get_shopper_statement(&item.router_data),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits: None,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &PayLaterData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &PayLaterData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, pay_later_data) = value;
        let payment_method = AdyenPaymentMethod::try_from((&item.router_data, pay_later_data))?;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;
        let additional_data = get_additional_data(&item.router_data);
        let payment_method_wrapper = PaymentMethod::AdyenPaymentMethod(Box::new(payment_method));
        let billing_address = get_address_info(
            item.router_data
                .resource_common_data
                .address
                .get_payment_billing(),
        )
        .and_then(Result::ok);
        let delivery_address = get_address_info(
            item.router_data
                .resource_common_data
                .get_optional_shipping(),
        )
        .and_then(Result::ok);

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());

        let country_code =
            get_country_code(item.router_data.resource_common_data.get_optional_billing());

        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;

        let (store, splits) = get_adyen_split_request(
            &item.router_data.request.metadata,
            &adyen_metadata.store,
            item.router_data.request.currency,
        );

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method: payment_method_wrapper,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info: get_browser_info(&item.router_data)?,
            additional_data,
            mpi_data: None,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_billing_phone_number()
                .ok(),
            shopper_name: get_shopper_name(
                item.router_data.resource_common_data.get_optional_billing(),
            ),
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address,
            delivery_address,
            country_code,
            line_items: Some(get_line_items(&item)),
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: get_shopper_statement(&item.router_data),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits,
            device_fingerprint: adyen_metadata.device_fingerprint.clone(),
            platform_chargeback_logic: adyen_metadata.platform_chargeback_logic.clone(),
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            session_validity: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &VoucherData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &VoucherData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, voucher_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((voucher_data, &item.router_data))?,
        ));
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;

        let social_security_number = get_social_security_number(voucher_data);

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        let billing_address = get_address_info(
            item.router_data
                .resource_common_data
                .address
                .get_payment_billing(),
        )
        .and_then(Result::ok);
        let delivery_address = get_address_info(
            item.router_data
                .resource_common_data
                .get_optional_shipping(),
        )
        .and_then(Result::ok);

        let shopper_name =
            get_shopper_name(item.router_data.resource_common_data.get_optional_billing());
        let shopper_email = item
            .router_data
            .resource_common_data
            .get_optional_billing_email();
        let telephone_number = item
            .router_data
            .resource_common_data
            .get_optional_billing_phone_number();
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;
        let (store, splits) = get_adyen_split_request(
            &item.router_data.request.metadata,
            &adyen_metadata.store,
            item.router_data.request.currency,
        );

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info: get_browser_info(&item.router_data)?,
            additional_data: get_additional_data(&item.router_data),
            mpi_data: None,
            telephone_number,
            shopper_name,
            shopper_email,
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number,
            billing_address,
            delivery_address,
            country_code: None,
            line_items: None,
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: get_shopper_statement(&item.router_data),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store,
            splits,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        })
    }
}

/// Validates social_security_number (Brazilian social security number) for Boleto
/// Rules: exactly 11 digits (0-9)
fn is_valid_social_security_number(social_security_number: &str) -> bool {
    match (
        social_security_number.len() == 11,
        social_security_number.chars().all(|c| c.is_ascii_digit()),
    ) {
        (false, _) => {
            tracing::warn!(
                "Invalid social_security_number: must be exactly 11
  digits, got {}",
                social_security_number.len()
            );
            false
        }
        (_, false) => {
            tracing::warn!(
                "Invalid social_security_number: must contain
   only digits (0-9)"
            );
            false
        }
        (true, true) => true,
    }
}

fn get_social_security_number(voucher_data: &VoucherData) -> Option<Secret<String>> {
    match voucher_data {
        VoucherData::Boleto(boleto_data) => match &boleto_data.social_security_number {
            Some(social_security_number)
                if is_valid_social_security_number(social_security_number.peek()) =>
            {
                Some(social_security_number.clone())
            }
            _ => None,
        },
        VoucherData::Alfamart { .. }
        | VoucherData::Indomaret { .. }
        | VoucherData::Efecty
        | VoucherData::PagoEfectivo
        | VoucherData::RedCompra
        | VoucherData::RedPagos
        | VoucherData::Oxxo
        | VoucherData::SevenEleven { .. }
        | VoucherData::Lawson { .. }
        | VoucherData::MiniStop { .. }
        | VoucherData::FamilyMart { .. }
        | VoucherData::Seicomart { .. }
        | VoucherData::PayEasy { .. } => None,
    }
}

fn get_redirect_extra_details<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<(Option<String>, Option<common_enums::CountryAlpha2>), IntegrationError> {
    match &item.request.payment_method_data {
        PaymentMethodData::BankRedirect(
            BankRedirectData::Trustly { .. } | BankRedirectData::OpenBankingUk { .. },
        ) => {
            let country = item
                .resource_common_data
                .address
                .get_payment_billing()
                .and_then(|billing| billing.address.as_ref())
                .and_then(|addr| addr.country);
            Ok((
                item.request.get_optional_language_from_browser_info(),
                country,
            ))
        }
        _ => Ok((None, None)),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item
            .router_data
            .request
            .mandate_id
            .to_owned()
            .and_then(|mandate_ids| mandate_ids.mandate_reference_id)
        {
            Some(_mandate_ref) => Err(IntegrationError::not_implemented("payment_method").into()),
            None => match item.router_data.request.payment_method_data.clone() {
                PaymentMethodData::Card(ref card) => Self::try_from((item, card)).map_err(|err| {
                    err.change_context(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })
                }),
                PaymentMethodData::CardRedirect(ref card_redirect_data) => {
                    Self::try_from((item, card_redirect_data)).map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })
                }
                PaymentMethodData::Wallet(ref wallet_data) => Self::try_from((item, wallet_data))
                    .map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    }),
                PaymentMethodData::BankRedirect(ref bank_redirect) => {
                    Self::try_from((item, bank_redirect)).map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })
                }
                PaymentMethodData::BankTransfer(ref bank_transfer) => {
                    Self::try_from((item, bank_transfer.as_ref())).map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })
                }
                PaymentMethodData::BankDebit(ref bank_debit) => Self::try_from((item, bank_debit))
                    .map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    }),
                PaymentMethodData::GiftCard(ref gift_card) => {
                    Self::try_from((item, gift_card.as_ref())).map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })
                }
                PaymentMethodData::Voucher(ref voucher_data) => {
                    Self::try_from((item, voucher_data)).map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })
                }
                PaymentMethodData::PayLater(ref pay_later_data) => {
                    Self::try_from((item, pay_later_data)).map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })
                }
                PaymentMethodData::NetworkToken(ref token_data) => {
                    Self::try_from((item, token_data)).map_err(|err| {
                        err.change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
                        })
                    })
                }
                PaymentMethodData::Crypto(_)
                | PaymentMethodData::MandatePayment
                | PaymentMethodData::Reward
                | PaymentMethodData::RealTimePayment(_)
                | PaymentMethodData::Upi(_)
                | PaymentMethodData::OpenBanking(_)
                | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
                | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
                | PaymentMethodData::MobilePayment(_)
                | PaymentMethodData::Netbanking(_)
                | PaymentMethodData::CardToken(_) => {
                    Err(IntegrationError::not_implemented("payment method").into())
                }
            },
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AdyenRedirectRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let encoded_data = item
            .router_data
            .request
            .encoded_data
            .clone()
            .get_required_value("encoded_data")
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "encoded_data: AdyenRedirectRequestTypes",
                context: Default::default(),
            })?;
        let adyen_redirection_type =
            serde_urlencoded::from_str::<AdyenRedirectRequestTypes>(encoded_data.as_str())
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

        let adyen_redirect_request = match adyen_redirection_type {
            AdyenRedirectRequestTypes::AdyenRedirection(req) => Self {
                details: AdyenRedirectRequestTypes::AdyenRedirection(AdyenRedirection {
                    redirect_result: req.redirect_result,
                    type_of_redirection_result: None,
                    result_code: None,
                }),
            },
            AdyenRedirectRequestTypes::AdyenThreeDS(req) => Self {
                details: AdyenRedirectRequestTypes::AdyenThreeDS(AdyenThreeDS {
                    three_ds_result: req.three_ds_result,
                    type_of_redirection_result: None,
                    result_code: None,
                }),
            },
            AdyenRedirectRequestTypes::AdyenRefusal(req) => Self {
                details: AdyenRedirectRequestTypes::AdyenRefusal(AdyenRefusal {
                    payload: req.payload,
                    type_of_redirection_result: None,
                    result_code: None,
                }),
            },
        };
        Ok(adyen_redirect_request)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AdyenVoidRequest
{
    type Error = Error;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            merchant_account: auth_type.merchant_account,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AdyenPaymentResponse {
    Response(Box<AdyenResponse>),
    PresentToShopper(Box<PresentToShopperResponse>),
    QrCodeResponse(Box<QrCodeResponseResponse>),
    RedirectionResponse(Box<RedirectionResponse>),
    RedirectionErrorResponse(Box<RedirectionErrorResponse>),
    WebhookResponse(Box<AdyenWebhookResponse>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdyenPSyncResponse(AdyenPaymentResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SetupMandateResponse(AdyenPaymentResponse);

pub struct AdyenPaymentsResponseData {
    pub status: AttemptStatus,
    pub error: Option<ErrorResponse>,
    pub payments_response_data: PaymentsResponseData,
    pub txn_amount: Option<MinorUnit>,
    pub connector_response: Option<ConnectorResponseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenResponse {
    psp_reference: String,
    result_code: AdyenStatus,
    amount: Option<Amount>,
    merchant_reference: String,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
    additional_data: Option<AdditionalData>,
    store: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenVoidResponse {
    payment_psp_reference: String,
    status: AdyenVoidStatus,
    reference: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectionResponse {
    result_code: AdyenStatus,
    action: AdyenRedirectAction,
    amount: Option<Amount>,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
    psp_reference: Option<String>,
    merchant_reference: Option<String>,
    store: Option<String>,
    additional_data: Option<AdditionalData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenRedirectAction {
    payment_method_type: PaymentType,
    url: Option<Url>,
    method: Option<Method>,
    #[serde(rename = "type")]
    type_of_response: ActionType,
    data: Option<std::collections::HashMap<String, String>>,
    payment_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenPtsAction {
    reference: String,
    download_url: Option<Url>,
    payment_method_type: PaymentType,
    #[serde(rename = "expiresAt")]
    #[serde(
        default,
        with = "common_utils::custom_serde::iso8601::option_without_timezone"
    )]
    expires_at: Option<PrimitiveDateTime>,
    initial_amount: Option<Amount>,
    pass_creation_token: Option<String>,
    total_amount: Option<Amount>,
    #[serde(rename = "type")]
    type_of_response: ActionType,
    instructions_url: Option<Url>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenQrCodeAction {
    payment_method_type: PaymentType,
    #[serde(rename = "type")]
    type_of_response: ActionType,
    #[serde(rename = "url")]
    qr_code_url: Option<Url>,
    qr_code_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeAdditionalData {
    #[serde(rename = "pix.expirationDate")]
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pix_expiration_date: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    Redirect,
    Await,
    #[serde(rename = "qrCode")]
    QrCode,
    Voucher,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentToShopperResponse {
    psp_reference: Option<String>,
    result_code: AdyenStatus,
    action: AdyenPtsAction,
    amount: Option<Amount>,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
    merchant_reference: Option<String>,
    store: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectionErrorResponse {
    result_code: AdyenStatus,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
    psp_reference: Option<String>,
    merchant_reference: Option<String>,
    additional_data: Option<AdditionalData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrCodeResponseResponse {
    result_code: AdyenStatus,
    action: AdyenQrCodeAction,
    amount: Option<Amount>,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
    psp_reference: Option<String>,
    merchant_reference: Option<String>,
    store: Option<String>,
    additional_data: Option<QrCodeAdditionalData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdyenWebhookStatus {
    Authorised,
    AuthorisationFailed,
    Cancelled,
    CancelFailed,
    Captured,
    CaptureFailed,
    Reversed,
    UnexpectedEvent,
    Expired,
    AdjustedAuthorization,
    AdjustAuthorizationFailed,
}

//Creating custom struct which can be consumed in Psync Handler triggered from Webhooks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenWebhookResponse {
    transaction_id: String,
    payment_reference: Option<String>,
    status: AdyenWebhookStatus,
    amount: Option<Amount>,
    merchant_reference_id: String,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
    event_code: WebhookEventCode,
    #[serde(with = "common_utils::custom_serde::iso8601::option")]
    event_date: Option<PrimitiveDateTime>,
    // Raw acquirer refusal code
    refusal_code_raw: Option<String>,
    // Raw acquirer refusal reason
    refusal_reason_raw: Option<String>,
    recurring_detail_reference: Option<Secret<String>>,
    recurring_shopper_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdyenStatus {
    AuthenticationFinished,
    AuthenticationNotRequired,
    Authorised,
    Cancelled,
    ChallengeShopper,
    Error,
    Pending,
    Received,
    RedirectShopper,
    Refused,
    PresentToShopper,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMethod {
    /// Post the payment authorization, the capture will be executed on the full amount immediately
    #[default]
    Automatic,
    /// The capture will happen only if the merchant triggers a Capture API request
    Manual,
    /// The capture will happen only if the merchant triggers a Capture API request
    ManualMultiple,
    /// The capture can be scheduled to automatically get triggered at a specific date & time
    Scheduled,
    /// Handles separate auth and capture sequentially; same as `Automatic` for most connectors.
    SequentialAutomatic,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    Credit,
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

impl ForeignTryFrom<(bool, AdyenWebhookStatus)> for AttemptStatus {
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn foreign_try_from(
        (is_manual_capture, adyen_webhook_status): (bool, AdyenWebhookStatus),
    ) -> Result<Self, Self::Error> {
        match adyen_webhook_status {
            AdyenWebhookStatus::Authorised | AdyenWebhookStatus::AdjustedAuthorization => {
                match is_manual_capture {
                    true => Ok(Self::Authorized),
                    // In case of Automatic capture Authorized is the final status of the payment
                    false => Ok(Self::Charged),
                }
            }
            AdyenWebhookStatus::AuthorisationFailed
            | AdyenWebhookStatus::AdjustAuthorizationFailed => Ok(Self::Failure),
            AdyenWebhookStatus::Cancelled => Ok(Self::Voided),
            AdyenWebhookStatus::CancelFailed => Ok(Self::VoidFailed),
            AdyenWebhookStatus::Captured => Ok(Self::Charged),
            AdyenWebhookStatus::CaptureFailed => Ok(Self::CaptureFailed),
            AdyenWebhookStatus::Expired => Ok(Self::Expired),
            //If Unexpected Event is received, need to understand how it reached this point
            //Webhooks with Payment Events only should try to consume this resource object.
            AdyenWebhookStatus::UnexpectedEvent | AdyenWebhookStatus::Reversed => {
                Err(error_stack::report!(
                    ConnectorResponseTransformationError::response_handling_failed_http_status_unknown()
                ))
            }
        }
    }
}

fn get_adyen_payment_status(
    is_manual_capture: bool,
    adyen_status: AdyenStatus,
    pmt: Option<common_enums::PaymentMethodType>,
) -> AttemptStatus {
    match adyen_status {
        AdyenStatus::AuthenticationFinished => AttemptStatus::AuthenticationSuccessful,
        AdyenStatus::AuthenticationNotRequired | AdyenStatus::Received => AttemptStatus::Pending,
        AdyenStatus::Authorised => match is_manual_capture {
            true => AttemptStatus::Authorized,
            // In case of Automatic capture Authorized is the final status of the payment
            false => AttemptStatus::Charged,
        },
        AdyenStatus::Cancelled => AttemptStatus::Voided,
        AdyenStatus::ChallengeShopper
        | AdyenStatus::RedirectShopper
        | AdyenStatus::PresentToShopper => AttemptStatus::AuthenticationPending,
        AdyenStatus::Error | AdyenStatus::Refused => AttemptStatus::Failure,
        // Pix returns Pending status but requires customer action (QR code)
        // so we map it to AuthenticationPending like Hyperswitch does
        AdyenStatus::Pending => match pmt {
            Some(common_enums::PaymentMethodType::Pix) => AttemptStatus::AuthenticationPending,
            _ => AttemptStatus::Pending,
        },
    }
}

// Unified ForeignTryFrom for Authorize and Psync Responses
impl<F, Req>
    ForeignTryFrom<(
        ResponseRouterData<AdyenPaymentResponse, Self>,
        Option<common_enums::CaptureMethod>,
        bool, // is_multiple_capture_psync_flow
        Option<common_enums::PaymentMethodType>,
    )> for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
where
    F: Clone,
    Req: Clone,
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn foreign_try_from(
        (value, capture_method, is_multiple_capture_psync_flow, payment_method_type): (
            ResponseRouterData<AdyenPaymentResponse, Self>,
            Option<common_enums::CaptureMethod>,
            bool,
            Option<common_enums::PaymentMethodType>,
        ),
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let is_manual_capture = is_manual_capture(capture_method);
        let pmt = payment_method_type;

        let adyen_payments_response_data = match response {
            AdyenPaymentResponse::Response(response) => {
                get_adyen_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::PresentToShopper(response) => {
                get_present_to_shopper_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::QrCodeResponse(response) => {
                get_qr_code_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::RedirectionResponse(response) => {
                get_redirection_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::RedirectionErrorResponse(response) => {
                get_redirection_error_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::WebhookResponse(response) => get_webhook_response(
                *response,
                is_manual_capture,
                is_multiple_capture_psync_flow,
                http_code,
            )?,
        };

        let minor_amount_captured = match adyen_payments_response_data.status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => adyen_payments_response_data.txn_amount,
            _ => None,
        };

        Ok(Self {
            response: adyen_payments_response_data.error.map_or_else(
                || Ok(adyen_payments_response_data.payments_response_data),
                Err,
            ),
            resource_common_data: PaymentFlowData {
                status: adyen_payments_response_data.status,
                amount_captured: minor_amount_captured.map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured,
                connector_response: adyen_payments_response_data.connector_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AdyenPaymentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    F: Clone,
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(
        value: ResponseRouterData<AdyenPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let capture_method = value.router_data.request.capture_method;
        let payment_method_type = value.router_data.request.payment_method_type;
        Self::foreign_try_from((
            value,
            capture_method,
            false, // is_multiple_capture_psync_flow = false for authorize
            payment_method_type,
        ))
    }
}

impl<F> TryFrom<ResponseRouterData<AdyenPSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
where
    F: Clone,
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(value: ResponseRouterData<AdyenPSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Extract the inner AdyenPaymentResponse from AdyenPSyncResponse
        let adyen_payment_response = value.response.0;

        // Check if this is a multiple capture sync flow
        let is_multiple_capture_psync_flow = match value.router_data.request.sync_type {
            SyncRequestType::MultipleCaptureSync => true,
            SyncRequestType::SinglePaymentSync => false,
        };

        let capture_method = value.router_data.request.capture_method;
        let payment_method_type = value.router_data.request.payment_method_type;

        let converted_value = ResponseRouterData {
            response: adyen_payment_response,
            router_data: value.router_data,
            http_code: value.http_code,
        };

        Self::foreign_try_from((
            converted_value,
            capture_method,
            is_multiple_capture_psync_flow,
            payment_method_type,
        ))
    }
}

// Response transformer for RepeatPayment - similar to Cybersource/other connectors pattern
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AdyenRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(
        value: ResponseRouterData<AdyenRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: repeat_response,
            router_data,
            http_code,
        } = value;
        // Unwrap the response wrapper to get AdyenPaymentResponse
        let response = repeat_response.0;
        let is_manual_capture = is_manual_capture(router_data.request.capture_method);
        let pmt = router_data.request.payment_method_type;

        // Process response using existing helper functions (same as Authorize)
        let adyen_payments_response_data = match response {
            AdyenPaymentResponse::Response(response) => {
                get_adyen_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::PresentToShopper(response) => {
                get_present_to_shopper_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::QrCodeResponse(response) => {
                get_qr_code_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::RedirectionResponse(response) => {
                get_redirection_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::RedirectionErrorResponse(response) => {
                get_redirection_error_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::WebhookResponse(response) => get_webhook_response(
                *response,
                is_manual_capture,
                false, // is_multiple_capture_psync_flow = false for repeat payment
                http_code,
            )?,
        };

        let minor_amount_captured = match adyen_payments_response_data.status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => adyen_payments_response_data.txn_amount,
            _ => None,
        };

        Ok(Self {
            response: adyen_payments_response_data.error.map_or_else(
                || Ok(adyen_payments_response_data.payments_response_data),
                Err,
            ),
            resource_common_data: PaymentFlowData {
                status: adyen_payments_response_data.status,
                amount_captured: minor_amount_captured.map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured,
                connector_response: adyen_payments_response_data.connector_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AdyenVoidStatus {
    Received,
    #[default]
    Processing,
}

impl ForeignTryFrom<AdyenVoidStatus> for AttemptStatus {
    type Error = ConnectorResponseTransformationError;
    fn foreign_try_from(item: AdyenVoidStatus) -> Result<Self, Self::Error> {
        match item {
            AdyenVoidStatus::Received => Ok(Self::Voided),
            AdyenVoidStatus::Processing => Ok(Self::VoidInitiated),
        }
    }
}

impl TryFrom<ResponseRouterData<AdyenVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(value: ResponseRouterData<AdyenVoidResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let status = AttemptStatus::Pending;

        let payment_void_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.payment_psp_reference),
            redirection_data: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.reference),
            incremental_authorization_allowed: None,
            mandate_reference: None,
            status_code: http_code,
        };

        Ok(Self {
            response: Ok(payment_void_response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

pub fn get_adyen_response(
    response: AdyenResponse,
    is_capture_manual: bool,
    status_code: u16,
    pmt: Option<common_enums::PaymentMethodType>,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorResponseTransformationError> {
    let status = get_adyen_payment_status(is_capture_manual, response.result_code, pmt);
    let error = if response.refusal_reason.is_some()
        || response.refusal_reason_code.is_some()
        || status == AttemptStatus::Failure
    {
        let (network_decline_code, network_error_message) = response
            .additional_data
            .as_ref()
            .map(|data| {
                match (
                    data.refusal_code_raw.clone(),
                    data.refusal_reason_raw
                        .clone()
                        .or(data.merchant_advice_code.clone()),
                ) {
                    (None, Some(reason_raw)) => match reason_raw.split_once(':') {
                        Some((code, msg)) => {
                            (Some(code.trim().to_string()), Some(msg.trim().to_string()))
                        }
                        None => (None, Some(reason_raw.trim().to_string())),
                    },
                    (code, reason) => (code, reason),
                }
            })
            .unwrap_or((None, None));

        Some(ErrorResponse {
            code: response
                .refusal_reason_code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .refusal_reason
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.refusal_reason,
            status_code,
            attempt_status: None,
            connector_transaction_id: Some(response.psp_reference.clone()),
            network_advice_code: response
                .additional_data
                .as_ref()
                .and_then(|data| data.extract_network_advice_code()),
            network_decline_code,
            network_error_message,
        })
    } else {
        None
    };
    let mandate_reference = response
        .additional_data
        .as_ref()
        .and_then(|data| data.recurring_detail_reference.to_owned())
        .map(|mandate_id| MandateReference {
            connector_mandate_id: Some(mandate_id.expose()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        });
    let network_txn_id = response
        .additional_data
        .as_ref()
        .and_then(|additional_data| {
            additional_data
                .network_tx_reference
                .as_ref()
                .map(|network_tx_id| network_tx_id.clone().expose())
        });

    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.psp_reference),
        redirection_data: None,
        connector_metadata: None,
        network_txn_id,
        connector_response_reference_id: Some(response.merchant_reference),
        incremental_authorization_allowed: None,
        mandate_reference: mandate_reference.map(Box::new),
        status_code,
    };

    let txn_amount = response.amount.map(|amount| amount.value);
    let connector_response = pmt.and_then(|pmt| {
        response
            .additional_data
            .as_ref()
            .and_then(|additional_data| additional_data.auth_code.clone())
            .map(|auth_code| ConnectorResponseData::with_auth_code(auth_code, pmt))
    });

    Ok(AdyenPaymentsResponseData {
        status,
        error,
        payments_response_data,
        txn_amount,
        connector_response,
    })
}

#[derive(Debug, Deserialize)]
pub struct AdyenTestingData {
    holder_name: Option<Secret<String>>,
}

impl TryFrom<SecretSerdeValue> for AdyenTestingData {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(testing_data: SecretSerdeValue) -> Result<Self, Self::Error> {
        let testing_data = testing_data
            .expose()
            .parse_value::<Self>("AdyenTestingData")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "connector_metadata.adyen.testing",
                context: Default::default(),
            })?;
        Ok(testing_data)
    }
}

pub fn get_present_to_shopper_response(
    response: PresentToShopperResponse,
    is_manual_capture: bool,
    status_code: u16,
    pmt: Option<common_enums::PaymentMethodType>,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorResponseTransformationError> {
    let status = get_adyen_payment_status(is_manual_capture, response.result_code.clone(), pmt);
    let error = if response.refusal_reason.is_some()
        || response.refusal_reason_code.is_some()
        || status == AttemptStatus::Failure
    {
        Some(ErrorResponse {
            code: response
                .refusal_reason_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .refusal_reason
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.refusal_reason.to_owned(),
            status_code,
            attempt_status: None,
            connector_transaction_id: response.psp_reference.clone(),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    } else {
        None
    };

    let connector_metadata = get_present_to_shopper_metadata(&response)?;

    // We don't get connector transaction id for redirections in Adyen.
    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: match response.psp_reference.as_ref() {
            Some(psp) => ResponseId::ConnectorTransactionId(psp.to_string()),
            None => ResponseId::NoResponseId,
        },
        redirection_data: None,
        connector_metadata,
        network_txn_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        mandate_reference: None,
        status_code,
    };

    let txn_amount = response.amount.map(|amount| amount.value);

    Ok(AdyenPaymentsResponseData {
        status,
        error,
        payments_response_data,
        txn_amount,
        connector_response: None,
    })
}

pub fn get_redirection_error_response(
    response: RedirectionErrorResponse,
    is_manual_capture: bool,
    status_code: u16,
    pmt: Option<common_enums::PaymentMethodType>,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorResponseTransformationError> {
    let status = get_adyen_payment_status(is_manual_capture, response.result_code, pmt);
    let error = {
        let (network_decline_code, network_error_message) = response
            .additional_data
            .as_ref()
            .map(|data| {
                match (
                    data.refusal_code_raw.clone(),
                    data.refusal_reason_raw.clone(),
                ) {
                    (None, Some(reason_raw)) => match reason_raw.split_once(':') {
                        Some((code, msg)) => {
                            (Some(code.trim().to_string()), Some(msg.trim().to_string()))
                        }
                        None => (None, Some(reason_raw.trim().to_string())),
                    },
                    (code, reason) => (code, reason),
                }
            })
            .unwrap_or((None, None));

        let network_advice_code = response
            .additional_data
            .as_ref()
            .and_then(|data| data.extract_network_advice_code());

        Some(ErrorResponse {
            code: status.to_string(),
            message: response
                .refusal_reason
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.refusal_reason,
            status_code,
            attempt_status: None,
            connector_transaction_id: response.psp_reference.clone(),
            network_advice_code,
            network_decline_code,
            network_error_message,
        })
    };
    // We don't get connector transaction id for redirections in Adyen.
    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::NoResponseId,
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        status_code,
    };

    Ok(AdyenPaymentsResponseData {
        status,
        error,
        payments_response_data,
        txn_amount: None,
        connector_response: None,
    })
}

pub fn get_qr_code_response(
    response: QrCodeResponseResponse,
    is_manual_capture: bool,
    status_code: u16,
    pmt: Option<common_enums::PaymentMethodType>,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorResponseTransformationError> {
    let status = get_adyen_payment_status(is_manual_capture, response.result_code.clone(), pmt);
    let error = if response.refusal_reason.is_some()
        || response.refusal_reason_code.is_some()
        || status == AttemptStatus::Failure
    {
        Some(ErrorResponse {
            code: response
                .refusal_reason_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .refusal_reason
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.refusal_reason.to_owned(),
            status_code,
            attempt_status: None,
            connector_transaction_id: response.psp_reference.clone(),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    } else {
        None
    };

    // Generate QR metadata matching Hyperswitch implementation
    let connector_metadata = get_qr_metadata(&response)?;

    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: match response.psp_reference.as_ref() {
            Some(psp) => ResponseId::ConnectorTransactionId(psp.to_string()),
            None => ResponseId::NoResponseId,
        },
        redirection_data: None,
        connector_metadata,
        network_txn_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        mandate_reference: None,
        status_code,
    };

    Ok(AdyenPaymentsResponseData {
        status,
        error,
        payments_response_data,
        txn_amount: response.amount.map(|amount| amount.value),
        connector_response: None,
    })
}

/// Get QR code metadata for Pix and other QR-based payment methods
/// Matches Hyperswitch's get_qr_metadata implementation
fn get_qr_metadata(
    response: &QrCodeResponseResponse,
) -> CustomResult<Option<serde_json::Value>, ConnectorResponseTransformationError> {
    // Generate QR code image from qr_code_data
    let image_data = QrImage::new_from_data(response.action.qr_code_data.clone()).change_context(
        ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(),
    )?;

    let image_data_url = Url::parse(image_data.data.as_str()).ok();
    let qr_code_url = response.action.qr_code_url.clone();

    // Extract pix expiration date and convert to timestamp in milliseconds
    let display_to_timestamp = response
        .additional_data
        .as_ref()
        .and_then(|additional_data| additional_data.pix_expiration_date)
        .map(|time| {
            // Convert PrimitiveDateTime to Unix timestamp in milliseconds
            time.assume_utc().unix_timestamp() * 1000
        });

    let qr_code_info = match (image_data_url, qr_code_url) {
        (Some(image_data_url), Some(qr_code_url)) => Some(QrCodeInformation::QrCodeUrl {
            image_data_url,
            qr_code_url,
            display_to_timestamp,
        }),
        (None, Some(qr_code_url)) => Some(QrCodeInformation::QrCodeImageUrl {
            qr_code_url,
            display_to_timestamp,
        }),
        (Some(image_data_url), None) => Some(QrCodeInformation::QrDataUrl {
            image_data_url,
            display_to_timestamp,
        }),
        (None, None) => None,
    };

    qr_code_info
        .map(|info| info.encode_to_value())
        .transpose()
        .change_context(
            ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(),
        )
}

pub fn get_webhook_response(
    response: AdyenWebhookResponse,
    is_manual_capture: bool,
    is_multiple_capture_psync_flow: bool,
    status_code: u16,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorResponseTransformationError> {
    let status = AttemptStatus::foreign_try_from((is_manual_capture, response.status.clone()))?;
    let error = if response.refusal_reason.is_some()
        || response.refusal_reason_code.is_some()
        || status == AttemptStatus::Failure
    {
        let (network_decline_code, network_error_message) = match (
            response.refusal_code_raw.clone(),
            response.refusal_reason_raw.clone(),
        ) {
            (None, Some(reason_raw)) => match reason_raw.split_once(':') {
                Some((code, msg)) => (Some(code.trim().to_string()), Some(msg.trim().to_string())),
                None => (None, Some(reason_raw.trim().to_string())),
            },
            (code, reason) => (code, reason),
        };

        Some(ErrorResponse {
            code: response
                .refusal_reason_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .refusal_reason
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.refusal_reason.clone(),
            status_code,
            attempt_status: None,
            connector_transaction_id: Some(response.transaction_id.clone()),
            network_advice_code: None,
            network_decline_code,
            network_error_message,
        })
    } else {
        None
    };

    let txn_amount = response.amount.as_ref().map(|amount| amount.value);
    let connector_response = build_connector_response(&response);

    if is_multiple_capture_psync_flow {
        let capture_sync_response_list =
            utils::construct_captures_response_hashmap(vec![response.clone()]).change_context(
                utils::response_handling_fail_for_connector(status_code, "adyen"),
            )?;
        Ok(AdyenPaymentsResponseData {
            status,
            error,
            payments_response_data: PaymentsResponseData::MultipleCaptureResponse {
                capture_sync_response_list,
            },
            txn_amount,
            connector_response,
        })
    } else {
        let mandate_reference = response
            .recurring_detail_reference
            .as_ref()
            .map(|mandate_id| MandateReference {
                connector_mandate_id: Some(mandate_id.clone().expose()),
                payment_method_id: response.recurring_shopper_reference.clone(),
                connector_mandate_request_reference_id: None,
            });
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                response
                    .payment_reference
                    .unwrap_or(response.transaction_id),
            ),
            redirection_data: None,
            mandate_reference: mandate_reference.map(Box::new),
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.merchant_reference_id),
            incremental_authorization_allowed: None,
            status_code,
        };

        Ok(AdyenPaymentsResponseData {
            status,
            error,
            payments_response_data,
            txn_amount,
            connector_response,
        })
    }
}

fn build_connector_response(
    adyen_webhook_response: &AdyenWebhookResponse,
) -> Option<ConnectorResponseData> {
    let extended_authentication_applied = match adyen_webhook_response.status {
        AdyenWebhookStatus::AdjustedAuthorization => Some(true),
        AdyenWebhookStatus::AdjustAuthorizationFailed => Some(false),
        _ => None,
    };

    let extended_authorization_last_applied_at = extended_authentication_applied
        .filter(|val| *val)
        .and(adyen_webhook_response.event_date);

    let extend_authorization_response = ExtendedAuthorizationResponseData {
        extended_authentication_applied,
        capture_before: None,
        extended_authorization_last_applied_at,
    };

    Some(ConnectorResponseData::new(
        None,
        None,
        Some(extend_authorization_response),
    ))
}

// Triggered in PSync handler of webhook response (parity with Hyperswitch)
impl utils::MultipleCaptureSyncResponse for AdyenWebhookResponse {
    fn get_connector_capture_id(&self) -> String {
        self.transaction_id.clone()
    }

    fn get_capture_attempt_status(&self) -> AttemptStatus {
        match self.status {
            AdyenWebhookStatus::Captured => AttemptStatus::Charged,
            _ => AttemptStatus::CaptureFailed,
        }
    }

    fn is_capture_response(&self) -> bool {
        matches!(
            self.event_code,
            WebhookEventCode::Capture | WebhookEventCode::CaptureFailed
        )
    }

    fn get_connector_reference_id(&self) -> Option<String> {
        Some(self.merchant_reference_id.clone())
    }

    fn get_amount_captured(
        &self,
    ) -> Result<Option<MinorUnit>, error_stack::Report<common_utils::errors::ParsingError>> {
        Ok(self.amount.clone().map(|amount| amount.value))
    }
}

pub fn get_redirection_response(
    response: RedirectionResponse,
    is_manual_capture: bool,
    status_code: u16,
    pmt: Option<common_enums::PaymentMethodType>,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorResponseTransformationError> {
    let status = get_adyen_payment_status(is_manual_capture, response.result_code.clone(), pmt);
    let error = if response.refusal_reason.is_some()
        || response.refusal_reason_code.is_some()
        || status == AttemptStatus::Failure
    {
        let (network_decline_code, network_error_message) = response
            .additional_data
            .as_ref()
            .map(|data| {
                match (
                    data.refusal_code_raw.clone(),
                    data.refusal_reason_raw.clone(),
                ) {
                    (None, Some(reason_raw)) => match reason_raw.split_once(':') {
                        Some((code, msg)) => {
                            (Some(code.trim().to_string()), Some(msg.trim().to_string()))
                        }
                        None => (None, Some(reason_raw.trim().to_string())),
                    },
                    (code, reason) => (code, reason),
                }
            })
            .unwrap_or((None, None));

        Some(ErrorResponse {
            code: response
                .refusal_reason_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .refusal_reason
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.refusal_reason.to_owned(),
            status_code,
            attempt_status: None,
            connector_transaction_id: response.psp_reference.clone(),
            network_advice_code: None,
            network_decline_code,
            network_error_message,
        })
    } else {
        None
    };

    let redirection_data = response.action.url.clone().map(|url| {
        let form_fields = response.action.data.clone().unwrap_or_else(|| {
            std::collections::HashMap::from_iter(
                url.query_pairs()
                    .map(|(key, value)| (key.to_string(), value.to_string())),
            )
        });
        RedirectForm::Form {
            endpoint: url.to_string(),
            method: response.action.method.unwrap_or(Method::Get),
            form_fields,
        }
    });

    let connector_metadata = get_wait_screen_metadata(&response)?;

    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: match response.psp_reference.as_ref() {
            Some(psp) => ResponseId::ConnectorTransactionId(psp.to_string()),
            None => ResponseId::NoResponseId,
        },
        redirection_data: redirection_data.map(Box::new),
        mandate_reference: None,
        connector_metadata,
        network_txn_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        status_code,
    };

    let txn_amount = response.amount.map(|amount| amount.value);

    Ok(AdyenPaymentsResponseData {
        status,
        error,
        payments_response_data,
        txn_amount,
        connector_response: None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitScreenData {
    display_from_timestamp: i128,
    display_to_timestamp: Option<i128>,
}

pub fn get_wait_screen_metadata(
    next_action: &RedirectionResponse,
) -> CustomResult<Option<serde_json::Value>, ConnectorResponseTransformationError> {
    match next_action.action.payment_method_type {
        PaymentType::Blik => {
            let current_time = OffsetDateTime::now_utc().unix_timestamp_nanos();
            Ok(Some(serde_json::json!(WaitScreenData {
                display_from_timestamp: current_time,
                display_to_timestamp: Some(current_time + Duration::minutes(1).whole_nanoseconds())
            })))
        }
        PaymentType::Mbway => {
            let current_time = OffsetDateTime::now_utc().unix_timestamp_nanos();
            Ok(Some(serde_json::json!(WaitScreenData {
                display_from_timestamp: current_time,
                display_to_timestamp: None
            })))
        }
        PaymentType::Affirm
        | PaymentType::Oxxo
        | PaymentType::Afterpaytouch
        | PaymentType::Alipay
        | PaymentType::AlipayHk
        | PaymentType::Alfamart
        | PaymentType::Alma
        | PaymentType::Applepay
        | PaymentType::Bizum
        | PaymentType::Atome
        | PaymentType::BoletoBancario
        | PaymentType::ClearPay
        | PaymentType::Dana
        | PaymentType::Eps
        | PaymentType::Gcash
        | PaymentType::Googlepay
        | PaymentType::GoPay
        | PaymentType::Ideal
        | PaymentType::Indomaret
        | PaymentType::Klarna
        | PaymentType::Kakaopay
        | PaymentType::MobilePay
        | PaymentType::Momo
        | PaymentType::MomoAtm
        | PaymentType::OnlineBankingCzechRepublic
        | PaymentType::OnlineBankingFinland
        | PaymentType::OnlineBankingPoland
        | PaymentType::OnlineBankingSlovakia
        | PaymentType::OnlineBankingFpx
        | PaymentType::OnlineBankingThailand
        | PaymentType::OpenBankingUK
        | PaymentType::PayBright
        | PaymentType::Paypal
        | PaymentType::Scheme
        | PaymentType::NetworkToken
        | PaymentType::Trustly
        | PaymentType::TouchNGo
        | PaymentType::Walley
        | PaymentType::WeChatPayWeb
        | PaymentType::AchDirectDebit
        | PaymentType::SepaDirectDebit
        | PaymentType::BacsDirectDebit
        | PaymentType::Samsungpay
        | PaymentType::Twint
        | PaymentType::Vipps
        | PaymentType::Swish
        | PaymentType::Knet
        | PaymentType::Benefit
        | PaymentType::PermataBankTransfer
        | PaymentType::BcaBankTransfer
        | PaymentType::BniVa
        | PaymentType::BriVa
        | PaymentType::CimbVa
        | PaymentType::DanamonVa
        | PaymentType::Giftcard
        | PaymentType::MandiriVa
        | PaymentType::PaySafeCard
        | PaymentType::SevenEleven
        | PaymentType::Lawson
        | PaymentType::Pix => Ok(None),
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenErrorResponse {
    pub status: i32,
    pub error_code: String,
    pub message: String,
    pub error_type: String,
    pub psp_reference: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, strum::Display, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhookEventCode {
    Authorisation,
    AuthorisationAdjustment,
    Cancellation,
    Capture,
    CaptureFailed,
    Refund,
    RefundFailed,
    RefundReversed,
    CancelOrRefund,
    NotificationOfChargeback,
    Chargeback,
    ChargebackReversed,
    SecondChargeback,
    PrearbitrationWon,
    PrearbitrationLost,
    OfferClosed,
    RecurringContract,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub enum DisputeStatus {
    Undefended,
    Pending,
    Lost,
    Accepted,
    Won,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenAdditionalDataWH {
    #[serde(rename = "hmacSignature")]
    pub hmac_signature: Option<String>,
    pub dispute_status: Option<DisputeStatus>,
    pub chargeback_reason_code: Option<String>,
    /// Enable recurring details in Adyen dashboard to receive this ID.
    #[serde(rename = "recurring.recurringDetailReference")]
    pub recurring_detail_reference: Option<Secret<String>>,
    #[serde(rename = "recurring.shopperReference")]
    pub recurring_shopper_reference: Option<String>,
    pub network_tx_reference: Option<Secret<String>>,
    pub refusal_reason_raw: Option<String>,
    pub refusal_code_raw: Option<String>,
    pub shopper_email: Option<Secret<String>>,
    pub shopper_reference: Option<String>,
    pub expiry_date: Option<Secret<String>>,
    pub card_summary: Option<Secret<String>>,
    pub card_issuing_country: Option<String>,
    pub card_issuing_bank: Option<String>,
    pub payment_method_variant: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
    pub defense_period_ends_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenNotificationRequestItemWH {
    pub original_reference: Option<String>,
    pub psp_reference: String,
    pub amount: AdyenAmountWH,
    pub event_code: WebhookEventCode,
    pub merchant_account_code: String,
    pub merchant_reference: String,
    pub success: String,
    pub reason: Option<String>,
    pub additional_data: AdyenAdditionalDataWH,
}

#[derive(Debug, Deserialize)]
pub struct AdyenAmountWH {
    pub value: MinorUnit,
    pub currency: common_enums::Currency,
}

pub(crate) fn get_adyen_mandate_reference_from_webhook(
    notif: &AdyenNotificationRequestItemWH,
) -> Option<Box<MandateReference>> {
    notif
        .additional_data
        .recurring_detail_reference
        .as_ref()
        .map(|mandate_id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(mandate_id.peek().to_string()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        })
}

pub(crate) fn get_adyen_network_txn_id_from_webhook(
    notif: &AdyenNotificationRequestItemWH,
) -> Option<String> {
    notif
        .additional_data
        .network_tx_reference
        .as_ref()
        .map(|network_tx_id| network_tx_id.peek().to_string())
}

pub(crate) fn get_adyen_payment_method_update_from_webhook(
    notif: &AdyenNotificationRequestItemWH,
) -> Option<PaymentMethodUpdate> {
    // Align with HS semantics: if Adyen provides an expiry date, it's a strong signal this
    // notification is meant to update stored card details (e.g., account updater).
    notif
        .additional_data
        .expiry_date
        .as_ref()
        .and_then(|expiry_date| {
            let expiry_date = expiry_date.peek().to_string();
            let (month, year) = expiry_date.split_once('/')?;
            Some((month.trim().to_string(), year.trim().to_string()))
        })
        .map(|(month, year)| {
            PaymentMethodUpdate::Card(CardDetailUpdate {
                card_exp_month: Some(month),
                card_exp_year: Some(year),
                last4_digits: notif
                    .additional_data
                    .card_summary
                    .as_ref()
                    .map(|last4| last4.peek().to_string()),
                issuer_country: notif.additional_data.card_issuing_country.clone(),
                card_issuer: notif.additional_data.card_issuing_bank.clone(),
                card_network: notif
                    .additional_data
                    .payment_method_variant
                    .as_ref()
                    .map(|network| network.peek().to_string()),
                card_holder_name: notif
                    .additional_data
                    .card_holder_name
                    .as_ref()
                    .map(|name| name.peek().to_string()),
            })
        })
}

fn is_success_scenario(is_success: &str) -> bool {
    is_success == "true"
}

pub(crate) fn get_adyen_payment_webhook_event(
    code: WebhookEventCode,
    is_success: String,
) -> Result<AttemptStatus, WebhookError> {
    match code {
        WebhookEventCode::Authorisation | WebhookEventCode::RecurringContract => {
            if is_success_scenario(&is_success) {
                Ok(AttemptStatus::Authorized)
            } else {
                Ok(AttemptStatus::Failure)
            }
        }
        WebhookEventCode::AuthorisationAdjustment => {
            if is_success_scenario(&is_success) {
                Ok(AttemptStatus::Authorized)
            } else {
                Ok(AttemptStatus::Failure)
            }
        }
        WebhookEventCode::Cancellation => {
            if is_success_scenario(&is_success) {
                Ok(AttemptStatus::Voided)
            } else {
                Ok(AttemptStatus::Authorized)
            }
        }
        WebhookEventCode::Capture => {
            if is_success_scenario(&is_success) {
                Ok(AttemptStatus::Charged)
            } else {
                Ok(AttemptStatus::Failure)
            }
        }
        WebhookEventCode::CaptureFailed => Ok(AttemptStatus::Failure),
        WebhookEventCode::OfferClosed => Ok(AttemptStatus::Expired),
        _ => Err(WebhookError::WebhookProcessingFailed),
    }
}

pub(crate) fn get_adyen_refund_webhook_event(
    code: WebhookEventCode,
    is_success: String,
) -> Result<RefundStatus, WebhookError> {
    match code {
        WebhookEventCode::Refund | WebhookEventCode::CancelOrRefund => {
            if is_success_scenario(&is_success) {
                Ok(RefundStatus::Success)
            } else {
                Ok(RefundStatus::Failure)
            }
        }
        WebhookEventCode::RefundFailed | WebhookEventCode::RefundReversed => {
            Ok(RefundStatus::Failure)
        }
        _ => Err(WebhookError::WebhookProcessingFailed),
    }
}

pub(crate) fn get_adyen_webhook_event_type(
    code: WebhookEventCode,
) -> Result<EventType, WebhookError> {
    match code {
        WebhookEventCode::Authorisation | WebhookEventCode::RecurringContract => {
            Ok(EventType::PaymentIntentAuthorizationSuccess)
        }
        WebhookEventCode::AuthorisationAdjustment => {
            Ok(EventType::PaymentIntentAuthorizationSuccess)
        }
        WebhookEventCode::Cancellation => Ok(EventType::PaymentIntentCancelled),
        WebhookEventCode::Capture => Ok(EventType::PaymentIntentCaptureSuccess),
        WebhookEventCode::CaptureFailed => Ok(EventType::PaymentIntentCaptureFailure),
        WebhookEventCode::OfferClosed => Ok(EventType::PaymentIntentExpired),
        WebhookEventCode::Refund | WebhookEventCode::CancelOrRefund => Ok(EventType::RefundSuccess),
        WebhookEventCode::RefundFailed | WebhookEventCode::RefundReversed => {
            Ok(EventType::RefundFailure)
        }
        WebhookEventCode::NotificationOfChargeback | WebhookEventCode::Chargeback => {
            Ok(EventType::DisputeOpened)
        }
        WebhookEventCode::ChargebackReversed | WebhookEventCode::PrearbitrationWon => {
            Ok(EventType::DisputeWon)
        }
        WebhookEventCode::SecondChargeback | WebhookEventCode::PrearbitrationLost => {
            Ok(EventType::DisputeLost)
        }
        WebhookEventCode::Unknown => Err(WebhookError::WebhookEventTypeNotFound),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AdyenItemObjectWH {
    pub notification_request_item: AdyenNotificationRequestItemWH,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenIncomingWebhook {
    pub notification_items: Vec<AdyenItemObjectWH>,
}

pub fn get_webhook_object_from_body(
    body: Vec<u8>,
) -> Result<AdyenNotificationRequestItemWH, error_stack::Report<IntegrationError>> {
    let mut webhook: AdyenIncomingWebhook =
        body.parse_struct("AdyenIncomingWebhook").change_context(
            IntegrationError::not_implemented("webhook body decoding failed".to_string()),
        )?;

    let item_object =
        webhook
            .notification_items
            .drain(..)
            .next()
            .ok_or(IntegrationError::not_implemented(
                "webhook body decoding failed".to_string(),
            ))?;

    Ok(item_object.notification_request_item)
}

fn get_shopper_statement<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Option<String> {
    item.request
        .billing_descriptor
        .clone()
        .and_then(|descriptor| descriptor.statement_descriptor)
}

fn get_shopper_statement_for_repeat_payment<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
) -> Option<String> {
    item.request
        .billing_descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.statement_descriptor.clone())
}

fn get_additional_data_for_repeat_payment<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
) -> Option<AdditionalData> {
    let (authorisation_type, manual_capture) = match item.request.capture_method {
        Some(common_enums::CaptureMethod::Manual)
        | Some(common_enums::CaptureMethod::ManualMultiple) => {
            (Some(AuthType::PreAuth), Some("true".to_string()))
        }
        _ => (None, None),
    };
    let riskdata = item.request.metadata.clone().and_then(|m| {
        m.expose()
            .parse_value::<serde_json::Value>("metadata")
            .ok()
            .and_then(get_risk_data)
    });

    let execute_three_d = if matches!(
        item.resource_common_data.auth_type,
        common_enums::AuthenticationType::ThreeDs
    ) {
        Some("true".to_string())
    } else {
        Some("false".to_string())
    };

    Some(AdditionalData {
        authorisation_type,
        manual_capture,
        execute_three_d,
        network_tx_reference: None,
        recurring_detail_reference: None,
        recurring_shopper_reference: None,
        recurring_processing_model: None,
        riskdata,
        sca_exemption: item.request.authentication_data.as_ref().and_then(|data| {
            data.exemption_indicator
                .as_ref()
                .and_then(to_adyen_exemption)
        }),
        ..AdditionalData::default()
    })
}

type RecurringDetails = (Option<AdyenRecurringModel>, Option<bool>, Option<String>);

fn get_recurring_processing_model_for_repeat_payment<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
) -> Result<RecurringDetails, error_stack::Report<IntegrationError>> {
    let shopper_reference = item.resource_common_data.get_connector_customer_id().ok();

    match item.request.off_session {
        // Off-session payment
        Some(true) => {
            let shopper_reference =
                shopper_reference.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_customer_id",
                    context: Default::default(),
                })?;
            Ok((
                Some(AdyenRecurringModel::UnscheduledCardOnFile),
                None,
                Some(shopper_reference),
            ))
        }
        // On-session payment
        _ => Ok((None, None, shopper_reference)),
    }
}

fn get_recurring_processing_model<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<RecurringDetails, Error> {
    let shopper_reference = item.resource_common_data.get_connector_customer_id().ok();

    match (item.request.setup_future_usage, item.request.off_session) {
        // Setup for future off-session usage
        (Some(common_enums::FutureUsage::OffSession), _) => {
            let store_payment_method = item.request.is_mandate_payment();
            let shopper_reference =
                shopper_reference.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_customer_id",
                    context: Default::default(),
                })?;
            Ok((
                Some(AdyenRecurringModel::UnscheduledCardOnFile),
                Some(store_payment_method),
                Some(shopper_reference),
            ))
        }
        // Off-session payment
        (_, Some(true)) => {
            let shopper_reference =
                shopper_reference.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_customer_id",
                    context: Default::default(),
                })?;
            Ok((
                Some(AdyenRecurringModel::UnscheduledCardOnFile),
                None,
                Some(shopper_reference),
            ))
        }
        // On-session payment
        _ => Ok((None, None, shopper_reference)),
    }
}

pub fn get_address_info(
    address: Option<&domain_types::payment_address::Address>,
) -> Option<Result<Address, error_stack::Report<IntegrationError>>> {
    address.and_then(|add| {
        add.address.as_ref().map(
            |a| -> Result<Address, error_stack::Report<IntegrationError>> {
                Ok(Address {
                    city: a.get_city()?.to_owned(),
                    country: a.get_country()?.to_owned(),
                    house_number_or_name: a.get_line1()?.to_owned(),
                    postal_code: a.get_zip()?.to_owned(),
                    state_or_province: a.state.clone(),
                    street: a.line2.clone(),
                })
            },
        )
    })
}

fn get_additional_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Option<AdditionalData> {
    let (authorisation_type, manual_capture) = match item.request.capture_method {
        Some(common_enums::CaptureMethod::Manual)
        | Some(common_enums::CaptureMethod::ManualMultiple) => {
            (Some(AuthType::PreAuth), Some("true".to_string()))
        }
        _ => (None, None),
    };
    let riskdata = item
        .request
        .metadata
        .clone()
        .expose_option()
        .and_then(get_risk_data);

    let execute_three_d = if matches!(
        item.resource_common_data.auth_type,
        common_enums::AuthenticationType::ThreeDs
    ) {
        Some("true".to_string())
    } else {
        Some("false".to_string())
    };

    Some(AdditionalData {
        authorisation_type,
        manual_capture,
        execute_three_d,
        network_tx_reference: None,
        recurring_detail_reference: None,
        recurring_shopper_reference: None,
        recurring_processing_model: None,
        riskdata,
        sca_exemption: item.request.authentication_data.as_ref().and_then(|data| {
            data.exemption_indicator
                .as_ref()
                .and_then(to_adyen_exemption)
        }),
        ..AdditionalData::default()
    })
}

pub fn get_risk_data(metadata: serde_json::Value) -> Option<RiskData> {
    let item_i_d = get_str("riskdata.basket.item1.itemID", &metadata);
    let product_title = get_str("riskdata.basket.item1.productTitle", &metadata);
    let amount_per_item = get_str("riskdata.basket.item1.amountPerItem", &metadata);
    let currency = get_str("riskdata.basket.item1.currency", &metadata);
    let upc = get_str("riskdata.basket.item1.upc", &metadata);
    let brand = get_str("riskdata.basket.item1.brand", &metadata);
    let manufacturer = get_str("riskdata.basket.item1.manufacturer", &metadata);
    let category = get_str("riskdata.basket.item1.category", &metadata);
    let quantity = get_str("riskdata.basket.item1.quantity", &metadata);
    let color = get_str("riskdata.basket.item1.color", &metadata);
    let size = get_str("riskdata.basket.item1.size", &metadata);

    let device_country = get_str("riskdata.deviceCountry", &metadata);
    let house_numberor_name = get_str("riskdata.houseNumberorName", &metadata);
    let account_creation_date = get_str("riskdata.accountCreationDate", &metadata);
    let affiliate_channel = get_str("riskdata.affiliateChannel", &metadata);
    let avg_order_value = get_str("riskdata.avgOrderValue", &metadata);
    let delivery_method = get_str("riskdata.deliveryMethod", &metadata);
    let email_name = get_str("riskdata.emailName", &metadata);
    let email_domain = get_str("riskdata.emailDomain", &metadata);
    let last_order_date = get_str("riskdata.lastOrderDate", &metadata);
    let merchant_reference = get_str("riskdata.merchantReference", &metadata);
    let payment_method = get_str("riskdata.paymentMethod", &metadata);
    let promotion_name = get_str("riskdata.promotionName", &metadata);
    let secondary_phone_number = get_str("riskdata.secondaryPhoneNumber", &metadata);
    let timefrom_loginto_order = get_str("riskdata.timefromLogintoOrder", &metadata);
    let total_session_time = get_str("riskdata.totalSessionTime", &metadata);
    let total_authorized_amount_in_last30_days =
        get_str("riskdata.totalAuthorizedAmountInLast30Days", &metadata);
    let total_order_quantity = get_str("riskdata.totalOrderQuantity", &metadata);
    let total_lifetime_value = get_str("riskdata.totalLifetimeValue", &metadata);
    let visits_month = get_str("riskdata.visitsMonth", &metadata);
    let visits_week = get_str("riskdata.visitsWeek", &metadata);
    let visits_year = get_str("riskdata.visitsYear", &metadata);
    let ship_to_name = get_str("riskdata.shipToName", &metadata);
    let first8charactersof_address_line1_zip =
        get_str("riskdata.first8charactersofAddressLine1Zip", &metadata);
    let affiliate_order = get_bool("riskdata.affiliateOrder", &metadata);

    Some(RiskData {
        item_i_d,
        product_title,
        amount_per_item,
        currency,
        upc,
        brand,
        manufacturer,
        category,
        quantity,
        color,
        size,
        device_country,
        house_numberor_name,
        account_creation_date,
        affiliate_channel,
        avg_order_value,
        delivery_method,
        email_name,
        email_domain,
        last_order_date,
        merchant_reference,
        payment_method,
        promotion_name,
        secondary_phone_number: secondary_phone_number.map(Secret::new),
        timefrom_loginto_order,
        total_session_time,
        total_authorized_amount_in_last30_days,
        total_order_quantity,
        total_lifetime_value,
        visits_month,
        visits_week,
        visits_year,
        ship_to_name,
        first8charactersof_address_line1_zip,
        affiliate_order,
    })
}

fn get_str(key: &str, riskdata: &serde_json::Value) -> Option<String> {
    riskdata
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn get_bool(key: &str, riskdata: &serde_json::Value) -> Option<bool> {
    riskdata.get(key).and_then(|v| v.as_bool())
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AdyenRedirectRequest {
    pub details: AdyenRedirectRequestTypes,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum AdyenRedirectRequestTypes {
    AdyenRedirection(AdyenRedirection),
    AdyenThreeDS(AdyenThreeDS),
    AdyenRefusal(AdyenRefusal),
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdyenRedirection {
    pub redirect_result: String,
    #[serde(rename = "type")]
    pub type_of_redirection_result: Option<String>,
    pub result_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdyenThreeDS {
    #[serde(rename = "threeDSResult")]
    pub three_ds_result: String,
    #[serde(rename = "type")]
    pub type_of_redirection_result: Option<String>,
    pub result_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdyenRefusal {
    pub payload: String,
    #[serde(rename = "type")]
    pub type_of_redirection_result: Option<String>,
    pub result_code: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenRefundRequest {
    merchant_account: Secret<String>,
    amount: Amount,
    merchant_refund_reason: Option<String>,
    reference: String,
    splits: Option<Vec<AdyenSplitData>>,
    store: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenRefundResponse {
    merchant_account: Secret<String>,
    psp_reference: String,
    payment_psp_reference: String,
    reference: String,
    status: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for AdyenRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;

        Ok(Self {
            merchant_account: auth_type.merchant_account,
            amount: Amount {
                currency: item.router_data.request.currency,
                value: item.router_data.request.minor_refund_amount,
            },
            merchant_refund_reason: item.router_data.request.reason.clone(),
            reference: item.router_data.request.refund_id.clone(),
            store: None,
            splits: None,
        })
    }
}

impl<F, Req> TryFrom<ResponseRouterData<AdyenRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(value: ResponseRouterData<AdyenRefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let status = RefundStatus::Pending;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: response.psp_reference,
            refund_status: status,
            status_code: http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: Ok(refunds_response_data),
            ..router_data
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenCaptureRequest {
    merchant_account: Secret<String>,
    amount: Amount,
    reference: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AdyenCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let reference = match item.router_data.request.multiple_capture_data.clone() {
            // if multiple capture request, send capture_id as our reference for the capture
            Some(multiple_capture_request_data) => multiple_capture_request_data.capture_reference,
            // if single capture request, send connector_request_reference_id(attempt_id)
            None => item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        };
        Ok(Self {
            merchant_account: auth_type.merchant_account,
            reference,
            amount: Amount {
                currency: item.router_data.request.currency,
                value: item.router_data.request.minor_amount_to_capture.to_owned(),
            },
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenCaptureResponse {
    merchant_account: Secret<String>,
    payment_psp_reference: String,
    psp_reference: String,
    reference: String,
    status: String,
    amount: Amount,
    merchant_reference: Option<String>,
    store: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<AdyenCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(
        value: ResponseRouterData<AdyenCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let is_multiple_capture_psync_flow = router_data.request.multiple_capture_data.is_some();
        let connector_transaction_id = if is_multiple_capture_psync_flow {
            response.psp_reference.clone()
        } else {
            response.payment_psp_reference
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.reference),
                incremental_authorization_allowed: None,
                mandate_reference: None,
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Pending,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &Card<T>,
    )> for SetupMandateRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    SetupMandate,
                    PaymentFlowData,
                    SetupMandateRequestData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &Card<T>,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, card_data) = value;
        let amount = get_amount_data_for_setup_mandate(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let shopper_reference = match item
            .router_data
            .resource_common_data
            .connector_customer
            .clone()
        {
            Some(connector_customer_id) => Some(connector_customer_id),
            None => match item.router_data.request.customer_id.clone() {
                Some(customer_id) => Some(format!(
                    "{}_{}",
                    item.router_data
                        .resource_common_data
                        .merchant_id
                        .get_string_repr(),
                    customer_id.get_string_repr()
                )),
                None => None,
            },
        };
        let (recurring_processing_model, store_payment_method, _) =
            get_recurring_processing_model_for_setup_mandate(&item.router_data)?;

        let return_url = item.router_data.request.router_return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            },
        )?;

        let billing_address = get_address_info(
            item.router_data
                .resource_common_data
                .address
                .get_payment_billing(),
        )
        .and_then(Result::ok);

        let testing_data = item
            .router_data
            .request
            .get_connector_testing_data()
            .map(AdyenTestingData::try_from)
            .transpose()?;
        let test_holder_name = testing_data.and_then(|test_data| test_data.holder_name);
        let card_holder_name = test_holder_name.or_else(|| {
            item.router_data
                .resource_common_data
                .get_optional_billing_full_name()
        });

        let additional_data = get_additional_data_for_setup_mandate(&item.router_data);

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();

        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((card_data, card_holder_name))?,
        ));

        Ok(Self(AdyenPaymentRequest {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model,
            browser_info: get_browser_info_for_setup_mandate(&item.router_data)?,
            additional_data,
            mpi_data: None,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            shopper_name: get_shopper_name(
                item.router_data
                    .resource_common_data
                    .address
                    .get_payment_billing(),
            ),
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            shopper_locale: item.router_data.request.locale.clone(),
            social_security_number: None,
            billing_address,
            delivery_address: get_address_info(
                item.router_data
                    .resource_common_data
                    .get_optional_shipping(),
            )
            .and_then(Result::ok),
            country_code: get_country_code(
                item.router_data.resource_common_data.get_optional_billing(),
            ),
            line_items: None,
            shopper_reference,
            store_payment_method,
            channel: None,
            shopper_statement: item
                .router_data
                .request
                .billing_descriptor
                .clone()
                .and_then(|descriptor| descriptor.statement_descriptor),
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            store: None,
            splits: None,
            device_fingerprint,
            metadata: None,
            platform_chargeback_logic,
            session_validity: None,
        }))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for SetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item
            .router_data
            .request
            .mandate_id
            .to_owned()
            .and_then(|mandate_ids| mandate_ids.mandate_reference_id)
        {
            Some(_mandate_ref) => Err(IntegrationError::not_implemented("payment_method").into()),
            None => match item.router_data.request.payment_method_data.clone() {
                PaymentMethodData::Card(ref card) => Self::try_from((item, card)).map_err(|err| {
                    err.change_context(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })
                }),
                PaymentMethodData::Wallet(_)
                | PaymentMethodData::PayLater(_)
                | PaymentMethodData::BankRedirect(_)
                | PaymentMethodData::BankDebit(_)
                | PaymentMethodData::BankTransfer(_)
                | PaymentMethodData::CardRedirect(_)
                | PaymentMethodData::Voucher(_)
                | PaymentMethodData::GiftCard(_)
                | PaymentMethodData::Crypto(_)
                | PaymentMethodData::MandatePayment
                | PaymentMethodData::Reward
                | PaymentMethodData::RealTimePayment(_)
                | PaymentMethodData::Upi(_)
                | PaymentMethodData::OpenBanking(_)
                | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
                | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
                | PaymentMethodData::NetworkToken(_)
                | PaymentMethodData::MobilePayment(_)
                | PaymentMethodData::Netbanking(_)
                | PaymentMethodData::CardToken(_) => {
                    Err(IntegrationError::not_implemented("payment method").into())
                }
            },
        }
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<SetupMandateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;
    fn try_from(
        value: ResponseRouterData<SetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let pmt = router_data.request.payment_method_type;
        let is_manual_capture = false;
        // Unwrap the response wrapper to get AdyenPaymentResponse
        let SetupMandateResponse(adyen_response) = response;

        // Process response using existing helper functions (same as Authorize/RepeatPayment)
        let adyen_payments_response_data = match adyen_response {
            AdyenPaymentResponse::Response(response) => {
                get_adyen_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::PresentToShopper(response) => {
                get_present_to_shopper_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::QrCodeResponse(response) => {
                get_qr_code_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::RedirectionResponse(response) => {
                get_redirection_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::RedirectionErrorResponse(response) => {
                get_redirection_error_response(*response, is_manual_capture, http_code, pmt)?
            }
            AdyenPaymentResponse::WebhookResponse(response) => get_webhook_response(
                *response,
                is_manual_capture,
                false, // is_multiple_capture_psync_flow = false for setup mandate
                http_code,
            )?,
        };

        let minor_amount_captured = match adyen_payments_response_data.status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => adyen_payments_response_data.txn_amount,
            _ => None,
        };

        Ok(Self {
            response: adyen_payments_response_data.error.map_or_else(
                || Ok(adyen_payments_response_data.payments_response_data),
                Err,
            ),
            resource_common_data: PaymentFlowData {
                status: adyen_payments_response_data.status,
                amount_captured: minor_amount_captured.map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured,
                connector_response: adyen_payments_response_data.connector_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

fn get_amount_data_for_setup_mandate<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &AdyenRouterData<
        RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
        T,
    >,
) -> Amount {
    Amount {
        currency: item.router_data.request.currency,
        value: MinorUnit::new(item.router_data.request.amount.unwrap_or(0)),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    From<
        &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    > for AdyenShopperInteraction
{
    fn from(
        item: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> Self {
        match item.request.off_session {
            Some(true) => Self::ContinuedAuthentication,
            _ => Self::Ecommerce,
        }
    }
}

fn get_recurring_processing_model_for_setup_mandate<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> Result<RecurringDetails, Error> {
    let customer_id = item
        .request
        .customer_id
        .clone()
        .ok_or_else(Box::new(move || IntegrationError::MissingRequiredField {
            field_name: "customer_id",
            context: Default::default(),
        }))?;

    match (item.request.setup_future_usage, item.request.off_session) {
        (Some(common_enums::FutureUsage::OffSession), _) => {
            let shopper_reference = format!(
                "{}_{}",
                item.resource_common_data.merchant_id.get_string_repr(),
                customer_id.get_string_repr()
            );
            let store_payment_method = is_mandate_payment_for_setup_mandate(item);
            Ok((
                Some(AdyenRecurringModel::UnscheduledCardOnFile),
                Some(store_payment_method),
                Some(shopper_reference),
            ))
        }
        (_, Some(true)) => Ok((
            Some(AdyenRecurringModel::UnscheduledCardOnFile),
            None,
            Some(format!(
                "{}_{}",
                item.resource_common_data.merchant_id.get_string_repr(),
                customer_id.get_string_repr()
            )),
        )),
        _ => Ok((None, None, None)),
    }
}

fn get_additional_data_for_setup_mandate<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> Option<AdditionalData> {
    let (authorisation_type, manual_capture) = match item.request.capture_method {
        Some(common_enums::CaptureMethod::Manual)
        | Some(common_enums::CaptureMethod::ManualMultiple) => {
            (Some(AuthType::PreAuth), Some("true".to_string()))
        }
        _ => (None, None),
    };
    let riskdata = item
        .request
        .metadata
        .clone()
        .expose_option()
        .and_then(get_risk_data);

    let execute_three_d = if matches!(
        item.resource_common_data.auth_type,
        common_enums::AuthenticationType::ThreeDs
    ) {
        Some("true".to_string())
    } else {
        Some("false".to_string())
    };

    Some(AdditionalData {
        authorisation_type,
        manual_capture,
        execute_three_d,
        network_tx_reference: None,
        recurring_detail_reference: None,
        recurring_shopper_reference: None,
        recurring_processing_model: None,
        riskdata,
        sca_exemption: None,
        ..AdditionalData::default()
    })
}

fn is_mandate_payment_for_setup_mandate<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> bool {
    (item.request.setup_future_usage == Some(common_enums::FutureUsage::OffSession))
        || item
            .request
            .mandate_id
            .as_ref()
            .and_then(|mandate_ids| mandate_ids.mandate_reference_id.as_ref())
            .is_some()
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let mandate_ref_id = item.router_data.request.mandate_reference.clone();
        let amount = Amount {
            currency: item.router_data.request.currency,
            value: item.router_data.request.minor_amount,
        };
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::ContinuedAuthentication;
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model_for_repeat_payment(&item.router_data)?;
        let browser_info = None;
        let additional_data = get_additional_data_for_repeat_payment(&item.router_data);
        let return_url = item.router_data.request.router_return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: Default::default(),
            },
        )?;
        let payment_method_type = item.router_data.request.payment_method_type;
        let testing_data = item
            .router_data
            .request
            .get_connector_testing_data()
            .map(AdyenTestingData::try_from)
            .transpose()?;
        let test_holder_name = testing_data.and_then(|test_data| test_data.holder_name);
        let payment_method = match mandate_ref_id {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => {
                let adyen_mandate = AdyenMandate {
                    payment_type: match payment_method_type {
                        Some(pm_type) => PaymentType::try_from(&pm_type).change_context(
                            IntegrationError::RequestEncodingFailed {
                                context: Default::default(),
                            },
                        )?,
                        None => PaymentType::Scheme,
                    },
                    stored_payment_method_id: Secret::new(
                        connector_mandate_ids.get_connector_mandate_id().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "connector_mandate_id",
                                context: Default::default(),
                            },
                        )?,
                    ),
                    holder_name: test_holder_name,
                };
                PaymentMethod::AdyenMandatePaymentMethod(Box::new(adyen_mandate))
            }
            MandateReferenceId::NetworkMandateId(network_mandate_id) => {
                match &item.router_data.request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(
                        ref card_details_for_network_transaction_id,
                    ) => {
                        let brand = match card_details_for_network_transaction_id
                            .card_network
                            .clone()
                            .and_then(get_adyen_card_network)
                        {
                            Some(card_network) => card_network,
                            None => CardBrand::try_from(
                                &card_details_for_network_transaction_id
                                    .get_card_issuer()
                                    .change_context(IntegrationError::RequestEncodingFailed {
                                        context: Default::default(),
                                    })?,
                            )
                            .change_context(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            )?,
                        };
                        let card_holder_name = item
                            .router_data
                            .resource_common_data
                            .get_optional_billing_full_name();
                        let raw_card_number = RawCardNumber(
                            card_details_for_network_transaction_id.card_number.clone(),
                        );
                        let adyen_card = AdyenCard {
                            number: raw_card_number,
                            expiry_month: card_details_for_network_transaction_id
                                .card_exp_month
                                .clone(),
                            expiry_year: card_details_for_network_transaction_id
                                .get_expiry_year_4_digit()
                                .clone(),
                            cvc: None,
                            holder_name: test_holder_name.or(card_holder_name),
                            brand: Some(brand),
                            network_payment_reference: Some(Secret::new(network_mandate_id)),
                        };
                        PaymentMethod::AdyenPaymentMethod(Box::new(AdyenPaymentMethod::AdyenCard(
                            Box::new(adyen_card),
                        )))
                    }
                    _ => {
                        return Err(error_stack::report!(IntegrationError::NotSupported {
                            message: "Network tokenization for payment method".to_string(),
                            connector: "Adyen",
                            context: Default::default()
                        }))
                    }
                }
            }
            MandateReferenceId::NetworkTokenWithNTI(network_mandate_id) => {
                match &item.router_data.request.payment_method_data {
                    PaymentMethodData::NetworkToken(ref token_data) => {
                        let card_issuer = token_data.get_card_issuer().change_context(
                            IntegrationError::RequestEncodingFailed {
                                context: Default::default(),
                            },
                        )?;
                        let brand = CardBrand::try_from(&card_issuer).change_context(
                            IntegrationError::RequestEncodingFailed {
                                context: Default::default(),
                            },
                        )?;
                        let card_holder_name = item
                            .router_data
                            .resource_common_data
                            .get_optional_billing_full_name();
                        let adyen_network_token = AdyenNetworkTokenData {
                            number: token_data.get_network_token(),
                            expiry_month: token_data.get_network_token_expiry_month(),
                            expiry_year: token_data.get_expiry_year_4_digit(),
                            holder_name: test_holder_name.or(card_holder_name),
                            brand: Some(brand),
                            network_payment_reference: Some(Secret::new(
                                network_mandate_id.network_transaction_id.clone(),
                            )),
                        };
                        PaymentMethod::AdyenPaymentMethod(Box::new(
                            AdyenPaymentMethod::NetworkToken(Box::new(adyen_network_token)),
                        ))
                    }
                    _ => {
                        return Err(error_stack::report!(IntegrationError::NotSupported {
                            message: "Network tokenization for payment method".to_string(),
                            connector: "Adyen",
                            context: Default::default()
                        }))
                    }
                }
            }
        };

        let adyen_metadata = get_adyen_metadata(
            item.router_data
                .request
                .metadata
                .clone()
                .map(|m| m.expose()),
        );

        let (store, splits) = (adyen_metadata.store.clone(), None);
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();

        let billing_address =
            get_address_info(item.router_data.resource_common_data.get_optional_billing())
                .and_then(Result::ok);
        let delivery_address = get_address_info(
            item.router_data
                .resource_common_data
                .get_optional_shipping(),
        )
        .and_then(Result::ok);
        let telephone_number = item
            .router_data
            .resource_common_data
            .get_optional_billing_phone_number();

        Ok(Self(AdyenPaymentRequest {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            mpi_data: None,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            browser_info,
            shopper_interaction,
            recurring_processing_model: recurring_processing_model.clone(),
            additional_data,
            shopper_reference: shopper_reference.clone(),
            store_payment_method,
            shopper_ip: item.router_data.request.get_ip_address_as_optional(),
            shopper_name: None,
            shopper_locale: item.router_data.request.locale.clone(),
            shopper_email: None,
            shopper_statement: get_shopper_statement_for_repeat_payment(&item.router_data),
            social_security_number: None,
            telephone_number,
            billing_address,
            delivery_address,
            country_code: None,
            line_items: None,
            channel: None,
            merchant_order_reference: item.router_data.request.merchant_order_id.clone(),
            splits,
            store,
            device_fingerprint,
            metadata: item
                .router_data
                .request
                .metadata
                .clone()
                .map(|value| Secret::new(filter_adyen_metadata(value.expose()))),
            platform_chargeback_logic,
            session_validity: None,
        }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDisputeAcceptRequest {
    pub dispute_psp_reference: String,
    pub merchant_account_code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    > for AdyenDisputeAcceptRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AdyenAuthType::try_from(&item.router_data.connector_config)?;

        Ok(Self {
            dispute_psp_reference: item
                .router_data
                .resource_common_data
                .connector_dispute_id
                .clone(),
            merchant_account_code: auth.merchant_account.peek().to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDisputeAcceptResponse {
    pub dispute_service_result: Option<DisputeServiceResult>,
}

impl<F, Req> TryFrom<ResponseRouterData<AdyenDisputeAcceptResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        value: ResponseRouterData<AdyenDisputeAcceptResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let success = response
            .dispute_service_result
            .as_ref()
            .is_some_and(|r| r.success);

        if success {
            let status = common_enums::DisputeStatus::DisputeAccepted;

            let dispute_response_data = DisputeResponseData {
                dispute_status: status,
                connector_dispute_id: router_data
                    .resource_common_data
                    .connector_dispute_id
                    .clone(),
                connector_dispute_status: None,
                status_code: http_code,
            };

            Ok(Self {
                resource_common_data: DisputeFlowData {
                    ..router_data.resource_common_data
                },
                response: Ok(dispute_response_data),
                ..router_data
            })
        } else {
            let error_message = response
                .dispute_service_result
                .as_ref()
                .and_then(|r| r.error_message.clone())
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());

            let error_response = ErrorResponse {
                code: NO_ERROR_CODE.to_string(),
                message: error_message.clone(),
                reason: Some(error_message.clone()),
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: router_data.resource_common_data.clone(),
                response: Err(error_response),
                ..router_data
            })
        }
    }
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDisputeSubmitEvidenceRequest {
    defense_documents: Vec<DefenseDocuments>,
    merchant_account_code: Secret<String>,
    dispute_psp_reference: String,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefenseDocuments {
    content: Secret<String>,
    content_type: Option<String>,
    defense_document_type_code: String,
}

fn get_defence_documents(item: SubmitEvidenceData) -> Option<Vec<DefenseDocuments>> {
    let mut defense_documents: Vec<DefenseDocuments> = Vec::new();
    if let Some(shipping_documentation) = item.shipping_documentation {
        defense_documents.push(DefenseDocuments {
            content: get_content(shipping_documentation).into(),
            content_type: item.shipping_documentation_provider_file_id,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(receipt) = item.receipt {
        defense_documents.push(DefenseDocuments {
            content: get_content(receipt).into(),
            content_type: item.receipt_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(invoice_showing_distinct_transactions) = item.invoice_showing_distinct_transactions
    {
        defense_documents.push(DefenseDocuments {
            content: get_content(invoice_showing_distinct_transactions).into(),
            content_type: item.invoice_showing_distinct_transactions_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(customer_communication) = item.customer_communication {
        defense_documents.push(DefenseDocuments {
            content: get_content(customer_communication).into(),
            content_type: item.customer_communication_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(refund_policy) = item.refund_policy {
        defense_documents.push(DefenseDocuments {
            content: get_content(refund_policy).into(),
            content_type: item.refund_policy_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(recurring_transaction_agreement) = item.recurring_transaction_agreement {
        defense_documents.push(DefenseDocuments {
            content: get_content(recurring_transaction_agreement).into(),
            content_type: item.recurring_transaction_agreement_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(uncategorized_file) = item.uncategorized_file {
        defense_documents.push(DefenseDocuments {
            content: get_content(uncategorized_file).into(),
            content_type: item.uncategorized_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(cancellation_policy) = item.cancellation_policy {
        defense_documents.push(DefenseDocuments {
            content: get_content(cancellation_policy).into(),
            content_type: item.cancellation_policy_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(customer_signature) = item.customer_signature {
        defense_documents.push(DefenseDocuments {
            content: get_content(customer_signature).into(),
            content_type: item.customer_signature_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }
    if let Some(service_documentation) = item.service_documentation {
        defense_documents.push(DefenseDocuments {
            content: get_content(service_documentation).into(),
            content_type: item.service_documentation_file_type,
            defense_document_type_code: "DefenseMaterial".into(),
        })
    }

    if defense_documents.is_empty() {
        None
    } else {
        Some(defense_documents)
    }
}

fn get_content(item: Vec<u8>) -> String {
    STANDARD.encode(item)
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
            T,
        >,
    > for AdyenDisputeSubmitEvidenceRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = AdyenAuthType::try_from(&item.router_data.connector_config)?;

        Ok(Self {
            defense_documents: get_defence_documents(item.router_data.request.clone()).ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "Missing Defence Documents",
                    context: Default::default(),
                },
            )?,
            merchant_account_code: auth.merchant_account.peek().to_string().into(),
            dispute_psp_reference: item.router_data.request.connector_dispute_id.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenSubmitEvidenceResponse {
    pub dispute_service_result: Option<DisputeServiceResult>,
}

impl<F, Req> TryFrom<ResponseRouterData<AdyenSubmitEvidenceResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        value: ResponseRouterData<AdyenSubmitEvidenceResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let success = response
            .dispute_service_result
            .as_ref()
            .is_some_and(|r| r.success);

        if success {
            let status = common_enums::DisputeStatus::DisputeChallenged;

            let dispute_response_data = DisputeResponseData {
                dispute_status: status,
                connector_dispute_id: router_data
                    .resource_common_data
                    .connector_dispute_id
                    .clone(),
                connector_dispute_status: None,
                status_code: http_code,
            };

            Ok(Self {
                resource_common_data: DisputeFlowData {
                    ..router_data.resource_common_data
                },
                response: Ok(dispute_response_data),
                ..router_data
            })
        } else {
            let error_message = response
                .dispute_service_result
                .as_ref()
                .and_then(|r| r.error_message.clone())
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());

            let error_response = ErrorResponse {
                code: NO_ERROR_CODE.to_string(),
                message: error_message.clone(),
                reason: Some(error_message.clone()),
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: router_data.resource_common_data.clone(),
                response: Err(error_response),
                ..router_data
            })
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDefendDisputeRequest {
    dispute_psp_reference: String,
    merchant_account_code: Secret<String>,
    defense_reason_code: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
            T,
        >,
    > for AdyenDefendDisputeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;

        Ok(Self {
            dispute_psp_reference: item.router_data.request.connector_dispute_id.clone(),
            merchant_account_code: auth_type.merchant_account.clone(),
            defense_reason_code: item.router_data.request.defense_reason_code.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum AdyenDefendDisputeResponse {
    DefendDisputeSuccessResponse(DefendDisputeSuccessResponse),
    DefendDisputeFailedResponse(DefendDisputeErrorResponse),
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefendDisputeErrorResponse {
    error_code: String,
    error_type: String,
    message: String,
    psp_reference: String,
    status: String,
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefendDisputeSuccessResponse {
    dispute_service_result: DisputeServiceResult,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisputeServiceResult {
    error_message: Option<String>,
    success: bool,
}

impl<F, Req> TryFrom<ResponseRouterData<AdyenDefendDisputeResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        value: ResponseRouterData<AdyenDefendDisputeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        match response {
            AdyenDefendDisputeResponse::DefendDisputeSuccessResponse(result) => {
                let dispute_status = if result.dispute_service_result.success {
                    common_enums::DisputeStatus::DisputeWon
                } else {
                    common_enums::DisputeStatus::DisputeLost
                };

                Ok(Self {
                    response: Ok(DisputeResponseData {
                        dispute_status,
                        connector_dispute_status: None,
                        connector_dispute_id: router_data
                            .resource_common_data
                            .connector_dispute_id
                            .clone(),
                        status_code: http_code,
                    }),
                    ..router_data
                })
            }

            AdyenDefendDisputeResponse::DefendDisputeFailedResponse(result) => Ok(Self {
                response: Err(ErrorResponse {
                    code: result.error_code,
                    message: result.message.clone(),
                    reason: Some(result.message),
                    status_code: http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(result.psp_reference),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data
            }),
        }
    }
}

pub(crate) fn get_dispute_stage_and_status(
    code: WebhookEventCode,
    dispute_status: Option<DisputeStatus>,
) -> (common_enums::DisputeStage, common_enums::DisputeStatus) {
    use common_enums::{DisputeStage, DisputeStatus as HSDisputeStatus};

    match code {
        WebhookEventCode::NotificationOfChargeback => {
            (DisputeStage::PreDispute, HSDisputeStatus::DisputeOpened)
        }
        WebhookEventCode::Chargeback => {
            let status = match dispute_status {
                Some(DisputeStatus::Undefended) | Some(DisputeStatus::Pending) => {
                    HSDisputeStatus::DisputeOpened
                }
                Some(DisputeStatus::Lost) | None => HSDisputeStatus::DisputeLost,
                Some(DisputeStatus::Accepted) => HSDisputeStatus::DisputeAccepted,
                Some(DisputeStatus::Won) => HSDisputeStatus::DisputeWon,
            };
            (DisputeStage::Dispute, status)
        }
        WebhookEventCode::ChargebackReversed => {
            let status = match dispute_status {
                Some(DisputeStatus::Pending) => HSDisputeStatus::DisputeChallenged,
                _ => HSDisputeStatus::DisputeWon,
            };
            (DisputeStage::Dispute, status)
        }
        WebhookEventCode::SecondChargeback => {
            (DisputeStage::PreArbitration, HSDisputeStatus::DisputeLost)
        }
        WebhookEventCode::PrearbitrationWon => {
            let status = match dispute_status {
                Some(DisputeStatus::Pending) => HSDisputeStatus::DisputeOpened,
                _ => HSDisputeStatus::DisputeWon,
            };
            (DisputeStage::PreArbitration, status)
        }
        WebhookEventCode::PrearbitrationLost => {
            (DisputeStage::PreArbitration, HSDisputeStatus::DisputeLost)
        }
        _ => (DisputeStage::Dispute, HSDisputeStatus::DisputeOpened),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdyenPlatformChargeBackBehaviour {
    #[serde(alias = "deduct_from_liable_account")]
    DeductFromLiableAccount,
    #[serde(alias = "deduct_from_one_balance_account")]
    DeductFromOneBalanceAccount,
    #[serde(alias = "deduct_according_to_split_ratio")]
    DeductAccordingToSplitRatio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenPlatformChargeBackLogicMetadata {
    pub behavior: Option<AdyenPlatformChargeBackBehaviour>,
    #[serde(alias = "target_account")]
    pub target_account: Option<Secret<String>>,
    #[serde(alias = "cost_allocation_account")]
    pub cost_allocation_account: Option<Secret<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdyenMetadata {
    #[serde(alias = "device_fingerprint")]
    pub device_fingerprint: Option<Secret<String>>,
    pub store: Option<String>,
    #[serde(alias = "platform_chargeback_logic")]
    pub platform_chargeback_logic: Option<AdyenPlatformChargeBackLogicMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdyenConnectorMetadataObject {
    pub endpoint_prefix: Option<String>,
}

impl TryFrom<&Option<SecretSerdeValue>> for AdyenConnectorMetadataObject {
    type Error = Error;
    fn try_from(meta_data: &Option<SecretSerdeValue>) -> Result<Self, Self::Error> {
        match meta_data {
            Some(metadata) => to_connector_meta_from_secret::<Self>(Some(metadata.clone()))
                .change_context(IntegrationError::InvalidConnectorConfig {
                    config: "metadata",
                    context: Default::default(),
                }),
            None => Ok(Self::default()),
        }
    }
}

fn get_adyen_metadata(metadata: Option<serde_json::Value>) -> AdyenMetadata {
    metadata
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn filter_adyen_metadata(metadata: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(mut map) = metadata.clone() {
        // Remove the fields that are specific to Adyen and should not be passed in metadata
        map.remove("device_fingerprint");
        map.remove("deviceFingerprint");
        map.remove("platform_chargeback_logic");
        map.remove("platformChargebackLogic");
        map.remove("store");

        serde_json::Value::Object(map)
    } else {
        metadata.clone()
    }
}

pub fn get_device_fingerprint(metadata: serde_json::Value) -> Option<Secret<String>> {
    metadata
        .get("device_fingerprint")
        .and_then(|v| v.as_str())
        .map(|fingerprint| Secret::new(fingerprint.to_string()))
}

fn get_browser_info<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<Option<AdyenBrowserInfo>, Error> {
    if router_data.resource_common_data.auth_type == common_enums::AuthenticationType::ThreeDs
        || router_data.resource_common_data.payment_method == common_enums::PaymentMethod::Card
        || router_data.resource_common_data.payment_method
            == common_enums::PaymentMethod::BankRedirect
        || router_data.request.payment_method_type == Some(common_enums::PaymentMethodType::GoPay)
        || router_data.request.payment_method_type
            == Some(common_enums::PaymentMethodType::GooglePay)
    {
        let info = router_data.request.get_browser_info()?;
        Ok(Some(AdyenBrowserInfo {
            accept_header: info.get_accept_header()?,
            language: info.get_language()?,
            screen_height: info.get_screen_height()?,
            screen_width: info.get_screen_width()?,
            color_depth: info.get_color_depth()?,
            user_agent: info.get_user_agent()?,
            time_zone_offset: info.get_time_zone()?,
            java_enabled: info.get_java_enabled()?,
        }))
    } else {
        Ok(None)
    }
}

fn get_browser_info_for_setup_mandate<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >,
) -> Result<Option<AdyenBrowserInfo>, Error> {
    if router_data.resource_common_data.auth_type == common_enums::AuthenticationType::ThreeDs
        || router_data.resource_common_data.payment_method == common_enums::PaymentMethod::Card
        || router_data.resource_common_data.payment_method
            == common_enums::PaymentMethod::BankRedirect
        || router_data.request.payment_method_type == Some(common_enums::PaymentMethodType::GoPay)
        || router_data.request.payment_method_type
            == Some(common_enums::PaymentMethodType::GooglePay)
    {
        let info = router_data.request.get_browser_info()?;
        Ok(Some(AdyenBrowserInfo {
            accept_header: info.get_accept_header()?,
            language: info.get_language()?,
            screen_height: info.get_screen_height()?,
            screen_width: info.get_screen_width()?,
            color_depth: info.get_color_depth()?,
            user_agent: info.get_user_agent()?,
            time_zone_offset: info.get_time_zone()?,
            java_enabled: info.get_java_enabled()?,
        }))
    } else {
        Ok(None)
    }
}

fn get_shopper_name(
    address: Option<&domain_types::payment_address::Address>,
) -> Option<ShopperName> {
    let billing = address.and_then(|billing| billing.address.as_ref());
    Some(ShopperName {
        first_name: billing.and_then(|a| a.first_name.clone()),
        last_name: billing.and_then(|a| a.last_name.clone()),
    })
}

fn get_country_code(
    address: Option<&domain_types::payment_address::Address>,
) -> Option<common_enums::CountryAlpha2> {
    address.and_then(|billing| billing.address.as_ref().and_then(|address| address.country))
}

fn get_line_items<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &AdyenRouterData<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        T,
    >,
) -> Vec<LineItem> {
    let order_details = item.router_data.resource_common_data.order_details.clone();
    match order_details {
        Some(od) => od
            .iter()
            .enumerate()
            .map(|(i, data)| LineItem {
                amount_including_tax: Some(data.amount),
                amount_excluding_tax: Some(data.amount),
                description: Some(data.product_name.clone()),
                id: Some(format!("Items #{i}")),
                tax_amount: None,
                quantity: Some(data.quantity),
            })
            .collect(),
        None => {
            let line_item = LineItem {
                amount_including_tax: Some(item.router_data.request.amount),
                amount_excluding_tax: Some(item.router_data.request.amount),
                description: item.router_data.resource_common_data.description.clone(),
                id: Some(String::from("Items #1")),
                tax_amount: None,
                quantity: Some(1),
            };
            vec![line_item]
        }
    }
}

pub fn get_present_to_shopper_metadata(
    response: &PresentToShopperResponse,
) -> CustomResult<Option<serde_json::Value>, ConnectorResponseTransformationError> {
    let reference = response.action.reference.clone();
    let expires_at = response
        .action
        .expires_at
        .map(|time| get_timestamp_in_milliseconds(&time));
    match response.action.payment_method_type {
        // Supported voucher payment methods
        PaymentType::Alfamart
        | PaymentType::Indomaret
        | PaymentType::BoletoBancario
        | PaymentType::Oxxo => {
            let voucher_data = VoucherNextStepData {
                expires_at,
                reference,
                download_url: response.action.download_url.clone().map(|u| u.to_string()),
                instructions_url: response
                    .action
                    .instructions_url
                    .clone()
                    .map(|u| u.to_string()),
                entry_date: None,
                digitable_line: None,
                qr_code_url: None,
                barcode: None,
                expiry_date: None,
            };

            Some(voucher_data.encode_to_value())
                .transpose()
                .change_context(
                ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(
                ),
            )
        }
        // NOTE: Support for other payment methods will be added in future iterations
        // - Bank transfer methods (PermataBankTransfer, BcaBankTransfer, BniVa, BriVa, CimbVa, DanamonVa, MandiriVa)
        // - Pay later methods (Affirm, Afterpaytouch, ClearPay, Klarna, Atome, Alma, PayBright, Walley)
        // - Wallet methods (Alipay, AlipayHk, Applepay, Bizum, Gcash, Googlepay, GoPay, KakaoPay, Mbway, MobilePay, Momo, MomoAtm, PayPal, Samsungpay, TouchNGo, Twint, Vipps, Swish, WeChatPayWeb)
        // - Other methods (Blik, Dana, Eps, Ideal, Knet, Benefit, Pix, Trustly, SepaDirectDebit, BacsDirectDebit, AchDirectDebit, etc.)
        // - voucher or bank transfer methods would require special metadata, so return None for all cases
        // - for vouchers metadata support is added as it needs download url
        PaymentType::PermataBankTransfer
        | PaymentType::BcaBankTransfer
        | PaymentType::BniVa
        | PaymentType::BriVa
        | PaymentType::CimbVa
        | PaymentType::DanamonVa
        | PaymentType::Giftcard
        | PaymentType::MandiriVa
        | PaymentType::Affirm
        | PaymentType::Afterpaytouch
        | PaymentType::Alipay
        | PaymentType::AlipayHk
        | PaymentType::Alma
        | PaymentType::Applepay
        | PaymentType::Bizum
        | PaymentType::Atome
        | PaymentType::Blik
        | PaymentType::ClearPay
        | PaymentType::Dana
        | PaymentType::Eps
        | PaymentType::Gcash
        | PaymentType::Googlepay
        | PaymentType::GoPay
        | PaymentType::Ideal
        | PaymentType::Klarna
        | PaymentType::Kakaopay
        | PaymentType::Mbway
        | PaymentType::Knet
        | PaymentType::Benefit
        | PaymentType::MobilePay
        | PaymentType::Momo
        | PaymentType::MomoAtm
        | PaymentType::OnlineBankingCzechRepublic
        | PaymentType::OnlineBankingFinland
        | PaymentType::OnlineBankingPoland
        | PaymentType::OnlineBankingSlovakia
        | PaymentType::OnlineBankingFpx
        | PaymentType::OnlineBankingThailand
        | PaymentType::OpenBankingUK
        | PaymentType::PayBright
        | PaymentType::Paypal
        | PaymentType::Scheme
        | PaymentType::NetworkToken
        | PaymentType::Trustly
        | PaymentType::TouchNGo
        | PaymentType::Walley
        | PaymentType::WeChatPayWeb
        | PaymentType::AchDirectDebit
        | PaymentType::SepaDirectDebit
        | PaymentType::BacsDirectDebit
        | PaymentType::Samsungpay
        | PaymentType::Twint
        | PaymentType::Vipps
        | PaymentType::Swish
        | PaymentType::PaySafeCard
        | PaymentType::SevenEleven
        | PaymentType::Lawson
        | PaymentType::Pix => Ok(None),
    }
}

impl AdditionalData {
    // Split merchant advice code into at most 2 parts and get the first part and trim spaces,
    // Return the first part as a String.
    pub fn extract_network_advice_code(&self) -> Option<String> {
        self.merchant_advice_code.as_ref().and_then(|code| {
            let mut parts = code.splitn(2, ':');
            let first_part = parts.next()?.trim();
            // Ensure there is a second part (meaning ':' was present).
            parts.next()?;
            Some(first_part.to_string())
        })
    }
}

fn get_adyen_split_request(
    metadata: &Option<SecretSerdeValue>,
    adyen_store: &Option<String>,
    currency: common_enums::Currency,
) -> (Option<String>, Option<Vec<AdyenSplitData>>) {
    metadata
        .as_ref()
        .and_then(|secret| {
            serde_json::from_value::<AdyenSplitPaymentRequest>(secret.clone().expose()).ok()
        })
        .map(|split_request| {
            let splits: Vec<AdyenSplitData> = split_request
                .split_items
                .into_iter()
                .map(|split_item| {
                    let amount = split_item.amount.map(|value| Amount { currency, value });
                    AdyenSplitData {
                        amount,
                        reference: split_item.reference,
                        split_type: split_item.split_type,
                        account: split_item.account,
                        description: split_item.description,
                    }
                })
                .collect();
            let store = split_request.store.clone().or(adyen_store.clone());
            (store, Some(splits))
        })
        .unwrap_or_else(|| (adyen_store.clone(), None))
}
