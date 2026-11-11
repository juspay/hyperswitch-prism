use std::str::FromStr;

use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, BankNames, CardNetwork, CountryAlpha2, Currency, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::ParsingError,
    pii::Email,
    types::{MinorUnit, StringMajorUnitWithTwoDecimals},
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        EventType, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RawConnectorStatus, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        BankRedirectData, Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        RealTimePaymentData, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{
        AuthoriseIntegrityObject, PaymentSynIntegrityObject, RefundIntegrityObject,
        RefundSyncIntegrityObject,
    },
    router_response_types::RedirectForm,
    utils::missing_field_err,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

use crate::connectors::revpay::{RevpayAmountConvertor, RevpayRouterData};

// =============================================================================
// AUTHENTICATION
// =============================================================================

/// Credentials from `ConnectorSpecificConfig::Revpay` (populated from a
/// `ConnectorAuthType::SignatureKey`).
#[derive(Debug, Clone)]
pub struct RevpayAuthType {
    /// revPAY Merchant ID (`Revpay_Merchant_ID`) — sent in the body. Config `merchant_id`.
    pub merchant_id: Secret<String>,
    /// Merchant Key — signature secret only, never transmitted. Config `merchant_key`.
    pub merchant_key: Secret<String>,
    /// `Key_Index` decimal string, sent in the body. Config `key_index`.
    pub key_index: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for RevpayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Revpay {
                merchant_id,
                merchant_key,
                key_index,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                merchant_key: merchant_key.to_owned(),
                key_index: key_index.to_owned(),
            }),
            _ => Err(error_stack::report!(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure the connector account with Revpay credentials".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "ConnectorSpecificConfig variant mismatch: expected Revpay credentials but received credentials for a different connector; the request may have been routed to the wrong connector"
                            .to_string(),
                    ),
                }
            })),
        }
    }
}

// =============================================================================
// SIGNATURE
// =============================================================================

/// revPAY electronic signature: plain SHA-512 (NOT HMAC) over the Merchant Key
/// prepended to the raw concatenation of the message fields — no separators,
/// no field names, no URL-encoding. Output is the lowercase hex digest
/// (128 chars). Verified against the doc's known-answer vectors in tests below.
fn generate_revpay_signature(merchant_key: &str, fields: &[&str]) -> String {
    let preimage = std::iter::once(merchant_key)
        .chain(fields.iter().copied())
        .collect::<String>();
    hex::encode(Sha512::digest(preimage.as_bytes()))
}

// =============================================================================
// AMOUNT CONVERSION
// =============================================================================

/// Parse a connector-echoed `Amount` through the same shared converter used for
/// request encoding, returning framework minor units for the integrity object.
pub fn parse_revpay_amount(
    amount: &str,
    currency: Currency,
) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
    let amount = amount.parse::<StringMajorUnitWithTwoDecimals>()?;
    RevpayAmountConvertor::convert_back(amount, currency).map(|money| money.amount)
}

// =============================================================================
// RESPONSE CODE
// =============================================================================

/// revPAY `Response_Code` wire enum ("00" success, "09" redirect, everything
/// else a declined/failed attempt).
///
/// A literal `#[serde(other)]` catch-all would drop the raw code, and the raw
/// code is exactly what belongs in `ErrorResponse.code` — so deserialization
/// is hand-rolled and the catch-all variant CARRIES the wire value. The
/// status mapping below stays exhaustive over named variants (no `_ =>`), so
/// adding a newly classified code forces a human decision here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevpayResponseCode {
    /// "00" — success; the sale is authorised and settled in one shot.
    Success,
    /// "09" — in progress; shopper must follow `Bank_Redirect_URL`.
    RedirectRequired,
    /// "99" — pending authorization (FPX B2B); the acquirer settles the
    /// authorisation asynchronously, so the attempt is still resolvable.
    PendingAuthorization,
    /// "76" — transaction not found. Classified apart from `Unknown` because
    /// its weight is flow-dependent: provably not chargeable on payment
    /// inquiry (§2.2), but NOT proof of a failed refund on RSync — revPAY
    /// 3.1.10 documents no refund-status inquiry, so a not-found refund
    /// reference may just mean the inquiry cannot see refunds at all (Flow 4
    /// of `grace/rulesbook/codegen/references/revpay/technical_specification.md`).
    TransactionNotFound,
    /// Any other documented or future code ("01", "30" signature mismatch,
    /// buyer-cancelled "17"/"1A"–"1L" variants, …): a failed attempt.
    Unknown(String),
}

impl RevpayResponseCode {
    fn as_str(&self) -> &str {
        match self {
            Self::Success => "00",
            Self::RedirectRequired => "09",
            Self::PendingAuthorization => "99",
            Self::TransactionNotFound => "76",
            Self::Unknown(raw) => raw.as_str(),
        }
    }

    /// Inquiry-side status mapping (§2.2). The redirect already happened on
    /// the Payment call, so on inquiry "09" is a shopper still in flight at
    /// the bank — Pending, not a fresh `AuthenticationPending`. "99" is
    /// pending authorization. Everything else is a failed attempt —
    /// "76" Transaction not found included: a payment revPAY cannot find is
    /// provably not chargeable (§2.2).
    fn psync_status(&self) -> AttemptStatus {
        match self {
            Self::Success => AttemptStatus::Charged,
            Self::RedirectRequired | Self::PendingAuthorization => AttemptStatus::Pending,
            Self::TransactionNotFound | Self::Unknown(_) => AttemptStatus::Failure,
        }
    }

    /// Refund-side status mapping (§2.5). The Refund API answers
    /// synchronously (immediate accept/reject), so only "00" proves the
    /// refund was accepted; every other code is a declined refund and the
    /// response transformer routes it into `Err(ErrorResponse)`.
    fn refund_status(&self) -> RefundStatus {
        match self {
            Self::Success => RefundStatus::Success,
            // A "76" on the Refund CALL itself is a synchronous not-found of
            // the original payment (§2.5) — a declined refund, correctly
            // terminal here. (RSync treats it differently; see below.)
            Self::RedirectRequired
            | Self::PendingAuthorization
            | Self::TransactionNotFound
            | Self::Unknown(_) => RefundStatus::Failure,
        }
    }

    /// Refund-sync status mapping (Flow 4). Unlike the synchronous Refund
    /// call, RSync is a status QUERY over the shared inquiry endpoint, so the
    /// in-flight codes "09"/"99" are resolvable intermediate states →
    /// Pending, exactly like `psync_status`. Only "00" proves the refund
    /// completed and only a failure code on a FOUND reference proves it
    /// failed. "76" is deliberately Pending, never Failure: the Refund call
    /// (§2.5) already answered accept/reject synchronously, and whether
    /// `/v1/inquiry` resolves REFUND references at all is UNVERIFIED from the
    /// vendor doc (Flow 4 of the technical spec) — so "76" may just mean the
    /// inquiry cannot see refunds. A terminal Failure here would report a
    /// completed refund as failed and invite duplicate refunds. Reported
    /// through the status, never `Err`.
    fn refund_sync_status(&self) -> RefundStatus {
        match self {
            Self::Success => RefundStatus::Success,
            Self::RedirectRequired | Self::PendingAuthorization => RefundStatus::Pending,
            Self::TransactionNotFound => RefundStatus::Pending,
            Self::Unknown(_) => RefundStatus::Failure,
        }
    }
}

impl Serialize for RevpayResponseCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RevpayResponseCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(match raw.as_str() {
            "00" => Self::Success,
            "09" => Self::RedirectRequired,
            "99" => Self::PendingAuthorization,
            "76" => Self::TransactionNotFound,
            _ => Self::Unknown(raw),
        })
    }
}

