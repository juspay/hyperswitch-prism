//! Transformers for the Global Payments **Ecommerce XML API** (legacy *Realex Payments*
//! "XML API" / "Remote API").
//!
//! This is **not** the GP-API JSON product handled by the `globalpay` connector. Everything here
//! speaks XML over a single CGI endpoint and is authenticated by a two-stage SHA-1 digest.
//!
//! Reference: `grace/rulesbook/codegen/references/globalpayments_realex/technical_specification.md`

use base64::Engine;
use common_enums::{
    AttemptStatus, AuthenticationType, CaptureMethod, CardNetwork, CountryAlpha2, RefundStatus,
    TransactionStatus,
};
use common_utils::{
    consts,
    pii::SecretSerdeValue,
    types::{MinorUnit, SemanticVersion},
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId as DomainResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation},
    router_response_types::RedirectForm,
    utils::{get_card_issuer, CardIssuer},
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug, str::FromStr};

use super::super::macros::GetSoapXml;
use super::GlobalpaymentsRealexRouterData;
use crate::types::ResponseRouterData;

// =============================================================================
// CONSTANTS
// =============================================================================

/// Root element `type` attribute for the Authorize flow.
pub const REQUEST_TYPE_AUTH: &str = "auth";
/// Root element `type` attribute for the Capture flow. Same endpoint, same envelope — only the
/// `type` attribute and the body elements differ (tech spec §12.1).
pub const REQUEST_TYPE_SETTLE: &str = "settle";
/// Root element `type` attribute for the Void flow. Same endpoint and envelope again — a `void`
/// reverses an authorization that has not been settled yet (tech spec §12.2).
pub const REQUEST_TYPE_VOID: &str = "void";
/// Root element `type` attribute for the Refund flow. A `rebate` returns funds to the cardholder
/// against an existing, captured transaction; the *unreferenced* credit-to-a-card request is
/// `type="credit"`, which is a different request type with a different password and is out of
/// scope (tech spec §12.3).
pub const REQUEST_TYPE_REBATE: &str = "rebate";
/// Root element `type` attribute for the PSync flow. A `query` reads back the state of an existing
/// transaction; it is undocumented but verified live, and it is the only status-enquiry operation
/// this API exposes (tech spec §12.4).
pub const REQUEST_TYPE_QUERY: &str = "query";
/// Ecommerce channel. The other documented value is `MOTO`, which is out of scope.
pub const CHANNEL_ECOM: &str = "ECOM";
/// `<cvn><presind>` — `1` means "CVN present on the card and supplied by the cardholder".
pub const CVN_PRESENCE_INDICATOR_PRESENT: u8 = 1;
/// Cardholder name is mandatory on the wire; RealEx rejects an empty `<chname>`.
pub const DEFAULT_CARDHOLDER_NAME: &str = "Cardholder";
/// `<orderid>` maximum length accepted by the gateway.
const ORDER_ID_MAX_LEN: usize = 50;
/// Success result code. Every other value is a decline or an error (tech spec §9).
const RESULT_SUCCESS: &str = "00";
/// `508 Original transaction not found.` — on a `type="query"` this means the gateway holds no
/// transaction under the `<orderid>` we sent. On the `_rebate_` leg it is the "no successful refund
/// exists for this order" signal (tech spec §12.6.3, controls D / E / G).
const RESULT_ORIGINAL_TRANSACTION_NOT_FOUND: &str = "508";

// =============================================================================
// AUTH
// =============================================================================

/// RealEx credentials, sourced from the `MultiAuthKey` connector auth type.
///
/// | UCS field    | RealEx name                | Used by                          |
/// |--------------|----------------------------|----------------------------------|
/// | `api_key`    | Shared Secret              | every request/response digest    |
/// | `key1`       | Merchant ID                | `<merchantid>` + digest          |
/// | `key2`       | Account (sub-account)      | `<account>`                      |
/// | `api_secret` | Refund / Rebate password   | `<refundhash>` on `rebate` only  |
#[derive(Debug, Clone)]
pub struct GlobalpaymentsRealexAuthType {
    pub shared_secret: Secret<String>,
    pub merchant_id: Secret<String>,
    pub account: Secret<String>,
    /// The **Rebate** password. Consumed by the Refund flow alone, as the sole input to
    /// `<refundhash>` (see [`build_refund_password_hash`]) — never by `<sha1hash>`, which is
    /// always keyed on `shared_secret`.
    pub refund_password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GlobalpaymentsRealexAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::GlobalpaymentsRealex {
                shared_secret,
                merchant_id,
                account,
                refund_password,
                ..
            } => Ok(Self {
                shared_secret: shared_secret.clone(),
                merchant_id: merchant_id.clone(),
                account: account.clone(),
                refund_password: refund_password.clone(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Expected ConnectorSpecificConfig::GlobalpaymentsRealex containing \
                         shared_secret, merchant_id, account and refund_password"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })),
        }
    }
}

// =============================================================================
// CONNECTOR METADATA (carried from Authorize to the follow-up flows)
// =============================================================================

/// The values a follow-up `settle` / `void` / `rebate` / `query` needs that are **not** carried by
/// any standard UCS field.
///
/// RealEx requires the **original** `<orderid>` — and, for `rebate`, the original `<authcode>` —
/// verbatim, and it requires the pasref the original `auth` minted for `void` and `rebate`. None of
/// them can be regenerated at capture time (a fresh timestamp-derived order id is rejected), so
/// Authorize publishes them as `connector_metadata`, which the gRPC layer surfaces as
/// `connector_feature_data` on the Authorize response and accepts back on every follow-up request.
///
/// The auth pasref is carried here rather than read from `connector_transaction_id` because that
/// field stops being the auth pasref the moment a manual capture happens: a `settle` mints a **new**
/// pasref, which becomes the Capture response's `connector_transaction_id`, and a `void` / `rebate`
/// sent with it answers `508 Original transaction not found.` (see [`Self::auth_pasref`]).
///
/// Tech spec §14.5 ("what to persist").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalpaymentsRealexPaymentMetadata {
    /// The `<orderid>` **we sent** on the original `auth` (not the gateway's echo, which is
    /// prefixed with `_settle_` on some error documents).
    pub orderid: String,
    /// `<authcode>` from the original `auth` response. Optional because a `5xx` auth carries none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authcode: Option<String>,
    /// The `<pasref>` a successful `void` minted, republished by the Void flow.
    ///
    /// **This is the only way PSync can tell that a payment was voided.** A `query` echoes the
    /// authorization leg forever and is completely blind to a void (tech spec §12.4.5), so without
    /// this marker a voided payment syncs as `Authorized` / `Charged`. See
    /// [`map_psync_attempt_status`].
    ///
    /// It carries the void leg's gateway reference rather than a bare boolean because that
    /// reference is independently useful — it is the id of the `_void_<orderid>` transaction the
    /// gateway stores (tech spec §12.4.6) — and costs nothing extra to persist.
    ///
    /// Additive and optional on purpose: metadata written by the already-shipped Authorize,
    /// Capture and Refund flows carries no such field and must keep deserializing unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub void_pasref: Option<String>,
    /// The `<pasref>` the **original `auth`** minted — the only pasref `void` and `rebate` accept.
    ///
    /// `connector_transaction_id` cannot be trusted for this after a manual capture: a `settle`
    /// mints its own pasref, UCS surfaces *that* one as the Capture response's transaction id, and
    /// a caller that persists it then sends it back on the reversal. Verified live: `void` and
    /// `rebate` both answer `508 Original transaction not found.` for the settle-minted pasref and
    /// `00` for this one (tech spec §12.2, §12.3). With auto-capture there is no separate settle,
    /// so the two values coincide and nothing was ever wrong there — which is why only the
    /// manual-capture path was broken.
    ///
    /// Additive and optional on purpose: metadata written before this field existed carries no
    /// `auth_pasref`, and those payments must keep working through the
    /// `connector_transaction_id` fallback in [`resolve_reference_pasref`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_pasref: Option<String>,
}

// =============================================================================
// HASHING (tech spec §3)
// =============================================================================

/// Lowercase hex SHA-1 of the UTF-8 bytes of `input`.
///
/// SHA-1 is dictated by the RealEx wire protocol; it is not used here as a security primitive
/// of our own choosing.
fn sha1_hex(input: &str) -> String {
    hex::encode(ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        input.as_bytes(),
    ))
}

/// The two-stage RealEx digest:
///
/// ```text
/// stage1 = SHA1_hex(field1 "." field2 "." … "." fieldN)
/// digest = SHA1_hex(stage1 "." sharedSecret)
/// ```
///
/// `fields` are joined verbatim; an absent field contributes an empty string but still emits its
/// separator, because the dots are positional.
fn realex_digest(fields: &[&str], shared_secret: &str) -> String {
    let stage1 = sha1_hex(&fields.join("."));
    sha1_hex(&format!("{stage1}.{shared_secret}"))
}

/// Digest for `type="auth"`: `timestamp.merchantid.orderid.amount.currency.cardnumber`.
///
/// `<account>`, `<channel>`, `<autosettle>` and the whole `<mpi>` block are **not** hashed
/// (verified live — tech spec §3.2).
#[allow(clippy::too_many_arguments)]
fn build_auth_request_hash(
    timestamp: &str,
    merchant_id: &str,
    order_id: &str,
    amount: &str,
    currency: &str,
    card_number: &str,
    shared_secret: &str,
) -> Secret<String> {
    Secret::new(realex_digest(
        &[
            timestamp,
            merchant_id,
            order_id,
            amount,
            currency,
            card_number,
        ],
        shared_secret,
    ))
}

/// Digest for the request types that reference an existing transaction rather than a card —
/// `settle` (Capture), `void` (Void) and `rebate` (Refund).
///
/// The blueprint is the same six positional slots as `auth`
/// (`timestamp.merchantid.orderid.amount.currency.cardnumber`), and every slot must mirror the
/// body **exactly**. The card-number slot is always empty here (no card data is ever sent on a
/// reference request), leaving these three shapes:
///
/// | Flow | `<amount>` on the wire | Stage-1 string |
/// |---|---|---|
/// | `void` | absent | `timestamp.merchantid.orderid...` |
/// | `settle` | present, **no** `currency` attribute | `timestamp.merchantid.orderid.<amount>..` |
/// | `rebate` | present, **with** a `currency` attribute | `timestamp.merchantid.orderid.<amount>.<currency>.` |
///
/// The `settle` and `void` variants were **verified live** against the sandbox while implementing
/// those flows; the documentation's partial-settle worked example is a placeholder and could not be
/// used (tech spec §16 item 6). Filling the amount slot while omitting `<amount>` — or filling the
/// currency slot on a `settle` — is rejected with `505 sha1hash incorrect`.
///
/// `rebate` is the only reference request that carries a `currency`, and it *always* carries one,
/// so it always fills both the amount and the currency slot (tech spec §12.3). Verified live.
fn build_reference_request_hash(
    timestamp: &str,
    merchant_id: &str,
    order_id: &str,
    amount: Option<&str>,
    currency: Option<&str>,
    shared_secret: &str,
) -> Secret<String> {
    Secret::new(realex_digest(
        &[
            timestamp,
            merchant_id,
            order_id,
            amount.unwrap_or_default(),
            // `settle` and `void` carry no currency attribute, so they pass `None` and the slot
            // stays empty; `rebate` fills it. No reference request ever carries a card number, so
            // the last slot is always empty — but its separator is still emitted.
            currency.unwrap_or_default(),
            "",
        ],
        shared_secret,
    ))
}

/// The second, **independent** digest that only `rebate` (and the out-of-scope `credit`) carries.
///
/// ```text
/// refundhash = SHA1_hex(plaintext rebate password)
/// ```
///
/// One stage, 40 lowercase hex characters. It is **not** the two-stage
/// [`realex_digest`] construction, it is **not** salted or concatenated with the Shared Secret
/// (`api_key`), and it contains **no** transaction fields — it is a bare SHA-1 of the rebate
/// password, which UCS holds in `api_secret` and this connector exposes as
/// [`GlobalpaymentsRealexAuthType::refund_password`]. It is therefore constant for the account.
///
/// Getting this wrong fails with `505 The refund password you entered was incorrect.`, which is a
/// *different message* from the `505 sha1hash incorrect …` a bad [`build_reference_request_hash`]
/// produces — the `result` code alone cannot tell the two apart, so `<message>` is surfaced
/// verbatim in `error_message` (tech spec §12.3).
///
/// Illustrative vector (not an account credential): `SHA1("password")` =
/// `5baa61e4c9b93f3f0682250b6cf8331b7ee68fd8`.
fn build_refund_password_hash(refund_password: &str) -> Secret<String> {
    Secret::new(sha1_hex(refund_password))
}

/// Response digest blueprint: `timestamp.merchantid.orderid.result.message.pasref.authcode`,
/// computed from the values in the **response** — in particular the response's own `timestamp`,
/// which is server-local and routinely differs from the request's (tech spec §3.4).
fn build_response_hash(
    response: &GlobalpaymentsRealexPaymentsResponse,
    shared_secret: &str,
) -> String {
    realex_digest(
        &[
            response.timestamp.as_deref().unwrap_or_default(),
            response.merchantid.as_deref().unwrap_or_default(),
            response.orderid.as_deref().unwrap_or_default(),
            response.result.as_str(),
            response.message.as_deref().unwrap_or_default(),
            response.pasref.as_deref().unwrap_or_default(),
            response.authcode.as_deref().unwrap_or_default(),
        ],
        shared_secret,
    )
}

/// Outcome of verifying `<sha1hash>` on a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HashVerification {
    /// The response carried a `<sha1hash>` and it matched.
    Verified,
    /// The response carried no `<sha1hash>` — this is normal for `5xx` error documents, which are
    /// a smaller shape with no digest (tech spec §10.2). Verification is *skipped*, not failed.
    Skipped,
    /// A digest was present and did not match what we computed.
    Mismatch,
}

fn verify_response_hash(
    response: &GlobalpaymentsRealexPaymentsResponse,
    shared_secret: &str,
) -> HashVerification {
    match response.sha1hash.as_deref() {
        None => HashVerification::Skipped,
        Some(returned) => {
            let computed = build_response_hash(response, shared_secret);
            if computed.eq_ignore_ascii_case(returned) {
                HashVerification::Verified
            } else {
                HashVerification::Mismatch
            }
        }
    }
}

// -----------------------------------------------------------------------------
// 3DS2 JSON API digests (separate API — see `super::three_ds_two`)
// -----------------------------------------------------------------------------

/// The `Authorization: securehash <digest>` blueprints of the Global Payments **3DS2 JSON API**.
///
/// The JSON API reuses the XML API's two-stage SHA-1 primitive ([`realex_digest`]) and the same
/// Shared Secret, but **nothing else**: different field lists, a different timestamp format
/// ([`current_3ds2_timestamp`]) and a different card-scheme vocabulary
/// (`MASTERCARD`, not the XML `MC` — see [`map_card_type`] and
/// [`super::three_ds_two::map_3ds2_scheme`]).
///
/// Blueprints (source: `sources/source_2_3d_secure_two.md` § "Generate hash"):
///
/// | Call | Stage-1 string |
/// |---|---|
/// | `POST /3ds2/protocol-versions` | `request_timestamp.merchant_id.cardnumber` |
/// | `POST /3ds2/authentications` | `request_timestamp.merchant_id.cardnumber.server_trans_id` |
/// | `GET  /3ds2/authentications/{sid}` | `request_timestamp.merchant_id.server_trans_id` |
///
/// The `request_timestamp` slot must be **byte-identical** to the one the request body (or query
/// string) carries; see the blocking comments on each flow's `build_request_v2`.
#[derive(Debug, Clone, Copy)]
pub enum Gp3ds2Digest<'a> {
    /// *Check version* — `POST /3ds2/protocol-versions`.
    CheckVersion {
        timestamp: &'a str,
        merchant_id: &'a str,
        card_number: &'a str,
    },
    /// *Initiate authentication* — `POST /3ds2/authentications`.
    InitiateAuthentication {
        timestamp: &'a str,
        merchant_id: &'a str,
        card_number: &'a str,
        server_trans_id: &'a str,
    },
    /// *Obtain authentication data* — `GET /3ds2/authentications/{server_trans_id}`.
    ObtainAuthenticationData {
        timestamp: &'a str,
        merchant_id: &'a str,
        server_trans_id: &'a str,
    },
}

/// Build the value for the 3DS2 `Authorization: securehash …` header.
///
/// Deliberately shares only [`realex_digest`] with the XML flows — see [`Gp3ds2Digest`].
pub fn build_3ds2_securehash(digest: Gp3ds2Digest<'_>, shared_secret: &str) -> Secret<String> {
    let fields: Vec<&str> = match digest {
        Gp3ds2Digest::CheckVersion {
            timestamp,
            merchant_id,
            card_number,
        } => vec![timestamp, merchant_id, card_number],
        Gp3ds2Digest::InitiateAuthentication {
            timestamp,
            merchant_id,
            card_number,
            server_trans_id,
        } => vec![timestamp, merchant_id, card_number, server_trans_id],
        Gp3ds2Digest::ObtainAuthenticationData {
            timestamp,
            merchant_id,
            server_trans_id,
        } => vec![timestamp, merchant_id, server_trans_id],
    };

    Secret::new(realex_digest(&fields, shared_secret))
}

// =============================================================================
// FIELD FORMATTING HELPERS (tech spec §5)
// =============================================================================

/// `YYYYMMDDHHMMSS`, UTC, no separators. The gateway rejects anything more than 86400 s away from
/// its own clock. The same string is used for both the `@timestamp` attribute and the digest.
fn current_timestamp() -> Result<String, error_stack::Report<IntegrationError>> {
    let now = time::OffsetDateTime::now_utc();
    let format = time::macros::format_description!("[year][month][day][hour][minute][second]");
    now.format(&format)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Failed to format the current UTC time as YYYYMMDDHHMMSS for the \
                     GlobalpaymentsRealex request timestamp"
                        .to_string(),
                ),
                ..Default::default()
            },
        })
}

/// The 3DS2 JSON API's `request_timestamp`: `yyyy-MM-ddTHH:mm:ss.SSSSSS`, UTC, **no** trailing
/// `Z` and no offset (the documented format is `yyyy-MM-ddTHH:mm:ss.SSS` with 3–6 fractional
/// digits; every worked example in the vendor docs uses 6).
///
/// Completely unrelated to the XML API's [`current_timestamp`] (`YYYYMMDDHHMMSS`). The two APIs
/// are never called with the same string, and this value additionally appears verbatim inside the
/// `securehash` — see [`Gp3ds2Digest`].
pub fn current_3ds2_timestamp() -> Result<String, error_stack::Report<IntegrationError>> {
    let now = time::OffsetDateTime::now_utc();
    let format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]"
    );
    now.format(&format)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Failed to format the current UTC time as yyyy-MM-ddTHH:mm:ss.SSSSSS for the \
                     GlobalpaymentsRealex 3DS2 request_timestamp"
                        .to_string(),
                ),
                ..Default::default()
            },
        })
}

/// `<orderid>` accepts `[a-zA-Z0-9_-]` only, 1–50 characters, and must be unique per attempt
/// (a reuse returns `501 … already been processed`). Anything outside the charset is replaced
/// with `-`, then the value is truncated.
pub fn sanitize_order_id(reference: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    let sanitized: String = reference
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(ORDER_ID_MAX_LEN)
        .collect();

    if sanitized.is_empty() {
        return Err(report!(IntegrationError::InvalidDataFormat {
            field_name: "connector_request_reference_id",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "GlobalpaymentsRealex <orderid> cannot be empty; it must be 1-50 characters \
                     of [a-zA-Z0-9_-]"
                        .to_string(),
                ),
                ..Default::default()
            },
        }));
    }

    Ok(sanitized)
}

