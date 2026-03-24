use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Currency related errors.
#[derive(Debug, thiserror::Error)]
pub enum CurrencyError {
    /// The provided currency is not supported for amount conversion
    #[error("Unsupported currency: {currency}. Please add this currency to the supported currency list with appropriate decimal configuration.")]
    UnsupportedCurrency { currency: String },
}

/// The three-letter ISO 4217 currency code (e.g., "USD", "EUR") for the payment amount. This field is mandatory for creating a payment.
#[allow(clippy::upper_case_acronyms)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    strum::EnumIter,
    strum::VariantNames,
    ToSchema,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum Currency {
    AED,
    AFN,
    ALL,
    AMD,
    ANG,
    AOA,
    ARS,
    AUD,
    AWG,
    AZN,
    BAM,
    BBD,
    BDT,
    BGN,
    BHD,
    BIF,
    BMD,
    BND,
    BOB,
    BRL,
    BSD,
    BTN,
    BWP,
    BYN,
    BZD,
    CAD,
    CDF,
    CHF,
    CLF,
    CLP,
    CNY,
    COP,
    CRC,
    CUC,
    CUP,
    CVE,
    CZK,
    DJF,
    DKK,
    DOP,
    DZD,
    EGP,
    ERN,
    ETB,
    EUR,
    FJD,
    FKP,
    GBP,
    GEL,
    GHS,
    GIP,
    GMD,
    GNF,
    GTQ,
    GYD,
    HKD,
    HNL,
    HRK,
    HTG,
    HUF,
    IDR,
    ILS,
    INR,
    IQD,
    IRR,
    ISK,
    JMD,
    JOD,
    JPY,
    KES,
    KGS,
    KHR,
    KMF,
    KPW,
    KRW,
    KWD,
    KYD,
    KZT,
    LAK,
    LBP,
    LKR,
    LRD,
    LSL,
    LYD,
    MAD,
    MDL,
    MGA,
    MKD,
    MMK,
    MNT,
    MOP,
    MRU,
    MUR,
    MVR,
    MWK,
    MXN,
    MYR,
    MZN,
    NAD,
    NGN,
    NIO,
    NOK,
    NPR,
    NZD,
    OMR,
    PAB,
    PEN,
    PGK,
    PHP,
    PKR,
    PLN,
    PYG,
    QAR,
    RON,
    RSD,
    RUB,
    RWF,
    SAR,
    SBD,
    SCR,
    SDG,
    SEK,
    SGD,
    SHP,
    SLE,
    SLL,
    SOS,
    SRD,
    SSP,
    STD,
    STN,
    SVC,
    SYP,
    SZL,
    THB,
    TJS,
    TMT,
    TND,
    TOP,
    TRY,
    TTD,
    TWD,
    TZS,
    UAH,
    UGX,
    #[default]
    USD,
    UYU,
    UZS,
    VES,
    VND,
    VUV,
    WST,
    XAF,
    XCD,
    XOF,
    XPF,
    YER,
    ZAR,
    ZMW,
    ZWL,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, strum::Display)]