// ===== STATUS-MAPPING LAYER =====
// Deliberately exhaustive: NO `_ =>` catch-all. If revPAY adds a classifiable
// code and the enum above gains a named variant, this match stops compiling
// until a human picks the right AttemptStatus.
/// Status for a Payment API (Authorize) outcome, decided purely from the
/// response code and the presence of `Bank_Redirect_URL`. Payment authorises
/// AND captures (sale) in one call, so any decline shades into
/// `AuthorizationFailed` rather than generic `Failure` — including a "09"
/// that omits its redirect URL (a redirect that cannot complete, not a
/// pending payment). "99" is an asynchronous settle — still resolvable.
fn authorize_attempt_status(
    code: &RevpayResponseCode,
    bank_redirect_url: Option<&str>,
) -> AttemptStatus {
    match (code, bank_redirect_url) {
        (RevpayResponseCode::Success, _) => AttemptStatus::Charged,
        (RevpayResponseCode::RedirectRequired, Some(_)) => AttemptStatus::AuthenticationPending,
        (RevpayResponseCode::RedirectRequired, None) => AttemptStatus::AuthorizationFailed,
        (RevpayResponseCode::PendingAuthorization, _) => AttemptStatus::Pending,
        (RevpayResponseCode::TransactionNotFound | RevpayResponseCode::Unknown(_), _) => {
            AttemptStatus::AuthorizationFailed
        }
    }
}

impl From<&RevpayResponseCode> for AttemptStatus {
    fn from(code: &RevpayResponseCode) -> Self {
        match code {
            // The Payment API authorises and captures (sale) in one call.
            RevpayResponseCode::Success => Self::Charged,
            RevpayResponseCode::RedirectRequired => Self::AuthenticationPending,
            // FPX B2B: the acquirer confirms the authorisation asynchronously.
            RevpayResponseCode::PendingAuthorization => Self::Pending,
            // "76": a payment revPAY cannot find is provably not chargeable.
            RevpayResponseCode::TransactionNotFound => Self::Failure,
            RevpayResponseCode::Unknown(_) => Self::Failure,
        }
    }
}

// =============================================================================
// PAYMENT METHOD MAPPING
// =============================================================================

/// Payment_ID = 2 (VISA/MASTERCARD). `Customer_Email`/`Customer_Contact` are
/// mandatory on this rail, per revPAY's field table.
const CARD_PAYMENT_ID: u16 = 2;
/// Payment_ID = 4 (AMEX).
const AMEX_PAYMENT_ID: u16 = 4;
/// Payment_ID = 29 (Diners Club).
const DINERS_PAYMENT_ID: u16 = 29;
/// Merchant API 3.1.10, Appendix A.
const MYDEBIT_PAYMENT_ID: u16 = 31;
const UNIONPAY_CNP_PAYMENT_ID: u16 = 38;

/// revPAY `Card_Type` (Appendix H): `02`=VISA, `03`=MASTERCARD, `04`=AMEX,
/// `05`=MYDEBIT, `06`=UPI. The table has no Diners Club code, so `None` omits
/// the field. A card payment without `Card_Type` is rejected by the UAT
/// gateway with `Response_Code 14` "Invalid buyer account or card information"
/// (confirmed by revPAY support, 22 Oct 2026).
#[derive(Debug, Clone, Copy, Serialize)]
pub enum RevpayCardType {
    #[serde(rename = "02")]
    Visa,
    #[serde(rename = "03")]
    Mastercard,
    #[serde(rename = "04")]
    Amex,
    /// Appendix H: MyDebit Secure.
    #[serde(rename = "05")]
    MyDebit,
    /// Appendix H: Union Pay International.
    #[serde(rename = "06")]
    UnionPay,
}

fn unsupported_payment_method(
    what: &str,
    context_detail: &str,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: format!("Selected payment method ({what})"),
        connector: "revpay",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Use a card, FPX personal banking, DuitNow QR, or a supported wallet (Alipay, Alipay+, WeChat Pay, Touch 'n Go, GrabPay, Boost)"
                    .to_string(),
            ),
            doc_url: None,
            additional_context: Some(context_detail.to_string()),
        },
    })
}

/// Explicit networks choose the dedicated RevPay payment rail. MyDebit must
/// be explicit: its co-badged BIN ranges cannot identify the intended network.
fn card_payment_details(network: Option<&CardNetwork>, pan: &str) -> (u16, Option<RevpayCardType>) {
    match network {
        Some(CardNetwork::AmericanExpress) => (AMEX_PAYMENT_ID, Some(RevpayCardType::Amex)),
        Some(CardNetwork::DinersClub) => (DINERS_PAYMENT_ID, None),
        Some(CardNetwork::Visa) => (CARD_PAYMENT_ID, Some(RevpayCardType::Visa)),
        Some(CardNetwork::Mastercard) => (CARD_PAYMENT_ID, Some(RevpayCardType::Mastercard)),
        Some(CardNetwork::MyDebit) => (MYDEBIT_PAYMENT_ID, Some(RevpayCardType::MyDebit)),
        Some(CardNetwork::UnionPay) => (UNIONPAY_CNP_PAYMENT_ID, Some(RevpayCardType::UnionPay)),
        _ => {
            let card_type = card_type_from_pan(pan);
            let payment_id = if matches!(card_type, Some(RevpayCardType::UnionPay)) {
                UNIONPAY_CNP_PAYMENT_ID
            } else {
                CARD_PAYMENT_ID
            };
            (payment_id, card_type)
        }
    }
}

/// Infer the card type only when no recognized explicit network is supplied.
/// BIN 62 selects UnionPay CNP; an explicit MyDebit network takes precedence.
fn card_type_from_pan(pan: &str) -> Option<RevpayCardType> {
    let digits: String = pan.chars().filter(char::is_ascii_digit).collect();
    let bin2: u32 = digits.get(..2)?.parse().ok()?;
    let bin4: u32 = digits.get(..4)?.parse().ok()?;
    match (bin2, bin4) {
        (34 | 37, _) => Some(RevpayCardType::Amex),
        (51..=55, _) => Some(RevpayCardType::Mastercard),
        (40..=49, _) => Some(RevpayCardType::Visa),
        (62, _) => Some(RevpayCardType::UnionPay),
        (_, 2221..=2720) => Some(RevpayCardType::Mastercard),
        _ => None,
    }
}

/// revPAY `Funding_Exp_Date`: strictly `YYYYMM`, six digits, year first (doc
/// §2.1 field 24; format confirmed by the UAT gateway, e.g. `"203901"` = Jan
/// 2039). Composed from the shared expiry helpers — `get_expiry_year_4_digit`
/// expands `YY`, `get_card_expiry_month_2_digit` validates and zero-pads the
/// month (the same building blocks jpmorganorbital uses for its fixed 6-char
/// `ccExp`). The composed `get_expiry_date_as_yyyymm` is NOT usable here: it
/// interpolates the raw month and year unvalidated, and the year helper
/// passes any non-2-char input through unchanged — so the joined value is
/// shape-guarded below rather than forwarded: a malformed value reaches
/// revPAY as `Response_Code 14`.
fn revpay_funding_exp_date<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let exp = format!(
        "{}{}",
        card.get_expiry_year_4_digit().peek(),
        card.get_card_expiry_month_2_digit()?.peek(),
    );
    if exp.len() == 6 && exp.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(Secret::new(exp))
    } else {
        Err(error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.card.card_exp_year",
            context: IntegrationErrorContext {
                additional_context: Some("Expected year format: YY or YYYY".to_string()),
                ..Default::default()
            },
        }))
    }
}

/// `Card_Issuer_Bank_Country_Code` (doc §2.1 field 26): ISO 3166-1 alpha-3,
/// e.g. `MYS`. Prefers the explicit card issuing country and falls back to
/// the billing country.
fn issuer_country_alpha3(
    card_issuing_country: Option<&str>,
    billing_country: Option<CountryAlpha2>,
) -> Option<String> {
    card_issuing_country
        .map(str::trim)
        .filter(|country| country.len() == 3)
        .map(str::to_uppercase)
        .or_else(|| {
            billing_country.map(|country| CountryAlpha2::from_alpha2_to_alpha3(country).to_string())
        })
}

/// Payment_ID = 15 (Alipay Online, Web) — spec §2.1 / Appendix A.
const ALIPAY_PAYMENT_ID: u16 = 15;
/// Payment_ID = 14 (WeChat Pay) — spec §2.1 / Appendix A.
const WECHAT_PAYMENT_ID: u16 = 14;
/// Payment_ID = 28 (Touch 'n Go eWallet) — spec §2.1 / Appendix A.
const TOUCHNGO_PAYMENT_ID: u16 = 28;
/// Payment_ID = 17 (GrabPay) — spec §2.1 / Appendix A.
const GRABPAY_PAYMENT_ID: u16 = 17;
/// Merchant API 3.1.10, Appendix A.
const BOOST_PAYMENT_ID: u16 = 11;
const FPX_PAYMENT_ID: u16 = 3;
const ALIPAY_PLUS_PAYMENT_ID: u16 = 36;
const DUITNOW_QR_PAYMENT_ID: u16 = 37;

