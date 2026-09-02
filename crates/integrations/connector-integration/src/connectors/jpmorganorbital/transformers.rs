use common_enums::{AttemptStatus, CaptureMethod, CardNetwork, Currency};
use common_utils::{
    pii::SecretSerdeValue,
    types::{AmountConvertor, StringImpliedDecimal, StringImpliedDecimalForConnector},
};
use domain_types::{
    connector_flow::{Authorize, PSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    utils::{get_card_issuer, CardIssuer},
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;

use crate::connectors::jpmorganorbital::JpmorganOrbitalRouterData;
use crate::types::ResponseRouterData;

/// Feature release version. Required to receive v4 / Feature Version 5.2 response
/// elements; hard-coded because the connector only speaks that dialect.
pub const ORBITAL_VERSION: &str = "5.2";
/// `order.industryType`. Only e-commerce is in scope.
const INDUSTRY_TYPE_ECOMMERCE: &str = "EC";
/// `transType` for authorization + capture (UCS `CaptureMethod::Automatic`).
const TRANS_TYPE_AUTH_CAPTURE: &str = "AC";
/// `cardholderVerification.ccCardVerifyPresenceInd` — "value is present".
const CARD_VERIFY_PRESENCE_PRESENT: &str = "1";
/// `order.status.procStatus` value meaning "passed all Gateway edit checks".
const PROC_STATUS_SUCCESS: &str = "0";
/// `order.status.procStatus` value meaning "timed out waiting for the transaction to
/// complete". The authorization may still have succeeded at the issuer, so this is the one
/// failure code that must NOT be recorded as terminal.
const PROC_STATUS_TIMED_OUT: &str = "9710";
/// HTTP 408. Orbital returns it when the gateway did not complete in time.
const HTTP_REQUEST_TIMEOUT: u16 = 408;
/// `additionalAuthInfo.authenticationECIInd` meaning "authentication attempted but not
/// completed". Orbital expects the indicator on its own; there is no cryptogram to send.
const ECI_NOT_AUTHENTICATED: &str = "7";
/// `order.status.approvalStatus` value meaning "approved by the issuer".
const APPROVAL_STATUS_APPROVED: &str = "1";
/// `merchant.bin` selecting the Stratus host. Only Stratus supports
/// `mcProgramProtocol` / `mcDirectoryTransID`.
const BIN_STRATUS: &str = "000001";
/// `additionalAuthInfo.pymtBrandProgramCode` for Discover ProtectBuy.
const BRAND_PROGRAM_DISCOVER_PROTECTBUY: &str = "DPB";
/// `additionalAuthInfo.pymtBrandProgramCode` for American Express SafeKey.
const BRAND_PROGRAM_AMEX_SAFEKEY: &str = "ASK";

/// Max length of `order.orderID`.
const MAX_ORDER_ID_LEN: usize = 22;
/// Orbital requires `order.orderID` to be unique across transactions within its first 8
/// characters, not merely as a whole.
const ORDER_ID_UNIQUE_PREFIX_LEN: usize = 8;
/// Max length of `order.amount`.
const MAX_AMOUNT_LEN: usize = 12;

// =============================================================================
// AUTH
// =============================================================================

#[derive(Debug, Clone)]
pub struct JpmorganOrbitalAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub merchant_id: Secret<String>,
    pub bin: String,
    pub terminal_id: String,
    /// ISO-4217 alphabetic code the MID is provisioned for. `None` disables the
    /// currency check entirely, which is the behaviour for every config that does
    /// not set it.
    pub merchant_config_currency: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for JpmorganOrbitalAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::JpmorganOrbital {
                username,
                password,
                merchant_id,
                bin,
                terminal_id,
                merchant_config_currency,
                ..
            } => {
                let bin = bin
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::InvalidConnectorConfig {
                        config: "jpmorganorbital.bin",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Set `bin` on the JpmorganOrbital connector config: \"000001\" \
                                 for the Stratus host (US) or \"000002\" for Tandem (Canada)."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })
                    })?;
                let terminal_id = terminal_id
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::InvalidConnectorConfig {
                            config: "jpmorganorbital.terminal_id",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Set `terminal_id` on the JpmorganOrbital connector config \
                                     (3 digits; \"001\" on Stratus, \"001\"-\"999\" on Tandem)."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })
                    })?;

                Ok(Self {
                    username: username.clone(),
                    password: password.clone(),
                    merchant_id: merchant_id.clone(),
                    bin: bin.trim().to_string(),
                    terminal_id: terminal_id.trim().to_string(),
                    merchant_config_currency: merchant_config_currency
                        .clone()
                        .filter(|value| !value.trim().is_empty()),
                })
            }
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the JP Morgan Orbital credentials as \
                             x-connector-config: {\"config\":{\"JpmorganOrbital\":{\"username\":…,\
                             \"password\":…,\"merchant_id\":…,\"bin\":…,\"terminal_id\":…}}}."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }
            )),
        }
    }
}

// =============================================================================
// ORDER ID + RETRY TRACE
// =============================================================================

/// The `order.orderID` alphabet used when the reference has to be derived. Orbital also
/// allows `- , $ @ &` and space, but restricting the digest to `[0-9a-z]` keeps the value
/// URL-safe, unambiguous in logs, and free of any leading-space risk.
const ORDER_ID_DIGEST_ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// Orbital requires the **first 8 characters** of `orderID` to be unique per transaction,
/// so a reference may only travel verbatim when it is short enough that its first 8
/// characters are the whole thing. A longer reference is derived instead: `ORDER0000001`
/// and `ORDER0000002` are both charset-legal and both start `ORDER000`, which would
/// collide.
fn is_order_id_verbatim_safe(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= ORDER_ID_UNIQUE_PREFIX_LEN
        && !reference.starts_with(' ')
        && reference
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ',' | '$' | '@' | '&' | ' '))
}

