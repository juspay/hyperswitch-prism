//! Payhound request/response transformers.
//!
//! Payhound is a hosted **crypto invoice** gateway: the only thing UCS can create is an invoice
//! (`POST /api/v1/invoices`), and the shopper pays it by sending crypto to the returned address
//! from the hosted invoice page. There is no card, wallet or bank rail, no capture/void/refund
//! endpoint and no stored-credential concept anywhere in the API, which is why this module models
//! exactly one request struct and one response struct.
//!
//! Everything Payhound-specific that goes on the wire is declared as a documented module-level
//! `const` or a closed enum in this file so that no bare literal reaches a transformer.

use std::sync::atomic::{AtomicU64, Ordering};

use common_utils::{
    consts,
    crypto::{self, GenerateDigest, SignMessage},
    errors::CustomResult,
    pii,
    types::{MinorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
        WebhookDetailsResponse,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::{CryptoData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::{get_unimplemented_payment_method_error_message, is_payment_failure},
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    connectors::payhound::{PayhoundAmountConvertor, PayhoundRouterData},
    types::ResponseRouterData,
};

// ---------------------------------------------------------------------------------------------
// Connector constants
// ---------------------------------------------------------------------------------------------

/// Connector id as registered in UCS. Used in every error context emitted from this module so a
/// log line always names the connector that produced it.
pub(super) const PAYHOUND_CONNECTOR: &str = "payhound";

/// Display name used in `IntegrationError::NotSupported { connector }`, which requires a
/// `&'static str` and is surfaced verbatim to the merchant.
pub(super) const PAYHOUND_CONNECTOR_DISPLAY: &str = "Payhound";

/// Media type Payhound mandates on both `Content-Type` and `Accept`.
///
/// `v1_request_response` requires `application/vnd.api+json`; a different content type is answered
/// with `415 {"message":"Invalid Content-Type"}`. The `application/json` shown in the
/// `v1_invoices_create` example is a stale doc snippet and must not be used.
pub(super) const PAYHOUND_CONTENT_TYPE: &str = "application/vnd.api+json";

/// Header carrying the Payhound API key.
pub(super) const PAYHOUND_HEADER_KEY: &str = "X-MB-Key";

/// Header carrying the strictly-increasing request nonce (decimal string).
pub(super) const PAYHOUND_HEADER_NONCE: &str = "X-MB-Nonce";

/// Header carrying the lowercase-hex HMAC-SHA512 request signature (128 chars).
pub(super) const PAYHOUND_HEADER_SIGNATURE: &str = "X-MB-Signature";

/// Header Payhound stamps on every outgoing callback; it is part of the callback signing string.
/// Incoming webhook headers are looked up lowercased, matching how `RequestDetails` normalises them.
pub(super) const PAYHOUND_HEADER_CALLBACK_ID_LOWER: &str = "x-mb-callback-id";

/// Lowercase lookup key for the signature header on an incoming callback.
pub(super) const PAYHOUND_HEADER_SIGNATURE_LOWER: &str = "x-mb-signature";

/// Path of the Payhound create-invoice endpoint (Authorize).
///
/// The same constant feeds the request URL and — via the path component of that URL — the
/// `uri_path` element of the HMAC signing string, so the two can never drift.
pub(super) const PAYHOUND_INVOICES_PATH: &str = "/api/v1/invoices";

/// Base for Payhound's hosted invoice payment page.
///
/// The API returns `invoice_url` as an absolute URL in the documentation but as a path-relative
/// URL in the sandbox (`/invoices/{id}`), so a relative value is resolved against this base.
/// If this ever has to vary per environment it belongs in connector metadata, not in a branch on
/// the API host.
pub(super) const PAYHOUND_HOSTED_INVOICE_BASE: &str = "https://pay.payhound.com";

/// Failure message for a Payhound invoice the shopper cancelled on the hosted page.
///
/// Payhound returns no machine-readable code or reason on a status-driven failure, so the message
/// is supplied by the connector.
pub(super) const PAYHOUND_ERROR_ABORTED: &str = "Invoice was cancelled by the customer";

/// Failure message for a Payhound invoice that expired before sufficient payment arrived.
pub(super) const PAYHOUND_ERROR_TIMEOUT: &str =
    "Invoice expired before sufficient payment was received";

/// HTTP status reported on a webhook-derived response. A callback that reached the handler was
/// delivered successfully, so there is no connector HTTP status of its own to propagate.
const PAYHOUND_WEBHOOK_STATUS_CODE: u16 = 200;

/// Complete set of fiat settlement currencies Payhound supports, rendered for error contexts so a
/// merchant that sends an unsupported currency learns which ones are accepted.
const PAYHOUND_SUPPORTED_SETTLEMENT_CURRENCIES: &str = "EUR GBP USD JPY CNY THB MYR IDR CHF TRY KRW INR UAH KZT NOK BRL HKD NGN SEK ZAR PLN DKK NZD AUD CLP PEN VND HUF RON";

/// Complete set of crypto assets Payhound accepts as `invoice_currency`, rendered for error
/// contexts. These are Payhound symbols, not ISO-4217 codes.
const PAYHOUND_SUPPORTED_INVOICE_CURRENCIES: &str = "BTC ETH SOL TRX POL BNB_BSC USDC_ERC20 \
     USDC_SOL USDC_POL USDC_BEP20 USDC_BASE USDT_SOL USDT_TRC20 USDT_ERC20 USDT_POL USDT_BEP20 \
     USDT_BASE EURC_ERC20 ETH_BASE";

// ---------------------------------------------------------------------------------------------
// Nonce
// ---------------------------------------------------------------------------------------------

/// Strictly-increasing nonce source for Payhound's `X-MB-Nonce`.
///
/// Payhound rejects any nonce that is not strictly greater than the previous one used with the
/// same API key (`400 {"message":"Invalid nonce"}`), so a bare microsecond clock read is unsafe
/// under concurrency: two requests landing in the same microsecond would emit the same value and
/// one would be rejected.
///
/// **Operational limitation:** this counter is per-process. A horizontally scaled deployment
/// sharing one Payhound API key can still emit a non-increasing nonce across pods, and Payhound
/// offers no server-side mitigation — use one API key per deployment. A `400 "Invalid nonce"` is
/// surfaced to the merchant as an ordinary connector error and is deliberately **not** retried with
/// a bumped nonce, because a retry would re-send a payment intent.
static PAYHOUND_NONCE: AtomicU64 = AtomicU64::new(0);

/// Returns the next `X-MB-Nonce` value: `max(now_micros, previous + 1)`.
///
/// The clock read is propagated with `?` rather than defaulted — a nonce of `0` would be rejected
/// by Payhound and would be far harder to diagnose than the clock error itself.
pub(super) fn next_nonce() -> CustomResult<u64, IntegrationError> {
    let now_micros = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: payhound_context(
                    "payhound: system clock is before the UNIX epoch, so no X-MB-Nonce could be \
                     derived",
                ),
            })?
            .as_micros(),
    )
    .change_context(IntegrationError::RequestEncodingFailed {
        context: payhound_context(
            "payhound: microsecond epoch does not fit in the unsigned 64-bit X-MB-Nonce range",
        ),
    })?;

    // The closure always returns `Some`, so `fetch_update` cannot fail today. The `Result` is
    // mapped explicitly rather than unwrapped so a future edit to the closure cannot silently panic.
    PAYHOUND_NONCE
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |previous| {
            Some(std::cmp::max(now_micros, previous.saturating_add(1)))
        })
        .map(|previous| std::cmp::max(now_micros, previous.saturating_add(1)))
        .map_err(|_| {
            report!(IntegrationError::RequestEncodingFailed {
                context: payhound_context("payhound: failed to advance the X-MB-Nonce counter"),
            })
        })
}

// ---------------------------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------------------------

/// Builds an `IntegrationErrorContext` carrying a Payhound-specific explanation.
pub(super) fn payhound_context(detail: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        additional_context: Some(detail.to_owned()),
        ..Default::default()
    }
}

/// Lowercase hex SHA-256 of the given bytes — the `request_digest` element of Payhound's signing
/// string, and also the body digest used for callback verification.
pub(super) fn payhound_sha256_hex(data: &[u8]) -> CustomResult<String, IntegrationError> {
    crypto::Sha256
        .generate_digest(data)
        .map(hex::encode)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: payhound_context("payhound: failed to SHA-256 the request data for signing"),
        })
}