/// RealEx expects the integer amount in the smallest unit of the currency, with no leading zeros,
/// no decimal point and no separators.
///
/// One documented exception: **JPY amounts must be multiplied by 100 before submission**, unlike
/// every other zero-exponent currency (tech spec §5.4). This could not be exercised against the
/// sandbox account, so it is implemented from the documentation and flagged here.
pub fn format_amount(
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let raw = amount.get_amount_as_i64();

    let adjusted = if currency == common_enums::Currency::JPY {
        raw.checked_mul(100).ok_or_else(|| {
            report!(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Overflow while applying the GlobalpaymentsRealex JPY x100 rule"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?
    } else {
        raw
    };

    if adjusted <= 0 {
        return Err(report!(IntegrationError::InvalidDataFormat {
            field_name: "amount",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "GlobalpaymentsRealex rejects zero or negative amounts (508 Zero, negative or \
                     insufficient amount specified)"
                        .to_string(),
                ),
                ..Default::default()
            },
        }));
    }

    Ok(adjusted.to_string())
}

/// The `<card><type>` enum. Case-sensitive uppercase; note `MC`, **not** `MASTERCARD` — the 3DS2
/// JSON API uses a different spelling and the two tables must not be shared (tech spec §5.6).
pub(super) fn map_card_type<T>(
    card: &Card<T>,
) -> Result<&'static str, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    // Prefer the network supplied by the caller; fall back to BIN detection. Never default to a
    // brand — production cross-validates `<type>` against the PAN.
    if let Some(network) = card.card_network.as_ref() {
        return match network {
            CardNetwork::Visa => Ok("VISA"),
            CardNetwork::Mastercard => Ok("MC"),
            CardNetwork::AmericanExpress => Ok("AMEX"),
            CardNetwork::DinersClub => Ok("DINERS"),
            CardNetwork::Discover => Ok("DISCOVER"),
            CardNetwork::JCB => Ok("JCB"),
            unsupported => Err(report!(IntegrationError::NotImplemented(
                format!("Card network {unsupported:?} is not supported by GlobalpaymentsRealex"),
                IntegrationErrorContext::default(),
            ))),
        };
    }

    match get_card_issuer(card.card_number.peek())? {
        CardIssuer::Visa => Ok("VISA"),
        CardIssuer::Master => Ok("MC"),
        CardIssuer::AmericanExpress => Ok("AMEX"),
        CardIssuer::DinersClub => Ok("DINERS"),
        CardIssuer::Discover => Ok("DISCOVER"),
        CardIssuer::JCB => Ok("JCB"),
        unsupported => Err(report!(IntegrationError::NotImplemented(
            format!("Card issuer {unsupported} is not supported by GlobalpaymentsRealex"),
            IntegrationErrorContext::default(),
        ))),
    }
}

// =============================================================================
// AUTOSETTLE (tech spec §5.7)
// =============================================================================

/// `<autosettle flag="…"/>` is **mandatory** for `type="auth"` — omitting it returns
/// `502 Mandatory Fields missing: [/request/autosettle]`.
///
/// The flag also determines how `result=00` maps to an `AttemptStatus`, so it is threaded through
/// to the response transformer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum GlobalpaymentsRealexAutoSettleFlag {
    /// `flag="1"` — authorize and add to the next settlement file (auth + capture).
    #[serde(rename = "1")]
    AutoSettle,
    /// `flag="0"` — authorize only; a later `type="settle"` is required.
    #[serde(rename = "0")]
    DelayedSettle,
}

impl TryFrom<Option<CaptureMethod>> for GlobalpaymentsRealexAutoSettleFlag {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(capture_method: Option<CaptureMethod>) -> Result<Self, Self::Error> {
        match capture_method {
            None | Some(CaptureMethod::Automatic) | Some(CaptureMethod::SequentialAutomatic) => {
                Ok(Self::AutoSettle)
            }
            Some(CaptureMethod::Manual) => Ok(Self::DelayedSettle),
            // `MULTI` is not enabled on the sandbox account (503 Request type [multisettle] not
            // allowed for this merchant), so multiple partial captures are not claimed here.
            Some(capture_method) => Err(report!(IntegrationError::NotImplemented(
                format!(
                    "Capture method {capture_method:?} is not supported by GlobalpaymentsRealex"
                ),
                IntegrationErrorContext::default(),
            ))),
        }
    }
}

// =============================================================================
// REQUEST (tech spec §4, §6, §7)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexPaymentsRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    pub channel: String,
    pub orderid: String,
    pub amount: GlobalpaymentsRealexAmount,
    pub card: GlobalpaymentsRealexCard,
    pub autosettle: GlobalpaymentsRealexAutoSettle,
    /// 3DS2 authentication result, forwarded verbatim. Absent for non-3DS payments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mpi: Option<GlobalpaymentsRealexMpi>,
    pub sha1hash: Secret<String>,
}

/// `<amount currency="EUR">1001</amount>` — text content plus a currency attribute.
#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexAmount {
    #[serde(rename = "@currency")]
    pub currency: String,
    #[serde(rename = "$text")]
    pub value: String,
}

/// `<autosettle flag="1"/>` — attribute only, no text content.
#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexAutoSettle {
    #[serde(rename = "@flag")]
    pub flag: GlobalpaymentsRealexAutoSettleFlag,
}

#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexCard {
    pub number: Secret<String>,
    /// `MMYY` — four digits, no separator.
    pub expdate: Secret<String>,
    pub chname: Secret<String>,
    #[serde(rename = "type")]
    pub card_type: String,
    /// Omitted entirely when no CVC is available; an empty `<cvn>` is rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvn: Option<GlobalpaymentsRealexCvn>,
}

#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexCvn {
    pub number: Secret<String>,
    /// Mandatory whenever `<cvn>` is present.
    pub presind: u8,
}

/// The 3DS2 `<mpi>` block. Populated from an authentication that happened **before** this call —
/// this connector never drives the separate 3DS2 JSON API itself.
///
/// `<exempt_status>` is deliberately not modelled: the sandbox account returns
/// `508 Exemption is not configured for this merchant.` whenever it is sent.
#[derive(Debug, Serialize)]
pub struct GlobalpaymentsRealexMpi {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_trans_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_value: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_version: Option<String>,
}

impl GetSoapXml for GlobalpaymentsRealexPaymentsRequest {
    fn to_soap_xml(&self) -> String {
        // On a serialization failure we emit a minimal, well-formed document rather than panicking;
        // the gateway answers it with `502 Mandatory Fields missing`, which surfaces cleanly.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex auth request to XML"
            );
            "<request/>".to_string()
        })
    }
}

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlobalpaymentsRealexPaymentsRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
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
        let request = &router_data.request;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by GlobalpaymentsRealex".to_string(),
                    IntegrationErrorContext::default(),
                )))
            }
        };

        // Fail closed before anything else: a three_ds payment without a liability-shifting
        // authentication result must never reach the gateway as a bare `auth`.
        ensure_liability_shift(router_data.resource_common_data.auth_type, request)?;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;

        // All six digest inputs must be byte-identical to what ends up on the wire, so build the
        // strings once and reuse them for both the body and the hash.
        let timestamp = current_timestamp()?;
        let order_id = sanitize_order_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        let amount = format_amount(request.minor_amount, request.currency)?;
        let currency = request.currency.to_string();
        let card_number = card.card_number.peek().to_string();
        let merchant_id = auth.merchant_id.clone().expose();

        let sha1hash = build_auth_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            &amount,
            &currency,
            &card_number,
            auth.shared_secret.peek(),
        );

        // A CVC of "" means the caller had none; sending an empty `<cvn>` is rejected, so the whole
        // element is dropped in that case (verified live: accepted, result 00).
        let cvn = Some(card.card_cvc.clone())
            .filter(|cvc| !cvc.peek().is_empty())
            .map(|cvc| GlobalpaymentsRealexCvn {
                number: cvc,
                presind: CVN_PRESENCE_INDICATOR_PRESENT,
            });

        let card_element = GlobalpaymentsRealexCard {
            number: Secret::new(card_number),
            expdate: card.get_expiry_date_as_mmyy()?,
            chname: card
                .get_optional_cardholder_name()
                .filter(|name| !name.peek().trim().is_empty())
                .unwrap_or_else(|| Secret::new(DEFAULT_CARDHOLDER_NAME.to_string())),
            card_type: map_card_type(card)?.to_string(),
            cvn,
        };

        Ok(Self {
            request_type: REQUEST_TYPE_AUTH.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            channel: CHANNEL_ECOM.to_string(),
            orderid: order_id,
            amount: GlobalpaymentsRealexAmount {
                currency,
                value: amount,
            },
            card: card_element,
            autosettle: GlobalpaymentsRealexAutoSettle {
                flag: GlobalpaymentsRealexAutoSettleFlag::try_from(request.capture_method)?,
            },
            mpi: build_mpi(request),
            sha1hash,
        })
    }
}

/// Builds the `<mpi>` block from an authentication that already completed elsewhere.
///
/// Returns `None` when there is nothing to forward — sending an empty `<mpi>` would be pointless
/// and the digest is unaffected either way.
/// Refuses to build an `auth` for a payment the merchant asked to be 3DS-authenticated unless a
/// **liability-shifting** authentication result is actually attached.
///
/// Without this guard the connector fails *open*. [`build_mpi`] returns `None` whenever
/// `authentication_data` is absent, and an absent `<mpi>` is not an error to the gateway — it is
/// simply a non-3DS authorization. So a cardholder who failed or abandoned the challenge, or a
/// 3DS2 leg that errored, would be authorized as if no authentication had ever been requested,
/// with the liability that implies. Whether the caller's own orchestration stops first is not
/// something this connector can verify, and it must not be the only thing standing between a
/// failed challenge and a captured payment.
///
/// `Y` (authenticated) and `A` (attempted — the issuer's proof of attempted authentication) both
/// carry liability shift and are accepted. Every other `transStatus` — `N`, `U`, `R`, `C`, `D`,
/// `I` — does not, and is refused here rather than silently downgraded.
///
/// A `NoThreeDs` payment is untouched: it never had an `<mpi>` to begin with.
fn ensure_liability_shift<T: PaymentMethodDataTypes>(
    auth_type: AuthenticationType,
    request: &PaymentsAuthorizeData<T>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    if auth_type != AuthenticationType::ThreeDs {
        return Ok(());
    }

    let refuse = |detail: &str| {
        report!(IntegrationError::MissingRequiredField {
            field_name: "authentication_data",
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "GlobalpaymentsRealex refuses to send a non-3DS <auth> for a payment requested \
                     as three_ds: {detail}. Complete the 3DS2 authentication legs first, or submit \
                     the payment as no_three_ds if authentication is genuinely not wanted."
                )),
                ..Default::default()
            },
        })
    };

    let authentication_data = request
        .authentication_data
        .as_ref()
        .ok_or_else(|| refuse("no authentication_data is attached"))?;

    match authentication_data.trans_status.as_ref() {
        Some(TransactionStatus::Success) | Some(TransactionStatus::NotVerified) => {}
        Some(other) => {
            let detail = format!(
                "the authentication finished with transStatus {other:?}, which carries no \
                 liability shift"
            );
            return Err(refuse(&detail));
        }
        None => return Err(refuse("the authentication result carries no transStatus")),
    }

    // A liability-shifting status with no cryptogram is not something the schemes accept, and
    // `build_mpi` would drop the whole element rather than send a half-populated one.
    if authentication_data.cavv.is_none() && authentication_data.eci.is_none() {
        return Err(refuse(
            "the authentication result carries neither a cryptogram nor an ECI",
        ));
    }

    Ok(())
}

fn build_mpi<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Option<GlobalpaymentsRealexMpi> {
    let authentication_data = request.authentication_data.as_ref()?;

    let mpi = GlobalpaymentsRealexMpi {
        eci: authentication_data.eci.clone(),
        // 3DS2 uses `ds_trans_id`; the 3DS1 shape (`xid`) is deliberately not emitted.
        ds_trans_id: authentication_data.ds_trans_id.clone(),
        authentication_value: authentication_data.cavv.clone(),
        message_version: authentication_data
            .message_version
            .as_ref()
            .map(|version| version.to_string()),
    };

    let is_empty = mpi.eci.is_none()
        && mpi.ds_trans_id.is_none()
        && mpi.authentication_value.is_none()
        && mpi.message_version.is_none();

    (!is_empty).then_some(mpi)
}

// =============================================================================
// RESPONSE (tech spec §8, §10)
// =============================================================================

/// The single response shape for **every** outcome.
///
/// The gateway always answers HTTP 200 — success, decline, bad digest and malformed XML alike —
/// and `5xx` result codes come back as a *smaller* document carrying only `timestamp`, `result`,
/// `message` and `orderid`. Every field except `result` is therefore optional.
///
/// `deny_unknown_fields` is deliberately **not** used: the live sandbox returns an undocumented
/// `<fraudresponse>` element and a `<cardissuer>` whose children differ from the documentation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsRealexPaymentsResponse {
    #[serde(rename = "@timestamp")]
    pub timestamp: Option<String>,
    /// The outcome code — `00` is success. Kept as a string because the leading zero is
    /// significant to the response digest and to `error_code`.
    pub result: String,
    pub message: Option<String>,
    pub orderid: Option<String>,
    pub merchantid: Option<String>,
    pub account: Option<String>,
    /// The gateway transaction reference — this is the `connector_transaction_id`, and every
    /// follow-up operation (`settle` / `void` / `rebate` / `query`) needs it.
    pub pasref: Option<String>,
    /// Issuer authorization code. Required by a later `rebate`.
    pub authcode: Option<String>,
    /// Settlement batch id. Can be negative (`-1` from `query` when not yet batched), is `0` on a
    /// successful `void`, and comes back as an **empty element** on declines, hence the tolerant
    /// deserializer.
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub batchid: Option<i64>,
    pub cvnresult: Option<String>,
    pub avspostcoderesponse: Option<String>,
    pub avsaddressresponse: Option<String>,
    /// Scheme Reference Data — the scheme transaction id, needed by any future MIT / COF work.
    pub srd: Option<String>,
    pub cardissuer: Option<GlobalpaymentsRealexCardIssuer>,
    /// Absent on `5xx` error documents; verification is skipped in that case.
    pub sha1hash: Option<String>,
}

/// The `settle` response is the **same document shape** as the `auth` response — same root, same
/// optional fields, same `<batchid>`-may-be-empty trap — so it is modelled by the same struct.
///
/// The alias exists because the macro layer derives per-flow helper types from the response type's
/// name and therefore needs a distinct identifier per flow.
pub type GlobalpaymentsRealexCaptureResponse = GlobalpaymentsRealexPaymentsResponse;

/// Deserializes an optional integer that the gateway may send as an **empty element**.
///
/// A declined `auth` answers with `<batchid></batchid>` while a successful one carries a number,
/// and serde cannot turn `""` into an `i64`. Blank text becomes `None` rather than an error.
fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<String> = Option::deserialize(deserializer)?;
    match raw.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) => value
            .parse::<i64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalpaymentsRealexCardIssuer {
    pub bank: Option<String>,
    pub country: Option<String>,
    pub countrycode: Option<String>,
    /// Documented but not returned by the live sandbox.
    pub region: Option<String>,
    /// Returned live but undocumented.
    pub commercial: Option<String>,
    /// Returned live but undocumented.
    pub cardtype: Option<String>,
}

impl GlobalpaymentsRealexPaymentsResponse {
    fn is_success(&self) -> bool {
        self.result == RESULT_SUCCESS
    }
}

/// `result` + the `autosettle` flag we sent → `AttemptStatus` (tech spec §9.3).
///
/// There is no pending/async state in this API: every `auth` resolves synchronously, so
/// `AttemptStatus::Pending` is never correct here and every non-`00` code is a `Failure`.
/// `111` (Strong Customer Authentication Required) is a soft decline — still a failure, but its
/// `error_code` is surfaced verbatim so callers can choose to retry through 3DS.
fn map_attempt_status(
    result: &str,
    auto_settle: GlobalpaymentsRealexAutoSettleFlag,
) -> AttemptStatus {
    match (result, auto_settle) {
        (RESULT_SUCCESS, GlobalpaymentsRealexAutoSettleFlag::AutoSettle) => AttemptStatus::Charged,
        (RESULT_SUCCESS, GlobalpaymentsRealexAutoSettleFlag::DelayedSettle) => {
            AttemptStatus::Authorized
        }
        _ => AttemptStatus::Failure,
    }
}

impl<T> TryFrom<ResponseRouterData<GlobalpaymentsRealexPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        // The flag we sent is what decides whether `00` means Charged or Authorized, so recover it
        // from the request rather than guessing from the response.
        let auto_settle =
            GlobalpaymentsRealexAutoSettleFlag::try_from(router_data.request.capture_method)
                .change_context(ConnectorError::ResponseHandlingFailed {
                    context: Default::default(),
                })?;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        let status = match hash_verification {
            // A tampered response must not be reported as a successful payment.
            HashVerification::Mismatch => AttemptStatus::IntegrityFailure,
            HashVerification::Verified | HashVerification::Skipped => {
                map_attempt_status(&response.result, auto_settle)
            }
        };

        if hash_verification == HashVerification::Mismatch {
            tracing::warn!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex response sha1hash did not match the computed digest"
            );
        }

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

        let payments_response = if response.is_success()
            && hash_verification != HashVerification::Mismatch
        {
            // `pasref` is the gateway transaction reference every follow-up flow needs; a success
            // without one would leave the payment unreferenceable.
            let pasref = response.pasref.clone().ok_or_else(|| {
                report!(ConnectorError::ResponseHandlingFailed {
                    context: Default::default(),
                })
                .attach_printable(
                    "GlobalpaymentsRealex returned result 00 without a <pasref> transaction \
                     reference",
                )
            })?;

            // The order id we actually put on the wire — recomputed rather than read back from
            // the response so that a follow-up `settle` reuses a byte-identical value.
            let sent_order_id = sanitize_order_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

            let connector_metadata = serde_json::to_value(GlobalpaymentsRealexPaymentMetadata {
                orderid: sent_order_id,
                authcode: response.authcode.clone(),
                // Nothing has been voided yet; the Void flow republishes this block with the
                // marker filled in.
                void_pasref: None,
                // The authorization's own pasref, pinned here because a later `settle` mints a new
                // one that `void` / `rebate` reject (see the field's doc).
                auth_pasref: Some(pasref.clone()),
            })
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })
            .attach_printable(
                "Failed to serialize the GlobalpaymentsRealex follow-up metadata (orderid / \
                     authcode / auth_pasref)",
            )?;

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: DomainResponseId::ConnectorTransactionId(pasref),
                redirection_data: None,
                connector_metadata: Some(connector_metadata),
                mandate_reference: None,
                // Scheme Reference Data is the scheme-level transaction id for this payment.
                network_txn_id: response.srd.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: response.orderid.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        } else {
            Err(ErrorResponse {
                status_code: http_code,
                // The `<result>` code verbatim — `"101"`, `"505"`, `"111"`, …
                code: response.result.clone(),
                message: message.clone(),
                reason: response.message.clone(),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: response.pasref.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: payments_response,
            ..router_data
        })
    }
}

// =============================================================================
// CAPTURE — `type="settle"` (tech spec §12.1)
// =============================================================================

/// The `settle` request.
///
/// It reuses the shared envelope (`<merchantid>`, `<account>`, `<sha1hash>`) but references an
/// existing authorization instead of carrying card data: `<orderid>`, `<pasref>` and `<authcode>`
/// must all be the **original** values from the `auth` that is being settled.
///
/// `<amount>` deliberately has **no** `currency` attribute — unlike `auth` and `rebate`, `settle`
/// takes a bare integer and inherits the currency from the original authorization (tech spec §12.1).
#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexCaptureRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    /// The `<orderid>` of the transaction being settled, not a new one.
    pub orderid: String,
    /// The gateway reference returned by the original `auth`.
    pub pasref: String,
    /// The issuer authorization code returned by the original `auth`. Omitted when Authorize did
    /// not surface one; the sandbox accepts a `settle` without it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authcode: Option<String>,
    /// Bare integer in the currency's minor unit — **no** `currency` attribute.
    pub amount: String,
    pub sha1hash: Secret<String>,
}

impl GetSoapXml for GlobalpaymentsRealexCaptureRequest {
    fn to_soap_xml(&self) -> String {
        // Mirrors the Authorize flow: a serialization failure emits a minimal well-formed document
        // that the gateway answers with `502 Mandatory Fields missing`, rather than panicking.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex settle request to XML"
            );
            "<request/>".to_string()
        })
    }
}

/// Recovers the `<orderid>` / `<authcode>` that Authorize published as `connector_metadata`.
///
/// The gRPC layer hands the same JSON back on either `connector_feature_data` (where the Authorize
/// response emits it) or `metadata`, so both are accepted, in that order. This is the **single**
/// metadata channel for every follow-up flow — Capture and Void both go through here.
fn extract_followup_metadata(
    connector_feature_data: Option<&SecretSerdeValue>,
    metadata: Option<&SecretSerdeValue>,
) -> Option<GlobalpaymentsRealexPaymentMetadata> {
    connector_feature_data.or(metadata).and_then(|value| {
        match serde_json::from_value::<GlobalpaymentsRealexPaymentMetadata>(value.peek().clone()) {
            Ok(metadata) => Some(metadata),
            Err(error) => {
                // Not fatal: the caller may be passing unrelated metadata, in which case the
                // order id falls back to the flow's own merchant reference below.
                tracing::warn!(
                    connector = "globalpayments_realex",
                    ?error,
                    "GlobalpaymentsRealex follow-up metadata did not contain an <orderid>; \
                     falling back to the request reference"
                );
                None
            }
        }
    })
}