/// Render a digest in base-36 over [`ORDER_ID_DIGEST_ALPHABET`], most significant digit
/// first, to exactly `len` characters.
///
/// Base-36 rather than hex because only the leading 8 characters carry the uniqueness
/// requirement: hex gives those 8 characters 32 bits, base-36 gives them ~41.
fn encode_base36(bytes: &[u8], len: usize) -> String {
    let mut digits = vec![0_u8; len];
    for byte in bytes {
        let mut carry = usize::from(*byte);
        for digit in digits.iter_mut().rev() {
            let value = usize::from(*digit) * 256 + carry;
            *digit = u8::try_from(value % 36).unwrap_or(0);
            carry = value / 36;
        }
    }
    digits
        .into_iter()
        .map(|d| {
            ORDER_ID_DIGEST_ALPHABET
                .get(usize::from(d))
                .map_or('0', |b| char::from(*b))
        })
        .collect()
}

/// Orbital's `order.orderID` is at most 22 characters of `a-z A-Z 0-9 - , $ @ &` plus
/// space, may not start with a space, and its **first 8 characters must be unique per
/// transaction**. A UCS `connector_request_reference_id` is routinely longer than that and
/// carries `_`, which is outside the set.
///
/// Sanitising and truncating cannot satisfy the uniqueness rule, so this mirrors
/// `ilixium::derive_merchant_ref`: send the reference verbatim when it is short enough to
/// be unique on its own, otherwise derive a base-36 digest of it.
///
/// The derivation is deterministic, which is what lets PSync rebuild the same `orderID`
/// from the same reference without persisting a mapping. It is deliberately **not**
/// idempotent — a 22-character digest is longer than the verbatim limit, so re-deriving
/// from an already-derived value would hash it again. Nothing does that: both Authorize
/// and PSync derive from `connector_request_reference_id`, never from a previous output.
fn build_order_id(reference: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    if is_order_id_verbatim_safe(reference) {
        return Ok(reference.to_string());
    }

    if reference.trim().is_empty() {
        return Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "connector_request_reference_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "JP Morgan Orbital requires a non-empty order.orderID".to_string(),
                    ),
                    ..Default::default()
                },
            }
        ));
    }

    Ok(encode_base36(
        &Sha256::digest(reference.as_bytes()),
        MAX_ORDER_ID_LEN,
    ))
}

/// Reject a payment whose currency does not match what the MID is provisioned for.
///
/// Orbital derives the currency from the Merchant ID setup and has no request field to
/// receive it, so a EUR request routed to a USD MID would be authorized as USD at the
/// same numeric amount. When `merchant_config_currency` is not configured this is a
/// no-op, preserving the behaviour of every existing config.
fn validate_currency(
    request_currency: Currency,
    merchant_config_currency: Option<&str>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    // A blank string is treated as "not configured", so a config that sets the key to
    // "" behaves like one that omits it rather than failing every payment.
    let Some(configured) = merchant_config_currency
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let configured_currency =
        Currency::from_str(configured.to_uppercase().as_str()).map_err(|_| {
            error_stack::report!(IntegrationError::InvalidConnectorConfig {
                config: "jpmorganorbital.merchant_config_currency",
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "{configured:?} is not an ISO-4217 alphabetic currency code"
                    )),
                    ..Default::default()
                },
            })
        })?;
    if request_currency != configured_currency {
        return Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!(
                "Currency {request_currency} (this MID is provisioned for \
                 {configured_currency})"
            ),
            connector: "jpmorganorbital",
            context: IntegrationErrorContext::default(),
        }));
    }
    Ok(())
}