/// Computes `X-MB-Signature` for an outgoing API request.
///
/// `signature = lowercase_hex(HMAC_SHA512(api_secret, uri_path ++ nonce ++ hex(SHA256(request_data))))`
///
/// * `uri_path` — path component only, always leading-slash, no host and no query string.
/// * `request_data` — for POST the exact JSON body bytes that go on the wire; for GET the
///   RFC3986-encoded query string, or the empty string when there is none.
pub(super) fn payhound_request_signature(
    api_secret: &Secret<String>,
    uri_path: &str,
    nonce: u64,
    request_data: &str,
) -> CustomResult<String, IntegrationError> {
    let digest = payhound_sha256_hex(request_data.as_bytes())?;
    let message = format!("{uri_path}{nonce}{digest}");
    crypto::HmacSha512
        .sign_message(api_secret.peek().as_bytes(), message.as_bytes())
        .map(hex::encode)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: payhound_context(
                "payhound: failed to compute the HMAC-SHA512 X-MB-Signature for the request",
            ),
        })
}

/// Builds the callback verification message.
///
/// Callback signing differs from request signing: the message is
/// `X-MB-Callback-Id ++ hex(SHA256(raw_body))` — **no** `uri_path` and **no** nonce.
pub(super) fn payhound_callback_message(
    callback_id: &str,
    raw_body: &[u8],
) -> CustomResult<Vec<u8>, IntegrationError> {
    let digest = payhound_sha256_hex(raw_body)?;
    Ok(format!("{callback_id}{digest}").into_bytes())
}

/// Payhound returns no machine-readable error code — every 4xx/5xx body is exactly
/// `{"message": "<text>"}` — so the HTTP status is the only stable discriminator available to put
/// in `ErrorResponse::code`.
pub(super) fn payhound_error_code(status_code: u16) -> String {
    status_code.to_string()
}

// ---------------------------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------------------------

/// Payhound credentials: an API key (`X-MB-Key`) and the secret used to sign requests and to
/// verify incoming callbacks.
pub struct PayhoundAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PayhoundAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Payhound {
            api_key,
            api_secret,
            ..
        } = auth_type
        {
            Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: payhound_context(
                    "payhound: connector config is not ConnectorSpecificConfig::Payhound; \
                     Payhound requires api_key + api_secret (SignatureKey)",
                ),
            }
            .into())
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Currencies
// ---------------------------------------------------------------------------------------------

/// The complete set of crypto assets Payhound accepts as `invoice_currency`.
///
/// Each symbol encodes the asset **and** its settlement network in a single token. These are
/// **not** ISO-4217 codes and are **not** all three characters — the docs' `string(3)` annotation
/// is wrong (`USDT_TRC20` is ten characters), so no length check or ISO parse may be applied here.
///
/// Every variant carries an explicit `#[serde(rename)]`: a `SCREAMING_SNAKE_CASE` blanket rename
/// would not reproduce `USDC_ERC20`/`BNB_BSC` reliably, and a silent drift would invoice the
/// shopper in the wrong asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayhoundInvoiceCurrency {
    /// Bitcoin.
    #[serde(rename = "BTC")]
    Btc,
    /// Ethereum.
    #[serde(rename = "ETH")]
    Eth,
    /// Solana.
    #[serde(rename = "SOL")]
    Sol,
    /// Tron.
    #[serde(rename = "TRX")]
    Trx,
    /// Polygon.
    #[serde(rename = "POL")]
    Pol,
    /// Binance Coin on the BSC network.
    #[serde(rename = "BNB_BSC")]
    BnbBsc,
    /// USDC on the ERC20 network.
    #[serde(rename = "USDC_ERC20")]
    UsdcErc20,
    /// USDC on the Solana network.
    #[serde(rename = "USDC_SOL")]
    UsdcSol,
    /// USDC on the Polygon network.
    #[serde(rename = "USDC_POL")]
    UsdcPol,
    /// USDC on the BEP20 network.
    #[serde(rename = "USDC_BEP20")]
    UsdcBep20,
    /// USDC on the Base network.
    #[serde(rename = "USDC_BASE")]
    UsdcBase,
    /// USDT on the Solana network.
    #[serde(rename = "USDT_SOL")]
    UsdtSol,
    /// USDT on the TRC20 network.
    #[serde(rename = "USDT_TRC20")]
    UsdtTrc20,
    /// USDT on the ERC20 network.
    #[serde(rename = "USDT_ERC20")]
    UsdtErc20,
    /// USDT on the Polygon network.
    #[serde(rename = "USDT_POL")]
    UsdtPol,
    /// USDT on the BEP20 network.
    #[serde(rename = "USDT_BEP20")]
    UsdtBep20,
    /// USDT on the Base network.
    #[serde(rename = "USDT_BASE")]
    UsdtBase,
    /// EURC on the ERC20 network.
    #[serde(rename = "EURC_ERC20")]
    EurcErc20,
    /// Ethereum on the Base network.
    #[serde(rename = "ETH_BASE")]
    EthBase,
}

impl PayhoundInvoiceCurrency {
    /// The exact token Payhound expects on the wire.
    pub(super) fn as_symbol(self) -> &'static str {
        match self {
            Self::Btc => "BTC",
            Self::Eth => "ETH",
            Self::Sol => "SOL",
            Self::Trx => "TRX",
            Self::Pol => "POL",
            Self::BnbBsc => "BNB_BSC",
            Self::UsdcErc20 => "USDC_ERC20",
            Self::UsdcSol => "USDC_SOL",
            Self::UsdcPol => "USDC_POL",
            Self::UsdcBep20 => "USDC_BEP20",
            Self::UsdcBase => "USDC_BASE",
            Self::UsdtSol => "USDT_SOL",
            Self::UsdtTrc20 => "USDT_TRC20",
            Self::UsdtErc20 => "USDT_ERC20",
            Self::UsdtPol => "USDT_POL",
            Self::UsdtBep20 => "USDT_BEP20",
            Self::UsdtBase => "USDT_BASE",
            Self::EurcErc20 => "EURC_ERC20",
            Self::EthBase => "ETH_BASE",
        }
    }

    /// The network half of the symbol, or `None` for symbols that name a chain's native asset and
    /// therefore carry no separate network component.
    pub(super) fn network_component(self) -> Option<&'static str> {
        match self {
            Self::Btc | Self::Eth | Self::Sol | Self::Trx | Self::Pol => None,
            Self::BnbBsc => Some("BSC"),
            Self::UsdcErc20 | Self::UsdtErc20 | Self::EurcErc20 => Some("ERC20"),
            Self::UsdcSol | Self::UsdtSol => Some("SOL"),
            Self::UsdtTrc20 => Some("TRC20"),
            Self::UsdcPol | Self::UsdtPol => Some("POL"),
            Self::UsdcBep20 | Self::UsdtBep20 => Some("BEP20"),
            Self::UsdcBase | Self::UsdtBase | Self::EthBase => Some("BASE"),
        }
    }

    /// Parses an already-normalised (trimmed, upper-cased) Payhound symbol. Membership only — no
    /// length validation, because the symbols are three to ten characters long.
    pub(super) fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol {
            "BTC" => Some(Self::Btc),
            "ETH" => Some(Self::Eth),
            "SOL" => Some(Self::Sol),
            "TRX" => Some(Self::Trx),
            "POL" => Some(Self::Pol),
            "BNB_BSC" => Some(Self::BnbBsc),
            "USDC_ERC20" => Some(Self::UsdcErc20),
            "USDC_SOL" => Some(Self::UsdcSol),
            "USDC_POL" => Some(Self::UsdcPol),
            "USDC_BEP20" => Some(Self::UsdcBep20),
            "USDC_BASE" => Some(Self::UsdcBase),
            "USDT_SOL" => Some(Self::UsdtSol),
            "USDT_TRC20" => Some(Self::UsdtTrc20),
            "USDT_ERC20" => Some(Self::UsdtErc20),
            "USDT_POL" => Some(Self::UsdtPol),
            "USDT_BEP20" => Some(Self::UsdtBep20),
            "USDT_BASE" => Some(Self::UsdtBase),
            "EURC_ERC20" => Some(Self::EurcErc20),
            "ETH_BASE" => Some(Self::EthBase),
            _ => None,
        }
    }
}