#[serde(rename_all = "lowercase")]
pub enum SamsungPayCardBrand {
    Visa,
    MasterCard,
    Amex,
    Discover,
    Unknown,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BankType {
    Checking,
    Savings,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BankHolderType {
    Personal,
    Business,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BankNames {
    AmericanExpress,
    AffinBank,
    AgroBank,
    AllianceBank,
    AmBank,
    BankOfAmerica,
    BankOfChina,
    BankIslam,
    BankMuamalat,
    BankRakyat,
    BankSimpananNasional,
    Barclays,
    BlikPSP,
    CapitalOne,
    Chase,
    Citi,
    CimbBank,
    Discover,
    NavyFederalCreditUnion,
    PentagonFederalCreditUnion,
    SynchronyBank,
    WellsFargo,
    AbnAmro,
    AsnBank,
    Bunq,
    Handelsbanken,
    HongLeongBank,
    HsbcBank,
    Ing,
    Knab,
    KuwaitFinanceHouse,
    Moneyou,
    Rabobank,
    Regiobank,
    Revolut,
    SnsBank,
    TriodosBank,
    VanLanschot,
    ArzteUndApothekerBank,
    AustrianAnadiBankAg,
    BankAustria,
    Bank99Ag,
    BankhausCarlSpangler,
    BankhausSchelhammerUndSchatteraAg,
    BankMillennium,
    BankPEKAOSA,
    BawagPskAg,
    BksBankAg,
    BrullKallmusBankAg,
    BtvVierLanderBank,
    CapitalBankGraweGruppeAg,
    CeskaSporitelna,
    Dolomitenbank,
    EasybankAg,
    EPlatbyVUB,
    ErsteBankUndSparkassen,
    FrieslandBank,
    HypoAlpeadriabankInternationalAg,
    HypoNoeLbFurNiederosterreichUWien,
    HypoOberosterreichSalzburgSteiermark,
    HypoTirolBankAg,
    HypoVorarlbergBankAg,
    HypoBankBurgenlandAktiengesellschaft,
    KomercniBanka,
    MBank,
    MarchfelderBank,
    Maybank,
    OberbankAg,
    OsterreichischeArzteUndApothekerbank,
    OcbcBank,
    PayWithING,
    PlaceZIPKO,
    PlatnoscOnlineKartaPlatnicza,
    PosojilnicaBankEGen,
    PostovaBanka,
    PublicBank,
    RaiffeisenBankengruppeOsterreich,
    RhbBank,
    SchelhammerCapitalBankAg,
    StandardCharteredBank,
    SchoellerbankAg,
    SpardaBankWien,
    SporoPay,
    SantanderPrzelew24,
    TatraPay,
    Viamo,
    VolksbankGruppe,
    VolkskreditbankAg,
    VrBankBraunau,
    UobBank,
    PayWithAliorBank,
    BankiSpoldzielcze,
    PayWithInteligo,
    BNPParibasPoland,
    BankNowySA,
    CreditAgricole,
    PayWithBOS,
    PayWithCitiHandlowy,
    PayWithPlusBank,
    ToyotaBank,
    VeloBank,
    ETransferPocztowy24,
    PlusBank,
    EtransferPocztowy24,
    BankiSpbdzielcze,
    BankNowyBfgSa,
    GetinBank,
    Blik,
    NoblePay,
    IdeaBank,
    EnveloBank,
    NestPrzelew,
    MbankMtransfer,
    Inteligo,
    PbacZIpko,
    BnpParibas,
    BankPekaoSa,
    VolkswagenBank,
    AliorBank,
    Boz,
    BangkokBank,
    KrungsriBank,
    KrungThaiBank,
    TheSiamCommercialBank,
    KasikornBank,
    OpenBankSuccess,
    OpenBankFailure,
    OpenBankCancelled,
    Aib,
    BankOfScotland,
    DanskeBank,
    FirstDirect,
    FirstTrust,
    Halifax,
    Lloyds,
    Monzo,
    NatWest,
    NationwideBank,
    RoyalBankOfScotland,
    Starling,
    TsbBank,
    TescoBank,
    UlsterBank,
    Yoursafe,
    N26,
    NationaleNederlanden,
}

/// Specifies the regulated name for a card network, primarily used for US debit card routing regulations.
/// This helps identify specific regulatory categories that cards may fall under.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RegulatedName {
    #[serde(rename = "GOVERNMENT NON-EXEMPT INTERCHANGE FEE (WITH FRAUD)")]
    #[strum(serialize = "GOVERNMENT NON-EXEMPT INTERCHANGE FEE (WITH FRAUD)")]
    NonExemptWithFraud,

    #[serde(untagged)]
    #[strum(default)]
    Unknown(String),
}

impl Currency {
    pub fn to_currency_base_unit(self, amount: i64) -> Result<String, CurrencyError> {
        let amount_f64 = self.to_currency_base_unit_asf64(amount)?;
        Ok(format!("{amount_f64:.2}"))
    }

    pub fn to_currency_base_unit_asf64(self, amount: i64) -> Result<f64, CurrencyError> {
        let exponent = self.number_of_digits_after_decimal_point()?;
        let divisor = 10_u32.pow(exponent.into());
        #[allow(clippy::as_conversions)]
        let amount_f64 = amount as f64 / f64::from(divisor);
        Ok(amount_f64)
    }

    pub fn to_currency_lower_unit(self, amount: String) -> Result<String, CurrencyError> {
        let amount_decimal =
            amount
                .parse::<f64>()
                .map_err(|_| CurrencyError::UnsupportedCurrency {
                    currency: format!("Invalid amount format: {amount}"),
                })?;
        let exponent = self.number_of_digits_after_decimal_point()?;
        let multiplier = 10_u32.pow(exponent.into());
        let final_amount = amount_decimal * f64::from(multiplier);
        Ok(final_amount.to_string())
    }

    pub fn to_currency_base_unit_with_zero_decimal_check(
        self,
        amount: i64,
    ) -> Result<String, CurrencyError> {
        if self.is_zero_decimal_currency() {
            Ok(amount.to_string())
        } else {
            self.to_currency_base_unit(amount)
        }
    }

    pub fn iso_4217(self) -> &'static str {
        match self {
            Self::AED => "784",
            Self::AFN => "971",
            Self::ALL => "008",
            Self::AMD => "051",
            Self::ANG => "532",
            Self::AOA => "973",
            Self::ARS => "032",
            Self::AUD => "036",
            Self::AWG => "533",
            Self::AZN => "944",
            Self::BAM => "977",
            Self::BBD => "052",
            Self::BDT => "050",
            Self::BGN => "975",
            Self::BHD => "048",
            Self::BIF => "108",
            Self::BMD => "060",
            Self::BND => "096",
            Self::BOB => "068",
            Self::BRL => "986",
            Self::BSD => "044",
            Self::BTN => "064",
            Self::BWP => "072",
            Self::BYN => "933",
            Self::BZD => "084",
            Self::CAD => "124",
            Self::CDF => "976",
            Self::CHF => "756",
            Self::CLF => "990",
            Self::CLP => "152",
            Self::CNY => "156",
            Self::COP => "170",
            Self::CRC => "188",
            Self::CUC => "931",
            Self::CUP => "192",
            Self::CVE => "132",
            Self::CZK => "203",
            Self::DJF => "262",
            Self::DKK => "208",
            Self::DOP => "214",
            Self::DZD => "012",
            Self::EGP => "818",
            Self::ERN => "232",
            Self::ETB => "230",
            Self::EUR => "978",
            Self::FJD => "242",
            Self::FKP => "238",
            Self::GBP => "826",
            Self::GEL => "981",
            Self::GHS => "936",
            Self::GIP => "292",
            Self::GMD => "270",
            Self::GNF => "324",
            Self::GTQ => "320",
            Self::GYD => "328",
            Self::HKD => "344",
            Self::HNL => "340",
            Self::HRK => "191",
            Self::HTG => "332",
            Self::HUF => "348",
            Self::IDR => "360",
            Self::ILS => "376",
            Self::INR => "356",
            Self::IQD => "368",
            Self::IRR => "364",
            Self::ISK => "352",
            Self::JMD => "388",
            Self::JOD => "400",
            Self::JPY => "392",
            Self::KES => "404",
            Self::KGS => "417",
            Self::KHR => "116",
            Self::KMF => "174",
            Self::KPW => "408",
            Self::KRW => "410",
            Self::KWD => "414",
            Self::KYD => "136",
            Self::KZT => "398",
            Self::LAK => "418",
            Self::LBP => "422",
            Self::LKR => "144",
            Self::LRD => "430",
            Self::LSL => "426",
            Self::LYD => "434",
            Self::MAD => "504",
            Self::MDL => "498",
            Self::MGA => "969",
            Self::MKD => "807",
            Self::MMK => "104",
            Self::MNT => "496",
            Self::MOP => "446",
            Self::MRU => "929",
            Self::MUR => "480",
            Self::MVR => "462",
            Self::MWK => "454",
            Self::MXN => "484",
            Self::MYR => "458",
            Self::MZN => "943",
            Self::NAD => "516",
            Self::NGN => "566",
            Self::NIO => "558",
            Self::NOK => "578",
            Self::NPR => "524",
            Self::NZD => "554",
            Self::OMR => "512",
            Self::PAB => "590",
            Self::PEN => "604",
            Self::PGK => "598",
            Self::PHP => "608",
            Self::PKR => "586",
            Self::PLN => "985",
            Self::PYG => "600",
            Self::QAR => "634",
            Self::RON => "946",
            Self::RSD => "941",
            Self::RUB => "643",
            Self::RWF => "646",
            Self::SAR => "682",
            Self::SBD => "090",
            Self::SCR => "690",
            Self::SDG => "938",
            Self::SEK => "752",
            Self::SGD => "702",
            Self::SHP => "654",
            Self::SLE => "925",
            Self::SLL => "694",
            Self::SOS => "706",
            Self::SRD => "968",
            Self::SSP => "728",
            Self::STD => "678",
            Self::STN => "930",
            Self::SVC => "222",
            Self::SYP => "760",
            Self::SZL => "748",
            Self::THB => "764",
            Self::TJS => "972",
            Self::TMT => "934",
            Self::TND => "788",
            Self::TOP => "776",
            Self::TRY => "949",
            Self::TTD => "780",
            Self::TWD => "901",
            Self::TZS => "834",
            Self::UAH => "980",
            Self::UGX => "800",
            Self::USD => "840",
            Self::UYU => "858",
            Self::UZS => "860",
            Self::VES => "928",
            Self::VND => "704",
            Self::VUV => "548",
            Self::WST => "882",
            Self::XAF => "950",
            Self::XCD => "951",
            Self::XOF => "952",
            Self::XPF => "953",
            Self::YER => "886",
            Self::ZAR => "710",
            Self::ZMW => "967",
            Self::ZWL => "932",
        }
    }

    pub fn is_zero_decimal_currency(self) -> bool {
        matches!(
            self,
            Self::BIF
                | Self::CLP
                | Self::DJF
                | Self::GNF
                | Self::JPY
                | Self::KMF
                | Self::KRW
                | Self::MGA
                | Self::PYG
                | Self::RWF
                | Self::UGX
                | Self::VND
                | Self::VUV
                | Self::XAF
                | Self::XOF
                | Self::XPF
        )
    }

    pub fn is_three_decimal_currency(self) -> bool {
        matches!(
            self,
            Self::BHD | Self::JOD | Self::KWD | Self::OMR | Self::TND
        )
    }

    pub fn is_four_decimal_currency(self) -> bool {
        matches!(self, Self::CLF)
    }

    /// Returns the number of decimal places for the currency based on ISO 4217 standard.
    ///
    /// **Reference**: <https://www.iso.org/iso-4217-currency-codes.html>
    ///
    /// **To add new currency**: Add to appropriate method (`is_zero_decimal_currency`, `is_two_decimal_currency`, etc.)
    pub fn number_of_digits_after_decimal_point(self) -> Result<u8, CurrencyError> {
        if self.is_zero_decimal_currency() {
            Ok(0)
        } else if self.is_three_decimal_currency() {
            Ok(3)
        } else if self.is_four_decimal_currency() {
            Ok(4)
        } else if self.is_two_decimal_currency() {
            Ok(2)
        } else {
            Err(CurrencyError::UnsupportedCurrency {
                currency: format!("{self:?}"),
            })
        }
    }

    /// Checks if the currency uses 2 decimal places (most common case).
    /// This replaces the implicit default fallback with explicit validation.
    pub fn is_two_decimal_currency(self) -> bool {
        matches!(
            self,
            Self::AED
                | Self::AFN
                | Self::ALL
                | Self::AMD
                | Self::ANG
                | Self::AOA
                | Self::ARS
                | Self::AUD
                | Self::AWG
                | Self::AZN
                | Self::BAM
                | Self::BBD
                | Self::BDT
                | Self::BGN
                | Self::BMD
                | Self::BND
                | Self::BOB
                | Self::BRL
                | Self::BSD
                | Self::BTN
                | Self::BWP
                | Self::BYN
                | Self::BZD
                | Self::CAD
                | Self::CDF
                | Self::CHF
                | Self::CNY
                | Self::COP
                | Self::CRC
                | Self::CUC
                | Self::CUP
                | Self::CVE
                | Self::CZK
                | Self::DKK
                | Self::DOP
                | Self::DZD
                | Self::EGP
                | Self::ERN
                | Self::ETB
                | Self::EUR
                | Self::FJD
                | Self::FKP
                | Self::GBP
                | Self::GEL
                | Self::GHS
                | Self::GIP
                | Self::GMD
                | Self::GTQ
                | Self::GYD
                | Self::HKD
                | Self::HNL
                | Self::HRK
                | Self::HTG
                | Self::HUF
                | Self::IDR
                | Self::ILS
                | Self::INR
                | Self::IQD
                | Self::IRR
                | Self::ISK
                | Self::JMD
                | Self::KES
                | Self::KGS
                | Self::KHR
                | Self::KPW
                | Self::KYD
                | Self::KZT
                | Self::LAK
                | Self::LBP
                | Self::LKR
                | Self::LRD
                | Self::LSL
                | Self::LYD
                | Self::MAD
                | Self::MDL
                | Self::MKD
                | Self::MMK
                | Self::MNT
                | Self::MOP
                | Self::MRU
                | Self::MUR
                | Self::MVR
                | Self::MWK
                | Self::MXN
                | Self::MYR
                | Self::MZN
                | Self::NAD
                | Self::NGN
                | Self::NIO
                | Self::NOK
                | Self::NPR
                | Self::NZD
                | Self::PAB
                | Self::PEN
                | Self::PGK
                | Self::PHP
                | Self::PKR
                | Self::PLN
                | Self::QAR
                | Self::RON
                | Self::RSD
                | Self::RUB
                | Self::SAR
                | Self::SBD
                | Self::SCR
                | Self::SDG
                | Self::SEK
                | Self::SGD
                | Self::SHP
                | Self::SLE
                | Self::SLL
                | Self::SOS
                | Self::SRD
                | Self::SSP
                | Self::STD
                | Self::STN
                | Self::SVC
                | Self::SYP
                | Self::SZL
                | Self::THB
                | Self::TJS
                | Self::TMT
                | Self::TOP
                | Self::TRY
                | Self::TTD
                | Self::TWD
                | Self::TZS
                | Self::UAH
                | Self::USD
                | Self::UYU
                | Self::UZS
                | Self::VES
                | Self::WST
                | Self::XCD
                | Self::YER
                | Self::ZAR
                | Self::ZMW
                | Self::ZWL
        )
    }
}

/// Specifies how the payment is captured.
/// - `automatic`: Funds are captured immediately after successful authorization. This is the default behavior if the field is omitted.
/// - `manual`: Funds are authorized but not captured. A separate request to the `/payments/{payment_id}/capture` endpoint is required to capture the funds.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::VariantNames,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CaptureMethod {
    #[default]
    Automatic,
    Manual,
    ManualMultiple,
    Scheduled,
    SequentialAutomatic,
}