/// Derive `order.retryTrace` from the attempt reference.
///
/// `retryTrace` is both the idempotency key and the only `/inquiry` lookup key, so a
/// collision inside Orbital's 48-hour window makes the gateway echo a different
/// payment's approval back as this payment's result. SHA-256 is used rather than a
/// short non-cryptographic hash because the modulo fold below keeps only the low bits,
/// which is exactly where a hash like FNV-1a is weakest.
///
/// The derivation must stay deterministic: PSync recomputes it from the same reference.
pub fn build_retry_trace(reference: &str) -> String {
    let digest = Sha256::digest(reference.as_bytes());
    // The digest is 32 bytes, so the first 8 always exist; fall back rather than panic.
    let leading = digest
        .get(..8)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map_or(0_u64, u64::from_be_bytes);
    // Fold into [1e15, 1e16) so the value is numeric and exactly 16 characters.
    let trace = 1_000_000_000_000_000_u64 + (leading % 9_000_000_000_000_000_u64);
    trace.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrbitalBrand {
    Visa,
    Mastercard,
    Discover,
    Amex,
    Other,
}

fn resolve_brand<T: PaymentMethodDataTypes>(card: &Card<T>) -> OrbitalBrand {
    if let Some(network) = card.card_network.as_ref() {
        return match network {
            CardNetwork::Visa => OrbitalBrand::Visa,
            // International Maestro shares Mastercard's AAV field and ECI scale.
            CardNetwork::Mastercard | CardNetwork::Maestro => OrbitalBrand::Mastercard,
            CardNetwork::Discover => OrbitalBrand::Discover,
            CardNetwork::AmericanExpress => OrbitalBrand::Amex,
            _ => OrbitalBrand::Other,
        };
    }

    match get_card_issuer(card.card_number.peek()) {
        Ok(CardIssuer::Visa) => OrbitalBrand::Visa,
        Ok(CardIssuer::Master) | Ok(CardIssuer::Maestro) => OrbitalBrand::Mastercard,
        Ok(CardIssuer::Discover) => OrbitalBrand::Discover,
        Ok(CardIssuer::AmericanExpress) => OrbitalBrand::Amex,
        _ => OrbitalBrand::Other,
    }
}

// =============================================================================
// REQUEST — POST /payments
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalMerchant {
    /// `000001` = Stratus (US), `000002` = Tandem (Canada).
    pub bin: String,
    /// Documented mandatory in the Payments Request guide but absent from every
    /// OpenAPI example; the header is authoritative. Sent here too because it cannot
    /// conflict (there is only one credential) and it satisfies the stricter reading.
    #[serde(rename = "merchantID")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "terminalID")]
    pub terminal_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalCard<T: PaymentMethodDataTypes> {
    /// The PAN.
    #[serde(rename = "ccAccountNum")]
    pub cc_account_num: RawCardNumber<T>,
    /// `YYYYMM` — four-digit year first.
    #[serde(rename = "ccExp")]
    pub cc_exp: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalPaymentInstrument<T: PaymentMethodDataTypes> {
    pub card: JpmorganOrbitalCard<T>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalOrder {
    #[serde(rename = "orderID")]
    pub order_id: String,
    /// Two implied decimals for every currency. See [`JpmorganOrbitalAmount`].
    pub amount: StringImpliedDecimal,
    #[serde(rename = "industryType")]
    pub industry_type: String,
    /// Idempotency key **and** the `/inquiry` lookup key. Always sent.
    #[serde(rename = "retryTrace")]
    pub retry_trace: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalCardholderVerification {
    #[serde(rename = "ccCardVerifyNum")]
    pub cc_card_verify_num: Secret<String>,
    /// `1` = the value is present. Mandatory alongside the CVV for Visa, Discover,
    /// JCB and UnionPay, optional for Mastercard — always sending `1` when a CVV is
    /// present is correct for every brand.
    #[serde(rename = "ccCardVerifyPresenceInd")]
    pub cc_card_verify_presence_ind: String,
}

/// 3DS only. Emitted solely when the external authentication produced a usable
/// CAVV/ECI pair; never emitted empty.
#[derive(Debug, Clone, Default, Serialize)]
pub struct JpmorganOrbitalCryptogram {
    /// Visa (and ChaseNet) CAVV. Already Base64 — passed through verbatim.
    #[serde(rename = "verifyByVisaCAVV", skip_serializing_if = "Option::is_none")]
    pub verify_by_visa_cavv: Option<Secret<String>>,
    /// Visa XID. Base64-only; sent only when the upstream value already is Base64.
    #[serde(rename = "verifyByVisaXID", skip_serializing_if = "Option::is_none")]
    pub verify_by_visa_xid: Option<Secret<String>>,
    /// Mastercard / International Maestro AAV.
    #[serde(rename = "mcSecureCodeAAV", skip_serializing_if = "Option::is_none")]
    pub mc_secure_code_aav: Option<Secret<String>>,
    /// Discover ProtectBuy and Amex SafeKey both carry the CAVV here.
    #[serde(
        rename = "digitalTokenCryptogram",
        skip_serializing_if = "Option::is_none"
    )]
    pub digital_token_cryptogram: Option<Secret<String>>,
}

/// 3DS only. Companion of [`JpmorganOrbitalCryptogram`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct JpmorganOrbitalAdditionalAuthInfo {
    /// `5` authenticated, `6` attempted, `7` not authenticated. Note this is *not*
    /// the raw UCS ECI — see [`map_eci`].
    #[serde(
        rename = "authenticationECIInd",
        skip_serializing_if = "Option::is_none"
    )]
    pub authentication_eci_ind: Option<String>,
    /// `DPB` (Discover ProtectBuy) or `ASK` (Amex SafeKey); omitted for Visa/MC.
    #[serde(
        rename = "pymtBrandProgramCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub pymt_brand_program_code: Option<String>,
    /// Mastercard UCAF Collection Indicator. Passed through only when UCS supplies
    /// one; never derived, because Merchant Services derives every value but `4`.
    #[serde(rename = "ucafInd", skip_serializing_if = "Option::is_none")]
    pub ucaf_ind: Option<String>,
    /// `1` = 3DS 1.0, `2` = 3DS 2.x. Mastercard, Stratus (BIN `000001`) only.
    #[serde(rename = "mcProgramProtocol", skip_serializing_if = "Option::is_none")]
    pub mc_program_protocol: Option<String>,
    /// Mastercard Directory Server Transaction ID. Stratus only.
    #[serde(rename = "mcDirectoryTransID", skip_serializing_if = "Option::is_none")]
    pub mc_directory_trans_id: Option<String>,
}

/// `POST /payments` body. Identical for non-3DS and 3DS apart from the two trailing
/// objects — Orbital never redirects, so there is no return/callback URL to carry.
#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalPaymentsRequest<T: PaymentMethodDataTypes> {
    pub version: String,
    #[serde(rename = "transType")]
    pub trans_type: String,
    pub merchant: JpmorganOrbitalMerchant,
    #[serde(rename = "paymentInstrument")]
    pub payment_instrument: JpmorganOrbitalPaymentInstrument<T>,
    pub order: JpmorganOrbitalOrder,
    #[serde(
        rename = "cardholderVerification",
        skip_serializing_if = "Option::is_none"
    )]
    pub cardholder_verification: Option<JpmorganOrbitalCardholderVerification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<JpmorganOrbitalCryptogram>,
    #[serde(rename = "additionalAuthInfo", skip_serializing_if = "Option::is_none")]
    pub additional_auth_info: Option<JpmorganOrbitalAdditionalAuthInfo>,
}

