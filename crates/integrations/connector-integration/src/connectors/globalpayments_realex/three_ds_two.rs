//! Global Payments **3D Secure 2 JSON API** — the merchant-driven authentication legs.
//!
//! This module is deliberately separate from [`super::transformers`], which speaks the legacy
//! *RealEx* **XML** API. The two products share exactly one thing: the two-stage SHA-1 digest
//! primitive (`realex_digest`) and the Shared Secret it is keyed on. Everything else differs:
//!
//! | | XML API (`transformers.rs`) | 3DS2 JSON API (this file) |
//! |---|---|---|
//! | Host | `api[.sandbox].realexpayments.com` (`base_url`) | `api[.sandbox].globalpay-ecommerce.com` (`secondary_base_url`) |
//! | Encoding | XML | JSON |
//! | Auth | `<sha1hash>` **element in the body** | `Authorization: securehash …` **header** |
//! | Timestamp | `YYYYMMDDHHMMSS` | `yyyy-MM-ddTHH:mm:ss.SSSSSS` |
//! | Mastercard | `MC` | `MASTERCARD` |
//! | Failure | always HTTP 200, `<result>` carries the outcome | real HTTP 4xx/5xx |
//!
//! **The `MC` / `MASTERCARD` divergence is the reason these two type sets do not live in one
//! file.** Keeping them apart makes an accidental "let's unify the card-type mapper" refactor
//! visibly wrong rather than subtly wrong. [`tests::the_two_scheme_mappers_disagree_on_mastercard`]
//! guards it.
//!
//! Flow mapping (vendor doc `sources/source_2_3d_secure_two.md`):
//!
//! | # | Call | UCS flow | Side |
//! |---|---|---|---|
//! | 1 | `POST /3ds2/protocol-versions` | `PreAuthenticate` | server |
//! | 2 | `POST {method_url}` (`threeDSMethodData`) | *(redirect emitted by 1)* | browser |
//! | 3 | `POST /3ds2/authentications` | `Authenticate` | server |
//! | 4 | `POST {challenge_request_url}` (`creq`) | *(redirect emitted by 3)* | browser |
//! | 5 | `GET /3ds2/authentications/{sid}` | `PostAuthenticate` | server |
//! | 6 | XML `auth` with `<mpi>` | `Authorize` (already built) | server |
//!
//! A frictionless authentication skips 4 and 5 — call 3 already returns `eci`,
//! `authentication_value` and `ds_trans_id`.

use std::{collections::HashMap, fmt::Debug, str::FromStr};

use base64::Engine;
use common_enums::{AttemptStatus, CardNetwork, CountryAlpha2, TransactionStatus};
use common_utils::{consts, types::SemanticVersion};
use domain_types::{
    connector_flow::{Authenticate, PreAuthenticate},
    connector_types::{
        PaymentFlowData, PaymentsAuthenticateData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation},
    router_response_types::RedirectForm,
    utils::{get_card_issuer, CardIssuer},
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::transformers::{format_amount, sanitize_order_id, GlobalpaymentsRealexAuthType};
use crate::types::ResponseRouterData;

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

    server_trans_id_from_encoded(&encoded).ok_or_else(|| {
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
    })
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
            // 4xx means the authentication cannot proceed, so mark the attempt failed and let
            // hyperswitch's `should_continue` guards suppress the XML auth that would follow.
            attempt_status: (400..500)
                .contains(&status_code)
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

        let xml = super::super::transformers::map_card_type(&mastercard).expect("xml scheme");
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
            super::super::transformers::map_card_type(&visa).expect("xml scheme"),
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
