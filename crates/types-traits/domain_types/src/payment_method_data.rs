use std::fmt::Debug;

use base64::Engine;
use common_enums::{CardNetwork, CountryAlpha2, RegulatedName, SamsungPayCardBrand};
use common_utils::{
    ext_traits::OptionExt, new_types::MaskedBankAccount, pii::UpiVpaMaskingStrategy, Email,
    ValidationError,
};
use error_stack::{self, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use time::{Date, PrimitiveDateTime};
use utoipa::ToSchema;

pub use crate::router_data::PazeDecryptedData;
use crate::{
    errors::{self, ApiError, ApplicationErrorResponse, ConnectorError},
    utils::{get_card_issuer, missing_field_err, CardIssuer, Error},
};

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct Card<T: PaymentMethodDataTypes> {
    pub card_number: RawCardNumber<T>,
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub card_cvc: Secret<String>,
    pub card_issuer: Option<String>,
    pub card_network: Option<CardNetwork>,
    pub card_type: Option<String>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<String>,
    pub nick_name: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
    pub co_badged_card_data: Option<CoBadgedCardData>,
}

pub trait PaymentMethodDataTypes: Clone {
    type Inner: Default + Debug + Send + Eq + PartialEq + Serialize + DeserializeOwned + Clone;

    fn peek_inner(inner: &Self::Inner) -> &str;
    fn is_cobadged_inner(inner: &Self::Inner) -> Result<bool, ConnectorError>;
}

/// PCI holder implementation for handling raw PCI data
#[derive(Default, Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
pub struct DefaultPCIHolder;

/// Vault token holder implementation for handling vault token data
#[derive(Default, Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
pub struct VaultTokenHolder;
/// Generic CardNumber struct that uses PaymentMethodDataTypes trait
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawCardNumber<T: PaymentMethodDataTypes>(pub T::Inner);

impl<T: PaymentMethodDataTypes> RawCardNumber<T> {
    pub fn peek(&self) -> &str {
        T::peek_inner(&self.0)
    }

    pub fn is_cobadged_card(&self) -> Result<bool, ConnectorError> {
        T::is_cobadged_inner(&self.0)
    }
}

impl PaymentMethodDataTypes for DefaultPCIHolder {
    type Inner = cards::CardNumber;

    fn peek_inner(inner: &Self::Inner) -> &str {
        inner.peek()
    }

    fn is_cobadged_inner(inner: &Self::Inner) -> Result<bool, ConnectorError> {
        inner
            .is_cobadged_card()
            .map_err(|_| ConnectorError::RequestEncodingFailed)
    }
}

impl PaymentMethodDataTypes for VaultTokenHolder {
    type Inner = String; //Token

    fn peek_inner(inner: &Self::Inner) -> &str {
        inner
    }

    fn is_cobadged_inner(_inner: &Self::Inner) -> Result<bool, ConnectorError> {
        // Vault tokens don't have cobadged concept - always return false
        Ok(false)
    }
}

// Generic implementation for all Card<T> types
impl<T: PaymentMethodDataTypes> Card<T> {
    pub fn get_card_expiry_year_2_digit(&self) -> Result<Secret<String>, ConnectorError> {
        let binding = self.card_exp_year.clone();
        let year = binding.peek();
        Ok(Secret::new(
            year.get(year.len() - 2..)
                .ok_or(ConnectorError::RequestEncodingFailed)?
                .to_string(),
        ))
    }

    pub fn get_card_expiry_month_2_digit(&self) -> Result<Secret<String>, errors::ConnectorError> {
        let exp_month = self
            .card_exp_month
            .peek()
            .to_string()
            .parse::<u8>()
            .map_err(|_| errors::ConnectorError::InvalidDataFormat {
                field_name: "payment_method_data.card.card_exp_month",
            })?;
        let month = cards::validate::CardExpirationMonth::try_from(exp_month).map_err(|_| {
            errors::ConnectorError::InvalidDataFormat {
                field_name: "payment_method_data.card.card_exp_month",
            }
        })?;
        Ok(Secret::new(month.two_digits()))
    }

    pub fn get_card_expiry_month_year_2_digit_with_delimiter(
        &self,
        delimiter: String,
    ) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?;
        Ok(Secret::new(format!(
            "{}{}{}",
            self.card_exp_month.peek(),
            delimiter,
            year.peek()
        )))
    }

    pub fn get_expiry_year_4_digit(&self) -> Secret<String> {
        let mut year = self.card_exp_year.peek().clone();
        if year.len() == 2 {
            year = format!("20{year}");
        }
        Secret::new(year)
    }

    pub fn get_expiry_month_as_i8(&self) -> Result<Secret<i8>, Error> {
        self.card_exp_month
            .peek()
            .clone()
            .parse::<i8>()
            .change_context(ConnectorError::ResponseDeserializationFailed)
            .map(Secret::new)
    }

    pub fn get_expiry_date_as_yyyymm(&self, delimiter: &str) -> Secret<String> {
        let year = self.get_expiry_year_4_digit();
        Secret::new(format!(
            "{}{}{}",
            year.peek(),
            delimiter,
            self.card_exp_month.peek()
        ))
    }

    pub fn get_expiry_date_as_mmyy(&self) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?;
        let month = self.get_card_expiry_month_2_digit()?;
        Ok(Secret::new(format!("{}{}", month.peek(), year.peek())))
    }

    pub fn get_card_expiry_year_month_2_digit_with_delimiter(
        &self,
        delimiter: String,
    ) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?;
        Ok(Secret::new(format!(
            "{}{}{}",
            year.peek(),
            delimiter,
            self.card_exp_month.peek()
        )))
    }

    pub fn get_cardholder_name(&self) -> Result<Secret<String>, Error> {
        self.card_holder_name
            .clone()
            .ok_or_else(missing_field_err("card.card_holder_name"))
    }
}