/// The complete set of fiat settlement currencies Payhound supports (`currency` on the invoice).
///
/// Modelled as a closed enum rather than passing `common_enums::Currency` straight through, so an
/// unsupported currency is rejected at the UCS boundary with an actionable message instead of
/// producing an opaque `400 {"message":"Unsupported currency"}` from Payhound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayhoundSettlementCurrency {
    /// Payhound settlement currency `EUR`.
    #[serde(rename = "EUR")]
    EUR,
    /// Payhound settlement currency `GBP`.
    #[serde(rename = "GBP")]
    GBP,
    /// Payhound settlement currency `USD`.
    #[serde(rename = "USD")]
    USD,
    /// Payhound settlement currency `JPY`.
    #[serde(rename = "JPY")]
    JPY,
    /// Payhound settlement currency `CNY`.
    #[serde(rename = "CNY")]
    CNY,
    /// Payhound settlement currency `THB`.
    #[serde(rename = "THB")]
    THB,
    /// Payhound settlement currency `MYR`.
    #[serde(rename = "MYR")]
    MYR,
    /// Payhound settlement currency `IDR`.
    #[serde(rename = "IDR")]
    IDR,
    /// Payhound settlement currency `CHF`.
    #[serde(rename = "CHF")]
    CHF,
    /// Payhound settlement currency `TRY`.
    #[serde(rename = "TRY")]
    TRY,
    /// Payhound settlement currency `KRW`.
    #[serde(rename = "KRW")]
    KRW,
    /// Payhound settlement currency `INR`.
    #[serde(rename = "INR")]
    INR,
    /// Payhound settlement currency `UAH`.
    #[serde(rename = "UAH")]
    UAH,
    /// Payhound settlement currency `KZT`.
    #[serde(rename = "KZT")]
    KZT,
    /// Payhound settlement currency `NOK`.
    #[serde(rename = "NOK")]
    NOK,
    /// Payhound settlement currency `BRL`.
    #[serde(rename = "BRL")]
    BRL,
    /// Payhound settlement currency `HKD`.
    #[serde(rename = "HKD")]
    HKD,
    /// Payhound settlement currency `NGN`.
    #[serde(rename = "NGN")]
    NGN,
    /// Payhound settlement currency `SEK`.
    #[serde(rename = "SEK")]
    SEK,
    /// Payhound settlement currency `ZAR`.
    #[serde(rename = "ZAR")]
    ZAR,
    /// Payhound settlement currency `PLN`.
    #[serde(rename = "PLN")]
    PLN,
    /// Payhound settlement currency `DKK`.
    #[serde(rename = "DKK")]
    DKK,
    /// Payhound settlement currency `NZD`.
    #[serde(rename = "NZD")]
    NZD,
    /// Payhound settlement currency `AUD`.
    #[serde(rename = "AUD")]
    AUD,
    /// Payhound settlement currency `CLP`.
    #[serde(rename = "CLP")]
    CLP,
    /// Payhound settlement currency `PEN`.
    #[serde(rename = "PEN")]
    PEN,
    /// Payhound settlement currency `VND`.
    #[serde(rename = "VND")]
    VND,
    /// Payhound settlement currency `HUF`.
    #[serde(rename = "HUF")]
    HUF,
    /// Payhound settlement currency `RON`.
    #[serde(rename = "RON")]
    RON,
}

impl TryFrom<common_enums::Currency> for PayhoundSettlementCurrency {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(currency: common_enums::Currency) -> Result<Self, Self::Error> {
        // Matched exhaustively (no `_` arm) so that a currency added to `common_enums::Currency`
        // breaks this build and forces a deliberate decision rather than silently becoming
        // "supported" or silently unreachable.
        match currency {
            common_enums::Currency::EUR => Ok(Self::EUR),
            common_enums::Currency::GBP => Ok(Self::GBP),
            common_enums::Currency::USD => Ok(Self::USD),
            common_enums::Currency::JPY => Ok(Self::JPY),
            common_enums::Currency::CNY => Ok(Self::CNY),
            common_enums::Currency::THB => Ok(Self::THB),
            common_enums::Currency::MYR => Ok(Self::MYR),
            common_enums::Currency::IDR => Ok(Self::IDR),
            common_enums::Currency::CHF => Ok(Self::CHF),
            common_enums::Currency::TRY => Ok(Self::TRY),
            common_enums::Currency::KRW => Ok(Self::KRW),
            common_enums::Currency::INR => Ok(Self::INR),
            common_enums::Currency::UAH => Ok(Self::UAH),
            common_enums::Currency::KZT => Ok(Self::KZT),
            common_enums::Currency::NOK => Ok(Self::NOK),
            common_enums::Currency::BRL => Ok(Self::BRL),
            common_enums::Currency::HKD => Ok(Self::HKD),
            common_enums::Currency::NGN => Ok(Self::NGN),
            common_enums::Currency::SEK => Ok(Self::SEK),
            common_enums::Currency::ZAR => Ok(Self::ZAR),
            common_enums::Currency::PLN => Ok(Self::PLN),
            common_enums::Currency::DKK => Ok(Self::DKK),
            common_enums::Currency::NZD => Ok(Self::NZD),
            common_enums::Currency::AUD => Ok(Self::AUD),
            common_enums::Currency::CLP => Ok(Self::CLP),
            common_enums::Currency::PEN => Ok(Self::PEN),
            common_enums::Currency::VND => Ok(Self::VND),
            common_enums::Currency::HUF => Ok(Self::HUF),
            common_enums::Currency::RON => Ok(Self::RON),
            common_enums::Currency::AED | common_enums::Currency::AFN | common_enums::Currency::ALL | common_enums::Currency::AMD | common_enums::Currency::ANG |
            common_enums::Currency::AOA | common_enums::Currency::ARS | common_enums::Currency::AWG | common_enums::Currency::AZN | common_enums::Currency::BAM |
            common_enums::Currency::BBD | common_enums::Currency::BDT | common_enums::Currency::BGN | common_enums::Currency::BHD | common_enums::Currency::BIF |
            common_enums::Currency::BMD | common_enums::Currency::BND | common_enums::Currency::BOB | common_enums::Currency::BSD | common_enums::Currency::BTN |
            common_enums::Currency::BWP | common_enums::Currency::BYN | common_enums::Currency::BZD | common_enums::Currency::CAD | common_enums::Currency::CDF |
            common_enums::Currency::CLF | common_enums::Currency::COP | common_enums::Currency::CRC | common_enums::Currency::CUC | common_enums::Currency::CUP |
            common_enums::Currency::CVE | common_enums::Currency::CZK | common_enums::Currency::DJF | common_enums::Currency::DOP | common_enums::Currency::DZD |
            common_enums::Currency::EGP | common_enums::Currency::ERN | common_enums::Currency::ETB | common_enums::Currency::FJD | common_enums::Currency::FKP |
            common_enums::Currency::GEL | common_enums::Currency::GHS | common_enums::Currency::GIP | common_enums::Currency::GMD | common_enums::Currency::GNF |
            common_enums::Currency::GTQ | common_enums::Currency::GYD | common_enums::Currency::HNL | common_enums::Currency::HRK | common_enums::Currency::HTG |
            common_enums::Currency::ILS | common_enums::Currency::IQD | common_enums::Currency::IRR | common_enums::Currency::ISK | common_enums::Currency::JMD |
            common_enums::Currency::JOD | common_enums::Currency::KES | common_enums::Currency::KGS | common_enums::Currency::KHR | common_enums::Currency::KMF |
            common_enums::Currency::KPW | common_enums::Currency::KWD | common_enums::Currency::KYD | common_enums::Currency::LAK | common_enums::Currency::LBP |
            common_enums::Currency::LKR | common_enums::Currency::LRD | common_enums::Currency::LSL | common_enums::Currency::LYD | common_enums::Currency::MAD |
            common_enums::Currency::MDL | common_enums::Currency::MGA | common_enums::Currency::MKD | common_enums::Currency::MMK | common_enums::Currency::MNT |
            common_enums::Currency::MOP | common_enums::Currency::MRU | common_enums::Currency::MUR | common_enums::Currency::MVR | common_enums::Currency::MWK |
            common_enums::Currency::MXN | common_enums::Currency::MZN | common_enums::Currency::NAD | common_enums::Currency::NIO | common_enums::Currency::NPR |
            common_enums::Currency::OMR | common_enums::Currency::PAB | common_enums::Currency::PGK | common_enums::Currency::PHP | common_enums::Currency::PKR |
            common_enums::Currency::PYG | common_enums::Currency::QAR | common_enums::Currency::RSD | common_enums::Currency::RUB | common_enums::Currency::RWF |
            common_enums::Currency::SAR | common_enums::Currency::SBD | common_enums::Currency::SCR | common_enums::Currency::SDG | common_enums::Currency::SGD |
            common_enums::Currency::SHP | common_enums::Currency::SLE | common_enums::Currency::SLL | common_enums::Currency::SOS | common_enums::Currency::SRD |
            common_enums::Currency::SSP | common_enums::Currency::STD | common_enums::Currency::STN | common_enums::Currency::SVC | common_enums::Currency::SYP |
            common_enums::Currency::SZL | common_enums::Currency::TJS | common_enums::Currency::TMT | common_enums::Currency::TND | common_enums::Currency::TOP |
            common_enums::Currency::TTD | common_enums::Currency::TWD | common_enums::Currency::TZS | common_enums::Currency::UGX | common_enums::Currency::UYU |
            common_enums::Currency::UZS | common_enums::Currency::VES | common_enums::Currency::VUV | common_enums::Currency::WST | common_enums::Currency::XAF |
            common_enums::Currency::XCD | common_enums::Currency::XOF | common_enums::Currency::XPF | common_enums::Currency::YER | common_enums::Currency::ZMW |
            common_enums::Currency::ZWL => {
                Err(report!(IntegrationError::CurrencyNotSupported {
                    message: currency.to_string(),
                    connector: PAYHOUND_CONNECTOR_DISPLAY,
                    context: payhound_context(&format!(
                        "payhound: Authorize settlement currency `{currency}` is not one of the \
                         currencies Payhound settles in. Supported: {PAYHOUND_SUPPORTED_SETTLEMENT_CURRENCIES}"
                    )),
                }))
            }
        }
    }
}