/// FPX B2C bank codes from Merchant API 3.1.10, Appendix B.
/// Maybank is the consumer Maybank2u channel, not corporate M2E.
fn fpx_bank_code(
    issuer: &BankNames,
) -> Result<&'static str, error_stack::Report<IntegrationError>> {
    match issuer {
        BankNames::AffinBank => Ok("ABB0233"),
        BankNames::AllianceBank => Ok("ABMB0212"),
        BankNames::AgroBank => Ok("AGRO01"),
        BankNames::AmBank => Ok("AMBB0209"),
        BankNames::BankIslam => Ok("BIMB0340"),
        BankNames::BankMuamalat => Ok("BMMB0341"),
        BankNames::BankRakyat => Ok("BKRM0602"),
        BankNames::BankSimpananNasional => Ok("BSN0601"),
        BankNames::CimbBank => Ok("BCBB0235"),
        BankNames::HongLeongBank => Ok("HLB0224"),
        BankNames::HsbcBank => Ok("HSBC0223"),
        BankNames::KuwaitFinanceHouse => Ok("KFH0346"),
        BankNames::Maybank => Ok("MB2U0227"),
        BankNames::OcbcBank => Ok("OCBC0229"),
        BankNames::PublicBank => Ok("PBB0233"),
        BankNames::RhbBank => Ok("RHB0218"),
        BankNames::StandardCharteredBank => Ok("SCB0216"),
        BankNames::UobBank => Ok("UOB0226"),
        BankNames::BankOfChina => Ok("BOCM01"),
        _ => Err(unsupported_payment_method(
            "FPX issuer",
            "Select a supported Malaysian FPX B2C bank",
        )),
    }
}

/// revPAY `Source_Of_Funds` value for raw-PAN card payments (spec §2.1,
/// Appendix O; gateway-token payments would use `TOKEN`, unsupported here).
const SOURCE_OF_FUNDS_CARD: &str = "CARD";

/// Loopback placeholder for `Customer_IP`: loopback is unambiguously
/// non-routable and cannot collide with a real shopper address. Same
/// convention as phonepe (`DEFAULT_IP`) and razorpay.
const DEFAULT_CUSTOMER_IP: &str = "127.0.0.1";

/// Wallet rails — Payment_ID only; revPAY needs no wallet credential fields
/// on the request (the shopper completes payment on the wallet's page).
fn wallet_payment_id(wallet: &WalletData) -> Result<u16, error_stack::Report<IntegrationError>> {
    Ok(match wallet {
        // Alipay Online (Web)
        WalletData::AliPayRedirect(_) => ALIPAY_PAYMENT_ID,
        WalletData::WeChatPayRedirect(_) => WECHAT_PAYMENT_ID,
        WalletData::TouchNGoRedirect(_) => TOUCHNGO_PAYMENT_ID,
        WalletData::GrabpayRedirect {} => GRABPAY_PAYMENT_ID,
        WalletData::BoostRedirect {} => BOOST_PAYMENT_ID,
        WalletData::AliPayPlusRedirect {} => ALIPAY_PLUS_PAYMENT_ID,
        // revPAY wallets with no UCS WalletData variant today (Appendix A):
        // Merchantrade 10, Mae 12, Vcash 13, AirAsia Big 16, ShopeePay 33.
        _ => {
            return Err(unsupported_payment_method(
                "wallet",
                "UCS exposes no revPAY rail for this wallet; revPAY supports Alipay, \
                 WeChat Pay, GrabPay, Touch 'n Go, Boost and Alipay+ plus wallets that \
                 have no UCS WalletData variant today (Merchantrade, Mae, Vcash, \
                 AirAsia Big, ShopeePay)",
            ));
        }
    })
}

/// Resource id for an authorize outcome: revPAY assigns `Transaction_ID` only
/// once a redirect completes — the "09 Request in progress" response carries
/// `Transaction_ID: null` (captured from the UAT sandbox, 22 Oct 2026) — so
/// fall back to our `Reference_Number`, which is also what the `/v1/inquiry`
/// sync correlates on (PSync later overwrites the resource id with the real
/// `Transaction_ID`), and fail only when the response carries neither
/// identifier.
fn authorize_resource_id(
    response: &RevpayPaymentsResponse,
    http_code: u16,
) -> Result<ResponseId, error_stack::Report<ConnectorError>> {
    let transaction_id = response
        .transaction_id
        .clone()
        .or_else(|| response.reference_number.clone())
        .ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                http_code,
                "revpay: success/redirect response missing Transaction_ID \
                 and Reference_Number; confirm API contract.",
            )
        })?;
    Ok(ResponseId::ConnectorTransactionId(transaction_id))
}

// =============================================================================
// REQUEST TYPES
// =============================================================================