/// Map UCS `capture_method` onto Orbital's `transType`.
fn trans_type_for(
    capture_method: Option<CaptureMethod>,
) -> Result<&'static str, error_stack::Report<IntegrationError>> {
    match capture_method {
        // `SequentialAutomatic` is an auto-capture as far as this connector is
        // concerned; the framework groups it with `Automatic` in
        // `PaymentsSyncData::is_auto_capture`.
        None | Some(CaptureMethod::Automatic) | Some(CaptureMethod::SequentialAutomatic) => {
            Ok(TRANS_TYPE_AUTH_CAPTURE)
        }
        // Manual capture would send `transType: "A"`, holding funds at the issuer with
        // no way to settle or release them: Capture, Void and Refund are all
        // `not_implemented` for this connector. Refuse it rather than stranding an
        // authorization the caller cannot act on.
        Some(other) => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("Capture method {other:?}"),
            connector: "jpmorganorbital",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "JP Morgan Orbital is integrated for one-time auto-capture card payments \
                     only. Use automatic capture."
                        .to_string(),
                ),
                ..Default::default()
            },
        })),
    }
}

fn looks_base64(value: &str) -> bool {
    !value.is_empty()
        && value.len().is_multiple_of(4)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

fn map_eci(eci: &str, brand: OrbitalBrand) -> Option<&'static str> {
    let trimmed = eci.trim();
    let unpadded = trimmed.trim_start_matches('0');
    let normalized = if unpadded.is_empty() { "0" } else { unpadded };

    match brand {
        // Mastercard's own ECI enum is 00/01/02 on an inverted scale, so those are
        // translated. 05/06/07 are not Mastercard values at all: a 3DS server emitting
        // them on a Mastercard has normalised onto the Visa scale, where the meaning is
        // unambiguous and lands on the same `authenticationECIInd`. Accepting both keeps
        // such a payment 3DS instead of rejecting an authentication that did happen.
        OrbitalBrand::Mastercard => match normalized {
            "2" | "5" => Some("5"), // full authentication
            "1" | "6" => Some("6"), // attempted
            "0" | "7" => Some("7"), // not authenticated
            _ => None,
        },
        OrbitalBrand::Visa | OrbitalBrand::Discover | OrbitalBrand::Amex => match normalized {
            "5" => Some("5"),
            "6" => Some("6"),
            "7" => Some("7"),
            _ => None,
        },
        // No documented ECI mapping for other brands; treat as non-3DS rather than
        // guessing a value the gateway would interpret as something else.
        OrbitalBrand::Other => None,
    }
}

/// Orbital carries the CAVV in a brand-specific cryptogram field, so a brand this
/// connector cannot resolve has nowhere to put it.
fn unsupported_three_ds_brand() -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: "3DS authentication data for an unrecognised card brand".to_string(),
        connector: "jpmorganorbital",
        context: IntegrationErrorContext {
            additional_context: Some(
                "Orbital carries the CAVV in a brand-specific cryptogram field; the card \
                 network could not be resolved to Visa, Mastercard, Discover or Amex"
                    .to_string(),
            ),
            ..Default::default()
        },
    })
}

/// Build the two 3DS objects from externally produced authentication data.
///
/// Called only when `authentication_data` is `Some`, i.e. 3DS was attempted. If the
/// data cannot be encoded for Orbital this returns an error rather than falling back
/// to a plain non-3DS authorization: a silent downgrade costs the merchant the
/// liability shift it already paid for. Orbital's own rule is that the CAVV and the
/// ECI must always travel together, so a half-populated pair is never emitted.
fn build_three_ds_objects<T: PaymentMethodDataTypes>(
    card: &Card<T>,
    authentication_data: &AuthenticationData,
    bin: &str,
) -> Result<
    (
        Option<JpmorganOrbitalCryptogram>,
        JpmorganOrbitalAdditionalAuthInfo,
    ),
    error_stack::Report<IntegrationError>,