/// Specifies how the payment method can be used for future payments.
/// - `off_session`: The payment method can be used for future payments when the customer is not present.
/// - `on_session`: The payment method is intended for use only when the customer is present during checkout.
///   If omitted, defaults to `on_session`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::VariantNames,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum FutureUsage {
    OffSession,
    #[default]
    OnSession,
}

/// To indicate the type of payment experience that the customer would go through
#[derive(
    Eq,
    strum::EnumString,
    PartialEq,
    Hash,
    Copy,
    Clone,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    ToSchema,
    Default,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PaymentExperience {
    #[default]
    RedirectToUrl,
    InvokeSdkClient,
    DisplayQrCode,
    OneClick,
    LinkWallet,
    InvokePaymentApp,
    DisplayWaitScreen,
    CollectOtp,
}

/// Indicates the sub type of payment method. Eg: 'google_pay' & 'apple_pay' for wallets.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::VariantNames,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PaymentMethodType {
    Ach,
    Affirm,
    AfterpayClearpay,
    Alfamart,
    AliPay,
    AliPayHk,
    Alma,
    AmazonPay,
    ApplePay,
    Atome,
    Bluecode,
    Bacs,
    BancontactCard,
    Becs,
    Benefit,
    Bizum,
    Blik,
    Boleto,
    BcaBankTransfer,
    BniVa,
    BriVa,
    CardRedirect,
    CimbVa,
    #[serde(rename = "classic")]
    ClassicReward,
    Card,
    CryptoCurrency,
    Cashapp,
    Dana,
    DanamonVa,
    DuitNow,
    Efecty,
    Eft,
    Eps,
    Fps,
    Evoucher,
    Giropay,
    Givex,
    GooglePay,
    GoPay,
    Gcash,
    Ideal,
    Interac,
    Indomaret,
    Klarna,
    KakaoPay,
    LocalBankRedirect,
    MandiriVa,
    Knet,
    MbWay,
    MobilePay,
    Momo,
    MomoAtm,
    Multibanco,
    OnlineBankingThailand,
    OnlineBankingCzechRepublic,
    OnlineBankingFinland,
    OnlineBankingFpx,
    OnlineBankingPoland,
    OnlineBankingSlovakia,
    Oxxo,
    PagoEfectivo,
    PermataBankTransfer,
    OpenBankingUk,
    OpenBanking,
    PayBright,
    Paypal,
    Paze,
    Pix,
    PaySafeCard,
    Przelewy24,
    PromptPay,
    Pse,
    RedCompra,
    RedPagos,
    SamsungPay,
    Satispay,
    Sepa,
    SepaBankTransfer,
    Sofort,
    Swish,
    TouchNGo,
    Trustly,
    Twint,
    UpiCollect,
    UpiIntent,
    UpiQr,
    Vipps,
    VietQr,
    Venmo,
    Walley,
    WeChatPay,
    Wero,
    SevenEleven,
    Lawson,
    MiniStop,
    FamilyMart,
    Seicomart,
    PayEasy,
    LocalBankTransfer,
    Mifinity,
    #[serde(rename = "open_banking_pis")]
    OpenBankingPIS,
    DirectCarrierBilling,
    InstantBankTransfer,
    InstantBankTransferFinland,
    InstantBankTransferPoland,
    RevolutPay,
    SepaGuaranteedDebit,
    IndonesianBankTransfer,
}