/// Resolves UCS's two independent `CryptoData` fields onto the single token Payhound expects.
///
/// Payhound encodes asset **and** network in one symbol (`USDT_TRC20`) whereas `CryptoData` carries
/// `pay_currency` and `network` separately. Resolution order:
///
/// 1. `pay_currency` is required. It is fetched through `CryptoData::get_pay_currency`, which
///    already reports the caller-facing path `crypto_data.pay_currency` when absent. Requiring it
///    rather than letting Payhound silently default to Bitcoin makes the misconfiguration visible.
/// 2. If the normalised `pay_currency` already **is** a Payhound symbol, it wins — and a supplied
///    `network` must agree with that symbol's network component. A mismatch is an error, never a
///    silent ignore: dropping the caller's chain choice would put the shopper's funds on the wrong
///    chain.
/// 3. Otherwise, if `network` is present, the composed `{pay_currency}_{network}` token is tried.
/// 4. Otherwise the pair is rejected, naming what was received and enumerating the supported set.
///
/// A network is never appended to a symbol that already has one, and a suffix is never stripped.
pub(super) fn resolve_invoice_currency(
    crypto_data: &CryptoData,
) -> CustomResult<PayhoundInvoiceCurrency, IntegrationError> {
    let pay_currency = crypto_data.get_pay_currency()?;
    let normalized_currency = pay_currency.trim().to_uppercase();
    let normalized_network = crypto_data
        .network
        .as_ref()
        .map(|network| network.trim().to_uppercase());

    let unsupported = |detail: String| {
        report!(IntegrationError::NotSupported {
            message: format!("Crypto currency `{normalized_currency}`"),
            connector: PAYHOUND_CONNECTOR_DISPLAY,
            context: payhound_context(&detail),
        })
    };

    match PayhoundInvoiceCurrency::from_symbol(&normalized_currency) {
        Some(symbol) => match (normalized_network.as_deref(), symbol.network_component()) {
            // No network supplied: the symbol already names the chain.
            (None, _) => Ok(symbol),
            // Symbol carries a network component and the caller agrees with it.
            (Some(network), Some(component)) if network == component => Ok(symbol),
            // Symbol names a chain's native asset; the only network the caller may name is the
            // symbol itself (e.g. pay_currency = "BTC", network = "BTC").
            (Some(network), None) if network == symbol.as_symbol() => Ok(symbol),
            (Some(network), _) => Err(unsupported(format!(
                "payhound: crypto_data.network `{network}` conflicts with the network encoded in \
                 crypto_data.pay_currency `{normalized_currency}`. Payhound accepts exactly one \
                 currency token per invoice, so the conflicting chain choice cannot be honoured. \
                 Supported symbols: {PAYHOUND_SUPPORTED_INVOICE_CURRENCIES}"
            ))),
        },
        None => match normalized_network.as_deref() {
            Some(network) => {
                let composed = format!("{normalized_currency}_{network}");
                PayhoundInvoiceCurrency::from_symbol(&composed).ok_or_else(|| {
                    unsupported(format!(
                        "payhound: neither crypto_data.pay_currency `{normalized_currency}` nor \
                         the composed token `{composed}` is a Payhound invoice currency. \
                         Supported symbols: {PAYHOUND_SUPPORTED_INVOICE_CURRENCIES}"
                    ))
                })
            }
            None => Err(unsupported(format!(
                "payhound: crypto_data.pay_currency `{normalized_currency}` is not a Payhound \
                 invoice currency and no crypto_data.network was supplied to compose one. \
                 Supported symbols: {PAYHOUND_SUPPORTED_INVOICE_CURRENCIES}"
            ))),
        },
    }
}

/// Rejects capture methods Payhound cannot honour.
///
/// Payhound settles a hosted crypto invoice automatically on payment and exposes **no** capture and
/// **no** void endpoint, so accepting a manual-capture Authorize would create a payment that can
/// never be settled or released. `Automatic`, `SequentialAutomatic` and an absent capture method
/// are the auto-capture set used repo-wide.
pub(super) fn ensure_supported_capture_method(
    capture_method: Option<common_enums::CaptureMethod>,
) -> CustomResult<(), IntegrationError> {
    match capture_method {
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(()),
        Some(common_enums::CaptureMethod::Manual)
        | Some(common_enums::CaptureMethod::ManualMultiple)
        | Some(common_enums::CaptureMethod::Scheduled) => {
            Err(report!(IntegrationError::CaptureMethodNotSupported {
                context: payhound_context(
                    "payhound: Payhound settles hosted crypto invoices automatically and exposes \
                     no capture or void endpoint; only automatic capture is supported",
                ),
            }))
        }
    }
}

/// Turns the `invoice_url` Payhound returned into an absolute redirect target.
///
/// The documentation shows an absolute URL while the sandbox returns a path-relative one
/// (`/invoices/{id}`), and `RedirectForm::from((Url, Method))` needs a fully-qualified URL, so a
/// relative value is resolved against [`PAYHOUND_HOSTED_INVOICE_BASE`].
///
/// A parse failure is an error rather than a missing redirect: Payhound is redirect-only, so an
/// authorization whose invoice URL cannot be resolved leaves the shopper with no way to pay.
pub(super) fn build_invoice_redirect(invoice_url: &str) -> CustomResult<Url, ConnectorError> {
    let trimmed = invoice_url.trim();
    let parsed = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Url::parse(trimmed)
    } else {
        Url::parse(PAYHOUND_HOSTED_INVOICE_BASE).and_then(|base| base.join(trimmed))
    };

    parsed.change_context(ConnectorError::UnexpectedResponseError {
        context: ResponseTransformationErrorContext {
            http_status_code: None,
            additional_context: Some(format!(
                "payhound: could not resolve the hosted invoice URL `{trimmed}` into an absolute \
                 redirect target"
            )),
        },
    })
}

// ---------------------------------------------------------------------------------------------
// Authorize request
// ---------------------------------------------------------------------------------------------