/// Resolves the `<pasref>` a `void` or `rebate` must address.
///
/// The ladder is deliberate:
///
/// 1. [`GlobalpaymentsRealexPaymentMetadata::auth_pasref`] — the pasref the original `auth` minted,
///    which is the only one the gateway accepts on a reversal.
/// 2. `connector_transaction_id` — correct for auto-captured payments (no `settle` ever ran, so the
///    transaction id *is* the auth pasref) and for payments authorized before `auth_pasref` was
///    published, which must keep working.
/// 3. Neither — an actionable error naming the field, rather than a request guaranteed to `508`.
///
/// When both are present and disagree the metadata wins, because it is literally the `<pasref>` the
/// `auth` returned, whereas `connector_transaction_id` is whatever the caller last persisted — the
/// settle-minted pasref for a manually captured payment. The divergence is logged loudly: it is the
/// signature of exactly the bug this ladder exists to absorb.
fn resolve_reference_pasref(
    metadata: Option<&GlobalpaymentsRealexPaymentMetadata>,
    connector_transaction_id: Option<&str>,
    request_type: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let metadata_pasref = metadata
        .and_then(|metadata| metadata.auth_pasref.as_deref())
        .map(str::trim)
        .filter(|pasref| !pasref.is_empty());
    let transaction_id = connector_transaction_id
        .map(str::trim)
        .filter(|pasref| !pasref.is_empty());

    if let (Some(metadata_pasref), Some(transaction_id)) = (metadata_pasref, transaction_id) {
        if metadata_pasref != transaction_id {
            tracing::warn!(
                connector = "globalpayments_realex",
                request_type = %request_type,
                metadata_pasref = %metadata_pasref,
                connector_transaction_id = %transaction_id,
                "GlobalpaymentsRealex was given two different pasrefs for the same payment; using \
                 the one from connector_feature_data, which is the <pasref> the original auth \
                 returned — the transaction id is most likely the pasref a settle minted, which the \
                 gateway rejects with 508 Original transaction not found."
            );
        }
    }

    metadata_pasref
        .or(transaction_id)
        .map(str::to_string)
        .ok_or_else(|| {
            report!(IntegrationError::MissingRequiredField {
                field_name: "connector_feature_data.auth_pasref",
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "GlobalpaymentsRealex {request_type} needs the <pasref> the original auth \
                         minted: echo the Authorize response's connector_feature_data back on the \
                         request, or set the connector_transaction_id to the authorization's pasref"
                    )),
                    ..Default::default()
                },
            })
        })
}

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpaymentsRealexCaptureRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // RealEx expresses multiple partial captures through `autosettle flag="MULTI"` + repeated
        // `multisettle` requests, which this sandbox account is not provisioned for
        // (`503 Request type [multisettle] not allowed for this merchant` — tech spec §16 item 7).
        // Reject it explicitly rather than silently degrading to a single settle.
        if matches!(request.capture_method, Some(CaptureMethod::ManualMultiple))
            || request.is_multiple_capture()
        {
            return Err(report!(IntegrationError::NotImplemented(
                "Multiple partial captures (ManualMultiple) are not supported by \
                 GlobalpaymentsRealex: the account must be provisioned for the `multisettle` \
                 request type, which returns 503 otherwise"
                    .to_string(),
                IntegrationErrorContext::default(),
            )));
        }

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;
        let metadata = extract_followup_metadata(
            request.connector_feature_data.as_ref(),
            request.metadata.as_ref(),
        );

        // `<orderid>` must be the original one. Preferred source is the metadata Authorize
        // published; `merchant_capture_id` is accepted as a fallback for callers that echo the
        // original reference there.
        let order_id = match metadata.as_ref().map(|metadata| metadata.orderid.clone()) {
            Some(order_id) => sanitize_order_id(&order_id)?,
            None => sanitize_order_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .attach_printable(
                "GlobalpaymentsRealex settle needs the original <orderid>: echo the Authorize \
                 response's connector_feature_data back on the capture request, or set \
                 merchant_capture_id to the original order id",
            )?,
        };

        // `<pasref>` is the gateway reference, i.e. the connector_transaction_id.
        let pasref = request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "GlobalpaymentsRealex settle needs the original <pasref> as the \
                         connector_transaction_id"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        // Always sent explicitly, so full and partial settles share one code path and one digest
        // blueprint (both verified live — see `build_reference_request_hash`).
        let amount = format_amount(request.minor_amount_to_capture, request.currency)?;

        let timestamp = current_timestamp()?;
        let merchant_id = auth.merchant_id.clone().expose();
        let sha1hash = build_reference_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            Some(&amount),
            // `settle`'s `<amount>` has no `currency` attribute, so the currency slot stays empty:
            // `timestamp.merchantid.orderid.<amount>..` (verified live).
            None,
            auth.shared_secret.peek(),
        );

        Ok(Self {
            request_type: REQUEST_TYPE_SETTLE.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            orderid: order_id,
            pasref,
            authcode: metadata.and_then(|metadata| metadata.authcode),
            amount,
            sha1hash,
        })
    }
}

/// `result` → `AttemptStatus` for `settle` (tech spec §9.4).
///
/// As with `auth` there is no asynchronous state: a `settle` either succeeds (`00 Settled
/// Successfully`) or fails outright, so `Pending` is never correct here.
fn map_capture_attempt_status(result: &str) -> AttemptStatus {
    if result == RESULT_SUCCESS {
        AttemptStatus::Charged
    } else {
        AttemptStatus::CaptureFailed
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsRealexCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        // The `settle` response uses the same check-hash blueprint as `auth` (verified live), and
        // the same "no <sha1hash> on 5xx error documents" rule applies.
        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        let status = match hash_verification {
            HashVerification::Mismatch => AttemptStatus::IntegrityFailure,
            HashVerification::Verified | HashVerification::Skipped => {
                map_capture_attempt_status(&response.result)
            }
        };

        if hash_verification == HashVerification::Mismatch {
            tracing::warn!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex settle response sha1hash did not match the computed digest"
            );
        }

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

        let capture_response =
            if response.is_success() && hash_verification != HashVerification::Mismatch {
                // A settle mints its own `<pasref>`; keep the original one when the gateway does
                // not return a new one so the payment stays referenceable either way.
                let resource_id = response
                    .pasref
                    .clone()
                    .map(DomainResponseId::ConnectorTransactionId)
                    .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());

                // Carry the follow-up metadata across the capture rather than dropping it. This
                // response's `connector_transaction_id` is the settle-minted pasref, so a caller
                // that replaces its stored block here would lose the `<orderid>`, the `<authcode>`
                // and — the reason this matters — the auth pasref that `void` and `rebate` need.
                //
                // The *capture request's* `connector_transaction_id` is still the auth pasref by
                // construction, so it also backfills `auth_pasref` for payments authorized before
                // that field existed. Nothing is republished when the caller sent no metadata:
                // inventing an order id from the request reference could overwrite a correct
                // stored one.
                let connector_metadata = extract_followup_metadata(
                    router_data.request.connector_feature_data.as_ref(),
                    router_data.request.metadata.as_ref(),
                )
                .map(|previous| GlobalpaymentsRealexPaymentMetadata {
                    auth_pasref: previous.auth_pasref.clone().or_else(|| {
                        router_data
                            .request
                            .connector_transaction_id
                            .get_connector_transaction_id()
                            .ok()
                    }),
                    ..previous
                })
                .map(serde_json::to_value)
                .transpose()
                .change_context(ConnectorError::ResponseHandlingFailed {
                    context: Default::default(),
                })
                .attach_printable(
                    "Failed to serialize the GlobalpaymentsRealex follow-up metadata (orderid / \
                     authcode / auth_pasref) on the settle response",
                )?;

                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id,
                    redirection_data: None,
                    connector_metadata,
                    mandate_reference: None,
                    network_txn_id: response.srd.clone(),
                    network_txn_link_id: None,
                    connector_response_reference_id: response.orderid.clone(),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: http_code,
                    payment_account_reference: None,
                })
            } else {
                Err(ErrorResponse {
                    status_code: http_code,
                    code: response.result.clone(),
                    message: message.clone(),
                    reason: response.message.clone(),
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: response.pasref.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: capture_response,
            ..router_data
        })
    }
}

// =============================================================================
// VOID — `type="void"` (tech spec §12.2)
// =============================================================================

/// The documented `<reasoncode>` enum. RealEx defaults to `NOTGIVEN` when the element is absent, so
/// an unrecognised free-text cancellation reason is dropped rather than guessed at — sending an
/// unknown value is rejected by the schema.
const VOID_REASON_CODES: [&str; 6] = [
    "FRAUD",
    "OUTOFSTOCK",
    "DUPLICATE",
    "MISTAKE",
    "OTHER",
    "NOTGIVEN",
];

/// Maps `cancellation_reason` onto `<reasoncode>` when — and only when — it is one of the six
/// documented values (case-insensitively). Anything else yields `None` and the element is omitted.
///
/// Verified live: sending `<reasoncode>FRAUD</reasoncode>` does not change the digest — the
/// blueprint stays `timestamp.merchantid.orderid...` and the void still returns `00`.
fn map_void_reason_code(cancellation_reason: Option<&str>) -> Option<&'static str> {
    let reason = cancellation_reason?.trim();
    VOID_REASON_CODES
        .into_iter()
        .find(|code| code.eq_ignore_ascii_case(reason))
}

/// The `void` request.
///
/// Reuses the shared envelope and, like `settle`, references an existing authorization by its
/// original `<orderid>` plus the gateway's `<pasref>`. It carries **no `<amount>`** — a void always
/// reverses the full authorization — and no card data.
///
/// `<authcode>` is documented as optional for `void` and is deliberately not sent; the sandbox
/// accepts the request without it (verified live).
///
/// **`<pasref>` must be the pasref minted by the original `auth`.** A `settle` mints a *new*
/// pasref, and voiding with that one returns `508 Original transaction not found.` — verified live.
/// UCS surfaces the settle pasref as the Capture response's `connector_transaction_id`, so this is
/// **not** left to the caller: the auth pasref is read from
/// [`GlobalpaymentsRealexPaymentMetadata::auth_pasref`], which Authorize publishes and Capture
/// carries forward, and `connector_transaction_id` is only the fallback for payments that never had
/// a separate settle (see [`resolve_reference_pasref`]).
#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexVoidRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    /// The `<orderid>` of the transaction being voided, not a new one.
    pub orderid: String,
    /// The gateway reference returned by the original `auth`.
    pub pasref: String,
    /// One of the six documented reason codes; omitted entirely when the caller supplied none, in
    /// which case the gateway applies its `NOTGIVEN` default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoncode: Option<String>,
    pub sha1hash: Secret<String>,
}

impl GetSoapXml for GlobalpaymentsRealexVoidRequest {
    fn to_soap_xml(&self) -> String {
        // Mirrors the other flows: a serialization failure emits a minimal well-formed document
        // that the gateway answers with `502 Mandatory Fields missing`, rather than panicking.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex void request to XML"
            );
            "<request/>".to_string()
        })
    }
}

/// The `void` response is the **same document shape** as every other response on this API, so it
/// reuses the one response struct — including the tolerant `<batchid>` deserializer, which matters
/// here because a successful void returns `<batchid>0</batchid>` while a failure returns the small
/// error document with no `<batchid>` at all.
pub type GlobalpaymentsRealexVoidResponse = GlobalpaymentsRealexPaymentsResponse;

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpaymentsRealexVoidRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;

        // Exactly the same metadata channel Capture uses: Authorize publishes the original
        // `<orderid>` as `connector_metadata`, which comes back on `connector_feature_data`.
        let metadata = extract_followup_metadata(
            request.connector_feature_data.as_ref(),
            request.metadata.as_ref(),
        );

        // `<orderid>` must be the reference we originally sent — never the gateway echo, which is
        // prefixed (`_void_…` / `_settle_…`) on error documents. `merchant_void_id` (surfaced as
        // `connector_request_reference_id`) is accepted as a fallback for callers that echo the
        // original reference there.
        let order_id = match metadata.as_ref().map(|metadata| metadata.orderid.clone()) {
            Some(order_id) => sanitize_order_id(&order_id)?,
            None => sanitize_order_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .attach_printable(
                "GlobalpaymentsRealex void needs the original <orderid>: echo the Authorize \
                 response's connector_feature_data back on the void request, or set \
                 merchant_void_id to the original order id",
            )?,
        };

        // `<pasref>` must be the one the original `auth` minted, never the settle-minted pasref a
        // captured payment carries as its `connector_transaction_id` — see the struct doc.
        let pasref = resolve_reference_pasref(
            metadata.as_ref(),
            Some(request.connector_transaction_id.as_str()),
            REQUEST_TYPE_VOID,
        )?;

        let timestamp = current_timestamp()?;
        let merchant_id = auth.merchant_id.clone().expose();
        // No `<amount>` on the wire, so the amount and currency slots of the digest both stay
        // empty: `timestamp.merchantid.orderid...` (tech spec §12.2).
        let sha1hash = build_reference_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            None,
            None,
            auth.shared_secret.peek(),
        );

        Ok(Self {
            request_type: REQUEST_TYPE_VOID.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            orderid: order_id,
            pasref,
            reasoncode: map_void_reason_code(request.cancellation_reason.as_deref())
                .map(str::to_string),
            sha1hash,
        })
    }
}

/// `result` → `AttemptStatus` for `void` (tech spec §9.4).
///
/// As with `auth` and `settle` there is no asynchronous state: a void either succeeds
/// (`00 Voided Successfully`) or fails outright, so `Pending` is never correct here. Every non-`00`
/// code is a plain `VoidFailed` carrying the gateway's own code and message — never a
/// transport-level error and never silently mapped to success.
///
/// Failure codes observed live, all as the small error document (HTTP 200, no `<sha1hash>`,
/// no `<batchid>`):
///
/// | Scenario | `result` | `message` |
/// |---|---|---|
/// | Voiding an already-voided transaction | `508` | `That transaction has already been voided.` |
/// | Voiding with the pasref a `settle` minted | `508` | `Original transaction not found.` |
/// | Unparsable `<pasref>` | `506` | `… does not conform to the schema` |
///
/// The documented `513 Can't void a settled transaction` could **not** be reproduced: an
/// `auth` + `settle` pair is still voidable through its *original* pasref (`00 Voided
/// Successfully`) because `settle` only queues the transaction into the open batch. `513` presumably
/// only appears once that batch has actually been closed, which a merchant cannot trigger on
/// demand. Either way it lands in the same `VoidFailed` branch as the codes above.
fn map_void_attempt_status(result: &str) -> AttemptStatus {
    if result == RESULT_SUCCESS {
        AttemptStatus::Voided
    } else {
        AttemptStatus::VoidFailed
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsRealexVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        // Same check-hash blueprint as `auth` and `settle`, and the same "no <sha1hash> on the
        // small 5xx error document" rule.
        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        let status = match hash_verification {
            HashVerification::Mismatch => AttemptStatus::IntegrityFailure,
            HashVerification::Verified | HashVerification::Skipped => {
                map_void_attempt_status(&response.result)
            }
        };

        if hash_verification == HashVerification::Mismatch {
            tracing::warn!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex void response sha1hash did not match the computed digest"
            );
        }

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

        let void_response = if response.is_success()
            && hash_verification != HashVerification::Mismatch
        {
            // Keep the payment referenceable: a void response echoes a `<pasref>`, but fall back to
            // the one we sent if the gateway omits it.
            let void_pasref = response
                .pasref
                .clone()
                .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());
            let resource_id = DomainResponseId::ConnectorTransactionId(void_pasref.clone());

            // Republish the follow-up metadata with the void marker set, so that a later PSync
            // can report `Voided` — a `query` is blind to voids and would otherwise keep
            // reporting the authorization leg (tech spec §12.4.5, and `map_psync_attempt_status`).
            //
            // `orderid` and `authcode` are carried over from the metadata the caller sent us,
            // falling back to the gateway echo and then to the request reference, so that a
            // caller which *replaces* its stored `connector_feature_data` with this block does
            // not lose the order id every follow-up flow depends on.
            let previous = extract_followup_metadata(
                router_data.request.connector_feature_data.as_ref(),
                router_data.request.metadata.as_ref(),
            );
            let order_id = previous
                .as_ref()
                .map(|metadata| metadata.orderid.clone())
                .or_else(|| response.orderid.clone())
                .unwrap_or_else(|| {
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone()
                });
            let connector_metadata = serde_json::to_value(GlobalpaymentsRealexPaymentMetadata {
                orderid: order_id,
                // The ORIGINAL auth's authcode. A void response carries its own
                // (`000000`), which a later `rebate` must not use, so it is never read
                // from the response here.
                authcode: previous
                    .as_ref()
                    .and_then(|metadata| metadata.authcode.clone()),
                void_pasref: Some(void_pasref),
                // Likewise the ORIGINAL auth's pasref: a void mints its own, and republishing that
                // as `auth_pasref` would poison a later reversal. Falls back to the pasref this
                // request actually addressed, which the ladder already resolved to the auth one.
                auth_pasref: previous
                    .and_then(|metadata| metadata.auth_pasref)
                    .or_else(|| {
                        Some(router_data.request.connector_transaction_id.clone())
                            .map(|pasref| pasref.trim().to_string())
                            .filter(|pasref| !pasref.is_empty())
                    }),
            })
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })
            .attach_printable(
                "Failed to serialize the GlobalpaymentsRealex follow-up metadata \
                         (orderid / authcode / void_pasref / auth_pasref) on the void response",
            )?;

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                connector_metadata: Some(connector_metadata),
                mandate_reference: None,
                network_txn_id: response.srd.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: response.orderid.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        } else {
            Err(ErrorResponse {
                status_code: http_code,
                // The `<result>` code verbatim — e.g. `"513"` for an already-settled transaction.
                code: response.result.clone(),
                message: message.clone(),
                reason: response.message.clone(),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: response.pasref.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: void_response,
            ..router_data
        })
    }
}

// =============================================================================
// REFUND — `type="rebate"` (tech spec §12.3)
// =============================================================================

/// The `rebate` request — a **referenced** refund against an existing, captured transaction.
///
/// Like `settle` and `void` it carries no card data and addresses the original transaction by
/// `<orderid>` + `<pasref>` + `<authcode>`, but it differs from both in two ways that each have
/// their own failure mode:
///
/// 1. **`<amount>` carries the `currency` as an attribute** — `<amount currency="EUR">1999</amount>`
///    — where `settle` sends a bare `<amount>599</amount>`. The digest blueprint mirrors that, so
///    `rebate` is the only reference request whose currency slot is filled
///    (see [`build_reference_request_hash`]).
/// 2. **A second, independent digest `<refundhash>`** derived from the *rebate password*
///    (`api_secret`), not from the shared secret (see [`build_refund_password_hash`]). This is the
///    only flow on this connector that consumes `api_secret` at all.
///
/// `<authcode>` is graded **mandatory** by the vendor for `rebate` (it is merely accepted, and
/// ignored, by `settle`), and it must be the **original authorization's** auth code — not the
/// `000000` that `settle` and `void` responses return. It reaches us through the same
/// `connector_metadata` channel Authorize publishes and [`extract_followup_metadata`] reads.
///
/// **Which `<pasref>`:** verified live against the sandbox with the A/B test tech spec §12.3
/// prescribes — `auth`(MANUAL) -> `settle` -> `rebate` with each candidate in turn:
///
/// | `<pasref>` sent | Result |
/// |---|---|
/// | the **original `auth`** pasref | `00 Successful` |
/// | the `settle`-minted pasref | `508 Original transaction not found.` |
///
/// So `rebate` behaves exactly like `void` (tech spec §12.2) despite acting *after* capture: it
/// wants the **`auth`** pasref. Because UCS surfaces the settle-minted pasref as the Capture
/// response's `connector_transaction_id`, the pasref is taken from
/// [`GlobalpaymentsRealexPaymentMetadata::auth_pasref`] first and only falls back to the
/// transaction id (see [`resolve_reference_pasref`]).
#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexRefundRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    /// The `<orderid>` of the transaction being refunded, not a new one.
    pub orderid: String,
    /// `<amount currency="…">…</amount>` — the same shape Authorize uses, and unlike `settle`,
    /// which sends a bare integer.
    pub amount: GlobalpaymentsRealexAmount,
    /// The gateway reference minted by the original `auth`.
    pub pasref: String,
    /// The **original authorization's** `<authcode>`. Mandatory for `rebate`; omitted only when
    /// Authorize never surfaced one, in which case the gateway answers `502 Compulsory field not
    /// present` rather than silently refunding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authcode: Option<String>,
    /// Single SHA-1 of the plaintext rebate password (`api_secret`) — **not** the shared secret.
    pub refundhash: Secret<String>,
    /// The ordinary two-stage request digest, keyed on the shared secret (`api_key`).
    pub sha1hash: Secret<String>,
}

