use base64::{engine::general_purpose::STANDARD, Engine};
use cards::{CardNumber, NetworkToken};
use common_enums::{self, AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    ext_traits::{ByteSliceExt, Encode, OptionExt, ValueExt},
    request::Method,
    types::{AmountConvertor, MinorUnit, SemanticVersion, StringMinorUnit},
    SecretSerdeValue,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder,
        DefendDispute, IncrementalAuthorization, PSync, PostAuthenticate, PreAuthenticate, Refund,
        RepeatPayment, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        self, AcceptDisputeData,
        AdyenClientAuthenticationResponse as AdyenClientAuthenticationResponseDomain,
        AdyenSplitItem, CardDetailUpdate, ClientAuthenticationTokenData,
        ClientAuthenticationTokenRequestData, ConnectorSpecificClientAuthenticationResponse,
        ConnectorSplitResponseData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        EventType, MandateReference, MandateReferenceId, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodUpdate, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
        SplitPaymentsDetails, SubmitEvidenceData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, BankRedirectData, BankTransferData, Card,
        CardRedirectData, DefaultPCIHolder, GiftCardData, GpayTokenizationData, NetworkTokenData,
        PayLaterData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, TokenSource,
        VoucherData, VoucherNextStepData, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, ExtendedAuthorizationResponseData, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, SyncRequestType},
    router_response_types::RedirectForm,
    utils::{self as domain_utils, get_timestamp_in_milliseconds},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use time::{Duration, PrimitiveDateTime};
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
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};

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

/// `paymentMethod` for a merchant-initiated payment with a merchant/Hyperswitch-decrypted Apple
/// Pay or Google Pay token: the decrypted card fields with `brand` `applepay`/`googlepay`, the
/// network transaction id of the initial payment, and no `mpiData` (the shopper is not present).
/// https://docs.adyen.com/payment-methods/apple-pay/api-only/apple-pay-token-decryption
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDecryptedWalletMitData {
    number: NetworkToken,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    holder_name: Option<Secret<String>>,
    brand: String,
    network_payment_reference: Option<Secret<String>>,
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
/// `paymentMethod.brand` for a merchant/Hyperswitch-decrypted Apple Pay token
/// (https://docs.adyen.com/payment-methods/apple-pay/api-only/apple-pay-token-decryption).
const APPLE_PAY_BRAND: &str = "applepay";

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
    DecryptedWalletMit(Box<AdyenDecryptedWalletMitData>),
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
    // Wallet redirect payment methods
    #[serde(rename = "alipay")]
    AliPay,
    #[serde(rename = "alipay_hk")]
    AliPayHk,
    #[serde(rename = "dana")]
    Dana,
    #[serde(rename = "gcash")]
    Gcash(Box<GcashData>),
    #[serde(rename = "gopay_wallet")]
    GoPay(Box<GoPayData>),
    #[serde(rename = "kakaopay")]
    Kakaopay(Box<KakaoPayData>),
    #[serde(rename = "mbway")]
    Mbway(Box<MbwayData>),
    #[serde(rename = "mobilepay")]
    MobilePay,
    #[serde(rename = "momo_wallet")]
    Momo(Box<MomoData>),
    #[serde(rename = "touchngo")]
    TouchNGo(Box<TouchNGoData>),
    #[serde(rename = "wechatpayWeb")]
    WeChatPayWeb,
    #[serde(rename = "twint")]
    Twint,
    #[serde(rename = "vipps")]
    Vipps,
    #[serde(rename = "swish")]
    Swish,
    #[serde(rename = "paypal")]
    AdyenPaypal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlikRedirectionData {
    blik_code: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoPayData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KakaoPayData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcashData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomoData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchNGoData {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MbwayData {
    telephone_number: Secret<String>,
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
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
                Default::default(),
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
            _ => Err(IntegrationError::NotImplemented(
                ("payment method").into(),
                Default::default(),
            )
            .into()),
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
            _ => Err(IntegrationError::NotImplemented(
                ("payment method").into(),
                Default::default(),
            )
            .into()),
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
            _ => Err(IntegrationError::NotImplemented(
                ("payment method").into(),
                Default::default(),
            )
            .into()),
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
            _ => Err(IntegrationError::NotImplemented(
                ("payment method").into(),
                Default::default(),
            )
            .into()),
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
            _ => Err(IntegrationError::NotImplemented(
                ("payment method").into(),
                Default::default(),
            )
            .into()),
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
            _ => Err(IntegrationError::NotImplemented(
                ("payment method").into(),
                Default::default(),
            )
            .into()),
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

#[serde_with::skip_serializing_none]
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

fn shopper_interaction(
    off_session: Option<bool>,
    payment_channel: &Option<common_enums::PaymentChannel>,
) -> AdyenShopperInteraction {
    match off_session {
        Some(true) => AdyenShopperInteraction::ContinuedAuthentication,
        _ => match payment_channel {
            Some(
                common_enums::PaymentChannel::MailOrder
                | common_enums::PaymentChannel::TelephoneOrder,
            ) => AdyenShopperInteraction::Moto,
            Some(common_enums::PaymentChannel::Ecommerce) | None => {
                AdyenShopperInteraction::Ecommerce
            }
        },
    }
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
        shopper_interaction(item.request.off_session, &item.request.payment_channel)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AdyenRecurringModel {
    UnscheduledCardOnFile,
    CardOnFile,
    #[serde(other)]
    Unknown,
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
    /// Network-issued link id chaining related transactions (`transactionLinkId`).
    #[serde(rename = "transactionLinkId")]
    transaction_link_id: Option<String>,
    funds_availability: Option<String>,
    refusal_reason_raw: Option<String>,
    refusal_code_raw: Option<String>,
    /// Google Pay FPAN, see [`AdyenPaymentDataSource`].
    #[serde(flatten)]
    paymentdatasource: Option<AdyenPaymentDataSource>,
    merchant_advice_code: Option<String>,
    #[serde(flatten)]
    riskdata: Option<RiskData>,
    sca_exemption: Option<AdyenExemptionValues>,
    capture_delay_hours: Option<u64>,
    pub auth_code: Option<String>,
    /// Response only: AVS result, e.g. "7 Both postal code and address match"
    /// (https://docs.adyen.com/risk-management/configure-standard-risk-rules/consistency-rules#avs).
    avs_result: Option<String>,
    avs_result_raw: Option<String>,
    /// Response only: CVC result, e.g. "1 Matches".
    cvc_result: Option<String>,
    cvc_result_raw: Option<String>,
    /// Response only: the stored token id (replaces the deprecated
    /// `recurring.recurringDetailReference`).
    #[serde(rename = "tokenization.storedPaymentMethodId")]
    tokenization_stored_payment_method_id: Option<Secret<String>>,
    /// Request only: legacy (API < v70) Level 2/3 keys `enhancedSchemeData.*`. Adyen never
    /// returns them, so a response never populates this.
    #[serde(flatten, skip_deserializing)]
    level_two_three: Option<AdyenLevelTwoThreeData>,
}

impl AdditionalData {
    /// `payment_checks` for the connector response: the AVS / CVC results Adyen returned, with
    /// only the keys that are present. `None` when Adyen returned none of them.
    fn get_payment_checks(&self) -> Option<serde_json::Value> {
        let checks: serde_json::Map<String, serde_json::Value> = [
            ("avs_result", &self.avs_result),
            ("avs_result_raw", &self.avs_result_raw),
            ("cvc_result", &self.cvc_result),
            ("cvc_result_raw", &self.cvc_result_raw),
        ]
        .into_iter()
        .filter_map(|(key, value)| {
            value
                .clone()
                .map(|value| (key.to_string(), serde_json::Value::String(value)))
        })
        .collect();
        (!checks.is_empty()).then_some(serde_json::Value::Object(checks))
    }

    /// The connector mandate id: `tokenization.storedPaymentMethodId`, else the deprecated
    /// `recurring.recurringDetailReference` (both carry the same token on v68).
    fn get_stored_payment_method_id(&self) -> Option<Secret<String>> {
        self.tokenization_stored_payment_method_id
            .clone()
            .or_else(|| self.recurring_detail_reference.clone())
    }
}

/// Level 2/3 data as the legacy flat `additionalData` keys (`AdditionalDataLevel23`, used below
/// Checkout API v70): `enhancedSchemeData.customerReference`, `.totalTaxAmount`, ... and one
/// `enhancedSchemeData.itemDetailLine{n}.*` group per order line, numbered from 1. Request-only:
/// Adyen never returns these keys, so [`AdditionalData`] skips the field when it deserialises a
/// response.
/// https://docs.adyen.com/payment-methods/cards/enhanced-scheme-data/l2-l3
#[serde_with::skip_serializing_none]
#[derive(Clone, Default, Debug, Serialize)]
pub struct AdyenLevelTwoThreeData {
    #[serde(rename = "enhancedSchemeData.customerReference")]
    customer_reference: Option<String>,
    #[serde(rename = "enhancedSchemeData.totalTaxAmount")]
    total_tax_amount: Option<StringMinorUnit>,
    #[serde(rename = "enhancedSchemeData.freightAmount")]
    freight_amount: Option<StringMinorUnit>,
    #[serde(rename = "enhancedSchemeData.dutyAmount")]
    duty_amount: Option<StringMinorUnit>,
    #[serde(rename = "enhancedSchemeData.destinationPostalCode")]
    destination_postal_code: Option<Secret<String>>,
    #[serde(rename = "enhancedSchemeData.destinationCountryCode")]
    destination_country_code: Option<common_enums::CountryAlpha2>,
    #[serde(rename = "enhancedSchemeData.destinationStateProvinceCode")]
    destination_state_province_code: Option<Secret<String>>,
    #[serde(rename = "enhancedSchemeData.orderDate")]
    order_date: Option<String>,
    /// One `enhancedSchemeData.itemDetailLine{n}.*` group per order line, numbered from 1. The
    /// index is part of the key, which `#[serde(rename)]` cannot express, so the lines arrive
    /// already named from [`AdyenItemDetailLine::entries`].
    #[serde(flatten)]
    item_detail_lines: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Default, Debug)]
struct AdyenItemDetailLine {
    description: Option<String>,
    product_code: Option<String>,
    commodity_code: Option<String>,
    quantity: u16,
    unit_of_measure: Option<String>,
    unit_price: StringMinorUnit,
    discount_amount: Option<StringMinorUnit>,
    total_amount: StringMinorUnit,
}

impl AdyenItemDetailLine {
    /// The `enhancedSchemeData.itemDetailLine{index}.*` entries for this line, `index` numbered
    /// from 1. Every `additionalData` value is a string on the wire, so the amounts and the
    /// quantity are rendered as strings.
    fn entries(self, index: usize) -> impl Iterator<Item = (String, String)> {
        let prefix = format!("enhancedSchemeData.itemDetailLine{index}");
        [
            ("description", self.description),
            ("productCode", self.product_code),
            ("commodityCode", self.commodity_code),
            ("quantity", Some(self.quantity.to_string())),
            ("unitOfMeasure", self.unit_of_measure),
            ("unitPrice", Some(self.unit_price.to_string())),
            (
                "discountAmount",
                self.discount_amount.map(|amount| amount.to_string()),
            ),
            ("totalAmount", Some(self.total_amount.to_string())),
        ]
        .into_iter()
        .filter_map(move |(key, value)| value.map(|value| (format!("{prefix}.{key}"), value)))
    }
}

/// `additionalData.paymentdatasource.*`, sent for a decrypted Google Pay token that carries a
/// raw PAN and no cryptogram (FPAN / PAN_ONLY). Both the brand `googlepay` and a network token
/// reach Adyen as `paymentMethod.type = scheme`, so without this pair Adyen assumes a network
/// token, expects `mpiData`, and declines the payment with error 158
/// ("Required field 'MpiData' is not provided."). Mirrors the Hyperswitch Adyen connector.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct AdyenPaymentDataSource {
    #[serde(rename = "paymentdatasource.type")]
    data_type: String,
    #[serde(rename = "paymentdatasource.tokenized")]
    tokenized: String,
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
    email_name: Option<Secret<String>>,
    #[serde(rename = "riskdata.emailDomain")]
    email_domain: Option<Secret<String>>,
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
    split_type: common_enums::AdyenSplitType,
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
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
                Default::default(),
            )
            .into()),
        }
    }
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
    application_info: Option<ApplicationInfo>,
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInfo {
    external_platform: Option<ExternalPlatform>,
    merchant_application: Option<MerchantApplication>,
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalPlatform {
    name: Option<String>,
    version: Option<String>,
    integrator: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantApplication {
    name: Option<String>,
    version: Option<String>,
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
        value: item.router_data.request.amount.amount.to_owned(),
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
        common_enums::CardNetwork::Interac
        | common_enums::CardNetwork::Prop
        | common_enums::CardNetwork::PrivateLabel
        | common_enums::CardNetwork::Dinacard
        | common_enums::CardNetwork::AirPlus
        | common_enums::CardNetwork::Aurore
        | common_enums::CardNetwork::EftposAustralia
        | common_enums::CardNetwork::GeCapital
        | common_enums::CardNetwork::Uatp => None,
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
        let brand = if card.card_number.is_cobadged_card()? {
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

// Takes the flow-agnostic PaymentFlowData (not a flow-specific RouterDataV2) so both Authorize
// and SetupMandate can share this -- the only field ever read off it is the billing phone number.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&WalletData, &PaymentFlowData)> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(value: (&WalletData, &PaymentFlowData)) -> Result<Self, Self::Error> {
        let (wallet_data, item) = value;
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
                            brand: APPLE_PAY_BRAND.to_string(),
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

            WalletData::AliPayRedirect(_) => Ok(Self::AliPay),
            WalletData::AliPayHkRedirect(_) => Ok(Self::AliPayHk),
            WalletData::DanaRedirect { .. } => Ok(Self::Dana),
            WalletData::GcashRedirect(_) => Ok(Self::Gcash(Box::new(GcashData {}))),
            WalletData::GoPayRedirect(_) => Ok(Self::GoPay(Box::new(GoPayData {}))),
            WalletData::KakaoPayRedirect(_) => Ok(Self::Kakaopay(Box::new(KakaoPayData {}))),
            WalletData::MbWayRedirect(_) => Ok(Self::Mbway(Box::new(MbwayData {
                telephone_number: item.get_billing_phone_number()?,
            }))),
            WalletData::MobilePayRedirect(_) => Ok(Self::MobilePay),
            WalletData::MomoRedirect(_) => Ok(Self::Momo(Box::new(MomoData {}))),
            WalletData::TouchNGoRedirect(_) => Ok(Self::TouchNGo(Box::new(TouchNGoData {}))),
            WalletData::WeChatPayRedirect(_) => Ok(Self::WeChatPayWeb),
            WalletData::TwintRedirect { .. } => Ok(Self::Twint),
            WalletData::VippsRedirect { .. } => Ok(Self::Vipps),
            WalletData::SwishQr(_) => Ok(Self::Swish),
            WalletData::PaypalRedirect(_) => Ok(Self::AdyenPaypal),
            WalletData::AmazonPayRedirect(_)
            | WalletData::Paze(_)
            | WalletData::RevolutPay(_)
            | WalletData::SamsungPay(_)
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
            | WalletData::Wero(_)
            | WalletData::LazyPayRedirect(_)
            | WalletData::PhonePeRedirect(_)
            | WalletData::BillDeskRedirect(_)
            | WalletData::CashfreeRedirect(_)
            | WalletData::PayURedirect(_)
            | WalletData::EaseBuzzRedirect(_)
            | WalletData::PaymayaRedirect(_)
            | WalletData::PayhereRedirect {}
            | WalletData::QwikcilverWalletDirect(_)
            | WalletData::GrabpayRedirect { .. }
            | WalletData::Skrill(_)
            | WalletData::Neteller(_) => Err(IntegrationError::NotImplemented(
                ("payment_method").into(),
                Default::default(),
            )
            .into()),
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
            | VoucherData::RedPagos => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
                Default::default(),
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
            | BankRedirectData::OpenBanking { .. }
            | BankRedirectData::Netbanking { .. } => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
                Default::default(),
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
                Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Adyen"),
                    Default::default(),
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
            CardRedirectData::CardRedirect {} => Err(IntegrationError::NotImplemented(
                ("payment_method").into(),
                Default::default(),
            )
            .into()),
            CardRedirectData::Webpay {} => Err(IntegrationError::NotImplemented(
                ("payment_method").into(),
                Default::default(),
            )
            .into()),
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
                    .transpose()?;
                let test_holder_name = testing_data.and_then(|test_data| test_data.holder_name);
                Ok(Self::BacsDirectDebit(Box::new(BacsDirectDebitData {
                    bank_account_number: account_number.clone(),
                    bank_location_id: sort_code.clone(),
                    holder_name: test_holder_name
                        .unwrap_or(item.resource_common_data.get_billing_full_name()?),
                })))
            }
            BankDebitData::BecsBankDebit { .. }
            | BankDebitData::SepaGuaranteedBankDebit { .. }
            | BankDebitData::EftBankDebit { .. } => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
                Default::default(),
            )
            .into()),
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
            PayLaterData::TamaraRedirect { .. } => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
                Default::default(),
            )
            .into()),
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

        let card_holder_name = get_adyen_holder_name(
            item.router_data.request.get_connector_testing_data(),
            card_data.card_holder_name.as_ref(),
            item.router_data
                .resource_common_data
                .get_optional_billing_full_name(),
        )?;

        let additional_data =
            get_additional_data(&item.router_data, item.connector.amount_converter)?;

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let (store, splits) = match item.router_data.request.split_payments.as_ref() {
            Some(SplitPaymentsDetails::AdyenSplitPayment(adyen_split_payment)) => {
                get_adyen_split_request(adyen_split_payment, item.router_data.request.currency)
            }
            _ => (adyen_metadata.store.clone(), None),
        };
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        let country_code =
            get_country_code(item.router_data.resource_common_data.get_optional_billing());

        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((card_data, card_holder_name))?,
        ));

        let mpi_data =
            if let Some(auth_data) = item.router_data.request.authentication_data.as_ref() {
                // G-Authorize-02: an external 3DS result is only sent as mpiData when its ECI
                // corroborates the transaction status (see is_three_ds_outcome_corroborated).
                if !is_three_ds_outcome_corroborated(
                    auth_data.trans_status.as_ref(),
                    auth_data.eci.as_deref(),
                ) {
                    return Err(IntegrationError::InvalidDataFormat {
                    field_name: "authentication_data.trans_status",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "send trans_status Y with ECI 05/02, A with 06/02/01 or I with 07/02, \
                                 or omit authentication_data"
                                .to_string(),
                        ),
                        doc_url: Some(ADYEN_MPI_DATA_DOC_URL.to_string()),
                        additional_context: Some(format!(
                            "external 3DS outcome is not corroborated: trans_status={:?}, eci={:?}",
                            auth_data.trans_status, auth_data.eci
                        )),
                    },
                }
                .into());
                }
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
            AdyenPaymentMethod::try_from((wallet_data, &item.router_data.resource_common_data))?,
        ));
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model(&item.router_data)?;
        let return_url = item.router_data.request.get_router_return_url()?;
        let additional_data =
            get_additional_data(&item.router_data, item.connector.amount_converter)?;

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();
        // Platform merchants settle wallet payments through the same store/splits as cards, so
        // resolve them exactly like the Card arm does. Leaving them out makes Adyen reject the
        // payment with 905_1/905_2 ("could not find an acquirer account ...").
        let (store, splits) = match item.router_data.request.split_payments.as_ref() {
            Some(SplitPaymentsDetails::AdyenSplitPayment(adyen_split_payment)) => {
                get_adyen_split_request(adyen_split_payment, item.router_data.request.currency)
            }
            _ => (adyen_metadata.store.clone(), None),
        };
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
            shopper_name: None,
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
            country_code: None,
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
        let additional_data =
            get_additional_data(&item.router_data, item.connector.amount_converter)?;
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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

        let additional_data =
            get_additional_data(&item.router_data, item.connector.amount_converter)?;

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
            | BankDebitData::BecsBankDebit { .. }
            | BankDebitData::EftBankDebit { .. } => billing_address,
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
                        let now = common_utils::date_time::now().assume_utc();
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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

        let (store, splits) = match item.router_data.request.split_payments.as_ref() {
            Some(SplitPaymentsDetails::AdyenSplitPayment(adyen_split_payment)) => {
                get_adyen_split_request(adyen_split_payment, item.router_data.request.currency)
            }
            _ => (adyen_metadata.store.clone(), None),
        };
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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

        let additional_data =
            get_additional_data(&item.router_data, item.connector.amount_converter)?;

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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
        let additional_data =
            get_additional_data(&item.router_data, item.connector.amount_converter)?;
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

        let (store, splits) = match item.router_data.request.split_payments.as_ref() {
            Some(SplitPaymentsDetails::AdyenSplitPayment(adyen_split_payment)) => {
                get_adyen_split_request(adyen_split_payment, item.router_data.request.currency)
            }
            _ => (adyen_metadata.store.clone(), None),
        };

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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
        let (store, splits) = match item.router_data.request.split_payments.as_ref() {
            Some(SplitPaymentsDetails::AdyenSplitPayment(adyen_split_payment)) => {
                get_adyen_split_request(adyen_split_payment, item.router_data.request.currency)
            }
            _ => (adyen_metadata.store.clone(), None),
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
            additional_data: get_additional_data(
                &item.router_data,
                item.connector.amount_converter,
            )?,
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
        // G-Authorize-01 (every arm): refuse a capture method Adyen cannot honour before any
        // request is built.
        validate_adyen_capture_method(item.router_data.request.capture_method)?;
        // G-Authorize-03: arms that cannot carry an external 3DS result as mpiData.
        validate_external_three_ds_arm(
            &item.router_data.request.payment_method_data,
            item.router_data.request.authentication_data.is_some(),
        )?;
        match item
            .router_data
            .request
            .mandate_id
            .to_owned()
            .and_then(|mandate_ids| mandate_ids.mandate_reference_id)
        {
            Some(_mandate_ref) => Err(IntegrationError::NotImplemented(
                ("payment_method").into(),
                Default::default(),
            )
            .into()),
            None => match item.router_data.request.payment_method_data.clone() {
                PaymentMethodData::Card(ref card) => Self::try_from((item, card)),
                PaymentMethodData::CardRedirect(ref card_redirect_data) => {
                    Self::try_from((item, card_redirect_data))
                }
                PaymentMethodData::Wallet(ref wallet_data) => Self::try_from((item, wallet_data)),
                PaymentMethodData::BankRedirect(ref bank_redirect) => {
                    Self::try_from((item, bank_redirect))
                }
                PaymentMethodData::BankTransfer(ref bank_transfer) => {
                    Self::try_from((item, bank_transfer.as_ref()))
                }
                PaymentMethodData::BankDebit(ref bank_debit) => Self::try_from((item, bank_debit)),
                PaymentMethodData::GiftCard(ref gift_card) => {
                    Self::try_from((item, gift_card.as_ref()))
                }
                PaymentMethodData::Voucher(ref voucher_data) => {
                    Self::try_from((item, voucher_data))
                }
                PaymentMethodData::PayLater(ref pay_later_data) => {
                    Self::try_from((item, pay_later_data))
                }
                PaymentMethodData::NetworkToken(ref token_data) => {
                    Self::try_from((item, token_data))
                }
                PaymentMethodData::PaymentMethodToken(token_data) => {
                    let token = token_data.token.clone();

                    let amount = get_amount_data(&item);
                    let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
                    let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
                    let (recurring_processing_model, store_payment_method, shopper_reference) =
                        get_recurring_processing_model(&item.router_data)?;
                    let return_url = item.router_data.request.get_router_return_url()?;
                    let billing_address = get_address_info(
                        item.router_data.resource_common_data.get_optional_billing(),
                    )
                    .and_then(Result::ok);
                    let additional_data =
                        get_additional_data(&item.router_data, item.connector.amount_converter)?;
                    let adyen_metadata = get_adyen_metadata(
                        item.router_data.request.metadata.clone().expose_option(),
                    );
                    let store = adyen_metadata.store.clone();
                    let device_fingerprint = adyen_metadata.device_fingerprint.clone();
                    let platform_chargeback_logic =
                        adyen_metadata.platform_chargeback_logic.clone();
                    let country_code = get_country_code(
                        item.router_data.resource_common_data.get_optional_billing(),
                    );

                    let payment_method =
                        PaymentMethod::AdyenMandatePaymentMethod(Box::new(AdyenMandate {
                            payment_type: PaymentType::Scheme,
                            stored_payment_method_id: token,
                            holder_name: None,
                        }));

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
                        merchant_order_reference: item
                            .router_data
                            .request
                            .merchant_order_id
                            .clone(),
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
                        application_info: get_application_info(
                            item.router_data
                                .request
                                .partner_merchant_identifier_details
                                .as_ref(),
                        ),
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
                | PaymentMethodData::CardWithNoCvc(_)
                | PaymentMethodData::MobilePayment(_) => Err(IntegrationError::NotImplemented(
                    ("payment method").into(),
                    Default::default(),
                )
                .into()),
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
        // encoded_data is the form-urlencoded return query; each value is decoded once (as the
        // former serde_urlencoded parse did). G-PSync-01: a lone MD or PaRes is refused here,
        // before the /payments/details call.
        let redirect_return = get_adyen_redirect_return(
            |key| {
                url::form_urlencoded::parse(encoded_data.as_bytes())
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value.into_owned())
            },
            true,
            AdyenMdPaResFieldNames {
                md: "encoded_data.MD",
                pa_res: "encoded_data.PaRes",
            },
        )?
        .ok_or_else(|| IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "pass the shopper return query carrying redirectResult, threeDSResult, payload, or MD and PaRes"
                        .to_string(),
                ),
                doc_url: Some(ADYEN_PAYMENTS_DETAILS_DOC_URL.to_string()),
                additional_context: Some(
                    "encoded_data carries no /payments/details return shape".to_string(),
                ),
            },
        })?;

        let details = match redirect_return {
            AdyenRedirectReturn::RedirectResult(redirect_result) => {
                AdyenRedirectRequestTypes::AdyenRedirection(AdyenRedirection {
                    redirect_result,
                    type_of_redirection_result: None,
                    result_code: None,
                })
            }
            AdyenRedirectReturn::ThreeDsResult(three_ds_result) => {
                AdyenRedirectRequestTypes::AdyenThreeDS(AdyenThreeDS {
                    three_ds_result,
                    type_of_redirection_result: None,
                    result_code: None,
                })
            }
            AdyenRedirectReturn::Payload(payload) => {
                AdyenRedirectRequestTypes::AdyenRefusal(AdyenRefusal {
                    payload,
                    type_of_redirection_result: None,
                    result_code: None,
                })
            }
            AdyenRedirectReturn::MdPaRes { md, pa_res } => {
                AdyenRedirectRequestTypes::AdyenMdPaRes(AdyenMdPaRes { md, pa_res })
            }
        };
        Ok(Self { details })
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
    // Must stay before `RedirectionResponse`: `AdyenRedirectAction`'s url / method / data are
    // all optional, so a native `threeDS2` action body parses cleanly as a redirect, yields
    // `redirection_data: None`, and hangs the payment with no next action and no error. The
    // required `subtype` + `token` on `AdyenThreeDs2Action` keep this variant from matching
    // anything but a real native action.
    ThreeDs2Action(Box<AdyenThreeDs2ActionResponse>),
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
    splits: Option<Vec<AdyenSplitData>>,
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
    splits: Option<Vec<AdyenSplitData>>,
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
    /// Native 3DS2 fingerprint / challenge action.
    #[serde(rename = "threeDS2")]
    ThreeDs2,
    #[serde(other)]
    Unknown,
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
    splits: Option<Vec<AdyenSplitData>>,
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
    splits: Option<Vec<AdyenSplitData>>,
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
    #[serde(other)]
    Unknown,
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
    IdentifyShopper,
    PartiallyAuthorised,
    Pending,
    Received,
    RedirectShopper,
    Refused,
    PresentToShopper,
    Success,
    #[serde(other)]
    Unknown,
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
    type Error = error_stack::Report<ConnectorError>;
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
                    ConnectorError::response_handling_failed_http_status_unknown()
                ))
            }
            AdyenWebhookStatus::Unknown => Ok(Self::Unspecified),
        }
    }
}