/// Body of `POST /api/v1/invoices`.
///
/// No `Default` derive: `currency`, `price` and `invoice_currency` are required and must never be
/// fillable by a default. Every optional field is skipped when `None` rather than serialized as
/// `null` — an explicit `null` changes the SHA-256 digest for no reason and, for the three URL
/// fields, is not the documented way to inherit the merchant-profile default (omission is).
#[derive(Debug, Serialize)]
pub struct PayhoundInvoiceRequest {
    /// Merchant settlement currency.
    currency: PayhoundSettlementCurrency,
    /// Invoice amount as a decimal major-unit string, e.g. `"266.45"`.
    price: StringMajorUnit,
    /// Crypto asset (and chain) the shopper pays in.
    invoice_currency: PayhoundInvoiceCurrency,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    callback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    success_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notify_email: Option<pii::Email>,
    /// Percentage of the settlement Payhound keeps in BTC.
    ///
    /// Always `None`: this mirrors a merchant-level Payhound setting (Payouts → *Percentage Kept
    /// in BTC*) that UCS has no field for, and sending a fabricated value would override the
    /// merchant's own configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    share_to_keep_in_btc: Option<u8>,
}

/// Resolves the Payhound invoice currency from the payment method data.
///
/// The match is exhaustive with no `_` arm so that adding a `PaymentMethodData` variant to the repo
/// breaks Payhound's build and forces a deliberate decision.
fn invoice_currency_from_payment_method<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
) -> CustomResult<PayhoundInvoiceCurrency, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Crypto(crypto_data) => resolve_invoice_currency(crypto_data),
        PaymentMethodData::Card(_)
        | PaymentMethodData::CardWithNoCvc(_)
        | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::CardRedirect(_)
        | PaymentMethodData::Wallet(_)
        | PaymentMethodData::PayLater(_)
        | PaymentMethodData::BankRedirect(_)
        | PaymentMethodData::BankDebit(_)
        | PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::MandatePayment
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::PaymentMethodToken(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::MobilePayment(_) => Err(report!(IntegrationError::NotSupported {
            message: get_unimplemented_payment_method_error_message(PAYHOUND_CONNECTOR_DISPLAY),
            connector: PAYHOUND_CONNECTOR_DISPLAY,
            context: payhound_context(
                "payhound: Payhound is a hosted crypto invoice gateway and accepts only \
                     PaymentMethodData::Crypto; it has no card, wallet or bank rails",
            ),
        })),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayhoundRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayhoundInvoiceRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PayhoundRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let common = &item.router_data.resource_common_data;

        ensure_supported_capture_method(request.capture_method)?;

        let invoice_currency = invoice_currency_from_payment_method(&request.payment_method_data)?;
        let currency = PayhoundSettlementCurrency::try_from(request.currency)?;
        // Payhound's `price` is a decimal major-unit string ("266.45"); minor units would be
        // rejected as a wildly different amount.
        let price = PayhoundAmountConvertor::convert(request.minor_amount, request.currency)?;

        Ok(Self {
            currency,
            price,
            invoice_currency,
            // Shown to the shopper on the hosted invoice page. Only populated from a descriptor the
            // merchant actually supplied — never fabricated.
            name: request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
            description: common.description.clone(),
            // Echoed back by Payhound and read back as `connector_response_reference_id`.
            reference: Some(common.connector_request_reference_id.clone()),
            callback_url: request.webhook_url.clone(),
            success_url: request.router_return_url.clone(),
            // UCS carries a single return URL, so success and cancel deliberately point at the same
            // place; this duplication is not a copy-paste slip.
            cancel_url: request.router_return_url.clone(),
            notify_email: request.email.clone(),
            share_to_keep_in_btc: None,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Statuses
// ---------------------------------------------------------------------------------------------

/// Payhound invoice status: the six documented values plus a soft-fail fallback so a status
/// Payhound introduces later is diagnosable rather than an opaque deserialization failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PayhoundInvoiceStatus {
    /// Invoice created, no payment received yet.
    Pending,
    /// Invoice fully paid.
    Completed,
    /// Invoice fully paid, plus surplus funds.
    Overpaid,
    /// Insufficient payment received so far; not terminal.
    Underpaid,
    /// Cancelled by the shopper on the hosted page.
    Aborted,
    /// Expired before sufficient payment arrived.
    Timeout,
    /// Any status Payhound introduces after this integration was written.
    #[serde(other)]
    Unknown,
}

impl From<PayhoundInvoiceStatus> for common_enums::AttemptStatus {
    fn from(status: PayhoundInvoiceStatus) -> Self {
        match status {
            // The shopper still has to open `invoice_url` and send funds, i.e. the attempt is
            // waiting on customer action at a redirect.
            PayhoundInvoiceStatus::Pending => Self::AuthenticationPending,
            // Payhound has already converted to the settlement currency; there is no separate
            // capture step.
            PayhoundInvoiceStatus::Completed => Self::Charged,
            // Deliberate: the ordered amount is fully covered, so the order must be fulfilled. The
            // surplus is an account-level deposit into the merchant's Payhound balance, not an
            // attribute of this attempt — `Unresolved` would leave a fully-paid order permanently
            // unfulfilled and `PartialCharged` would be wrong in the other direction.
            PayhoundInvoiceStatus::Overpaid => Self::Charged,
            // Deliberate: `underpaid` is explicitly non-terminal — the shopper can still top up,
            // and the invoice always resolves to completed/overpaid/timeout/aborted. `Failure`
            // would kill a recoverable order, `Charged` would credit an unpaid one, and
            // `PartialCharged` would assert a partial capture Payhound does not model.
            // Operational note: an `underpaid` invoice that reaches `timeout` leaves the partial
            // funds deposited in the merchant's Payhound account out-of-band; that reconciliation
            // is a merchant-dashboard concern and is deliberately not modelled as a partial capture.
            PayhoundInvoiceStatus::Underpaid => Self::Pending,
            // Shopper-initiated cancellation on the hosted page. Not `Voided`, which implies a
            // merchant-initiated cancellation through a void endpoint Payhound does not expose.
            PayhoundInvoiceStatus::Aborted => Self::Failure,
            // Terminal. Together with `aborted` this guarantees every non-terminal state has a
            // terminal failure path, so nothing can poll forever.
            PayhoundInvoiceStatus::Timeout => Self::Failure,
            // Soft fail: an unrecognised status must never be optimistically treated as success,
            // nor hard-failed (a status Payhound adds later would decline every affected payment).
            // `Pending` keeps PSync/webhooks alive so a later poll can resolve it. `serde(other)`
            // discards the raw text, so `raw_connector_response` is what makes the actual value
            // recoverable from the event log.
            PayhoundInvoiceStatus::Unknown => {
                tracing::warn!(
                    connector = PAYHOUND_CONNECTOR,
                    "payhound: unrecognised invoice status received; mapped to Pending. Inspect \
                     raw_connector_response for the literal value"
                );
                Self::Pending
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Invoice response (shared by Authorize, PSync and webhooks — the shapes are identical)
// ---------------------------------------------------------------------------------------------

/// One of the alternative deposit addresses Payhound offers for an invoice (for example a
/// `base58` and a `bech32` rendering of the same Bitcoin address).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayhoundAltAddress {
    /// The deposit address itself.
    pub address: Secret<String>,
    /// Address encoding, e.g. `base58`.
    #[serde(rename = "type")]
    pub address_type: Option<String>,
    /// Whether this is the address the hosted page shows by default.
    pub default: Option<bool>,
}

/// Payhound invoice object.
///
/// Returned verbatim by `POST /api/v1/invoices` (201), `GET /api/v1/invoices/{id}` (200) and as the
/// entire body of an incoming callback, so one struct serves all three.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayhoundInvoiceResponse {
    /// Payhound invoice id — the connector transaction id.
    pub id: String,
    /// Invoice state.
    pub status: PayhoundInvoiceStatus,
    /// Crypto deposit address the shopper sends funds to.
    pub address: Option<Secret<String>>,
    /// Alternative encodings of the deposit address.
    pub alt_addresses: Option<Vec<PayhoundAltAddress>>,
    /// Fiat settlement currency the merchant receives.
    pub merchant_currency: Option<common_enums::Currency>,
    /// Settlement amount in `merchant_currency`, decimal major-unit string.
    pub merchant_amount: Option<StringMajorUnit>,
    /// Crypto asset symbol. A free `String`: these are Payhound symbols, not ISO-4217 codes.
    pub invoice_currency: Option<String>,
    /// Crypto-denominated invoice amount. Deliberately **not** `StringMajorUnit`:
    /// `common_enums::Currency` cannot represent BTC, so it must never reach the amount converter.
    pub invoice_amount: Option<String>,
    /// Crypto asset actually paid in.
    pub paid_currency: Option<String>,
    /// Crypto-denominated amount received so far. Informational only, see `invoice_amount`.
    pub paid_amount: Option<String>,
    /// Crypto asset of funds seen but not yet accepted.
    pub pending_currency: Option<String>,
    /// Crypto-denominated amount seen but not yet accepted. Informational only.
    pub pending_amount: Option<String>,
    /// Merchant-level percentage of the settlement Payhound keeps in BTC.
    pub share_to_keep_in_btc: Option<u8>,
    /// Invoice name shown to the shopper.
    pub name: Option<String>,
    /// Invoice description shown to the shopper.
    pub description: Option<String>,
    /// Merchant correlation key echoed back from the request.
    pub reference: Option<String>,
    /// Addresses the funds were sent from.
    pub sender_addresses: Option<Vec<Secret<String>>>,
    /// Blockchain transaction ids backing the payment (PSync/webhook only).
    pub blockchain_txid: Option<Vec<String>>,
    /// Hosted invoice page. May be **relative** — resolve with [`build_invoice_redirect`].
    pub invoice_url: Option<String>,
    /// Callback target for this invoice.
    pub callback_url: Option<String>,
    /// Success return URL for this invoice.
    pub success_url: Option<String>,
    /// Cancel return URL for this invoice.
    pub cancel_url: Option<String>,
    /// Address Payhound notifies about this invoice.
    pub notify_email: Option<Secret<String, pii::EmailStrategy>>,
    /// Creation time, epoch seconds. A **float** on the wire (`1560168720.0`), so `i64` would fail
    /// to deserialize.
    pub create_time: Option<f64>,
    /// Expiry time, epoch seconds. Also a float — see `create_time`.
    pub valid_until_time: Option<f64>,
    /// Highest risk rating among the transactions related to this invoice.
    pub max_risk_rating_of_related_txes: Option<String>,
    /// Entity Payhound attributes the incoming funds to, when known.
    pub entity_name: Option<String>,
    /// Wallet category Payhound attributes the incoming funds to, when known.
    pub wallet_category: Option<String>,
}

/// Whether the caller must have a usable redirect target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PayhoundRedirectRequirement {
    /// Authorize: Payhound is redirect-only, so a successful authorization without a resolvable
    /// `invoice_url` is a dead end and must fail loudly.
    Required,
    /// PSync and webhooks: surfacing the redirect is useful when a shopper resumes a `pending`
    /// invoice, but its absence is not by itself a failure of the sync.
    Optional,
}

/// The parts of a Payhound invoice every flow needs, derived in exactly one place so that the
/// Authorize, PSync and webhook paths cannot drift apart.
pub(super) struct PayhoundInvoiceOutcome {
    /// Mapped attempt status.
    pub(super) status: common_enums::AttemptStatus,
    /// Settlement amount, populated only when the status is `Charged`.
    pub(super) minor_amount_captured: Option<MinorUnit>,
    /// Failure message when the mapped status is a terminal failure, otherwise `None`. Payhound
    /// supplies no code or reason on a status-driven failure, so the message comes from a const.
    pub(super) failure_message: Option<&'static str>,
}

/// Derives status, captured amount and failure detail from a Payhound invoice body.
///
/// `fallback_currency` is the currency the invoice was created in, used only when Payhound omits
/// `merchant_currency` from the body (the docs do not promise it is always present). The webhook
/// path has no router request to source it from and passes `None`; in that case no captured amount
/// is reported rather than a fabricated one.
pub(super) fn payhound_invoice_outcome(
    response: &PayhoundInvoiceResponse,
    http_code: u16,
    fallback_currency: Option<common_enums::Currency>,
) -> CustomResult<PayhoundInvoiceOutcome, ConnectorError> {
    let status = common_enums::AttemptStatus::from(response.status);

    let failure_message = match response.status {
        PayhoundInvoiceStatus::Aborted => Some(PAYHOUND_ERROR_ABORTED),
        PayhoundInvoiceStatus::Timeout => Some(PAYHOUND_ERROR_TIMEOUT),
        PayhoundInvoiceStatus::Pending
        | PayhoundInvoiceStatus::Completed
        | PayhoundInvoiceStatus::Overpaid
        | PayhoundInvoiceStatus::Underpaid
        | PayhoundInvoiceStatus::Unknown => None,
    };

    // Only the settlement leg the merchant actually receives is reported as captured, and only once
    // the invoice is fully paid. `invoice_amount`/`paid_amount` are crypto-denominated and cannot
    // be expressed as a `common_enums::Currency` amount.
    let minor_amount_captured = match (&response.merchant_amount, status) {
        (Some(amount), common_enums::AttemptStatus::Charged) => {
            match response.merchant_currency.or(fallback_currency) {
                Some(currency) => Some(
                    PayhoundAmountConvertor::convert_back(amount.clone(), currency)
                        .change_context(crate::utils::response_handling_fail(
                            http_code,
                            "payhound: merchant_amount could not be converted back to minor units",
                        ))?,
                ),
                // Deliberate, logged degradation: a settlement amount cannot be interpreted
                // without its currency, and guessing one would report a wrong captured amount.
                None => {
                    tracing::warn!(
                        connector = PAYHOUND_CONNECTOR,
                        "payhound: invoice is charged but carries no merchant_currency and no \
                         fallback currency was available, so no captured amount is reported"
                    );
                    None
                }
            }
        }
        _ => None,
    };

    Ok(PayhoundInvoiceOutcome {
        status,
        minor_amount_captured,
        failure_message,
    })
}

/// Builds the `PaymentsResponseData`/`ErrorResponse` pair shared by Authorize and PSync.
fn payhound_payments_response(
    response: &PayhoundInvoiceResponse,
    outcome: &PayhoundInvoiceOutcome,
    http_code: u16,
    redirect_requirement: PayhoundRedirectRequirement,
) -> CustomResult<Result<PaymentsResponseData, ErrorResponse>, ConnectorError> {
    if is_payment_failure(outcome.status) {
        let message = outcome
            .failure_message
            .unwrap_or(consts::NO_ERROR_MESSAGE)
            .to_owned();
        return Ok(Err(ErrorResponse {
            // Payhound emits no machine-readable code on a status-driven failure.
            code: consts::NO_ERROR_CODE.to_owned(),
            message: message.clone(),
            reason: Some(message),
            status_code: http_code,
            attempt_status: None,
            connector_transaction_id: Some(response.id.clone()),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }));
    }

    let redirection_data = match (&response.invoice_url, redirect_requirement) {
        (Some(invoice_url), _) => Some(RedirectForm::from((
            build_invoice_redirect(invoice_url)?,
            common_utils::request::Method::Get,
        ))),
        (None, PayhoundRedirectRequirement::Required) => {
            return Err(report!(crate::utils::unexpected_response_fail(
                http_code,
                "payhound: Authorize succeeded without an invoice_url, so the shopper has no \
                 hosted page to pay on",
            )))
        }
        (None, PayhoundRedirectRequirement::Optional) => None,
    };

    Ok(Ok(PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
        redirection_data: redirection_data.map(Box::new),
        // Payhound has no stored-credential concept.
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: response
            .reference
            .clone()
            .or_else(|| Some(response.id.clone())),
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
    }))
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PayhoundInvoiceResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayhoundInvoiceResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: payhound_response,
            router_data,
            http_code,
        } = item;

        let outcome = payhound_invoice_outcome(
            &payhound_response,
            http_code,
            Some(router_data.request.currency),
        )?;
        let response = payhound_payments_response(
            &payhound_response,
            &outcome,
            http_code,
            PayhoundRedirectRequirement::Required,
        )?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: outcome.status,
                amount_captured: outcome
                    .minor_amount_captured
                    .map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured: outcome.minor_amount_captured,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<PayhoundInvoiceResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PayhoundInvoiceResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: payhound_response,
            router_data,
            http_code,
        } = item;

        let outcome = payhound_invoice_outcome(
            &payhound_response,
            http_code,
            Some(router_data.request.currency),
        )?;
        // PSync surfaces the redirect when Payhound still provides one — useful for a shopper
        // resuming a `pending` invoice — but, unlike Authorize, its absence does not fail the sync:
        // a terminal invoice legitimately has nothing left to redirect to.
        let response = payhound_payments_response(
            &payhound_response,
            &outcome,
            http_code,
            PayhoundRedirectRequirement::Optional,
        )?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: outcome.status,
                amount_captured: outcome
                    .minor_amount_captured
                    .map(|amount| amount.get_amount_as_i64()),
                minor_amount_captured: outcome.minor_amount_captured,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Webhooks
// ---------------------------------------------------------------------------------------------

/// Builds the webhook response from a callback body.
///
/// There is no router request behind a callback, so the invoice's own `merchant_currency` is the
/// only currency available; when it is absent no captured amount is reported rather than guessing
/// one.
pub(super) fn payhound_webhook_details(
    response: &PayhoundInvoiceResponse,
) -> CustomResult<WebhookDetailsResponse, ConnectorError> {
    let outcome = payhound_invoice_outcome(response, PAYHOUND_WEBHOOK_STATUS_CODE, None)?;

    let reference = response
        .reference
        .clone()
        .or_else(|| Some(response.id.clone()));

    let (error_code, error_message, error_reason) = match outcome.failure_message {
        Some(message) => (
            Some(consts::NO_ERROR_CODE.to_owned()),
            Some(message.to_owned()),
            Some(message.to_owned()),
        ),
        None => (None, None, None),
    };

    Ok(WebhookDetailsResponse {
        status: outcome.status,
        resource_id: Some(ResponseId::ConnectorTransactionId(response.id.clone())),
        connector_response_reference_id: reference.clone(),
        connector_request_reference_id: reference,
        // Payhound has no stored credentials.
        mandate_reference: None,
        status_code: PAYHOUND_WEBHOOK_STATUS_CODE,
        error_code,
        error_message,
        error_reason,
        amount_captured: outcome
            .minor_amount_captured
            .map(|amount| amount.get_amount_as_i64()),
        minor_amount_captured: outcome.minor_amount_captured,
        raw_connector_response: None,
        response_headers: None,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

/// Payhound's error envelope. Every 4xx/5xx body is exactly `{"message": "<text>"}` — there is no
/// `code`, `error`, `errors[]` or `type` field, and Payhound never signals an error inside a 2xx
/// envelope, so there is no in-band error branch to write.
#[derive(Debug, Serialize, Deserialize)]
pub struct PayhoundErrorResponse {
    /// Human-readable failure description.
    pub message: String,
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use common_utils::types::MinorUnit;
    use domain_types::payment_method_data::{Card, DefaultPCIHolder};

    use super::*;

    /// Test vector 1 from `v1_authentication` — POST with a JSON body.
    const VECTOR_SECRET_POST: &str =
        "93yJJ8LBDe3zNSewHBdXlXIQDjCMDIn0EKNnXrd3kfzL72fvLz99uKnXFLYuCfkt";
    /// Test vector 2 from `v1_authentication` — GET with a query string.
    const VECTOR_SECRET_GET: &str =
        "M2NkN2EwZGI3NmZmOWRjYTQ4OTc5ZTI0YzM5YjQwOGMgIC0KM2NkN2EwZGI3NmZm";

    fn crypto_data(pay_currency: Option<&str>, network: Option<&str>) -> CryptoData {
        CryptoData {
            pay_currency: pay_currency.map(ToOwned::to_owned),
            network: network.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn sign_post_vector() {
        let signature = payhound_request_signature(
            &Secret::new(VECTOR_SECRET_POST.to_owned()),
            "/api/v1/test",
            123,
            r#"{"attr1": 123, "attr2": "hello"}"#,
        )
        .expect("POST vector must sign");

        assert_eq!(
            signature,
            "f6cd63ea92d66baaa618cbf5ae0cc9c65bed284c93a91ebef079c747a930eb7e\
             497b5874658fb5557b42dfb7937c2fd273fef01205cae6a13ed068f0d0c4f735"
        );
    }

    #[test]
    fn sign_get_query_vector() {
        let signature = payhound_request_signature(
            &Secret::new(VECTOR_SECRET_GET.to_owned()),
            "/api/v1/info",
            4711,
            "first=this+is+a+field&second=was+it+clear+%28already%29%3F",
        )
        .expect("GET vector must sign");

        assert_eq!(
            signature,
            "24c2a83c15581c85de5b180716bd8e86467c089665d6ab51bd6e979815e9e740\
             a74a265d9b2aaee3db9146766583254d64280b1fbdf1e8cf91bf98ef09aff114"
        );
    }

    #[test]
    fn sign_get_empty_body_digest() {
        // UCS's PSync issues a GET with no query string, so the empty-`request_data` branch of the
        // signer is the one that actually runs in production.
        assert_eq!(
            payhound_sha256_hex(b"").expect("empty digest"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn callback_signature_vector() {
        let message = payhound_callback_message("ABCDEFGH", br#"{"attr1": 123, "attr2": "hello"}"#)
            .expect("callback message");

        assert_eq!(
            String::from_utf8(message.clone()).expect("utf8"),
            "ABCDEFGH947753ba472927154c534cf2e4e11de27ed7a9560dc033e77d6cc24ee950ea56"
        );

        let signature = crypto::HmacSha512
            .sign_message(VECTOR_SECRET_POST.as_bytes(), &message)
            .map(hex::encode)
            .expect("callback signature");

        assert_eq!(
            signature,
            "545466159ca3fc8584ab1115b83614f45c2bc12bc596720579668cde289965eb\
             6fe4c3f33ae3fefbd942691aa1cf7b3c1f081740d0ae7d0936ceb7608b670146"
        );
    }

    #[test]
    fn nonce_is_strictly_increasing() {
        let mut previous = next_nonce().expect("first nonce");
        for _ in 0..10_000 {
            let current = next_nonce().expect("nonce");
            assert!(
                current > previous,
                "payhound nonce must strictly increase: {current} followed {previous}"
            );
            previous = current;
        }
    }

    /// Payhound rejects a replayed or out-of-order `X-MB-Nonce`, so the counter must never hand the
    /// same value to two callers even when several flows sign requests at the same instant — which
    /// is the normal case for a server under load, where many threads read an identical
    /// `now_micros`. Distinctness across threads is the property that matters: if it holds, the
    /// stronger per-caller ordering follows from the CAS loop in `next_nonce`.
    #[test]
    fn nonce_is_distinct_across_threads() {
        const THREADS: usize = 8;
        const PER_THREAD: usize = 2_000;

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                std::thread::spawn(|| {
                    (0..PER_THREAD)
                        .map(|_| next_nonce().expect("nonce"))
                        .collect::<Vec<u64>>()
                })
            })
            .collect();

        let mut observed = std::collections::HashSet::new();
        for handle in handles {
            for nonce in handle.join().expect("nonce thread must not panic") {
                assert!(
                    observed.insert(nonce),
                    "payhound nonce {nonce} was handed out to more than one concurrent caller"
                );
            }
        }

        assert_eq!(
            observed.len(),
            THREADS * PER_THREAD,
            "every concurrent payhound nonce must be unique"
        );
    }

    fn sample_request(
        currency: common_enums::Currency,
        minor_amount: i64,
    ) -> PayhoundInvoiceRequest {
        PayhoundInvoiceRequest {
            currency: PayhoundSettlementCurrency::try_from(currency).expect("supported currency"),
            price: PayhoundAmountConvertor::convert(MinorUnit::new(minor_amount), currency)
                .expect("amount conversion"),
            invoice_currency: PayhoundInvoiceCurrency::Btc,
            name: None,
            description: None,
            reference: None,
            callback_url: None,
            success_url: None,
            cancel_url: None,
            notify_email: None,
            share_to_keep_in_btc: None,
        }
    }

    #[test]
    fn amount_is_major_unit_string() {
        let body = serde_json::to_string(&sample_request(common_enums::Currency::EUR, 1000))
            .expect("serialize");
        assert!(
            body.contains(r#""price":"10.00""#),
            "expected a major-unit decimal string, got {body}"
        );
        assert!(
            !body.contains(r#""price":1000"#),
            "minor units must never be sent to Payhound, got {body}"
        );
    }

    #[test]
    fn optional_fields_are_omitted_not_null() {
        let body = serde_json::to_string(&sample_request(common_enums::Currency::EUR, 26645))
            .expect("serialize");
        assert!(
            !body.contains("null"),
            "absent optional fields must be omitted, not serialized as null: {body}"
        );
        assert_eq!(
            body,
            r#"{"currency":"EUR","price":"266.45","invoice_currency":"BTC"}"#
        );
    }

    #[test]
    fn crypto_currency_resolution() {
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("USDT_TRC20"), None)).expect("symbol"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("usdt_trc20"), None))
                .expect("case-insensitive symbol"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("USDT"), Some("TRC20")))
                .expect("composed symbol"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("USDT_TRC20"), Some("TRC20")))
                .expect("agreeing network"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert!(
            resolve_invoice_currency(&crypto_data(Some("USDT_TRC20"), Some("ERC20"))).is_err(),
            "a conflicting network must be rejected, never silently ignored"
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("BTC"), None)).expect("btc"),
            PayhoundInvoiceCurrency::Btc
        );

        let unsupported = resolve_invoice_currency(&crypto_data(Some("DOGE"), None))
            .expect_err("DOGE is not a Payhound asset");
        match unsupported.current_context() {
            IntegrationError::NotSupported { context, .. } => assert!(
                context
                    .additional_context
                    .as_deref()
                    .is_some_and(|detail| detail.contains("USDT_TRC20")),
                "the error context must enumerate the supported symbols, got {context:?}"
            ),
            other => panic!("expected NotSupported, got {other:?}"),
        }

        let missing = resolve_invoice_currency(&crypto_data(None, None))
            .expect_err("pay_currency is required");
        match missing.current_context() {
            IntegrationError::MissingRequiredField { field_name, .. } => assert_eq!(
                *field_name, "crypto_data.pay_currency",
                "the missing-field error must name the caller-facing request path"
            ),
            other => panic!("expected MissingRequiredField, got {other:?}"),
        }
    }

    #[test]
    fn status_mapping() {
        use common_enums::AttemptStatus;
        let cases = [
            (
                PayhoundInvoiceStatus::Pending,
                AttemptStatus::AuthenticationPending,
            ),
            (PayhoundInvoiceStatus::Completed, AttemptStatus::Charged),
            (PayhoundInvoiceStatus::Overpaid, AttemptStatus::Charged),
            (PayhoundInvoiceStatus::Underpaid, AttemptStatus::Pending),
            (PayhoundInvoiceStatus::Aborted, AttemptStatus::Failure),
            (PayhoundInvoiceStatus::Timeout, AttemptStatus::Failure),
            (PayhoundInvoiceStatus::Unknown, AttemptStatus::Pending),
        ];
        for (payhound_status, expected) in cases {
            assert_eq!(
                AttemptStatus::from(payhound_status),
                expected,
                "unexpected mapping for {payhound_status:?}"
            );
        }
    }

    #[test]
    fn unknown_status_deserializes_to_unknown() {
        let parsed: PayhoundInvoiceStatus =
            serde_json::from_str(r#""brand_new""#).expect("unknown status must not fail to parse");
        assert_eq!(parsed, PayhoundInvoiceStatus::Unknown);
    }

    #[test]
    fn invoice_url_relative_is_resolved() {
        let resolved = build_invoice_redirect("/invoices/abc").expect("relative url");
        assert_eq!(resolved.as_str(), "https://pay.payhound.com/invoices/abc");
    }

    #[test]
    fn invoice_url_absolute_passes_through() {
        let resolved =
            build_invoice_redirect("https://pay.payhound.com/invoices/abc").expect("absolute url");
        assert_eq!(resolved.as_str(), "https://pay.payhound.com/invoices/abc");
    }

    #[test]
    fn error_envelope_maps_status_as_code() {
        let parsed: PayhoundErrorResponse =
            serde_json::from_str(r#"{"message":"Invalid signature"}"#).expect("error envelope");
        assert_eq!(parsed.message, "Invalid signature");
        assert_eq!(payhound_error_code(403), "403");
    }

    /// The real sandbox `201` body, verbatim.
    const LIVE_201_BODY: &str = r#"{
        "id": "9d8c0ade8d1f78048b4905821e44691b",
        "status": "pending",
        "address": "mrpNRa5PydTcTQBV2i5BpWaqN1DgofkoZB",
        "alt_addresses": [{"address": "tb1qexample", "type": "base58", "default": true}],
        "merchant_currency": "EUR", "merchant_amount": "20.00",
        "invoice_currency": "BTC", "invoice_amount": "0.00100000",
        "paid_currency": "BTC", "paid_amount": "0.0",
        "pending_currency": "BTC", "pending_amount": "0.0",
        "share_to_keep_in_btc": 0,
        "name": "Test Invoice", "description": "Lorem Ipsum Test Invoice",
        "reference": "TSTINCUSTOMREF",
        "sender_addresses": [],
        "invoice_url": "/invoices/9d8c0ade8d1f78048b4905821e44691b",
        "callback_url": null, "success_url": null, "cancel_url": null, "notify_email": null,
        "create_time": 1560168720.0, "valid_until_time": 1787591345.0,
        "max_risk_rating_of_related_txes": null
    }"#;

    #[test]
    fn float_timestamps_deserialize() {
        let parsed: PayhoundInvoiceResponse =
            serde_json::from_str(LIVE_201_BODY).expect("float timestamps must parse");
        assert_eq!(parsed.create_time, Some(1_560_168_720.0));
        assert_eq!(parsed.valid_until_time, Some(1_787_591_345.0));
    }

    #[test]
    fn live_201_body_round_trips() {
        let parsed: PayhoundInvoiceResponse =
            serde_json::from_str(LIVE_201_BODY).expect("live 201 body must parse");
        assert_eq!(parsed.id, "9d8c0ade8d1f78048b4905821e44691b");
        assert_eq!(parsed.status, PayhoundInvoiceStatus::Pending);
        assert_eq!(
            parsed
                .alt_addresses
                .as_ref()
                .map(|addresses| addresses.len()),
            Some(1)
        );
        assert!(parsed.max_risk_rating_of_related_txes.is_none());
        assert_eq!(
            parsed.invoice_url.as_deref(),
            Some("/invoices/9d8c0ade8d1f78048b4905821e44691b")
        );
    }

    #[test]
    fn capture_method_manual_is_rejected() {
        for capture_method in [
            common_enums::CaptureMethod::Manual,
            common_enums::CaptureMethod::ManualMultiple,
            common_enums::CaptureMethod::Scheduled,
        ] {
            let error = ensure_supported_capture_method(Some(capture_method))
                .expect_err("Payhound cannot settle or release a manual-capture authorization");
            assert!(
                matches!(
                    error.current_context(),
                    IntegrationError::CaptureMethodNotSupported { .. }
                ),
                "expected CaptureMethodNotSupported for {capture_method:?}, got {error:?}"
            );
        }
        for capture_method in [
            Some(common_enums::CaptureMethod::Automatic),
            Some(common_enums::CaptureMethod::SequentialAutomatic),
            None,
        ] {
            assert!(ensure_supported_capture_method(capture_method).is_ok());
        }
    }

    #[test]
    fn non_crypto_payment_method_is_rejected() {
        let error = invoice_currency_from_payment_method(&PaymentMethodData::Card(Card::<
            DefaultPCIHolder,
        >::default(
        )))
        .expect_err("Payhound accepts only PaymentMethodData::Crypto");
        match error.current_context() {
            IntegrationError::NotSupported {
                connector, context, ..
            } => {
                assert_eq!(*connector, PAYHOUND_CONNECTOR_DISPLAY);
                assert!(
                    context
                        .additional_context
                        .as_deref()
                        .is_some_and(|detail| detail.contains("hosted crypto invoice gateway")),
                    "the rejection must carry Payhound-specific context, got {context:?}"
                );
            }
            other => panic!("expected NotSupported, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_settlement_currency_is_rejected() {
        let error = PayhoundSettlementCurrency::try_from(common_enums::Currency::AFN)
            .expect_err("AFN is not a Payhound settlement currency");
        match error.current_context() {
            IntegrationError::CurrencyNotSupported {
                message, context, ..
            } => {
                assert_eq!(message, "AFN");
                assert!(
                    context
                        .additional_context
                        .as_deref()
                        .is_some_and(|detail| detail.contains("EUR")),
                    "the error context must enumerate the supported settlement currencies, got \
                     {context:?}"
                );
            }
            other => panic!("expected CurrencyNotSupported, got {other:?}"),
        }
    }
}