#[derive(Debug, Serialize)]
pub struct RevpayCardDetails<T: PaymentMethodDataTypes> {
    /// `CARD` — raw PAN, per revPAY's `Source_Of_Funds` contract.
    #[serde(rename = "Source_Of_Funds")]
    pub source_of_funds: &'static str,
    #[serde(rename = "Card_Type", skip_serializing_if = "Option::is_none")]
    pub card_type: Option<RevpayCardType>,
    #[serde(rename = "Card_Holder_Name", skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    #[serde(rename = "Funding_Pan")]
    pub funding_pan: RawCardNumber<T>,
    /// `YYYYMM` — year first, zero-padded two-digit month.
    #[serde(rename = "Funding_Exp_Date")]
    pub funding_exp_date: Secret<String>,
    #[serde(rename = "Funding_CVV")]
    pub funding_cvv: Secret<String>,
    /// ISO 3166-1 alpha-3 issuer country (doc §2.1 field 26), e.g. `MYS`.
    #[serde(
        rename = "Card_Issuer_Bank_Country_Code",
        skip_serializing_if = "Option::is_none"
    )]
    pub card_issuer_bank_country_code: Option<String>,
    /// Mandatory when Payment_ID = 2 (revPAY field table).
    #[serde(rename = "Customer_Email", skip_serializing_if = "Option::is_none")]
    pub customer_email: Option<Email>,
    /// Mandatory when Payment_ID = 2 (revPAY field table), format `+6012…`.
    #[serde(rename = "Customer_Contact", skip_serializing_if = "Option::is_none")]
    pub customer_contact: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct RevpayPaymentsRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "Revpay_Merchant_ID")]
    pub revpay_merchant_id: String,
    #[serde(rename = "Payment_ID")]
    pub payment_id: u16,
    #[serde(rename = "Bank_Code", skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<&'static str>,
    #[serde(rename = "Reference_Number")]
    pub reference_number: String,
    /// The exact two-decimal string used in the signature preimage.
    #[serde(rename = "Amount")]
    pub amount: StringMajorUnitWithTwoDecimals,
    /// Typed enum — serde renders the ISO alpha-3 code (`"MYR"`).
    #[serde(rename = "Currency")]
    pub currency: Currency,
    #[serde(rename = "Transaction_Description")]
    pub transaction_description: String,
    #[serde(rename = "Customer_ID", skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(rename = "Customer_Name", skip_serializing_if = "Option::is_none")]
    pub customer_name: Option<String>,
    #[serde(rename = "Customer_IP")]
    pub customer_ip: String,
    #[serde(rename = "Return_URL")]
    pub return_url: String,
    #[serde(rename = "Backend_URL", skip_serializing_if = "Option::is_none")]
    pub backend_url: Option<String>,
    #[serde(rename = "Key_Index")]
    pub key_index: u32,
    #[serde(rename = "Signature")]
    pub signature: String,
    /// Card-side fields are flattened in; omitted for wallets, FPX and DuitNow QR.
    #[serde(flatten)]
    pub payment_method_fields: Option<RevpayCardDetails<T>>,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for RevpayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = RevpayAuthType::try_from(&item.connector_config)?;

        // Key_Index is Int(3) on the wire; it is configured as a string.
        let key_index = auth.key_index.peek().trim().parse::<u32>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_config.api_secret (revPAY Key_Index)",
                context: IntegrationErrorContext {
                    additional_context: Some("expected a decimal integer, e.g. \"1\"".to_string()),
                    ..Default::default()
                },
            })
        })?;

        let currency = item.request.currency;
        let currency_str = currency.to_string();
        let amount = RevpayAmountConvertor::convert(&item.request.amount)?
            .validate_unsigned("Amount")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "request.amount",
                context: IntegrationErrorContext::default(),
            })?;

        let reference_number = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Signature = sha512(MerchantKey + Revpay_Merchant_ID + Reference_Number
        // + Amount + Currency) — the SAME Amount string that goes in the body.
        let signature = generate_revpay_signature(
            auth.merchant_key.peek(),
            &[
                auth.merchant_id.peek(),
                &reference_number,
                amount.get_amount_as_string(),
                &currency_str,
            ],
        );

        // Return_URL is mandatory in revPAY's Payment field table.
        let return_url = item.request.router_return_url.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "router_return_url",
                context: IntegrationErrorContext::default(),
            })
        })?;

        // spec §2.1 field #18: Customer_IP is Mandatory on every payment request.
        // When the shopper's browser info is absent (e.g. server-side flows),
        // substitute the loopback placeholder rather than failing the payment
        // (same exemption UCS applies to other IP-mandating connectors).
        let customer_ip = item
            .request
            .browser_info
            .as_ref()
            .and_then(|info| info.ip_address)
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| DEFAULT_CUSTOMER_IP.to_string());

        // spec §2.1 field #7: Transaction_Description is Mandatory; when the
        // merchant supplies none — or a blank string (euler sends
        // `description: ""`) — fall back to the unique order reference so
        // the merchant-portal report row stays meaningful. revPAY rejects an
        // empty value with HTTP 422 "Unprocessable Entity" (verified against
        // the UAT sandbox), which is not documented in the 3.1.10 spec.
        let transaction_description = item
            .resource_common_data
            .description
            .clone()
            .filter(|description| !description.trim().is_empty())
            .unwrap_or_else(|| format!("Payment {reference_number}"));

        let mut bank_code = None;
        let (payment_id, payment_method_fields) = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let (payment_id, card_type) = card_payment_details(
                    card_data.card_network.as_ref(),
                    card_data.card_number.peek(),
                );

                let customer_email = item
                    .request
                    .email
                    .clone()
                    .or_else(|| item.resource_common_data.get_optional_billing_email());
                // revPAY wants "+<cc><number>". Euler forwards the phone number
                // without a country code (UCSTransforms.hs maps
                // phoneCountryCode = Nothing), and get_number_with_country_code
                // errors without one — so fall back to the raw number rather
                // than reporting the phone as missing.
                let customer_contact = item
                    .resource_common_data
                    .get_billing_phone_number()
                    .ok()
                    .or_else(|| {
                        item.resource_common_data
                            .get_optional_billing_phone_number()
                    });

                if payment_id == CARD_PAYMENT_ID {
                    if customer_email.is_none() {
                        return Err(error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "email",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "revPAY marks Customer_Email mandatory for Payment_ID = 2 (Visa/Mastercard rail)"
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        }));
                    }
                    if customer_contact.is_none() {
                        return Err(error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "billing.phone",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "revPAY marks Customer_Contact mandatory for Payment_ID = 2 (Visa/Mastercard rail)"
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        }));
                    }
                }

                (
                    payment_id,
                    Some(RevpayCardDetails {
                        source_of_funds: SOURCE_OF_FUNDS_CARD,
                        card_type,
                        card_holder_name: card_data.card_holder_name.clone(),
                        funding_pan: card_data.card_number.clone(),
                        funding_exp_date: revpay_funding_exp_date(card_data)?,
                        funding_cvv: card_data.card_cvc.clone(),
                        card_issuer_bank_country_code: issuer_country_alpha3(
                            card_data.card_issuing_country.as_deref(),
                            item.resource_common_data.get_optional_billing_country(),
                        ),
                        customer_email,
                        customer_contact,
                    }),
                )
            }
            PaymentMethodData::Wallet(wallet_data) => (wallet_payment_id(wallet_data)?, None),
            // The Merchant API uses Bank_Redirect_URL to open RevPay's hosted
            // payment page. DuitNow's QR is displayed there, not synthesized here.
            PaymentMethodData::RealTimePayment(data)
                if matches!(data.as_ref(), RealTimePaymentData::DuitNow {}) =>
            {
                (DUITNOW_QR_PAYMENT_ID, None)
            }
            PaymentMethodData::BankRedirect(BankRedirectData::OnlineBankingFpx { issuer }) => {
                bank_code = Some(fpx_bank_code(issuer)?);
                (FPX_PAYMENT_ID, None)
            }
            _ => {
                return Err(unsupported_payment_method(
                    "other",
                    "revPAY Authorize supports cards, FPX personal banking, DuitNow QR and mapped redirect wallets; \
                     other bank redirects and QR payment methods are unsupported",
                ));
            }
        };

        Ok(Self {
            revpay_merchant_id: auth.merchant_id.peek().to_string(),
            payment_id,
            bank_code,
            reference_number,
            amount,
            currency,
            transaction_description,
            customer_id: item
                .request
                .customer_id
                .as_ref()
                .map(|customer| customer.get_string_repr().to_string()),
            customer_name: item.request.customer_name.clone().or_else(|| {
                // revPAY's gateway sample sends `Customer_Name`; fall back to
                // the billing name when the request carries none.
                let name = [
                    item.resource_common_data.get_optional_billing_first_name(),
                    item.resource_common_data.get_optional_billing_last_name(),
                ]
                .into_iter()
                .flatten()
                .map(|part| part.peek().trim().to_string())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
                (!name.is_empty()).then_some(name)
            }),
            customer_ip,
            return_url,
            backend_url: item.request.webhook_url.clone(),
            key_index,
            signature,
            payment_method_fields,
        })
    }
}

// Bridge between the macro-generated `RevpayRouterData` wrapper and the
// `TryFrom<&RouterDataV2<..>>` transform above (same idiom as travelhub).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RevpayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: RevpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

// =============================================================================
// RESPONSE TYPES
// =============================================================================

/// revPAY may echo `Amount`/`Currency`/dates either as JSON strings (matching
/// what we sent) or as bare JSON numbers (as in the doc samples). `Number`
/// exists only to parse lenient response and webhook echoes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RevpayAmount {
    Text(String),
    Number(f64),
}