impl PaymentMethodType {
    pub fn should_check_for_customer_saved_payment_method_type(self) -> bool {
        matches!(self, Self::Card)
    }

    pub fn to_display_name(&self) -> String {
        match self {
            Self::ApplePay => "Apple Pay".to_string(),
            Self::GooglePay => "Google Pay".to_string(),
            Self::SamsungPay => "Samsung Pay".to_string(),
            Self::AliPay => "AliPay".to_string(),
            Self::WeChatPay => "WeChat Pay".to_string(),
            Self::KakaoPay => "Kakao Pay".to_string(),
            Self::GoPay => "GoPay".to_string(),
            Self::Gcash => "GCash".to_string(),
            _ => format!("{self:?}"),
        }
    }
}

/// Connector accepted currency unit as either "Base" or "Minor"
#[derive(Debug)]
pub enum CurrencyUnit {
    /// Base currency unit
    Base,
    /// Minor currency unit
    Minor,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    strum::Display,
    strum::EnumString,
    strum::EnumIter,
    serde::Serialize,
    serde::Deserialize,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RefundStatus {
    #[serde(alias = "Failure")]
    Failure,
    #[serde(alias = "ManualReview")]
    ManualReview,
    #[default]
    #[serde(alias = "Pending")]
    Pending,
    #[serde(alias = "Success")]
    Success,
    #[serde(alias = "TransactionFailure")]
    TransactionFailure,
}

/// The status of the attempt
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Hash,
    Eq,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AttemptStatus {
    Started,
    AuthenticationFailed,
    RouterDeclined,
    AuthenticationPending,
    AuthenticationSuccessful,
    Authorized,
    PartiallyAuthorized,
    AuthorizationFailed,
    Charged,
    Authorizing,
    CodInitiated,
    Expired,
    Voided,
    VoidedPostCapture,
    VoidInitiated,
    VoidPostCaptureInitiated,
    CaptureInitiated,
    CaptureFailed,
    VoidFailed,
    AutoRefunded,
    PartialCharged,
    PartialChargedAndChargeable,
    Unresolved,
    Unspecified,
    #[default]
    Pending,
    Failure,
    PaymentMethodAwaited,
    ConfirmationAwaited,
    DeviceDataCollectionPending,
    IntegrityFailure,
    Unknown,
}

impl TryFrom<u32> for AttemptStatus {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::Started,
            2 => Self::AuthenticationFailed,
            3 => Self::RouterDeclined,
            4 => Self::AuthenticationPending,
            5 => Self::AuthenticationSuccessful,
            6 => Self::Authorized,
            7 => Self::AuthorizationFailed,
            8 => Self::Charged,
            9 => Self::Authorizing,
            10 => Self::CodInitiated,
            11 => Self::Voided,
            12 => Self::VoidedPostCapture,
            13 => Self::VoidInitiated,
            14 => Self::VoidPostCaptureInitiated,
            15 => Self::CaptureInitiated,
            16 => Self::CaptureFailed,
            17 => Self::VoidFailed,
            18 => Self::AutoRefunded,
            19 => Self::PartialCharged,
            20 => Self::PartialChargedAndChargeable,
            21 => Self::Unresolved,
            22 => Self::Pending,
            23 => Self::Failure,
            24 => Self::PaymentMethodAwaited,
            25 => Self::ConfirmationAwaited,
            26 => Self::DeviceDataCollectionPending,
            27 => Self::Unspecified,
            _ => Self::Unknown,
        })
    }
}