const ADYEN_FRAUD_CANCELLED_CODE: &str = "22";
const ADYEN_FRAUD_CANCELLED_REASON: &str = "FRAUD-CANCELLED";

fn get_adyen_payment_status(
    is_manual_capture: bool,
    adyen_status: AdyenStatus,
    pmt: Option<common_enums::PaymentMethodType>,
    refusal_reason_code: Option<&str>,
    refusal_reason: Option<&str>,
) -> AttemptStatus {
    match adyen_status {
        AdyenStatus::AuthenticationFinished => AttemptStatus::AuthenticationSuccessful,
        AdyenStatus::AuthenticationNotRequired | AdyenStatus::Received => AttemptStatus::Pending,
        AdyenStatus::Authorised => match is_manual_capture {
            true => AttemptStatus::Authorized,
            // In case of Automatic capture Authorized is the final status of the payment
            false => AttemptStatus::Charged,
        },
        AdyenStatus::Cancelled => match (refusal_reason_code, refusal_reason) {
            (Some(ADYEN_FRAUD_CANCELLED_CODE), _) | (_, Some(ADYEN_FRAUD_CANCELLED_REASON)) => {
                AttemptStatus::Failure
            }
            _ => AttemptStatus::Voided,
        },
        // https://docs.adyen.com/online-payments/build-your-integration/payment-result-codes:
        // the shopper still has to act (3DS2 fingerprint / challenge, redirect, voucher).
        AdyenStatus::ChallengeShopper
        | AdyenStatus::IdentifyShopper
        | AdyenStatus::RedirectShopper
        | AdyenStatus::PresentToShopper => AttemptStatus::AuthenticationPending,
        // Partial authorisation (CheckoutService-v71 resultCode enum): only part of the amount
        // was authorised; captured immediately on automatic capture.
        AdyenStatus::PartiallyAuthorised => match is_manual_capture {
            true => AttemptStatus::PartiallyAuthorized,
            false => AttemptStatus::PartialCharged,
        },
        // `Success` is listed in the CheckoutService-v71 resultCode enum without documented
        // semantics, so it is treated as non-terminal until a webhook settles the payment.
        AdyenStatus::Success => AttemptStatus::Pending,
        AdyenStatus::Error | AdyenStatus::Refused => AttemptStatus::Failure,
        // Pix returns Pending status but requires customer action (QR code)
        // so we map it to AuthenticationPending like Hyperswitch does
        AdyenStatus::Pending => match pmt {
            Some(common_enums::PaymentMethodType::Pix) => AttemptStatus::AuthenticationPending,
            _ => AttemptStatus::Pending,
        },
        // Unknown means Adyen returned a status value not in our enum; signal hyperswitch core
        // to retain the previous attempt status rather than corrupting DB state.
        AdyenStatus::Unknown => AttemptStatus::Unspecified,
    }
}

// Unified ForeignTryFrom for Authorize and Psync Responses
impl<F, Req, T>
    ForeignTryFrom<(
        ResponseRouterData<AdyenPaymentResponse, AdyenRouterData<Self, T>>,
        Option<common_enums::CaptureMethod>,
        bool, // is_multiple_capture_psync_flow
        Option<common_enums::PaymentMethodType>,
    )> for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