impl RevpayAmount {
    /// Normalised two-decimal-major rendering, ready for `parse_revpay_amount`.
    fn as_two_decimal_string(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Number(number) => format!("{number:.2}"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RevpayPaymentsResponse {
    #[serde(rename = "Revpay_Merchant_ID", skip_serializing_if = "Option::is_none")]
    pub revpay_merchant_id: Option<String>,
    #[serde(rename = "Payment_ID", skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<u16>,
    #[serde(rename = "Transaction_ID", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(rename = "Reference_Number", skip_serializing_if = "Option::is_none")]
    pub reference_number: Option<String>,
    #[serde(rename = "Amount", skip_serializing_if = "Option::is_none")]
    pub amount: Option<RevpayAmount>,
    #[serde(rename = "Currency", skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(rename = "Response_Code")]
    pub response_code: RevpayResponseCode,
    #[serde(rename = "Error_Description", skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
    #[serde(rename = "Bank_Redirect_URL", skip_serializing_if = "Option::is_none")]
    pub bank_redirect_url: Option<String>,
    #[serde(rename = "Key_Index", skip_serializing_if = "Option::is_none")]
    pub key_index: Option<i64>,
    // revPAY echoes `Key_Index` + `Signature` on responses. Incoming
    // notifications verify them in `IncomingWebhook::verify_webhook_source`
    // using sha512(key + mid + Transaction_ID + Response_Code +
    // Reference_Number + Amount + Currency). Verification is intentionally not
    // duplicated for initial Payment responses here: a "09" redirect response
    // can omit Transaction_ID, which is part of that response preimage.
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(
        rename = "Bank_Reference_Number",
        skip_serializing_if = "Option::is_none"
    )]
    pub bank_reference_number: Option<String>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<RevpayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RevpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        // Single decision point for the authorize outcome — used unchanged on
        // both the success and the in-band-decline branches.
        let status = authorize_attempt_status(
            &response.response_code,
            response.bank_redirect_url.as_deref(),
        );

        // IN-BAND 2xx FAILURE.
        // revPAY answers EVERYTHING (declines included) with HTTP 200; success
        // vs declination lives in the body `Response_Code`. Only `00` (sale
        // charged) and `09` (redirect, needs `Bank_Redirect_URL`) are
        // non-error outcomes — every other code is a failed attempt and must
        // land in `Err(ErrorResponse)` here, because `build_error_response`
        // never sees a 200 on this connector. Both halves of the enum are
        // matched as named arms (no `_ =>` at the code level).
        let in_band_failure_code: Option<&str> = match (
            &response.response_code,
            response.bank_redirect_url.as_deref(),
        ) {
            (RevpayResponseCode::Success, _) => None,
            (RevpayResponseCode::RedirectRequired, Some(_)) => None,
            // FPX B2B settles asynchronously — pending, not an error.
            (RevpayResponseCode::PendingAuthorization, _) => None,
            // A 09 without its redirect URL cannot complete the payment.
            (code @ RevpayResponseCode::RedirectRequired, None) => Some(code.as_str()),
            // "76" here: the gateway cannot find the payment being authorized.
            (code @ RevpayResponseCode::TransactionNotFound, _) => Some(code.as_str()),
            (code @ RevpayResponseCode::Unknown(_), _) => Some(code.as_str()),
        };

        if let Some(raw_code) = in_band_failure_code {
            let message = response.error_description.clone().unwrap_or_else(|| {
                if matches!(response.response_code, RevpayResponseCode::RedirectRequired) {
                    "revPAY returned Response_Code 09 without Bank_Redirect_URL".to_string()
                } else {
                    NO_ERROR_MESSAGE.to_string()
                }
            });
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: raw_code.to_string(),
                    message,
                    reason: response.error_description.clone(),
                    // Terminal here is correct: the decline state came from the
                    // connector's own body and revPAY does not re-drive a
                    // failed Payment API attempt. Do NOT copy this into
                    // `build_error_response`, which is shared by every flow.
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.transaction_id.clone(),
                    ..Default::default()
                }),
                ..router_data.clone()
            });
        }

        // revPAY's "09 Request in progress" (redirect) response carries
        // `Transaction_ID: null` — the id is only assigned once the shopper
        // completes the payment (captured from the UAT sandbox, 22 Oct 2026).
        // See `authorize_resource_id` for the interim-identifier contract.
        let resource_id = authorize_resource_id(response, item.http_code)?;

        // revPAY echoes Amount/Currency on the payment response — populate the
        // integrity object from the echoed values so the framework detects
        // tampering between request build and response handling. Amount is
        // mandatory in the doc; fall back to the request value only when the
        // connector genuinely omits it.
        let integrity_amount = match &response.amount {
            Some(amount) => parse_revpay_amount(
                &amount.as_two_decimal_string(),
                router_data.request.currency,
            )
            .change_context(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "revpay",
            ))
            .attach_printable("revpay: echoed Amount is not in revPAY's 2-decimal form")?,
            None => router_data.request.amount.amount,
        };
        let integrity_currency = response
            .currency
            .as_deref()
            .and_then(|echoed| Currency::from_str(echoed).ok())
            .unwrap_or(router_data.request.currency);

        // 09 → shopper must open Bank_Redirect_URL with a plain HTTP GET;
        // RedirectForm::Uri navigates callers directly to the URL as-is.
        let redirection_data = response
            .bank_redirect_url
            .clone()
            .map(|url| Box::new(RedirectForm::Uri { uri: url }));

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data,
            connector_metadata: None,
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.reference_number.clone(),
            incremental_authorization_allowed: None,
            splits: None,
            status_code: item.http_code,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            request: PaymentsAuthorizeData {
                integrity_object: Some(AuthoriseIntegrityObject {
                    amount: integrity_amount,
                    currency: integrity_currency,
                }),
                ..router_data.request.clone()
            },
            ..router_data.clone()
        })
    }
}

// =============================================================================
// PSYNC TYPES — POST {base}/v1/inquiry (§2.2)
// =============================================================================

/// The inquiry response is the very same wire shape as the Payment response
/// (§2.2: "`Transaction_ID`, `Response_Code`, `Error_Description`,
/// `Key_Index`, `Signature`, … — see Flow 1 table"); only the status mapping
/// and in-band handling differ, so it reuses `RevpayPaymentsResponse`. The
/// alias exists so the two flows get distinct idents in the macro-generated
/// templating layer.
pub type RevpayPSyncResponse = RevpayPaymentsResponse;

/// Payment Status Inquiry request. revPAY's inquiry has NO
/// query-by-transaction-id input: it resolves only the merchant reference the
/// Payment call was signed with, so `Reference_Number` must be byte-identical
/// to the value sent on Payment.
#[derive(Debug, Serialize)]
pub struct RevpayPSyncRequest {
    #[serde(rename = "Revpay_Merchant_ID")]
    pub revpay_merchant_id: String,
    #[serde(rename = "Reference_Number")]
    pub reference_number: String,
    /// The ORIGINAL payment amount in the same 2-decimal form signed at
    /// Payment time — the doc requires the inquiry to re-sign it.
    #[serde(rename = "Amount")]
    pub amount: StringMajorUnitWithTwoDecimals,
    /// Typed enum — serde renders the ISO alpha-3 code (`"MYR"`).
    #[serde(rename = "Currency")]
    pub currency: Currency,
    #[serde(rename = "Key_Index")]
    pub key_index: u32,
    #[serde(rename = "Signature")]
    pub signature: String,
}

impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for RevpayPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = RevpayAuthType::try_from(&item.connector_config)?;

        // Key_Index is Int(3) on the wire; it is configured as a string.
        let key_index = auth.key_index.peek().trim().parse::<u32>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_config.api_secret (revPAY Key_Index)",
                context: IntegrationErrorContext {
                    additional_context: Some("expected a decimal integer, e.g. \"1\"".to_string()),
                    ..Default::default()
                },
            })
        })?;

        // PRE-FLIGHT GATE: the inquiry Reference_Number must be the exact
        // merchant reference the Authorize flow sent, carried into PSync by
        // `connector_request_reference_id`. NEVER re-derive it from
        // `connector_transaction_id` (revPAY only mints Transaction_ID in its
        // Payment response, and the inquiry cannot be keyed by it) and NEVER
        // fall back to a default — an empty reference resolves nothing and
        // the payment would be miss-reported as not found. Ensure the original
        // payment carried connector_request_reference_id before syncing.
        let reference_number = item
            .resource_common_data
            .connector_request_reference_id
            .clone();
        if reference_number.trim().is_empty() {
            return Err(missing_field_err("connector_request_reference_id")());
        }

        // The inquiry must re-send the ORIGINAL payment amount with the same
        // 2-decimal encoding that was signed at Payment time.
        // `PaymentsSyncData.amount` is mandatory in the framework's
        // PaymentServiceGet → PaymentsSyncData conversion, so the exact
        // authorize-time figure (and currency) is available here without any
        // reconstruction.
        let currency = item.request.currency;
        let currency_str = currency.to_string();
        let amount = RevpayAmountConvertor::convert(&item.request.amount)?
            .validate_unsigned("Amount")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "request.amount",
                context: IntegrationErrorContext::default(),
            })?;

        // Signature = sha512(MerchantKey + Revpay_Merchant_ID +
        // Reference_Number + Amount + Currency) — the same formula as the
        // Payment call (§3.4), over the same byte values.
        let signature = generate_revpay_signature(
            auth.merchant_key.peek(),
            &[
                auth.merchant_id.peek(),
                &reference_number,
                amount.get_amount_as_string(),
                &currency_str,
            ],
        );

        Ok(Self {
            revpay_merchant_id: auth.merchant_id.peek().to_string(),
            reference_number,
            amount,
            currency,
            key_index,
            signature,
        })
    }
}