> {
    let brand = resolve_brand(card);
    // Resolved before the ECI so an unsupported brand reports itself rather than
    // surfacing as an unmappable-ECI error (`map_eci` also rejects `Other`).
    if brand == OrbitalBrand::Other {
        return Err(unsupported_three_ds_brand());
    }
    let raw_eci = authentication_data.eci.as_deref().ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.eci",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "JP Morgan Orbital requires an ECI to encode a 3DS authorization".to_string(),
                ),
                ..Default::default()
            },
        })
    })?;
    let eci = map_eci(raw_eci, brand).ok_or_else(|| {
        error_stack::report!(IntegrationError::NotSupported {
            message: format!(
                "3DS ECI {raw_eci:?} for card brand {brand:?} (no mapping onto \
                 Orbital's authenticationECIInd scale)"
            ),
            connector: "jpmorganorbital",
            context: IntegrationErrorContext::default(),
        })
    })?;

    // A "not authenticated" outcome carries no CAVV by definition, so the ECI travels
    // alone. Demanding a CAVV here would reject a payment Orbital accepts. For a full
    // (5) or attempted (6) authentication the CAVV is mandatory, and its absence is a
    // genuine error rather than a reason to silently downgrade to non-3DS.
    let cavv = match authentication_data.cavv.as_ref() {
        Some(cavv) => Some(cavv),
        None if eci == ECI_NOT_AUTHENTICATED => None,
        None => {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "authentication_data.cavv",
                    context: IntegrationErrorContext {
                        additional_context: Some(format!(
                            "JP Morgan Orbital requires a CAVV to encode a 3DS authorization at \
                         authenticationECIInd {eci}; downgrading to non-3DS would silently \
                         forfeit the liability shift"
                        )),
                        ..Default::default()
                    },
                }
            ))
        }
    };

    let mut cryptogram = JpmorganOrbitalCryptogram::default();
    let mut additional = JpmorganOrbitalAdditionalAuthInfo {
        authentication_eci_ind: Some(eci.to_string()),
        ..Default::default()
    };

    match brand {
        OrbitalBrand::Visa => {
            // Pass the CAVV through verbatim — it is already Base64 as produced by
            // the 3DS server. Re-encoding it is the single most likely bug here and
            // surfaces as a respCode 37/245 decline, not as an error.
            cryptogram.verify_by_visa_cavv = cavv.cloned();
            // `verifyByVisaXID` is Conditional, not Mandatory, and a non-Base64 value
            // is a guaranteed respCode 245 decline. Under 3DS 2.x there is no XID at
            // all and the 3DS server transaction id is a UUID, so omitting it here is
            // the documented behaviour and does not cost the liability shift — the
            // CAVV and ECI still travel. Unlike a missing CAVV/ECI this is not a
            // silent downgrade, so it stays a filter rather than an error.
            cryptogram.verify_by_visa_xid = authentication_data
                .threeds_server_transaction_id
                .as_deref()
                .filter(|xid| looks_base64(xid))
                .map(|xid| Secret::new(xid.to_string()));
        }
        OrbitalBrand::Mastercard => {
            cryptogram.mc_secure_code_aav = cavv.cloned();
            // UCS carries a first-class UCAF collection indicator; pass it through
            // when present, never derive one.
            additional
                .ucaf_ind
                .clone_from(&authentication_data.ucaf_collection_indicator);
            if bin == BIN_STRATUS {
                additional.mc_program_protocol = authentication_data
                    .message_version
                    .as_ref()
                    .and_then(|version| version.to_string().chars().next())
                    .filter(|major| matches!(major, '1' | '2'))
                    .map(|major| major.to_string());
                additional
                    .mc_directory_trans_id
                    .clone_from(&authentication_data.ds_trans_id);
            }
        }
        OrbitalBrand::Discover => {
            cryptogram.digital_token_cryptogram = cavv.cloned();
            additional.pymt_brand_program_code =
                Some(BRAND_PROGRAM_DISCOVER_PROTECTBUY.to_string());
        }
        OrbitalBrand::Amex => {
            cryptogram.digital_token_cryptogram = cavv.cloned();
            additional.pymt_brand_program_code = Some(BRAND_PROGRAM_AMEX_SAFEKEY.to_string());
        }
        OrbitalBrand::Other => return Err(unsupported_three_ds_brand()),
    }

    // With no CAVV there is nothing for the cryptogram object to carry, and Orbital
    // must not receive an empty one.
    Ok((cavv.is_some().then_some(cryptogram), additional))
}

type AuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<JpmorganOrbitalRouterData<AuthorizeRouterData<T>, T>>
    for JpmorganOrbitalPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: JpmorganOrbitalRouterData<AuthorizeRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "This payment method".to_string(),
                    connector: "jpmorganorbital",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "JP Morgan Orbital is integrated for card payments only.".to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        };

        // Mandates / MIT / stored credentials are explicitly out of scope: the
        // gateway models them with `merchantInitiatedTransaction` / `profile` fields
        // this connector does not send, so accepting the request would quietly drop
        // the credential-on-file signalling the networks require.
        if request.mandate_id.is_some() || request.setup_mandate_details.is_some() {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Mandates / merchant-initiated transactions".to_string(),
                connector: "jpmorganorbital",
                context: IntegrationErrorContext::default(),
            }));
        }

        let auth = JpmorganOrbitalAuthType::try_from(&router_data.connector_config)?;
        validate_currency(request.currency, auth.merchant_config_currency.as_deref())?;
        let common = &router_data.resource_common_data;

        // `ccExp` is a fixed 6-char YYYYMM field. `get_expiry_date_as_yyyymm` interpolates
        // the raw month, so a single-digit month would yield 5 chars ("20257"). Build it
        // from the zero-padding helpers instead.
        let cc_exp = Secret::new(format!(
            "{}{}",
            card.get_expiry_year_4_digit().peek(),
            card.get_card_expiry_month_2_digit()?.peek()
        ));

        let cvc = card.card_cvc.peek().trim().to_string();
        let cardholder_verification =
            (!cvc.is_empty()).then(|| JpmorganOrbitalCardholderVerification {
                cc_card_verify_num: Secret::new(cvc),
                cc_card_verify_presence_ind: CARD_VERIFY_PRESENCE_PRESENT.to_string(),
            });

        // Orbital 3DS is a passthrough: the challenge (if any) already happened in the
        // merchant's own MPI / the UCS external-authentication flow. There is no
        // second call and no redirect to build here.
        // No authentication data at all is a plain non-3DS payment and stays valid.
        // Authentication data that cannot be encoded is an error, never a downgrade.
        let (cryptogram, additional_auth_info) = match request.authentication_data.as_ref() {
            Some(authentication_data) => {
                let (cryptogram, additional) =
                    build_three_ds_objects(card, authentication_data, &auth.bin)?;
                (cryptogram, Some(additional))
            }
            None => (None, None),
        };

        Ok(Self {
            version: ORBITAL_VERSION.to_string(),
            trans_type: trans_type_for(request.capture_method)?.to_string(),
            merchant: JpmorganOrbitalMerchant {
                bin: auth.bin.clone(),
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            payment_instrument: JpmorganOrbitalPaymentInstrument {
                card: JpmorganOrbitalCard {
                    cc_account_num: card.card_number.clone(),
                    cc_exp,
                },
            },
            order: JpmorganOrbitalOrder {
                order_id: build_order_id(&common.connector_request_reference_id)?,
                // Currency is deliberately absent: Orbital derives it from the
                // Merchant ID setup and has no field to receive it.
                // The encoding is shared; the unsigned and 12-character rules are
                // Orbital's own field constraints, so they stay visible here.
                amount: StringImpliedDecimalForConnector
                    .convert(request.minor_amount, request.currency)
                    .and_then(|amount| amount.validate_unsigned("order.amount"))
                    .and_then(|amount| amount.validate_max_len(MAX_AMOUNT_LEN, "order.amount"))
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "order.amount",
                        context: IntegrationErrorContext::default(),
                    })?,
                industry_type: INDUSTRY_TYPE_ECOMMERCE.to_string(),
                retry_trace: build_retry_trace(&common.connector_request_reference_id),
            },
            cardholder_verification,
            cryptogram,
            additional_auth_info,
        })
    }
}