where
    F: Clone,
    Req: Clone,
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;

    fn foreign_try_from(
        (value, capture_method, is_multiple_capture_psync_flow, payment_method_type): (
            ResponseRouterData<AdyenPaymentResponse, AdyenRouterData<Self, T>>,
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
        let connector = router_data.connector;
        let router_data = router_data.router_data;
        let is_manual_capture = is_manual_capture(capture_method);
        let pmt = payment_method_type;

        let adyen_payments_response_data = match response {
            AdyenPaymentResponse::ThreeDs2Action(response) => {
                get_three_ds2_action_response(*response, http_code)?
            }
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

        let amount_captured = match adyen_payments_response_data.status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => adyen_payments_response_data.txn_amount,
            _ => None,
        }
        .map(|amount| {
            let currency = router_data
                .resource_common_data
                .amount
                .as_ref()
                .map(|money| money.currency)
                .unwrap_or_default();

            utils::response_amount_to_money(
                &connector,
                amount,
                currency,
                http_code,
                connector_types::ConnectorEnum::Adyen,
            )
        })
        .transpose()?;

        Ok(Self {
            response: adyen_payments_response_data.error.map_or_else(
                || Ok(adyen_payments_response_data.payments_response_data),
                Err,
            ),
            resource_common_data: PaymentFlowData {
                status: adyen_payments_response_data.status,
                amount_captured,
                connector_response: adyen_payments_response_data.connector_response,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AdyenPaymentResponse, AdyenRouterData<Self, T>>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    F: Clone,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AdyenPaymentResponse, AdyenRouterData<Self, T>>,
    ) -> Result<Self, Self::Error> {
        let capture_method = value.router_data.router_data.request.capture_method;
        let payment_method_type = value.router_data.router_data.request.payment_method_type;
        Self::foreign_try_from((
            value,
            capture_method,
            false, // is_multiple_capture_psync_flow = false for authorize
            payment_method_type,
        ))
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AdyenPSyncResponse, AdyenRouterData<Self, T>>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
where
    F: Clone,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AdyenPSyncResponse, AdyenRouterData<Self, T>>,
    ) -> Result<Self, Self::Error> {
        // Extract the inner AdyenPaymentResponse from AdyenPSyncResponse
        let adyen_payment_response = value.response.0;

        // Check if this is a multiple capture sync flow
        let is_multiple_capture_psync_flow = match value.router_data.router_data.request.sync_type {
            SyncRequestType::MultipleCaptureSync => true,
            SyncRequestType::SinglePaymentSync => false,
        };

        let capture_method = value.router_data.router_data.request.capture_method;
        let payment_method_type = value.router_data.router_data.request.payment_method_type;

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
    TryFrom<ResponseRouterData<AdyenRepeatPaymentResponse, AdyenRouterData<Self, T>>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AdyenRepeatPaymentResponse, AdyenRouterData<Self, T>>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: repeat_response,
            router_data,
            http_code,
        } = value;
        let connector = router_data.connector;
        let router_data = router_data.router_data;
        // Unwrap the response wrapper to get AdyenPaymentResponse
        let response = repeat_response.0;
        let is_manual_capture = is_manual_capture(router_data.request.capture_method);
        let pmt = router_data.request.payment_method_type;

        // Process response using existing helper functions (same as Authorize)
        let adyen_payments_response_data = match response {
            AdyenPaymentResponse::ThreeDs2Action(response) => {
                get_three_ds2_action_response(*response, http_code)?
            }
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

        let amount_captured = match adyen_payments_response_data.status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => adyen_payments_response_data.txn_amount,
            _ => None,
        }
        .map(|amount| {
            utils::response_amount_to_money(
                &connector,
                amount,
                router_data.request.currency,
                http_code,
                connector_types::ConnectorEnum::Adyen,
            )
        })
        .transpose()?;

        Ok(Self {
            response: adyen_payments_response_data.error.map_or_else(
                || Ok(adyen_payments_response_data.payments_response_data),
                Err,
            ),
            resource_common_data: PaymentFlowData {
                status: adyen_payments_response_data.status,
                amount_captured,
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

impl TryFrom<ResponseRouterData<AdyenVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
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
            network_txn_link_id: None,
            connector_response_reference_id: Some(response.reference),
            incremental_authorization_allowed: None,
            mandate_reference: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
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

// ===== VoidPC (Post-Capture Reversal) Types =====

/// Request for Adyen post-capture reversal via /reversals endpoint.
/// Adyen's /reversals API cancels a captured payment before settlement.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenReversalRequest {
    merchant_account: Secret<String>,
    reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenReversalRequest
{
    type Error = Error;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            merchant_account: auth_type.merchant_account,
            reference: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

/// Response from Adyen /reversals endpoint.
/// Status is always "received"; outcome delivered via CANCEL_OR_REFUND webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenReversalResponse {
    pub psp_reference: String,
    pub payment_psp_reference: String,
    pub merchant_account: Secret<String>,
    pub reference: Option<String>,
    pub status: AdyenReversalStatus,
    pub refusal_reason: Option<String>,
    pub refusal_reason_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AdyenReversalStatus {
    Received,
}

impl TryFrom<ResponseRouterData<AdyenReversalResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AdyenReversalResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        // Adyen always returns "received" — actual void outcome arrives via webhook.
        // Preserve the existing attempt status; do not override it here.
        let status = router_data.resource_common_data.status;

        let post_capture_void_status =
            if response.refusal_reason.is_some() || response.refusal_reason_code.is_some() {
                common_enums::PostCaptureVoidStatus::Failed
            } else {
                common_enums::PostCaptureVoidStatus::Pending
            };
        let description = post_capture_void_status
            .is_post_capture_void_failure()
            .then(|| response.refusal_reason.clone())
            .flatten();

        let payments_response_data = PaymentsResponseData::PostCaptureVoidResponse {
            post_capture_void_status,
            connector_reference_id: Some(response.payment_psp_reference),
            description,
            status_code: http_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ===== End VoidPC Types =====

/// `attempt_status` for a refusal that arrived on HTTP 200 (`resultCode` Refused / Error, or
/// Cancelled with FRAUD-CANCELLED): the payment failed, so it is stated rather than left for
/// the caller to guess. Any other status yields `None`.
fn get_adyen_refusal_attempt_status(status: AttemptStatus) -> Option<FlowStatus> {
    (status == AttemptStatus::Failure).then_some(FlowStatus::Payment(AttemptStatus::Failure))
}

/// Connector response for a payment: the AVS / CVC checks and the auth code. Card-shaped
/// payments carry them as `Card { payment_checks, auth_code }`; an Apple Pay / Google Pay
/// payment without checks keeps its wallet-shaped auth code (those variants have no
/// `payment_checks` field).
fn get_adyen_connector_response(
    additional_data: &AdditionalData,
    pmt: Option<common_enums::PaymentMethodType>,
) -> Option<ConnectorResponseData> {
    let payment_checks = additional_data.get_payment_checks();
    let auth_code = additional_data.auth_code.clone();
    match (payment_checks, auth_code, pmt) {
        (None, None, _) => None,
        (
            None,
            Some(auth_code),
            Some(
                pmt @ (common_enums::PaymentMethodType::ApplePay
                | common_enums::PaymentMethodType::GooglePay),
            ),
        ) => Some(ConnectorResponseData::with_auth_code(auth_code, pmt)),
        (payment_checks, auth_code, _) => {
            Some(ConnectorResponseData::with_additional_payment_method_data(
                AdditionalPaymentMethodConnectorResponse::Card {
                    authentication_data: None,
                    payment_checks,
                    card_network: None,
                    domestic_network: None,
                    auth_code,
                    // Adyen does not return card-metadata on this response; the fields exist
                    // for connectors that do.
                    processor_card_network: None,
                    card_subtype: None,
                    card_segment_type: None,
                    funding_source: None,
                    card_type: None,
                    issuer_name: None,
                    issuer_country: None,
                },
            ))
        }
    }
}

pub fn get_adyen_response(
    response: AdyenResponse,
    is_capture_manual: bool,
    status_code: u16,
    pmt: Option<common_enums::PaymentMethodType>,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorError> {
    let status = get_adyen_payment_status(
        is_capture_manual,
        response.result_code,
        pmt,
        response.refusal_reason_code.as_deref(),
        response.refusal_reason.as_deref(),
    );
    let error = if response.refusal_reason.is_some()
        || response.refusal_reason_code.is_some()
        || status == AttemptStatus::Failure
    {
        let (network_decline_code, network_error_message) = response
            .additional_data
            .as_ref()
            .map(|data| {
                // Issuer decline fields come only from the raw acquirer result
                // (refusalCodeRaw / refusalReasonRaw). merchantAdviceCode is a Mastercard
                // retry advice and feeds network_advice_code only.
                let (reason_code, reason_message) =
                    match data.refusal_reason_raw.as_deref().map(str::trim) {
                        Some(reason_raw) => match reason_raw.split_once(':') {
                            Some((code, msg)) => {
                                (Some(code.trim().to_string()), Some(msg.trim().to_string()))
                            }
                            None => (None, Some(reason_raw.to_string())),
                        },
                        None => (None, None),
                    };
                (
                    data.refusal_code_raw.clone().or(reason_code),
                    reason_message,
                )
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
            attempt_status: get_adyen_refusal_attempt_status(status),
            connector_transaction_id: Some(response.psp_reference.clone()),
            network_advice_code: response
                .additional_data
                .as_ref()
                .and_then(|data| data.extract_network_advice_code()),
            network_decline_code,
            network_error_message,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        None
    };
    let mandate_reference = response
        .additional_data
        .as_ref()
        .and_then(AdditionalData::get_stored_payment_method_id)
        .map(|mandate_id| MandateReference {
            connector_mandate_id: Some(mandate_id.expose()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: None,
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

    let network_txn_link_id = response
        .additional_data
        .as_ref()
        .and_then(|additional_data| additional_data.transaction_link_id.clone());

    let splits = match &response.splits {
        Some(split_items) => Some(construct_charge_response(response.store, split_items)),
        None => None,
    };

    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.psp_reference),
        redirection_data: None,
        connector_metadata: None,
        network_txn_id,
        network_txn_link_id,
        connector_response_reference_id: Some(response.merchant_reference),
        incremental_authorization_allowed: None,
        mandate_reference: mandate_reference.map(Box::new),
        status_code,
        splits,
        payment_account_reference: None,
    };

    let txn_amount = response.amount.map(|amount| amount.value);
    let connector_response = response
        .additional_data
        .as_ref()
        .and_then(|additional_data| get_adyen_connector_response(additional_data, pmt));

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

/// `paymentMethod.holderName` for every card-carrying request, in Adyen's own precedence: the
/// connector testing data (sandbox test triggers such as `DECLINED`), else the card's own
/// cardholder name, else the billing full name. `None` only when all three are absent.
/// https://docs.adyen.com/development-resources/testing/result-codes
pub(crate) fn get_adyen_holder_name(
    connector_testing_data: Option<SecretSerdeValue>,
    card_holder_name: Option<&Secret<String>>,
    billing_full_name: Option<Secret<String>>,
) -> Result<Option<Secret<String>>, Error> {
    let test_holder_name = connector_testing_data
        .map(AdyenTestingData::try_from)
        .transpose()?
        .and_then(|test_data| test_data.holder_name);
    Ok(test_holder_name
        .or_else(|| card_holder_name.cloned())
        .or(billing_full_name))
}

/// Whether an external 3DS result may be sent to Adyen as authenticated `mpiData`: the ECI must
/// corroborate the transaction status, for every status accepted. Per Adyen's MPI data
/// combinations (https://docs.adyen.com/online-payments/3d-secure/authorize-mpidata):
/// - `Y` (authenticated): ECI `05` (Visa/Amex/JCB/CUP) or `02` (Mastercard/BCMC)
/// - `A` (attempt): ECI `06` (Visa/Amex/JCB/CUP), `02` (Adyen table) or `01` (Mastercard
///   attempt; Adyen authorises it with liability shift)
/// - `I` (3DS2 exemption): ECI `07` or `02`
///
/// Every other status (N, U, R, C, D, S) and any accepted status with a missing or different
/// ECI is not an authentication outcome Adyen can authorise against.
pub(crate) fn is_three_ds_outcome_corroborated(
    trans_status: Option<&common_enums::TransactionStatus>,
    eci: Option<&str>,
) -> bool {
    use common_enums::TransactionStatus;
    let accepted_eci: &[&str] = match trans_status {
        Some(TransactionStatus::Success) => &["05", "02"],
        Some(TransactionStatus::NotVerified) => &["06", "02", "01"],
        Some(TransactionStatus::InformationOnly) => &["07", "02"],
        Some(
            TransactionStatus::Failure
            | TransactionStatus::VerificationNotPerformed
            | TransactionStatus::Rejected
            | TransactionStatus::ChallengeRequired
            | TransactionStatus::ChallengeRequiredDecoupledAuthentication
            | TransactionStatus::SecurePaymentConfirmationRequired,
        )
        | None => &[],
    };
    eci.map(str::trim)
        .is_some_and(|eci| accepted_eci.contains(&eci))
}

const ADYEN_MPI_DATA_DOC_URL: &str =
    "https://docs.adyen.com/online-payments/3d-secure/authorize-mpidata";
const ADYEN_CAPTURE_DOC_URL: &str = "https://docs.adyen.com/online-payments/capture";
const ADYEN_WALLET_DECRYPTION_DOC_URL: &str =
    "https://docs.adyen.com/payment-methods/apple-pay/api-only/apple-pay-token-decryption";

/// A decrypted Apple Pay / Google Pay token without `token_source`: Adyen needs
/// `paymentMethod.brand` `applepay` or `googlepay`, which cannot be derived otherwise.
fn missing_wallet_token_source_error() -> Error {
    IntegrationError::MissingRequiredField {
        field_name: "token_source",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "send token_source APPLE_PAY or GOOGLE_PAY with the decrypted wallet token"
                    .to_string(),
            ),
            doc_url: Some(ADYEN_WALLET_DECRYPTION_DOC_URL.to_string()),
            additional_context: Some(
                "adyen requires paymentMethod.brand applepay or googlepay for a decrypted wallet"
                    .to_string(),
            ),
        },
    }
    .into()
}

/// Adyen captures automatically (optionally delayed by `captureDelayHours`) or manually via
/// `/captures`; it has no scheduled capture, so `Scheduled` is refused before any request is
/// built instead of silently falling into automatic capture.
/// External 3DS (`authentication_data`) is only mapped to `mpiData` on the raw card arm. Apple
/// Pay / Google Pay (PSP-encrypted or decrypted), network tokens and stored-token payments carry
/// their own cryptogram or none, so an external 3DS result sent with them is refused rather than
/// silently dropped. Every other arm is unaffected.
fn validate_external_three_ds_arm<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
    has_authentication_data: bool,
) -> Result<(), Error> {
    let cannot_carry_mpi_data = match payment_method_data {
        // ApplePay + ApplePayDecrypted, GooglePay + GooglePayDecrypted
        PaymentMethodData::Wallet(WalletData::ApplePay(_) | WalletData::GooglePay(_))
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::PaymentMethodToken(_) => true,
        _ => false,
    };
    if has_authentication_data && cannot_carry_mpi_data {
        return Err(IntegrationError::NotSupported {
            message: "external 3DS authentication data for this payment method".to_string(),
            connector: "adyen",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "omit authentication_data; external 3DS is supported for raw cards only"
                        .to_string(),
                ),
                doc_url: Some(ADYEN_MPI_DATA_DOC_URL.to_string()),
                additional_context: None,
            },
        }
        .into());
    }
    Ok(())
}

pub(crate) fn validate_adyen_capture_method(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<(), Error> {
    match capture_method {
        Some(common_enums::CaptureMethod::Scheduled) => {
            Err(IntegrationError::CaptureMethodNotSupported {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "use AUTOMATIC or MANUAL capture; Adyen has no scheduled capture"
                            .to_string(),
                    ),
                    doc_url: Some(ADYEN_CAPTURE_DOC_URL.to_string()),
                    additional_context: Some(
                        "capture_method SCHEDULED is not supported by adyen".to_string(),
                    ),
                },
            }
            .into())
        }
        Some(
            common_enums::CaptureMethod::Automatic
            | common_enums::CaptureMethod::SequentialAutomatic
            | common_enums::CaptureMethod::Manual
            | common_enums::CaptureMethod::ManualMultiple,
        )
        | None => Ok(()),
    }
}

pub fn get_present_to_shopper_response(
    response: PresentToShopperResponse,
    is_manual_capture: bool,
    status_code: u16,
    pmt: Option<common_enums::PaymentMethodType>,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorError> {
    let status = get_adyen_payment_status(
        is_manual_capture,
        response.result_code.clone(),
        pmt,
        response.refusal_reason_code.as_deref(),
        response.refusal_reason.as_deref(),
    );
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
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        None
    };

    let connector_metadata = get_present_to_shopper_metadata(&response)?;

    let splits = match &response.splits {
        Some(split_items) => Some(construct_charge_response(
            response.store.clone(),
            split_items,
        )),
        None => None,
    };

    // We don't get connector transaction id for redirections in Adyen.
    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: match response.psp_reference.as_ref() {
            Some(psp) => ResponseId::ConnectorTransactionId(psp.to_string()),
            None => ResponseId::NoResponseId,
        },
        redirection_data: None,
        connector_metadata,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        mandate_reference: None,
        status_code,
        splits,
        payment_account_reference: None,
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
) -> CustomResult<AdyenPaymentsResponseData, ConnectorError> {
    let status = get_adyen_payment_status(
        is_manual_capture,
        response.result_code,
        pmt,
        response.refusal_reason_code.as_deref(),
        response.refusal_reason.as_deref(),
    );
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
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    };
    // We don't get connector transaction id for redirections in Adyen.
    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::NoResponseId,
        redirection_data: None,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        status_code,
        splits: None,
        payment_account_reference: None,
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
) -> CustomResult<AdyenPaymentsResponseData, ConnectorError> {
    let status = get_adyen_payment_status(
        is_manual_capture,
        response.result_code.clone(),
        pmt,
        response.refusal_reason_code.as_deref(),
        response.refusal_reason.as_deref(),
    );
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
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        None
    };

    // Generate QR metadata matching Hyperswitch implementation
    let connector_metadata = get_qr_metadata(&response)?;

    let splits = match &response.splits {
        Some(split_items) => Some(construct_charge_response(
            response.store.clone(),
            split_items,
        )),
        None => None,
    };

    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: match response.psp_reference.as_ref() {
            Some(psp) => ResponseId::ConnectorTransactionId(psp.to_string()),
            None => ResponseId::NoResponseId,
        },
        redirection_data: None,
        connector_metadata,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        mandate_reference: None,
        status_code,
        splits,
        payment_account_reference: None,
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
) -> CustomResult<Option<serde_json::Value>, ConnectorError> {
    // Generate QR code image from qr_code_data
    let image_data = QrImage::new_from_data(response.action.qr_code_data.clone())
        .change_context(ConnectorError::response_handling_failed_http_status_unknown())?;

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
        .change_context(ConnectorError::response_handling_failed_http_status_unknown())
}

pub fn get_webhook_response(
    response: AdyenWebhookResponse,
    is_manual_capture: bool,
    is_multiple_capture_psync_flow: bool,
    status_code: u16,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorError> {
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
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
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
                status_code,
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
                mandate_metadata: None,
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
            network_txn_link_id: None,
            connector_response_reference_id: Some(response.merchant_reference_id),
            incremental_authorization_allowed: None,
            status_code,
            splits: None,
            payment_account_reference: None,
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
) -> CustomResult<AdyenPaymentsResponseData, ConnectorError> {
    let status = get_adyen_payment_status(
        is_manual_capture,
        response.result_code.clone(),
        pmt,
        response.refusal_reason_code.as_deref(),
        response.refusal_reason.as_deref(),
    );
    let error = if response.refusal_reason.is_some()
        || response.refusal_reason_code.is_some()
        || status == AttemptStatus::Failure
    {
        let (network_decline_code, network_error_message) = response
            .additional_data
            .as_ref()
            .map(|data| {
                // Issuer decline fields come only from the raw acquirer result
                // (refusalCodeRaw / refusalReasonRaw). merchantAdviceCode is a Mastercard
                // retry advice and feeds network_advice_code only.
                let (reason_code, reason_message) =
                    match data.refusal_reason_raw.as_deref().map(str::trim) {
                        Some(reason_raw) => match reason_raw.split_once(':') {
                            Some((code, msg)) => {
                                (Some(code.trim().to_string()), Some(msg.trim().to_string()))
                            }
                            None => (None, Some(reason_raw.to_string())),
                        },
                        None => (None, None),
                    };
                (
                    data.refusal_code_raw.clone().or(reason_code),
                    reason_message,
                )
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
            attempt_status: get_adyen_refusal_attempt_status(status),
            connector_transaction_id: response.psp_reference.clone(),
            network_advice_code: response
                .additional_data
                .as_ref()
                .and_then(|data| data.extract_network_advice_code()),
            network_decline_code,
            network_error_message,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
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

    let splits = match &response.splits {
        Some(split_items) => Some(construct_charge_response(response.store, split_items)),
        None => None,
    };

    let payments_response_data = PaymentsResponseData::TransactionResponse {
        resource_id: match response.psp_reference.as_ref() {
            Some(psp) => ResponseId::ConnectorTransactionId(psp.to_string()),
            None => ResponseId::NoResponseId,
        },
        redirection_data: redirection_data.map(Box::new),
        mandate_reference: None,
        connector_metadata,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: response
            .merchant_reference
            .clone()
            .or(response.psp_reference),
        incremental_authorization_allowed: None,
        status_code,
        splits,
        payment_account_reference: None,
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
) -> CustomResult<Option<serde_json::Value>, ConnectorError> {
    match next_action.action.payment_method_type {
        PaymentType::Blik => {
            let current_time = common_utils::date_time::now()
                .assume_utc()
                .unix_timestamp_nanos();
            Ok(Some(serde_json::json!(WaitScreenData {
                display_from_timestamp: current_time,
                display_to_timestamp: Some(current_time + Duration::minutes(1).whole_nanoseconds())
            })))
        }
        PaymentType::Mbway => {
            let current_time = common_utils::date_time::now()
                .assume_utc()
                .unix_timestamp_nanos();
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
    /// Optional because adyen omits it on some gateway errors, and returns `title`
    /// instead on others. Hyperswitch models both as optional and falls back
    /// `message` -> `title` -> placeholder; requiring either here would fail the whole
    /// parse on bodies hyperswitch handles.
    pub message: Option<String>,
    pub title: Option<String>,
    pub error_type: Option<AdyenErrorType>,
    pub psp_reference: Option<String>,
}

/// `ServiceError.errorType` (https://docs.adyen.com/development-resources/response-handling).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdyenErrorType {
    Internal,
    Validation,
    Security,
    Configuration,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, strum::Display, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhookEventCode {
    Authorisation,
    AuthorisationAdjustment,
    Cancellation,
    /// Result of `/cancels` by merchant reference (spec "Event Types": Void).
    TechnicalCancel,
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
    RequestForInformation,
    NotificationOfFraud,
    InformationSupplied,
    PrearbitrationOpen,
    PrearbitrationAccepted,
    PrearbitrationDeclined,
    PrearbitrationIssuerWithdrawn,
    SchemeArbitration,
    SchemeArbitrationWon,
    SchemeArbitrationLost,
    DisputeDefensePeriodEnded,
    IssuerResponseTimeframeExpired,
    IssuerComments,
    OfferClosed,
    /// Authorisation expired (spec "Event Types": PSync).
    Expire,
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
    Responded,
    Expired,
    Unresponded,
    #[serde(other)]
    Unknown,
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
    /// Carried by failure events (REFUND_FAILED, CAPTURE_FAILED, refused AUTHORISATION);
    /// absent on success events, so both stay optional.
    pub refusal_reason: Option<String>,
    pub refusal_reason_code: Option<String>,
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
    /// RECURRING_CONTRACT: pspReference of the initial payment that created the contract.
    pub original_psp: Option<String>,
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
                mandate_metadata: None,
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
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<AttemptStatus, WebhookError> {
    // Status per spec "Webhook `eventCode` + `success`" and "Event Types".
    match code {
        // AUTHORISATION success: Authorised is final; it settles only under automatic capture.
        // Without an event context the capture mode is unknown, so Authorized (non-terminal
        // for money movement) is reported rather than Charged.
        WebhookEventCode::Authorisation => {
            if is_success_scenario(&is_success) {
                match capture_method {
                    Some(
                        common_enums::CaptureMethod::Automatic
                        | common_enums::CaptureMethod::SequentialAutomatic,
                    ) => Ok(AttemptStatus::Charged),
                    Some(
                        common_enums::CaptureMethod::Manual
                        | common_enums::CaptureMethod::ManualMultiple
                        | common_enums::CaptureMethod::Scheduled,
                    )
                    | None => Ok(AttemptStatus::Authorized),
                }
            } else {
                Ok(AttemptStatus::Failure)
            }
        }
        // RECURRING_CONTRACT: a stored-payment contract/token was (not) created. It is a mandate
        // event; `success` reports contract creation, never a payment status (spec
        // "RECURRING_CONTRACT — decision"), whatever the success value or capture method.
        WebhookEventCode::RecurringContract => Ok(AttemptStatus::Unspecified),
        // AUTHORISATION_ADJUSTMENT: result of an authorisation adjustment.
        WebhookEventCode::AuthorisationAdjustment => {
            if is_success_scenario(&is_success) {
                Ok(AttemptStatus::Authorized)
            } else {
                Ok(AttemptStatus::Failure)
            }
        }
        // CANCELLATION / TECHNICAL_CANCEL: "Voided / cancel failed (`reason`)".
        WebhookEventCode::Cancellation | WebhookEventCode::TechnicalCancel => {
            if is_success_scenario(&is_success) {
                Ok(AttemptStatus::Voided)
            } else {
                Ok(AttemptStatus::VoidFailed)
            }
        }
        // CAPTURE: "Capture sent to bank / failed (`reason`)".
        WebhookEventCode::Capture => {
            if is_success_scenario(&is_success) {
                Ok(AttemptStatus::Charged)
            } else {
                Ok(AttemptStatus::CaptureFailed)
            }
        }
        // CAPTURE_FAILED: "Capture rejected by scheme after success".
        WebhookEventCode::CaptureFailed => Ok(AttemptStatus::CaptureFailed),
        // OFFER_CLOSED: offer expired; EXPIRE: "Authorisation expired".
        WebhookEventCode::OfferClosed | WebhookEventCode::Expire => Ok(AttemptStatus::Expired),
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
    is_success: String,
    dispute_status: Option<DisputeStatus>,
) -> Result<EventType, WebhookError> {
    match code {
        // Adyen sends the same AUTHORISATION eventCode for both success and
        // failure, distinguished only by `success` -- mirrors
        // get_adyen_payment_webhook_event's Authorized/Failure split below.
        // Most Adyen integrations auto-capture, so a successful AUTHORISATION
        // is the final settlement signal, not just an intermediate auth step.
        WebhookEventCode::Authorisation => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentSuccess)
            } else {
                Ok(EventType::PaymentIntentFailure)
            }
        }
        // RECURRING_CONTRACT: contract/token created (success "true") or not; a mandate event
        // (spec "RECURRING_CONTRACT — decision").
        WebhookEventCode::RecurringContract => {
            if is_success_scenario(&is_success) {
                Ok(EventType::MandateActive)
            } else {
                Ok(EventType::MandateFailed)
            }
        }
        WebhookEventCode::AuthorisationAdjustment => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentExtendAuthorizationSuccess)
            } else {
                Ok(EventType::PaymentIntentExtendAuthorizationFailure)
            }
        }
        // CANCELLATION / TECHNICAL_CANCEL: "Voided / cancel failed" (spec "Webhook `eventCode`
        // + `success`").
        WebhookEventCode::Cancellation | WebhookEventCode::TechnicalCancel => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentCancelled)
            } else {
                Ok(EventType::PaymentIntentCancelFailure)
            }
        }
        WebhookEventCode::Capture => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentCaptureSuccess)
            } else {
                Ok(EventType::PaymentIntentCaptureFailure)
            }
        }
        WebhookEventCode::CaptureFailed => Ok(EventType::PaymentIntentCaptureFailure),
        // OFFER_CLOSED: offer expired; EXPIRE: "Authorisation expired" (spec "Event Types").
        WebhookEventCode::OfferClosed | WebhookEventCode::Expire => {
            Ok(EventType::PaymentIntentExpired)
        }
        WebhookEventCode::Refund | WebhookEventCode::CancelOrRefund => {
            if is_success_scenario(&is_success) {
                Ok(EventType::RefundSuccess)
            } else {
                Ok(EventType::RefundFailure)
            }
        }
        WebhookEventCode::RefundFailed => Ok(EventType::RefundFailure),
        WebhookEventCode::RefundReversed => Ok(EventType::RefundReview),
        WebhookEventCode::NotificationOfChargeback => Ok(EventType::DisputeOpened),
        WebhookEventCode::Chargeback => match dispute_status {
            Some(DisputeStatus::Won) => Ok(EventType::DisputeWon),
            Some(DisputeStatus::Lost) | None => Ok(EventType::DisputeLost),
            Some(DisputeStatus::Accepted) => Ok(EventType::DisputeAccepted),
            _ => Ok(EventType::DisputeOpened),
        },
        WebhookEventCode::RequestForInformation => match dispute_status {
            Some(DisputeStatus::Expired) => Ok(EventType::DisputeExpired),
            _ => Ok(EventType::DisputeOpened),
        },
        WebhookEventCode::InformationSupplied => match dispute_status {
            Some(DisputeStatus::Responded) => Ok(EventType::DisputeChallenged),
            _ => Ok(EventType::DisputeOpened),
        },
        WebhookEventCode::ChargebackReversed => match dispute_status {
            Some(DisputeStatus::Pending) => Ok(EventType::DisputeChallenged),
            _ => Ok(EventType::DisputeWon),
        },
        WebhookEventCode::PrearbitrationWon => Ok(EventType::DisputeWon),
        WebhookEventCode::SecondChargeback | WebhookEventCode::PrearbitrationLost => {
            Ok(EventType::DisputeLost)
        }
        WebhookEventCode::PrearbitrationOpen | WebhookEventCode::SchemeArbitration => {
            Ok(EventType::DisputeOpened)
        }
        WebhookEventCode::PrearbitrationAccepted => Ok(EventType::DisputeAccepted),
        WebhookEventCode::PrearbitrationDeclined => Ok(EventType::DisputeChallenged),
        WebhookEventCode::PrearbitrationIssuerWithdrawn
        | WebhookEventCode::SchemeArbitrationWon
        | WebhookEventCode::IssuerResponseTimeframeExpired => Ok(EventType::DisputeWon),
        WebhookEventCode::SchemeArbitrationLost => Ok(EventType::DisputeLost),
        WebhookEventCode::DisputeDefensePeriodEnded => match dispute_status {
            Some(DisputeStatus::Accepted) => Ok(EventType::DisputeAccepted),
            _ => Ok(EventType::DisputeLost),
        },
        WebhookEventCode::NotificationOfFraud | WebhookEventCode::IssuerComments => {
            Ok(EventType::IncomingWebhookEventUnspecified)
        }
        WebhookEventCode::Unknown => {
            tracing::warn!(
                "Received unknown Adyen webhook event code; acknowledging without processing"
            );
            Ok(EventType::IncomingWebhookEventUnspecified)
        }
    }
}

const ADYEN_WEBHOOK_STRUCTURE_DOC_URL: &str =
    "https://docs.adyen.com/development-resources/webhooks/webhook-types";

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
    let webhook: AdyenIncomingWebhook = body
        .parse_struct("AdyenIncomingWebhook")
        .change_context(IntegrationError::NotImplemented(
            "webhook body decoding failed".to_string(),
            IntegrationErrorContext {
                suggested_action: Some(
                    "send an Adyen Standard webhook JSON body (notificationItems[].NotificationRequestItem)"
                        .to_string(),
                ),
                doc_url: Some(ADYEN_WEBHOOK_STRUCTURE_DOC_URL.to_string()),
                additional_context: None,
            },
        ))?;

    // JSON/HTTP POST webhooks carry exactly one NotificationRequestItem (spec "Webhook Payload
    // Structure"); several items is a SOAP-style batch this endpoint does not accept, and
    // picking one of them would verify and process an arbitrary element.
    let item_count = webhook.notification_items.len();
    match <[AdyenItemObjectWH; 1]>::try_from(webhook.notification_items) {
        Ok([item_object]) => Ok(item_object.notification_request_item),
        Err(_) => Err(error_stack::report!(IntegrationError::NotImplemented(
            "webhook body decoding failed".to_string(),
            IntegrationErrorContext {
                suggested_action: Some(
                    "send exactly one notificationItems entry per webhook request".to_string(),
                ),
                doc_url: Some(ADYEN_WEBHOOK_STRUCTURE_DOC_URL.to_string()),
                additional_context: Some(format!(
                    "expected 1 NotificationRequestItem, received {item_count}"
                )),
            },
        ))),
    }
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
) -> Result<Option<AdditionalData>, Error> {
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

    let transaction_link_id = match &item.request.mandate_reference {
        MandateReferenceId::NetworkMandateId(ref_data) => ref_data.transaction_link_id.clone(),
        MandateReferenceId::NetworkTokenWithNTI(ref_data) => ref_data.transaction_link_id.clone(),
        MandateReferenceId::ConnectorMandateId(_) => None,
    };

    let capture_delay_hours =
        get_capture_delay_hours(&item.request.metadata, item.request.capture_method)?;

    Ok(Some(AdditionalData {
        authorisation_type,
        manual_capture,
        execute_three_d,
        capture_delay_hours,
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
        transaction_link_id,
        ..AdditionalData::default()
    }))
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
                // Mirror hyperswitch exactly: line1 is `houseNumberOrName`, line2 is `street`,
                // and `street` is omitted when there is no line2. A line1-only address is not
                // refused for the missing street - measured on the sandbox, a card payment with
                // line1 and no line2 is authorised.
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

/// Mirror hyperswitch: map request.partner_merchant_identifier_details to Adyen `applicationInfo`.
/// Shared by Authorize and repeat/MIT flows (hyperswitch reuses the Authorize builder for MIT).
fn get_application_info(
    partner_merchant_identifier_details: Option<&connector_types::PartnerMerchantIdentifierDetails>,
) -> Option<ApplicationInfo> {
    partner_merchant_identifier_details.map(|details| ApplicationInfo {
        merchant_application: details.merchant_details.as_ref().map(|merchant_details| {
            MerchantApplication {
                name: merchant_details.name.clone(),
                version: merchant_details.version.clone(),
            }
        }),
        external_platform: details.partner_details.as_ref().map(|platform_details| {
            ExternalPlatform {
                name: platform_details.name.clone(),
                version: platform_details.version.clone(),
                integrator: platform_details.integrator.clone(),
            }
        }),
    })
}

/// A decrypted Google Pay token reaches Adyen as `paymentMethod.type = scheme` with
/// `brand = googlepay` whether it carries a raw PAN or a network token, so Adyen can only tell
/// the two apart from `additionalData.paymentdatasource.*`. Send the pair when there is no
/// cryptogram (FPAN / PAN_ONLY); tokens that do carry one send `mpiData` instead, which is what
/// Adyen documents for DPAN and what the Hyperswitch Adyen connector branches on.
fn get_google_pay_payment_data_source<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
) -> Option<AdyenPaymentDataSource> {
    match payment_method_data {
        PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => match &google_pay_data
            .tokenization_data
        {
            GpayTokenizationData::Decrypted(decrypt_data) if decrypt_data.cryptogram.is_none() => {
                Some(AdyenPaymentDataSource {
                    data_type: GOOGLE_PAY_BRAND.to_string(),
                    tokenized: "false".to_string(),
                })
            }
            _ => None,
        },
        _ => None,
    }
}

/// Level 2/3 data is a card-scheme feature: only the arms that reach Adyen as a scheme payment
/// (raw card, network token, decrypted Apple Pay / Google Pay) carry it.
fn is_scheme_payment_method<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
) -> bool {
    match payment_method_data {
        PaymentMethodData::Card(_) | PaymentMethodData::NetworkToken(_) => true,
        PaymentMethodData::Wallet(WalletData::ApplePay(apple_pay_data)) => matches!(
            apple_pay_data.payment_data,
            ApplePayPaymentData::Decrypted(_)
        ),
        PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => matches!(
            google_pay_data.tokenization_data,
            GpayTokenizationData::Decrypted(_)
        ),
        _ => false,
    }
}

const ADYEN_L2_L3_DOC_URL: &str =
    "https://docs.adyen.com/payment-methods/cards/enhanced-scheme-data/l2-l3";

fn l2_l3_amount_overflow() -> IntegrationError {
    IntegrationError::InvalidDataFormat {
        field_name: "l2_l3_data.order_info.order_details",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "check order_details amount, quantity and unit_discount_amount".to_string(),
            ),
            doc_url: Some(ADYEN_L2_L3_DOC_URL.to_string()),
            additional_context: Some("order line amount overflows i64 minor units".to_string()),
        },
    }
}