// Bridge between the macro-generated `RevpayRouterData` wrapper and the
// `TryFrom<&RouterDataV2<..>>` transform above (same idiom as Authorize).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevpayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for RevpayPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: RevpayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl TryFrom<ResponseRouterData<RevpayPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RevpayPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // INQUIRY SEMANTICS: PSync is a status query and revPAY answers every
        // inquiry with HTTP 200, so the outcome is REPORTED through the
        // attempt status — never returned as `Err(ErrorResponse)`. "00" →
        // Charged, "09"/"99" → Pending, every other code (including "76"
        // Transaction not found) → Failure; the raw code and description ride
        // along on `raw_connector_status`, and non-2xx transport faults are
        // still handled by the shared `build_error_response`.
        let status = response.response_code.psync_status();

        let resource_id = match response.transaction_id.clone() {
            Some(transaction_id) => ResponseId::ConnectorTransactionId(transaction_id),
            // revPAY omits Transaction_ID when the reference resolves to
            // nothing (e.g. "76"); mirror the request-side id the sync was
            // keyed by instead of manufacturing a new identifier.
            None => router_data.request.connector_transaction_id.clone(),
        };

        // Identifier round-trip: prefer the Reference_Number revPAY echoes;
        // the fallback is the very reference the inquiry was keyed by, so the
        // caller can always correlate the sync with the original attempt.
        let connector_response_reference_id = response.reference_number.clone().or_else(|| {
            Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            )
        });

        // Identifier carrier chain: re-publish whatever metadata arrived on
        // the sync request so later flows keep access to it.
        let connector_metadata = router_data
            .request
            .connector_feature_data
            .clone()
            .map(|meta| meta.expose());

        // Integrity object: the inquiry echoes the AUTHORIZE-side Amount /
        // Currency, so prefer the echoed values (parsed back through the same
        // 2-decimal contract). The request-side figures are the fallback for
        // the one case revPAY genuinely omits the echo — an unresolved
        // reference ("76"), whose body carries no Amount/Currency.
        let integrity_currency = response
            .currency
            .as_deref()
            .and_then(|echoed| Currency::from_str(echoed).ok())
            .unwrap_or(router_data.request.currency);
        let integrity_amount = match &response.amount {
            Some(amount) => {
                parse_revpay_amount(&amount.as_two_decimal_string(), integrity_currency)
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "revpay",
                    ))
                    .attach_printable(
                        "revpay: inquiry echoed Amount is not in revPAY's 2-decimal form",
                    )?
            }
            None => router_data.request.amount.amount,
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data: None,
            connector_metadata,
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id,
            incremental_authorization_allowed: None,
            splits: None,
            status_code: item.http_code,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(RawConnectorStatus {
                    code: Some(response.response_code.as_str().to_string()),
                    message: Some(
                        response
                            .error_description
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    ),
                    reason: response.error_description.clone(),
                }),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            request: PaymentsSyncData {
                integrity_object: Some(PaymentSynIntegrityObject {
                    amount: integrity_amount,
                    currency: integrity_currency,
                }),
                ..router_data.request.clone()
            },
            ..router_data.clone()
        })
    }
}

// =============================================================================
// REFUND TYPES — POST {base}/v1/refund (§2.5), synchronous accept/reject
// =============================================================================

/// revPAY refunds the ORIGINAL payment by `Original_Reference_Number` — the
/// Reference_Number signed on the Payment call — and identifies THIS refund
/// with a NEW `Reference_Number` (UCS `refund_id`).
///
/// `RefundFlowData::connector_request_reference_id` cannot serve as the
/// original reference: the framework derives it from `merchant_refund_id`, so
/// it identifies the refund, not the payment. The original payment's Reference
/// is taken from `RefundsData::connector_order_id` ("connector-side identifier
/// for the original payment that this refund targets" — same idiom as
/// travelhub). Absent => refuse locally rather than sending revPAY a reference
/// that can never match a payment.
fn resolve_original_reference_number(
    connector_order_id: Option<&str>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    connector_order_id
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the original Authorize call's merchant reference (the \
                         connector_request_reference_id that revPAY signed as \
                         `Reference_Number`) as `connector_order_id` on the refund request."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "revPAY refunds a payment by `Original_Reference_Number` alone, the \
                         only original-payment identifier the refund body carries. \
                         `refund_id` identifies this refund call and cannot locate the \
                         original payment."
                            .to_string(),
                    ),
                },
            })
        })
}

#[derive(Debug, Serialize)]
pub struct RevpayRefundRequest {
    #[serde(rename = "Revpay_Merchant_ID")]
    pub revpay_merchant_id: String,
    /// NEW unique reference for this refund — UCS `refund_id`.
    #[serde(rename = "Reference_Number")]
    pub reference_number: String,
    #[serde(rename = "Original_Reference_Number")]
    pub original_reference_number: String,
    /// The exact two-decimal string used in the signature preimage.
    #[serde(rename = "Refund_Amount")]
    pub refund_amount: StringMajorUnitWithTwoDecimals,
    #[serde(rename = "Key_Index")]
    pub key_index: u32,
    #[serde(rename = "Signature")]
    pub signature: String,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for RevpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = RevpayAuthType::try_from(&item.connector_config)?;

        // Key_Index is Int(3) on the wire; it is configured as a string.
        let key_index = auth.key_index.peek().trim().parse::<u32>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_config.api_secret (revPAY Key_Index)",
                context: IntegrationErrorContext {
                    additional_context: Some("expected a decimal integer, e.g. \"1\"".to_string()),
                    ..Default::default()
                },
            })
        })?;

        // Reference_Number is a NEW reference identifying this refund —
        // revPAY rejects a refund that duplicates or blanks a previous
        // reference, so an empty `refund_id` fails here rather than at the
        // gateway.
        let reference_number = item.request.refund_id.clone();
        if reference_number.trim().is_empty() {
            return Err(missing_field_err("refund_id")());
        }

        let original_reference_number =
            resolve_original_reference_number(item.request.connector_order_id.as_deref())?;

        // Refund_Amount in the exact 2-decimal form that is also signed;
        // partial refunds are supported by the API (§2.5).
        let refund_amount = RevpayAmountConvertor::convert(&item.request.refund_amount)?
            .validate_unsigned("Refund_Amount")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "request.refund_amount",
                context: IntegrationErrorContext::default(),
            })?;

        // Signature = sha512(MerchantKey + Revpay_Merchant_ID + Reference_Number
        // + Refund_Amount + Original_Reference_Number) (§3.10) — DIFFERENT field
        // order from the Payment formula (no Currency; Original_Reference_Number
        // appended). Verified against a self-computed vector in tests below.
        let signature = generate_revpay_signature(
            auth.merchant_key.peek(),
            &[
                auth.merchant_id.peek(),
                &reference_number,
                refund_amount.get_amount_as_string(),
                &original_reference_number,
            ],
        );

        Ok(Self {
            revpay_merchant_id: auth.merchant_id.peek().to_string(),
            reference_number,
            original_reference_number,
            refund_amount,
            key_index,
            signature,
        })
    }
}