impl GetSoapXml for GlobalpaymentsRealexRefundRequest {
    fn to_soap_xml(&self) -> String {
        // Mirrors the other flows: a serialization failure emits a minimal well-formed document
        // that the gateway answers with `502 Mandatory Fields missing`, rather than panicking.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex rebate request to XML"
            );
            "<request/>".to_string()
        })
    }
}

/// The `rebate` response is the **same document shape** as every other response on this API — a
/// success carries a `<batchid>` (unlike `void`, which returns `0`) and a **new** `<pasref>`, and a
/// failure is the small error document with only `@timestamp`, `<result>`, `<message>` and
/// `<orderid>`. It therefore reuses the one response struct, including the tolerant `<batchid>`
/// deserializer.
///
/// Two live observations that contradict the vendor documentation, both harmless because nothing
/// here matches on either value — but worth recording so that a future reader does not "fix" them:
///
/// * `<authcode>` is a **real issuer auth code** (e.g. `003712`), not the `000000` the vendor sample
///   prints and that `settle` / `void` genuinely do return. It is hashed verbatim into the response
///   check-hash, so it must be read from the response, never assumed.
/// * `<message>` on a successful rebate is `AUTH CODE: nnnnnn` — neither the `Successful` of the XML
///   sample nor the `Rebated Successfully` of the SDK guide. Success is decided by `<result>` alone.
pub type GlobalpaymentsRealexRefundResponse = GlobalpaymentsRealexPaymentsResponse;

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for GlobalpaymentsRealexRefundRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;

        // Exactly the same metadata channel Capture and Void use: Authorize publishes the original
        // `<orderid>` and `<authcode>` as `connector_metadata`, which comes back on
        // `connector_feature_data` (or, for callers that route it there, `refund_metadata`).
        let metadata = extract_followup_metadata(
            request.connector_feature_data.as_ref(),
            request.refund_connector_metadata.as_ref(),
        );

        // `<orderid>` must be the reference **we** originally sent — never the gateway echo. Every
        // failed rebate observed live echoes it prefixed, `_rebate_ucs-1234` (the `settle`
        // equivalent is `_settle_…`). Feeding an echo back yields
        // `508 Invalid characters in order id …` or `508 Original transaction not found.`
        //
        // `connector_order_id` is accepted as a fallback for callers that carry the original order
        // id there; `connector_request_reference_id` on a refund is the *merchant refund id*, which
        // is a different value, so it is deliberately not used.
        let order_id = match metadata
            .as_ref()
            .map(|metadata| metadata.orderid.clone())
            .or_else(|| request.connector_order_id.clone())
        {
            Some(order_id) => sanitize_order_id(&order_id)?,
            None => {
                return Err(report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.orderid",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "GlobalpaymentsRealex rebate needs the original <orderid>: echo the \
                             Authorize response's connector_feature_data back on the refund \
                             request, or set connector_order_id to the original order id"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        };

        // `<pasref>` must be the one the **original `auth`** minted — see the struct doc for the
        // live A/B result. A refund of a manually captured payment arrives with the settle-minted
        // pasref as its `connector_transaction_id`, so the metadata is preferred over it.
        let pasref = resolve_reference_pasref(
            metadata.as_ref(),
            Some(request.connector_transaction_id.as_str()),
            REQUEST_TYPE_REBATE,
        )?;

        // Minor units, integer, no decimal point. The gateway owns the 115% / 105% over-refund
        // ceiling; no client-side limit is applied so its refusal surfaces verbatim.
        let amount = format_amount(request.minor_refund_amount, request.currency)?;
        let currency = request.currency.to_string();

        let timestamp = current_timestamp()?;
        let merchant_id = auth.merchant_id.clone().expose();
        // `rebate` always carries `<amount currency="…">`, so both the amount and the currency slot
        // are filled: `timestamp.merchantid.orderid.<amount>.<currency>.` (tech spec §12.3).
        let sha1hash = build_reference_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            Some(&amount),
            Some(&currency),
            auth.shared_secret.peek(),
        );

        Ok(Self {
            request_type: REQUEST_TYPE_REBATE.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            orderid: order_id,
            amount: GlobalpaymentsRealexAmount {
                currency,
                value: amount,
            },
            pasref,
            authcode: metadata.and_then(|metadata| metadata.authcode),
            // The one and only consumer of `api_secret` on this connector. Deriving this from
            // `shared_secret` instead is a silent bug on any account where the two differ.
            refundhash: build_refund_password_hash(auth.refund_password.peek()),
            sha1hash,
        })
    }
}

/// `result` → `RefundStatus` for `rebate` (tech spec §9.4, §12.3).
///
/// There is no pending or asynchronous refund state in this API — the answer is final in the same
/// HTTP 200 response — so `RefundStatus::Pending` is never correct here. Every non-`00` code is a
/// plain `Failure` carrying the gateway's own `<result>` as `error_code` and its `<message>` as
/// `error_message`; nothing is retried and nothing is mapped to success.
///
/// The `<message>` matters more here than on any other flow, because the two digests fail with the
/// *same* result code and are only distinguishable by their text. Codes marked **verified live**
/// were reproduced against the sandbox while implementing this flow:
///
/// | `result` | `message` | Cause |
/// |---|---|---|
/// | `505` | `sha1hash incorrect - check your code and the Developers Documentation` | bad `<sha1hash>` — blueprint / mirror-rule mistake, or a stale timestamp. **Verified live** by supplying a wrong shared secret with a correct rebate password |
/// | `505` | `The refund password you entered was incorrect.` | bad `<refundhash>` — wrong `api_secret`, or the two-stage construction used instead of the single SHA-1. **Verified live** by supplying a wrong rebate password with a correct shared secret |
/// | `512` | `You may only refund up to 100% of the original amount.` | over-refund. **Verified live** — note the *ceiling is account-configured*: the docs advertise 115% (`508`) and 105% (`512`), and this sandbox account enforces **100%**. This is exactly why no client-side limit is applied |
/// | `508` | `You may only rebate up to 115% of the original amount.` | the 115% variant of the ceiling on accounts configured that way |
/// | `508` | `Original transaction not found.` | wrong `<pasref>` / `<orderid>` pair. **Verified live** with the `settle`-minted pasref |
/// | `512` | `This transaction has already been rebated and cannot be rebated again.` | second partial refund on an account not enabled for multiple refunds |
/// | `512` | `You can't refund a delayed transaction that has not been sent for settlement …` | the authorization was never captured — void it instead |
/// | `506` | `The line number 2 which contains '…' does not conform to the schema` | unparsable `<pasref>`. **Verified live** |
///
/// Every one of these arrives as HTTP 200 in the small error document, so they are classified from
/// `<result>` here rather than from a transport status.
fn map_refund_status(result: &str) -> RefundStatus {
    if result == RESULT_SUCCESS {
        RefundStatus::Success
    } else {
        RefundStatus::Failure
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsRealexRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        // Same check-hash blueprint as `auth`, `settle` and `void`
        // (`timestamp.merchantid.orderid.result.message.pasref.authcode`), keyed on the **shared
        // secret** — `refundhash` plays no part in response verification — and the same "no
        // <sha1hash> on the small 5xx error document" rule.
        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        let status = match hash_verification {
            // A tampered response must never be reported as a completed refund — but it must not
            // be reported as a *failed* one either. The gateway answered, the rebate very probably
            // executed, and only the document's integrity is in doubt; calling that `Failure`
            // invites the caller to retry and refund the shopper twice. `ManualReview` is the
            // honest terminal state: stop, do not retry, have a human reconcile. This mirrors the
            // `IntegrityFailure` the payment flows use for the same situation.
            HashVerification::Mismatch => RefundStatus::ManualReview,
            HashVerification::Verified | HashVerification::Skipped => {
                map_refund_status(&response.result)
            }
        };

        if hash_verification == HashVerification::Mismatch {
            tracing::warn!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex rebate response sha1hash did not match the computed digest"
            );
        }

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

        let refund_response =
            if response.is_success() && hash_verification != HashVerification::Mismatch {
                // A rebate mints a **new** `<pasref>`, and that is the refund's own identity. Falling
                // back to the payment's pasref would make two different refunds indistinguishable, so
                // a success without one is a response-handling failure rather than a silent alias.
                let connector_refund_id = response.pasref.clone().ok_or_else(|| {
                    report!(ConnectorError::ResponseHandlingFailed {
                        context: Default::default(),
                    })
                    .attach_printable(
                        "GlobalpaymentsRealex returned result 00 on a rebate without a <pasref> \
                     refund reference",
                    )
                })?;

                Ok(RefundsResponseData {
                    connector_refund_id,
                    refund_status: status,
                    status_code: http_code,
                    acquirer_reference_number: None,
                })
            } else {
                Err(ErrorResponse {
                    status_code: http_code,
                    // The `<result>` code verbatim — `"505"`, `"508"`, `"512"`, …
                    code: response.result.clone(),
                    // Surfaced verbatim: on a `505` this text is the *only* thing separating a bad
                    // `<sha1hash>` from a bad `<refundhash>`.
                    message: message.clone(),
                    reason: response.message.clone(),
                    attempt_status: Some(FlowStatus::Refund(status)),
                    connector_transaction_id: response.pasref.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: refund_response,
            ..router_data
        })
    }
}

// =============================================================================
// PSYNC — `type="query"` (tech spec §12.4)
// =============================================================================

/// The `query` request.
///
/// Same envelope and same single endpoint as every other flow; only the `type` attribute and the
/// body elements change. A `query` carries **no `<amount>`** and no card data.
///
/// **`query` keys on `<orderid>` only** (tech spec §12.4.2, verified live). This is the one place
/// on this connector where `<pasref>` is *not* the lookup key: the gateway ignores it completely —
/// garbage, an empty element, a pasref belonging to a different order, or omitting the element
/// altogether all return the same transaction, while an unknown `<orderid>` returns
/// `508 Original transaction not found.` even when a perfectly valid `<pasref>` accompanies it.
/// It is still sent when we have one, because it costs nothing and keeps the request shape uniform
/// with `settle` / `void` / `rebate` — but nothing may ever depend on it.
///
/// The corollary is that PSync is entirely dependent on the **original** `<orderid>`, the one we
/// sent on the `auth`. It comes from the same `connector_metadata` → `connector_feature_data`
/// channel Authorize publishes and Capture / Void / Refund already consume. It must never be taken
/// from a gateway echo, which is prefixed on the follow-up legs (`_settle_…`, `_void_…`,
/// `_rebate_…`).
#[derive(Debug, Serialize)]
#[serde(rename = "request")]
pub struct GlobalpaymentsRealexPSyncRequest {
    #[serde(rename = "@type")]
    pub request_type: String,
    #[serde(rename = "@timestamp")]
    pub timestamp: String,
    pub merchantid: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Secret<String>>,
    /// The sole lookup key: the `<orderid>` of the original `auth`.
    pub orderid: String,
    /// Accepted but **ignored** by the gateway (tech spec §12.4.2). Omitted when we have no
    /// gateway reference to hand, which changes nothing about the result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pasref: Option<String>,
    pub sha1hash: Secret<String>,
}

impl GetSoapXml for GlobalpaymentsRealexPSyncRequest {
    fn to_soap_xml(&self) -> String {
        // Mirrors the other flows: a serialization failure emits a minimal well-formed document
        // that the gateway answers with `502 Mandatory Fields missing`, rather than panicking.
        quick_xml::se::to_string_with_root("request", self).unwrap_or_else(|error| {
            tracing::error!(
                connector = "globalpayments_realex",
                ?error,
                "Failed to serialize the GlobalpaymentsRealex query request to XML"
            );
            "<request/>".to_string()
        })
    }
}

/// The `query` response is a **superset** of the `auth` response — same root and same fields, plus
/// `<cardnumber>`, `<cardissuer>`, `<tss>`, `<threedsecure>`, `<srd>`, `<timetaken>` and
/// `<authtimetaken>`. On this sandbox account the extra blocks come back as **empty elements** on a
/// plain card auth, which is precisely why the shared struct does not use `deny_unknown_fields` and
/// why `<batchid>` goes through the tolerant [`deserialize_optional_i64`]. Modelling it with the
/// existing struct therefore parses every observed document; the additional elements are simply not
/// read.
///
/// # The check-hash blueprint is UNRECOVERED on this flow — not absent
///
/// A `query` response **does** carry a `<sha1hash>`, and it is a real, live digest: two queries
/// against the same order return different values that track the response timestamp, so a
/// timestamp is certainly one of its inputs. What could not be established is the rest of the
/// field list. It is emphatically **not** the documented response blueprint
/// `timestamp.merchantid.orderid.result.message.pasref.authcode`, which the `auth`, `settle`,
/// `void` and `rebate` responses all satisfy through this very same [`build_response_hash`] —
/// so this is specific to `query` and is not a bug in the shared helper.
///
/// Ruled out by exhaustive live search against a known-good order, recorded here so nobody repeats
/// it:
///
/// * every ordering of the response's own field values under the prefixes `timestamp`,
///   `timestamp.merchantid` and `timestamp.merchantid.orderid`, up to nine positional slots, with
///   repeated empty slots allowed (~5x10^8 candidates);
/// * the document-order prefixes of the response as it appears on the wire;
/// * the `auth` **request** blueprint (`timestamp.merchantid.orderid.amount.currency.cardnumber`);
/// * the masked (`424242XXXXXX4242`-style) and unmasked card numbers;
/// * `<message>` with and without the sandbox's `[ test system ] ` prefix;
/// * the request timestamp as well as the response timestamp;
/// * the second credential (the rebate password) as the stage-2 key, and the single-stage digest.
///
/// Consequently this flow **cannot** implement the usual integrity check. The digest is still
/// computed and logged at `debug` so that a change on the gateway's side becomes visible, but it
/// never decides the status: mapping a mismatch to `IntegrityFailure` would fail every single sync,
/// and there is no way to tell "blueprint unrecovered" apart from "response tampered". The
/// transport is TLS and a `query` is a read-only status enquiry that moves no money. Should Global
/// Payments ever document the `query` blueprint, re-enabling verification is a one-line change in
/// the caller below. The `IntegrityFailure` mapping on the other four flows is untouched.
pub type GlobalpaymentsRealexPSyncResponse = GlobalpaymentsRealexPaymentsResponse;

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for GlobalpaymentsRealexPSyncRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;

        // Exactly the same metadata channel Capture, Void and Refund use. `PaymentsSyncData`
        // carries no free-form `metadata` field, so `connector_feature_data` is the only source.
        let metadata = extract_followup_metadata(request.connector_feature_data.as_ref(), None);

        // `<orderid>` is the ONLY lookup key on a `query`, so getting it right is the whole flow.
        // Preferred source is the metadata Authorize published; `connector_request_reference_id`
        // is accepted as a fallback for callers that reuse the original reference there.
        let order_id = match metadata.as_ref().map(|metadata| metadata.orderid.clone()) {
            Some(order_id) => sanitize_order_id(&order_id)?,
            None => sanitize_order_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
            .attach_printable(
                "GlobalpaymentsRealex query needs the original <orderid>: echo the Authorize \
                 response's connector_feature_data back on the sync request, or reuse the original \
                 order id as the request reference",
            )?,
        };

        // Sent when available purely for shape parity — the gateway ignores it, so a payment whose
        // `connector_transaction_id` is missing (or is the settle-minted pasref, which trips up
        // `void` and `rebate`) still syncs correctly.
        let pasref = request
            .get_connector_transaction_id()
            .ok()
            .map(|pasref| pasref.trim().to_string())
            .filter(|pasref| !pasref.is_empty());

        let timestamp = current_timestamp()?;
        let merchant_id = auth.merchant_id.clone().expose();
        // No `<amount>` and no `<currency>` on the wire, so both slots stay empty and the
        // blueprint is byte-identical to `void`: `timestamp.merchantid.orderid...`
        // (tech spec §12.4.3 — 12 alternative blueprints were tried live and every one of them
        // returned `505 sha1hash incorrect`).
        let sha1hash = build_reference_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            None,
            None,
            auth.shared_secret.peek(),
        );

        Ok(Self {
            request_type: REQUEST_TYPE_QUERY.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            orderid: order_id,
            pasref,
            sha1hash,
        })
    }
}

/// `query` result + `<batchid>` + the void marker in `connector_feature_data` → `AttemptStatus`
/// (tech spec §12.4.7).
///
/// The difficulty of this flow is that **a `query` echoes the original `auth` leg forever**:
/// `result`, `message`, `authcode` and `pasref` never change no matter what happens to the payment
/// afterwards. The only field that moves is `<batchid>`, which goes from `-1` (never batched) to a
/// positive batch id the moment the transaction is settled — by an explicit `settle` *or* by
/// `autosettle flag="1"` at auth time. Both were verified live.
///
/// Consequently a **void is invisible to the gateway response**: a voided authorization still
/// reads `00 … AUTHORISED` with the same batch id it had before, whether it was voided before or
/// after settlement. The gateway does store the void as its own transaction under a synthetic
/// `_void_<orderid>` order id (tech spec §12.4.6), but reading it would need a *second* HTTP
/// request, which the one-request-per-invocation `ConnectorIntegrationV2` PSync contract does not
/// allow.
///
/// Rule 2 closes that gap without a second request: when UCS performs the void itself, the Void
/// flow republishes [`GlobalpaymentsRealexPaymentMetadata::void_pasref`] as `connector_metadata`,
/// which the caller returns on the sync request as `connector_feature_data`. A `00` on the
/// authorization leg is **not** evidence that the payment is live, so the marker wins.
///
/// | # | Condition | Result |
/// |---|---|---|
/// | 1 | `result != "00"` | `Failure` |
/// | 2 | `00` and the void marker is present in `connector_feature_data` | `Voided` |
/// | 3 | `00` and `batchid > 0` | `Charged` |
/// | 4 | `00` and `batchid` is `-1`, `0` or absent | `Authorized` |
///
/// A response-digest mismatch does **not** map to `IntegrityFailure` on this flow — unlike
/// Authorize, Capture, Void and Refund, whose mapping is untouched. See
/// [`GlobalpaymentsRealexPSyncResponse`] for why the `query` blueprint could not be recovered.
///
/// Refund state is deliberately not derived here: a rebate is likewise invisible to a query, and in
/// UCS a refunded payment's attempt status stays `Charged` — refund state lives on the refund
/// object, not on the payment attempt.
///
/// # Residual limitation — an out-of-band void is still undetectable
///
/// Rule 2 only fires for a void **UCS itself performed**, and only when the caller round-trips
/// `connector_feature_data` from the Void response into the sync request. That round-trip is
/// already a hard requirement on this connector — Capture, Void and Refund all depend on the same
/// channel for the original `<orderid>` — so it is not a new obligation, but it is a real one: a
/// caller that drops the metadata gets `Authorized` / `Charged` back.
///
/// A void performed **out of band** — through the merchant portal, or by a different integration —
/// leaves no trace UCS can see. The only gateway-side evidence is the `_void_<orderid>` leg, and
/// reading it needs a second request this flow will not make. Such a payment keeps syncing as
/// `Authorized` / `Charged`. This is inherent to the API and is documented in tech spec §12.4.8.
///
/// # Why the attempt status UCS already holds is not an input
///
/// An earlier revision guarded on `RouterDataV2::resource_common_data`
/// (`PaymentFlowData::status`) instead of the metadata marker. That guard can never fire on the
/// gRPC surface: `PaymentServiceGetRequest`
/// (`crates/types-traits/grpc-api-types/proto/payment.proto`) carries no attempt status, and
/// `impl ForeignTryFrom<(PaymentServiceGetRequest, ..)> for PaymentFlowData`
/// (`crates/types-traits/domain_types/src/types.rs`) hard-codes `status: AttemptStatus::Pending`.
/// Dead guards that read as safety are worse than no guard, so it was removed rather than left in
/// place. Carrying the current attempt status on the sync request is the proper long-term fix, but
/// it is a proto and shared-domain change and therefore out of scope for this connector.
/// `query` result codes that describe the **lookup**, not the payment.
///
/// The `5xx` family on this API is the integration-error family: `505` bad digest, `506` malformed
/// order id, `508` no such transaction, `502`/`503` malformed or disallowed request. None of them
/// is evidence about the underlying payment's outcome, so a sync that hits one must not overwrite
/// a known status.
fn is_query_lookup_fault(result: &str) -> bool {
    matches!(result, "502" | "503" | "505" | "506" | "508")
}