/// Builds the legacy `additionalData.enhancedSchemeData.*` keys from `l2_l3_data`. `None` when
/// the request carries no L2/L3 data or the payment is not a scheme payment.
fn get_adyen_level_two_three_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    amount_converter: &dyn AmountConvertor<Output = StringMinorUnit>,
) -> Result<Option<AdyenLevelTwoThreeData>, Error> {
    if !is_scheme_payment_method(&item.request.payment_method_data) {
        return Ok(None);
    }
    let Some(l2_l3_data) = item.resource_common_data.l2_l3_data.as_deref() else {
        return Ok(None);
    };
    let order_info = l2_l3_data.order_info.as_ref();
    let currency = item.request.currency;

    // `enhancedSchemeData.totalTaxAmount` / `freightAmount` / `dutyAmount` are minor-unit
    // integers in the payment's own currency -- Adyen carries no per-field currency for them.
    // These three arrive as `Money`, so a caller can hand over an amount denominated in
    // something else; converting it with its own exponent and sending the bare number would
    // silently change the value (100 JPY would reach Adyen as 100 minor units of a
    // two-decimal payment currency, i.e. 1.00). A mismatch is refused rather than converted.
    let convert_money = |field_name: &'static str, money: &common_utils::types::Money| {
        if money.currency != currency {
            return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name,
                context: IntegrationErrorContext {
                    suggested_action: Some(format!(
                        "send this amount in the payment currency ({currency}); Adyen carries \
                         enhancedSchemeData amounts in the payment's currency only"
                    )),
                    doc_url: Some(ADYEN_L2_L3_DOC_URL.to_string()),
                    additional_context: Some(format!(
                        "{field_name} is denominated in {} but the payment is in {currency}",
                        money.currency
                    )),
                },
            }));
        }
        domain_utils::convert_amount(amount_converter, money.amount, money.currency)
    };

    // One itemDetailLine group per order line, numbered from 1 in order.
    let item_detail_lines: std::collections::BTreeMap<String, String> = order_info
        .and_then(|info| info.order_details.as_ref())
        .into_iter()
        .flatten()
        .map(|line| {
            let quantity = i64::from(line.quantity);
            // discountAmount is the line total discount: unit discount x quantity.
            let discount_amount = line
                .unit_discount_amount
                .map(|unit_discount| {
                    unit_discount
                        .get_amount_as_i64()
                        .checked_mul(quantity)
                        .ok_or_else(l2_l3_amount_overflow)
                })
                .transpose()?;
            // Visa: totalAmount = quantity x unitPrice - discountAmount.
            let total_amount = line
                .amount
                .get_amount_as_i64()
                .checked_mul(quantity)
                .and_then(|gross| gross.checked_sub(discount_amount.unwrap_or(0)))
                .ok_or_else(l2_l3_amount_overflow)?;
            Ok(AdyenItemDetailLine {
                description: Some(line.product_name.clone()),
                product_code: line.product_id.clone(),
                commodity_code: line.commodity_code.clone(),
                quantity: line.quantity,
                unit_of_measure: line.unit_of_measure.clone(),
                unit_price: domain_utils::convert_amount(amount_converter, line.amount, currency)?,
                discount_amount: discount_amount
                    .map(|discount| {
                        domain_utils::convert_amount(
                            amount_converter,
                            MinorUnit::new(discount),
                            currency,
                        )
                    })
                    .transpose()?,
                total_amount: domain_utils::convert_amount(
                    amount_converter,
                    MinorUnit::new(total_amount),
                    currency,
                )?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?
        .into_iter()
        .enumerate()
        .flat_map(|(index, line)| line.entries(index + 1))
        .collect();

    let order_date = order_info
        .and_then(|info| info.order_date)
        .map(|date| {
            date.format(time::macros::format_description!("[year]-[month]-[day]"))
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "l2_l3_data.order_info.order_date",
                    context: IntegrationErrorContext {
                        suggested_action: Some("send a valid order date".to_string()),
                        doc_url: Some(ADYEN_L2_L3_DOC_URL.to_string()),
                        additional_context: None,
                    },
                })
        })
        .transpose()?;

    Ok(Some(AdyenLevelTwoThreeData {
        customer_reference: order_info.and_then(|info| info.merchant_order_reference_id.clone()),
        total_tax_amount: l2_l3_data
            .tax_info
            .as_ref()
            .and_then(|tax| tax.order_tax_amount.as_ref())
            .map(|money| convert_money("l2_l3_data.tax_info.order_tax_amount", money))
            .transpose()?,
        freight_amount: order_info
            .and_then(|info| info.shipping_cost.as_ref())
            .map(|money| convert_money("l2_l3_data.order_info.shipping_cost", money))
            .transpose()?,
        duty_amount: order_info
            .and_then(|info| info.duty_amount.as_ref())
            .map(|money| convert_money("l2_l3_data.order_info.duty_amount", money))
            .transpose()?,
        destination_postal_code: l2_l3_data.get_shipping_zip(),
        destination_country_code: l2_l3_data.get_shipping_country(),
        destination_state_province_code: l2_l3_data.get_shipping_state(),
        order_date,
        item_detail_lines,
    }))
}

fn get_additional_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    amount_converter: &dyn AmountConvertor<Output = StringMinorUnit>,
) -> Result<Option<AdditionalData>, Error> {
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

    let capture_delay_hours =
        get_capture_delay_hours(&item.request.metadata, item.request.capture_method)?;

    let paymentdatasource = get_google_pay_payment_data_source(&item.request.payment_method_data);
    let level_two_three = get_adyen_level_two_three_data(item, amount_converter)?;

    Ok(Some(AdditionalData {
        level_two_three,
        authorisation_type,
        manual_capture,
        execute_three_d,
        capture_delay_hours,
        network_tx_reference: None,
        recurring_detail_reference: None,
        recurring_shopper_reference: None,
        recurring_processing_model: None,
        paymentdatasource,
        riskdata,
        sca_exemption: item.request.authentication_data.as_ref().and_then(|data| {
            data.exemption_indicator
                .as_ref()
                .and_then(to_adyen_exemption)
        }),
        ..AdditionalData::default()
    }))
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
        email_name: email_name.map(Secret::new),
        email_domain: email_domain.map(Secret::new),
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
    AdyenMdPaRes(AdyenMdPaRes),
}