impl AttemptStatus {
    pub fn is_terminal_status(self) -> bool {
        matches!(
            self,
            Self::Charged
                | Self::AutoRefunded
                | Self::Voided
                | Self::VoidedPostCapture
                | Self::PartialCharged
                | Self::AuthenticationFailed
                | Self::AuthorizationFailed
                | Self::VoidFailed
                | Self::CaptureFailed
                | Self::Failure
                | Self::IntegrityFailure
        )
    }
}

/// Status of the dispute
#[derive(
    Clone,
    Debug,
    Copy,
    Default,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    strum::EnumIter,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DisputeStatus {
    #[default]
    DisputeOpened,
    DisputeExpired,
    DisputeAccepted,
    DisputeCancelled,
    DisputeChallenged,
    DisputeWon,
    DisputeLost,
}

/// Stage of the dispute
#[derive(
    Clone,
    Copy,
    Default,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DisputeStage {
    PreDispute,
    #[default]
    Dispute,
    PreArbitration,
}

/// Indicates the card network.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::VariantNames,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
pub enum CardNetwork {
    #[serde(alias = "VISA")]
    Visa,
    #[serde(alias = "MASTERCARD")]
    Mastercard,
    #[serde(alias = "AMERICANEXPRESS")]
    #[serde(alias = "AMEX")]
    AmericanExpress,
    JCB,
    #[serde(alias = "DINERSCLUB")]
    DinersClub,
    #[serde(alias = "DISCOVER")]
    Discover,
    #[serde(alias = "CARTESBANCAIRES")]
    CartesBancaires,
    #[serde(alias = "UNIONPAY")]
    UnionPay,
    #[serde(alias = "INTERAC")]
    Interac,
    #[serde(alias = "RUPAY")]
    RuPay,
    #[serde(alias = "MAESTRO")]
    Maestro,
    #[serde(alias = "STAR")]
    Star,
    #[serde(alias = "PULSE")]
    Pulse,
    #[serde(alias = "ACCEL")]
    Accel,
    #[serde(alias = "NYCE")]
    Nyce,
}

impl CardNetwork {
    pub fn is_global_network(&self) -> bool {
        matches!(
            self,
            Self::Visa
                | Self::Mastercard
                | Self::AmericanExpress
                | Self::JCB
                | Self::DinersClub
                | Self::Discover
                | Self::CartesBancaires
                | Self::UnionPay
        )
    }

    pub fn is_us_local_network(&self) -> bool {
        matches!(self, Self::Star | Self::Pulse | Self::Accel | Self::Nyce)
    }
}

/// Indicates the type of payment method. Eg: 'card', 'wallet', etc.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::VariantNames,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PaymentMethod {
    #[default]
    Card,
    CardRedirect,
    PayLater,
    Wallet,
    BankRedirect,
    BankTransfer,
    Crypto,
    BankDebit,
    Reward,
    RealTimePayment,
    Upi,
    Voucher,
    GiftCard,
    OpenBanking,
    MobilePayment,
    NetworkToken,
}

/// Specifies the type of cardholder authentication to be applied for a payment.
///
/// - `ThreeDs`: Requests 3D Secure (3DS) authentication. If the card is enrolled, 3DS authentication will be activated, potentially shifting chargeback liability to the issuer.
/// - `NoThreeDs`: Indicates that 3D Secure authentication should not be performed. The liability for chargebacks typically remains with the merchant. This is often the default if not specified.
///
/// Note: The actual authentication behavior can also be influenced by merchant configuration and specific connector defaults. Some connectors might still enforce 3DS or bypass it regardless of this parameter.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::VariantNames,
    strum::EnumIter,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AuthenticationType {
    ThreeDs,
    #[default]
    NoThreeDs,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Hash,
    Eq,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EventClass {
    Payments,
    Refunds,
    Disputes,
}

#[derive(
    Clone,
    Debug,
    Eq,
    Default,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumIter,
    strum::EnumString,
    utoipa::ToSchema,
    Copy,
)]