fn map_psync_attempt_status(
    result: &str,
    batchid: Option<i64>,
    voided_by_ucs: bool,
) -> AttemptStatus {
    // Rule 1a. A `5xx` result on a `query` says nothing about the payment: it says the *lookup*
    // failed. `508 Original transaction not found.` is what a caller that did not round-trip
    // `connector_feature_data` gets, because the order id then falls back to the request
    // reference; `505` is a digest mistake; `506` a malformed order id. Reporting `Failure` for
    // any of them would flip a perfectly healthy Charged payment to failed on a sync — the exact
    // trap `map_rsync_outcome` already refuses to fall into. Leave the attempt where it was and
    // let the structured connector error carry the fault.
    if is_query_lookup_fault(result) {
        return AttemptStatus::Unresolved;
    }

    // Rule 1b. Otherwise there is no pending/async state anywhere on this API, so every non-`00`
    // code is a genuine terminal outcome for the payment (tech spec §9.4).
    if result != RESULT_SUCCESS {
        return AttemptStatus::Failure;
    }

    // Rule 2. The query cannot observe a void, so the marker the Void flow published is the only
    // evidence there is — and it outranks the authorization leg's stale `00 … AUTHORISED`.
    if voided_by_ucs {
        return AttemptStatus::Voided;
    }

    match batchid {
        // Rule 3. Batched ⇒ settled, by an explicit `settle` or by `autosettle flag="1"`.
        Some(batch) if batch > 0 => AttemptStatus::Charged,
        // Rule 4. `-1` (or an empty/absent element) ⇒ authorized, not yet batched.
        _ => AttemptStatus::Authorized,
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsRealexPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        // A `query` response's `<sha1hash>` does not follow the documented blueprint and is not
        // reproducible (see `GlobalpaymentsRealexPSyncResponse`), so the check is **informational
        // only** on this flow: it is still computed and logged so that a future change on the
        // gateway's side becomes visible, but it never decides the status. `Skipped` remains the
        // normal outcome for the small `5xx` error document, which carries no digest at all.
        let hash_verification = verify_response_hash(&response, auth.shared_secret.peek());

        if hash_verification == HashVerification::Mismatch {
            tracing::debug!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex query response sha1hash did not match the documented \
                 response blueprint; `query` digests are not reproducible, so the check is \
                 informational for this flow and the response is processed on its merits"
            );
        }

        // The void marker the Void flow published, round-tripped by the caller. `PaymentsSyncData`
        // has no free-form `metadata` field, so `connector_feature_data` is the only channel.
        let voided_by_ucs =
            extract_followup_metadata(router_data.request.connector_feature_data.as_ref(), None)
                .is_some_and(|metadata| metadata.void_pasref.is_some());

        let status = map_psync_attempt_status(&response.result, response.batchid, voided_by_ucs);

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

        let sync_response = if response.is_success() {
            // A query echoes the original auth's `<pasref>`; keep the id we synced with when
            // the gateway omits it so the payment stays referenceable either way.
            let resource_id = response
                .pasref
                .clone()
                .map(DomainResponseId::ConnectorTransactionId)
                .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: response.srd.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: response.orderid.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: http_code,
                payment_account_reference: None,
            })
        } else {
            Err(ErrorResponse {
                status_code: http_code,
                // The `<result>` code verbatim — `"101"` for a queried decline, `"508"` for an
                // unknown order id, `"506"` for a malformed one, `"505"` for a bad digest.
                // Surfaced as a structured connector error, never as a gRPC Internal.
                code: response.result.clone(),
                message: message.clone(),
                reason: response.message.clone(),
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: response.pasref.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: sync_response,
            ..router_data
        })
    }
}

// =============================================================================
// RSYNC — `type="query"` on the `_rebate_` leg (tech spec §12.6)
// =============================================================================

/// The synthetic order-id prefix under which the gateway stores a **successful** `rebate`.
///
/// A `rebate` does not merely mutate the payment it refunds: the gateway records it as its own
/// transaction, keyed by the original order id with this prefix, and that transaction is readable
/// with an ordinary `type="query"` (tech spec §12.4.6 / §12.6). It is the **only** gateway-side
/// evidence of a refund's state, and therefore the whole basis of this flow.
///
/// Lowercase, no separator beyond the underscores, no numbering and no suffix. Eight alternative
/// spellings (`_rebate__rebate_…`, `_rebate_1_…`, `…_1`, `_rebate2_…`, …) were tried live and every
/// one returned `508 Original transaction not found.` The lookup happens to be case-insensitive on
/// the prefix, but the gateway echoes it back lowercased, so lowercase is what we always emit.
const REBATE_LEG_ORDER_ID_PREFIX: &str = "_rebate_";

/// The synthetic prefixes the gateway puts on the order id it **echoes** in an error document.
///
/// Every failed `settle` / `void` / `rebate` answers with `<orderid>_settle_…` / `_void_…` /
/// `_rebate_…` rather than the value we sent. A caller that persists that echo and feeds it back
/// would have us build `_rebate__rebate_<ORIG>`, which returns `508 Original transaction not found.`
/// — a silent "this refund does not exist" for a refund that does. The order id must always come
/// from the metadata Authorize published, so an already-prefixed value is rejected outright rather
/// than prefixed again (tech spec §12.6.2 item 4).
const GATEWAY_ECHO_ORDER_ID_PREFIXES: [&str; 3] = ["_rebate_", "_settle_", "_void_"];

/// The longest **unprefixed** original order id that can still be synced.
///
/// `<orderid>` accepts 50 characters and `_rebate_` consumes 8 of them, so an original order id
/// longer than 42 characters cannot be addressed on the rebate leg at all. UCS order ids are 26
/// characters, so this is headroom rather than a live constraint — but it must be an explicit,
/// actionable error rather than a silently truncated request that would `508`
/// (tech spec §12.6.2 item 3).
const RSYNC_ORIGINAL_ORDER_ID_MAX_LEN: usize = ORDER_ID_MAX_LEN - REBATE_LEG_ORDER_ID_PREFIX.len();

/// The RSync request is **byte-identical in shape** to the PSync request — same `type="query"`,
/// same elements, same digest blueprint. Only the `<orderid>` value differs: RSync sends
/// `_rebate_<ORIGINAL_ORDER_ID>` instead of `<ORIGINAL_ORDER_ID>`.
///
/// The alias exists because the macro layer derives per-flow helper types from the request type's
/// name and therefore needs a distinct identifier per flow; there is deliberately **no** second
/// struct and **no** RSync-specific hash builder (tech spec §12.6.1, §12.6.8 item 2).
pub type GlobalpaymentsRealexRSyncRequest = GlobalpaymentsRealexPSyncRequest;

/// The `_rebate_` leg's response is a **subset** of the shared response document, so the existing
/// struct parses it unchanged (tech spec §12.6.5).
///
/// A success carries `<result>00</result>`, `<message>AUTH CODE: nnnnnn</message>`, a real issuer
/// `<authcode>`, a `<batchid>` and — the field this whole flow turns on — `<pasref>`, which is the
/// pasref the `rebate` itself minted, i.e. exactly what UCS stores as `connector_refund_id`. A
/// failure is the small error document with only `@timestamp`, `<result>`, `<message>` and
/// `<orderid>`.
///
/// Two absences matter more than anything present:
///
/// * **No `<amount>` and no currency.** No `query` response on this API carries either, so a
///   partial refund is indistinguishable from a full one and no amount-based integrity object can
///   be populated (tech spec §12.6.7). It is not faked here.
/// * **No usable `<sha1hash>` semantics.** The `query` response digest blueprint is unrecovered
///   (see [`GlobalpaymentsRealexPSyncResponse`]); a mismatch is logged at `debug` and never decides
///   the status. The `IntegrityFailure` mapping on `auth` / `settle` / `void` / `rebate` is
///   untouched.
///
/// Ten elements come back as **empty elements** on this sandbox (`<cardissuer>`'s five children,
/// `<tss><result>`, `<threedsecure>`'s three children, `<srd>`), which is why the tolerant
/// deserialization the shared struct already applies is mandatory rather than advisory.
pub type GlobalpaymentsRealexRSyncResponse = GlobalpaymentsRealexPaymentsResponse;

impl<T>
    TryFrom<
        GlobalpaymentsRealexRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for GlobalpaymentsRealexRSyncRequest
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GlobalpaymentsRealexRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;

        // Exactly the resolution order the Refund flow implements — it has to be, because a refund
        // accepted under order id `X` is only ever readable back under `_rebate_X`.
        let metadata = extract_followup_metadata(
            request.connector_feature_data.as_ref(),
            request.refund_connector_metadata.as_ref(),
        );

        let metadata_order_id = metadata.as_ref().map(|metadata| metadata.orderid.clone());

        // `connector_order_id` is not a mere convenience here: hyperswitch hard-codes
        // `connector_feature_data: None` on the refund-sync request it builds — RSync is the one
        // follow-up flow that does *not* receive the Authorize metadata — while populating
        // `connector_order_id` from the same reference the order id is derived from. So the
        // fallback below is the **live** path when this connector is driven from hyperswitch, and
        // the metadata branch is what direct gRPC callers exercise. Both must work.
        //
        // When both are present and disagree, the metadata wins — it is literally the string
        // Authorize sent as `<orderid>`, whereas `connector_order_id` is the caller's rendering of
        // it — but the divergence is loud, because it means one of the two channels is carrying a
        // reference this payment was never booked under and every RSync built from the wrong one
        // will `508`.
        if let (Some(metadata_order_id), Some(connector_order_id)) = (
            metadata_order_id.as_ref(),
            request.connector_order_id.as_ref(),
        ) {
            if metadata_order_id != connector_order_id {
                tracing::warn!(
                    connector = "globalpayments_realex",
                    metadata_order_id = %metadata_order_id,
                    connector_order_id = %connector_order_id,
                    "GlobalpaymentsRealex refund sync was given two different original order ids; \
                     using the one from connector_feature_data, which is the value Authorize sent \
                     as <orderid>"
                );
            }
        }

        let original_order_id = metadata_order_id
            .or_else(|| request.connector_order_id.clone())
            .ok_or_else(|| {
                // A request built without it is guaranteed to `508`, which rule 5 would then have
                // to surface as "contradictory" — so fail here, naming the field, instead.
                report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.orderid",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "GlobalpaymentsRealex refund sync needs the original <orderid>: echo \
                             the Authorize response's connector_feature_data back on the refund \
                             sync request, or set connector_order_id to the original order id"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
            })?;

        // Charset and length are validated on the **unprefixed** value, so the 8 characters
        // `_rebate_` costs are accounted for against the gateway's 50-character limit.
        let original_order_id = sanitize_order_id(&original_order_id)?;

        if GATEWAY_ECHO_ORDER_ID_PREFIXES
            .iter()
            .any(|prefix| original_order_id.starts_with(prefix))
        {
            return Err(report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data.orderid",
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "GlobalpaymentsRealex refund sync was given an already-prefixed order id \
                         ('{original_order_id}'), which is a gateway echo from a failed follow-up \
                         request rather than the order id sent on the original auth; prefixing it \
                         again would query a transaction that cannot exist"
                    )),
                    ..Default::default()
                },
            }));
        }

        if original_order_id.len() > RSYNC_ORIGINAL_ORDER_ID_MAX_LEN {
            return Err(report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data.orderid",
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "GlobalpaymentsRealex refund sync needs the original <orderid> to be at \
                         most {RSYNC_ORIGINAL_ORDER_ID_MAX_LEN} characters: the rebate leg is \
                         addressed as '{REBATE_LEG_ORDER_ID_PREFIX}<orderid>' and <orderid> \
                         accepts {ORDER_ID_MAX_LEN} characters in total"
                    )),
                    ..Default::default()
                },
            }));
        }

        // THE lookup key. `query` keys on `<orderid>` and nothing else.
        let order_id = format!("{REBATE_LEG_ORDER_ID_PREFIX}{original_order_id}");

        // Sent purely for shape uniformity with `settle` / `void` / `rebate`. The gateway ignores
        // `<pasref>` on a query — verified live on the `_rebate_` leg exactly as on the auth leg:
        // garbage, a foreign pasref, an empty element and omission all return the same document —
        // so a pasref can never be a lookup key here, only the identity check the response mapping
        // performs (tech spec §12.6.2).
        let pasref = Some(request.connector_refund_id.trim().to_string())
            .filter(|pasref| !pasref.is_empty());

        let timestamp = current_timestamp()?;
        let merchant_id = auth.merchant_id.clone().expose();
        // The order-id slot carries the **prefixed** string, character for character as it appears
        // in `<orderid>` — the ordinary mirror rule. A query carries no amount and no currency, so
        // both of those slots stay empty and the blueprint is the one Void and PSync already use:
        // `timestamp.merchantid.orderid...` (tech spec §12.6.1).
        let sha1hash = build_reference_request_hash(
            &timestamp,
            &merchant_id,
            &order_id,
            None,
            None,
            auth.shared_secret.peek(),
        );

        Ok(Self {
            request_type: REQUEST_TYPE_QUERY.to_string(),
            timestamp,
            merchantid: auth.merchant_id,
            account: Some(auth.account),
            orderid: order_id,
            pasref,
            sha1hash,
        })
    }
}

/// What the `_rebate_` leg's answer tells us about **our** refund.
///
/// The three variants exist because the gateway can only answer two questions — "does a rebate leg
/// exist for this order?" and "which pasref does it carry?" — and neither is by itself a statement
/// about the refund UCS is syncing.
#[derive(Debug, Clone)]
enum GlobalpaymentsRealexRSyncOutcome {
    /// Rule 1: the leg exists **and** its own reference is our `connector_refund_id`.
    Refunded,
    /// Rule 4: no leg exists and we hold no gateway reference, so no money moved.
    NotRefunded,
    /// Rules 2, 3, 5, 6 and 7: the gateway's answer does not identify our refund. The refund status
    /// is left **unchanged** and the connector's own code and message are surfaced instead.
    Indeterminate(String),
}

/// `query` result on the `_rebate_` leg → `RefundStatus` (tech spec §12.6.6).
///
/// > **HARD CORRECTNESS BAR.** A fully refunded payment must not sync back as merely charged. A
/// > partial refund must be distinguishable from a full one **if the gateway makes that
/// > observable**. If the gateway genuinely cannot express a state, the mapping must **leave the
/// > status unchanged** and surface an actionable error rather than inventing a terminal state.
/// > **Never map an ambiguous response to success.**
///
/// The inputs are only these: `<result>`, `<message>` and `<pasref>` from the query, and
/// `RefundSyncData::connector_refund_id` — the pasref the `rebate` minted. `RefundSyncData::
/// refund_status` is **not** an input: the gRPC conversion hard-codes it to `Pending`, exactly as
/// `PaymentFlowData::status` is for PSync, so "leave the status unchanged" cannot be expressed by
/// echoing it back. It is expressed the only way this surface allows — an `ErrorResponse` with
/// `attempt_status: None`, which surfaces the connector code and message without asserting any
/// refund state (the gRPC layer then reports `REFUND_STATUS_UNSPECIFIED`).
///
/// | # | Condition | Outcome |
/// |---|---|---|
/// | 1 | `00` **and** `pasref == connector_refund_id` | `Success` |
/// | 2 | `00` **and** `connector_refund_id` empty/absent | unchanged — the leg is someone else's refund |
/// | 3 | `00` **and** `pasref != connector_refund_id` | unchanged — the leg is a *different* rebate |
/// | 4 | `508` **and** `connector_refund_id` empty/absent | `Failure` |
/// | 5 | `508` **and** `connector_refund_id` present | unchanged — contradictory, most likely a wrong `<orderid>` |
/// | 6 | any other `5xx` (`505`, `506`, `503`, `502`) | unchanged — integration/config fault, not a refund outcome |
/// | 7 | `1xx` / `2xx` / `3xx` | unchanged — never observed; a leg only exists for a *successful* rebate |
/// | 8 | `<sha1hash>` mismatch | ignored for status purposes, logged at `debug` |
///
/// **Rule 3 is not hypothetical.** The gateway mints exactly one `_rebate_<ORIGINAL>` key per
/// payment, and on an account not enabled for multiple refunds a second rebate is refused with
/// `512 This transaction has already been rebated and cannot be rebated again.` while the leg keeps
/// answering `00` with the **first** rebate's pasref. Without the identity check, syncing that
/// rejected second refund would report `Success`. The same rule catches a refund raised out of band
/// — through the merchant portal or another integration — which lands in the same gateway-side leg.
///
/// **Rule 4 is not a race.** The leg is readable ~1 s after the rebate response and does not change
/// afterwards, and a *failed* rebate creates no leg at all (verified against three controls: no
/// rebate, an over-refund rejected `512`, and a rebate of an uncaptured auth rejected `512`). So
/// `508` reliably means "no successful rebate exists for this order" — its one ambiguity, a wrong
/// order id, is exactly what rule 5 refuses to guess about.
///
/// `RefundStatus::Pending` is **never** produced. This API has no asynchronous refund state: a
/// `rebate` is decided in its own HTTP 200 and its leg is queryable immediately.
fn map_rsync_outcome(
    result: &str,
    response_pasref: Option<&str>,
    connector_refund_id: &str,
) -> GlobalpaymentsRealexRSyncOutcome {
    let connector_refund_id = connector_refund_id.trim();
    let response_pasref = response_pasref.map(str::trim).filter(|p| !p.is_empty());

    match result {
        RESULT_SUCCESS => {
            if connector_refund_id.is_empty() {
                // Rule 2. A refund with no gateway reference never got a `00` from `rebate`, so a
                // leg that exists must belong to a different refund. Reporting `Success` here would
                // credit our refund with someone else's money movement.
                return GlobalpaymentsRealexRSyncOutcome::Indeterminate(
                    "GlobalpaymentsRealex holds a rebate leg for this order, but this refund has \
                     no connector_refund_id, so the leg cannot be attributed to it; the refund \
                     status is left unchanged"
                        .to_string(),
                );
            }

            match response_pasref {
                // Rule 1. The gateway holds a rebate leg whose own reference is *our* refund's
                // reference — the only condition under which it has confirmed **this** refund.
                Some(pasref) if pasref == connector_refund_id => {
                    GlobalpaymentsRealexRSyncOutcome::Refunded
                }
                // Rule 3. A different rebate on the same payment: a prior successful refund (after
                // which ours would have been rejected `512 already been rebated`), or an out-of-band
                // refund. This is the trap the identity check exists to catch.
                other => GlobalpaymentsRealexRSyncOutcome::Indeterminate(format!(
                    "GlobalpaymentsRealex rebate leg reports pasref {} but this refund's \
                     connector_refund_id is {connector_refund_id}; the leg belongs to a different \
                     rebate on the same payment, so the refund status is left unchanged",
                    other.unwrap_or("<absent>")
                )),
            }
        }
        RESULT_ORIGINAL_TRANSACTION_NOT_FOUND => {
            if connector_refund_id.is_empty() {
                // Rule 4. No leg exists and we hold no reference: the refund provably did not move
                // money. The leg appears with no lag, so this is not a race with settlement.
                GlobalpaymentsRealexRSyncOutcome::NotRefunded
            } else {
                // Rule 5. Contradictory: we hold a pasref the gateway minted, yet it reports no
                // leg. The overwhelmingly likely cause is a wrong `<orderid>` — dropped or
                // incorrect metadata — not a vanished refund. Flipping an already-successful refund
                // to `Failure` on that evidence would be inventing a terminal state.
                GlobalpaymentsRealexRSyncOutcome::Indeterminate(format!(
                    "GlobalpaymentsRealex reports no rebate leg for this order, yet this refund \
                     holds connector_refund_id {connector_refund_id}; the <orderid> the sync was \
                     built from is most likely wrong, so the refund status is left unchanged"
                ))
            }
        }
        // Rules 6 and 7. `505` (digest), `506` (schema/account), `503` (not allowed) and `502`
        // (mandatory field) are integration or configuration faults, not refund outcomes — mapping
        // them onto a refund state would encode "our request was malformed" as "the customer's
        // money did/did not move". A `1xx`/`2xx`/`3xx` on a `_rebate_` query has never been
        // observed and is treated as unknown rather than guessed at.
        other => GlobalpaymentsRealexRSyncOutcome::Indeterminate(format!(
            "GlobalpaymentsRealex answered the rebate leg query with result {other}, which is not \
             a refund outcome; the refund status is left unchanged"
        )),
    }
}