/// 3DS1 redirect return: Adyen's `/payments/details` takes `details.MD` and `details.PaRes`
/// together and rejects either one alone with `14_0392` (techspec "Submit details for a payment").
#[derive(Debug, Clone, Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct AdyenMdPaRes {
    #[serde(rename = "MD")]
    pub md: Secret<String>,
    #[serde(rename = "PaRes")]
    pub pa_res: Secret<String>,
}

/// Key of the 3DS2 redirect result in a shopper return.
const ADYEN_THREE_DS_RESULT_KEY: &str = "threeDSResult";
/// Key of the redirect payload in a shopper return (PSync redirect completion only).
const ADYEN_PAYLOAD_KEY: &str = "payload";
/// Keys of the 3DS1 merchant data / payer authentication response pair.
const ADYEN_MD_KEY: &str = "MD";
const ADYEN_PA_RES_KEY: &str = "PaRes";
const ADYEN_PAYMENTS_DETAILS_DOC_URL: &str =
    "https://docs.adyen.com/api-explorer/Checkout/71/post/payments/details";

/// One documented `/payments/details` return shape, picked from a shopper return.
#[derive(Debug, Clone)]
enum AdyenRedirectReturn {
    RedirectResult(String),
    ThreeDsResult(String),
    Payload(String),
    MdPaRes {
        md: Secret<String>,
        pa_res: Secret<String>,
    },
}

/// Field names reported when only one member of the MD/PaRes pair is present; the caller names
/// them after the request field the return came from (e.g. `encoded_data.PaRes`).
#[derive(Debug, Clone, Copy)]
struct AdyenMdPaResFieldNames {
    md: &'static str,
    pa_res: &'static str,
}

fn adyen_incomplete_md_pa_res(field_name: &'static str) -> Error {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(
                "send both MD and PaRes from the 3DS1 return form; Adyen rejects either one alone"
                    .to_string(),
            ),
            doc_url: Some(ADYEN_PAYMENTS_DETAILS_DOC_URL.to_string()),
            additional_context: Some(
                "adyen /payments/details requires details.MD and details.PaRes together (14_0392)"
                    .to_string(),
            ),
        },
    }
    .into()
}

/// Shared redirect-return selector for the PSync and PostAuthenticate redirect completions.
///
/// `lookup` returns the (already decoded) value of a return key; blank values count as absent.
/// Precedence: `redirectResult` > `threeDSResult` > `payload` (only when `allow_payload`) >
/// the `MD` + `PaRes` pair, so exactly one shape is sent. Exactly one of `MD` / `PaRes` is refused
/// before any connector call (Adyen `14_0392`); no shape at all is `Ok(None)`. Values are passed
/// through as given.
fn get_adyen_redirect_return(
    lookup: impl Fn(&str) -> Option<String>,
    allow_payload: bool,
    field_names: AdyenMdPaResFieldNames,
) -> Result<Option<AdyenRedirectReturn>, Error> {
    let get = |key: &str| lookup(key).filter(|value| !value.trim().is_empty());
    if let Some(redirect_result) = get(ADYEN_REDIRECT_RESULT_KEY) {
        return Ok(Some(AdyenRedirectReturn::RedirectResult(redirect_result)));
    }
    if let Some(three_ds_result) = get(ADYEN_THREE_DS_RESULT_KEY) {
        return Ok(Some(AdyenRedirectReturn::ThreeDsResult(three_ds_result)));
    }
    if allow_payload {
        if let Some(payload) = get(ADYEN_PAYLOAD_KEY) {
            return Ok(Some(AdyenRedirectReturn::Payload(payload)));
        }
    }
    match (get(ADYEN_MD_KEY), get(ADYEN_PA_RES_KEY)) {
        (Some(md), Some(pa_res)) => Ok(Some(AdyenRedirectReturn::MdPaRes {
            md: Secret::new(md),
            pa_res: Secret::new(pa_res),
        })),
        (Some(_), None) => Err(adyen_incomplete_md_pa_res(field_names.pa_res)),
        (None, Some(_)) => Err(adyen_incomplete_md_pa_res(field_names.md)),
        (None, None) => Ok(None),
    }
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
    merchant_refund_reason: Option<AdyenRefundRequestReason>,
    reference: String,
    splits: Option<Vec<AdyenSplitData>>,
    store: Option<String>,
}

/// `merchantRefundReason` accepts only these five values; any other text is rejected with a 422
/// "Invalid merchant refund reason".
/// https://docs.adyen.com/api-explorer/Checkout/68/post/payments/-paymentPspReference-/refunds
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AdyenRefundRequestReason {
    #[serde(rename = "FRAUD")]
    Fraud,
    #[serde(rename = "CUSTOMER REQUEST")]
    CustomerRequest,
    #[serde(rename = "RETURN")]
    Return,
    #[serde(rename = "DUPLICATE")]
    Duplicate,
    #[serde(rename = "OTHER")]
    Other,
}

impl AdyenRefundRequestReason {
    /// Maps the caller's free-text refund reason onto Adyen's enum. The text is trimmed,
    /// uppercased and `_` becomes a space, so `customer_requested` and `CUSTOMER REQUEST` both
    /// map to `CustomerRequest`. Text that names none of the five values is sent as `OTHER`
    /// rather than forwarded verbatim (which Adyen would reject).
    pub fn from_reason(reason: &str) -> Self {
        match reason.trim().to_uppercase().replace('_', " ").as_str() {
            "FRAUD" => Self::Fraud,
            "CUSTOMER REQUEST" | "CUSTOMERREQUEST" | "CUSTOMER REQUESTED" => Self::CustomerRequest,
            "RETURN" => Self::Return,
            "DUPLICATE" => Self::Duplicate,
            _ => Self::Other,
        }
    }
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

        let (store, splits) = match item.router_data.request.split_refunds.as_ref() {
            Some(connector_types::SplitRefundsDetails::AdyenSplitRefund(adyen_split_data)) => {
                get_adyen_split_request(adyen_split_data, item.router_data.request.currency)
            }
            _ => (
                item.router_data
                    .request
                    .refund_connector_metadata
                    .clone()
                    .and_then(|metadata| get_store_id(metadata.expose())),
                None,
            ),
        };

        Ok(Self {
            merchant_account: auth_type.merchant_account,
            amount: Amount {
                currency: item.router_data.request.currency,
                value: item.router_data.request.refund_amount.amount,
            },
            merchant_refund_reason: item
                .router_data
                .request
                .reason
                .as_deref()
                .map(AdyenRefundRequestReason::from_reason),
            reference: item.router_data.request.refund_id.clone(),
            store,
            splits,
        })
    }
}

impl<F, Req> TryFrom<ResponseRouterData<AdyenRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
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
            acquirer_reference_number: None,
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
                value: item.router_data.request.amount_to_capture.amount.to_owned(),
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
    splits: Option<Vec<AdyenSplitData>>,
}

impl<F> TryFrom<ResponseRouterData<AdyenCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
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
            response.payment_psp_reference.clone()
        };

        let splits = match &response.splits {
            Some(split_items) => Some(construct_charge_response(
                response.store.clone(),
                split_items,
            )),
            None => None,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.reference),
                incremental_authorization_allowed: None,
                mandate_reference: None,
                status_code: http_code,
                splits,
                payment_account_reference: None,
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

        let card_holder_name = get_adyen_holder_name(
            item.router_data.request.get_connector_testing_data(),
            card_data.card_holder_name.as_ref(),
            item.router_data
                .resource_common_data
                .get_optional_billing_full_name(),
        )?;

        let additional_data = get_additional_data_for_setup_mandate(&item.router_data)?;

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
            application_info: None,
        }))
    }
}