// Bridge between the macro-generated `RevpayRouterData` wrapper and the
// `TryFrom<&RouterDataV2<..>>` transform above (same idiom as Authorize).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevpayRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for RevpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: RevpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RevpayRefundResponse {
    #[serde(rename = "Revpay_Merchant_ID", skip_serializing_if = "Option::is_none")]
    pub revpay_merchant_id: Option<String>,
    /// No separate refund id exists in the doc — this becomes `connector_refund_id`.
    #[serde(rename = "Transaction_ID", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    /// Echoed refund-side Reference_Number.
    #[serde(rename = "Reference_Number", skip_serializing_if = "Option::is_none")]
    pub reference_number: Option<String>,
    #[serde(
        rename = "Original_Reference_Number",
        skip_serializing_if = "Option::is_none"
    )]
    pub original_reference_number: Option<String>,
    #[serde(rename = "Refund_Amount", skip_serializing_if = "Option::is_none")]
    pub refund_amount: Option<RevpayAmount>,
    /// Remaining refundable balance — informational only, not mapped onward.
    #[serde(rename = "Amount_Balance", skip_serializing_if = "Option::is_none")]
    pub amount_balance: Option<RevpayAmount>,
    #[serde(rename = "Response_Code")]
    pub response_code: RevpayResponseCode,
    #[serde(rename = "Error_Description", skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
    #[serde(rename = "Key_Index", skip_serializing_if = "Option::is_none")]
    pub key_index: Option<i64>,
    // §3.11 synchronously signed responses use sha512(key + mid +
    // Transaction_ID + Response_Code + Reference_Number + Refund_Amount). Keep
    // the signature in the wire model; this transformer only maps the refund
    // outcome and does not perform synchronous-response signature verification.
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl TryFrom<ResponseRouterData<RevpayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RevpayRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // IN-BAND 2xx FAILURE. revPAY answers EVERY refund (declines included)
        // with HTTP 200; only "00" is acceptance. Every other code must land in
        // `Err(ErrorResponse)` here, because `build_error_response` never sees a
        // 200 on this connector. Terminal FlowStatus::Refund(Failure) is correct:
        // the decline state came from the connector's own body and the Refund
        // API is synchronous — it never re-drives a declined refund.
        if !matches!(response.response_code, RevpayResponseCode::Success) {
            let raw_code = response.response_code.as_str();
            return Ok(Self {
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: if raw_code.trim().is_empty() {
                        NO_ERROR_CODE.to_string()
                    } else {
                        raw_code.to_string()
                    },
                    message: response
                        .error_description
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.error_description.clone(),
                    attempt_status: Some(FlowStatus::Refund(
                        response.response_code.refund_status(),
                    )),
                    connector_transaction_id: response.transaction_id.clone(),
                    ..Default::default()
                }),
                ..router_data.clone()
            });
        }

        let refund_status = response.response_code.refund_status();

        let connector_refund_id = response.transaction_id.clone().ok_or_else(|| {
            crate::utils::response_deserialization_fail(
                item.http_code,
                "revpay: successful refund response missing Transaction_ID; confirm API contract.",
            )
        })?;

        // Integrity object (refund_amount, currency). revPAY's refund response
        // carries NO Currency field and echoes Refund_Amount, so the amount is
        // parsed from the echo whenever present; currency can only come from
        // the request side (§2.5 refund table has no currency echo).
        let integrity_currency = router_data.request.currency;
        let integrity_amount = match &response.refund_amount {
            Some(amount) => {
                parse_revpay_amount(&amount.as_two_decimal_string(), integrity_currency)
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "revpay",
                    ))
                    .attach_printable(
                        "revpay: echoed Refund_Amount is not in revPAY's 2-decimal form",
                    )?
            }
            None => router_data.request.refund_amount.amount,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            request: RefundsData {
                integrity_object: Some(RefundIntegrityObject {
                    refund_amount: integrity_amount,
                    currency: integrity_currency,
                }),
                ..router_data.request.clone()
            },
            ..router_data.clone()
        })
    }
}

// =============================================================================
// RSYNC TYPES — POST {base}/v1/inquiry (Flow 4, re-uses the payment inquiry)
// =============================================================================

/// FLOW 4 / PRODUCT DECISION: revPAY 3.1.10 has NO dedicated refund-inquiry
/// endpoint — refund outcomes on the Refund call are synchronous. RSync
/// reuses the PAYMENT inquiry endpoint (`POST /v1/inquiry`, §2.2), keyed on
/// the REFUND's own `Reference_Number` (the `refund_id` the Refund call
/// signed) with `Amount` = the refund amount, signed with the
/// payment/inquiry formula sha512(key + mid + Reference_Number + Amount +
/// Currency). Whether revPAY's inquiry resolves refund references is
/// UNVERIFIED from the vendor doc, so the wire shape and parsing deliberately
/// stay identical to PSync.
///
/// The inquiry response is the same wire shape as the Payment/Inquiry
/// response (§2.2), so the alias trick from PSync is applied once more: a
/// distinct ident for the macro-generated templating layer, one wire struct.
pub type RevpayRSyncResponse = RevpayPaymentsResponse;

/// Refund-status inquiry request — field-for-field the PSync inquiry shape,
/// but each value is sourced from the REFUND side (reference and amount
/// identify the refund, not the original payment). A distinct struct (not
/// another alias) because the TryFrom input differs (`RefundSyncData`).
#[derive(Debug, Serialize)]
pub struct RevpayRSyncRequest {
    #[serde(rename = "Revpay_Merchant_ID")]
    pub revpay_merchant_id: String,
    /// The REFUND's own Reference_Number — the `refund_id` sent as
    /// `Reference_Number` on the Refund call. NOT the original payment's
    /// reference and NOT the connector's Transaction_ID.
    #[serde(rename = "Reference_Number")]
    pub reference_number: String,
    /// The refund amount in the exact 2-decimal form that is also signed.
    #[serde(rename = "Amount")]
    pub amount: StringMajorUnitWithTwoDecimals,
    /// Typed enum — serde renders the ISO alpha-3 code (`"MYR"`).
    #[serde(rename = "Currency")]
    pub currency: Currency,
    #[serde(rename = "Key_Index")]
    pub key_index: u32,
    #[serde(rename = "Signature")]
    pub signature: String,
}

impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for RevpayRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = RevpayAuthType::try_from(&item.connector_config)?;

        // Key_Index is Int(3) on the wire; it is configured as a string.
        let key_index = auth.key_index.peek().trim().parse::<u32>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_config.api_secret (revPAY Key_Index)",
                context: IntegrationErrorContext {
                    additional_context: Some("expected a decimal integer, e.g. \"1\"".to_string()),
                    ..Default::default()
                },
            })
        })?;

        // PRE-FLIGHT GATE: Reference_Number must be the refund's own merchant
        // reference, byte-identical to the one the Refund call signed —
        // revPAY's inquiry resolves only merchant references, not
        // Transaction_IDs. The RSync carrier is
        // `RefundFlowData::connector_request_reference_id` (derived from
        // `merchant_refund_id`, same bytes as `RefundsData::refund_id` at
        // Refund time). NEVER re-derive it from
        // `RefundSyncData::connector_refund_id` — that is revPAY's
        // Transaction_ID, which the inquiry cannot be keyed by — and never
        // default it: an empty reference resolves nothing, so a blank carrier
        // fails here as a missing field rather than at the gateway.
        let reference_number = item
            .resource_common_data
            .connector_request_reference_id
            .clone();
        if reference_number.trim().is_empty() {
            return Err(missing_field_err("connector_request_reference_id")());
        }

        // Amount = the refund amount, signed with the payment/inquiry
        // formula. `RefundSyncData` carries it only as the optional
        // `refund_money`; there is no legal reconstruction available (the
        // original payment amount would sign the wrong figure), so absence is
        // a hard MissingRequiredField rather than a fabrication.
        let refund_money = item.request.refund_money.as_ref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "refund_money",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "revPAY RSync (inquiry re-use) signs the REFUND amount; the framework \
                         did not carry refund amount/currency into the refund-sync request"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Pass the refund amount on the refund-get (RSync) request".to_string(),
                    ),
                    doc_url: None,
                },
            })
        })?;
        let currency = refund_money.currency;
        let currency_str = currency.to_string();
        let amount = RevpayAmountConvertor::convert(refund_money)?
            .validate_unsigned("Amount")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "request.refund_money.amount",
                context: IntegrationErrorContext::default(),
            })?;

        // Signature = sha512(MerchantKey + Revpay_Merchant_ID +
        // Reference_Number + Amount + Currency) — the payment/inquiry formula
        // (§3.4), applied to the refund-side values per the Flow 4 decision.
        let signature = generate_revpay_signature(
            auth.merchant_key.peek(),
            &[
                auth.merchant_id.peek(),
                &reference_number,
                amount.get_amount_as_string(),
                &currency_str,
            ],
        );

        Ok(Self {
            revpay_merchant_id: auth.merchant_id.peek().to_string(),
            reference_number,
            amount,
            currency,
            key_index,
            signature,
        })
    }
}