impl Card<DefaultPCIHolder> {
    pub fn get_card_issuer(&self) -> Result<CardIssuer, Error> {
        get_card_issuer(self.card_number.peek())
    }
    pub fn get_expiry_date_as_mmyyyy(&self, delimiter: &str) -> Secret<String> {
        let year = self.get_expiry_year_4_digit();
        Secret::new(format!(
            "{}{}{}",
            self.card_exp_month.peek(),
            delimiter,
            year.peek()
        ))
    }
    pub fn get_expiry_date_as_yymm(&self) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?.expose();
        let month = self.card_exp_month.clone().expose();
        Ok(Secret::new(format!("{year}{month}")))
    }
    pub fn get_expiry_year_as_i32(&self) -> Result<Secret<i32>, Error> {
        self.card_exp_year
            .peek()
            .clone()
            .parse::<i32>()
            .change_context(ConnectorError::ResponseDeserializationFailed)
            .map(Secret::new)
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    Card(Card<T>),
    CardDetailsForNetworkTransactionId(CardDetailsForNetworkTransactionId),
    DecryptedWalletTokenDetailsForNetworkTransactionId(
        DecryptedWalletTokenDetailsForNetworkTransactionId,
    ),
    CardRedirect(CardRedirectData),
    Wallet(WalletData),
    PayLater(PayLaterData),
    BankRedirect(BankRedirectData),
    BankDebit(BankDebitData),
    BankTransfer(Box<BankTransferData>),
    Crypto(CryptoData),
    MandatePayment,
    Reward,
    RealTimePayment(Box<RealTimePaymentData>),
    Upi(UpiData),
    Voucher(VoucherData),
    GiftCard(Box<GiftCardData>),
    CardToken(CardToken),
    OpenBanking(OpenBankingData),
    NetworkToken(NetworkTokenData),
    MobilePayment(MobilePaymentData),
}