// Wallets (Apple Pay / Google Pay decrypted tokens, plus redirect wallets like AliPayHk) can seed
// a mandate the same way a card can, so a zero-amount SetupMandate must accept them too -- mirrors
// how Authorize builds AdyenPaymentMethod/mpi_data for the same WalletData variants.
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
        &WalletData,
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
            &WalletData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, wallet_data) = value;
        let amount = get_amount_data_for_setup_mandate(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        // Prefer the already-known Adyen shopper reference (connector_customer) over deriving
        // one from customer_id, matching the Card arm above -- otherwise a Wallet and a Card
        // SetupMandate for the same customer would send Adyen two different shopperReference
        // values.
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

        let additional_data = get_additional_data_for_setup_mandate(&item.router_data)?;

        let adyen_metadata =
            get_adyen_metadata(item.router_data.request.metadata.clone().expose_option());
        let device_fingerprint = adyen_metadata.device_fingerprint.clone();
        let platform_chargeback_logic = adyen_metadata.platform_chargeback_logic.clone();

        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from((wallet_data, &item.router_data.resource_common_data))?,
        ));

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
            mpi_data,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            // Hyperswitch does not send shopperName for wallet setup mandates.
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
            application_info: None,
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
            Some(_mandate_ref) => Err(IntegrationError::NotImplemented(
                ("payment_method").into(),
                Default::default(),
            )
            .into()),
            None => match item.router_data.request.payment_method_data.clone() {
                PaymentMethodData::Card(ref card) => Self::try_from((item, card)),
                PaymentMethodData::Wallet(ref wallet_data) => Self::try_from((item, wallet_data)),
                PaymentMethodData::PayLater(_)
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
                | PaymentMethodData::CardWithNoCvc(_)
                | PaymentMethodData::MobilePayment(_)
                | PaymentMethodData::PaymentMethodToken(_) => Err(
                    IntegrationError::NotImplemented(("payment method").into(), Default::default())
                        .into(),
                ),
            },
        }
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<SetupMandateResponse, AdyenRouterData<Self, T>>>
    for RouterDataV2<F, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<SetupMandateResponse, AdyenRouterData<Self, T>>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let connector = router_data.connector;
        let router_data = router_data.router_data;
        let pmt = router_data.request.payment_method_type;
        let is_manual_capture = false;
        // Unwrap the response wrapper to get AdyenPaymentResponse
        let SetupMandateResponse(adyen_response) = response;

        // Process response using existing helper functions (same as Authorize/RepeatPayment)
        let adyen_payments_response_data = match adyen_response {
            AdyenPaymentResponse::ThreeDs2Action(response) => {
                get_three_ds2_action_response(*response, http_code)?
            }
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

        let amount_captured = match adyen_payments_response_data.status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => adyen_payments_response_data.txn_amount,
            _ => None,
        }
        .map(|amount| {
            utils::response_amount_to_money(
                &connector,
                amount,
                router_data.request.currency,
                http_code,
                connector_types::ConnectorEnum::Adyen,
            )
        })
        .transpose()?;

        Ok(Self {
            response: adyen_payments_response_data.error.map_or_else(
                || Ok(adyen_payments_response_data.payments_response_data),
                Err,
            ),
            resource_common_data: PaymentFlowData {
                status: adyen_payments_response_data.status,
                amount_captured,
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
        value: item
            .router_data
            .request
            .amount
            .as_ref()
            .map(|money| money.amount)
            .unwrap_or_else(MinorUnit::zero),
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
        shopper_interaction(item.request.off_session, &item.request.payment_channel)
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
) -> Result<Option<AdditionalData>, Error> {
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

    let capture_delay_hours =
        get_capture_delay_hours(&item.request.metadata, item.request.capture_method)?;

    let paymentdatasource = get_google_pay_payment_data_source(&item.request.payment_method_data);

    Ok(Some(AdditionalData {
        authorisation_type,
        manual_capture,
        execute_three_d,
        capture_delay_hours,
        network_tx_reference: None,
        recurring_detail_reference: None,
        recurring_shopper_reference: None,
        recurring_processing_model: None,
        paymentdatasource,
        riskdata,
        sca_exemption: None,
        ..AdditionalData::default()
    }))
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
        // Refused before any arm builds a request (G-RepeatPayment-01).
        validate_adyen_capture_method(item.router_data.request.capture_method)?;
        let mandate_ref_id = item.router_data.request.mandate_reference.clone();
        let amount = Amount {
            currency: item.router_data.request.currency,
            value: item.router_data.request.amount.amount,
        };
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        let shopper_interaction = AdyenShopperInteraction::ContinuedAuthentication;
        let (recurring_processing_model, store_payment_method, shopper_reference) =
            get_recurring_processing_model_for_repeat_payment(&item.router_data)?;
        let browser_info = None;
        let additional_data = get_additional_data_for_repeat_payment(&item.router_data)?;
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
                        Some(pm_type) => PaymentType::try_from(&pm_type)?,
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
                                &card_details_for_network_transaction_id.get_card_issuer()?,
                            )?,
                        };
                        let card_holder_name = get_adyen_holder_name(
                            item.router_data.request.get_connector_testing_data(),
                            card_details_for_network_transaction_id
                                .card_holder_name
                                .as_ref(),
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name(),
                        )?;
                        let raw_card_number = RawCardNumber(
                            card_details_for_network_transaction_id
                                .card_number
                                .try_card_number("Adyen")?,
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
                            holder_name: card_holder_name,
                            brand: Some(brand),
                            network_payment_reference: Some(Secret::new(
                                network_mandate_id.network_transaction_id.clone(),
                            )),
                        };
                        PaymentMethod::AdyenPaymentMethod(Box::new(AdyenPaymentMethod::AdyenCard(
                            Box::new(adyen_card),
                        )))
                    }
                    // Raw card + network transaction id of the initial CIT (card details held
                    // outside Adyen): `paymentMethod.networkPaymentReference`, no `cvc`,
                    // `shopperInteraction: ContAuth`.
                    // https://docs.adyen.com/online-payments/psd2-sca-compliance-and-implementation-guide/business-models/subsequent-payments
                    PaymentMethodData::Card(ref card) => {
                        let card_number = card.card_number.try_card_number("Adyen")?;
                        let brand = match card.card_network.clone().and_then(get_adyen_card_network)
                        {
                            Some(card_network) => card_network,
                            None => CardBrand::try_from(&domain_utils::get_card_issuer(
                                card_number.peek(),
                            )?)?,
                        };
                        let card_holder_name = get_adyen_holder_name(
                            item.router_data.request.get_connector_testing_data(),
                            card.card_holder_name.as_ref(),
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name(),
                        )?;
                        let adyen_card = AdyenCard {
                            number: RawCardNumber(card_number),
                            expiry_month: card.card_exp_month.clone(),
                            expiry_year: card.get_expiry_year_4_digit(),
                            cvc: None,
                            holder_name: card_holder_name,
                            brand: Some(brand),
                            network_payment_reference: Some(Secret::new(
                                network_mandate_id.network_transaction_id.clone(),
                            )),
                        };
                        PaymentMethod::AdyenPaymentMethod(Box::new(AdyenPaymentMethod::AdyenCard(
                            Box::new(adyen_card),
                        )))
                    }
                    // Merchant/Hyperswitch-decrypted Apple Pay / Google Pay MIT: the decrypted
                    // card fields with `brand` `applepay`/`googlepay`, the network transaction id,
                    // and no `mpiData` (the shopper is not present).
                    // https://docs.adyen.com/payment-methods/apple-pay/api-only/apple-pay-token-decryption
                    PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(
                        ref wallet_token,
                    ) => {
                        // G-RepeatPayment-02: `paymentMethod.brand` cannot be derived without it.
                        let brand = match wallet_token.token_source {
                            Some(TokenSource::ApplePay) => APPLE_PAY_BRAND,
                            Some(TokenSource::GooglePay) => GOOGLE_PAY_BRAND,
                            None => return Err(missing_wallet_token_source_error()),
                        };
                        let wallet_mit_data = AdyenDecryptedWalletMitData {
                            number: wallet_token.decrypted_token.clone(),
                            expiry_month: wallet_token.token_exp_month.clone(),
                            expiry_year: wallet_token.get_expiry_year_4_digit(),
                            holder_name: wallet_token.card_holder_name.clone(),
                            brand: brand.to_string(),
                            network_payment_reference: Some(Secret::new(
                                network_mandate_id.network_transaction_id.clone(),
                            )),
                        };
                        PaymentMethod::AdyenPaymentMethod(Box::new(
                            AdyenPaymentMethod::DecryptedWalletMit(Box::new(wallet_mit_data)),
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
            MandateReferenceId::NetworkTokenWithNTI(network_mandate_id) => {
                match &item.router_data.request.payment_method_data {
                    PaymentMethodData::NetworkToken(ref token_data) => {
                        let card_issuer = token_data.get_card_issuer()?;
                        let brand = CardBrand::try_from(&card_issuer)?;
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

        let (store, splits) = match item.router_data.request.split_payments.as_ref() {
            Some(SplitPaymentsDetails::AdyenSplitPayment(adyen_split_payment)) => {
                get_adyen_split_request(adyen_split_payment, item.router_data.request.currency)
            }
            _ => (adyen_metadata.store.clone(), None),
        };
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
            application_info: get_application_info(
                item.router_data
                    .request
                    .partner_merchant_identifier_details
                    .as_ref(),
            ),
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
    type Error = error_stack::Report<ConnectorError>;

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
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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
    type Error = error_stack::Report<ConnectorError>;

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
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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
    type Error = error_stack::Report<ConnectorError>;

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
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data
            }),
        }
    }
}

pub(crate) fn get_dispute_stage_and_status(
    code: WebhookEventCode,
    dispute_status: Option<DisputeStatus>,
) -> Result<
    (common_enums::DisputeStage, common_enums::DisputeStatus),
    error_stack::Report<WebhookError>,
> {
    use common_enums::{DisputeStage, DisputeStatus as HSDisputeStatus};

    // Stage and status mirror HS direct Adyen behavior:
    // - stage: `impl From<WebhookEventCode> for DisputeStage` in the
    //   hyperswitch Adyen connector (PreDispute is never produced there)
    // - status: the dispute event produced by `get_adyen_webhook_event`,
    //   which HS core converts 1:1 into `DisputeStatus`
    match code {
        // Stage::Dispute
        WebhookEventCode::NotificationOfChargeback => {
            Ok((DisputeStage::Dispute, HSDisputeStatus::DisputeOpened))
        }
        WebhookEventCode::Chargeback => {
            let status = match dispute_status {
                Some(DisputeStatus::Won) => HSDisputeStatus::DisputeWon,
                Some(DisputeStatus::Lost) | None => HSDisputeStatus::DisputeLost,
                Some(DisputeStatus::Accepted) => HSDisputeStatus::DisputeAccepted,
                Some(DisputeStatus::Unknown) => {
                    return Err(
                        error_stack::report!(WebhookError::WebhookBodyDecodingFailed)
                            .attach_printable(
                                "Received unknown Adyen dispute status in Chargeback event; \
                         cannot determine dispute state without a known status",
                            ),
                    );
                }
                Some(_) => HSDisputeStatus::DisputeOpened,
            };
            Ok((DisputeStage::Dispute, status))
        }
        WebhookEventCode::RequestForInformation => {
            let status = match dispute_status {
                Some(DisputeStatus::Expired) => HSDisputeStatus::DisputeExpired,
                _ => HSDisputeStatus::DisputeOpened,
            };
            Ok((DisputeStage::Dispute, status))
        }
        WebhookEventCode::InformationSupplied => {
            let status = match dispute_status {
                Some(DisputeStatus::Responded) => HSDisputeStatus::DisputeChallenged,
                _ => HSDisputeStatus::DisputeOpened,
            };
            Ok((DisputeStage::Dispute, status))
        }
        WebhookEventCode::ChargebackReversed => {
            if let Some(DisputeStatus::Unknown) = dispute_status {
                return Err(
                    error_stack::report!(WebhookError::WebhookBodyDecodingFailed).attach_printable(
                        "Received unknown Adyen dispute status in ChargebackReversed event",
                    ),
                );
            }
            let status = match dispute_status {
                Some(DisputeStatus::Pending) => HSDisputeStatus::DisputeChallenged,
                _ => HSDisputeStatus::DisputeWon,
            };
            Ok((DisputeStage::Dispute, status))
        }
        WebhookEventCode::DisputeDefensePeriodEnded => {
            let status = match dispute_status {
                Some(DisputeStatus::Accepted) => HSDisputeStatus::DisputeAccepted,
                _ => HSDisputeStatus::DisputeLost,
            };
            Ok((DisputeStage::Dispute, status))
        }
        WebhookEventCode::IssuerResponseTimeframeExpired => {
            Ok((DisputeStage::Dispute, HSDisputeStatus::DisputeWon))
        }
        // Stage::PreArbitration
        WebhookEventCode::SecondChargeback => {
            Ok((DisputeStage::PreArbitration, HSDisputeStatus::DisputeLost))
        }
        WebhookEventCode::PrearbitrationWon => {
            Ok((DisputeStage::PreArbitration, HSDisputeStatus::DisputeWon))
        }
        WebhookEventCode::PrearbitrationLost => {
            Ok((DisputeStage::PreArbitration, HSDisputeStatus::DisputeLost))
        }
        WebhookEventCode::PrearbitrationOpen | WebhookEventCode::SchemeArbitration => {
            Ok((DisputeStage::PreArbitration, HSDisputeStatus::DisputeOpened))
        }
        WebhookEventCode::PrearbitrationAccepted => Ok((
            DisputeStage::PreArbitration,
            HSDisputeStatus::DisputeAccepted,
        )),
        WebhookEventCode::PrearbitrationDeclined => Ok((
            DisputeStage::PreArbitration,
            HSDisputeStatus::DisputeChallenged,
        )),
        WebhookEventCode::PrearbitrationIssuerWithdrawn
        | WebhookEventCode::SchemeArbitrationWon => {
            Ok((DisputeStage::PreArbitration, HSDisputeStatus::DisputeWon))
        }
        WebhookEventCode::SchemeArbitrationLost => {
            Ok((DisputeStage::PreArbitration, HSDisputeStatus::DisputeLost))
        }
        // Dispute stage/status must only be resolved for dispute events;
        // refuse to guess for anything else.
        _ => Err(
            error_stack::report!(WebhookError::WebhookProcessingFailed).attach_printable(format!(
                "Received non-dispute Adyen webhook event code {code:?}; \
                 cannot resolve dispute stage and status"
            )),
        ),
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
    #[serde(alias = "capture_delay_hours")]
    pub capture_delay_hours: Option<u64>,
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
        // hyperswitch strips capture_delay_hours from the request-body metadata
        // (it is mapped to additionalData.captureDelayHours instead); mirror that.
        map.remove("capture_delay_hours");
        map.remove("captureDelayHours");

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
    if router_data.request.payment_method_type == Some(common_enums::PaymentMethodType::ApplePay) {
        return Ok(None);
    }

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
    if router_data.request.payment_method_type == Some(common_enums::PaymentMethodType::ApplePay) {
        return Ok(None);
    }

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
    let first_name = billing.and_then(|a| a.first_name.clone());
    let last_name = billing.and_then(|a| a.last_name.clone());
    // Adyen requires both names when `shopperName` is sent; never send an empty object.
    (first_name.is_some() || last_name.is_some()).then_some(ShopperName {
        first_name,
        last_name,
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
                amount_including_tax: Some(item.router_data.request.amount.amount),
                amount_excluding_tax: Some(item.router_data.request.amount.amount),
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
) -> CustomResult<Option<serde_json::Value>, ConnectorError> {
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
                .change_context(ConnectorError::response_handling_failed_http_status_unknown())
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
    /// `merchantAdviceCode` as a network advice code: the part before the first ':' when the
    /// value is `"<code> : <text>"`, else the whole trimmed value (the Mastercard Merchant Advice
    /// Code table documents bare codes such as "03"). `None` when absent or blank.
    pub fn extract_network_advice_code(&self) -> Option<String> {
        self.merchant_advice_code.as_ref().and_then(|code| {
            let code = code
                .split_once(':')
                .map_or(code.as_str(), |(first, _)| first)
                .trim();
            (!code.is_empty()).then(|| code.to_string())
        })
    }
}

fn get_adyen_split_request(
    split_request: &connector_types::AdyenSplitData,
    currency: common_enums::enums::Currency,
) -> (Option<String>, Option<Vec<AdyenSplitData>>) {
    let splits = split_request
        .split_items
        .iter()
        .map(|split_item| {
            let amount = split_item.amount.map(|value| Amount { currency, value });
            AdyenSplitData {
                amount,
                reference: split_item.reference.clone(),
                split_type: split_item.split_type.clone(),
                account: split_item.account.clone(),
                description: split_item.description.clone(),
            }
        })
        .collect();
    (split_request.store.clone(), Some(splits))
}

fn get_store_id(metadata: serde_json::Value) -> Option<String> {
    metadata
        .get("store")
        .and_then(|store| store.as_str())
        .map(|store| store.to_string())
}

// ---- ClientAuthenticationToken flow types ----

/// Creates an Adyen session for client-side SDK initialization.
/// The session_id and session_data are returned to the frontend for
/// Adyen Drop-in/Components initialization.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenClientAuthRequest {
    pub amount: Amount,
    pub merchant_account: Secret<String>,
    pub reference: String,
    pub return_url: String,
    pub country_code: Option<common_enums::CountryAlpha2>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth_type = AdyenAuthType::try_from(&router_data.connector_config)?;

        let amount = Amount {
            currency: router_data.request.currency,
            value: router_data.request.amount.amount,
        };

        let reference = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let return_url = router_data
            .resource_common_data
            .return_url
            .clone()
            .unwrap_or_else(|| "https://hyperswitch.io".to_string());

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            reference,
            return_url,
            country_code: router_data.request.country,
        })
    }
}

/// Adyen session response containing session_id and sessionData for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenClientAuthResponse {
    pub id: String,
    pub session_data: Secret<String>,
}

impl TryFrom<ResponseRouterData<AdyenClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<AdyenClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Adyen(
                AdyenClientAuthenticationResponseDomain {
                    session_id: response.id,
                    session_data: response.session_data,
                },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== OrderCreate (Sessions) Flow =====

/// Adyen CreateSession request (POST /v68/sessions)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenOrderCreateRequest {
    pub merchant_account: Secret<String>,
    pub amount: Amount,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<common_enums::CountryAlpha2>,
}

/// Adyen CreateSession response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenOrderCreateResponse {
    pub psp_reference: String,
    pub result_code: String,
    pub amount: Amount,
    pub expires_at: String,
    pub order_data: String,
    pub reference: String,
    pub remaining_amount: Amount,
}

// --- TryFrom: RouterDataV2 -> AdyenOrderCreateRequest (via macro wrapper) ---

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for AdyenOrderCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = AdyenAuthType::try_from(&router_data.connector_config)?;

        let amount = Amount {
            currency: router_data.request.currency,
            value: router_data.request.amount.amount,
        };

        let reference = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let country_code = router_data
            .resource_common_data
            .address
            .get_payment_billing()
            .and_then(|b| b.address.as_ref())
            .and_then(|a| a.country);

        Ok(Self {
            merchant_account: auth.merchant_account,
            amount,
            reference,
            country_code,
        })
    }
}

// --- TryFrom: AdyenOrderCreateResponse -> PaymentCreateOrderResponse ---

impl TryFrom<AdyenOrderCreateResponse> for PaymentCreateOrderResponse {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(response: AdyenOrderCreateResponse) -> Result<Self, Self::Error> {
        // Use psp_reference as the order_id (this is Adyen's unique reference for the order)
        Ok(Self {
            connector_order_id: response.psp_reference,
            session_data: None,
        })
    }
}

// --- TryFrom: ResponseRouterData -> RouterDataV2 (CreateOrder response handler) ---

impl TryFrom<ResponseRouterData<AdyenOrderCreateResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AdyenOrderCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let order_response = PaymentCreateOrderResponse::try_from(response.clone())?;
        let connector_order_id = order_response.connector_order_id.clone();

        let status = if item.http_code == 200 {
            // Update status to indicate successful order creation
            AttemptStatus::Pending
        } else {
            AttemptStatus::Failure
        };

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                status,
                connector_order_id: Some(connector_order_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ==================== Incremental Authorization ====================
// Adyen endpoint: POST /v68/payments/{paymentPspReference}/amountUpdates
// The `amount.value` is the new total authorized amount (replaces original).
// Response HTTP 201 indicates the adjustment request was accepted; the final
// outcome is delivered asynchronously via the AUTHORISATION_ADJUSTMENT webhook.
// Ref: https://docs.adyen.com/api-explorer/Checkout/68/post/payments/-paymentPspReference-/amountUpdates

// Adyen's /amountUpdates endpoint is documented to return exactly one synchronous
// status value — `"received"` — acknowledging that the adjustment has been queued.
// The eventual Authorised/Refused outcome is delivered asynchronously via the
// AUTHORISATION_ADJUSTMENT webhook (handled separately). The casing below matches
// Adyen's documented response verbatim, so we do not normalize casing here.
const ADYEN_INCREMENTAL_AUTH_STATUS_RECEIVED: &str = "received";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenIncrementalAuthRequest {
    pub merchant_account: Secret<String>,
    pub amount: Amount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenIncrementalAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            merchant_account: auth_type.merchant_account,
            amount: Amount {
                currency: item.router_data.request.currency,
                value: item.router_data.request.amount.amount.to_owned(),
            },
            reference: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenIncrementalAuthResponse {
    pub psp_reference: String,
    pub payment_psp_reference: Option<String>,
    pub amount: Option<Amount>,
    pub merchant_account: Option<Secret<String>>,
    pub reference: Option<String>,
    pub status: Option<String>,
}

// Maps the /amountUpdates synchronous status string to AuthorizationStatus.
//
// Adyen's /amountUpdates synchronous body carries only `status: "received"`
// (see doc link above). Final Authorised/Refused outcomes arrive asynchronously
// via the AUTHORISATION_ADJUSTMENT webhook. Non-2xx responses are handled
// upstream by get_error_response_v2, so this only runs on 2xx.
//   - status = "received"  -> Success (adjustment accepted)
//   - any other / missing  -> Processing (Adyen changed API; fall back
//                              to Processing and rely on the webhook)
fn map_incremental_auth_status(status: Option<&str>) -> common_enums::AuthorizationStatus {
    match status {
        Some(s) if s == ADYEN_INCREMENTAL_AUTH_STATUS_RECEIVED => {
            common_enums::AuthorizationStatus::Success
        }
        _ => common_enums::AuthorizationStatus::Processing,
    }
}

impl TryFrom<ResponseRouterData<AdyenIncrementalAuthResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<AdyenIncrementalAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let authorization_status = map_incremental_auth_status(item.response.status.as_deref());

        Ok(Self {
            response: Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
                status: authorization_status,
                connector_authorization_id: Some(item.response.psp_reference.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

fn construct_charge_response(
    store: Option<String>,
    split_item: &[AdyenSplitData],
) -> ConnectorSplitResponseData {
    let splits: Vec<AdyenSplitItem> = split_item
        .iter()
        .map(|split_item| AdyenSplitItem {
            amount: split_item.amount.as_ref().map(|amount| amount.value),
            reference: split_item.reference.clone(),
            split_type: split_item.split_type.clone(),
            account: split_item.account.clone(),
            description: split_item.description.clone(),
        })
        .collect();

    ConnectorSplitResponseData::AdyenSplitPayment(connector_types::AdyenSplitData {
        store,
        split_items: splits,
    })
}

fn get_capture_delay_hours(
    metadata: &Option<SecretSerdeValue>,
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<Option<u64>, Error> {
    let metadata_capture_delay =
        get_adyen_metadata(metadata.clone().map(|m| m.expose())).capture_delay_hours;

    // Mirror hyperswitch: metadata.capture_delay_hours -> additionalData.captureDelayHours.
    let capture_method = capture_method.unwrap_or_default();
    match capture_method {
        common_enums::CaptureMethod::Manual | common_enums::CaptureMethod::ManualMultiple => {
            // For manual capture, capture_delay_hours should be None.
            if let Some(hours) = metadata_capture_delay {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "metadata.capture_delay_hours",
                    context: IntegrationErrorContext {
                        additional_context: Some(format!(
                            "Adyen does not accept capture_delay_hours for manual capture \
                             (capture_method = {capture_method:?}), but metadata supplied {hours}"
                        )),
                        suggested_action: Some(
                            "Remove capture_delay_hours from metadata when capture_method is \
                             Manual/ManualMultiple, or switch to automatic capture"
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://docs.adyen.com/online-payments/capture/".to_string(),
                        ),
                    },
                }
                .into());
            }
            Ok(None)
        }
        // For automatic capture, only 0 (or None) is valid.
        common_enums::CaptureMethod::Automatic
        | common_enums::CaptureMethod::Scheduled
        | common_enums::CaptureMethod::SequentialAutomatic => match metadata_capture_delay {
            None => Ok(None),
            Some(0) => Ok(Some(0)),
            Some(hours) => Err(IntegrationError::InvalidDataFormat {
                field_name: "metadata.capture_delay_hours",
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "Adyen only accepts capture_delay_hours of 0 (or unset) for \
                         automatic capture (capture_method = {capture_method:?}), but \
                         metadata supplied {hours}"
                    )),
                    suggested_action: Some(
                        "Set metadata.capture_delay_hours to 0 (or omit it) for automatic capture"
                            .to_string(),
                    ),
                    doc_url: Some("https://docs.adyen.com/online-payments/capture/".to_string()),
                },
            }
            .into()),
        },
    }
}

/// Key of the shopper redirect result appended to `returnUrl`.
const ADYEN_REDIRECT_RESULT_KEY: &str = "redirectResult";
// ===== Native 3DS2: standalone authentication =====
//
// Native (browser, SDK-less) 3DS2 with `authenticationData.authenticationOnly: true`
// (https://docs.adyen.com/online-payments/3d-secure/standalone-authentication). Three legs that
// move no money, then the existing Authorize, which is the only money call:
//
// 1. PreAuthenticate: `POST v71/payments` with the card and
//    `authenticationData.threeDSRequestData.nativeThreeDS = preferred`. Adyen answers
//    `IdentifyShopper` + a `threeDS2` / `fingerprint` action, `ChallengeShopper` + a
//    `threeDS2` / `challenge` action, or a frictionless `AuthenticationFinished` +
//    `threeDS2Result`.
// 2. Authenticate: `POST v71/payments/details` with `details["threeds2.fingerprint"]` and the
//    `paymentData` carried over from leg 1. Answers `ChallengeShopper` or
//    `AuthenticationFinished`.
// 3. PostAuthenticate: `POST v71/payments/details` with `details.threeDSResult`, carrying the
//    challenge outcome and the `authorisationToken` taken from the challenge action.
// 4. Authorize (unchanged) sends the resulting `threeDS2Result` as `mpiData`.

const ADYEN_STANDALONE_3DS_DOC_URL: &str =
    "https://docs.adyen.com/online-payments/3d-secure/standalone-authentication";

/// Error code for an `Authorised` result on an `authenticationOnly` leg, which should never
/// happen: the payment was authorised by the authentication call itself.
const ADYEN_UNEXPECTED_AUTHORISATION: &str = "ADYEN_UNEXPECTED_AUTHORISATION";

/// `connector_feature_data` key under which a native 3DS2 leg hands its state to the next one.
/// Same convention (and the same tolerance for unrelated merchant settings in the carrier) as
/// cybersource's `three_ds_data`.
const THREE_DS_DATA_KEY: &str = "three_ds_data";

/// `threeDSCompInd` sent on the Authenticate leg: the 3DS method ran in the hidden iframe
/// PreAuthenticate returned, so Adyen only wants the completion indicator back.
const ADYEN_THREE_DS_METHOD_COMPLETED: &str = "Y";

/// `transStatus` sent on the PostAuthenticate leg when the shopper return carries none.
/// Adyen re-derives the real outcome from `authorisationToken`, and the response's
/// `threeDS2Result` is what [`get_corroborated_authentication_data`] gates on, so this is a
/// placeholder for the request shape and not a claim about the challenge.
const ADYEN_CHALLENGE_COMPLETED_TRANS_STATUS: &str = "Y";

fn standalone_3ds_missing_field(field_name: &'static str, suggested_action: &str) -> Error {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(suggested_action.to_string()),
            doc_url: Some(ADYEN_STANDALONE_3DS_DOC_URL.to_string()),
            additional_context: Some(
                "required by adyen native 3DS2 standalone authentication (authenticationOnly)"
                    .to_string(),
            ),
        },
    }
    .into()
}