impl TryFrom<ResponseRouterData<GlobalpaymentsRealexRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        value: ResponseRouterData<GlobalpaymentsRealexRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = value;

        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)
            .change_context(ConnectorError::ResponseHandlingFailed {
                context: Default::default(),
            })?;

        // Rule 8. The `query` response digest blueprint is unrecovered (see
        // `GlobalpaymentsRealexPSyncResponse`), and a mismatch is indistinguishable from "blueprint
        // unknown", so letting it decide the status would fail 100% of syncs. Computed and logged
        // at `debug` only; the `IntegrityFailure` mapping on the four money-moving flows is
        // untouched.
        if verify_response_hash(&response, auth.shared_secret.peek()) == HashVerification::Mismatch
        {
            tracing::debug!(
                connector = "globalpayments_realex",
                order_id = ?response.orderid,
                "GlobalpaymentsRealex rebate-leg query response sha1hash did not match the \
                 documented response blueprint; `query` digests are not reproducible, so the check \
                 is informational for this flow and the response is processed on its merits"
            );
        }

        let outcome = map_rsync_outcome(
            &response.result,
            response.pasref.as_deref(),
            &router_data.request.connector_refund_id,
        );

        let message = response
            .message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

        let previous_status = router_data.resource_common_data.status;

        let (status, refund_response) = match outcome {
            GlobalpaymentsRealexRSyncOutcome::Refunded => (
                RefundStatus::Success,
                Ok(RefundsResponseData {
                    // Proven identical to `<pasref>` by rule 1's check, so this is the leg's own
                    // reference and not an alias for anything else.
                    connector_refund_id: router_data.request.connector_refund_id.clone(),
                    refund_status: RefundStatus::Success,
                    status_code: http_code,
                    // No amount and no currency are returned by any `query` on this API, so no
                    // amount-based integrity object can be populated. It is not faked.
                    acquirer_reference_number: None,
                }),
            ),
            GlobalpaymentsRealexRSyncOutcome::NotRefunded => (
                RefundStatus::Failure,
                Err(ErrorResponse {
                    status_code: http_code,
                    code: response.result.clone(),
                    message: message.clone(),
                    reason: response.message.clone(),
                    attempt_status: Some(FlowStatus::Refund(RefundStatus::Failure)),
                    connector_transaction_id: response.pasref.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
            ),
            GlobalpaymentsRealexRSyncOutcome::Indeterminate(diagnostic) => {
                tracing::warn!(
                    connector = "globalpayments_realex",
                    order_id = ?response.orderid,
                    result = %response.result,
                    diagnostic = %diagnostic,
                    "GlobalpaymentsRealex refund sync could not attribute the rebate leg to this \
                     refund; leaving the refund status unchanged"
                );

                (
                    // Nothing is asserted about the refund: the status UCS already holds is carried
                    // through untouched and `attempt_status: None` tells the gRPC layer to report
                    // `REFUND_STATUS_UNSPECIFIED` rather than a terminal state.
                    previous_status,
                    Err(ErrorResponse {
                        status_code: http_code,
                        // The `<result>` code verbatim — `"00"` for rules 2 and 3, `"508"` for
                        // rule 5, `"505"` / `"506"` / … for rule 6.
                        code: response.result.clone(),
                        // The connector's `<message>` verbatim. On a `505` this text is the only
                        // thing separating a bad `sha1hash` from other causes.
                        message: message.clone(),
                        // The diagnostic names both references where rule 3 applies, so an operator
                        // can see which rebate the gateway is reporting.
                        reason: Some(diagnostic),
                        attempt_status: None,
                        connector_transaction_id: response.pasref.clone(),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                )
            }
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: refund_response,
            ..router_data
        })
    }
}

// =============================================================================
// 3DS2 — Global Payments 3D Secure 2 JSON API (merchant-driven authentication)
// =============================================================================
//
// Global Payments **3D Secure 2 JSON API** — the merchant-driven authentication legs.
//
// This section speaks the 3DS2 JSON API; everything above it speaks the legacy
// *RealEx* **XML** API. The two products share exactly one thing: the two-stage SHA-1 digest
// primitive (`realex_digest`) and the Shared Secret it is keyed on. Everything else differs:
//
// | | XML API (above) | 3DS2 JSON API (this section) |
// |---|---|---|
// | Host | `api[.sandbox].realexpayments.com` (`base_url`) | `api[.sandbox].globalpay-ecommerce.com` (`secondary_base_url`) |
// | Encoding | XML | JSON |
// | Auth | `<sha1hash>` **element in the body** | `Authorization: securehash …` **header** |
// | Timestamp | `YYYYMMDDHHMMSS` | `yyyy-MM-ddTHH:mm:ss.SSSSSS` |
// | Mastercard | `MC` | `MASTERCARD` |
// | Failure | always HTTP 200, `<result>` carries the outcome | real HTTP 4xx/5xx |
//
// **The `MC` / `MASTERCARD` divergence is the reason these two type sets stay distinct even in
// one file.** Keeping the two mappers apart makes an accidental "let's unify the card-type
// mapper" refactor visibly wrong rather than subtly wrong.
// [`tests::the_two_scheme_mappers_disagree_on_mastercard`] guards it.
//
// Flow mapping (vendor doc `sources/source_2_3d_secure_two.md`):
//
// | # | Call | UCS flow | Side |
// |---|---|---|---|
// | 1 | `POST /3ds2/protocol-versions` | `PreAuthenticate` | server |
// | 2 | `POST {method_url}` (`threeDSMethodData`) | *(redirect emitted by 1)* | browser |
// | 3 | `POST /3ds2/authentications` | `Authenticate` | server |
// | 4 | `POST {challenge_request_url}` (`creq`) | *(redirect emitted by 3)* | browser |
// | 5 | `GET /3ds2/authentications/{sid}` | `PostAuthenticate` | server |
// | 6 | XML `auth` with `<mpi>` | `Authorize` (already built) | server |
//
// A frictionless authentication skips 4 and 5 — call 3 already returns `eci`,
// `authentication_value` and `ds_trans_id`.

// =============================================================================
// CONSTANTS
// =============================================================================

/// `X-GP-VERSION` — the 3DS2 protocol version this integration speaks. Global Payments
/// recommends `2.2.0` for all merchants; the value also determines which request and response
/// fields the API will accept and return.
pub const GP_3DS2_VERSION: &str = "2.2.0";

/// `POST` — *Check version*.
pub const PATH_PROTOCOL_VERSIONS: &str = "3ds2/protocol-versions";

/// `POST` — *Initiate authentication*; also the `GET` prefix for *Obtain authentication data*.
pub const PATH_AUTHENTICATIONS: &str = "3ds2/authentications";

/// Query marker appended to the **Method** notification URL so the post-DDC browser return can be
/// told apart from the post-challenge one.
///
/// Both ACS returns POST to the same hyperswitch endpoint. Hyperswitch puts the query string into
/// `redirect_response.params` and the form body into `redirect_response.payload`, so:
///
/// * Method Notification URL  = `{continue_redirection_url}?gp3ds=method` → `params` non-empty
/// * Challenge Notification URL = `{continue_redirection_url}` (bare)     → `params` empty
///
/// which is byte-identical to the discriminator Cybersource and Barclaycard already use
/// (params non-empty ⇒ `Authenticate`, else ⇒ `PostAuthenticate`).
pub const METHOD_RETURN_MARKER: &str = "gp3ds=method";

/// Form field the ACS (and our synthesised no-DDC self-POST) carries the encoded method data in.
pub const FIELD_THREE_DS_METHOD_DATA: &str = "threeDSMethodData";

/// Form field the ACS carries the base64 CRes in after a challenge.
pub const FIELD_CRES: &str = "cres";

/// Form field the browser posts the base64 CReq to the ACS in.
pub const FIELD_CREQ: &str = "creq";

/// Form field our synthesised no-DDC self-POST carries so the `Authenticate` leg knows the 3DS
/// Method never ran. Present ⇒ `UNAVAILABLE`; absent ⇒ the ACS really did complete the method.
pub const FIELD_THREE_DS_METHOD_COMPLETION: &str = "threeDSMethodCompletion";

/// `error_code` surfaced when the card is not enrolled in 3DS2.
///
/// UCS models 3DS outcomes with the EMVCo [`TransactionStatus`] enum, which has no "not enrolled"
/// variant, and silently downgrading a payment the merchant marked `three_ds` would change the
/// liability model without telling anyone. So `enrolled: false` fails the authentication with a
/// code the caller can match on and retry as `no_three_ds` if it wants to.
pub const ERROR_CODE_NOT_ENROLLED: &str = "GP3DS2_NOT_ENROLLED";

/// `error_code` surfaced when the authentication resolved to a non-liability-shifting status.
pub const ERROR_CODE_AUTHENTICATION_REJECTED: &str = "GP3DS2_AUTHENTICATION_REJECTED";

// =============================================================================
// CARD SCHEME (the `MASTERCARD` half of the divergence)
// =============================================================================

/// Card scheme vocabulary of the **3DS2 JSON API**.
///
/// Mastercard is `MASTERCARD` here and `MC` on the XML API
/// ([`super::transformers::map_card_type`]). Both are correct for their own API and **must not**
/// be unified — a single payment sends both values, one per leg.
pub fn map_3ds2_scheme<T>(
    card: &Card<T>,
) -> Result<&'static str, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    // Prefer the network the caller supplied; fall back to BIN detection. Never guess a brand —
    // the 3DS2 API cross-validates `scheme` against the PAN and answers `501 Not Implemented`
    // for a scheme it does not support.
    if let Some(network) = card.card_network.as_ref() {
        return match network {
            CardNetwork::Visa => Ok("VISA"),
            CardNetwork::Mastercard => Ok("MASTERCARD"),
            CardNetwork::AmericanExpress => Ok("AMEX"),
            CardNetwork::DinersClub => Ok("DINERS"),
            CardNetwork::Discover => Ok("DISCOVER"),
            CardNetwork::JCB => Ok("JCB"),
            unsupported => Err(report!(IntegrationError::NotImplemented(
                format!(
                    "Card network {unsupported:?} is not supported by the GlobalpaymentsRealex \
                     3DS2 API"
                ),
                IntegrationErrorContext::default(),
            ))),
        };
    }

    match get_card_issuer(card.card_number.peek())? {
        CardIssuer::Visa => Ok("VISA"),
        CardIssuer::Master => Ok("MASTERCARD"),
        CardIssuer::AmericanExpress => Ok("AMEX"),
        CardIssuer::DinersClub => Ok("DINERS"),
        CardIssuer::Discover => Ok("DISCOVER"),
        CardIssuer::JCB => Ok("JCB"),
        unsupported => Err(report!(IntegrationError::NotImplemented(
            format!(
                "Card issuer {unsupported} is not supported by the GlobalpaymentsRealex 3DS2 API"
            ),
            IntegrationErrorContext::default(),
        ))),
    }
}

// =============================================================================
// SMALL TOLERANT WIRE TYPES
// =============================================================================

/// A field the API documents as a boolean but has been observed to send as the strings
/// `"true"` / `"false"` on some responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Gp3ds2Bool {
    Bool(bool),
    Str(String),
}

impl Gp3ds2Bool {
    pub fn is_true(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::Str(value) => value.eq_ignore_ascii_case("true"),
        }
    }
}

/// `error_code` is documented as an integer and sent as a JSON string in the vendor's own worked
/// example, so accept either.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Gp3ds2Scalar {
    Str(String),
    Int(i64),
}

impl std::fmt::Display for Gp3ds2Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Str(value) => f.write_str(value),
            Self::Int(value) => write!(f, "{value}"),
        }
    }
}

// =============================================================================
// TRANSACTION STATUS
// =============================================================================

/// The 3DS2 API's `status` field.
///
/// Note the wire values carry an `AUTHENTICATION_` prefix that the EMVCo names do not
/// (`AUTHENTICATION_FAILED`, not `FAILED`). The mapping onto UCS's [`TransactionStatus`] is 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Gp3ds2Status {
    /// EMVCo `Y`.
    #[serde(rename = "AUTHENTICATION_SUCCESSFUL")]
    AuthenticationSuccessful,
    /// EMVCo `A` — "attempts processing performed". **This is a success**: liability still shifts
    /// (ECI 06 on Visa, 01 on Mastercard) and the XML `auth` must still be sent.
    #[serde(rename = "AUTHENTICATION_ATTEMPTED_BUT_NOT_SUCCESSFUL")]
    AuthenticationAttemptedButNotSuccessful,
    /// EMVCo `N`.
    #[serde(rename = "AUTHENTICATION_FAILED")]
    AuthenticationFailed,
    /// EMVCo `U`.
    #[serde(rename = "AUTHENTICATION_COULD_NOT_BE_PERFORMED")]
    AuthenticationCouldNotBePerformed,
    /// EMVCo `R`.
    #[serde(rename = "AUTHENTICATION_ISSUER_REJECTED")]
    AuthenticationIssuerRejected,
    /// EMVCo `C`.
    #[serde(rename = "CHALLENGE_REQUIRED")]
    ChallengeRequired,
    /// EMVCo `D`.
    #[serde(rename = "DECOUPLED_AUTHENTICATION_CONFIRMED")]
    DecoupledAuthenticationConfirmed,
    /// EMVCo `I`.
    #[serde(rename = "CHALLENGE_PREFERENCE_ACKNOWLEDGED_INFORMATIONAL_ONLY")]
    ChallengePreferenceAcknowledgedInformationalOnly,
    /// Any status value added to the API after this integration was written. Deserialising into a
    /// catch-all rather than failing keeps an unknown status a *business* outcome we decline
    /// rather than a parse error that loses the whole response.
    #[serde(other)]
    Unknown,
}

impl Gp3ds2Status {
    /// The EMVCo transaction status this maps onto.
    pub fn to_transaction_status(self) -> TransactionStatus {
        match self {
            Self::AuthenticationSuccessful => TransactionStatus::Success,
            Self::AuthenticationAttemptedButNotSuccessful => TransactionStatus::NotVerified,
            Self::AuthenticationFailed => TransactionStatus::Failure,
            // No "not performed" distinction beyond `U`; `Unknown` is treated the same way so an
            // unrecognised status can never masquerade as an authenticated one.
            Self::AuthenticationCouldNotBePerformed | Self::Unknown => {
                TransactionStatus::VerificationNotPerformed
            }
            Self::AuthenticationIssuerRejected => TransactionStatus::Rejected,
            Self::ChallengeRequired => TransactionStatus::ChallengeRequired,
            Self::DecoupledAuthenticationConfirmed => {
                TransactionStatus::ChallengeRequiredDecoupledAuthentication
            }
            Self::ChallengePreferenceAcknowledgedInformationalOnly => {
                TransactionStatus::InformationOnly
            }
        }
    }

    /// Whether the browser must be sent to the ACS.
    ///
    /// Branch on the **status**, never on the presence of `challenge_request_url` /
    /// `encoded_creq`: an ACS can return `CHALLENGE_REQUIRED` without `challenge_mandated`, and a
    /// field-presence test would silently treat that payment as frictionless.
    pub fn is_challenge(self) -> bool {
        matches!(
            self,
            Self::ChallengeRequired | Self::DecoupledAuthenticationConfirmed
        )
    }

    /// Whether the payment may proceed to the XML `auth`.
    ///
    /// `ATTEMPTED_BUT_NOT_SUCCESSFUL` counts: liability shifts on an attempt. `FAILED`,
    /// `ISSUER_REJECTED`, `COULD_NOT_BE_PERFORMED`, the informational-only status and any
    /// unrecognised value do not.
    pub fn is_authorisable(self) -> bool {
        matches!(
            self,
            Self::AuthenticationSuccessful | Self::AuthenticationAttemptedButNotSuccessful
        )
    }
}

// =============================================================================
// BROWSER DATA ENUMS
// =============================================================================

/// `browser_data.color_depth`.
///
/// EMVCo's guidance for a depth that is not one of the accepted values is to submit the closest
/// **lower** one (a browser reporting 30 sends `TWENTY_FOUR_BITS`).
pub fn map_color_depth(color_depth: Option<u8>) -> &'static str {
    match color_depth.unwrap_or(24) {
        0..=1 => "ONE_BIT",
        2..=3 => "TWO_BITS",
        4..=7 => "FOUR_BITS",
        8..=14 => "EIGHT_BITS",
        15 => "FIFTEEN_BITS",
        16..=23 => "SIXTEEN_BITS",
        24..=31 => "TWENTY_FOUR_BITS",
        32..=47 => "THIRTY_TWO_BITS",
        _ => "FORTY_EIGHT_BITS",
    }
}

/// `method_url_completion` — did the ACS device-profiling step run?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Gp3ds2MethodUrlCompletion {
    /// The ACS returned a Method URL and the browser completed the profiling POST.
    #[serde(rename = "YES")]
    Yes,
    /// The ACS offered no Method URL, so profiling never happened.
    #[serde(rename = "UNAVAILABLE")]
    Unavailable,
}

// =============================================================================
// BASE64 (the ACS is inconsistent about which alphabet it uses)
// =============================================================================

/// Decode a base64 payload that may be standard or URL-safe, padded or not.
///
/// Global Payments emits `threeDSMethodData` as **base64url, unpadded**, while an ACS `cres` is
/// typically **standard** base64 and is sometimes padded. Rather than guessing per field, try
/// every engine — the four alphabets are mutually compatible on well-formed input, so the first
/// one that decodes is the right one.
pub fn decode_base64_tolerant(input: &str) -> Option<Vec<u8>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    consts::BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(trimmed)
        .or_else(|_| consts::BASE64_ENGINE_URL_SAFE.decode(trimmed))
        .or_else(|_| consts::BASE64_ENGINE_STD_NO_PAD.decode(trimmed))
        .or_else(|_| consts::BASE64_ENGINE.decode(trimmed))
        .ok()
}

/// Decode a base64 payload and read `threeDSServerTransID` out of the JSON it wraps.
///
/// Serves both browser returns: the ACS echo of `threeDSMethodData` after device profiling and
/// the `cres` it posts after a challenge. Both are base64 JSON objects carrying that key.
pub fn server_trans_id_from_encoded(encoded: &str) -> Option<String> {
    let decoded = decode_base64_tolerant(encoded)?;
    let value: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    value
        .get("threeDSServerTransID")
        .and_then(|id| id.as_str())
        .map(str::to_owned)
}

/// Pull a string field out of the form body hyperswitch forwarded on
/// `redirect_response.payload`.
///
/// The payload is the ACS's `application/x-www-form-urlencoded` POST decoded into a JSON object,
/// so every value arrives as a string.
fn payload_field(payload: &serde_json::Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .filter(|value| !value.is_empty())
}

/// Base64url-encode the `threeDSMethodData` object the ACS would have produced, for the branch
/// where there is no ACS Method URL at all.
fn encode_synthetic_method_data(server_trans_id: &str, notification_url: &str) -> String {
    let payload = serde_json::json!({
        "threeDSServerTransID": server_trans_id,
        "threeDSMethodNotificationURL": notification_url,
    });
    consts::BASE64_ENGINE_URL_SAFE_NO_PAD.encode(payload.to_string())
}

// =============================================================================
// SHARED REQUEST-BUILDING HELPERS
// =============================================================================

fn missing(field_name: &'static str) -> error_stack::Report<IntegrationError> {
    report!(IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext::default(),
    })
}

/// Extract the raw card, rejecting every other payment method.
fn require_card<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>(
    payment_method_data: &Option<PaymentMethodData<T>>,
) -> Result<Card<T>, error_stack::Report<IntegrationError>> {
    match payment_method_data {
        Some(PaymentMethodData::Card(card)) => Ok(card.clone()),
        Some(_) => Err(report!(IntegrationError::NotSupported {
            message: "3DS2 authentication is only supported for cards".to_string(),
            connector: "globalpayments_realex",
            context: IntegrationErrorContext::default(),
        })),
        None => Err(missing("payment_method_data")),
    }
}

/// The notification URL both browser returns are built from.
///
/// `router_return_url` is unusable here: it is absent on the `Authenticate` leg and it routes to
/// `PaymentRedirectSync` rather than to the completion handler. If `continue_redirection_url` is
/// absent we fail fast rather than sending a placeholder the ACS can never come back through —
/// the shopper would sit on the ACS page forever and the payment would never resolve.
fn require_continue_redirection_url(
    continue_redirection_url: Option<&url::Url>,
) -> Result<url::Url, error_stack::Report<IntegrationError>> {
    continue_redirection_url
        .cloned()
        .ok_or_else(|| missing("continue_redirection_url"))
}