impl<T: PaymentMethodDataTypes> PaymentMethodData<T> {
    /// Extracts the UpiSource from UPI payment method data
    /// Returns None if the payment method is not UPI or if upi_source is not set
    pub fn get_upi_source(&self) -> Option<&UpiSource> {
        match self {
            Self::Upi(upi_data) => match upi_data {
                UpiData::UpiIntent(intent_data) => intent_data.upi_source.as_ref(),
                UpiData::UpiQr(qr_data) => qr_data.upi_source.as_ref(),
                UpiData::UpiCollect(collect_data) => collect_data.upi_source.as_ref(),
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenBankingData {
    OpenBankingPIS {},
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobilePaymentData {
    DirectCarrierBilling {
        /// The phone number of the user
        msisdn: String,
        /// Unique user identifier
        client_uid: Option<String>,
    },
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct NetworkTokenData {
    pub token_number: cards::NetworkToken,
    pub token_exp_month: Secret<String>,
    pub token_exp_year: Secret<String>,
    pub token_cryptogram: Option<Secret<String>>,
    pub card_issuer: Option<String>,
    pub card_network: Option<common_enums::CardNetwork>,
    pub card_type: Option<String>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<String>,
    pub nick_name: Option<Secret<String>>,
    pub eci: Option<String>,
}

impl NetworkTokenData {
    pub fn get_card_issuer(&self) -> Result<CardIssuer, Error> {
        get_card_issuer(self.token_number.peek())
    }

    pub fn get_expiry_year_4_digit(&self) -> Secret<String> {
        let mut year = self.token_exp_year.peek().clone();
        if year.len() == 2 {
            year = format!("20{year}");
        }
        Secret::new(year)
    }
    pub fn get_token_expiry_year_2_digit(&self) -> Result<Secret<String>, ConnectorError> {
        let binding = self.token_exp_year.clone();
        let year = binding.peek();
        Ok(Secret::new(
            year.get(year.len() - 2..)
                .ok_or(ConnectorError::RequestEncodingFailed)?
                .to_string(),
        ))
    }

    pub fn get_network_token(&self) -> cards::NetworkToken {
        self.token_number.clone()
    }

    pub fn get_network_token_expiry_month(&self) -> Secret<String> {
        self.token_exp_month.clone()
    }

    pub fn get_network_token_expiry_year(&self) -> Secret<String> {
        self.token_exp_year.clone()
    }

    pub fn get_cryptogram(&self) -> Option<Secret<String>> {
        self.token_cryptogram.clone()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GiftCardData {
    Givex(GiftCardDetails),
    PaySafeCard {},
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GiftCardDetails {
    /// The gift card number
    pub number: Secret<String>,
    /// The card verification code.
    pub cvc: Secret<String>,
}

#[derive(Eq, PartialEq, Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct CardToken {
    /// The card holder's name
    pub card_holder_name: Option<Secret<String>>,

    /// The CVC number for the card
    pub card_cvc: Option<Secret<String>>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BoletoVoucherData {
    /// The shopper's social security number
    pub social_security_number: Option<Secret<String>>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AlfamartVoucherData {}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IndomaretVoucherData {}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JCSVoucherData {}

/// Data required for the next step in a voucher-based payment flow.
///
/// Voucher payments (like Boleto in Brazil) require the customer to complete payment offline
/// by visiting a physical location or using banking apps. This structure contains all the
/// information needed to display payment instructions to the customer, including:
/// - Reference number to identify the payment
/// - Barcode/digitable line for scanning or manual entry
/// - URLs to download or view payment instructions
/// - QR code URL for mobile wallet payments (Pix)
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VoucherNextStepData {
    /// Voucher entry date
    pub entry_date: Option<String>,
    /// Voucher expiry date and time
    pub expires_at: Option<i64>,
    /// Voucher expiry date and time
    pub expiry_date: Option<PrimitiveDateTime>,
    /// Reference number required for the transaction
    pub reference: String,
    /// Url to download the payment instruction
    pub download_url: Option<String>,
    /// Url to payment instruction page
    pub instructions_url: Option<String>,
    /// Human-readable numeric version of the barcode.
    pub digitable_line: Option<Secret<String>>,
    /// Machine-readable numeric code used to generate the barcode representation.
    pub barcode: Option<Secret<String>>,
    /// The url for Pix Qr code given by the connector associated with the voucher
    pub qr_code_url: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoucherData {
    Boleto(Box<BoletoVoucherData>),
    Efecty,
    PagoEfectivo,
    RedCompra,
    RedPagos,
    Alfamart(Box<AlfamartVoucherData>),
    Indomaret(Box<IndomaretVoucherData>),
    Oxxo,
    SevenEleven(Box<JCSVoucherData>),
    Lawson(Box<JCSVoucherData>),
    MiniStop(Box<JCSVoucherData>),
    FamilyMart(Box<JCSVoucherData>),
    Seicomart(Box<JCSVoucherData>),
    PayEasy(Box<JCSVoucherData>),
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpiData {
    /// UPI Collect - Customer approves a collect request sent to their UPI app
    UpiCollect(UpiCollectData),
    /// UPI Intent - Customer is redirected to their UPI app with a pre-filled payment request
    UpiIntent(UpiIntentData),
    /// UPI QR - Unique QR generated per txn
    UpiQr(UpiQrData),
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UpiSource {
    UpiCc,      // UPI Credit Card (RuPay credit on UPI)
    UpiCl,      // UPI Credit Line
    UpiAccount, // UPI Bank Account (Savings)
    UpiCcCl,    // UPI Credit Card + Credit Line
    UpiPpi,     // UPI Prepaid Payment Instrument
    UpiVoucher, // UPI Voucher
}

impl UpiSource {
    /// Converts UpiSource to payment mode string for PhonePe connector
    /// Maps: UPI_CC/UPI_CL/UPI_CC_CL/UPI_PPI/UPI_VOUCHER -> "ALL", UPI_ACCOUNT -> "ACCOUNT"
    pub fn to_payment_mode(&self) -> String {
        match self {
            Self::UpiCc | Self::UpiCl | Self::UpiCcCl | Self::UpiPpi | Self::UpiVoucher => {
                "ALL".to_string()
            }
            Self::UpiAccount => "ACCOUNT".to_string(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UpiCollectData {
    pub vpa_id: Option<Secret<String, UpiVpaMaskingStrategy>>,
    pub upi_source: Option<UpiSource>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct UpiIntentData {
    pub upi_source: Option<UpiSource>,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct UpiQrData {
    pub upi_source: Option<UpiSource>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum RealTimePaymentData {
    DuitNow {},
    Fps {},
    PromptPay {},
    VietQr {},
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CryptoData {
    pub pay_currency: Option<String>,
    pub network: Option<String>,
}

impl CryptoData {
    pub fn get_pay_currency(&self) -> Result<String, Error> {
        self.pay_currency
            .clone()
            .ok_or_else(missing_field_err("crypto_data.pay_currency"))
    }
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BankTransferData {
    AchBankTransfer {},
    SepaBankTransfer {},
    BacsBankTransfer {},
    MultibancoBankTransfer {},
    PermataBankTransfer {},
    BcaBankTransfer {},
    BniVaBankTransfer {},
    BriVaBankTransfer {},
    CimbVaBankTransfer {},
    DanamonVaBankTransfer {},
    MandiriVaBankTransfer {},
    Pix {
        /// Unique key for pix transfer
        pix_key: Option<Secret<String>>,
        /// CPF is a Brazilian tax identification number
        cpf: Option<Secret<String>>,
        /// CNPJ is a Brazilian company tax identification number
        cnpj: Option<Secret<String>>,
        /// Source bank account UUID
        source_bank_account_id: Option<MaskedBankAccount>,
        /// Destination bank account UUID.
        destination_bank_account_id: Option<MaskedBankAccount>,
        /// Session expiry date for Pix QR code (max 5 days from now for Adyen)
        expiry_date: Option<time::PrimitiveDateTime>,
    },
    Pse {},
    LocalBankTransfer {
        bank_code: Option<String>,
    },
    InstantBankTransfer {},
    InstantBankTransferFinland {},
    InstantBankTransferPoland {},
    IndonesianBankTransfer {
        bank_name: Option<common_enums::BankNames>,
    },
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BankDebitData {
    AchBankDebit {
        account_number: Secret<String>,
        routing_number: Secret<String>,
        card_holder_name: Option<Secret<String>>,
        bank_account_holder_name: Option<Secret<String>>,
        bank_name: Option<common_enums::BankNames>,
        bank_type: Option<common_enums::BankType>,
        bank_holder_type: Option<common_enums::BankHolderType>,
    },
    SepaBankDebit {
        iban: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
    SepaGuaranteedBankDebit {
        iban: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
    BecsBankDebit {
        account_number: Secret<String>,
        bsb_number: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
    BacsBankDebit {
        account_number: Secret<String>,
        sort_code: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum BankRedirectData {
    BancontactCard {
        card_number: Option<cards::CardNumber>,
        card_exp_month: Option<Secret<String>>,
        card_exp_year: Option<Secret<String>>,
        card_holder_name: Option<Secret<String>>,
    },
    Bizum {},
    Blik {
        blik_code: Option<String>,
    },
    Eps {
        bank_name: Option<common_enums::BankNames>,
        country: Option<CountryAlpha2>,
    },
    Giropay {
        bank_account_bic: Option<Secret<String>>,
        bank_account_iban: Option<Secret<String>>,
        country: Option<CountryAlpha2>,
    },
    Ideal {
        bank_name: Option<common_enums::BankNames>,
    },
    Interac {
        country: Option<CountryAlpha2>,
        email: Option<Email>,
    },
    OnlineBankingCzechRepublic {
        issuer: common_enums::BankNames,
    },
    OnlineBankingFinland {
        email: Option<Email>,
    },
    OnlineBankingPoland {
        issuer: common_enums::BankNames,
    },
    OnlineBankingSlovakia {
        issuer: common_enums::BankNames,
    },
    OpenBankingUk {
        issuer: Option<common_enums::BankNames>,
        country: Option<CountryAlpha2>,
    },
    Przelewy24 {
        bank_name: Option<common_enums::BankNames>,
    },
    Sofort {
        country: Option<CountryAlpha2>,
        preferred_language: Option<String>,
    },
    Trustly {
        country: Option<CountryAlpha2>,
    },
    OnlineBankingFpx {
        issuer: common_enums::BankNames,
    },
    OnlineBankingThailand {
        issuer: common_enums::BankNames,
    },
    LocalBankRedirect {},
    Eft {
        provider: String,
    },
    OpenBanking {},
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PayLaterData {
    KlarnaRedirect {},
    KlarnaSdk { token: String },
    AffirmRedirect {},
    AfterpayClearpayRedirect {},
    PayBrightRedirect {},
    WalleyRedirect {},
    AlmaRedirect {},
    AtomeRedirect {},
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum WalletData {
    AliPayQr(Box<AliPayQr>),
    AliPayRedirect(AliPayRedirection),
    AliPayHkRedirect(AliPayHkRedirection),
    BluecodeRedirect {},
    AmazonPayRedirect(Box<AmazonPayRedirectData>),
    MomoRedirect(MomoRedirection),
    KakaoPayRedirect(KakaoPayRedirection),
    GoPayRedirect(GoPayRedirection),
    GcashRedirect(GcashRedirection),
    ApplePay(ApplePayWalletData),
    ApplePayRedirect(Box<ApplePayRedirectData>),
    ApplePayThirdPartySdk(Box<ApplePayThirdPartySdkData>),
    DanaRedirect {},
    GooglePay(GooglePayWalletData),
    GooglePayRedirect(Box<GooglePayRedirectData>),
    GooglePayThirdPartySdk(Box<GooglePayThirdPartySdkData>),
    MbWayRedirect(Box<MbWayRedirection>),
    MobilePayRedirect(Box<MobilePayRedirection>),
    PaypalRedirect(PaypalRedirection),
    PaypalSdk(PayPalWalletData),
    Paze(Box<PazeWalletData>),
    SamsungPay(Box<SamsungPayWalletData>),
    TwintRedirect {},
    VippsRedirect {},
    TouchNGoRedirect(Box<TouchNGoRedirection>),
    WeChatPayRedirect(Box<WeChatPayRedirection>),
    WeChatPayQr(Box<WeChatPayQr>),
    CashappQr(Box<CashappQr>),
    SwishQr(SwishQrData),
    Mifinity(MifinityData),
    RevolutPay(RevolutPayData),
    MbWay(MbWayData),
    Satispay(SatispayData),
    Wero(WeroData),
}

impl WalletData {
    pub fn get_wallet_token(&self) -> Result<Secret<String>, Error> {
        match self {
            Self::GooglePay(data) => Ok(data.get_googlepay_encrypted_payment_data()?),
            Self::ApplePay(data) => Ok(data.get_applepay_decoded_payment_data()?),
            Self::PaypalSdk(data) => Ok(Secret::new(data.token.clone())),
            _ => Err(ConnectorError::InvalidWallet.into()),
        }
    }
    pub fn get_wallet_token_as_json<T>(&self, wallet_name: String) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        serde_json::from_str::<T>(self.get_wallet_token()?.peek())
            .change_context(ConnectorError::InvalidWalletToken { wallet_name })
    }

    pub fn get_encoded_wallet_token(&self) -> Result<String, Error> {
        match self {
            Self::GooglePay(_) => {
                let json_token: serde_json::Value =
                    self.get_wallet_token_as_json("Google Pay".to_owned())?;
                let token_as_vec = serde_json::to_vec(&json_token).change_context(
                    ConnectorError::InvalidWalletToken {
                        wallet_name: "Google Pay".to_string(),
                    },
                )?;
                let encoded_token = base64::engine::general_purpose::STANDARD.encode(token_as_vec);
                Ok(encoded_token)
            }
            _ => Err(ConnectorError::NotImplemented("SELECTED PAYMENT METHOD".to_owned()).into()),
        }
    }
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct RevolutPayData {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct MbWayData {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct SatispayData {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct WeroData {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct MifinityData {
    #[schema(value_type = Date)]
    pub date_of_birth: Secret<Date>,
    pub language_preference: Option<String>,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct SwishQrData {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct CashappQr {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct WeChatPayQr {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct WeChatPayRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct TouchNGoRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SamsungPayWalletCredentials {
    pub method: Option<String>,
    pub recurring_payment: Option<bool>,
    pub card_brand: SamsungPayCardBrand,
    pub dpan_last_four_digits: Option<String>,
    #[serde(rename = "card_last4digits")]
    pub card_last_four_digits: String,
    #[serde(rename = "3_d_s")]
    pub token_data: SamsungPayTokenData,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SamsungPayTokenData {
    #[serde(rename = "type")]
    pub three_ds_type: Option<String>,
    pub version: String,
    pub data: Secret<String>,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct SamsungPayWalletData {
    pub payment_credential: SamsungPayWalletCredentials,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum PazeWalletData {
    CompleteResponse(Secret<String>),
    Decrypted(Box<PazeDecryptedData>),
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct PayPalWalletData {
    /// Token generated for the Apple pay
    pub token: String,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct PaypalRedirection {
    /// paypal's email address
    #[schema(max_length = 255, value_type = Option<String>, example = "johntest@test.com")]
    pub email: Option<Email>,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct GooglePayThirdPartySdkData {
    pub token: Option<Secret<String>>,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct GooglePayWalletData {
    /// The type of payment method
    #[serde(rename = "type")]
    pub pm_type: String,
    /// User-facing message to describe the payment method that funds this transaction.
    pub description: String,
    /// The information of the payment method
    pub info: GooglePayPaymentMethodInfo,
    /// The tokenization data of Google pay
    pub tokenization_data: GpayTokenizationData,
}

impl GooglePayWalletData {
    fn get_googlepay_encrypted_payment_data(&self) -> Result<Secret<String>, Error> {
        let encrypted_data = self
            .tokenization_data
            .get_encrypted_google_pay_payment_data_mandatory()
            .change_context(ConnectorError::InvalidWalletToken {
                wallet_name: "Google Pay".to_string(),
            })?;

        Ok(Secret::new(encrypted_data.token.clone()))
    }

    pub fn validate_decrypted_card_exp_month(
        value: Option<Secret<String>>,
    ) -> Result<Secret<String>, error_stack::Report<ApplicationErrorResponse>> {
        value.ok_or_else(|| {
            error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "MISSING_CARD_EXP_MONTH".to_owned(),
                error_identifier: 400,
                error_message: "Google Pay tokenization data card exp month is required".to_owned(),
                error_object: None,
            }))
        })
    }

    pub fn validate_decrypted_card_exp_year(
        value: Option<Secret<String>>,
    ) -> Result<Secret<String>, error_stack::Report<ApplicationErrorResponse>> {
        value.ok_or_else(|| {
            error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "MISSING_CARD_EXP_YEAR".to_owned(),
                error_identifier: 400,
                error_message: "Google Pay tokenization data card exp year is required".to_owned(),
                error_object: None,
            }))
        })
    }

    pub fn validate_decrypted_primary_account_number(
        value: Option<cards::CardNumber>,
    ) -> Result<cards::CardNumber, error_stack::Report<ApplicationErrorResponse>> {
        value.ok_or_else(|| {
            error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "MISSING_APPLICATION_PRIMARY_ACCOUNT_NUMBER".to_owned(),
                error_identifier: 400,
                error_message:
                    "Google Pay tokenization data application primary account number is required"
                        .to_owned(),
                error_object: None,
            }))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
/// This enum is used to represent the Gpay payment data, which can either be encrypted or decrypted.
pub enum GpayTokenizationData {
    /// This variant contains the decrypted Gpay payment data as a structured object.
    Decrypted(GooglePayDecryptedData),
    /// This variant contains the encrypted Gpay payment data as a string.
    Encrypted(GpayEncryptedTokenizationData),
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
/// This struct represents the encrypted Gpay payment data
pub struct GpayEncryptedTokenizationData {
    /// The type of the token
    #[serde(rename = "type")]
    pub token_type: String,
    /// Token generated for the wallet
    pub token: String,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayDecryptedData {
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub application_primary_account_number: cards::CardNumber,
    pub cryptogram: Option<Secret<String>>,
    pub eci_indicator: Option<String>,
}

impl GooglePayDecryptedData {
    pub fn get_four_digit_expiry_year(
        &self,
    ) -> error_stack::Result<Secret<String>, ValidationError> {
        let mut year = self.card_exp_year.peek().clone();

        if year.len() == 2 {
            year = format!("20{year}");
        } else if year.len() != 4 {
            return Err(ValidationError::InvalidValue {
                message: format!(
                    "Invalid expiry year length: {}. Must be 2 or 4 digits",
                    year.len()
                ),
            }
            .into());
        }
        Ok(Secret::new(year))
    }

    pub fn get_two_digit_expiry_year(
        &self,
    ) -> error_stack::Result<Secret<String>, ValidationError> {
        let binding = self.card_exp_year.clone();
        let year = binding.peek();
        Ok(Secret::new(
            year.get(year.len() - 2..)
                .ok_or(ValidationError::InvalidValue {
                    message: "Invalid two-digit year".to_string(),
                })?
                .to_string(),
        ))
    }

    pub fn get_expiry_date_as_mmyy(&self) -> error_stack::Result<Secret<String>, ValidationError> {
        let year = self.get_two_digit_expiry_year()?.expose();
        let month = self.get_expiry_month()?.clone().expose();
        Ok(Secret::new(format!("{month}{year}")))
    }

    pub fn get_expiry_month(&self) -> error_stack::Result<Secret<String>, ValidationError> {
        let month_str = self.card_exp_month.peek();
        let month = month_str
            .parse::<u8>()
            .map_err(|_| ValidationError::InvalidValue {
                message: format!("Failed to parse expiry month: {month_str}"),
            })?;

        if !(1..=12).contains(&month) {
            return Err(ValidationError::InvalidValue {
                message: format!("Invalid expiry month: {month}. Must be between 1 and 12"),
            }
            .into());
        }

        Ok(self.card_exp_month.clone())
    }
}

impl GpayTokenizationData {
    /// Get the encrypted Google Pay payment data, returning an error if it does not exist
    pub fn get_encrypted_google_pay_payment_data_mandatory(
        &self,
    ) -> error_stack::Result<&GpayEncryptedTokenizationData, ValidationError> {
        match self {
            Self::Encrypted(encrypted_data) => Ok(encrypted_data),
            Self::Decrypted(_) => Err(ValidationError::InvalidValue {
                message: "Encrypted Google Pay payment data is mandatory".to_string(),
            }
            .into()),
        }
    }
    /// Get the token from Google Pay tokenization data
    /// Returns the token string if encrypted data exists, otherwise returns an error
    pub fn get_encrypted_google_pay_token(&self) -> error_stack::Result<String, ValidationError> {
        Ok(self
            .get_encrypted_google_pay_payment_data_mandatory()?
            .token
            .clone())
    }

    /// Get the token type from Google Pay tokenization data
    /// Returns the token_type string if encrypted data exists, otherwise returns an error
    pub fn get_encrypted_token_type(&self) -> error_stack::Result<String, ValidationError> {
        Ok(self
            .get_encrypted_google_pay_payment_data_mandatory()?
            .token_type
            .clone())
    }
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct GooglePayPaymentMethodInfo {
    /// The name of the card network
    pub card_network: String,
    /// The details of the card
    pub card_details: String,
    //assurance_details of the card
    pub assurance_details: Option<GooglePayAssuranceDetails>,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GooglePayAssuranceDetails {
    ///indicates that Cardholder possession validation has been performed
    pub card_holder_authenticated: bool,
    /// indicates that identification and verifications (ID&V) was performed
    pub account_verified: bool,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct GooglePayRedirectData {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct ApplePayThirdPartySdkData {
    pub token: Option<Secret<String>>,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct ApplePayRedirectData {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct ApplepayPaymentMethod {
    /// The name to be displayed on Apple Pay button
    pub display_name: String,
    /// The network of the Apple pay payment method
    pub network: String,
    /// The type of the payment method
    #[serde(rename = "type")]
    pub pm_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
/// This struct represents the decrypted Apple Pay payment data
pub struct ApplePayDecryptedData {
    /// The primary account number
    pub application_primary_account_number: cards::CardNumber,
    /// The application expiration date (PAN expiry month)
    pub application_expiration_month: Secret<String>,
    /// The application expiration date (PAN expiry year)
    pub application_expiration_year: Secret<String>,
    /// The payment data, which contains the cryptogram and ECI indicator
    pub payment_data: ApplePayCryptogramData,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
/// This struct represents the cryptogram data for Apple Pay transactions
pub struct ApplePayCryptogramData {
    /// The online payment cryptogram
    pub online_payment_cryptogram: Secret<String>,
    /// The ECI (Electronic Commerce Indicator) value
    pub eci_indicator: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
/// This enum is used to represent the Apple Pay payment data, which can either be encrypted or decrypted.
pub enum ApplePayPaymentData {
    /// This variant contains the decrypted Apple Pay payment data as a structured object.
    Decrypted(ApplePayDecryptedData),
    /// This variant contains the encrypted Apple Pay payment data as a string.
    Encrypted(String),
}

impl ApplePayPaymentData {
    /// Get the encrypted Apple Pay payment data if it exists
    pub fn get_encrypted_apple_pay_payment_data_optional(&self) -> Option<&String> {
        match self {
            Self::Encrypted(encrypted_data) => Some(encrypted_data),
            Self::Decrypted(_) => None,
        }
    }

    /// Get the decrypted Apple Pay payment data if it exists
    pub fn get_decrypted_apple_pay_payment_data_optional(&self) -> Option<&ApplePayDecryptedData> {
        match self {
            Self::Encrypted(_) => None,
            Self::Decrypted(decrypted_data) => Some(decrypted_data),
        }
    }

    /// Get the encrypted Apple Pay payment data, returning an error if it does not exist
    pub fn get_encrypted_apple_pay_payment_data_mandatory(
        &self,
    ) -> error_stack::Result<&String, ValidationError> {
        self.get_encrypted_apple_pay_payment_data_optional()
            .get_required_value("Encrypted Apple Pay payment data")
            .attach_printable("Encrypted Apple Pay payment data is mandatory")
    }

    /// Get the decrypted Apple Pay payment data, returning an error if it does not exist
    pub fn get_decrypted_apple_pay_payment_data_mandatory(
        &self,
    ) -> error_stack::Result<&ApplePayDecryptedData, ValidationError> {
        self.get_decrypted_apple_pay_payment_data_optional()
            .get_required_value("Decrypted Apple Pay payment data")
            .attach_printable("Decrypted Apple Pay payment data is mandatory")
    }
}

impl ApplePayDecryptedData {
    /// Get the four-digit expiration year from the Apple Pay pre-decrypt data
    pub fn get_two_digit_expiry_year(
        &self,
    ) -> error_stack::Result<Secret<String>, ValidationError> {
        let binding = self.application_expiration_year.clone();
        let year = binding.peek();
        Ok(Secret::new(
            year.get(year.len() - 2..)
                .ok_or(ValidationError::InvalidValue {
                    message: "Invalid two-digit year".to_string(),
                })?
                .to_string(),
        ))
    }

    /// Get the four-digit expiration year from the Apple Pay pre-decrypt data
    pub fn get_four_digit_expiry_year(&self) -> Secret<String> {
        let mut year = self.application_expiration_year.peek().clone();
        if year.len() == 2 {
            year = format!("20{year}");
        }
        Secret::new(year)
    }

    /// Get the expiration month from the Apple Pay pre-decrypt data
    pub fn get_expiry_month(&self) -> Secret<String> {
        self.application_expiration_month.clone()
    }

    /// Get the expiry date in MMYY format from the Apple Pay pre-decrypt data
    pub fn get_expiry_date_as_mmyy(&self) -> error_stack::Result<Secret<String>, ValidationError> {
        let year = self.get_two_digit_expiry_year()?.expose();
        let month = self.application_expiration_month.clone().expose();
        Ok(Secret::new(format!("{month}{year}")))
    }
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct ApplePayWalletData {
    /// The payment data of Apple pay
    pub payment_data: ApplePayPaymentData,
    /// The payment method of Apple pay
    pub payment_method: ApplepayPaymentMethod,
    /// The unique identifier for the transaction
    pub transaction_identifier: String,
}

impl ApplePayWalletData {
    pub fn validate_decrypted_primary_account_number(
        value: Option<cards::CardNumber>,
    ) -> Result<cards::CardNumber, error_stack::Report<ApplicationErrorResponse>> {
        value.ok_or_else(|| {
            error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "MISSING_APPLICATION_PRIMARY_ACCOUNT_NUMBER".to_owned(),
                error_identifier: 400,
                error_message:
                    "Apple Pay payment data application primary account number is required"
                        .to_owned(),
                error_object: None,
            }))
        })
    }

    pub fn validate_decrypted_expiration_month(
        value: Option<Secret<String>>,
    ) -> Result<Secret<String>, error_stack::Report<ApplicationErrorResponse>> {
        value.ok_or_else(|| {
            error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "MISSING_APPLICATION_EXPIRATION_MONTH".to_owned(),
                error_identifier: 400,
                error_message: "Apple Pay payment data application expiration month is required"
                    .to_owned(),
                error_object: None,
            }))
        })
    }

    pub fn validate_decrypted_expiration_year(
        value: Option<Secret<String>>,
    ) -> Result<Secret<String>, error_stack::Report<ApplicationErrorResponse>> {
        value.ok_or_else(|| {
            error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "MISSING_APPLICATION_EXPIRATION_YEAR".to_owned(),
                error_identifier: 400,
                error_message: "Apple Pay payment data application expiration year is required"
                    .to_owned(),
                error_object: None,
            }))
        })
    }

    pub fn validate_decrypted_payment_data(
        value: Option<grpc_api_types::payments::ApplePayCryptogramData>,
    ) -> Result<ApplePayCryptogramData, error_stack::Report<ApplicationErrorResponse>> {
        let decrypted_payment_data = value.ok_or_else(|| {
            error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "MISSING_DECRYPTED_PAYMENT_DATA".to_owned(),
                error_identifier: 400,
                error_message: "Apple Pay decrypted payment data is required".to_owned(),
                error_object: None,
            }))
        })?;

        Ok(ApplePayCryptogramData {
            online_payment_cryptogram: decrypted_payment_data
                .online_payment_cryptogram
                .ok_or_else(|| {
                    error_stack::report!(ApplicationErrorResponse::BadRequest(ApiError {
                        sub_code: "MISSING_ONLINE_PAYMENT_CRYPTOGRAM".to_owned(),
                        error_identifier: 400,
                        error_message:
                            "Apple Pay payment data online payment cryptogram is required"
                                .to_owned(),
                        error_object: None,
                    }))
                })?,
            eci_indicator: decrypted_payment_data.eci_indicator,
        })
    }

    pub fn get_applepay_decoded_payment_data(&self) -> Result<Secret<String>, Error> {
        let apple_pay_encrypted_data = self
            .payment_data
            .get_encrypted_apple_pay_payment_data_mandatory()
            .change_context(ConnectorError::MissingRequiredField {
                field_name: "Apple pay encrypted data",
            })?;
        let token = Secret::new(
            String::from_utf8(
                base64::engine::general_purpose::STANDARD
                    .decode(apple_pay_encrypted_data)
                    .change_context(ConnectorError::InvalidWalletToken {
                        wallet_name: "Apple Pay".to_string(),
                    })?,
            )
            .change_context(ConnectorError::InvalidWalletToken {
                wallet_name: "Apple Pay".to_string(),
            })?,
        );
        Ok(token)
    }
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct GoPayRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct GcashRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MobilePayRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MbWayRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KakaoPayRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MomoRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct AliPayHkRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct AliPayRedirection {}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct AliPayQr {}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum CardRedirectData {
    Knet {},
    Benefit {},
    MomoAtm {},
    CardRedirect {},
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct DecryptedWalletTokenDetailsForNetworkTransactionId {
    pub decrypted_token: cards::NetworkToken,
    pub token_exp_month: Secret<String>,
    pub token_exp_year: Secret<String>,
    pub card_holder_name: Option<Secret<String>>,
    pub eci: Option<String>,
    pub token_source: Option<TokenSource>,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum TokenSource {
    GooglePay,
    ApplePay,
}

impl DecryptedWalletTokenDetailsForNetworkTransactionId {
    pub fn get_card_expiry_year_2_digit(&self) -> Result<Secret<String>, ConnectorError> {
        let binding = self.token_exp_year.clone();
        let year = binding.peek();
        Ok(Secret::new(
            year.get(year.len() - 2..)
                .ok_or(ConnectorError::RequestEncodingFailed)?
                .to_string(),
        ))
    }
    pub fn get_card_issuer(&self) -> Result<CardIssuer, Error> {
        get_card_issuer(self.decrypted_token.peek())
    }
    pub fn get_card_expiry_month_year_2_digit_with_delimiter(
        &self,
        delimiter: String,
    ) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?;
        let month = self.token_exp_month.peek();
        let year_peek = year.peek();
        Ok(Secret::new(format!("{month}{delimiter}{year_peek}")))
    }
    pub fn get_expiry_date_as_yyyymm(&self, delimiter: &str) -> Secret<String> {
        let year = self.get_expiry_year_4_digit();
        let year_peek = year.peek();
        let month = self.token_exp_month.peek();
        Secret::new(format!("{year_peek}{delimiter}{month}"))
    }
    pub fn get_expiry_date_as_mmyyyy(&self, delimiter: &str) -> Secret<String> {
        let year = self.get_expiry_year_4_digit();
        let month = self.token_exp_month.peek();
        let year_peek = year.peek();
        Secret::new(format!("{month}{delimiter}{year_peek}"))
    }
    pub fn get_expiry_year_4_digit(&self) -> Secret<String> {
        let mut year = self.token_exp_year.peek().clone();
        if year.len() == 2 {
            year = format!("20{year}");
        }
        Secret::new(year)
    }
    pub fn get_expiry_date_as_yymm(&self) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?.expose();
        let month = self.token_exp_month.clone().expose();
        Ok(Secret::new(format!("{year}{month}")))
    }
    pub fn get_expiry_month_as_i8(&self) -> Result<Secret<i8>, Error> {
        self.token_exp_month
            .peek()
            .clone()
            .parse::<i8>()
            .change_context(ConnectorError::ResponseDeserializationFailed)
            .map(Secret::new)
    }
    pub fn get_expiry_year_as_i32(&self) -> Result<Secret<i32>, Error> {
        self.token_exp_year
            .peek()
            .clone()
            .parse::<i32>()
            .change_context(ConnectorError::ResponseDeserializationFailed)
            .map(Secret::new)
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Default)]
pub struct CardDetailsForNetworkTransactionId {
    pub card_number: cards::CardNumber,
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub card_issuer: Option<String>,
    pub card_network: Option<CardNetwork>,
    pub card_type: Option<String>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<String>,
    pub nick_name: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
}

impl CardDetailsForNetworkTransactionId {
    pub fn get_card_expiry_year_2_digit(&self) -> Result<Secret<String>, ConnectorError> {
        let binding = self.card_exp_year.clone();
        let year = binding.peek();
        Ok(Secret::new(
            year.get(year.len() - 2..)
                .ok_or(ConnectorError::RequestEncodingFailed)?
                .to_string(),
        ))
    }
    pub fn get_card_issuer(&self) -> Result<CardIssuer, Error> {
        get_card_issuer(self.card_number.peek())
    }
    pub fn get_card_expiry_month_year_2_digit_with_delimiter(
        &self,
        delimiter: String,
    ) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?;
        Ok(Secret::new(format!(
            "{}{}{}",
            self.card_exp_month.peek(),
            delimiter,
            year.peek()
        )))
    }
    pub fn get_expiry_date_as_yyyymm(&self, delimiter: &str) -> Secret<String> {
        let year = self.get_expiry_year_4_digit();
        Secret::new(format!(
            "{}{}{}",
            year.peek(),
            delimiter,
            self.card_exp_month.peek()
        ))
    }
    pub fn get_expiry_date_as_mmyyyy(&self, delimiter: &str) -> Secret<String> {
        let year = self.get_expiry_year_4_digit();
        Secret::new(format!(
            "{}{}{}",
            self.card_exp_month.peek(),
            delimiter,
            year.peek()
        ))
    }
    pub fn get_expiry_year_4_digit(&self) -> Secret<String> {
        let mut year = self.card_exp_year.peek().clone();
        if year.len() == 2 {
            year = format!("20{year}");
        }
        Secret::new(year)
    }
    pub fn get_expiry_date_as_yymm(&self) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?.expose();
        let month = self.card_exp_month.clone().expose();
        Ok(Secret::new(format!("{year}{month}")))
    }
    pub fn get_expiry_date_as_mmyy(&self) -> Result<Secret<String>, ConnectorError> {
        let year = self.get_card_expiry_year_2_digit()?.expose();
        let month = self.card_exp_month.clone().expose();
        Ok(Secret::new(format!("{month}{year}")))
    }
    pub fn get_expiry_month_as_i8(&self) -> Result<Secret<i8>, Error> {
        self.card_exp_month
            .peek()
            .clone()
            .parse::<i8>()
            .change_context(ConnectorError::ResponseDeserializationFailed)
            .map(Secret::new)
    }
    pub fn get_expiry_year_as_i32(&self) -> Result<Secret<i32>, Error> {
        self.card_exp_year
            .peek()
            .clone()
            .parse::<i32>()
            .change_context(ConnectorError::ResponseDeserializationFailed)
            .map(Secret::new)
    }
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct SamsungPayWebWalletData {
    /// Specifies authentication method used
    pub method: Option<String>,
    /// Value if credential is enabled for recurring payment
    pub recurring_payment: Option<bool>,
    /// Brand of the payment card
    pub card_brand: SamsungPayCardBrand,
    /// Last 4 digits of the card number
    #[serde(rename = "card_last4digits")]
    pub card_last_four_digits: String,
    /// Samsung Pay token data
    #[serde(rename = "3_d_s")]
    pub token_data: SamsungPayTokenData,
}

#[derive(Eq, PartialEq, Clone, Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct AmazonPayRedirectData {}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CoBadgedCardData {
    pub co_badged_card_networks: Vec<CardNetwork>,
    pub issuer_country_code: CountryAlpha2,
    pub is_regulated: bool,
    pub regulated_name: Option<RegulatedName>,
}

#[derive(
    Debug,
    serde::Deserialize,
    serde::Serialize,
    Clone,
    ToSchema,
    strum::EnumString,
    strum::Display,
    Eq,
    PartialEq,
)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Credit,
    Debit,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BankTransferNextStepsData {
    /// The instructions for performing a bank transfer
    #[serde(flatten)]
    pub bank_transfer_instructions: BankTransferInstructions,
    /// The details received by the receiver
    pub receiver: Option<ReceiverDetails>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BankTransferInstructions {
    /// The credit transfer for ACH transactions
    AchCreditTransfer(Box<AchTransfer>),
    /// The instructions for Multibanco bank transactions
    Multibanco(Box<MultibancoTransferInstructions>),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AchTransfer {
    pub account_number: Secret<String>,
    pub bank_name: String,
    pub routing_number: Secret<String>,
    pub swift_code: Secret<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MultibancoTransferInstructions {
    pub reference: Secret<String>,
    pub entity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReceiverDetails {
    /// The amount received by receiver
    amount_received: i64,
    /// The amount charged by ACH
    amount_charged: Option<i64>,
    /// The amount remaining to be sent via ACH
    amount_remaining: Option<i64>,
}

/// Customer Information Details
#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct CustomerInfoDetails {
    /// Customer Name
    #[schema(value_type = Option<String>)]
    pub customer_name: Option<Secret<String>>,
    /// Customer Email
    #[schema(value_type = Option<String>)]
    pub customer_email: Option<Email>,
    /// Customer Phone Number
    #[schema(value_type = Option<String>)]
    pub customer_phone_number: Option<Secret<String>>,
    /// Customer Bank Id
    #[schema(value_type = Option<String>)]
    pub customer_bank_id: Option<Secret<String>>,
    /// Customer Bank Name
    #[schema(value_type = Option<String>)]
    pub customer_bank_name: Option<Secret<String>>,
}