fn standalone_3ds_bad_format(field_name: &'static str, additional_context: &str) -> Error {
    IntegrationError::InvalidDataFormat {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(
                "forward the values adyen returned on the previous authentication leg unchanged"
                    .to_string(),
            ),
            doc_url: Some(ADYEN_STANDALONE_3DS_DOC_URL.to_string()),
            additional_context: Some(additional_context.to_string()),
        },
    }
    .into()
}

/// `authenticationData.threeDSRequestData.nativeThreeDS`. Only `preferred` is ever sent: it is
/// the single value that makes Adyen answer with a native fingerprint / challenge action
/// instead of a redirect.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AdyenNativeThreeDs {
    Preferred,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdyenThreeDsRequestData {
    #[serde(rename = "nativeThreeDS")]
    native_three_ds: AdyenNativeThreeDs,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenAuthenticationData {
    authentication_only: bool,
    #[serde(rename = "threeDSRequestData")]
    three_ds_request_data: Option<AdyenThreeDsRequestData>,
}

/// `native_three_ds` opt-in on the Adyen connector config. Absent or `false` leaves
/// `threeDSRequestData` off the request, so Adyen keeps answering the redirect variant.
fn native_three_ds_request_data(
    connector_config: &ConnectorSpecificConfig,
) -> Option<AdyenThreeDsRequestData> {
    matches!(
        connector_config,
        ConnectorSpecificConfig::Adyen {
            native_three_ds: Some(true),
            ..
        }
    )
    .then_some(AdyenThreeDsRequestData {
        native_three_ds: AdyenNativeThreeDs::Preferred,
    })
}

/// `POST v71/payments` body for the PreAuthenticate leg: standalone authentication, so it
/// carries no capture intent and cannot settle a payment.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenPreAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    amount: Amount,
    merchant_account: Secret<String>,
    reference: String,
    payment_method: AdyenPaymentMethod<T>,
    return_url: String,
    browser_info: AdyenBrowserInfo,
    channel: Channel,
    origin: String,
    authentication_data: AdyenAuthenticationData,
    billing_address: Option<Address>,
    shopper_email: Option<common_utils::pii::Email>,
    #[serde(rename = "shopperIP")]
    shopper_ip: Option<Secret<String, common_utils::pii::IpAddress>>,
}

/// `details` of a `v71/payments/details` call. `paymentData` is a *sibling* of `details` in
/// Adyen's body, never a member of it, so it lives on [`AdyenDetailsRequest`].
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AdyenPostAuthenticateDetails {
    /// Authenticate leg: outcome of the 3DS method (device fingerprint) iframe.
    Fingerprint {
        #[serde(rename = "threeds2.fingerprint")]
        fingerprint: String,
    },
    /// PostAuthenticate leg: outcome of the ACS challenge. Base64 of the `authorisationToken`,
    /// so it is as sensitive as the token itself and must not reach a request log in the clear.
    ThreeDsResult {
        #[serde(rename = "threeDSResult")]
        three_ds_result: Secret<String>,
    },
}

/// `POST v71/payments/details` body, shared by the Authenticate and PostAuthenticate legs.
/// No amount and no capture flag: nothing here can settle a payment.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenDetailsRequest {
    details: AdyenPostAuthenticateDetails,
    /// Adyen's handle on the suspended authentication; masked for the same reason as
    /// `authorisationToken`.
    payment_data: Option<Secret<String>>,
    authentication_data: AdyenAuthenticationData,
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct AdyenAuthenticateRequest(AdyenDetailsRequest);

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct AdyenPostAuthenticateRequest(AdyenDetailsRequest);

/// `subtype` of a native `threeDS2` action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AdyenThreeDs2Subtype {
    Fingerprint,
    Challenge,
}

/// `action` of a native 3DS2 response. `subtype` and `token` are required, so only a real
/// `threeDS2` action deserialises into this shape: a redirect action carries neither, and its
/// `url` / `method` / `data` are all optional.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenThreeDs2Action {
    #[serde(rename = "type")]
    type_of_response: ActionType,
    subtype: AdyenThreeDs2Subtype,
    token: Secret<String>,
    authorisation_token: Option<Secret<String>>,
    payment_data: Option<Secret<String>>,
    payment_method_type: Option<PaymentType>,
}

/// Decoded `action.token` of a `fingerprint` action.
#[derive(Debug, Clone, Deserialize)]
pub struct AdyenFingerprintToken {
    #[serde(rename = "threeDSMethodUrl")]
    three_ds_method_url: String,
    #[serde(rename = "threeDSMethodNotificationURL")]
    three_ds_method_notification_url: String,
    #[serde(rename = "threeDSServerTransID")]
    three_ds_server_trans_id: String,
    #[serde(rename = "threeDSMessageVersion")]
    three_ds_message_version: String,
}

/// Decoded `action.token` of a `challenge` action. `acsReferenceNumber` and
/// `threeDSNotificationURL` are also present on the wire but nothing here needs them.
#[derive(Debug, Clone, Deserialize)]
pub struct AdyenChallengeToken {
    #[serde(rename = "acsURL")]
    acs_url: String,
    #[serde(rename = "acsTransID")]
    acs_trans_id: String,
    #[serde(rename = "messageVersion")]
    message_version: String,
    #[serde(rename = "threeDSServerTransID")]
    three_ds_server_trans_id: String,
}

/// `resultCode` of a standalone authentication leg.
///
/// Deliberately separate from [`AdyenStatus`]: `IdentifyShopper` has no meaning on Authorize,
/// and teaching `AdyenStatus` about it would change how every other Adyen flow reads that code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum AdyenAuthenticationStatus {
    IdentifyShopper,
    ChallengeShopper,
    AuthenticationFinished,
    AuthenticationNotRequired,
    RedirectShopper,
    Authorised,
    Refused,
    Error,
    Cancelled,
    Received,
    Pending,
    #[serde(other)]
    Unknown,
}

/// `threeDS2Result` of a standalone authentication. `transStatus` is kept as received and
/// parsed when mapped, so an unlisted value fails closed instead of failing the whole parse.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenThreeDS2Result {
    authentication_value: Option<Secret<String>>,
    eci: Option<String>,
    trans_status: Option<String>,
    #[serde(rename = "dsTransID")]
    ds_trans_id: Option<String>,
    #[serde(rename = "threeDSServerTransID")]
    three_ds_server_trans_id: Option<String>,
    message_version: Option<String>,
    challenge_cancel: Option<String>,
}

/// Response body of a standalone authentication leg. All three legs answer the same shape; the
/// newtypes exist because the connector macro derives one templating type per response type.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenAuthenticationResponse {
    result_code: AdyenAuthenticationStatus,
    psp_reference: Option<String>,
    merchant_reference: Option<String>,
    action: Option<AdyenAuthenticationAction>,
    #[serde(rename = "threeDS2Result")]
    three_ds2_result: Option<AdyenThreeDS2Result>,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
}

/// `action` on a standalone-authentication response. Only the native `threeDS2` shape is
/// runnable on these legs, but Adyen answers the same call with a `redirect` action when native
/// 3DS2 was not requested or the account cannot do it - and that body carries no `subtype` and
/// no `token`. Typed as `threeDS2` alone, such a response failed to deserialize outright, so the
/// leg reported "response body did not match the expected format" instead of the actual reason,
/// and the `RedirectShopper` arm below could never be reached.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AdyenAuthenticationAction {
    /// Ordered first so a real `threeDS2` action cannot fall through to the catch-all.
    ThreeDs2(AdyenThreeDs2Action),
    Other(AdyenNonNativeAction),
}

/// Just enough of a non-native action to name it in the error.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdyenNonNativeAction {
    #[serde(rename = "type")]
    type_of_response: ActionType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AdyenPreAuthenticationResponse(AdyenAuthenticationResponse);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AdyenPostAuthenticationResponse(AdyenAuthenticationResponse);

/// A native `threeDS2` action surfacing on Authorize / PSync / SetupMandate, which have no
/// Authenticate or PostAuthenticate leg to run it. Its variant is ordered first in
/// [`AdyenPaymentResponse`] so the action stops matching the all-optional redirect shape,
/// which yielded `redirection_data: None` and left the payment in `AuthenticationPending`
/// with no next action and no error.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenThreeDs2ActionResponse {
    result_code: AdyenStatus,
    action: AdyenThreeDs2Action,
    psp_reference: Option<String>,
    merchant_reference: Option<String>,
    refusal_reason: Option<String>,
    refusal_reason_code: Option<String>,
}

/// State the native 3DS2 ladder carries from one leg to the next.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenThreeDsState {
    payment_data: Option<Secret<String>>,
    authorisation_token: Option<Secret<String>>,
    /// The browser step the shopper has just been handed, so the caller knows which leg resumes
    /// the ladder. The token cannot say: Adyen returns an `authorisationToken` on the fingerprint
    /// action as well as on the challenge, so a caller keying on the token alone would run the
    /// PostAuthenticate leg before any challenge had happened and be refused with
    /// `RReq not received from DS`.
    stage: Option<AdyenThreeDs2Subtype>,
}

/// `connector_feature_data` carrier. Unknown top-level keys are tolerated on purpose: the same
/// carrier also holds unrelated merchant settings.
#[derive(Debug, Deserialize)]
struct AdyenThreeDsMetadata {
    three_ds_data: AdyenThreeDsState,
}

/// Reads the ladder state a previous leg left in `connector_feature_data`.
///
/// `None` when the carrier is absent or holds no `three_ds_data` key. A `three_ds_data` that is
/// present but unreadable is refused rather than ignored: continuing without `paymentData` or
/// `authorisationToken` would send Adyen a details call it cannot match to the authentication.
fn get_adyen_three_ds_state(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<Option<AdyenThreeDsState>, Error> {
    let carrier = match connector_feature_data {
        Some(carrier) if carrier.peek().get(THREE_DS_DATA_KEY).is_some() => carrier,
        _ => return Ok(None),
    };
    AdyenThreeDsMetadata::deserialize(carrier.peek())
        .map(|metadata| Some(metadata.three_ds_data))
        .map_err(|_| {
            standalone_3ds_bad_format(
                "connector_feature_data.three_ds_data",
                "connector_feature_data carries a three_ds_data object that is not an adyen native 3DS2 state",
            )
        })
}

/// Puts the ladder state under `three_ds_data` in the feature-data carrier, keeping whatever
/// unrelated merchant settings the carrier already holds.
fn set_adyen_three_ds_state(
    carrier: Option<SecretSerdeValue>,
    state: &AdyenThreeDsState,
) -> Option<SecretSerdeValue> {
    let value = serde_json::to_value(state).ok()?;
    let mut object = match carrier.map(ExposeInterface::expose) {
        Some(serde_json::Value::Object(object)) => object,
        _ => serde_json::Map::new(),
    };
    object.insert(THREE_DS_DATA_KEY.to_string(), value);
    Some(Secret::new(serde_json::Value::Object(object)))
}

/// Decodes an `action.token`. The tokens are base64url without padding; the alphabet is
/// normalised first so a padded or standard-alphabet token decodes too. Malformed input is a
/// typed error, never a panic.
fn decode_adyen_native_token<D: serde::de::DeserializeOwned>(
    token: &Secret<String>,
    field_name: &'static str,
) -> Result<D, Error> {
    let malformed =
        || standalone_3ds_bad_format(field_name, "adyen action.token is base64url-encoded JSON");
    let normalised = token.peek().replace('-', "+").replace('_', "/");
    let decoded = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(normalised.trim_end_matches('='))
        .map_err(|_| malformed())?;
    serde_json::from_slice(&decoded).map_err(|_| malformed())
}

/// `details["threeds2.fingerprint"]` of the Authenticate leg.
fn encode_adyen_fingerprint_completion() -> String {
    STANDARD.encode(
        serde_json::json!({ "threeDSCompInd": ADYEN_THREE_DS_METHOD_COMPLETED }).to_string(),
    )
}

/// `details.threeDSResult` of the PostAuthenticate leg.
fn encode_adyen_three_ds_result(
    trans_status: &str,
    authorisation_token: &Secret<String>,
) -> Secret<String> {
    Secret::new(
        STANDARD.encode(
            serde_json::json!({
                "transStatus": trans_status,
                "authorisationToken": authorisation_token.peek(),
            })
            .to_string(),
        ),
    )
}

/// Hidden-iframe form that runs Adyen's 3DS method (device fingerprint).
///
/// The field names are the ones hyperswitch's converter sniffs for
/// (`threeDsMethodDataSubmission` in `authorize_flow.rs`) to build the hidden-iframe next
/// action, so they must keep this exact spelling.
fn get_adyen_fingerprint_form(token: &AdyenFingerprintToken) -> RedirectForm {
    // EMVCo `threeDSMethodData` is base64url without padding.
    let method_data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "threeDSServerTransID": token.three_ds_server_trans_id,
            "threeDSMethodNotificationURL": token.three_ds_method_notification_url,
        })
        .to_string(),
    );
    RedirectForm::Form {
        endpoint: token.three_ds_method_url.clone(),
        method: Method::Post,
        form_fields: std::collections::HashMap::from([
            ("threeDsMethodData".to_string(), method_data),
            (
                "threeDSServerTransID".to_string(),
                token.three_ds_server_trans_id.clone(),
            ),
            (
                "threeDsMethodUrl".to_string(),
                token.three_ds_method_url.clone(),
            ),
            (
                "messageVersion".to_string(),
                token.three_ds_message_version.clone(),
            ),
            (
                "threeDsMethodDataSubmission".to_string(),
                "true".to_string(),
            ),
        ]),
    }
}

/// ACS challenge form: the EMVCo `creq` posted to `acsURL`.
fn get_adyen_challenge_form(token: &AdyenChallengeToken) -> RedirectForm {
    let creq = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "threeDSServerTransID": token.three_ds_server_trans_id,
            "acsTransID": token.acs_trans_id,
            "messageVersion": token.message_version,
            "messageType": "CReq",
            "challengeWindowSize": "05",
        })
        .to_string(),
    );
    RedirectForm::Form {
        endpoint: token.acs_url.clone(),
        method: Method::Post,
        form_fields: std::collections::HashMap::from([("creq".to_string(), creq)]),
    }
}

/// Turns a native `threeDS2` action into the browser form that runs it plus the state the next
/// leg needs. Anything that is not a `threeDS2` action is refused.
fn get_adyen_native_action(
    action: &AdyenThreeDs2Action,
) -> Result<(RedirectForm, AdyenThreeDsState), Error> {
    if !matches!(action.type_of_response, ActionType::ThreeDs2) {
        return Err(standalone_3ds_bad_format(
            "action.type",
            "native 3DS2 expects an action of type threeDS2",
        ));
    }
    let form =
        match action.subtype {
            AdyenThreeDs2Subtype::Fingerprint => get_adyen_fingerprint_form(
                &decode_adyen_native_token::<AdyenFingerprintToken>(&action.token, "action.token")?,
            ),
            AdyenThreeDs2Subtype::Challenge => get_adyen_challenge_form(
                &decode_adyen_native_token::<AdyenChallengeToken>(&action.token, "action.token")?,
            ),
        };
    Ok((
        form,
        AdyenThreeDsState {
            payment_data: action.payment_data.clone(),
            authorisation_token: action.authorisation_token.clone(),
            stage: Some(action.subtype),
        },
    ))
}