#[rustfmt::skip]
pub enum CountryAlpha2 {
    AF, AX, AL, DZ, AS, AD, AO, AI, AQ, AG, AR, AM, AW, AU, AT,
    AZ, BS, BH, BD, BB, BY, BE, BZ, BJ, BM, BT, BO, BQ, BA, BW,
    BV, BR, IO, BN, BG, BF, BI, KH, CM, CA, CV, KY, CF, TD, CL,
    CN, CX, CC, CO, KM, CG, CD, CK, CR, CI, HR, CU, CW, CY, CZ,
    DK, DJ, DM, DO, EC, EG, SV, GQ, ER, EE, ET, FK, FO, FJ, FI,
    FR, GF, PF, TF, GA, GM, GE, DE, GH, GI, GR, GL, GD, GP, GU,
    GT, GG, GN, GW, GY, HT, HM, VA, HN, HK, HU, IS, IN, ID, IR,
    IQ, IE, IM, IL, IT, JM, JP, JE, JO, KZ, KE, KI, KP, KR, KW,
    KG, LA, LV, LB, LS, LR, LY, LI, LT, LU, MO, MK, MG, MW, MY,
    MV, ML, MT, MH, MQ, MR, MU, YT, MX, FM, MD, MC, MN, ME, MS,
    MA, MZ, MM, NA, NR, NP, NL, NC, NZ, NI, NE, NG, NU, NF, MP,
    NO, OM, PK, PW, PS, PA, PG, PY, PE, PH, PN, PL, PT, PR, QA,
    RE, RO, RU, RW, BL, SH, KN, LC, MF, PM, VC, WS, SM, ST, SA,
    SN, RS, SC, SL, SG, SX, SK, SI, SB, SO, ZA, GS, SS, ES, LK,
    SD, SR, SJ, SZ, SE, CH, SY, TW, TJ, TZ, TH, TL, TG, TK, TO,
    TT, TN, TR, TM, TC, TV, UG, UA, AE, GB, UM, UY, UZ, VU,
    VE, VN, VG, VI, WF, EH, YE, ZM, ZW,
    #[default]
    US
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Debug, Serialize, Deserialize, strum::Display)]
#[rustfmt::skip]
pub enum CountryAlpha3 {
    AFG, ALA, ALB, DZA, ASM, AND, AGO, AIA, ATA, ATG, ARG, ARM, ABW, AUS, AUT,
    AZE, BHS, BHR, BGD, BRB, BLR, BEL, BLZ, BEN, BMU, BTN, BOL, BES, BIH, BWA,
    BVT, BRA, IOT, BRN, BGR, BFA, BDI, CPV, KHM, CMR, CAN, CYM, CAF, TCD, CHL,
    CHN, CXR, CCK, COL, COM, COG, COD, COK, CRI, CIV, HRV, CUB, CUW, CYP, CZE,
    DNK, DJI, DMA, DOM, ECU, EGY, SLV, GNQ, ERI, EST, ETH, FLK, FRO, FJI, FIN,
    FRA, GUF, PYF, ATF, GAB, GMB, GEO, DEU, GHA, GIB, GRC, GRL, GRD, GLP, GUM,
    GTM, GGY, GIN, GNB, GUY, HTI, HMD, VAT, HND, HKG, HUN, ISL, IND, IDN, IRN,
    IRQ, IRL, IMN, ISR, ITA, JAM, JPN, JEY, JOR, KAZ, KEN, KIR, PRK, KOR, KWT,
    KGZ, LAO, LVA, LBN, LSO, LBR, LBY, LIE, LTU, LUX, MAC, MKD, MDG, MWI, MYS,
    MDV, MLI, MLT, MHL, MTQ, MRT, MUS, MYT, MEX, FSM, MDA, MCO, MNG, MNE, MSR,
    MAR, MOZ, MMR, NAM, NRU, NPL, NLD, NCL, NZL, NIC, NER, NGA, NIU, NFK, MNP,
    NOR, OMN, PAK, PLW, PSE, PAN, PNG, PRY, PER, PHL, PCN, POL, PRT, PRI, QAT,
    REU, ROU, RUS, RWA, BLM, SHN, KNA, LCA, MAF, SPM, VCT, WSM, SMR, STP, SAU,
    SEN, SRB, SYC, SLE, SGP, SXM, SVK, SVN, SLB, SOM, ZAF, SGS, SSD, ESP, LKA,
    SDN, SUR, SJM, SWZ, SWE, CHE, SYR, TWN, TJK, TZA, THA, TLS, TGO, TKL, TON,
    TTO, TUN, TUR, TKM, TCA, TUV, UGA, UKR, ARE, GBR, USA, UMI, URY, UZB, VUT,
    VEN, VNM, VGB, VIR, WLF, ESH, YEM, ZMB, ZWE
}

#[derive(Debug, thiserror::Error, PartialEq, Clone)]
pub enum ApiClientError {
    #[error("Header map construction failed")]
    HeaderMapConstructionFailed,
    #[error("Invalid proxy configuration")]
    InvalidProxyConfiguration,
    #[error("Client construction failed")]
    ClientConstructionFailed,
    #[error("Certificate decode failed")]
    CertificateDecodeFailed,
    #[error("Request body serialization failed")]
    BodySerializationFailed,
    #[error("Unexpected state reached/Invariants conflicted")]
    UnexpectedState,

    #[error("Failed to parse URL")]
    UrlParsingFailed,
    #[error("URL encoding of request payload failed")]
    UrlEncodingFailed,
    #[error("Failed to send request to connector {0}")]
    RequestNotSent(String),
    #[error("Failed to decode response")]
    ResponseDecodingFailed,

    #[error("Server responded with Request Timeout")]
    RequestTimeoutReceived,

    #[error("connection closed before a message could complete")]
    ConnectionClosedIncompleteMessage,

    #[error("Server responded with Internal Server Error")]
    InternalServerErrorReceived,
    #[error("Server responded with Bad Gateway")]
    BadGatewayReceived,
    #[error("Server responded with Service Unavailable")]
    ServiceUnavailableReceived,
    #[error("Server responded with Gateway Timeout")]
    GatewayTimeoutReceived,
    #[error("Server responded with unexpected response")]
    UnexpectedServerResponse,
}
impl ApiClientError {
    pub fn is_upstream_timeout(&self) -> bool {
        self == &Self::RequestTimeoutReceived
    }
    pub fn is_connection_closed_before_message_could_complete(&self) -> bool {
        self == &Self::ConnectionClosedIncompleteMessage
    }
}

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    strum::EnumString,
    strum::Display,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessTrackerRunner {
    PaymentsSyncWorkflow,
    RefundWorkflowRouter,
    DeleteTokenizeDataWorkflow,
    ApiKeyExpiryWorkflow,
    OutgoingWebhookRetryWorkflow,
    AttachPayoutAccountWorkflow,
    PaymentMethodStatusUpdateWorkflow,
    PassiveRecoveryWorkflow,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumString,
    strum::EnumIter,
    strum::VariantNames,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
/// RoutableConnectors are the subset of Connectors that are eligible for payments routing
pub enum RoutableConnectors {
    Adyenplatform,
    Aci,
    Adyen,
    Airwallex,
    // Amazonpay,
    Archipel,
    Authorizedotnet,
    Bamboraapac,
    Bluecode,
    Bankofamerica,
    Barclaycard,
    Billwerk,
    Bitpay,
    Bambora,
    Bluesnap,
    Boku,
    Braintree,
    Cashtocode,
    Chargebee,
    Checkout,
    Coinbase,
    Coingate,
    Cryptopay,
    Cybersource,
    Datatrans,
    Deutschebank,
    Digitalvirgo,
    Dlocal,
    Ebanx,
    Elavon,
    Facilitapay,
    Fiserv,
    Fiservemea,
    Fiuu,
    Forte,
    Getnet,
    Globalpay,
    Globepay,
    Gocardless,
    Hipay,
    Helcim,
    Iatapay,
    Inespay,
    Itaubank,
    Jpmorgan,
    Klarna,
    Mifinity,
    Mollie,
    Moneris,
    Multisafepay,
    Nexinets,
    Nexixpay,
    Nmi,
    Nomupay,
    Noon,
    // Nordea,
    Novalnet,
    Nuvei,
    // Opayo, added as template code for future usage
    Opennode,
    // Payeezy, As psync and rsync are not supported by this connector, it is added as template code for future usage
    Paybox,
    Payme,
    Payone,
    Paypal,
    Paystack,
    Payu,
    Placetopay,
    Powertranz,
    Prophetpay,
    Rapyd,
    Razorpay,
    Recurly,
    Redsys,
    Riskified,
    Shift4,
    Signifyd,
    Square,
    Stax,
    Stripe,
    Stripebilling,
    // Taxjar,
    Trustpay,
    // Thunes
    Tokenio,
    // Tsys,
    Tsys,
    // UnifiedAuthenticationService,
    // Vgs
    Volt,
    Wellsfargo,
    // Wellsfargopayout,
    Wise,
    Worldline,
    Worldpay,
    Worldpayvantiv,
    Worldpayxml,
    Xendit,
    Zen,
    Ppro,
    Plaid,
    Zsl,
}

/// Indicates the transaction status
#[derive(
    Clone,
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Eq,
    Hash,
    PartialEq,
    ToSchema,
    strum::Display,
    strum::EnumString,
)]