// =============================================================================
// REQUEST — POST /inquiry (PSync)
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalInquiryOrder {
    #[serde(rename = "orderID", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    /// **The `retryTrace` of the original `/payments` request.** Not a counter, not
    /// the `txRefNum` — Orbital has no "get transaction by txRefNum" endpoint.
    #[serde(rename = "inquiryRetryNumber")]
    pub inquiry_retry_number: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganOrbitalInquiryRequest {
    pub version: String,
    pub merchant: JpmorganOrbitalMerchant,
    pub order: JpmorganOrbitalInquiryOrder,
}

/// Read a string out of the `connector_metadata` the Authorize response persisted, which
/// the caller echoes back as `connector_feature_data`.
///
/// Orbital has no "get transaction by `txRefNum`" endpoint, so `/inquiry` can only be
/// keyed on `orderID` / `inquiryRetryNumber`, both derived from
/// `connector_request_reference_id`. That field is supplied independently on the Get
/// request, so a sync that passes a different value derives different keys, finds no
/// record, and reports a charged payment as failed. Preferring the echoed value keys the
/// inquiry off what Authorize actually sent, and callers that echo nothing keep the
/// derivation they have today.
fn persisted_str(feature_data: Option<&SecretSerdeValue>, key: &str) -> Option<String> {
    feature_data
        .and_then(|data| data.peek().get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
}

type SyncRouterData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<JpmorganOrbitalRouterData<SyncRouterData, T>> for JpmorganOrbitalInquiryRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: JpmorganOrbitalRouterData<SyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = JpmorganOrbitalAuthType::try_from(&router_data.connector_config)?;
        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;
        let persisted = router_data
            .resource_common_data
            .connector_feature_data
            .as_ref();

        Ok(Self {
            version: ORBITAL_VERSION.to_string(),
            merchant: JpmorganOrbitalMerchant {
                bin: auth.bin.clone(),
                merchant_id: auth.merchant_id.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            order: JpmorganOrbitalInquiryOrder {
                order_id: Some(match persisted_str(persisted, "order_id") {
                    Some(order_id) => order_id,
                    None => build_order_id(reference)?,
                }),
                inquiry_retry_number: persisted_str(persisted, "retry_trace")
                    .unwrap_or_else(|| build_retry_trace(reference)),
            },
        })
    }
}

// =============================================================================
// RESPONSE
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalStatus {
    #[serde(rename = "procStatus")]
    pub proc_status: Option<String>,
    #[serde(rename = "procStatusMessage")]
    pub proc_status_message: Option<String>,
    #[serde(rename = "hostRespCode")]
    pub host_resp_code: Option<String>,
    #[serde(rename = "actualRespCd")]
    pub actual_resp_cd: Option<String>,
    /// Normalized authorization response code. `"00"` = approved; every other value
    /// is passed through verbatim.
    #[serde(rename = "respCode")]
    pub resp_code: Option<String>,
    #[serde(rename = "respCodeMessage")]
    pub resp_code_message: Option<String>,
    /// `0` declined, `1` approved, `2` message/system error.
    #[serde(rename = "approvalStatus")]
    pub approval_status: Option<String>,
    #[serde(rename = "authorizationCode")]
    pub authorization_code: Option<String>,
    #[serde(rename = "partialAuthOccurred")]
    pub partial_auth_occurred: Option<String>,
    /// CAVV response code for Visa Secure. Informational only — never derive the
    /// attempt status from it.
    #[serde(rename = "visaVbVRespCode")]
    pub visa_vbv_resp_code: Option<String>,
    #[serde(rename = "pymtBrandAuthResponseCode")]
    pub pymt_brand_auth_response_code: Option<String>,
    #[serde(rename = "pymtBrandResponseCodeCategory")]
    pub pymt_brand_response_code_category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalOrderResponse {
    #[serde(rename = "orderID")]
    pub order_id: Option<String>,
    #[serde(rename = "industryType")]
    pub industry_type: Option<String>,
    /// Gateway Transaction Reference Number → `connector_transaction_id`.
    #[serde(rename = "txRefNum")]
    pub tx_ref_num: Option<String>,
    /// Gateway Transaction Index. Required *alongside* `txRefNum` by any later
    /// Capture / Void / Refund, so it is persisted even though those flows are out of
    /// scope — without it a follow-up PR would need a data migration.
    #[serde(rename = "txRefIdx")]
    pub tx_ref_idx: Option<String>,
    #[serde(rename = "respDateTime")]
    pub resp_date_time: Option<String>,
    #[serde(rename = "authNetwkID")]
    pub auth_netwk_id: Option<String>,
    /// `0` = first response for this `retryTrace`; `>= 1` = an echoed duplicate.
    #[serde(rename = "retryAttempCount")]
    pub retry_attempt_count: Option<String>,
    pub status: Option<JpmorganOrbitalStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalCardResponse {
    #[serde(rename = "cardBrand")]
    pub card_brand: Option<String>,
    /// Masked in the response (`545454XXXXXX5454`) — never a usable PAN.
    #[serde(rename = "ccAccountNum")]
    pub cc_account_num: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalPaymentInstrumentResponse {
    pub card: Option<JpmorganOrbitalCardResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalAvsBilling {
    #[serde(rename = "avsRespCode")]
    pub avs_resp_code: Option<String>,
    #[serde(rename = "hostAVSRespCode")]
    pub host_avs_resp_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalCardholderVerificationResponse {
    #[serde(rename = "cvvRespCode")]
    pub cvv_resp_code: Option<String>,
    #[serde(rename = "hostCVVRespCode")]
    pub host_cvv_resp_code: Option<String>,
}

/// `paymentsResponse`, and also the flat `{procStatus, procStatusMessage}` error body
/// every non-2xx uses.
///
/// Orbital nests the status at `order.status` on a 200 but puts the very same two
/// fields at the **top level** on an error, so both shapes are modelled here and
/// [`JpmorganOrbitalPaymentsResponse::proc_status_value`] reads whichever is present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalPaymentsResponse {
    pub version: Option<String>,
    #[serde(rename = "transType")]
    pub trans_type: Option<String>,
    #[serde(rename = "paymentInstrument")]
    pub payment_instrument: Option<JpmorganOrbitalPaymentInstrumentResponse>,
    pub order: Option<JpmorganOrbitalOrderResponse>,
    #[serde(rename = "avsBilling")]
    pub avs_billing: Option<JpmorganOrbitalAvsBilling>,
    #[serde(rename = "cardholderVerification")]
    pub cardholder_verification: Option<JpmorganOrbitalCardholderVerificationResponse>,
    /// Top-level, error-body-only form.
    #[serde(rename = "procStatus")]
    pub proc_status: Option<String>,
    /// Top-level, error-body-only form.
    #[serde(rename = "procStatusMessage")]
    pub proc_status_message: Option<String>,
}

/// `/inquiry` answers with the same envelope; the newtype only gives the macro
/// framework a distinct response type per flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganOrbitalInquiryResponse(pub JpmorganOrbitalPaymentsResponse);

impl JpmorganOrbitalPaymentsResponse {
    fn status(&self) -> Option<&JpmorganOrbitalStatus> {
        self.order.as_ref()?.status.as_ref()
    }

    /// `procStatus` from wherever this response carries it.
    fn proc_status_value(&self) -> Option<&str> {
        self.status()
            .and_then(|status| status.proc_status.as_deref())
            .or(self.proc_status.as_deref())
    }

    fn proc_status_message_value(&self) -> Option<&str> {
        self.status()
            .and_then(|status| status.proc_status_message.as_deref())
            .or(self.proc_status_message.as_deref())
    }

    fn approval_status_value(&self) -> Option<&str> {
        self.status()
            .and_then(|status| status.approval_status.as_deref())
    }

    fn gateway_accepted(&self) -> bool {
        self.proc_status_value()
            .map(|value| value.trim() == PROC_STATUS_SUCCESS)
            .unwrap_or(false)
    }

    fn issuer_approved(&self) -> bool {
        self.approval_status_value()
            .map(|value| value.trim() == APPROVAL_STATUS_APPROVED)
            .unwrap_or(false)
    }

    /// Orbital is a two-level protocol: the Gateway edit checks must pass **and** the
    /// issuer must approve. Both levels are checked, in that order.
    pub fn is_success(&self) -> bool {
        self.gateway_accepted() && self.issuer_approved()
    }

    pub fn connector_transaction_id(&self) -> Option<String> {
        self.order
            .as_ref()?
            .tx_ref_num
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned()
    }

    fn order_id(&self) -> Option<String> {
        self.order.as_ref()?.order_id.clone()
    }

    /// Persist everything a later Capture / Void / Refund or PSync will need.
    /// `txRefIdx` in particular is useless to this flow but mandatory for those.
    fn connector_metadata(&self, retry_trace: &str) -> Option<serde_json::Value> {
        let order = self.order.as_ref()?;
        Some(serde_json::json!({
            "tx_ref_idx": order.tx_ref_idx,
            "order_id": order.order_id,
            "retry_trace": retry_trace,
            "retry_attempt_count": order.retry_attempt_count,
            "auth_netwk_id": order.auth_netwk_id,
            "authorization_code": self.status().and_then(|s| s.authorization_code.clone()),
            "visa_vbv_resp_code": self.status().and_then(|s| s.visa_vbv_resp_code.clone()),
            "avs_resp_code": self.avs_billing.as_ref().and_then(|a| a.avs_resp_code.clone()),
            "cvv_resp_code": self
                .cardholder_verification
                .as_ref()
                .and_then(|c| c.cvv_resp_code.clone()),
        }))
    }

    /// `Charged` vs `Authorized` is decided by the `transType` **sent**, not by
    /// anything in the response: an `AC` approval and an `A` approval produce byte
    /// identical `status` objects.
    fn success_status(sent_trans_type: &str) -> AttemptStatus {
        if sent_trans_type == TRANS_TYPE_AUTH_CAPTURE {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Authorized
        }
    }

    /// Almost every failure mode of this connector is terminal: `/payments` is
    /// synchronous, there is no challenge state and no webhook, so nothing here may map
    /// to `Pending` / `AuthenticationPending` / `Started`.
    ///
    /// The one exception is `procStatus` 9710, which means Orbital gave up waiting past
    /// its own 90-second ceiling. The original authorization may still have been
    /// approved, so recording it as `Failure` would both defeat the PSync recovery path
    /// and invite a double charge on re-attempt. It is `Unresolved` instead.
    ///
    /// This is the single funnel for the attempt status of every failure path — the
    /// `ErrorResponse` below and the `PaymentFlowData` of both the Authorize and PSync
    /// transforms all read it, so the 9710 carve-out cannot drift between them.
    fn failure_status(&self) -> AttemptStatus {
        if self.proc_status_value().map(str::trim) == Some(PROC_STATUS_TIMED_OUT) {
            AttemptStatus::Unresolved
        } else {
            AttemptStatus::Failure
        }
    }

    /// Build an `ErrorResponse` covering all three of Orbital's failure layers:
    /// a non-2xx with the flat `messages` body, a 200 whose `procStatus != "0"`
    /// (gateway edit-check failure) and a 200 whose `approvalStatus != "1"`
    /// (issuer decline or host error).
    pub fn to_error_response(&self, http_code: u16) -> ErrorResponse {
        let gateway_failure = !self.gateway_accepted();

        let (code, message, reason) = if gateway_failure {
            (
                self.proc_status_value()
                    .map(str::to_string)
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                self.proc_status_message_value()
                    .map(str::to_string)
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                self.proc_status_message_value().map(str::to_string),
            )
        } else {
            let status = self.status();
            (
                status
                    .and_then(|s| s.resp_code.clone())
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                status
                    .and_then(|s| s.resp_code_message.clone())
                    .or_else(|| self.proc_status_message_value().map(str::to_string))
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                status
                    .and_then(|s| s.host_resp_code.clone())
                    .or_else(|| status.and_then(|s| s.resp_code_message.clone())),
            )
        };

        ErrorResponse {
            status_code: http_code,
            code,
            message,
            reason,
            // A 408 means Orbital did not finish inside its own ceiling; the
            // authorization may still land, so the attempt is left unresolved for PSync
            // to settle. 5xx never reaches here — the framework default handles it.
            attempt_status: (http_code != HTTP_REQUEST_TIMEOUT)
                .then(|| FlowStatus::Payment(self.failure_status())),
            // The attempt exists at the gateway whenever a txRefNum came back, even
            // on a decline, so surface it.
            connector_transaction_id: self.connector_transaction_id(),
            // Orbital exposes no network decline/advice fields of its own.
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

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<JpmorganOrbitalPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JpmorganOrbitalPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let retry_trace = build_retry_trace(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        if !response.is_success() {
            return Ok(Self {
                response: Err(response.to_error_response(item.http_code)),
                resource_common_data: PaymentFlowData {
                    status: response.failure_status(),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Derived from the request, never from the response — see `success_status`.
        let sent_trans_type =
            trans_type_for(item.router_data.request.capture_method).map_err(|_| {
                error_stack::report!(crate::utils::unexpected_response_fail(
                    item.http_code,
                    "jpmorganorbital: capture_method is not supported by this connector, so the \
                     transType that produced this response cannot be reconstructed",
                ))
            })?;

        let resource_id = match response.connector_transaction_id() {
            Some(tx_ref_num) => ResponseId::ConnectorTransactionId(tx_ref_num),
            None => ResponseId::NoResponseId,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                // Orbital never redirects: 3DS is external passthrough and
                // `paymentsResponse` has no URL field of any kind.
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.connector_metadata(&retry_trace),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_id(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: JpmorganOrbitalPaymentsResponse::success_status(sent_trans_type),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<JpmorganOrbitalInquiryResponse, Self>> for SyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<JpmorganOrbitalInquiryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        // Echo the trace the inquiry was actually keyed on. Re-deriving here would
        // overwrite a correctly-echoed trace with one built from a mismatched reference.
        let retry_trace = persisted_str(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
            "retry_trace",
        )
        .unwrap_or_else(|| {
            build_retry_trace(
                &item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )
        });

        if !response.is_success() {
            return Ok(Self {
                response: Err(response.to_error_response(item.http_code)),
                resource_common_data: PaymentFlowData {
                    status: response.failure_status(),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // On `/inquiry` the echoed `transType` comes from the stored original request,
        // so it *is* authoritative here (unlike on `/payments`). When it is absent, fall
        // back to what this payment must have been sent as rather than assuming
        // authorize-only — guessing "A" would report a settled payment as merely
        // `Authorized` and tell the merchant funds were never captured.
        let sent_trans_type = trans_type_for(item.router_data.request.capture_method)
            .unwrap_or(TRANS_TYPE_AUTH_CAPTURE);
        let echoed_trans_type = response
            .trans_type
            .as_deref()
            .unwrap_or(sent_trans_type)
            .trim();

        let resource_id = match response.connector_transaction_id() {
            Some(tx_ref_num) => ResponseId::ConnectorTransactionId(tx_ref_num),
            None => item.router_data.request.connector_transaction_id.clone(),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: response.connector_metadata(&retry_trace),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.order_id(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: JpmorganOrbitalPaymentsResponse::success_status(echoed_trans_type),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