/// Maps a `threeDS2Result` onto [`AuthenticationData`] only when its ECI corroborates the
/// transaction status ([`is_three_ds_outcome_corroborated`]). Any other combination, a missing
/// result or an unlisted `transStatus` yields `None`.
fn get_corroborated_authentication_data(
    three_ds2_result: Option<&AdyenThreeDS2Result>,
) -> Option<AuthenticationData> {
    let result = three_ds2_result?;
    let trans_status = result.trans_status.as_ref().and_then(|status| {
        serde_json::from_value::<common_enums::TransactionStatus>(serde_json::Value::String(
            status.clone(),
        ))
        .ok()
    });
    if !is_three_ds_outcome_corroborated(trans_status.as_ref(), result.eci.as_deref()) {
        return None;
    }
    Some(AuthenticationData {
        trans_status,
        eci: result.eci.clone(),
        cavv: result.authentication_value.clone(),
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: result.three_ds_server_trans_id.clone(),
        message_version: result
            .message_version
            .as_deref()
            .and_then(|version| version.parse::<SemanticVersion>().ok()),
        ds_trans_id: result.ds_trans_id.clone(),
        acs_transaction_id: None,
        transaction_id: None,
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: result.challenge_cancel.clone(),
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    })
}

/// Authentication failure of a standalone leg: a 2xx body that is not a usable authentication
/// outcome is an error, never a success.
fn get_adyen_authentication_error(
    response: &AdyenAuthenticationResponse,
    status_code: u16,
    code: Option<&str>,
    reason: Option<String>,
) -> ErrorResponse {
    ErrorResponse {
        status_code,
        code: code
            .map(str::to_string)
            .or_else(|| response.refusal_reason_code.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .refusal_reason
            .clone()
            .or_else(|| reason.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: reason.or_else(|| response.refusal_reason.clone()),
        connector_transaction_id: response.psp_reference.clone(),
        ..Default::default()
    }
}

/// Which leg is reading the response. The three legs accept different result codes, and only
/// the two whose response variant carries `redirection_data` can surface a shopper action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdyenAuthenticationLeg {
    PreAuthenticate,
    Authenticate,
    PostAuthenticate,
}

/// What a standalone authentication leg produced.
#[derive(Debug, Default)]
struct AdyenNativeOutcome {
    redirection_data: Option<RedirectForm>,
    authentication_data: Option<AuthenticationData>,
    three_ds_data: Option<AdyenThreeDsState>,
}

/// Outcome of a standalone authentication leg, or the status and error to report.
fn get_adyen_native_outcome(
    response: &AdyenAuthenticationResponse,
    status_code: u16,
    leg: AdyenAuthenticationLeg,
    is_manual_capture: bool,
) -> Result<(AttemptStatus, AdyenNativeOutcome), (AttemptStatus, ErrorResponse)> {
    let failed = |reason: Option<String>| {
        (
            AttemptStatus::AuthenticationFailed,
            get_adyen_authentication_error(response, status_code, None, reason),
        )
    };
    // A leg whose response variant has no `redirection_data` cannot surface a shopper action,
    // and neither can a fingerprint that arrives after the fingerprint leg. Fail closed rather
    // than drop the action and leave the payment pending with no next step.
    let shopper_action = |allowed: bool| {
        let action = response.action.as_ref().ok_or_else(|| {
            failed(Some(format!(
                "resultCode {:?} without a threeDS2 action: the shopper step cannot be shown",
                response.result_code
            )))
        })?;
        let action = match action {
            AdyenAuthenticationAction::ThreeDs2(action) => action,
            // Adyen chose the redirect variant of standalone authentication. These legs drive
            // the native one; the caller has to run the redirect flow through Authorize.
            AdyenAuthenticationAction::Other(other) => {
                return Err(failed(Some(format!(
                    "adyen answered with a {:?} action; the native legs cannot run it",
                    other.type_of_response
                ))))
            }
        };
        if !allowed {
            return Err(failed(Some(format!(
                "unexpected {:?} action on the {leg:?} leg",
                action.subtype
            ))));
        }
        let (form, state) =
            get_adyen_native_action(action).map_err(|error| failed(Some(error.to_string())))?;
        Ok((
            AttemptStatus::AuthenticationPending,
            AdyenNativeOutcome {
                redirection_data: Some(form),
                authentication_data: None,
                three_ds_data: Some(state),
            },
        ))
    };
    // https://docs.adyen.com/online-payments/build-your-integration/payment-result-codes
    match response.result_code {
        // Native fingerprint: only the first leg can ask for one.
        AdyenAuthenticationStatus::IdentifyShopper => {
            shopper_action(leg == AdyenAuthenticationLeg::PreAuthenticate)
        }
        // Native challenge: legs 1 and 2 surface it; leg 3 carries the challenge result itself.
        AdyenAuthenticationStatus::ChallengeShopper => {
            shopper_action(leg != AdyenAuthenticationLeg::PostAuthenticate)
        }
        // Frictionless or post-challenge result, usable only when the ECI corroborates it.
        AdyenAuthenticationStatus::AuthenticationFinished => {
            match get_corroborated_authentication_data(response.three_ds2_result.as_ref()) {
                Some(data) => Ok((
                    AttemptStatus::AuthenticationSuccessful,
                    AdyenNativeOutcome {
                        authentication_data: Some(data),
                        ..Default::default()
                    },
                )),
                None => Err(failed(Some(
                    "threeDS2Result transStatus is not corroborated by its ECI".to_string(),
                ))),
            }
        }
        // Nothing to authenticate: Authorize proceeds without mpiData, which is correct - there
        // is no authentication result to send and none was needed.
        AdyenAuthenticationStatus::AuthenticationNotRequired => {
            Ok((AttemptStatus::Pending, AdyenNativeOutcome::default()))
        }
        // Still in flight. Letting Authorize run here would authorise on an authentication that
        // has not finished: no mpiData, so no liability shift, and Adyen is free to start a 3DS
        // flow of its own on the money call and challenge the shopper a second time. Only a
        // finished result, or an explicit AuthenticationNotRequired, may continue.
        AdyenAuthenticationStatus::Received | AdyenAuthenticationStatus::Pending => {
            Err(failed(Some(
                "adyen has not finished the authentication; it cannot be authorised yet"
                    .to_string(),
            )))
        }
        // `Unknown` is the `serde(other)` catch-all: a resultCode this integration has never
        // seen. Grouping it with AuthenticationNotRequired would authorise the payment with no
        // mpiData on the strength of a response nobody has read, so it fails closed instead.
        AdyenAuthenticationStatus::Unknown => Err(failed(Some(
            "adyen returned a resultCode this integration does not recognise".to_string(),
        ))),
        AdyenAuthenticationStatus::Refused
        | AdyenAuthenticationStatus::Error
        | AdyenAuthenticationStatus::Cancelled => Err(failed(None)),
        // nativeThreeDS was asked for, so a redirect action is not something these legs run.
        AdyenAuthenticationStatus::RedirectShopper => Err(failed(Some(
            "adyen answered the redirect 3DS2 variant; the native legs cannot run it".to_string(),
        ))),
        // Money moved on an authenticationOnly call: report it honestly instead of letting the
        // composite loop authorise the payment a second time.
        AdyenAuthenticationStatus::Authorised => Err((
            if is_manual_capture {
                AttemptStatus::Authorized
            } else {
                AttemptStatus::Charged
            },
            get_adyen_authentication_error(
                response,
                status_code,
                Some(ADYEN_UNEXPECTED_AUTHORISATION),
                Some("adyen authorised the payment on the authentication-only call".to_string()),
            ),
        )),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenPreAuthenticateRequest<T>
{
    type Error = Error;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // Standalone 3DS2 authenticates raw card details only.
        let card = match request.payment_method_data.as_ref() {
            Some(PaymentMethodData::Card(card)) => card,
            Some(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "3DS authentication for this payment method".to_string(),
                    connector: "adyen",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "use Authorize for wallets, network tokens and stored tokens; \
                             standalone 3DS authenticates raw cards only"
                                .to_string(),
                        ),
                        doc_url: Some(ADYEN_STANDALONE_3DS_DOC_URL.to_string()),
                        additional_context: None,
                    },
                }
                .into())
            }
            None => {
                return Err(standalone_3ds_missing_field(
                    "payment_method_data",
                    "send the card to authenticate",
                ))
            }
        };

        // returnUrl must bring the shopper back to the complete-authorize route, so prefer
        // continue_redirection_url; a caller that only sets return_url (the global
        // PreAuthenticate scenario does) still gets a usable authentication.
        let continue_redirection_url = request
            .continue_redirection_url
            .as_ref()
            .or(request.router_return_url.as_ref())
            .ok_or_else(|| {
                standalone_3ds_missing_field(
                    "continue_redirection_url",
                    "send continue_redirection_url, or return_url, as the URL the shopper \
                     returns to after the 3DS steps",
                )
            })?;
        let origin = continue_redirection_url.origin().ascii_serialization();

        // browserInfo is required for channel Web; each getter names its own field.
        let browser = request.browser_info.as_ref().ok_or_else(|| {
            standalone_3ds_missing_field(
                "browser_info",
                "send browser_info with accept_header, user_agent, language, color_depth, \
                 screen_height, screen_width, time_zone_offset_minutes and java_enabled",
            )
        })?;
        let browser_info = AdyenBrowserInfo {
            accept_header: browser.get_accept_header()?,
            language: browser.get_language()?,
            screen_height: browser.get_screen_height()?,
            screen_width: browser.get_screen_width()?,
            color_depth: browser.get_color_depth()?,
            user_agent: browser.get_user_agent()?,
            time_zone_offset: browser.get_time_zone()?,
            java_enabled: browser.get_java_enabled()?,
        };

        let auth_type = AdyenAuthType::try_from(&router_data.connector_config)?;
        let card_holder_name = card.card_holder_name.clone().or_else(|| {
            router_data
                .resource_common_data
                .get_optional_billing_full_name()
        });
        let billing_address =
            get_address_info(router_data.resource_common_data.get_optional_billing())
                .and_then(Result::ok);

        Ok(Self {
            // Both halves come from the one `Money`, so the value can never be sent under a
            // different currency than it was priced in.
            amount: Amount {
                currency: request.amount.currency,
                value: request.amount.amount,
            },
            merchant_account: auth_type.merchant_account,
            reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_method: AdyenPaymentMethod::try_from((card, card_holder_name))?,
            return_url: continue_redirection_url.to_string(),
            browser_info,
            channel: Channel::Web,
            origin,
            authentication_data: AdyenAuthenticationData {
                authentication_only: true,
                three_ds_request_data: native_three_ds_request_data(&router_data.connector_config),
            },
            billing_address,
            shopper_email: request.email.clone(),
            shopper_ip: browser
                .ip_address
                .map(|ip_address| Secret::new(ip_address.to_string())),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenAuthenticateRequest
{
    type Error = Error;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_data = get_adyen_three_ds_state(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        )?
        .and_then(|state| state.payment_data)
        .ok_or_else(|| {
            standalone_3ds_missing_field(
                "connector_feature_data.three_ds_data.paymentData",
                "forward the connector_feature_data returned by PreAuthenticate unchanged",
            )
        })?;
        Ok(Self(AdyenDetailsRequest {
            details: AdyenPostAuthenticateDetails::Fingerprint {
                fingerprint: encode_adyen_fingerprint_completion(),
            },
            payment_data: Some(payment_data),
            authentication_data: AdyenAuthenticationData {
                authentication_only: true,
                three_ds_request_data: None,
            },
        }))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AdyenRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AdyenPostAuthenticateRequest
{
    type Error = Error;
    fn try_from(
        item: AdyenRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let authorisation_token = get_adyen_three_ds_state(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        )?
        .and_then(|state| state.authorisation_token)
        .ok_or_else(|| {
            standalone_3ds_missing_field(
                "connector_feature_data.three_ds_data.authorisationToken",
                "forward the connector_feature_data returned by the challenge leg unchanged",
            )
        })?;
        // The ACS posts its CRes to Adyen's own `threeDSNotificationURL`, never back here, so
        // there is no shopper-supplied `transStatus` to read - and Adyen does not take ours on
        // trust either. Measured against the sandbox: a challenge answered with the wrong ACS
        // password, with this leg still sending `Y`, came back `Refused` with
        // `threeDS2Result.transStatus: "N"`, `eci 07`, `refusalReason: "3D Not Authenticated"`.
        // Adyen resolves the real outcome from `authorisationToken`, which is its own handle on
        // the challenge; this field only says the browser step finished.
        Ok(Self(AdyenDetailsRequest {
            details: AdyenPostAuthenticateDetails::ThreeDsResult {
                three_ds_result: encode_adyen_three_ds_result(
                    ADYEN_CHALLENGE_COMPLETED_TRANS_STATUS,
                    &authorisation_token,
                ),
            },
            // `paymentData` is not sent on this leg: `authorisationToken` identifies the
            // authentication Adyen is finishing.
            payment_data: None,
            authentication_data: AdyenAuthenticationData {
                authentication_only: true,
                three_ds_request_data: None,
            },
        }))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AdyenPreAuthenticationResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AdyenPreAuthenticationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: AdyenPreAuthenticationResponse(response),
            router_data,
            http_code,
        } = value;
        let is_manual_capture = is_manual_capture(router_data.request.capture_method);
        let mut resource_common_data = router_data.resource_common_data;
        let response_data = match get_adyen_native_outcome(
            &response,
            http_code,
            AdyenAuthenticationLeg::PreAuthenticate,
            is_manual_capture,
        ) {
            Ok((status, outcome)) => {
                resource_common_data.status = status;
                if let Some(state) = outcome.three_ds_data.as_ref() {
                    resource_common_data.connector_feature_data = set_adyen_three_ds_state(
                        resource_common_data.connector_feature_data.clone(),
                        state,
                    );
                }
                Ok(PaymentsResponseData::PreAuthenticateResponse {
                    resource_id: response
                        .psp_reference
                        .clone()
                        .map(ResponseId::ConnectorTransactionId),
                    authentication_data: outcome.authentication_data,
                    redirection_data: outcome.redirection_data.map(Box::new),
                    connector_response_reference_id: response.merchant_reference.clone(),
                    status_code: http_code,
                })
            }
            Err((status, error)) => {
                resource_common_data.status = status;
                Err(error)
            }
        };
        Ok(Self {
            response: response_data,
            resource_common_data,
            ..router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AdyenAuthenticationResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AdyenAuthenticationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;
        let is_manual_capture = is_manual_capture(router_data.request.capture_method);
        let mut resource_common_data = router_data.resource_common_data;
        let response_data = match get_adyen_native_outcome(
            &response,
            http_code,
            AdyenAuthenticationLeg::Authenticate,
            is_manual_capture,
        ) {
            Ok((status, outcome)) => {
                resource_common_data.status = status;
                // This leg's grpc response reads `connector_feature_data` off the response
                // variant, not off the flow data.
                let connector_feature_data = outcome.three_ds_data.as_ref().and_then(|state| {
                    set_adyen_three_ds_state(
                        resource_common_data.connector_feature_data.clone(),
                        state,
                    )
                    .map(ExposeInterface::expose)
                });
                Ok(PaymentsResponseData::AuthenticateResponse {
                    resource_id: response
                        .psp_reference
                        .clone()
                        .map(ResponseId::ConnectorTransactionId),
                    redirection_data: outcome.redirection_data.map(Box::new),
                    authentication_data: outcome.authentication_data,
                    connector_feature_data,
                    connector_response_reference_id: response.merchant_reference.clone(),
                    status_code: http_code,
                    // This leg is authenticationOnly, so it never charges: an Adyen-side
                    // authorisation here is reported as ADYEN_UNEXPECTED_AUTHORISATION before
                    // this variant is built.
                    mandate_reference: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                })
            }
            Err((status, error)) => {
                resource_common_data.status = status;
                Err(error)
            }
        };
        Ok(Self {
            response: response_data,
            resource_common_data,
            ..router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<AdyenPostAuthenticationResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        value: ResponseRouterData<AdyenPostAuthenticationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: AdyenPostAuthenticationResponse(response),
            router_data,
            http_code,
        } = value;
        let is_manual_capture = is_manual_capture(router_data.request.capture_method);
        let mut resource_common_data = router_data.resource_common_data;
        let response_data = match get_adyen_native_outcome(
            &response,
            http_code,
            AdyenAuthenticationLeg::PostAuthenticate,
            is_manual_capture,
        ) {
            Ok((status, outcome)) => {
                resource_common_data.status = status;
                Ok(PaymentsResponseData::PostAuthenticateResponse {
                    authentication_data: outcome.authentication_data,
                    connector_response_reference_id: response.merchant_reference.clone(),
                    status_code: http_code,
                })
            }
            Err((status, error)) => {
                resource_common_data.status = status;
                Err(error)
            }
        };
        Ok(Self {
            response: response_data,
            resource_common_data,
            ..router_data
        })
    }
}

/// Authorize / PSync / SetupMandate cannot run a native 3DS2 action: those flows have no
/// Authenticate or PostAuthenticate leg to complete it. Fail closed naming the subtype, rather
/// than returning no next step.
pub fn get_three_ds2_action_response(
    response: AdyenThreeDs2ActionResponse,
    status_code: u16,
) -> CustomResult<AdyenPaymentsResponseData, ConnectorError> {
    let error = ErrorResponse {
        status_code,
        code: response
            .refusal_reason_code
            .clone()
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response.refusal_reason.clone().unwrap_or_else(|| {
            format!(
                "adyen returned a native 3DS2 {:?} action on a flow that cannot run it",
                response.action.subtype
            )
        }),
        reason: Some(
            "run native 3DS2 through the PreAuthenticate / Authenticate / PostAuthenticate legs, \
             or turn native 3DS2 off on the adyen account"
                .to_string(),
        ),
        connector_transaction_id: response.psp_reference.clone(),
        ..Default::default()
    };
    Ok(AdyenPaymentsResponseData {
        status: AttemptStatus::AuthenticationFailed,
        error: Some(error),
        payments_response_data: PaymentsResponseData::TransactionResponse {
            resource_id: response
                .psp_reference
                .clone()
                .map(ResponseId::ConnectorTransactionId)
                .unwrap_or(ResponseId::NoResponseId),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response
                .merchant_reference
                .clone()
                .or(response.psp_reference),
            incremental_authorization_allowed: None,
            status_code,
            splits: None,
            payment_account_reference: None,
        },
        txn_amount: None,
        connector_response: None,
    })
}