/// The Method Notification URL: the completion endpoint plus the [`METHOD_RETURN_MARKER`] query
/// marker that tells the two browser returns apart.
fn method_notification_url(continue_redirection_url: &url::Url) -> String {
    let base = continue_redirection_url.as_str().trim_end_matches('?');
    if base.contains('?') {
        format!("{base}&{METHOD_RETURN_MARKER}")
    } else {
        format!("{base}?{METHOD_RETURN_MARKER}")
    }
}

/// `merchant_contact_url` is a mandatory field with no home anywhere in the UCS payment model, so
/// it is derived from the origin of the merchant's own return URL.
///
/// A compromise, documented as such: the field is meant to point at a customer-care or "about"
/// page, and the correct long-term fix is a merchant-configurable MCA field. The origin is at
/// least a real, merchant-owned URL on the same site the shopper is already on.
fn merchant_contact_url(continue_redirection_url: &url::Url) -> String {
    let origin = continue_redirection_url.origin().ascii_serialization();
    if origin == "null" {
        continue_redirection_url.as_str().to_owned()
    } else {
        origin
    }
}

// =============================================================================
// CALL 1 — POST /3ds2/protocol-versions  (PreAuthenticate)
// =============================================================================

/// *Check version*: does this PAN support 3DS2, and does its ACS want to profile the device?
#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2ProtocolVersionsRequest {
    /// **Must** be the same string the `securehash` header was computed over.
    pub request_timestamp: String,
    pub merchant_id: Secret<String>,
    pub account_id: Secret<String>,
    pub number: Secret<String>,
    /// `VISA` | `MASTERCARD` | `AMEX` | `DINERS` | `DISCOVER` | `JCB` — see [`map_3ds2_scheme`].
    pub scheme: String,
    pub method_notification_url: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
        &str,
    )> for Gp3ds2ProtocolVersionsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (router_data, request_timestamp): (
            &RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            &str,
        ),
    ) -> Result<Self, Self::Error> {
        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;
        let card = require_card(&router_data.request.payment_method_data)?;
        let continue_redirection_url = require_continue_redirection_url(
            router_data.request.continue_redirection_url.as_ref(),
        )?;

        Ok(Self {
            request_timestamp: request_timestamp.to_owned(),
            merchant_id: auth.merchant_id,
            account_id: auth.account,
            number: Secret::new(card.card_number.peek().to_string()),
            scheme: map_3ds2_scheme(&card)?.to_owned(),
            method_notification_url: method_notification_url(&continue_redirection_url),
        })
    }
}