pub enum TransactionStatus {
    /// Authentication/ Account Verification Successful
    #[serde(rename = "Y")]
    Success,
    /// Not Authenticated /Account Not Verified; Transaction denied
    #[default]
    #[serde(rename = "N")]
    Failure,
    /// Authentication/ Account Verification Could Not Be Performed; Technical or other problem, as indicated in Authentication Response(ARes) or Result Request (RReq)
    #[serde(rename = "U")]
    VerificationNotPerformed,
    /// Attempts Processing Performed; Not Authenticated/Verified , but a proof of attempted authentication/verification is provided
    #[serde(rename = "A")]
    NotVerified,
    /// Authentication/ Account Verification Rejected; Issuer is rejecting authentication/verification and request that authorisation not be attempted.
    #[serde(rename = "R")]
    Rejected,
    /// Challenge Required; Additional authentication is required using the Challenge Request (CReq) / Challenge Response (CRes)
    #[serde(rename = "C")]
    ChallengeRequired,
    /// Challenge Required; Decoupled Authentication confirmed.
    #[serde(rename = "D")]
    ChallengeRequiredDecoupledAuthentication,
    /// Informational Only; 3DS Requestor challenge preference acknowledged.
    #[serde(rename = "I")]
    InformationOnly,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExemptionIndicator {
    /// Low-value payment exemption (below regulatory threshold).
    LowValue,
    /// Secure corporate payment (SCP) exemption.
    SecureCorporatePayment,
    /// Trusted beneficiary or whitelist exemption.
    TrustedListing,
    /// Transaction Risk Analysis (TRA) exemption.
    TransactionRiskAssessment,
    /// 3DS server or ACS outage exemption.
    ThreeDsOutage,
    /// SCA delegation exemption (authentication delegated to another party).
    ScaDelegation,
    /// Out of SCA scope (e.g., one-leg-out transactions).
    OutOfScaScope,
    /// Other exemption reason not covered by known types.
    Other,
    /// Low-risk program exemption (network-initiated low-risk flag).
    LowRiskProgram,
    /// Recurring transaction exemption (subsequent payment in a series).
    RecurringOperation,
}

/// This is typically provided by the card network or Access Control Server (ACS)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq, ToSchema)]
pub enum CavvAlgorithm {
    /// `00` — Reserved or unspecified algorithm.
    #[serde(rename = "00")]
    Zero,
    /// `01` — HMAC-based algorithm.
    #[serde(rename = "01")]
    One,
    /// `02` — RSA-based algorithm (standard 3DS cryptographic method).
    #[serde(rename = "02")]
    Two,
    /// `03` — Elliptic Curve algorithm.
    #[serde(rename = "03")]
    Three,
    /// `04` — Proprietary algorithm defined by the card network.
    #[serde(rename = "04")]
    Four,
    /// `A` — Custom or network-defined algorithm indicator.
    #[serde(rename = "A")]
    A,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GooglePayAuthMethod {
    /// Contain pan data only
    PanOnly,
    /// Contain cryptogram data along with pan data
    #[serde(rename = "CRYPTOGRAM_3DS")]
    Cryptogram,
}

#[derive(Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductType {
    #[default]
    Physical,
    Digital,
    Travel,
    Ride,
    Event,
    Accommodation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookTransformationStatus {
    /// Transformation completed successfully, no further action needed
    Complete,
    /// Transformation incomplete, requires second call for final status
    Incomplete,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallConnectorAction {
    Trigger,
    HandleResponse(Vec<u8>),
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
    Hash,
)]
pub enum PaymentChargeType {
    #[serde(untagged)]
    Stripe(StripeChargeType),
}

#[derive(
    Clone,
    Debug,
    Default,
    Hash,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum StripeChargeType {
    #[default]
    Direct,
    Destination,
}

#[derive(
    Default,
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
    Hash,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AuthorizationStatus {
    Success,
    Failure,
    // Processing state is before calling connector
    #[default]
    Processing,
    // Requires merchant action
    Unresolved,
}

#[derive(Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecoupledAuthenticationType {
    #[default]
    Challenge,
    Frictionless,
}

/// Enum representing the different content types that can be dynamically selected
/// for connector requests based on runtime conditions (e.g., payment method).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicContentType {
    Json,
    FormUrlEncoded,
    FormData,
}

/// US States Abbreviations (2-letter codes)
/// Used for converting full state names to abbreviations for connectors that require 2-letter codes
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display, strum::EnumString,
)]
pub enum UsStatesAbbreviation {
    AL, // Alabama
    AK, // Alaska
    AS, // American Samoa
    AZ, // Arizona
    AR, // Arkansas
    CA, // California
    CO, // Colorado
    CT, // Connecticut
    DE, // Delaware
    DC, // District of Columbia
    FM, // Federated States of Micronesia
    FL, // Florida
    GA, // Georgia
    GU, // Guam
    HI, // Hawaii
    ID, // Idaho
    IL, // Illinois
    IN, // Indiana
    IA, // Iowa
    KS, // Kansas
    KY, // Kentucky
    LA, // Louisiana
    ME, // Maine
    MH, // Marshall Islands
    MD, // Maryland
    MA, // Massachusetts
    MI, // Michigan
    MN, // Minnesota
    MS, // Mississippi
    MO, // Missouri
    MT, // Montana
    NE, // Nebraska
    NV, // Nevada
    NH, // New Hampshire
    NJ, // New Jersey
    NM, // New Mexico
    NY, // New York
    NC, // North Carolina
    ND, // North Dakota
    MP, // Northern Mariana Islands
    OH, // Ohio
    OK, // Oklahoma
    OR, // Oregon
    PW, // Palau
    PA, // Pennsylvania
    PR, // Puerto Rico
    RI, // Rhode Island
    SC, // South Carolina
    SD, // South Dakota
    TN, // Tennessee
    TX, // Texas
    UT, // Utah
    VT, // Vermont
    VI, // Virgin Islands
    VA, // Virginia
    WA, // Washington
    WV, // West Virginia
    WI, // Wisconsin
    WY, // Wyoming
}