// Bridge between the macro-generated `RevpayRouterData` wrapper and the
// `TryFrom<&RouterDataV2<..>>` transform above (same idiom as PSync).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RevpayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for RevpayRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: RevpayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl TryFrom<ResponseRouterData<RevpayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<RevpayRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // INQUIRY SEMANTICS (same posture as PSync): RSync is a status query
        // and revPAY answers every inquiry with HTTP 200, so the refund
        // outcome is REPORTED through the refund status — never returned as
        // `Err(ErrorResponse)`. "00" → Success, "09"/"99" → Pending, every
        // other code (including "76" reference not found) → Failure. The raw
        // code/description ride along on `raw_connector_status`; non-2xx
        // transport faults still go through the shared
        // `build_error_response`.
        let refund_status = response.response_code.refund_sync_status();

        // revPAY echoes Transaction_ID when the reference resolves; when the
        // refund reference resolves to nothing the echo is absent — mirror
        // the request-side id instead of manufacturing a new identifier.
        let connector_refund_id = response
            .transaction_id
            .clone()
            .unwrap_or_else(|| router_data.request.connector_refund_id.clone());

        // Integrity object: RSync compares the lookup ids (not amounts), and
        // this inquiry is keyed by a DIFFERENT id shape (the merchant refund
        // reference) than the connector's own Transaction_ID, so the echoed
        // id would be a false-mismatch target — mirror the request-side ids
        // exactly, per the integrity pattern for sync flows.
        let integrity_object = RefundSyncIntegrityObject {
            connector_transaction_id: router_data.request.connector_transaction_id.clone(),
            connector_refund_id: router_data.request.connector_refund_id.clone(),
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(RawConnectorStatus {
                    code: Some(response.response_code.as_str().to_string()),
                    message: Some(
                        response
                            .error_description
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    ),
                    reason: response.error_description.clone(),
                }),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            request: RefundSyncData {
                integrity_object: Some(integrity_object),
                ..router_data.request.clone()
            },
            ..router_data.clone()
        })
    }
}

// =============================================================================
// WEBHOOK TYPES — Backend_URL payment notification (Flow 5, §2.1.3)
// =============================================================================

/// revPAY POSTs the payment result server-to-server to the merchant
/// Backend_URL as `application/x-www-form-urlencoded` (NOT JSON). The form
/// body carries the same field set as the Payment backend response. This is
/// the FINAL status source for redirect/wallet payments that left Authorize
/// in `AuthenticationPending`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevpayWebhookBody {
    #[serde(rename = "Revpay_Merchant_ID")]
    pub revpay_merchant_id: String,
    #[serde(rename = "Payment_ID", skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<u16>,
    #[serde(rename = "Bank_Code", skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<String>,
    #[serde(rename = "Transaction_ID")]
    pub transaction_id: String,
    #[serde(rename = "Reference_Number")]
    pub reference_number: String,
    #[serde(
        rename = "Bank_Reference_Number",
        skip_serializing_if = "Option::is_none"
    )]
    pub bank_reference_number: Option<String>,
    /// The exact 2-decimal wire string — signed verbatim, never reformatted.
    #[serde(rename = "Amount")]
    pub amount: RevpayAmount,
    #[serde(rename = "Currency")]
    pub currency: Currency,
    #[serde(
        rename = "Transaction_Description",
        skip_serializing_if = "Option::is_none"
    )]
    pub transaction_description: Option<String>,
    #[serde(rename = "Fraud_Score", skip_serializing_if = "Option::is_none")]
    pub fraud_score: Option<String>,
    #[serde(rename = "Response_Code")]
    pub response_code: RevpayResponseCode,
    #[serde(rename = "Error_Description", skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
    /// Kept as a string (form bodies are all strings; leading zeros, if any,
    /// must survive) and compared trimmed against the configured index.
    #[serde(rename = "Key_Index")]
    pub key_index: String,
    #[serde(rename = "Signature")]
    pub signature: String,
    #[serde(rename = "Request_Datetime", skip_serializing_if = "Option::is_none")]
    pub request_datetime: Option<String>,
    #[serde(rename = "Response_Datetime", skip_serializing_if = "Option::is_none")]
    pub response_datetime: Option<String>,
}

impl RevpayWebhookBody {
    /// SOURCE VERIFICATION (§3.3, Flow 5): recompute
    /// `lowercase_hex(sha512(MerchantKey + Revpay_Merchant_ID +
    /// Transaction_ID + Response_Code + Reference_Number + Amount +
    /// Currency))` with the configured merchant key over the RECEIVED field
    /// bytes and compare full-string against the received `Signature`
    /// (lower-cased first). The received `Key_Index` must also equal the
    /// configured key index — we hold exactly one merchant key, so an
    /// unknown index can never verify. Returns `false` on ANY mismatch;
    /// never panics.
    pub fn verifies_against(&self, auth: &RevpayAuthType) -> bool {
        if self.key_index.trim() != auth.key_index.peek().trim() {
            return false;
        }
        // Form notifications must retain the signed amount text exactly.
        // A numeric amount has lost its original representation and cannot
        // safely be reconstructed for signature verification.
        let RevpayAmount::Text(amount) = &self.amount else {
            return false;
        };
        let currency = self.currency.to_string();
        let expected = generate_revpay_signature(
            auth.merchant_key.peek(),
            &[
                &self.revpay_merchant_id,
                &self.transaction_id,
                self.response_code.as_str(),
                &self.reference_number,
                amount,
                &currency,
            ],
        );
        expected == self.signature.trim().to_lowercase()
    }

    /// Event interpretation: "00" — the sale settled → success;
    /// "09"/"99" — still in flight at the bank / pending authorization →
    /// processing; every other code → failure.
    pub fn event_type(&self) -> EventType {
        match &self.response_code {
            RevpayResponseCode::Success => EventType::PaymentIntentSuccess,
            RevpayResponseCode::RedirectRequired | RevpayResponseCode::PendingAuthorization => {
                EventType::PaymentIntentProcessing
            }
            RevpayResponseCode::TransactionNotFound | RevpayResponseCode::Unknown(_) => {
                EventType::PaymentIntentFailure
            }
        }
    }

    /// Attempt-status interpretation, via the SAME payment status mapping
    /// helper the Authorize flow uses (`From<&RevpayResponseCode> for
    /// AttemptStatus`): "00" → Charged, "09" → AuthenticationPending,
    /// "99" → Pending, everything else → Failure.
    pub fn attempt_status(&self) -> AttemptStatus {
        AttemptStatus::from(&self.response_code)
    }

    /// `(code, message)` for a terminal failure webhook, or `None` for the
    /// non-failure codes ("00"/"09"/"99"). The raw `Response_Code` is the
    /// code; `Error_Description` falls back to NO_ERROR_MESSAGE.
    pub fn failure_details(&self) -> Option<(String, String)> {
        match &self.response_code {
            RevpayResponseCode::Success
            | RevpayResponseCode::RedirectRequired
            | RevpayResponseCode::PendingAuthorization => None,
            RevpayResponseCode::TransactionNotFound | RevpayResponseCode::Unknown(_) => Some((
                self.response_code.as_str().to_string(),
                self.error_description
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            )),
        }
    }
}

// =============================================================================
// ERROR RESPONSE TYPES
// =============================================================================

// Every field is `Option` on purpose: a vendor error body that omits the wire
// fields must still deserialize. The NO_ERROR_CODE / NO_ERROR_MESSAGE
// sentinels are applied at the call site in the connector's
// `build_error_response` - never `.unwrap_or_default()`, which turns a missing
// code into an empty string indistinguishable from a real empty code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevpayErrorResponse {
    /// Generic REST wrapper fields, when revPAY reports one (e.g. its
    /// undocumented HTTP 422 `{"code":422,"Description":"Unprocessable
    /// Entity"}`). revPAY emits `code` as a bare integer in these bodies but
    /// as a string in others, so accept both.
    #[serde(default, deserialize_with = "revpay_string_or_int")]
    pub code: Option<String>,
    #[serde(alias = "Description", alias = "description", default)]
    pub message: Option<String>,
    /// revPAY's own outcome fields (a non-2xx JSON body still carries them).
    #[serde(rename = "Response_Code", default)]
    pub response_code: Option<String>,
    #[serde(rename = "Error_Description", default)]
    pub error_description: Option<String>,
}

/// Deserializes a JSON string OR number into `Option<String>`, so revPAY's
/// `{"code": 422}` and `{"code": "422"}` error bodies both parse.
fn revpay_string_or_int<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        String(String),
        Int(i64),
        Float(f64),
    }

    Ok(match Option::<StringOrNumber>::deserialize(deserializer)? {
        Some(StringOrNumber::String(value)) => Some(value),
        Some(StringOrNumber::Int(value)) => Some(value.to_string()),
        Some(StringOrNumber::Float(value)) => Some(value.to_string()),
        None => None,
    })
}

// =============================================================================
// TESTS
// =============================================================================

#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod recovery_tests;