/// `method_data` — the object the browser POSTs to the ACS Method URL.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gp3ds2MethodData {
    pub three_ds_server_trans_id: Option<String>,
    pub three_ds_method_notification_url: Option<String>,
    /// Base64url JSON of the two fields above; this is the literal `threeDSMethodData` form value.
    pub encoded_method_data: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gp3ds2ProtocolVersionsResponse {
    pub server_trans_id: Option<String>,
    pub enrolled: Option<Gp3ds2Bool>,
    pub ds_protocol_version_start: Option<String>,
    pub ds_protocol_version_end: Option<String>,
    pub acs_protocol_version_start: Option<String>,
    pub acs_protocol_version_end: Option<String>,
    pub acs_info_indicator: Option<Vec<String>>,
    /// Absent when the issuer's ACS does not support device profiling.
    pub method_url: Option<String>,
    pub method_data: Option<Gp3ds2MethodData>,
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<Gp3ds2ProtocolVersionsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Gp3ds2ProtocolVersionsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let enrolled = response
            .enrolled
            .as_ref()
            .map(Gp3ds2Bool::is_true)
            .unwrap_or(false);

        let server_trans_id = match response.server_trans_id.clone() {
            Some(id) if enrolled => id,
            // Not enrolled (or enrolled with no id, which cannot be driven further): decline
            // rather than silently downgrading a `three_ds` payment to a non-3DS auth.
            _ => {
                return Ok(Self {
                    response: Err(ErrorResponse {
                        status_code: item.http_code,
                        code: ERROR_CODE_NOT_ENROLLED.to_string(),
                        message: "Card is not enrolled in 3D Secure 2".to_string(),
                        reason: Some(
                            "GlobalpaymentsRealex 3DS2 returned enrolled=false for this card. \
                             The payment was requested as three_ds; retry as no_three_ds if a \
                             non-authenticated authorisation is acceptable."
                                .to_string(),
                        ),
                        attempt_status: Some(FlowStatus::Payment(
                            AttemptStatus::AuthenticationFailed,
                        )),
                        connector_transaction_id: response.server_trans_id.clone(),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                    ..item.router_data
                });
            }
        };

        // Highest common protocol version, matching what Netcetera reports. If the ACS range is
        // narrower than the DS range this is the ACS end, which is the correct choice.
        let message_version = response
            .acs_protocol_version_end
            .as_deref()
            .or(response.ds_protocol_version_end.as_deref())
            .and_then(|version| SemanticVersion::from_str(version).ok());

        let continue_redirection_url = require_continue_redirection_url(
            item.router_data.request.continue_redirection_url.as_ref(),
        )
        .change_context(ConnectorError::ResponseHandlingFailed {
            context: Default::default(),
        })?;
        let notification_url = method_notification_url(&continue_redirection_url);

        // `RedirectForm::Form` and not one of the richer variants: the domain->proto converter and
        // hyperswitch's proto->domain converter between them only round-trip `Form`, `Html`,
        // `Braintree`, `Mifinity` and `Nmi`. Everything else is rejected as "Invalid response type
        // received from connector".
        let redirection_data = match (
            response.method_url.as_ref(),
            response
                .method_data
                .as_ref()
                .and_then(|data| data.encoded_method_data.clone()),
        ) {
            // The ACS wants to profile the device: POST the encoded method data at it. It will
            // then redirect the browser to our Method Notification URL.
            (Some(method_url), Some(encoded_method_data)) => {
                let mut form_fields = HashMap::new();
                form_fields.insert(FIELD_THREE_DS_METHOD_DATA.to_string(), encoded_method_data);
                RedirectForm::Form {
                    endpoint: method_url.clone(),
                    method: common_utils::Method::Post,
                    form_fields,
                }
            }
            // No Method URL: normalise the no-DDC case into the *same shape* by self-POSTing
            // straight back to the Method Notification URL with a synthesised `threeDSMethodData`
            // plus the `threeDSMethodCompletion` marker. Both branches then land on the same URL
            // with the same field names, so `Authenticate` has exactly one code path instead of
            // two that can drift apart.
            _ => {
                let mut form_fields = HashMap::new();
                form_fields.insert(
                    FIELD_THREE_DS_METHOD_DATA.to_string(),
                    encode_synthetic_method_data(&server_trans_id, &notification_url),
                );
                form_fields.insert(
                    FIELD_THREE_DS_METHOD_COMPLETION.to_string(),
                    "UNAVAILABLE".to_string(),
                );
                RedirectForm::Form {
                    endpoint: notification_url,
                    method: common_utils::Method::Post,
                    form_fields,
                }
            }
        };

        let authentication_data = AuthenticationData {
            trans_status: None,
            eci: None,
            cavv: None,
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: Some(server_trans_id.clone()),
            message_version,
            ds_trans_id: None,
            acs_transaction_id: None,
            transaction_id: None,
            network_params: None,
            exemption_indicator: None,
            created_at: None,
            challenge_code: None,
            challenge_cancel: None,
            challenge_code_reason: None,
            message_extension: None,
            authentication_type: None,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: None,
                authentication_data: Some(authentication_data),
                redirection_data: Some(Box::new(redirection_data)),
                connector_response_reference_id: Some(server_trans_id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// CALL 3 — POST /3ds2/authentications  (Authenticate)
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2CardDetail {
    pub number: Secret<String>,
    pub scheme: String,
    /// `MM`.
    pub expiry_month: Secret<String>,
    /// `YY`.
    pub expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2Order {
    /// `yyyy-MM-ddTHH:mm:ss.SSSSSSZ` — unlike `request_timestamp`, this one carries a zone.
    pub date_time_created: String,
    /// Smallest unit of the currency, as a string.
    pub amount: String,
    pub currency: String,
    /// The **same** value the XML `<orderid>` carries, so the two systems reconcile in the GP
    /// portal.
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2Address {
    pub line1: Secret<String>,
    /// Documented mandatory but explicitly allowed to be blank.
    pub line2: Secret<String>,
    /// Documented mandatory but explicitly allowed to be blank.
    pub line3: Secret<String>,
    pub city: Secret<String>,
    pub postal_code: Secret<String>,
    /// ISO 3166-2 subdivision minus the country prefix; applicable to US/CA addresses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    /// ISO 3166-1 **numeric**, zero padded to three digits (US = `840`).
    pub country: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2Phone {
    pub country_code: Secret<String>,
    pub subscriber_number: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2Payer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    pub billing_address: Gp3ds2Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobile_phone: Option<Gp3ds2Phone>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2BrowserData {
    pub accept_header: String,
    pub color_depth: String,
    pub ip: Secret<String>,
    pub java_enabled: bool,
    pub javascript_enabled: bool,
    pub language: String,
    pub screen_height: String,
    pub screen_width: String,
    /// Hyperswitch renders the ACS challenge as a top-level auto-submitting page, not a sized
    /// iframe, so the only honest value is full screen.
    pub challenge_window_size: String,
    /// Minutes of offset between UTC and the browser's local time.
    pub timezone: String,
    pub user_agent: String,
}

/// *Initiate authentication* — the AReq.
#[derive(Debug, Clone, Serialize)]
pub struct Gp3ds2AuthenticationsRequest {
    /// **Must** be the same string the `securehash` header was computed over.
    pub request_timestamp: String,
    pub authentication_source: String,
    pub authentication_request_type: String,
    pub message_category: String,
    pub message_version: String,
    pub challenge_request_indicator: String,
    pub server_trans_id: String,
    pub merchant_id: Secret<String>,
    pub account_id: Secret<String>,
    pub card_detail: Gp3ds2CardDetail,
    pub order: Gp3ds2Order,
    pub payer: Gp3ds2Payer,
    pub challenge_notification_url: String,
    pub method_url_completion: Gp3ds2MethodUrlCompletion,
    pub browser_data: Gp3ds2BrowserData,
    pub merchant_contact_url: String,
}

/// Everything the `Authenticate` leg has to recover from the post-DDC browser POST.
pub struct Gp3ds2MethodReturn {
    pub server_trans_id: String,
    pub method_url_completion: Gp3ds2MethodUrlCompletion,
}

/// Read the DDC outcome out of `redirect_response.payload`.
///
/// The `server_trans_id` **cannot** travel via `authentication_data`: hyperswitch sources the
/// `Authenticate` leg's authentication data from its own authentication table, which a
/// connector-driven 3DS never writes. Recovering it from the ACS's own echo is both self-contained
/// and authoritative.
pub fn read_method_return(
    payload: Option<&serde_json::Value>,
) -> Result<Gp3ds2MethodReturn, error_stack::Report<IntegrationError>> {
    let payload = payload.ok_or_else(|| missing("redirect_response.payload"))?;

    // A `cres` in this payload means the ACS is returning from the **challenge**, not from device
    // profiling — this leg has been reached by mistake. The two returns are told apart by whether
    // the browser came back with a query string, which silently misclassifies every merchant whose
    // `continue_redirection_url` already carries one (the method marker is then not the only
    // parameter, and the bare challenge return is no longer bare). Refuse loudly instead of
    // building an AReq out of a challenge result.
    if payload_field(payload, FIELD_CRES).is_some() {
        return Err(report!(IntegrationError::InvalidDataFormat {
            field_name: "redirect_response.payload",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "redirect_response.payload carries a `cres`, so this is the post-challenge \
                     return and belongs to PostAuthenticate, not Authenticate. This misrouting \
                     happens when continue_redirection_url already contains a query string, which \
                     defeats the params/no-params discriminator the caller routes on."
                        .to_string(),
                ),
                ..Default::default()
            },
        }));
    }

    let encoded = payload_field(payload, FIELD_THREE_DS_METHOD_DATA)
        .ok_or_else(|| missing("redirect_response.payload.threeDSMethodData"))?;

    let server_trans_id = server_trans_id_from_encoded(&encoded).ok_or_else(|| {
        report!(IntegrationError::InvalidDataFormat {
            field_name: "redirect_response.payload.threeDSMethodData",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "threeDSMethodData did not base64-decode into a JSON object carrying \
                     threeDSServerTransID"
                        .to_string(),
                ),
                ..Default::default()
            },
        })
    })?;

    // Our own synthesised no-DDC form is the only thing that ever sets this field; a real ACS
    // echo carries only `threeDSMethodData`.
    let method_url_completion =
        if payload_field(payload, FIELD_THREE_DS_METHOD_COMPLETION).is_some() {
            Gp3ds2MethodUrlCompletion::Unavailable
        } else {
            Gp3ds2MethodUrlCompletion::Yes
        };

    Ok(Gp3ds2MethodReturn {
        server_trans_id,
        method_url_completion,
    })
}

fn build_billing_address(
    billing: Option<&domain_types::payment_address::Address>,
) -> Result<Gp3ds2Address, error_stack::Report<IntegrationError>> {
    let details = billing
        .and_then(|address| address.address.as_ref())
        .ok_or_else(|| missing("billing.address"))?;

    let country = details
        .country
        .ok_or_else(|| missing("billing.address.country"))?;

    Ok(Gp3ds2Address {
        line1: details
            .line1
            .clone()
            .ok_or_else(|| missing("billing.address.line1"))?,
        line2: details
            .line2
            .clone()
            .unwrap_or_else(|| Secret::new(String::new())),
        line3: details
            .line3
            .clone()
            .unwrap_or_else(|| Secret::new(String::new())),
        city: details
            .city
            .clone()
            .ok_or_else(|| missing("billing.address.city"))?,
        postal_code: details
            .zip
            .clone()
            .ok_or_else(|| missing("billing.address.zip"))?,
        state: details.state.clone(),
        country: format!("{:03}", CountryAlpha2::to_numeric(country)),
    })
}

fn build_phone(billing: Option<&domain_types::payment_address::Address>) -> Option<Gp3ds2Phone> {
    let phone = billing.and_then(|address| address.phone.as_ref())?;
    // GP wants digits only in both halves; a `+` prefix on the country code is rejected.
    let digits = |value: &str| -> String { value.chars().filter(char::is_ascii_digit).collect() };
    let country_code = digits(phone.country_code.as_deref()?);
    let number = digits(phone.number.as_ref()?.peek());
    (!country_code.is_empty() && !number.is_empty()).then(|| Gp3ds2Phone {
        country_code: Secret::new(country_code),
        subscriber_number: Secret::new(number),
    })
}

fn build_browser_data(
    browser_info: &BrowserInformation,
) -> Result<Gp3ds2BrowserData, error_stack::Report<IntegrationError>> {
    Ok(Gp3ds2BrowserData {
        accept_header: browser_info
            .accept_header
            .clone()
            .ok_or_else(|| missing("browser_info.accept_header"))?,
        color_depth: map_color_depth(browser_info.color_depth).to_owned(),
        ip: Secret::new(
            browser_info
                .ip_address
                .ok_or_else(|| missing("browser_info.ip_address"))?
                .to_string(),
        ),
        java_enabled: browser_info.java_enabled.unwrap_or(false),
        javascript_enabled: browser_info.java_script_enabled.unwrap_or(true),
        language: browser_info
            .language
            .clone()
            .ok_or_else(|| missing("browser_info.language"))?,
        screen_height: browser_info
            .screen_height
            .ok_or_else(|| missing("browser_info.screen_height"))?
            .to_string(),
        screen_width: browser_info
            .screen_width
            .ok_or_else(|| missing("browser_info.screen_width"))?
            .to_string(),
        challenge_window_size: "FULL_SCREEN".to_string(),
        timezone: browser_info
            .time_zone
            .ok_or_else(|| missing("browser_info.time_zone"))?
            .to_string(),
        user_agent: browser_info
            .user_agent
            .clone()
            .ok_or_else(|| missing("browser_info.user_agent"))?,
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
        &str,
    )> for Gp3ds2AuthenticationsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (router_data, request_timestamp): (
            &RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            &str,
        ),
    ) -> Result<Self, Self::Error> {
        let auth = GlobalpaymentsRealexAuthType::try_from(&router_data.connector_config)?;
        let card = require_card(&router_data.request.payment_method_data)?;
        let request = &router_data.request;

        let method_return = read_method_return(
            request
                .redirect_response
                .as_ref()
                .and_then(|redirect| redirect.payload.as_ref())
                .map(|payload| payload.peek()),
        )?;

        let continue_redirection_url =
            require_continue_redirection_url(request.continue_redirection_url.as_ref())?;

        let currency = request.currency.ok_or_else(|| missing("currency"))?;
        let browser_info = request
            .browser_info
            .as_ref()
            .ok_or_else(|| missing("browser_info"))?;

        let billing = router_data
            .resource_common_data
            .address
            .get_payment_method_billing();

        // Hyperswitch never writes the connector-driven authentication into its authentication
        // table, so `authentication_data.message_version` is normally absent on this leg; fall
        // back to the protocol version the `X-GP-VERSION` header declares.
        let message_version = request
            .authentication_data
            .as_ref()
            .and_then(|data| data.message_version.as_ref())
            .map(|version| version.to_string())
            .unwrap_or_else(|| GP_3DS2_VERSION.to_string());

        Ok(Self {
            request_timestamp: request_timestamp.to_owned(),
            authentication_source: "BROWSER".to_string(),
            authentication_request_type: "PAYMENT_TRANSACTION".to_string(),
            message_category: "PAYMENT_AUTHENTICATION".to_string(),
            message_version,
            // SCA exemptions are out of scope: this sandbox account rejects the XML
            // `<exempt_status>` with `508`, and requesting an exemption forfeits the liability
            // shift the merchant asked for by choosing three_ds.
            challenge_request_indicator: "NO_PREFERENCE".to_string(),
            server_trans_id: method_return.server_trans_id,
            merchant_id: auth.merchant_id,
            account_id: auth.account,
            card_detail: Gp3ds2CardDetail {
                number: Secret::new(card.card_number.peek().to_string()),
                scheme: map_3ds2_scheme(&card)?.to_owned(),
                expiry_month: card.get_card_expiry_month_2_digit()?,
                expiry_year: card.get_card_expiry_year_2_digit()?,
                full_name: card.card_holder_name.clone(),
            },
            order: Gp3ds2Order {
                date_time_created: format!("{request_timestamp}Z"),
                amount: format_amount(request.amount, currency)?,
                currency: currency.to_string(),
                id: sanitize_order_id(
                    &router_data
                        .resource_common_data
                        .connector_request_reference_id,
                )?,
            },
            payer: Gp3ds2Payer {
                email: request.email.clone(),
                billing_address: build_billing_address(billing)?,
                mobile_phone: build_phone(billing),
            },
            // Bare, with no query marker: the empty `params` on the return is what tells
            // hyperswitch to run PostAuthenticate rather than Authenticate again.
            challenge_notification_url: continue_redirection_url.as_str().to_owned(),
            method_url_completion: method_return.method_url_completion,
            browser_data: build_browser_data(browser_info)?,
            merchant_contact_url: merchant_contact_url(&continue_redirection_url),
        })
    }
}

// =============================================================================
// THE SHARED AUTHENTICATION RESPONSE (calls 3 and 5)
// =============================================================================

/// The response body of both `POST /3ds2/authentications` and
/// `GET /3ds2/authentications/{server_trans_id}`.
///
/// Every field is optional: the challenge shape, the frictionless shape and the results shape are
/// one document with different subsets populated.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gp3ds2AuthenticationResponse {
    pub server_trans_id: Option<String>,
    pub acs_trans_id: Option<String>,
    pub ds_trans_id: Option<String>,
    pub authentication_type: Option<String>,
    /// The CAVV / AAV — the proof of authentication the XML `<mpi>` block carries.
    pub authentication_value: Option<Secret<String>>,
    pub eci: Option<String>,
    pub status: Option<Gp3ds2Status>,
    pub status_reason: Option<String>,
    pub challenge_mandated: Option<Gp3ds2Bool>,
    /// The ACS URL to POST `encoded_creq` to. Only meaningful on `CHALLENGE_REQUIRED`.
    pub challenge_request_url: Option<String>,
    /// Base64url CReq. Only meaningful on `CHALLENGE_REQUIRED`.
    pub encoded_creq: Option<String>,
    pub message_version: Option<String>,
    pub message_category: Option<String>,
    pub authentication_source: Option<String>,
    /// Text the issuer wants shown to a shopper whose frictionless authentication failed.
    pub cardholder_response_info: Option<String>,
    pub acs_reference_number: Option<String>,
    pub challenge_interaction_counter: Option<Gp3ds2Scalar>,
    pub decoupled_response_indicator: Option<String>,
    pub whitelist_status: Option<String>,
}

impl Gp3ds2AuthenticationResponse {
    fn status(&self) -> Gp3ds2Status {
        // A response with no status at all cannot be treated as authenticated.
        self.status.unwrap_or(Gp3ds2Status::Unknown)
    }

    /// The `AuthenticationData` the XML `auth` leg's `build_mpi` consumes verbatim.
    fn authentication_data(&self) -> AuthenticationData {
        AuthenticationData {
            trans_status: Some(self.status().to_transaction_status()),
            eci: self.eci.clone(),
            cavv: self.authentication_value.clone(),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: self.server_trans_id.clone(),
            message_version: self
                .message_version
                .as_deref()
                .and_then(|version| SemanticVersion::from_str(version).ok()),
            ds_trans_id: self.ds_trans_id.clone(),
            acs_transaction_id: self.acs_trans_id.clone(),
            transaction_id: None,
            network_params: None,
            exemption_indicator: None,
            created_at: None,
            challenge_code: None,
            challenge_cancel: None,
            challenge_code_reason: None,
            message_extension: None,
            authentication_type: None,
        }
    }

    /// The declined-authentication error surfaced to the caller. Carries
    /// `attempt_status: AuthenticationFailed` so hyperswitch's `should_continue` guards suppress
    /// the XML `auth` that would otherwise follow.
    fn rejection_error(&self, http_code: u16) -> ErrorResponse {
        let status = self.status();
        ErrorResponse {
            status_code: http_code,
            code: ERROR_CODE_AUTHENTICATION_REJECTED.to_string(),
            message: format!(
                "GlobalpaymentsRealex 3DS2 authentication was not successful: {status:?}"
            ),
            reason: self
                .cardholder_response_info
                .clone()
                .or_else(|| self.status_reason.clone()),
            attempt_status: Some(FlowStatus::Payment(AttemptStatus::AuthenticationFailed)),
            connector_transaction_id: self.server_trans_id.clone(),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }
    }
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<Gp3ds2AuthenticationResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Gp3ds2AuthenticationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = response.status();

        if status.is_challenge() {
            // Branch on the status, never on field presence — see `Gp3ds2Status::is_challenge`.
            let (Some(acs_url), Some(creq)) = (
                response.challenge_request_url.clone(),
                response.encoded_creq.clone(),
            ) else {
                return Ok(Self {
                    response: Err(ErrorResponse {
                        status_code: item.http_code,
                        code: ERROR_CODE_AUTHENTICATION_REJECTED.to_string(),
                        message: "GlobalpaymentsRealex 3DS2 requested a challenge but returned no \
                                  challenge_request_url / encoded_creq"
                            .to_string(),
                        reason: response.status_reason.clone(),
                        attempt_status: Some(FlowStatus::Payment(
                            AttemptStatus::AuthenticationFailed,
                        )),
                        connector_transaction_id: response.server_trans_id.clone(),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                    ..item.router_data
                });
            };

            // The CReq must reach the ACS within 30 seconds of this response or the challenge
            // times out at the issuer.
            let mut form_fields = HashMap::new();
            form_fields.insert(FIELD_CREQ.to_string(), creq);

            return Ok(Self {
                response: Ok(PaymentsResponseData::AuthenticateResponse {
                    resource_id: None,
                    redirection_data: Some(Box::new(RedirectForm::Form {
                        endpoint: acs_url,
                        method: common_utils::Method::Post,
                        form_fields,
                    })),
                    authentication_data: Some(response.authentication_data()),
                    connector_feature_data: None,
                    connector_response_reference_id: response.server_trans_id.clone(),
                    status_code: item.http_code,
                }),
                ..item.router_data
            });
        }

        if !status.is_authorisable() {
            return Ok(Self {
                response: Err(response.rejection_error(item.http_code)),
                ..item.router_data
            });
        }

        // Frictionless success (or a liability-shifting "attempted"): no browser leg, no results
        // fetch — this response already carries the eci / cavv / ds_trans_id the XML auth needs.
        Ok(Self {
            response: Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: None,
                redirection_data: None,
                authentication_data: Some(response.authentication_data()),
                connector_feature_data: None,
                connector_response_reference_id: response.server_trans_id.clone(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// CALL 5 — GET /3ds2/authentications/{sid}  (PostAuthenticate)
// =============================================================================

/// Read the 3DS Server transaction id out of the `cres` the ACS posted after the challenge.
///
/// Like the method return, this is recovered from the browser POST rather than from
/// `authentication_data`: hyperswitch builds `PostAuthenticateRequest` with
/// `authentication_data: None`, so there is nothing else to read it from.
pub fn read_challenge_return(
    payload: &serde_json::Value,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let encoded = payload_field(payload, FIELD_CRES)
        .ok_or_else(|| missing("redirect_response.payload.cres"))?;

    let server_trans_id = server_trans_id_from_encoded(&encoded).ok_or_else(|| {
        report!(IntegrationError::InvalidDataFormat {
            field_name: "redirect_response.payload.cres",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "cres did not base64-decode into a JSON object carrying threeDSServerTransID"
                        .to_string(),
                ),
                ..Default::default()
            },
        })
    })?;

    validate_server_trans_id(&server_trans_id)?;
    Ok(server_trans_id)
}

/// Rejects anything that is not a syntactically valid 3DS Server transaction id.
///
/// This value arrives inside a base64 blob the **browser** posts back, and it is then interpolated
/// into the results URL — `{base}/3ds2/authentications/{sid}?merchant_id=…&request_timestamp=…` —
/// and hashed into the `securehash`. It cannot be percent-encoded on the way in, because the
/// gateway hashes and matches the unencoded value, so the only safe handling is to refuse anything
/// that is not the shape EMVCo defines: a UUID.
///
/// Without this, a `/`, `?`, `#` or `&` in the field rewrites the request — a `?` alone drops the
/// digest-covered `request_timestamp` into the previous component and turns a valid call into one
/// the gateway rejects, while `../` walks to a different endpoint entirely.
///
/// **Residual risk this does *not* close:** a well-formed id belonging to a *different* payment is
/// still accepted, because `PaymentsPostAuthenticateData` carries no server-side copy of the id to
/// compare against (no `authentication_data`, no metadata channel) and the results document echoes
/// no merchant reference either. Binding the id to the payment needs the caller to carry it from
/// the `Authenticate` leg into the `PostAuthenticate` request; that is a contract change, tracked
/// separately, not something this connector can enforce alone.
fn validate_server_trans_id(
    server_trans_id: &str,
) -> Result<(), error_stack::Report<IntegrationError>> {
    let is_uuid_shaped = server_trans_id.len() == 36
        && server_trans_id.as_bytes().iter().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                *b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        });

    if is_uuid_shaped {
        return Ok(());
    }

    Err(report!(IntegrationError::InvalidDataFormat {
        field_name: "redirect_response.payload.cres.threeDSServerTransID",
        context: IntegrationErrorContext {
            additional_context: Some(
                "threeDSServerTransID is not a well-formed UUID; it is interpolated into the \
                 results URL and hashed into the securehash, so a malformed value is refused \
                 rather than sent"
                    .to_string(),
            ),
            ..Default::default()
        },
    }))
}

/// The *Obtain authentication data* body.
///
/// Byte-for-byte the same document as [`Gp3ds2AuthenticationResponse`] — the results endpoint is
/// the same resource read back. It exists as a distinct newtype only because the connector macro
/// derives one `…Templating` marker type per registered `response_body`, so two flows cannot name
/// the same struct.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Gp3ds2PostAuthenticationResponse(pub Gp3ds2AuthenticationResponse);

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<Gp3ds2PostAuthenticationResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Gp3ds2PostAuthenticationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        if !response.status().is_authorisable() {
            // Covers the shopper cancelling at the ACS (CRes `transStatus: N` →
            // `AUTHENTICATION_FAILED`): the payment must not go on to the XML auth.
            return Ok(Self {
                response: Err(response.rejection_error(item.http_code)),
                ..item.router_data
            });
        }

        Ok(Self {
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data: Some(response.authentication_data()),
                connector_response_reference_id: response.server_trans_id.clone(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// ERRORS
// =============================================================================

/// The structured body a `400 Bad Request` carries.
///
/// Only 400 has one. A `401` (bad `securehash`) returns **no body at all**, which is why the
/// per-flow `get_error_response_v2` falls through to
/// `handle_json_response_deserialization_failure` instead of insisting on this shape.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gp3ds2ErrorResponse {
    pub three_dsserver_trans_id: Option<String>,
    pub error_code: Option<Gp3ds2Scalar>,
    /// `C` (SDK) | `S` (GP 3DS server) | `D` (directory server) | `A` (issuer ACS).
    pub error_component: Option<String>,
    pub error_description: Option<String>,
    /// Lists each individual offending field.
    pub error_detail: Option<String>,
    pub error_message_type: Option<String>,
    pub message_type: Option<String>,
    pub message_version: Option<String>,
}

impl Gp3ds2ErrorResponse {
    /// Whether the body actually looks like a 3DS2 error rather than an unrelated JSON document
    /// that happens to deserialize into an all-optional struct.
    pub fn is_populated(&self) -> bool {
        self.error_code.is_some()
            || self.error_description.is_some()
            || self.error_detail.is_some()
            || self.message_type.is_some()
    }

    pub fn into_error_response(self, status_code: u16, typed: Option<String>) -> ErrorResponse {
        ErrorResponse {
            status_code,
            code: self
                .error_code
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            message: self
                .error_description
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
            reason: self.error_detail.clone().or(self.error_description.clone()),
            // Any non-2xx means the authentication cannot proceed — a 502/503 from the 3DS2 API
            // is no more resumable than a 400 — so mark the attempt failed rather than leaving a
            // non-terminal status that would let the XML auth follow unauthenticated.
            attempt_status: (!(200..300).contains(&status_code))
                .then_some(FlowStatus::Payment(AttemptStatus::AuthenticationFailed)),
            connector_transaction_id: self.three_dsserver_trans_id.clone(),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use domain_types::payment_method_data::DefaultPCIHolder;

    use super::*;

    fn card(network: CardNetwork, number: &str) -> Card<DefaultPCIHolder> {
        Card {
            card_number: domain_types::payment_method_data::RawCardNumber::<DefaultPCIHolder>(
                cards::CardNumber::from_str(number).expect("valid test PAN"),
            ),
            card_exp_month: Secret::new("12".to_string()),
            card_exp_year: Secret::new("30".to_string()),
            card_cvc: Secret::new("123".to_string()),
            card_issuer: None,
            card_network: Some(network),
            card_type: None,
            card_issuing_country: None,
            bank_code: None,
            nick_name: None,
            card_holder_name: None,
            co_badged_card_data: None,
        }
    }

    /// The single most load-bearing invariant in this connector: one payment sends **both**
    /// spellings of Mastercard, `MASTERCARD` on the 3DS2 JSON leg and `MC` on the XML auth leg.
    /// If someone ever "simplifies" the two mappers into one, this test fails loudly instead of
    /// the connector failing quietly at the gateway.
    #[test]
    fn the_two_scheme_mappers_disagree_on_mastercard() {
        let mastercard = card(CardNetwork::Mastercard, "5571596304025153");

        let xml = map_card_type(&mastercard).expect("xml scheme");
        let json = map_3ds2_scheme(&mastercard).expect("3ds2 scheme");

        assert_eq!(xml, "MC", "the XML API's <type> for Mastercard is MC");
        assert_eq!(
            json, "MASTERCARD",
            "the 3DS2 JSON API's scheme for Mastercard is MASTERCARD"
        );
        assert_ne!(
            xml, json,
            "the XML and 3DS2 card-scheme mappers must stay separate: they disagree on Mastercard"
        );
    }

    #[test]
    fn the_two_scheme_mappers_agree_on_visa() {
        let visa = card(CardNetwork::Visa, "4222000009719489");
        assert_eq!(
            map_card_type(&visa).expect("xml scheme"),
            map_3ds2_scheme(&visa).expect("3ds2 scheme"),
        );
    }

    #[test]
    fn base64_decoding_accepts_every_alphabet_gp_emits() {
        // base64url, unpadded — how GP encodes `threeDSMethodData`.
        let url_safe =
            consts::BASE64_ENGINE_URL_SAFE_NO_PAD.encode(r#"{"threeDSServerTransID":"abc-123"}"#);
        // standard, padded — how an ACS typically encodes `cres`.
        let standard = consts::BASE64_ENGINE.encode(r#"{"threeDSServerTransID":"abc-123"}"#);

        assert_eq!(
            server_trans_id_from_encoded(&url_safe).as_deref(),
            Some("abc-123")
        );
        assert_eq!(
            server_trans_id_from_encoded(&standard).as_deref(),
            Some("abc-123")
        );
    }

    #[test]
    fn method_completion_is_unavailable_only_for_the_synthesised_no_ddc_form() {
        let encoded = encode_synthetic_method_data("sid-1", "https://example.com/complete");

        let ddc = serde_json::json!({ FIELD_THREE_DS_METHOD_DATA: encoded.clone() });
        let no_ddc = serde_json::json!({
            FIELD_THREE_DS_METHOD_DATA: encoded,
            FIELD_THREE_DS_METHOD_COMPLETION: "UNAVAILABLE",
        });

        assert_eq!(
            read_method_return(Some(&ddc))
                .expect("ddc")
                .method_url_completion,
            Gp3ds2MethodUrlCompletion::Yes
        );
        assert_eq!(
            read_method_return(Some(&no_ddc))
                .expect("no ddc")
                .method_url_completion,
            Gp3ds2MethodUrlCompletion::Unavailable
        );
    }

    #[test]
    fn a_query_lookup_fault_does_not_fail_the_payment() {
        // 508 is what a caller that dropped connector_feature_data gets: the order id fell back to
        // the request reference and the gateway has never heard of it. The payment is fine.
        for fault in ["502", "503", "505", "506", "508"] {
            assert_eq!(
                map_psync_attempt_status(fault, Some(1), false),
                AttemptStatus::Unresolved,
                "{fault} must not be reported as a failed payment"
            );
        }

        // A real decline still fails, and the happy paths are untouched.
        assert_eq!(
            map_psync_attempt_status("101", None, false),
            AttemptStatus::Failure
        );
        assert_eq!(
            map_psync_attempt_status(RESULT_SUCCESS, Some(42), false),
            AttemptStatus::Charged
        );
        assert_eq!(
            map_psync_attempt_status(RESULT_SUCCESS, Some(-1), false),
            AttemptStatus::Authorized
        );
        assert_eq!(
            map_psync_attempt_status(RESULT_SUCCESS, Some(42), true),
            AttemptStatus::Voided
        );
    }

    #[test]
    fn the_pasref_ladder_prefers_the_authorization_over_the_transaction_id() {
        let metadata = GlobalpaymentsRealexPaymentMetadata {
            orderid: "order-1".to_string(),
            authcode: Some("123456".to_string()),
            void_pasref: None,
            auth_pasref: Some("auth-pasref".to_string()),
        };

        // The whole point of the field: the settle-minted id must lose.
        assert_eq!(
            resolve_reference_pasref(Some(&metadata), Some("settle-pasref"), REQUEST_TYPE_REBATE)
                .expect("metadata wins"),
            "auth-pasref"
        );

        // Metadata written before `auth_pasref` existed, and auto-captured payments: fall back.
        let old_shape = GlobalpaymentsRealexPaymentMetadata {
            auth_pasref: None,
            ..metadata.clone()
        };
        assert_eq!(
            resolve_reference_pasref(Some(&old_shape), Some(" auth-pasref "), REQUEST_TYPE_VOID)
                .expect("fallback"),
            "auth-pasref"
        );
        assert_eq!(
            resolve_reference_pasref(None, Some("auth-pasref"), REQUEST_TYPE_VOID)
                .expect("no metadata at all"),
            "auth-pasref"
        );

        // Neither source yields anything: an actionable error, not a request that would 508.
        assert!(
            resolve_reference_pasref(Some(&old_shape), Some("   "), REQUEST_TYPE_REBATE).is_err()
        );
        assert!(resolve_reference_pasref(None, None, REQUEST_TYPE_REBATE).is_err());
    }

    #[test]
    fn a_tampered_rebate_document_is_not_reported_as_a_failed_refund() {
        // Guards the distinction the status map alone cannot express: `Failure` invites a retry
        // and a double refund, `ManualReview` does not.
        assert_eq!(map_refund_status(RESULT_SUCCESS), RefundStatus::Success);
        assert_eq!(map_refund_status("508"), RefundStatus::Failure);
        assert_ne!(RefundStatus::ManualReview, RefundStatus::Failure);
    }

    #[test]
    fn a_browser_supplied_server_trans_id_must_be_uuid_shaped() {
        // The happy path: exactly what an ACS returns.
        assert!(validate_server_trans_id("6d8b0a1e-6f5f-4d67-9d1e-2f0b6a1c9e33").is_ok());

        // Everything that would rewrite the results URL or its digest-covered query.
        for hostile in [
            "../../3ds2/protocol-versions",
            "6d8b0a1e-6f5f-4d67-9d1e-2f0b6a1c9e33?merchant_id=someone-else",
            "6d8b0a1e-6f5f-4d67-9d1e-2f0b6a1c9e33#frag",
            "6d8b0a1e-6f5f-4d67-9d1e-2f0b6a1c9e33&request_timestamp=0",
            "6d8b0a1e/6f5f/4d67/9d1e/2f0b6a1c9e33",
            "",
            "not-a-uuid",
            // Right length, wrong alphabet: `z` is not a hex digit.
            "zd8b0a1e-6f5f-4d67-9d1e-2f0b6a1c9e33",
        ] {
            assert!(
                validate_server_trans_id(hostile).is_err(),
                "expected {hostile:?} to be refused"
            );
        }
    }

    #[test]
    fn a_challenge_return_is_refused_by_the_device_profiling_leg() {
        // A `cres` payload reaching `Authenticate` means the params/no-params discriminator
        // misrouted the browser return — most often because the merchant's
        // continue_redirection_url already carries a query string.
        let challenge_return = serde_json::json!({ FIELD_CRES: "irrelevant" });
        assert!(read_method_return(Some(&challenge_return)).is_err());

        // A genuine device-profiling return still works.
        let ddc = serde_json::json!({
            FIELD_THREE_DS_METHOD_DATA:
                encode_synthetic_method_data("sid-1", "https://example.com/complete"),
        });
        assert!(read_method_return(Some(&ddc)).is_ok());
    }

    #[test]
    fn the_method_notification_url_carries_the_discriminating_query_marker() {
        let url = url::Url::parse("https://example.com/redirect/complete").expect("url");
        assert_eq!(
            method_notification_url(&url),
            "https://example.com/redirect/complete?gp3ds=method"
        );

        let with_query = url::Url::parse("https://example.com/redirect/complete?a=b").expect("url");
        assert_eq!(
            method_notification_url(&with_query),
            "https://example.com/redirect/complete?a=b&gp3ds=method"
        );
    }

    #[test]
    fn attempted_but_not_successful_is_a_success() {
        assert!(Gp3ds2Status::AuthenticationAttemptedButNotSuccessful.is_authorisable());
        assert!(Gp3ds2Status::AuthenticationSuccessful.is_authorisable());
        for rejected in [
            Gp3ds2Status::AuthenticationFailed,
            Gp3ds2Status::AuthenticationIssuerRejected,
            Gp3ds2Status::AuthenticationCouldNotBePerformed,
            Gp3ds2Status::Unknown,
        ] {
            assert!(
                !rejected.is_authorisable(),
                "{rejected:?} must not authorise"
            );
        }
    }

    #[test]
    fn an_unknown_status_deserialises_instead_of_failing_the_whole_response() {
        let response: Gp3ds2AuthenticationResponse =
            serde_json::from_str(r#"{"server_trans_id":"sid","status":"SOME_FUTURE_STATUS"}"#)
                .expect("tolerant deserialization");
        assert_eq!(response.status(), Gp3ds2Status::Unknown);
        assert!(!response.status().is_authorisable());
    }
}