impl UsStatesAbbreviation {
    /// Convert full state name to abbreviation
    /// Supports common variations like "New York" -> "NY", "newyork" -> "NY", etc.
    pub fn from_state_name(state_name: &str) -> Option<Self> {
        let normalized = state_name
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "");

        match normalized.as_str() {
            "alabama" => Some(Self::AL),
            "alaska" => Some(Self::AK),
            "americansamoa" => Some(Self::AS),
            "arizona" => Some(Self::AZ),
            "arkansas" => Some(Self::AR),
            "california" => Some(Self::CA),
            "colorado" => Some(Self::CO),
            "connecticut" => Some(Self::CT),
            "delaware" => Some(Self::DE),
            "districtofcolumbia" => Some(Self::DC),
            "federatedstatesofmicronesia" => Some(Self::FM),
            "florida" => Some(Self::FL),
            "georgia" => Some(Self::GA),
            "guam" => Some(Self::GU),
            "hawaii" => Some(Self::HI),
            "idaho" => Some(Self::ID),
            "illinois" => Some(Self::IL),
            "indiana" => Some(Self::IN),
            "iowa" => Some(Self::IA),
            "kansas" => Some(Self::KS),
            "kentucky" => Some(Self::KY),
            "louisiana" => Some(Self::LA),
            "maine" => Some(Self::ME),
            "marshallislands" => Some(Self::MH),
            "maryland" => Some(Self::MD),
            "massachusetts" => Some(Self::MA),
            "michigan" => Some(Self::MI),
            "minnesota" => Some(Self::MN),
            "mississippi" => Some(Self::MS),
            "missouri" => Some(Self::MO),
            "montana" => Some(Self::MT),
            "nebraska" => Some(Self::NE),
            "nevada" => Some(Self::NV),
            "newhampshire" => Some(Self::NH),
            "newjersey" => Some(Self::NJ),
            "newmexico" => Some(Self::NM),
            "newyork" => Some(Self::NY),
            "northcarolina" => Some(Self::NC),
            "northdakota" => Some(Self::ND),
            "northernmarianaislands" => Some(Self::MP),
            "ohio" => Some(Self::OH),
            "oklahoma" => Some(Self::OK),
            "oregon" => Some(Self::OR),
            "palau" => Some(Self::PW),
            "pennsylvania" => Some(Self::PA),
            "puertorico" => Some(Self::PR),
            "rhodeisland" => Some(Self::RI),
            "southcarolina" => Some(Self::SC),
            "southdakota" => Some(Self::SD),
            "tennessee" => Some(Self::TN),
            "texas" => Some(Self::TX),
            "utah" => Some(Self::UT),
            "vermont" => Some(Self::VT),
            "virginislands" => Some(Self::VI),
            "virginia" => Some(Self::VA),
            "washington" => Some(Self::WA),
            "westvirginia" => Some(Self::WV),
            "wisconsin" => Some(Self::WI),
            "wyoming" => Some(Self::WY),
            _ => None,
        }
    }
}

/// Canada States/Provinces Abbreviations (2-letter codes)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display, strum::EnumString,
)]
pub enum CanadaStatesAbbreviation {
    AB, // Alberta
    BC, // British Columbia
    MB, // Manitoba
    NB, // New Brunswick
    NL, // Newfoundland and Labrador
    NS, // Nova Scotia
    NT, // Northwest Territories
    NU, // Nunavut
    ON, // Ontario
    PE, // Prince Edward Island
    QC, // Quebec
    SK, // Saskatchewan
    YT, // Yukon
}

impl CanadaStatesAbbreviation {
    /// Convert full province name to abbreviation
    pub fn from_province_name(province_name: &str) -> Option<Self> {
        let normalized = province_name
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "");

        match normalized.as_str() {
            "alberta" => Some(Self::AB),
            "britishcolumbia" => Some(Self::BC),
            "manitoba" => Some(Self::MB),
            "newbrunswick" => Some(Self::NB),
            "newfoundlandandlabrador" | "newfoundland" | "labrador" => Some(Self::NL),
            "novascotia" => Some(Self::NS),
            "northwestterritories" => Some(Self::NT),
            "nunavut" => Some(Self::NU),
            "ontario" => Some(Self::ON),
            "princeedwardisland" => Some(Self::PE),
            "quebec" => Some(Self::QC),
            "saskatchewan" => Some(Self::SK),
            "yukon" => Some(Self::YT),
            _ => None,
        }
    }
}

/// Describes the channel through which the payment was initiated.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PaymentChannel {
    #[default]
    Ecommerce,
    MailOrder,
    TelephoneOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MitCategory {
    /// A fixed purchase amount split into multiple scheduled payments until the total is paid.
    Installment,
    /// Merchant-initiated transaction using stored credentials, but not tied to a fixed schedule
    Unscheduled,
    /// Merchant-initiated payments that happen at regular intervals (usually the same amount each time).
    Recurring,
    /// A retried MIT after a previous transaction failed or was declined.
    Resubmission,
}

/// Padding schemes used for cryptographic operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoPadding {
    /// PKCS7 padding - adds bytes equal to the number of padding bytes needed
    PKCS7,
    /// Zero padding - pads with null bytes
    ZeroPadding,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "snake_case")]
pub enum MandateStatus {
    #[default]
    Active,
    Inactive,
    Pending,
    Revoked,
    Created,
    Failed,
    Paused,
}

/// The type of tokenization to use for the payment method
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    PartialEq,
    Deserialize,
    Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum Tokenization {
    /// Skip PSP-level tokenization
    SkipPsp,
    /// Tokenize at PSP Level
    TokenizeAtPsp,
}

#[derive(Clone, Copy, Debug)]
pub enum TaxStatus {
    Taxable,
    Exempt,
}
