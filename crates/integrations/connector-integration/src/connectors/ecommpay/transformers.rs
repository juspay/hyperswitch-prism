//! Ecommpay Gate API v2 — request/response types, request signing and response verification.
//!
//! Every Gate request is a JSON `POST` whose body carries `general.project_id` and
//! `general.signature`. There is no authentication header: the signature *is* the
//! authentication, so it is computed here, over the typed request, before the body leaves
//! the connector (see [`sign_request`]).
//!
//! Payment-processing calls (`sale`, `auth`, `3ds_result`, …) are asynchronous: HTTP 200 with
//! `status: "success"` only means the request was **accepted**. The payment result is read
//! later from `POST /v2/payment/status` (PSync) or from a callback, so the Authorize response
//! never reports a terminal status.
//!
//! Reference: `grace/rulesbook/codegen/references/ecommpay/technical_specification.md`

use std::{cmp::Ordering, collections::HashMap, fmt, iter::Peekable, str::Bytes};

use base64::Engine;
use common_enums::{
    AttemptStatus, CaptureMethod, CountryAlpha2, Currency, FutureUsage, MitCategory, RefundStatus,
    TransactionStatus,
};
use common_utils::{
    consts::{BASE64_ENGINE, NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{HmacSha512, SignMessage, VerifySignature},
    id_type::CustomerId,
    pii::{Email, IpAddress, SecretSerdeValue},
    request::Method,
    types::{MinorUnit, Money},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        EventType, MandateReference, MandateReferenceId, NetworkMandateIdRef, PaymentFlowData,
        PaymentVoidData, PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    errors::{self, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        ApplePayDecryptedData, ApplePayPaymentData, ApplePayWalletData, Card,
        GooglePayDecryptedData, GpayEncryptedTokenizationData, GpayTokenizationData,
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation},
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::EcommpayRouterData;
use crate::{types::ResponseRouterData, utils};

const CONNECTOR_NAME: &str = "ecommpay";
const SIGNATURE_DOC_URL: &str = "https://developers.ecommpay.com/en/en_platform_signature.html";
const SALE_DOC_URL: &str = "https://developers.ecommpay.com/en/en_gate_payment_sale.html";
const THREE_DS_DOC_URL: &str = "https://developers.ecommpay.com/en/en_gate_payment_3ds.html";
const STATUS_DOC_URL: &str =
    "https://developers.ecommpay.com/en/en_Gate_payment_status_request.html";
const CAPTURE_DOC_URL: &str = "https://developers.ecommpay.com/en/en_gate_payment_capture.html";
const CANCEL_DOC_URL: &str = "https://developers.ecommpay.com/en/en_platform_dms_model.html";
const REFUND_DOC_URL: &str = "https://developers.ecommpay.com/en/en_Gate_Refund.html";
/// `payment.description` sent when Hyperswitch supplies no refund `reason`.
const REFUND_DESCRIPTION: &str = "Refund";
const ACCOUNT_VERIFICATION_DOC_URL: &str =
    "https://developers.ecommpay.com/en/en_gate_account_verification.html";
/// `errors[].code` of a status request for a payment the Gate has not registered (yet).
const TRANSACTION_NOT_FOUND_CODE: &str = "3061";

/// `general.project_id` is an integer in this range on the wire.
const PROJECT_ID_MAX: i64 = 4_294_967_295;
/// `general.payment_id`, `customer.id`, `card.card_holder`, `payment.description`,
/// `merchant.descriptor` and `receipt_data.positions[].description` are all 1–255 chars.
const MAX_ID_LENGTH: usize = 255;
const MAX_BROWSER_LENGTH: usize = 512;
const MAX_BILLING_CITY_LENGTH: usize = 256;
const MAX_BILLING_ADDRESS_LENGTH: usize = 512;
const MAX_POSTAL_LENGTH: usize = 16;
const MAX_SHIPPING_ADDRESS_LENGTH: usize = 150;
const MAX_SHIPPING_CITY_LENGTH: usize = 50;
const MAX_REGION_LENGTH: usize = 255;
/// `receipt_data.positions` holds 1–300 items.
const MAX_RECEIPT_POSITIONS: usize = 300;
/// Characters `merchant.descriptor` must not contain.
const FORBIDDEN_DESCRIPTOR_CHARS: [char; 12] =
    ['!', '@', '&', '~', '№', '{', '}', '|', '<', '>', '[', ']'];

fn context(
    suggested_action: Option<&str>,
    doc_url: &str,
    detail: String,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: suggested_action.map(ToString::to_string),
        doc_url: Some(doc_url.to_string()),
        additional_context: Some(detail),
    }
}

fn missing_field(field_name: &'static str, detail: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::MissingRequiredField {
        field_name,
        context: context(
            Some("Supply the field on the payment request and retry."),
            SALE_DOC_URL,
            detail.to_string(),
        ),
    })
}

fn invalid_format(
    field_name: &'static str,
    detail: String,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::InvalidDataFormat {
        field_name,
        context: context(
            Some("Correct the value so it satisfies the Ecommpay Gate constraint and retry."),
            SALE_DOC_URL,
            detail,
        ),
    })
}

fn too_long(
    field_name: &str,
    max_length: usize,
    received_length: usize,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::MaxFieldLengthViolated {
        connector: CONNECTOR_NAME.to_string(),
        field_name: field_name.to_string(),
        max_length,
        received_length,
        context: context(
            Some("Shorten the value; Ecommpay rejects it and UCS never truncates silently."),
            SALE_DOC_URL,
            format!("{field_name} is limited to {max_length} characters by the Gate API"),
        ),
    })
}

/// Rejects a value longer than `max` characters instead of truncating it.
fn within_limit<S: PeekInterface<String>>(
    value: Option<S>,
    field_name: &str,
    max: usize,
) -> Result<Option<S>, error_stack::Report<IntegrationError>> {
    match value {
        Some(inner) => {
            let length = inner.peek().chars().count();
            if length > max {
                Err(too_long(field_name, max, length))
            } else {
                Ok(Some(inner))
            }
        }
        None => Ok(None),
    }
}

// =============================================================================
// Shared helpers
// =============================================================================

/// Trims an optional string and treats an empty or whitespace-only value as absent.
///
/// The gRPC layer turns an omitted string into `Some("")` in places, so `is_none()` alone lets
/// an empty return URL, email or descriptor through to the gateway.
pub fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|inner| inner.trim().to_string())
        .filter(|inner| !inner.is_empty())
}

/// [`non_empty`] for secret strings; the value is only peeked, never logged.
pub fn non_empty_secret(value: Option<Secret<String>>) -> Option<Secret<String>> {
    value
        .map(|inner| inner.peek().trim().to_string())
        .filter(|inner| !inner.is_empty())
        .map(Secret::new)
}

fn non_empty_email(value: Option<Email>) -> Option<Email> {
    value.filter(|email| !email.peek().trim().is_empty())
}

/// `customer.phone` must match `^[0-9]{4,24}$`: the billing phone's country code and number,
/// digits only. A value that cannot be normalised is omitted with a warning naming the field
/// (never the value), because the phone is optional for a card payment.
fn normalise_phone(common: &PaymentFlowData) -> Option<Secret<String>> {
    let phone = common
        .address
        .get_payment_method_billing()
        .and_then(|billing| billing.phone.as_ref())?;
    let number = phone.number.as_ref()?;
    // No country code: the number is normalised on its own.
    let digits: String = phone
        .country_code
        .iter()
        .flat_map(|code| code.chars())
        .chain(number.peek().chars())
        .filter(char::is_ascii_digit)
        .collect();
    if (4..=24).contains(&digits.len()) {
        Some(Secret::new(digits))
    } else {
        tracing::warn!(
            field = "customer.phone",
            "Ecommpay: billing phone does not normalise to 4-24 digits; customer.phone omitted"
        );
        None
    }
}

/// `customer.id`: required on every Gate payment, 1-255 characters, never truncated. `missing`
/// says why this call needs it.
fn required_customer_id(
    customer_id: Option<&CustomerId>,
    missing: &str,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let customer_id = non_empty(customer_id.map(|id| id.get_string_repr().to_string()))
        .ok_or_else(|| missing_field("customer_id", missing))?;
    let length = customer_id.chars().count();
    if length > MAX_ID_LENGTH {
        return Err(too_long("customer.id", MAX_ID_LENGTH, length));
    }
    Ok(customer_id)
}

/// `customer.language` is ISO 639-1; a browser language tag such as `en-US` is reduced to its
/// primary subtag, and anything that is not two letters is omitted with a warning.
fn browser_language(browser_info: &BrowserInformation) -> Option<String> {
    let tag = non_empty(browser_info.language.clone())?;
    let primary = tag.split(['-', '_']).next()?.to_lowercase();
    if primary.len() == 2 && primary.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(primary)
    } else {
        tracing::warn!(
            field = "customer.language",
            "Ecommpay: browser language is not an ISO 639-1 code; customer.language omitted"
        );
        None
    }
}

// =============================================================================
// Authentication
// =============================================================================

#[derive(Debug, Clone)]
pub struct EcommpayAuthType {
    /// `general.project_id`, already parsed into the integer the Gate API expects.
    pub project_id: Secret<i64>,
    /// HMAC-SHA512 signing key. Never transmitted.
    pub secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for EcommpayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Ecommpay {
                project_id,
                secret_key,
                ..
            } => {
                let project_id = project_id
                    .peek()
                    .trim()
                    .parse::<i64>()
                    .ok()
                    .filter(|id| (1..=PROJECT_ID_MAX).contains(id))
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::InvalidConnectorConfig {
                            config: "project_id",
                            context: context(
                                Some(
                                    "Configure the Ecommpay project_id as the integer project \
                                     identifier issued by Ecommpay.",
                                ),
                                SIGNATURE_DOC_URL,
                                "Ecommpay project_id must be an integer between 1 and \
                                 4294967295 (general.project_id)"
                                    .to_string(),
                            ),
                        })
                    })?;
                Ok(Self {
                    project_id: Secret::new(project_id),
                    secret_key: secret_key.to_owned(),
                })
            }
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: context(
                        Some("Configure the Ecommpay connector with project_id and secret_key."),
                        SIGNATURE_DOC_URL,
                        "The connector_config routed to Ecommpay is not the \
                     ConnectorSpecificConfig::Ecommpay variant"
                            .to_string(),
                    ),
                }
            )),
        }
    }
}

// =============================================================================
// Signature
// =============================================================================
//
// Preimage rules (techspec "Creating the Authentication Header", worked examples there):
//   * every leaf becomes `path:value`, the path being the parent keys joined by `:` from the
//     top level down; array elements use their zero-based index as the key
//     (`receipt_data:positions:0:amount:1000`);
//   * JSON booleans become `1` / `0`; numbers are rendered in decimal; strings verbatim,
//     including the empty string (`payment_description:`); `null` renders as an empty value;
//   * empty arrays and empty objects contribute nothing;
//   * the top-level `signature` and `general.signature` are never part of the preimage;
//   * the strings are sorted in natural order (digit runs compare numerically, so
//     `positions:2` precedes `positions:10`; otherwise plain byte order) and joined with `;`;
//   * signature = Base64(HMAC-SHA512(secret_key, preimage)).
//
// The integration harness re-implements these rules independently
// (`integration-tests/src/webhook_signatures.rs`) and so acts as an oracle for them; the
// baseline probe pins them against the documentation's worked examples.

fn flatten_leaves(path: &str, value: &Value, out: &mut Vec<String>) {
    let child_path = |key: &str| {
        if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}:{key}")
        }
    };
    match value {
        Value::Object(map) => map
            .iter()
            .for_each(|(key, child)| flatten_leaves(&child_path(key), child, out)),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .for_each(|(index, child)| flatten_leaves(&child_path(&index.to_string()), child, out)),
        Value::Bool(flag) => out.push(format!("{path}:{}", if *flag { "1" } else { "0" })),
        Value::Number(number) => out.push(format!("{path}:{number}")),
        Value::String(text) => out.push(format!("{path}:{text}")),
        Value::Null => out.push(format!("{path}:")),
    }
}

fn take_digit_run(bytes: &mut Peekable<Bytes<'_>>) -> String {
    let mut run = String::new();
    while let Some(digit) = bytes.next_if(u8::is_ascii_digit) {
        run.push(char::from(digit));
    }
    run
}

/// Natural-order comparison: digit runs compare by numeric value, everything else by byte.
fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left_bytes = left.bytes().peekable();
    let mut right_bytes = right.bytes().peekable();
    loop {
        match (left_bytes.peek().copied(), right_bytes.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) if l.is_ascii_digit() && r.is_ascii_digit() => {
                let left_run = take_digit_run(&mut left_bytes);
                let right_run = take_digit_run(&mut right_bytes);
                let left_value = left_run.trim_start_matches('0');
                let right_value = right_run.trim_start_matches('0');
                let ordering = left_value
                    .len()
                    .cmp(&right_value.len())
                    .then_with(|| left_value.cmp(right_value))
                    .then_with(|| left_run.len().cmp(&right_run.len()));
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(l), Some(r)) => {
                if l != r {
                    return l.cmp(&r);
                }
                left_bytes.next();
                right_bytes.next();
            }
        }
    }
}

/// The signature preimage of a JSON body, with `signature` and `general.signature` removed.
pub fn ecommpay_canonical_string(body: &Value) -> String {
    let mut unsigned = body.clone();
    if let Some(top) = unsigned.as_object_mut() {
        top.remove("signature");
        if let Some(general) = top.get_mut("general").and_then(Value::as_object_mut) {
            general.remove("signature");
        }
    }
    let mut leaves = Vec::new();
    flatten_leaves("", &unsigned, &mut leaves);
    leaves.sort_by(|left, right| natural_cmp(left, right));
    leaves.join(";")
}

fn signing_error(detail: &str) -> IntegrationError {
    IntegrationError::RequestEncodingFailed {
        context: context(
            None,
            SIGNATURE_DOC_URL,
            format!("Failed to compute the Ecommpay general.signature: {detail}"),
        ),
    }
}

/// Computes `general.signature` over a typed request whose signature is still `None`.
pub fn sign_request<S: Serialize>(
    request: &S,
    secret_key: &Secret<String>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let body = serde_json::to_value(request)
        .change_context(signing_error("the request did not serialise to JSON"))?;
    let preimage = ecommpay_canonical_string(&body);
    let mac = HmacSha512
        .sign_message(secret_key.peek().as_bytes(), preimage.as_bytes())
        .change_context(signing_error("HMAC-SHA512 failed"))?;
    Ok(Secret::new(BASE64_ENGINE.encode(mac)))
}

/// Verifies the top-level `signature` of a received status response or callback.
///
/// Returns `Ok(false)` for a missing, non-string or non-Base64 signature and for a mismatch; the
/// comparison is constant-time (`HmacSha512::verify_signature`).
pub fn verify_signature(
    body: &Value,
    secret_key: &Secret<String>,
) -> Result<bool, error_stack::Report<common_utils::errors::CryptoError>> {
    let Some(received) = body.get("signature").and_then(Value::as_str) else {
        return Ok(false);
    };
    let Ok(received) = BASE64_ENGINE.decode(received) else {
        return Ok(false);
    };
    let preimage = ecommpay_canonical_string(body);
    HmacSha512.verify_signature(secret_key.peek().as_bytes(), &received, preimage.as_bytes())
}

// =============================================================================
// Payment route
// =============================================================================

/// Which Gate path family a payment was created on. Card and wallet payments are captured,
/// cancelled and refunded on their own path (`/v2/payment/{card|applepay|googlepay}/…`), so
/// Authorize records the route in `connector_metadata` and the follow-up flows read it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EcommpayRoute {
    Card,
    Applepay,
    Googlepay,
}

impl EcommpayRoute {
    pub fn path_segment(self) -> &'static str {
        match self {
            Self::Card => "card",
            Self::Applepay => "applepay",
            Self::Googlepay => "googlepay",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayRouteMetadata {
    pub ecommpay_route: EcommpayRoute,
}

/// `connector_metadata` written by Authorize.
pub fn route_to_feature_data(route: EcommpayRoute) -> Option<Value> {
    serde_json::to_value(EcommpayRouteMetadata {
        ecommpay_route: route,
    })
    .ok()
}

/// `connector_metadata` written by the Authorize acceptance: the route plus `payment_handle_token`
/// (the Gate payment_id).
///
/// # Cross-repo contract
///
/// `payment_handle_token` is read by Hyperswitch, not by this connector, and the key name is the
/// contract. On the post-3DS settle leg the card is not persisted through the redirect (ecommpay
/// has no temp locker), so HS builds the gRPC `payment_method` from a named lookup in
/// `connector_meta`:
///
/// ```text
/// router_data.request.connector_meta["payment_handle_token"] -> PaymentMethod::Token
/// // hyperswitch crates/router/src/core/unified_connector_service/transformers.rs
/// ```
///
/// That lookup has no `connector_transaction_id` fallback, and the generic
/// `reconstruct_payment_method_data_for_redirect_completion` ahead of it does not cover cards.
/// So if this key stops being written, `payment_method` is `None` on the settle leg and UCS
/// rejects the Authorize with `MissingRequiredField: payment_method` -- every 3DS card payment
/// fails to settle. Renaming it here requires the matching change in Hyperswitch.
///
/// The value is the Gate `payment_id`, not a credential. Ecommpay itself ignores it on the
/// `3ds_result` re-entry; only HS consumes it.
pub fn authorize_feature_data(route: EcommpayRoute, payment_id: &str) -> Option<Value> {
    let mut value = route_to_feature_data(route)?;
    value.as_object_mut()?.insert(
        "payment_handle_token".to_string(),
        Value::String(payment_id.to_string()),
    );
    Some(value)
}

/// The route recorded by Authorize; a payment without one was created on the card path.
pub fn route_from_feature_data(connector_feature_data: Option<&SecretSerdeValue>) -> EcommpayRoute {
    connector_feature_data
        .and_then(|data| serde_json::from_value::<EcommpayRouteMetadata>(data.peek().clone()).ok())
        .map(|metadata| metadata.ecommpay_route)
        .unwrap_or(EcommpayRoute::Card)
}

// =============================================================================
// Shared request structs
// =============================================================================
//
// Every `Option` is skipped when `None`, so no `null` ever reaches the wire or the signature.

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayGeneral {
    pub project_id: Secret<i64>,
    pub payment_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_callback_url: Option<String>,
}

/// `customer.billing` and `customer.shipping` carry the same five address fields.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EcommpayAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct EcommpayCustomer {
    pub id: String,
    pub ip_address: Secret<String, IpAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_res: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<EcommpayAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<EcommpayAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_header: Option<String>,
    /// The browser User-Agent (max 512).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub js_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Minutes from UTC as a string ("-60"): the Gate drops a payment whose
    /// timezone_offset is a JSON integer (status 3061 on every later status request).
    pub timezone_offset: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayPaymentInfo {
    pub amount: MinorUnit,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayReturnUrl {
    pub success: String,
    pub decline: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayAcsReturnUrl {
    pub return_url: String,
    #[serde(rename = "3ds_notification_url")]
    pub notification_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayCard<T: PaymentMethodDataTypes> {
    pub pan: RawCardNumber<T>,
    /// Four-digit year, sent as an integer.
    pub year: Secret<u16>,
    /// 1–12 without a leading zero, sent as an integer.
    pub month: Secret<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder: Option<Secret<String>>,
    pub cvv: Secret<String>,
}

/// `card` of a merchant-stored COF debit (NTID-based MIT, RepeatPayment): the stored card,
/// linked to its registration by the network transaction id. No CVV is sent for an
/// on-demand / regular COF debit.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayStoredCard<T: PaymentMethodDataTypes> {
    pub pan: RawCardNumber<T>,
    /// Four-digit year, sent as an integer.
    pub year: Secret<u16>,
    /// 1–12 without a leading zero, sent as an integer.
    pub month: Secret<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder: Option<Secret<String>>,
    pub stored_card_type: EcommpayStoredCardType,
    pub scheme_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_link_id: Option<Secret<String>>,
}

/// `card.stored_card_type` of a merchant-initiated COF debit, sent as an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcommpayStoredCardType {
    /// `4` — on-demand (autopurchase); the only type `/v2/payment/card/auth` accepts.
    OnDemand,
    /// `6` — regular (scheduled) COF debit.
    Regular,
}

impl Serialize for EcommpayStoredCardType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(match self {
            Self::OnDemand => 4,
            Self::Regular => 6,
        })
    }
}

/// The `card` object: a raw card or a merchant-stored COF card.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EcommpayPaymentCard<T: PaymentMethodDataTypes> {
    Raw(EcommpayCard<T>),
    StoredCard(EcommpayStoredCard<T>),
}

/// `etoken.google_tokenization_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EcommpayGoogleTokenizationType {
    PaymentGateway,
    Direct,
}

/// `etoken` of a PSP-decrypted wallet: the encrypted token Ecommpay decrypts itself.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayEncryptedEtoken {
    /// Apple Pay: the payment token JSON as a string (not Base64). Google Pay: the encrypted
    /// `tokenizationData.token` unchanged.
    pub token: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_tokenization_type: Option<EcommpayGoogleTokenizationType>,
}

/// `etoken` of a Hyperswitch-decrypted Google Pay token (`/v2/payment/googlepay/…`).
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayGoogleDecryptedEtoken {
    pub google_gateway_type: &'static str,
    pub google_gateway_id: &'static str,
    pub google_tokenization_type: EcommpayGoogleTokenizationType,
    pub decrypted_token: EcommpayGoogleDecryptedToken,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayGoogleDecryptedToken {
    /// Unix time in milliseconds; Ecommpay silently drops a sale whose token has expired.
    #[serde(rename = "messageExpiration")]
    pub message_expiration: i64,
    #[serde(rename = "paymentMethodDetails")]
    pub payment_method_details: EcommpayGooglePaymentMethodDetails,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayGooglePaymentMethodDetails {
    pub pan: cards::CardNumber,
    /// 1-12 as an integer; a string is silently dropped.
    #[serde(rename = "expirationMonth")]
    pub expiration_month: Secret<u8>,
    /// Four-digit year as an integer; a string is silently dropped.
    #[serde(rename = "expirationYear")]
    pub expiration_year: Secret<u16>,
    #[serde(rename = "eciIndicator", skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
    /// The device cryptogram (TAVV). Never logged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(rename = "authMethod")]
    pub auth_method: EcommpayGoogleAuthMethod,
}

/// `paymentMethodDetails.authMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EcommpayGoogleAuthMethod {
    #[serde(rename = "CRYPTOGRAM_3DS")]
    Cryptogram3ds,
    #[serde(rename = "PAN_ONLY")]
    PanOnly,
}

/// `etoken` of a Hyperswitch-decrypted Apple Pay token (`/v2/payment/applepay/…`).
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayAppleDecryptedEtoken {
    #[serde(rename = "type")]
    pub token_type: &'static str,
    pub decrypted_token: EcommpayAppleDecryptedToken,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayAppleDecryptedToken {
    #[serde(rename = "applicationPrimaryAccountNumber")]
    pub application_primary_account_number: cards::CardNumber,
    /// `YYMMDD`.
    #[serde(rename = "applicationExpirationDate")]
    pub application_expiration_date: Secret<String>,
    /// ISO 4217 numeric code, e.g. `978`.
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    /// Minor units. Apple defines this field inside the decrypted token, so it is always
    /// minor units of `currency_code` and deliberately does not go through the connector's
    /// `amount_converter` (which encodes Ecommpay's own request convention, not Apple's).
    #[serde(rename = "transactionAmount")]
    pub transaction_amount: MinorUnit,
    #[serde(rename = "paymentDataType")]
    pub payment_data_type: &'static str,
    #[serde(rename = "deviceManufacturerIdentifier")]
    pub device_manufacturer_identifier: Secret<String>,
    #[serde(rename = "paymentData")]
    pub payment_data: EcommpayApplePaymentData,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayApplePaymentData {
    #[serde(rename = "onlinePaymentCryptogram")]
    pub online_payment_cryptogram: Secret<String>,
    #[serde(rename = "eciIndicator", skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
}

/// `etoken`: a wallet token, encrypted (Ecommpay decrypts it) or already decrypted by Hyperswitch.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EcommpayEtoken {
    Encrypted(EcommpayEncryptedEtoken),
    GoogleDecrypted(EcommpayGoogleDecryptedEtoken),
    AppleDecrypted(EcommpayAppleDecryptedEtoken),
}

impl EcommpayEtoken {
    /// Whether the issuer may still interpose 3-D Secure. A Google Pay token with a device
    /// cryptogram is already authenticated; a PAN-only or still-encrypted one may not be.
    fn may_require_three_ds(&self) -> bool {
        match self {
            Self::Encrypted(_) => true,
            Self::GoogleDecrypted(token) => {
                token.decrypted_token.payment_method_details.auth_method
                    == EcommpayGoogleAuthMethod::PanOnly
            }
            Self::AppleDecrypted(_) => false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayAvsData {
    pub avs_post_code: Secret<String>,
    pub avs_street_address: Secret<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayMerchant {
    pub descriptor: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayReceiptPosition {
    /// Line total (unit amount × quantity), minor units.
    pub amount: MinorUnit,
    pub quantity: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayReceiptData {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub positions: Vec<EcommpayReceiptPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tax_amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayAuthenticationData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threeds_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threeds_full_version: Option<String>,
    pub xid: String,
    pub authentication_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcommpayRecurring {
    pub register: bool,
    #[serde(rename = "type")]
    pub recurring_type: String,
}

// =============================================================================
// Authorize request
// =============================================================================

/// `POST /v2/payment/card/sale` and `/v2/payment/card/auth` body.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayPaymentRequest<T: PaymentMethodDataTypes> {
    pub general: EcommpayGeneral,
    pub customer: EcommpayCustomer,
    pub payment: EcommpayPaymentInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<EcommpayReturnUrl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acs_return_url: Option<EcommpayAcsReturnUrl>,
    /// The card. Absent for a wallet payment (`etoken`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<EcommpayPaymentCard<T>>,
    /// Apple Pay / Google Pay token, encrypted or decrypted (`/v2/payment/{applepay|googlepay}/…`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etoken: Option<EcommpayEtoken>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_data: Option<EcommpayAvsData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant: Option<EcommpayMerchant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_data: Option<EcommpayReceiptData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_data: Option<EcommpayAuthenticationData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring: Option<EcommpayRecurring>,
}

/// `POST /v2/payment/card/3ds_result` body: completes the suspended `sale`/`auth` identified
/// by the same `general.payment_id`. It creates no new operation and moves no money itself.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayThreeDsResultRequest {
    pub general: EcommpayGeneral,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cres: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pares: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md: Option<Secret<String>>,
}

/// Authorize body: the initial payment, or the 3-D Secure completion on the Authorize
/// re-entry. The leg is chosen by [`is_three_ds_completion`], which `get_url` also uses, so the
/// URL and the body cannot disagree.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EcommpayAuthorizeRequest<T: PaymentMethodDataTypes> {
    Payment(Box<EcommpayPaymentRequest<T>>),
    ThreeDsResult(Box<EcommpayThreeDsResultRequest>),
}

/// Whether this Authorize invocation is the 3DS completion leg (the ACS post-back arrived).
pub fn is_three_ds_completion<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    request.redirect_response.is_some()
}

/// `general.payment_id` is the UCS reference, 1–255 characters.
fn payment_id(common: &PaymentFlowData) -> Result<String, error_stack::Report<IntegrationError>> {
    let reference = common.connector_request_reference_id.clone();
    let length = reference.chars().count();
    if length > MAX_ID_LENGTH {
        return Err(too_long("general.payment_id", MAX_ID_LENGTH, length));
    }
    Ok(reference)
}

/// Guards shared by both Authorize legs (capture method and merchant-initiated), plus the
/// payment-leg-only amount guard (the 3ds_result completion carries no amount and also completes a
/// zero-amount account verification).
fn check_common_guards<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    // Multiple partial captures and scheduled capture are not offered.
    if let Some(method @ (CaptureMethod::ManualMultiple | CaptureMethod::Scheduled)) =
        request.capture_method
    {
        return Err(error_stack::report!(
            IntegrationError::CaptureMethodNotSupported {
                context: context(
                    Some("Use capture_method AUTOMATIC or MANUAL."),
                    "https://developers.ecommpay.com/en/en_gate_payment_capture.html",
                    format!(
                        "Ecommpay Gate supports one-step (sale) and two-step (auth + capture) \
                     payments only; capture_method {method} is not supported"
                    ),
                ),
            }
        ));
    }
    // payment.amount is 1..10^13 minor units; zero and negative are refused on
    // the payment leg only.
    let minor_amount = request.amount.amount.get_amount_as_i64();
    if !is_three_ds_completion(request) && minor_amount <= 0 {
        return Err(invalid_format(
            "amount",
            format!("payment.amount must be a positive integer in minor units, got {minor_amount}"),
        ));
    }
    // A merchant-initiated payment goes through RepeatPayment.
    if request.mandate_id.is_some() {
        return Err(error_stack::report!(IntegrationError::NotSupported {
            message: "Merchant-initiated payments through Authorize (mandate_id)".to_string(),
            connector: CONNECTOR_NAME,
            context: context(
                Some("Charge a stored credential through RecurringPaymentService/Charge."),
                "https://developers.ecommpay.com/en/en_Gate__cof_purchase.html",
                "Ecommpay COF debits use /v2/payment/card/recurring (RepeatPayment)".to_string(),
            ),
        }));
    }
    Ok(())
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for EcommpayAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
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
        let common = &router_data.resource_common_data;

        check_common_guards(request)?;
        let payment_id = payment_id(common)?;
        let auth = EcommpayAuthType::try_from(&router_data.connector_config)?;

        if is_three_ds_completion(request) {
            let mut body = build_three_ds_result(request, &auth, payment_id)?;
            body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
            return Ok(Self::ThreeDsResult(Box::new(body)));
        }

        // The payment instrument, every arm matched explicitly.
        let source = payment_source(request)?;

        // Customer id and IP are required on every Gate payment, card or wallet.
        let customer_id = required_customer_id(
            common.customer_id.as_ref().or(request.customer_id.as_ref()),
            "Ecommpay requires customer.id on every payment (CustomerInfoCard)",
        )?;
        let browser_info = request.browser_info.as_ref();
        let ip_address = browser_info
            .and_then(|info| info.ip_address)
            .map(|ip| Secret::<String, IpAddress>::new(ip.to_string()))
            .ok_or_else(|| {
                missing_field(
                    "browser_info.ip_address",
                    "Ecommpay requires customer.ip_address on every payment",
                )
            })?;

        // A card without an external 3DS result, and a wallet token the issuer may still
        // challenge,
        // (PSP-decrypted Google Pay): Ecommpay may interpose 3-D Secure. The Gate echoes
        // return_url.success as the hosted acs page's TermUrl, which receives the PaRes+MD
        // post-back, so it is the complete-authorize URL (the orchestrator then re-enters
        // Authorize -> 3ds_result); router_return_url only syncs and stays the decline URL.
        let router_return_url = non_empty(request.router_return_url.clone());
        let complete_authorize_url = non_empty(request.complete_authorize_url.clone());
        let may_redirect = match &source {
            EcommpayPaymentSource::Card(_) => request.authentication_data.is_none(),
            EcommpayPaymentSource::GooglePayToken(etoken) => etoken.may_require_three_ds(),
            EcommpayPaymentSource::ApplePayToken(_) => false,
        };
        if may_redirect {
            if router_return_url.is_none() {
                return Err(missing_field(
                    "return_url",
                    "Ecommpay may require 3-D Secure on this payment; return_url.success / \
                     decline are needed to bring the customer back",
                ));
            }
            // Wallet arm only: the card arm needs return_url alone.
            if matches!(source, EcommpayPaymentSource::GooglePayToken(_))
                && non_empty(request.complete_authorize_url.clone()).is_none()
            {
                return Err(missing_field(
                    "complete_authorize_url",
                    "Ecommpay may require 3-D Secure on this Google Pay payment; the \
                     complete-authorize URL is needed to finish it",
                ));
            }
        }

        // External 3DS result, a card-payment input only. A decrypted wallet
        // carries its own cryptogram and ECI in its etoken.
        let authentication_data = match &source {
            EcommpayPaymentSource::Card(_) => request
                .authentication_data
                .as_ref()
                .map(build_authentication_data)
                .transpose()?,
            EcommpayPaymentSource::ApplePayToken(_) | EcommpayPaymentSource::GooglePayToken(_) => {
                None
            }
        };
        // Dynamic descriptor.
        let merchant = build_merchant(request)?;
        // Receipt data (L2/L3 line items).
        let receipt_data = build_receipt_data(request, common)?;

        let amount = item
            .connector
            .amount_converter
            .convert(&request.amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: context(
                    None,
                    SALE_DOC_URL,
                    "Failed to convert the amount into Ecommpay minor units".to_string(),
                ),
            })?;

        let customer = build_customer(
            request.browser_info.as_ref(),
            request.email.clone(),
            common,
            customer_id,
            ip_address,
        )?;

        // A PAYMENT_GATEWAY Google Pay token needs customer.email.
        if matches!(source, EcommpayPaymentSource::GooglePayToken(_)) && customer.email.is_none() {
            return Err(missing_field(
                "email",
                "Ecommpay requires customer.email for a Google Pay PAYMENT_GATEWAY token",
            ));
        }

        let (card, etoken) = match source {
            EcommpayPaymentSource::Card(card) => (Some(EcommpayPaymentCard::Raw(card)), None),
            EcommpayPaymentSource::ApplePayToken(etoken)
            | EcommpayPaymentSource::GooglePayToken(etoken) => (None, Some(etoken)),
        };

        let mut body = EcommpayPaymentRequest {
            general: EcommpayGeneral {
                project_id: auth.project_id.clone(),
                payment_id,
                signature: None,
                merchant_callback_url: non_empty(request.webhook_url.clone()),
            },
            customer,
            payment: EcommpayPaymentInfo {
                amount,
                currency: request.amount.currency,
                description: description(common),
            },
            return_url: router_return_url.map(|decline| EcommpayReturnUrl {
                success: complete_authorize_url
                    .clone()
                    .unwrap_or_else(|| decline.clone()),
                decline,
            }),
            // Never sent: with acs_return_url present the Gate keeps 3DS data out of the
            // status response, so the hosted acs page could not be reached via PSync.
            acs_return_url: None,
            card,
            etoken,
            avs_data: build_avs_data(common),
            merchant,
            receipt_data,
            authentication_data,
            recurring: build_recurring(request),
        };
        body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
        Ok(Self::Payment(Box::new(body)))
    }
}

/// `payment.description` (max 255): sent when present; an over-long description is omitted
/// with a warning because the field is informational only.
fn description(common: &PaymentFlowData) -> Option<String> {
    non_empty(common.description.clone()).filter(|text| {
        let fits = text.chars().count() <= MAX_ID_LENGTH;
        if !fits {
            tracing::warn!(
                field = "payment.description",
                "Ecommpay: description exceeds 255 characters; payment.description omitted"
            );
        }
        fits
    })
}

fn build_card<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<EcommpayCard<T>, error_stack::Report<IntegrationError>> {
    let year = utils::parse_expiry_year_four_digit(
        card.get_expiry_year_4_digit().peek(),
        "card_exp_year",
    )?;
    let month = utils::parse_expiry_month(card.card_exp_month.peek(), "card_exp_month")?;
    let card_holder = within_limit(
        non_empty_secret(card.card_holder_name.clone()),
        "card.card_holder",
        MAX_ID_LENGTH,
    )?;
    Ok(EcommpayCard {
        pan: card.card_number.clone(),
        year: Secret::new(year),
        month: Secret::new(month),
        card_holder,
        cvv: card.card_cvc.clone(),
    })
}

// =============================================================================
// Wallets
// =============================================================================

const APPLE_PAY_DOC_URL: &str = "https://developers.ecommpay.com/en/pm_applepay.html";
const GOOGLE_PAY_DOC_URL: &str = "https://developers.ecommpay.com/en/pm_googlepay.html";
/// `GpayEncryptedTokenizationData.token_type` of a token Ecommpay can decrypt on its own.
const GOOGLE_PAY_PAYMENT_GATEWAY: &str = "PAYMENT_GATEWAY";

/// The payment instrument of an Authorize request.
enum EcommpayPaymentSource<T: PaymentMethodDataTypes> {
    Card(EcommpayCard<T>),
    /// Apple Pay (`/v2/payment/applepay/…`), PSP- or Hyperswitch-decrypted.
    ApplePayToken(EcommpayEtoken),
    /// Google Pay (`/v2/payment/googlepay/…`), PSP- or Hyperswitch-decrypted.
    GooglePayToken(EcommpayEtoken),
}

/// The Gate path family an Authorize request is sent on. `get_url` and the `ecommpay_route`
/// written into `connector_metadata` both read it, so the two cannot disagree.
pub fn authorize_route<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> EcommpayRoute {
    match payment_method_data {
        PaymentMethodData::Wallet(WalletData::ApplePay(_)) => EcommpayRoute::Applepay,
        PaymentMethodData::Wallet(WalletData::GooglePay(_)) => EcommpayRoute::Googlepay,
        // Cards, and every refused payment method (never sent).
        _ => EcommpayRoute::Card,
    }
}

fn unimplemented_payment_method(
    doc_url: &str,
    detail: &str,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotImplemented(
        utils::get_unimplemented_payment_method_error_message(CONNECTOR_NAME),
        context(None, doc_url, detail.to_string()),
    ))
}

/// Selects and builds the payment instrument; every unsupported arm is refused here, before
/// any request is built.
fn payment_source<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<EcommpayPaymentSource<T>, error_stack::Report<IntegrationError>> {
    match &request.payment_method_data {
        PaymentMethodData::Card(card) => Ok(EcommpayPaymentSource::Card(build_card(card)?)),
        PaymentMethodData::Wallet(wallet) => {
            wallet_source(wallet, request.amount.currency, request.amount.amount)
        }
        PaymentMethodData::CardWithNoCvc(_)
        | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::CardRedirect(_)
        | PaymentMethodData::PayLater(_)
        | PaymentMethodData::BankRedirect(_)
        | PaymentMethodData::BankDebit(_)
        | PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::Crypto(_)
        | PaymentMethodData::MandatePayment
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::PaymentMethodToken(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::MobilePayment(_) => Err(unimplemented_payment_method(
            SALE_DOC_URL,
            "Only card, Apple Pay and Google Pay payments are implemented for Ecommpay",
        )),
    }
}

fn wallet_source<T: PaymentMethodDataTypes>(
    wallet: &WalletData,
    currency: Currency,
    minor_amount: MinorUnit,
) -> Result<EcommpayPaymentSource<T>, error_stack::Report<IntegrationError>> {
    match wallet {
        WalletData::ApplePay(apple_pay) => match &apple_pay.payment_data {
            ApplePayPaymentData::Encrypted(payment_data) => Ok(
                EcommpayPaymentSource::ApplePayToken(apple_pay_etoken(apple_pay, payment_data)?),
            ),
            ApplePayPaymentData::Decrypted(decrypted) => Ok(EcommpayPaymentSource::ApplePayToken(
                apple_pay_decrypted_etoken(decrypted, currency, minor_amount)?,
            )),
        },
        WalletData::GooglePay(google_pay) => match &google_pay.tokenization_data {
            GpayTokenizationData::Encrypted(encrypted) => Ok(
                EcommpayPaymentSource::GooglePayToken(google_pay_etoken(encrypted)?),
            ),
            GpayTokenizationData::Decrypted(decrypted) => Ok(
                EcommpayPaymentSource::GooglePayToken(google_pay_decrypted_etoken(decrypted)?),
            ),
        },
        WalletData::AliPayQr(_)
        | WalletData::AliPayRedirect(_)
        | WalletData::AliPayHkRedirect(_)
        | WalletData::BluecodeRedirect {}
        | WalletData::AmazonPayRedirect(_)
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::DanaRedirect {}
        | WalletData::GrabpayRedirect {}
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_)
        | WalletData::MbWayRedirect(_)
        | WalletData::MobilePayRedirect(_)
        | WalletData::PaypalRedirect(_)
        | WalletData::PaypalSdk(_)
        | WalletData::Paze(_)
        | WalletData::SamsungPay(_)
        | WalletData::TwintRedirect {}
        | WalletData::VippsRedirect {}
        | WalletData::TouchNGoRedirect(_)
        | WalletData::WeChatPayRedirect(_)
        | WalletData::WeChatPayQr(_)
        | WalletData::CashappQr(_)
        | WalletData::SwishQr(_)
        | WalletData::Mifinity(_)
        | WalletData::RevolutPay(_)
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
        | WalletData::Skrill(_)
        | WalletData::Neteller(_) => Err(unimplemented_payment_method(
            SALE_DOC_URL,
            "Ecommpay Gate wallets are implemented for Apple Pay and Google Pay only",
        )),
    }
}

/// Apple Pay PSP decryption (spec "1. Apple Pay — gateway (PSP) decryption"). Hyperswitch
/// carries `paymentData` Base64-encoded; Ecommpay wants the whole Apple Pay token
/// (`paymentData`, `paymentMethod`, `transactionIdentifier`) as a JSON string, not Base64.
/// anything that is not Base64 JSON is refused.
fn apple_pay_etoken(
    apple_pay: &ApplePayWalletData,
    encrypted_payment_data: &str,
) -> Result<EcommpayEtoken, error_stack::Report<IntegrationError>> {
    let invalid_token = |detail: &str| {
        error_stack::report!(IntegrationError::InvalidWalletToken {
            wallet_name: "Apple Pay".to_string(),
            context: context(
                Some("Pass the Apple Pay paymentData exactly as received, Base64-encoded."),
                APPLE_PAY_DOC_URL,
                detail.to_string(),
            ),
        })
    };
    let decoded = BASE64_ENGINE
        .decode(encrypted_payment_data.trim())
        .map_err(|_| invalid_token("Apple Pay payment_data is not valid Base64"))?;
    let payment_data = serde_json::from_slice::<Value>(&decoded)
        .map_err(|_| invalid_token("Apple Pay payment_data does not decode to JSON"))?;
    if !payment_data.is_object() {
        return Err(invalid_token(
            "Apple Pay payment_data does not decode to a JSON object",
        ));
    }
    let token = serde_json::json!({
        "paymentData": payment_data,
        "paymentMethod": {
            "displayName": apple_pay.payment_method.display_name,
            "network": apple_pay.payment_method.network,
            "type": apple_pay.payment_method.pm_type,
        },
        "transactionIdentifier": apple_pay.transaction_identifier,
    });
    let token = serde_json::to_string(&token)
        .map_err(|_| invalid_token("the Apple Pay token could not be serialised"))?;
    Ok(EcommpayEtoken::Encrypted(EcommpayEncryptedEtoken {
        token: Secret::new(token),
        google_tokenization_type: None,
    }))
}

/// Google Pay PSP decryption (spec "2. Google Pay — gateway (PSP) decryption").
/// only a `PAYMENT_GATEWAY` token is accepted; `DIRECT` needs
/// `etoken.google_merchant_id`, which the payment request does not carry.
fn google_pay_etoken(
    encrypted: &GpayEncryptedTokenizationData,
) -> Result<EcommpayEtoken, error_stack::Report<IntegrationError>> {
    if encrypted.token_type != GOOGLE_PAY_PAYMENT_GATEWAY {
        return Err(error_stack::report!(IntegrationError::NotSupported {
            message: "google_tokenization_type DIRECT needs google_merchant_id".to_string(),
            connector: CONNECTOR_NAME,
            context: context(
                Some("Use Google Pay PAYMENT_GATEWAY tokenization with gateway `ecommpay`."),
                GOOGLE_PAY_DOC_URL,
                "Only Google Pay tokens of type PAYMENT_GATEWAY can be sent to Ecommpay; the \
                 payment request has no google_merchant_id for DIRECT tokenization"
                    .to_string(),
            ),
        }));
    }
    if encrypted.token.trim().is_empty() {
        return Err(error_stack::report!(IntegrationError::InvalidWalletToken {
            wallet_name: "Google Pay".to_string(),
            context: context(
                Some("Pass the Google Pay tokenizationData.token exactly as received."),
                GOOGLE_PAY_DOC_URL,
                "Google Pay tokenization_data.token is empty".to_string(),
            ),
        }));
    }
    Ok(EcommpayEtoken::Encrypted(EcommpayEncryptedEtoken {
        // Sent unchanged: Ecommpay decrypts the token as Google Pay issued it.
        token: Secret::new(encrypted.token.clone()),
        google_tokenization_type: Some(EcommpayGoogleTokenizationType::PaymentGateway),
    }))
}

/// Placeholder epoch-millis expiry for a Hyperswitch-decrypted Google Pay token. Ecommpay
/// requires `messageExpiration` to be in the future, but Hyperswitch parses Google's own value
/// and drops it before forwarding to UCS (`GooglePayDecryptedData` has no such field), so it
/// cannot be passed through -- tracked in juspay/hyperswitch#11684. A fixed far-future value is
/// sent, as paypal and paysafe already do, rather than a clock-derived one: nothing here knows
/// the token's real validity, and a constant keeps the transformer deterministic.
const GOOGLE_PAY_MESSAGE_EXPIRATION_MS: i64 = 9_999_999_999_999;

fn decrypted_wallet_field(
    field_name: &'static str,
    detail: &str,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::MissingRequiredField {
        field_name,
        context: context(
            Some("Supply the decrypted wallet token data and retry."),
            GOOGLE_PAY_DOC_URL,
            detail.to_string(),
        ),
    })
}

/// Hyperswitch-decrypted Google Pay (`etoken.decrypted_token`, spec "3. Apple Pay / Google Pay
/// — Hyperswitch decryption"). A token with a device cryptogram is `CRYPTOGRAM_3DS`; one
/// without is `PAN_ONLY`, which Ecommpay may route through 3-D Secure.
fn google_pay_decrypted_etoken(
    decrypted: &GooglePayDecryptedData,
) -> Result<EcommpayEtoken, error_stack::Report<IntegrationError>> {
    if decrypted
        .application_primary_account_number
        .peek()
        .trim()
        .is_empty()
    {
        return Err(decrypted_wallet_field(
            "application_primary_account_number",
            "Ecommpay requires paymentMethodDetails.pan (the decrypted Google Pay card number)",
        ));
    }
    let year = decrypted.get_four_digit_expiry_year().map_err(|_| {
        invalid_format(
            "card_exp_year",
            "Google Pay card_exp_year must be a two- or four-digit year".to_string(),
        )
    })?;
    let expiration_year = utils::parse_expiry_year_four_digit(year.peek(), "card_exp_year")?;
    let expiration_month =
        utils::parse_expiry_month(decrypted.card_exp_month.peek(), "card_exp_month")?;
    let cryptogram = non_empty_secret(decrypted.cryptogram.clone());
    let auth_method = if cryptogram.is_some() {
        EcommpayGoogleAuthMethod::Cryptogram3ds
    } else {
        EcommpayGoogleAuthMethod::PanOnly
    };
    Ok(EcommpayEtoken::GoogleDecrypted(
        EcommpayGoogleDecryptedEtoken {
            google_gateway_type: "gateway",
            google_gateway_id: "ecommpay",
            google_tokenization_type: EcommpayGoogleTokenizationType::PaymentGateway,
            decrypted_token: EcommpayGoogleDecryptedToken {
                message_expiration: GOOGLE_PAY_MESSAGE_EXPIRATION_MS,
                payment_method_details: EcommpayGooglePaymentMethodDetails {
                    pan: decrypted.application_primary_account_number.clone(),
                    expiration_month: Secret::new(expiration_month),
                    expiration_year: Secret::new(expiration_year),
                    eci_indicator: non_empty(decrypted.eci_indicator.clone()),
                    cryptogram,
                    auth_method,
                },
            },
        },
    ))
}

/// The last day of a month, for Apple Pay's `YYMMDD` expiry (the token carries only the month).
fn last_day_of_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Hyperswitch-decrypted Apple Pay (`etoken.decrypted_token`, spec "3. Apple Pay / Google Pay —
/// Hyperswitch decryption").
fn apple_pay_decrypted_etoken(
    decrypted: &ApplePayDecryptedData,
    currency: Currency,
    minor_amount: MinorUnit,
) -> Result<EcommpayEtoken, error_stack::Report<IntegrationError>> {
    if decrypted
        .application_primary_account_number
        .peek()
        .trim()
        .is_empty()
    {
        return Err(decrypted_wallet_field(
            "application_primary_account_number",
            "Ecommpay requires applicationPrimaryAccountNumber (the decrypted Apple Pay card number)",
        ));
    }
    let year = utils::parse_expiry_year_four_digit(
        decrypted.get_four_digit_expiry_year().peek(),
        "application_expiration_year",
    )?;
    let month = utils::parse_expiry_month(
        decrypted.application_expiration_month.peek(),
        "application_expiration_month",
    )?;
    let application_expiration_date = format!(
        "{:02}{month:02}{:02}",
        year % 100,
        last_day_of_month(year, month)
    );
    let online_payment_cryptogram = non_empty_secret(Some(
        decrypted.payment_data.online_payment_cryptogram.clone(),
    ))
    .ok_or_else(|| {
        decrypted_wallet_field(
            "online_payment_cryptogram",
            "Ecommpay requires paymentData.onlinePaymentCryptogram",
        )
    })?;
    // Without it Ecommpay accepts the call but never registers the sale (checked on the
    // sandbox), so a missing value is refused up front.
    let device_manufacturer_identifier =
        non_empty_secret(decrypted.device_manufacturer_identifier.clone()).ok_or_else(|| {
            decrypted_wallet_field(
                "device_manufacturer_identifier",
                "Ecommpay requires deviceManufacturerIdentifier on a decrypted Apple Pay token",
            )
        })?;
    Ok(EcommpayEtoken::AppleDecrypted(
        EcommpayAppleDecryptedEtoken {
            token_type: "apple",
            decrypted_token: EcommpayAppleDecryptedToken {
                application_primary_account_number: decrypted
                    .application_primary_account_number
                    .clone(),
                application_expiration_date: Secret::new(application_expiration_date),
                currency_code: currency.iso_4217().to_string(),
                transaction_amount: minor_amount,
                payment_data_type: "3DSecure",
                device_manufacturer_identifier,
                payment_data: EcommpayApplePaymentData {
                    online_payment_cryptogram,
                    eci_indicator: non_empty(decrypted.payment_data.eci_indicator.clone()),
                },
            },
        },
    ))
}

fn join_lines(
    line1: Option<Secret<String>>,
    line2: Option<Secret<String>>,
) -> Option<Secret<String>> {
    match (non_empty_secret(line1), non_empty_secret(line2)) {
        (Some(first), Some(second)) => {
            Some(Secret::new(format!("{} {}", first.peek(), second.peek())))
        }
        (Some(first), None) => Some(first),
        (None, second) => second,
    }
}

/// `customer.billing.*` (requirement row 22). `None` when no billing field is present.
fn build_billing(
    common: &PaymentFlowData,
) -> Result<Option<EcommpayAddress>, error_stack::Report<IntegrationError>> {
    let billing = EcommpayAddress {
        country: common.get_optional_billing_country(),
        region: within_limit(
            non_empty_secret(common.get_optional_billing_state()),
            "customer.billing.region",
            MAX_REGION_LENGTH,
        )?,
        city: within_limit(
            non_empty_secret(common.get_optional_billing_city()),
            "customer.billing.city",
            MAX_BILLING_CITY_LENGTH,
        )?,
        address: within_limit(
            join_lines(
                common.get_optional_billing_line1(),
                common.get_optional_billing_line2(),
            ),
            "customer.billing.address",
            MAX_BILLING_ADDRESS_LENGTH,
        )?,
        postal: within_limit(
            non_empty_secret(common.get_optional_billing_zip()),
            "customer.billing.postal",
            MAX_POSTAL_LENGTH,
        )?,
    };
    let any = billing.country.is_some()
        || billing.region.is_some()
        || billing.city.is_some()
        || billing.address.is_some()
        || billing.postal.is_some();
    Ok(any.then_some(billing))
}

/// `customer.shipping.*` (requirement row 23). `None` when no shipping field is present.
fn build_shipping(
    common: &PaymentFlowData,
) -> Result<Option<EcommpayAddress>, error_stack::Report<IntegrationError>> {
    let shipping = EcommpayAddress {
        address: within_limit(
            join_lines(
                common.get_optional_shipping_line1(),
                common.get_optional_shipping_line2(),
            ),
            "customer.shipping.address",
            MAX_SHIPPING_ADDRESS_LENGTH,
        )?,
        city: within_limit(
            non_empty_secret(common.get_optional_shipping_city()),
            "customer.shipping.city",
            MAX_SHIPPING_CITY_LENGTH,
        )?,
        country: common.get_optional_shipping_country(),
        postal: within_limit(
            non_empty_secret(common.get_optional_shipping_zip()),
            "customer.shipping.postal",
            MAX_POSTAL_LENGTH,
        )?,
        region: within_limit(
            non_empty_secret(common.get_optional_shipping_state()),
            "customer.shipping.region",
            MAX_REGION_LENGTH,
        )?,
    };
    let any = shipping.address.is_some()
        || shipping.city.is_some()
        || shipping.country.is_some()
        || shipping.postal.is_some()
        || shipping.region.is_some();
    Ok(any.then_some(shipping))
}

/// Request half of AVS (requirement row 20): sent only when both inputs are present.
fn build_avs_data(common: &PaymentFlowData) -> Option<EcommpayAvsData> {
    Some(EcommpayAvsData {
        avs_post_code: non_empty_secret(common.get_optional_billing_zip())?,
        avs_street_address: non_empty_secret(common.get_optional_billing_line1())?,
    })
}

/// `customer.*` shared by the Authorize and SetupMandate legs: every optional field is sent
/// whenever the request carries it.
fn build_customer(
    browser_info: Option<&BrowserInformation>,
    email: Option<Email>,
    common: &PaymentFlowData,
    id: String,
    ip_address: Secret<String, IpAddress>,
) -> Result<EcommpayCustomer, error_stack::Report<IntegrationError>> {
    let screen_res = browser_info.and_then(BrowserInformation::get_optional_screen_resolution);
    let browser = browser_info
        .and_then(|info| non_empty(info.user_agent.clone()))
        .filter(|agent| {
            let fits = agent.chars().count() <= MAX_BROWSER_LENGTH;
            if !fits {
                tracing::warn!(
                    field = "customer.browser",
                    "Ecommpay: user agent exceeds 512 characters; customer.browser omitted"
                );
            }
            fits
        });
    Ok(EcommpayCustomer {
        id,
        ip_address,
        email: non_empty_email(email)
            .or_else(|| non_empty_email(common.get_optional_billing_email())),
        phone: normalise_phone(common),
        screen_res,
        first_name: within_limit(
            non_empty_secret(common.get_optional_billing_first_name()),
            "customer.first_name",
            MAX_ID_LENGTH,
        )?,
        last_name: within_limit(
            non_empty_secret(common.get_optional_billing_last_name()),
            "customer.last_name",
            MAX_ID_LENGTH,
        )?,
        billing: build_billing(common)?,
        shipping: build_shipping(common)?,
        accept_header: browser_info.and_then(|info| non_empty(info.accept_header.clone())),
        browser,
        language: browser_info.and_then(browser_language),
        color_depth: browser_info.and_then(|info| info.color_depth),
        java_enabled: browser_info.and_then(|info| info.java_enabled),
        js_enabled: browser_info.and_then(|info| info.java_script_enabled),
        timezone_offset: browser_info
            .and_then(|info| info.time_zone)
            .map(|minutes| minutes.to_string()),
    })
}

/// `merchant.descriptor` (requirement row 28).
fn build_merchant<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<Option<EcommpayMerchant>, error_stack::Report<IntegrationError>> {
    let Some(descriptor) = non_empty(
        request
            .billing_descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.statement_descriptor.clone()),
    ) else {
        return Ok(None);
    };
    let length = descriptor.chars().count();
    if length > MAX_ID_LENGTH {
        return Err(invalid_format(
            "billing_descriptor.statement_descriptor",
            format!("merchant.descriptor is limited to 255 characters, got {length}"),
        ));
    }
    if descriptor
        .chars()
        .any(|c| FORBIDDEN_DESCRIPTOR_CHARS.contains(&c))
    {
        return Err(invalid_format(
            "billing_descriptor.statement_descriptor",
            "merchant.descriptor must not contain any of ! @ & ~ № { } | < > [ ]".to_string(),
        ));
    }
    Ok(Some(EcommpayMerchant { descriptor }))
}

/// `receipt_data` (requirement row 27). Each position's `amount` is the line
/// total, unit amount × quantity, computed with checked arithmetic.
fn build_receipt_data<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
    common: &PaymentFlowData,
) -> Result<Option<EcommpayReceiptData>, error_stack::Report<IntegrationError>> {
    let l2_l3 = common.l2_l3_data.as_deref();
    let order_details = common.order_details.as_ref().or_else(|| {
        l2_l3
            .and_then(|data| data.order_info.as_ref())
            .and_then(|info| info.order_details.as_ref())
    });
    let lines: &[_] = order_details.map_or(&[], Vec::as_slice);
    if lines.len() > MAX_RECEIPT_POSITIONS {
        return Err(invalid_format(
            "order_details",
            format!(
                "receipt_data.positions holds at most 300 lines, got {}; lines are never dropped \
                 silently",
                lines.len()
            ),
        ));
    }
    let positions = lines
        .iter()
        .map(|line| {
            let line_total = line
                .amount
                .get_amount_as_i64()
                .checked_mul(i64::from(line.quantity))
                .ok_or_else(|| {
                    invalid_format(
                        "order_details",
                        "order_details amount x quantity overflows".to_string(),
                    )
                })?;
            let description = non_empty(Some(line.product_name.clone()));
            let description_length = description
                .as_deref()
                .map_or(0, |text| text.chars().count());
            if description_length > MAX_ID_LENGTH {
                return Err(too_long(
                    "order_details.product_name",
                    MAX_ID_LENGTH,
                    description_length,
                ));
            }
            Ok(EcommpayReceiptPosition {
                amount: MinorUnit::new(line_total),
                quantity: line.quantity,
                description,
                tax_amount: line.total_tax_amount,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    // A zero tax is the gRPC default for "no tax" and is not sent.
    let total_tax_amount = l2_l3
        .and_then(|data| data.tax_info.as_ref())
        .and_then(|tax| tax.order_tax_amount.as_ref())
        .or(request.order_tax_amount.as_ref())
        .map(|money| money.amount)
        .filter(|amount| amount.get_amount_as_i64() > 0);
    if positions.is_empty() && total_tax_amount.is_none() {
        return Ok(None);
    }
    Ok(Some(EcommpayReceiptData {
        positions,
        total_tax_amount,
    }))
}

/// External 3DS result (requirement row 10).
fn build_authentication_data(
    data: &AuthenticationData,
) -> Result<EcommpayAuthenticationData, error_stack::Report<IntegrationError>> {
    let authentication_status = match &data.trans_status {
        Some(TransactionStatus::Success) => "Y",
        Some(TransactionStatus::NotVerified) => "A",
        Some(TransactionStatus::VerificationNotPerformed) => "U",
        Some(
            status @ (TransactionStatus::Failure
            | TransactionStatus::Rejected
            | TransactionStatus::ChallengeRequired
            | TransactionStatus::ChallengeRequiredDecoupledAuthentication
            | TransactionStatus::InformationOnly
            | TransactionStatus::SecurePaymentConfirmationRequired),
        ) => {
            return Err(invalid_format(
                "authentication_data.trans_status",
                format!(
                    "Ecommpay accepts an external 3DS result only with authentication_status Y, \
                     A or U; got {status:?}"
                ),
            ));
        }
        None => {
            return Err(missing_field(
                "authentication_data.trans_status",
                "authentication_data.authentication_status is required for an external 3DS \
                 result",
            ));
        }
    };
    let cavv = non_empty_secret(data.cavv.clone());
    let eci = non_empty(data.eci.clone());
    if matches!(authentication_status, "Y" | "A") {
        if cavv.is_none() {
            return Err(missing_field(
                "authentication_data.cavv",
                "authentication_data.cavv is required when authentication_status is Y or A",
            ));
        }
        if eci.is_none() {
            return Err(missing_field(
                "authentication_data.eci",
                "authentication_data.eci is required when authentication_status is Y or A",
            ));
        }
    }
    let xid = non_empty(data.transaction_id.clone())
        .or_else(|| non_empty(data.threeds_server_transaction_id.clone()))
        .ok_or_else(|| {
            missing_field(
                "authentication_data.transaction_id",
                "authentication_data.xid is required by Ecommpay; supply transaction_id or \
                 threeds_server_transaction_id",
            )
        })?;
    let threeds_version = data.message_version.as_ref().map(|version| {
        if version.get_major() == 2 {
            "3ds_2".to_string()
        } else {
            "3ds_1".to_string()
        }
    });
    Ok(EcommpayAuthenticationData {
        cavv,
        eci,
        ds_operation_id: non_empty(data.ds_trans_id.clone()),
        threeds_version,
        threeds_full_version: data.message_version.as_ref().map(ToString::to_string),
        xid,
        authentication_status: authentication_status.to_string(),
    })
}

/// COF registration on a customer-initiated purchase (requirement row 1, platform storage).
fn build_recurring<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Option<EcommpayRecurring> {
    (request.setup_future_usage == Some(FutureUsage::OffSession)).then(|| {
        let recurring_type = match request.mit_category {
            Some(MitCategory::Recurring | MitCategory::Subscription | MitCategory::Installment) => {
                "R"
            }
            Some(MitCategory::Unscheduled | MitCategory::Resubmission) | None => "U",
        };
        EcommpayRecurring {
            register: true,
            recurring_type: recurring_type.to_string(),
        }
    })
}

/// The 3DS completion body. `cres` (3DS 2) wins; otherwise
/// `PaRes` (+ `MD`) for 3DS 1. Keys are matched case-insensitively; values are never logged.
fn build_three_ds_result<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
    auth: &EcommpayAuthType,
    payment_id: String,
) -> Result<EcommpayThreeDsResultRequest, error_stack::Report<IntegrationError>> {
    let payload = request
        .redirect_response
        .as_ref()
        .and_then(|response| response.payload.clone())
        .map(ExposeInterface::expose);
    let pick = |name: &str| {
        payload
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|fields| {
                fields
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(name))
                    .and_then(|(_, value)| value.as_str())
            })
            .and_then(|value| non_empty(Some(value.to_string())))
            .map(Secret::new)
    };
    let (cres, pares, md) = match (pick("cres"), pick("pares")) {
        (Some(cres), _) => (Some(cres), None, None),
        (None, Some(pares)) => (None, Some(pares), pick("md")),
        (None, None) => {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: "redirect_response.payload.cres",
                    context: context(
                        Some("Forward the ACS post-back (cres, or PaRes and MD) unchanged."),
                        THREE_DS_DOC_URL,
                        "empty ACS post-back (abandoned challenge?)".to_string(),
                    ),
                }
            ));
        }
    };
    Ok(EcommpayThreeDsResultRequest {
        general: EcommpayGeneral {
            project_id: auth.project_id.clone(),
            payment_id,
            signature: None,
            merchant_callback_url: None,
        },
        cres,
        pares,
        md,
    })
}

// =============================================================================
// Acceptance response
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EcommpayAcceptanceStatus {
    Success,
    #[serde(other)]
    Unknown,
}

/// `GateSuccessResponse`: the asynchronous acceptance of every payment-processing request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayAcceptanceResponse {
    pub status: EcommpayAcceptanceStatus,
    pub request_id: String,
    pub payment_id: String,
}

/// An acceptance is never a payment result: both rows of the status map are `Pending`.
fn acceptance_status(status: EcommpayAcceptanceStatus) -> AttemptStatus {
    match status {
        EcommpayAcceptanceStatus::Success => AttemptStatus::Pending,
        EcommpayAcceptanceStatus::Unknown => AttemptStatus::Pending,
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<EcommpayAcceptanceResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EcommpayAcceptanceResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        // Identifier integrity: the acceptance echoes general.payment_id; a different value
        // would bind this attempt to someone else's payment.
        let sent = &item
            .router_data
            .resource_common_data
            .connector_request_reference_id;
        if &response.payment_id != sent {
            return Err(error_stack::report!(utils::response_deserialization_fail(
                item.http_code,
                "Ecommpay acceptance payment_id does not match the general.payment_id sent",
            )));
        }
        let status = acceptance_status(response.status);
        // The path family the payment was created on, for Capture / Void / Refund. The same
        // function picks the Authorize URL.
        //
        // Only the initial sale/auth leg may write this. On the 3DS completion leg Hyperswitch
        // re-enters Authorize with a Token payment method (the wallet is gone), so
        // `authorize_route` would fall to its `Card` arm and overwrite the stored
        // `applepay`/`googlepay` route -- sending a later capture/void/refund of a 3DS'd wallet
        // payment to `/v2/payment/card/*`. Leaving it unset keeps what the first leg stored.
        let route = authorize_route(&item.router_data.request.payment_method_data);
        let connector_metadata = (!is_three_ds_completion(&item.router_data.request))
            .then(|| authorize_feature_data(route, &response.payment_id))
            .flatten();
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data: None,
                connector_metadata,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// SetupMandate
// =============================================================================

/// `POST /v2/payment/card/account_verification` body: a zero-amount card verification that
/// registers the card for COF purchases on the platform (`recurring.register`).
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayAccountVerificationRequest<T: PaymentMethodDataTypes> {
    pub general: EcommpayGeneral,
    pub customer: EcommpayCustomer,
    pub payment: EcommpayPaymentInfo,
    pub return_url: EcommpayReturnUrl,
    /// Never sent: with acs_return_url the Gate keeps 3DS data out of the status response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acs_return_url: Option<EcommpayAcsReturnUrl>,
    pub card: EcommpayCard<T>,
    pub recurring: EcommpayRecurring,
}

/// The verification acceptance is the same `GateSuccessResponse` as every payment-processing
/// call; the alias gives the SetupMandate flow its own name in the connector macros.
pub type EcommpaySetupMandateResponse = EcommpayAcceptanceResponse;

fn setup_mandate_unimplemented_payment_method() -> error_stack::Report<IntegrationError> {
    unimplemented_payment_method(
        ACCOUNT_VERIFICATION_DOC_URL,
        "Only card account verification is implemented for Ecommpay SetupMandate",
    )
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for EcommpayAccountVerificationRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
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
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        // Payment-method arms: card only; every other arm is refused before any request is
        // built (the wallet account_verification routes exist but are not in scope).
        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => build_card(card)?,
            PaymentMethodData::Wallet(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_) => {
                return Err(setup_mandate_unimplemented_payment_method())
            }
        };

        // customer.id is required, and every later COF debit must reuse it.
        let customer_id = required_customer_id(
            common.customer_id.as_ref().or(request.customer_id.as_ref()),
            "Ecommpay requires customer.id on account verification; the same id must be \
                 sent on every later COF purchase",
        )?;

        // customer.ip_address is required.
        let ip_address = request
            .browser_info
            .as_ref()
            .and_then(|info| info.ip_address)
            .map(|ip| Secret::<String, IpAddress>::new(ip.to_string()))
            .ok_or_else(|| {
                missing_field(
                    "browser_info.ip_address",
                    "Ecommpay requires customer.ip_address on account verification",
                )
            })?;

        // 3-D Secure may interpose on the verification (the sandbox forces
        // it). The hosted acs page posts PaRes+MD to term_url = return_url.success, which is
        // the complete-authorize URL (Authorize re-entry -> 3ds_result at amount 0) when given;
        // router_return_url is required for return_url.decline.
        let complete_authorize_url = non_empty(request.complete_authorize_url.clone());
        let router_return_url = non_empty(request.router_return_url.clone()).ok_or_else(|| {
            missing_field(
                "return_url",
                "Ecommpay may require 3-D Secure on account verification; return_url.success / \
                 decline are needed to bring the customer back",
            )
        })?;

        // general.payment_id is 1-255 characters.
        let payment_id = payment_id(common)?;
        let auth = EcommpayAuthType::try_from(&router_data.connector_config)?;

        // payment.amount must be 0 on account verification (AccountVerificationPaymentInfo:
        // min 0, max 0). A requested non-zero amount cannot be charged on this route, so it is
        // refused rather than quietly replaced with 0 and reported as a success.
        if let Some(requested) = request
            .amount
            .as_ref()
            .map(|money| money.amount.get_amount_as_i64())
            .filter(|minor_amount| *minor_amount != 0)
        {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "A non-zero amount on SetupMandate".to_string(),
                connector: CONNECTOR_NAME,
                context: context(
                    Some(
                        "Use SetupMandate with a zero amount to verify the account, then charge \
                         the mandate with RepeatPayment.",
                    ),
                    ACCOUNT_VERIFICATION_DOC_URL,
                    format!(
                        "Ecommpay account verification is always zero-amount \
                         (AccountVerificationPaymentInfo.amount: min 0, max 0); the requested \
                         {requested} minor units cannot be charged on this route"
                    ),
                ),
            }));
        }

        let customer = build_customer(
            request.browser_info.as_ref(),
            request.email.clone(),
            common,
            customer_id,
            ip_address,
        )?;

        // COF type from the intended MIT category (same mapping as a COF-registering purchase).
        let recurring_type = match request.mit_category {
            Some(MitCategory::Recurring | MitCategory::Subscription | MitCategory::Installment) => {
                "R"
            }
            Some(MitCategory::Unscheduled | MitCategory::Resubmission) | None => "U",
        };

        let mut body = Self {
            general: EcommpayGeneral {
                project_id: auth.project_id.clone(),
                payment_id,
                signature: None,
                merchant_callback_url: non_empty(request.webhook_url.clone()),
            },
            customer,
            payment: EcommpayPaymentInfo {
                amount: MinorUnit::zero(),
                currency: request.currency,
                description: description(common),
            },
            return_url: EcommpayReturnUrl {
                success: complete_authorize_url.unwrap_or_else(|| router_return_url.clone()),
                decline: router_return_url,
            },
            acs_return_url: None,
            card,
            recurring: EcommpayRecurring {
                register: true,
                recurring_type: recurring_type.to_string(),
            },
        };
        body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
        Ok(body)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<EcommpaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EcommpaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        // Identifier integrity: the acceptance echoes general.payment_id.
        let sent = &item
            .router_data
            .resource_common_data
            .connector_request_reference_id;
        if &response.payment_id != sent {
            return Err(error_stack::report!(utils::response_deserialization_fail(
                item.http_code,
                "Ecommpay acceptance payment_id does not match the general.payment_id sent",
            )));
        }
        // An acceptance is never the verification result: the status is Pending, and the
        // mandate id (`recurring.id`) is only returned by the status request / callback, which
        // PSync and the webhook surface. Never Charged here.
        let status = acceptance_status(response.status);
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// RepeatPayment
// =============================================================================

const COF_PURCHASE_DOC_URL: &str = "https://developers.ecommpay.com/en/en_Gate__cof_purchase.html";
/// `card.scheme_id` matches `^[a-zA-Z0-9 ]{1,15}$`.
const MAX_SCHEME_ID_LENGTH: usize = 15;
/// `card.transaction_link_id` (Mastercard TLID) is exactly 22 characters.
const TRANSACTION_LINK_ID_LENGTH: usize = 22;

/// The Gate call a RepeatPayment is sent as. `get_url` and the body builder both derive it
/// from [`repeat_payment_operation`], so the endpoint and the body cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcommpayRepeatPaymentOperation {
    /// Platform-stored COF debit by `recurring.id`.
    Recurring,
    /// Merchant-stored COF debit (NTID-based MIT), one-step.
    StoredCardSale,
    /// Merchant-stored COF debit (NTID-based MIT), two-step (hold, then Capture).
    StoredCardAuth,
}

impl EcommpayRepeatPaymentOperation {
    pub fn path(self) -> &'static str {
        match self {
            Self::Recurring => "/v2/payment/card/recurring",
            Self::StoredCardSale => "/v2/payment/card/sale",
            Self::StoredCardAuth => "/v2/payment/card/auth",
        }
    }
}

fn repeat_capture_not_supported(detail: String) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::CaptureMethodNotSupported {
        context: context(
            Some("Use capture_method AUTOMATIC for a COF debit by recurring.id."),
            COF_PURCHASE_DOC_URL,
            detail,
        ),
    })
}

fn network_token_mit_not_supported() -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: "Network-token merchant-initiated payments (NetworkTokenWithNTI)".to_string(),
        connector: CONNECTOR_NAME,
        context: context(
            Some("Charge the mandate by connector_mandate_id (recurring.id) or by card + NTID."),
            COF_PURCHASE_DOC_URL,
            "Ecommpay RepeatPayment is implemented for platform-stored COF (recurring.id) and \
             merchant-stored card COF (scheme_id) only"
                .to_string(),
        ),
    })
}

/// Selects the Gate call, refusing what Ecommpay cannot do before any request is built:
/// multiple / scheduled capture, manual capture of a `recurring.id` debit
/// (`/v2/payment/card/recurring` has no hold variant) and the network-token MIT arm.
pub fn repeat_payment_operation<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
) -> Result<EcommpayRepeatPaymentOperation, error_stack::Report<IntegrationError>> {
    let manual = match request.capture_method {
        Some(CaptureMethod::Manual) => true,
        Some(CaptureMethod::Automatic | CaptureMethod::SequentialAutomatic) | None => false,
        Some(method @ (CaptureMethod::ManualMultiple | CaptureMethod::Scheduled)) => {
            return Err(repeat_capture_not_supported(format!(
                "Ecommpay COF debits are one-step (sale/recurring) or auth + capture only; \
                 capture_method {method} is not supported"
            )));
        }
    };
    match &request.mandate_reference {
        MandateReferenceId::ConnectorMandateId(_) => {
            if manual {
                return Err(repeat_capture_not_supported(
                    "POST /v2/payment/card/recurring always debits; Ecommpay documents no \
                     authorisation-only variant of a platform-stored COF debit"
                        .to_string(),
                ));
            }
            Ok(EcommpayRepeatPaymentOperation::Recurring)
        }
        MandateReferenceId::NetworkMandateId(_) => Ok(if manual {
            EcommpayRepeatPaymentOperation::StoredCardAuth
        } else {
            EcommpayRepeatPaymentOperation::StoredCardSale
        }),
        MandateReferenceId::NetworkTokenWithNTI(_) => Err(network_token_mit_not_supported()),
    }
}

/// `recurring` object of a platform-stored COF debit.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayRecurringId {
    /// The debiting-series id returned for the COF registration (an integer on the wire).
    pub id: u64,
}

/// `POST /v2/payment/card/recurring` body (platform-stored COF debit).
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayRecurringRequest {
    pub general: EcommpayGeneral,
    pub customer: EcommpayCustomer,
    pub payment: EcommpayPaymentInfo,
    pub recurring: EcommpayRecurringId,
}

/// RepeatPayment body: a `recurring.id` debit, or a merchant-stored card debit on
/// `sale`/`auth` (`card.stored_card_type` + `card.scheme_id`).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EcommpayRepeatPaymentRequest<T: PaymentMethodDataTypes> {
    Recurring(Box<EcommpayRecurringRequest>),
    StoredCard(Box<EcommpayPaymentRequest<T>>),
}

/// The COF debit acceptance is the same `GateSuccessResponse` as every payment-processing
/// call; the alias gives the RepeatPayment flow its own name in the connector macros.
pub type EcommpayRepeatPaymentResponse = EcommpayAcceptanceResponse;

/// `recurring.id` is the connector mandate id, an integer.
fn recurring_id(
    connector_mandate_id: Option<String>,
) -> Result<u64, error_stack::Report<IntegrationError>> {
    let id = non_empty(connector_mandate_id).ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingConnectorMandateID {
            context: context(
                Some(
                    "Pass the recurring.id of the COF registration (SetupMandate status or \
                     callback) as connector_mandate_id.",
                ),
                COF_PURCHASE_DOC_URL,
                "Ecommpay requires recurring.id on a platform-stored COF debit".to_string(),
            ),
        })
    })?;
    id.parse::<u64>().map_err(|_| {
        invalid_format(
            "connector_mandate_id",
            "recurring.id is an integer; connector_mandate_id is not a non-negative integer"
                .to_string(),
        )
    })
}

/// `card` of a merchant-stored COF debit.
fn stored_card<T: PaymentMethodDataTypes>(
    request: &RepeatPaymentData<T>,
    reference: &NetworkMandateIdRef,
    operation: EcommpayRepeatPaymentOperation,
) -> Result<EcommpayStoredCard<T>, error_stack::Report<IntegrationError>> {
    // The merchant stores the card, so the debit carries it.
    let (pan, exp_month, exp_year, card_holder) = match &request.payment_method_data {
        PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
            card.card_number.clone(),
            card.card_exp_month.clone(),
            card.get_expiry_year_4_digit(),
            card.card_holder_name.clone(),
        ),
        PaymentMethodData::Card(card) => (
            card.card_number.clone(),
            card.card_exp_month.clone(),
            card.get_expiry_year_4_digit(),
            card.card_holder_name.clone(),
        ),
        PaymentMethodData::Wallet(_)
        | PaymentMethodData::CardWithNoCvc(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::CardRedirect(_)
        | PaymentMethodData::PayLater(_)
        | PaymentMethodData::BankRedirect(_)
        | PaymentMethodData::BankDebit(_)
        | PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::Crypto(_)
        | PaymentMethodData::MandatePayment
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::PaymentMethodToken(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::NetworkToken(_)
        | PaymentMethodData::MobilePayment(_) => {
            return Err(missing_field(
                "payment_method_data.card",
                "A merchant-stored COF debit (network transaction id) needs the stored card: \
                 card.pan, card.year and card.month",
            ));
        }
    };

    // An absent NTID is refused, never dropped; values are never logged.
    let scheme_id = reference.network_transaction_id.as_str();
    if scheme_id.trim().is_empty() {
        return Err(missing_field(
            "network_transaction_id",
            "Ecommpay requires card.scheme_id (the network transaction id of the COF \
             registration) on a merchant-stored COF debit",
        ));
    }
    if scheme_id.chars().count() > MAX_SCHEME_ID_LENGTH
        || !scheme_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ')
    {
        return Err(invalid_format(
            "network_transaction_id",
            "card.scheme_id must match ^[a-zA-Z0-9 ]{1,15}$".to_string(),
        ));
    }
    let transaction_link_id = non_empty(reference.transaction_link_id.clone());
    if let Some(link_id) = &transaction_link_id {
        let length = link_id.chars().count();
        if length != TRANSACTION_LINK_ID_LENGTH {
            return Err(invalid_format(
                "network_transaction_id",
                format!(
                    "card.transaction_link_id (Mastercard TLID) must be exactly 22 characters, \
                     got {length}"
                ),
            ));
        }
    }

    let year = utils::parse_expiry_year_four_digit(exp_year.peek(), "card_exp_year")?;
    let month = utils::parse_expiry_month(exp_month.peek(), "card_exp_month")?;
    let card_holder = within_limit(
        non_empty_secret(card_holder),
        "card.card_holder",
        MAX_ID_LENGTH,
    )?;

    // `/v2/payment/card/auth` accepts on-demand (4) only; a one-step debit is regular (6) for
    // a scheduled series and on-demand (4) otherwise.
    let stored_card_type = match operation {
        EcommpayRepeatPaymentOperation::StoredCardAuth => EcommpayStoredCardType::OnDemand,
        EcommpayRepeatPaymentOperation::StoredCardSale
        | EcommpayRepeatPaymentOperation::Recurring => match request.mit_category {
            Some(MitCategory::Recurring | MitCategory::Subscription | MitCategory::Installment) => {
                EcommpayStoredCardType::Regular
            }
            Some(MitCategory::Unscheduled | MitCategory::Resubmission) | None => {
                EcommpayStoredCardType::OnDemand
            }
        },
    };

    Ok(EcommpayStoredCard {
        pan,
        year: Secret::new(year),
        month: Secret::new(month),
        card_holder,
        stored_card_type,
        scheme_id: Secret::new(scheme_id.to_string()),
        transaction_link_id: transaction_link_id.map(Secret::new),
    })
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for EcommpayRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let common = &router_data.resource_common_data;

        // Refuses what Ecommpay cannot do: multiple / scheduled capture, manual capture of
        // a recurring.id debit, and the network-token MIT arm.
        let operation = repeat_payment_operation(request)?;

        // PaymentInfoRecurring amount >= 1 minor unit.
        let minor_amount = request.amount.amount.get_amount_as_i64();
        if minor_amount <= 0 {
            return Err(invalid_format(
                "amount",
                format!(
                    "payment.amount must be a positive integer in minor units, got {minor_amount}"
                ),
            ));
        }

        // customer.id must be the id the card was registered under.
        let customer_id = required_customer_id(
            common.customer_id.as_ref().or(request
                .customer
                .as_ref()
                .and_then(|customer| customer.customer_id.as_ref())),
            "Ecommpay requires customer.id on a COF debit, equal to the customer id of the \
                 COF registration",
        )?;

        // customer.ip_address is required.
        let ip_address = request
            .browser_info
            .as_ref()
            .and_then(|info| info.ip_address)
            .map(|ip| Secret::<String, IpAddress>::new(ip.to_string()))
            .ok_or_else(|| {
                missing_field(
                    "browser_info.ip_address",
                    "Ecommpay requires customer.ip_address on a COF debit",
                )
            })?;

        // Arm-specific guards, still before any request is built.
        enum Instrument<T: PaymentMethodDataTypes> {
            Recurring(u64),
            StoredCard(EcommpayStoredCard<T>),
        }
        let instrument = match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(reference) => {
                Instrument::Recurring(recurring_id(reference.get_connector_mandate_id())?)
            }
            MandateReferenceId::NetworkMandateId(reference) => {
                Instrument::StoredCard(stored_card(request, reference, operation)?)
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(network_token_mit_not_supported())
            }
        };

        // A new general.payment_id per debit; recurring.id is the series, not the payment.
        let payment_id = payment_id(common)?;
        let auth = EcommpayAuthType::try_from(&router_data.connector_config)?;
        let amount = item
            .connector
            .amount_converter
            .convert(&request.amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: context(
                    None,
                    COF_PURCHASE_DOC_URL,
                    "Failed to convert the amount into Ecommpay minor units".to_string(),
                ),
            })?;

        let general = EcommpayGeneral {
            project_id: auth.project_id.clone(),
            payment_id,
            signature: None,
            merchant_callback_url: non_empty(request.webhook_url.clone()),
        };
        // RepeatPayment is merchant-initiated: no browser, address or contact data is sent.
        let customer = EcommpayCustomer {
            id: customer_id,
            ip_address,
            ..Default::default()
        };
        let payment = EcommpayPaymentInfo {
            amount,
            currency: request.amount.currency,
            description: description(common),
        };

        match instrument {
            Instrument::Recurring(id) => {
                let mut body = EcommpayRecurringRequest {
                    general,
                    customer,
                    payment,
                    recurring: EcommpayRecurringId { id },
                };
                body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
                Ok(Self::Recurring(Box::new(body)))
            }
            Instrument::StoredCard(card) => {
                let mut body = EcommpayPaymentRequest {
                    general,
                    customer,
                    payment,
                    return_url: None,
                    acs_return_url: None,
                    card: Some(EcommpayPaymentCard::StoredCard(card)),
                    etoken: None,
                    avs_data: None,
                    merchant: None,
                    receipt_data: None,
                    authentication_data: None,
                    recurring: None,
                };
                body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
                Ok(Self::StoredCard(Box::new(body)))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<EcommpayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EcommpayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        // Identifier integrity: the acceptance echoes the new general.payment_id of this debit.
        let sent = &item
            .router_data
            .resource_common_data
            .connector_request_reference_id;
        if &response.payment_id != sent {
            return Err(error_stack::report!(utils::response_deserialization_fail(
                item.http_code,
                "Ecommpay acceptance payment_id does not match the general.payment_id sent",
            )));
        }
        // An acceptance is never the debit result: Pending until PSync or the callback reports
        // the operation outcome.
        let status = acceptance_status(response.status);
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Capture
// =============================================================================

/// `payment` object of a capture: the amount to debit from the hold and its currency.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayCapturePayment {
    pub amount: MinorUnit,
    pub currency: Currency,
}

/// The capture acceptance is the same `GateSuccessResponse` as every payment-processing call;
/// the alias gives the Capture flow its own name in the connector macros.
pub type EcommpayCaptureResponse = EcommpayAcceptanceResponse;

/// `POST /v2/payment/{card|applepay|googlepay}/capture` body.
///
/// `payment.amount` is always sent: the Gate treats an amount equal to the hold as a
/// full capture and a smaller one as a partial capture, so one body shape covers both and no
/// full-vs-partial routing decision is made here.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayCaptureRequest {
    pub general: EcommpayGeneral,
    pub payment: EcommpayCapturePayment,
}

/// The `general.payment_id` of the authorised payment, which Authorize returned as the
/// connector transaction id. A capture is addressed by the original payment, never by an
/// operation or request id.
fn capture_payment_id(
    connector_transaction_id: &ResponseId,
) -> Result<String, error_stack::Report<IntegrationError>> {
    match connector_transaction_id {
        ResponseId::ConnectorTransactionId(id) => non_empty(Some(id.clone())),
        ResponseId::EncodedData(_) | ResponseId::NoResponseId => None,
    }
    .ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingConnectorTransactionID {
            context: context(
                Some(
                    "Pass the connector_transaction_id returned by Authorize (the Ecommpay \
                     general.payment_id).",
                ),
                CAPTURE_DOC_URL,
                "Ecommpay capture addresses the authorised payment by general.payment_id; no \
                 connector transaction id was supplied"
                    .to_string(),
            ),
        })
    })
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for EcommpayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        // The hold being captured.
        let payment_id = capture_payment_id(&request.connector_transaction_id)?;
        // The Gate accepts 1..=10^13 minor units; a zero or negative capture is
        // refused here instead of being sent.
        let amount_to_capture = request.amount_to_capture.amount;
        if amount_to_capture <= MinorUnit::zero() {
            return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "amount_to_capture",
                context: context(
                    Some("Send a positive amount_to_capture in minor units."),
                    CAPTURE_DOC_URL,
                    format!(
                        "Ecommpay payment.amount must be a positive integer in minor units; \
                         received {}",
                        amount_to_capture.get_amount_as_i64()
                    ),
                ),
            }));
        }
        let currency = request.amount_to_capture.currency;
        let amount = item
            .connector
            .amount_converter
            .convert(&request.amount_to_capture)
            .change_context(IntegrationError::AmountConversionFailed {
                context: context(
                    None,
                    CAPTURE_DOC_URL,
                    "Failed to convert the capture amount into Ecommpay minor units".to_string(),
                ),
            })?;
        let auth = EcommpayAuthType::try_from(&item.router_data.connector_config)?;
        let mut body = Self {
            general: EcommpayGeneral {
                project_id: auth.project_id.clone(),
                payment_id,
                signature: None,
                merchant_callback_url: None,
            },
            payment: EcommpayCapturePayment { amount, currency },
        };
        body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
        Ok(body)
    }
}

impl TryFrom<ResponseRouterData<EcommpayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EcommpayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        // Identifier integrity: the acceptance echoes the general.payment_id the capture was
        // sent for; a different value would bind this capture to another payment.
        let sent = match &item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => Some(id.as_str()),
            ResponseId::EncodedData(_) | ResponseId::NoResponseId => None,
        };
        if sent != Some(response.payment_id.as_str()) {
            return Err(error_stack::report!(utils::response_deserialization_fail(
                item.http_code,
                "Ecommpay capture acceptance payment_id does not match the general.payment_id sent",
            )));
        }
        // An acceptance only: the capture result (operation capture success/decline) is read by
        // PSync or delivered by callback, so the attempt stays Pending here.
        let status = acceptance_status(response.status);
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Void
// =============================================================================

/// The cancel acceptance is the same `GateSuccessResponse` as every payment-processing call;
/// the alias gives the Void flow its own name in the connector macros.
pub type EcommpayVoidResponse = EcommpayAcceptanceResponse;

/// `POST /v2/payment/{card|applepay|googlepay}/cancel` body: `general` only, which releases
/// the whole hold.
///
/// `payment.amount` is never sent: on a cancel it does not void the payment, it
/// creates a decremental operation that only lowers the hold.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayVoidRequest {
    pub general: EcommpayGeneral,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for EcommpayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        // The hold being released, addressed by the original payment's
        // general.payment_id (the connector transaction id Authorize returned).
        let payment_id =
            non_empty(Some(request.connector_transaction_id.clone())).ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingConnectorTransactionID {
                    context: context(
                        Some(
                            "Pass the connector_transaction_id returned by Authorize (the \
                             Ecommpay general.payment_id).",
                        ),
                        CANCEL_DOC_URL,
                        "Ecommpay cancel addresses the authorised payment by \
                         general.payment_id; no connector transaction id was supplied"
                            .to_string(),
                    ),
                })
            })?;
        // A supplied amount is not sent — the cancel always releases the full hold.
        if request.amount.is_some() {
            tracing::warn!(
                connector = CONNECTOR_NAME,
                "Ecommpay void ignores the requested amount: the cancel releases the full hold \
                 (payment.amount would create a decremental operation, not a void)"
            );
        }
        let auth = EcommpayAuthType::try_from(&item.router_data.connector_config)?;
        let mut body = Self {
            general: EcommpayGeneral {
                project_id: auth.project_id.clone(),
                payment_id,
                signature: None,
                merchant_callback_url: None,
            },
        };
        body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
        Ok(body)
    }
}

impl TryFrom<ResponseRouterData<EcommpayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<EcommpayVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        // Identifier integrity: the acceptance echoes the general.payment_id the cancel was
        // sent for; a different value would bind this void to another payment.
        if response.payment_id != item.router_data.request.connector_transaction_id {
            return Err(error_stack::report!(utils::response_deserialization_fail(
                item.http_code,
                "Ecommpay cancel acceptance payment_id does not match the general.payment_id sent",
            )));
        }
        // An acceptance only, never Voided: the cancel result (payment `canceled`, or a
        // declined cancel leaving it `awaiting capture`) is read by PSync or delivered by
        // callback, so the attempt stays Pending here.
        let status = acceptance_status(response.status);
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Refund
// =============================================================================

/// The refund acceptance is the same `GateSuccessResponse` as every payment-processing call;
/// the alias gives the Refund flow its own name in the connector macros.
pub type EcommpayRefundResponse = EcommpayAcceptanceResponse;

/// `payment` object of a refund.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayRefundPayment {
    pub description: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_refund_id: Option<String>,
}

/// `POST /v2/payment/{card|applepay|googlepay}/refund` body.
///
/// `general.payment_id` is the **original purchase**, never a refund id. `payment.amount` is
/// always sent: the Gate treats an amount equal to the refundable balance as a full refund and a
/// smaller one as a partial refund, so one body shape covers both, and each partial refund is
/// its own operation with its own `request_id`.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayRefundRequest {
    pub general: EcommpayGeneral,
    pub payment: EcommpayRefundPayment,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for EcommpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        // The purchase being refunded, addressed by its general.payment_id (the
        // connector transaction id Authorize returned).
        let payment_id =
            non_empty(Some(request.connector_transaction_id.clone())).ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingConnectorTransactionID {
                    context: context(
                        Some(
                            "Pass the connector_transaction_id returned by Authorize (the \
                             Ecommpay general.payment_id of the original purchase).",
                        ),
                        REFUND_DOC_URL,
                        "Ecommpay refund addresses the original purchase by general.payment_id; \
                         no connector transaction id was supplied"
                            .to_string(),
                    ),
                })
            })?;
        // A zero or negative refund is refused here instead of being sent.
        let refund_amount = request.refund_amount.amount;
        if refund_amount <= MinorUnit::zero() {
            return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "refund_amount",
                context: context(
                    Some("Send a positive refund_amount in minor units."),
                    REFUND_DOC_URL,
                    format!(
                        "Ecommpay payment.amount must be a positive integer in minor units; \
                         received {}",
                        refund_amount.get_amount_as_i64()
                    ),
                ),
            }));
        }
        // payment.merchant_refund_id is 1-255 characters; never truncated.
        let merchant_refund_id = non_empty(Some(request.refund_id.clone()));
        if let Some(refund_id) = &merchant_refund_id {
            let length = refund_id.chars().count();
            if length > MAX_ID_LENGTH {
                return Err(too_long("merchant_refund_id", MAX_ID_LENGTH, length));
            }
        }
        // payment.description is required by the Gate (RefundPaymentInfo): without it the sandbox
        // accepts the call but never registers the refund. Hyperswitch `reason` is optional, so
        // omitting it must not fail the refund either -- fall back to a fixed label, which is a
        // description of the operation rather than invented transaction data. A reason over the
        // 255-character limit is still refused rather than truncated.
        let description =
            non_empty(request.reason.clone()).unwrap_or_else(|| REFUND_DESCRIPTION.to_string());
        let description_length = description.chars().count();
        if description_length > MAX_ID_LENGTH {
            return Err(too_long("reason", MAX_ID_LENGTH, description_length));
        }
        let currency = request.refund_amount.currency;
        let amount = item
            .connector
            .amount_converter
            .convert(&request.refund_amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: context(
                    None,
                    REFUND_DOC_URL,
                    "Failed to convert the refund amount into Ecommpay minor units".to_string(),
                ),
            })?;
        let auth = EcommpayAuthType::try_from(&item.router_data.connector_config)?;
        let mut body = Self {
            general: EcommpayGeneral {
                project_id: auth.project_id.clone(),
                payment_id,
                signature: None,
                merchant_callback_url: None,
            },
            payment: EcommpayRefundPayment {
                description,
                amount,
                currency,
                merchant_refund_id,
            },
        };
        body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
        Ok(body)
    }
}

/// A refund acceptance is never a refund result: both rows of the status map are `Pending`.
fn refund_acceptance_status(status: EcommpayAcceptanceStatus) -> RefundStatus {
    match status {
        EcommpayAcceptanceStatus::Success => RefundStatus::Pending,
        EcommpayAcceptanceStatus::Unknown => RefundStatus::Pending,
    }
}

impl TryFrom<ResponseRouterData<EcommpayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EcommpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        // Identifier integrity: the acceptance echoes the general.payment_id the refund was
        // sent for; a different value would bind this refund to another payment.
        if response.payment_id != item.router_data.request.connector_transaction_id {
            return Err(error_stack::report!(utils::response_deserialization_fail(
                item.http_code,
                "Ecommpay refund acceptance payment_id does not match the general.payment_id sent",
            )));
        }
        // An acceptance only: the refund result (operation refund/reversal success or decline)
        // is read by RSync or delivered by callback. The refund is identified by this
        // request_id, which RSync matches against operations[].request_id.
        let refund_status = refund_acceptance_status(response.status);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.request_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Error body
// =============================================================================

/// "Clients must accept error codes as either strings or numbers."
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EcommpayCode {
    Text(String),
    Number(i64),
}

impl fmt::Display for EcommpayCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(code) => write!(f, "{code}"),
            Self::Number(code) => write!(f, "{code}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayErrorItem {
    pub code: Option<EcommpayCode>,
    pub message: Option<String>,
    pub field: Option<String>,
    pub constraint: Option<String>,
}

impl EcommpayErrorItem {
    fn render(&self) -> String {
        let message = self.message.as_deref().unwrap_or(NO_ERROR_MESSAGE);
        let code = self
            .code
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        match self.field.as_deref() {
            Some(field) => format!("{field}: {message} ({code})"),
            None => format!("{message} ({code})"),
        }
    }
}

/// `ErrorItems` (400 with `errors[]`) or the basic `ErrorResponse` (`status`, `code`,
/// `message`). `Items` is tried first because every field of `Basic` is optional.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EcommpayErrorResponse {
    Items {
        errors: Vec<EcommpayErrorItem>,
    },
    Basic {
        status: Option<String>,
        code: Option<EcommpayCode>,
        message: Option<String>,
    },
}

/// `(code, message, reason)` with no error item dropped.
pub struct EcommpayErrorDetails {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

impl EcommpayErrorResponse {
    pub fn details(&self) -> EcommpayErrorDetails {
        match self {
            Self::Basic { code, message, .. } => EcommpayErrorDetails {
                code: code
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: message.clone(),
            },
            Self::Items { errors } => {
                let rendered = errors
                    .iter()
                    .map(EcommpayErrorItem::render)
                    .collect::<Vec<_>>()
                    .join("; ");
                let first = errors.first();
                let code = first
                    .and_then(|item| item.code.as_ref())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string());
                let message = if errors.len() > 1 {
                    rendered.clone()
                } else {
                    first
                        .and_then(|item| item.message.clone())
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
                };
                EcommpayErrorDetails {
                    code,
                    message,
                    reason: (!rendered.is_empty()).then_some(rendered),
                }
            }
        }
    }
}

// =============================================================================
// Status response, callbacks and status mapping
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcommpayPaymentStatus {
    #[serde(rename = "processing")]
    Processing,
    #[serde(rename = "awaiting 3ds result")]
    Awaiting3dsResult,
    #[serde(rename = "awaiting redirect result")]
    AwaitingRedirectResult,
    #[serde(rename = "awaiting customer action")]
    AwaitingCustomerAction,
    #[serde(rename = "awaiting merchant auth")]
    AwaitingMerchantAuth,
    #[serde(rename = "awaiting clarification")]
    AwaitingClarification,
    #[serde(rename = "awaiting customer")]
    AwaitingCustomer,
    #[serde(rename = "awaiting capture")]
    AwaitingCapture,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "decline")]
    Decline,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "partially reversed")]
    PartiallyReversed,
    #[serde(rename = "reversed")]
    Reversed,
    #[serde(rename = "partially refunded")]
    PartiallyRefunded,
    #[serde(rename = "refunded")]
    Refunded,
    #[serde(rename = "scheduled recurring processing")]
    ScheduledRecurringProcessing,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcommpayOperationStatus {
    #[serde(rename = "processing")]
    Processing,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "decline")]
    Decline,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "awaiting 3ds result")]
    Awaiting3dsResult,
    #[serde(rename = "awaiting redirect result")]
    AwaitingRedirectResult,
    #[serde(rename = "awaiting clarification")]
    AwaitingClarification,
    #[serde(rename = "awaiting customer action")]
    AwaitingCustomerAction,
    #[serde(rename = "awaiting merchant auth")]
    AwaitingMerchantAuth,
    #[serde(rename = "awaiting customer")]
    AwaitingCustomer,
    #[serde(rename = "awaiting capture")]
    AwaitingCapture,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcommpayOperationType {
    #[serde(rename = "sale")]
    Sale,
    #[serde(rename = "auth")]
    Auth,
    #[serde(rename = "capture")]
    Capture,
    #[serde(rename = "cancel")]
    Cancel,
    #[serde(rename = "refund")]
    Refund,
    #[serde(rename = "reversal")]
    Reversal,
    #[serde(rename = "recurring")]
    Recurring,
    #[serde(rename = "account verification")]
    AccountVerification,
    #[serde(rename = "incremental")]
    Incremental,
    #[serde(rename = "payment confirmation")]
    PaymentConfirmation,
    #[serde(rename = "recurring_cancel")]
    RecurringCancel,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcommpayPaymentType {
    #[serde(rename = "purchase")]
    Purchase,
    #[serde(rename = "recurring")]
    Recurring,
    #[serde(rename = "account_verification")]
    AccountVerification,
    #[serde(rename = "payout")]
    Payout,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpaySum {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayPaymentDetails {
    /// Absent on an unregistered payment (`payment.status` `error`, code 3061).
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub payment_type: Option<EcommpayPaymentType>,
    pub status: EcommpayPaymentStatus,
    pub method: Option<String>,
    pub sum: Option<EcommpaySum>,
    pub scheme_id: Option<Secret<String>>,
    pub transaction_link_id: Option<Secret<String>>,
    pub merchant_refund_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayProvider {
    pub auth_code: Option<String>,
    pub result_code: Option<EcommpayCode>,
    pub result_message: Option<String>,
    pub payment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayOperation {
    pub id: u64,
    #[serde(rename = "type")]
    pub operation_type: EcommpayOperationType,
    pub status: EcommpayOperationStatus,
    pub request_id: Option<String>,
    pub code: Option<EcommpayCode>,
    pub message: Option<String>,
    pub eci: Option<String>,
    pub provider: Option<EcommpayProvider>,
    pub sum_initial: Option<EcommpaySum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayAccount {
    pub number: Option<Secret<String>>,
    pub token: Option<Secret<String>>,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
    pub card_holder: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayRecurringInfo {
    pub id: u64,
    pub currency: Option<Currency>,
    /// Expiry date of the stored credential's card.
    pub valid_thru: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayAcs {
    pub acs_url: String,
    pub pa_req: Secret<String>,
    pub md: Secret<String>,
    pub term_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayThreeDsRedirect {
    pub url: String,
    pub params: HashMap<String, Secret<String>>,
}

/// `threeds2` of a status response. Ecommpay also returns `threeds2.iframe` for an
/// iframe-only 3DS state, which is not deserialised: UCS has no iframe to hand back, and the
/// state is recognised by `redirect` being absent (see `status_redirect_form`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayThreeDs2 {
    pub redirect: Option<EcommpayThreeDsRedirect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayRedirectData {
    pub url: String,
    pub method: Option<String>,
    pub body: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayAvsResult {
    pub avs_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayFailedRequest {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub request_type: Option<String>,
    pub errors: Option<Vec<EcommpayErrorItem>>,
}

/// `POST /v2/payment/status` response and the callback body. Status responses carry
/// `operations[]`; callbacks carry a single `operation`. Both are modelled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcommpayStatusResponse {
    pub payment: EcommpayPaymentDetails,
    pub operations: Option<Vec<EcommpayOperation>>,
    pub operation: Option<EcommpayOperation>,
    pub account: Option<EcommpayAccount>,
    pub recurring: Option<EcommpayRecurringInfo>,
    pub acs: Option<EcommpayAcs>,
    pub threeds2: Option<EcommpayThreeDs2>,
    pub redirect_data: Option<EcommpayRedirectData>,
    pub avs_data: Option<EcommpayAvsResult>,
    pub last_failed_request: Option<EcommpayFailedRequest>,
    pub errors: Option<Vec<EcommpayErrorItem>>,
    pub signature: Secret<String>,
}

/// The raw status body, kept as JSON until its signature has been verified.
/// `Serialize` (connector event log) and `Debug` (the shared macro's `response from connector` log
/// line) both mask card/holder/token data.
#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct EcommpayRawStatusResponse(pub Value);

const STATUS_LOG_MASK: &str = "*** masked ***";

/// Keys of the status/callback body that may be logged in clear; every other key is masked.
///
/// This is an allowlist on purpose. A denylist logs in clear whatever it fails to name, which
/// over this body would mean `acs.pa_req`, `acs.md`, `threeds2.redirect.params`,
/// `threeds2.iframe`, `redirect_data.body` and any field Ecommpay adds later. Defaulting to
/// masked means a new sensitive field cannot leak by simply not being listed here.
///
/// The first group is containers, which are walked into rather than kept; the second is the
/// non-sensitive leaves worth having in a log.
const STATUS_LOG_CLEAR_KEYS: &[&str] = &[
    "payment",
    "operation",
    "operations",
    "provider",
    "sum",
    "sum_initial",
    "account",
    "recurring",
    "acs",
    "threeds2",
    "redirect",
    "redirect_data",
    "avs_data",
    "last_failed_request",
    "errors",
    "id",
    "type",
    "status",
    "method",
    "amount",
    "currency",
    "merchant_refund_id",
    "request_id",
    "code",
    "message",
    "eci",
    "auth_code",
    "result_code",
    "result_message",
    "payment_id",
    "field",
    "constraint",
    "acs_url",
    "term_url",
    "url",
    "avs_result",
];

fn mask_status_log_value(value: &mut Value) {
    match value {
        Value::Object(map) => map.iter_mut().for_each(|(key, inner)| {
            if STATUS_LOG_CLEAR_KEYS.contains(&key.as_str()) {
                mask_status_log_value(inner);
            } else if !inner.is_null() {
                *inner = Value::String(STATUS_LOG_MASK.to_string());
            }
        }),
        Value::Array(items) => items.iter_mut().for_each(mask_status_log_value),
        _ => {}
    }
}

impl fmt::Debug for EcommpayRawStatusResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut masked = self.0.clone();
        mask_status_log_value(&mut masked);
        formatter
            .debug_tuple("EcommpayRawStatusResponse")
            .field(&masked)
            .finish()
    }
}

impl Serialize for EcommpayRawStatusResponse {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut masked = self.0.clone();
        mask_status_log_value(&mut masked);
        masked.serialize(serializer)
    }
}

/// Verifies the top-level signature with the merchant's `secret_key` and only then parses the
/// body. Data from an unverified response is never used: a mismatch is an error, so the router
/// keeps the status it already had.
pub fn verified_status_response(
    raw: &EcommpayRawStatusResponse,
    connector_config: &ConnectorSpecificConfig,
    http_code: u16,
) -> Result<EcommpayStatusResponse, error_stack::Report<errors::ConnectorError>> {
    let auth = EcommpayAuthType::try_from(connector_config).change_context(
        utils::response_deserialization_fail(
            http_code,
            "Ecommpay credentials unavailable to verify the status response signature",
        ),
    )?;
    let verified = verify_signature(&raw.0, &auth.secret_key).change_context(
        utils::response_deserialization_fail(
            http_code,
            "Ecommpay status response signature could not be computed",
        ),
    )?;
    if !verified {
        return Err(error_stack::report!(utils::response_deserialization_fail(
            http_code,
            "Ecommpay status response signature mismatch",
        )));
    }
    serde_json::from_value::<EcommpayStatusResponse>(raw.0.clone()).change_context(
        utils::response_deserialization_fail(
            http_code,
            "Ecommpay status response did not match the documented shape",
        ),
    )
}

/// The shared payment-status table (PSync and callbacks). Exhaustive on purpose: a new
/// upstream status must be classified, never absorbed by a catch-all.
pub fn map_payment_status(status: EcommpayPaymentStatus) -> AttemptStatus {
    match status {
        EcommpayPaymentStatus::Processing => AttemptStatus::Pending,
        // The customer has to act (3DS, redirect, payment-system step, merchant-requested
        // authentication) before the payment can move on.
        EcommpayPaymentStatus::Awaiting3dsResult
        | EcommpayPaymentStatus::AwaitingRedirectResult
        | EcommpayPaymentStatus::AwaitingCustomerAction
        | EcommpayPaymentStatus::AwaitingMerchantAuth => AttemptStatus::AuthenticationPending,
        // Waiting on additional data or a customer retry: nothing to redirect to.
        EcommpayPaymentStatus::AwaitingClarification | EcommpayPaymentStatus::AwaitingCustomer => {
            AttemptStatus::Pending
        }
        EcommpayPaymentStatus::AwaitingCapture => AttemptStatus::Authorized,
        EcommpayPaymentStatus::Success => AttemptStatus::Charged,
        EcommpayPaymentStatus::Decline | EcommpayPaymentStatus::Error => AttemptStatus::Failure,
        EcommpayPaymentStatus::Canceled => AttemptStatus::Voided,
        // Refunds and reversals do not change the payment attempt: it stays charged.
        EcommpayPaymentStatus::PartiallyReversed
        | EcommpayPaymentStatus::Reversed
        | EcommpayPaymentStatus::PartiallyRefunded
        | EcommpayPaymentStatus::Refunded
        | EcommpayPaymentStatus::ScheduledRecurringProcessing => AttemptStatus::Charged,
        // Undocumented: keep the payment pollable rather than guess a terminal state.
        EcommpayPaymentStatus::Unknown => AttemptStatus::Pending,
    }
}

/// The operation that created the payment, selected by type — never by array position.
pub fn creating_operation(response: &EcommpayStatusResponse) -> Option<&EcommpayOperation> {
    let is_creating = |operation: &&EcommpayOperation| {
        matches!(
            operation.operation_type,
            EcommpayOperationType::Sale
                | EcommpayOperationType::Auth
                | EcommpayOperationType::Recurring
                | EcommpayOperationType::AccountVerification
        )
    };
    response
        .operations
        .as_ref()
        .and_then(|operations| operations.iter().find(is_creating))
        .or_else(|| response.operation.as_ref().filter(is_creating))
}

/// The error a declined operation maps to: Ecommpay's own code and message, the provider's raw
/// result, and the payment id, so GSM can key on the distinct code.
pub fn error_from_operation(
    operation: &EcommpayOperation,
    payment_id: &str,
    http_code: u16,
    attempt_status: Option<AttemptStatus>,
) -> ErrorResponse {
    let provider = operation.provider.as_ref();
    ErrorResponse {
        status_code: http_code,
        code: operation
            .code
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: operation
            .message
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: provider.and_then(|provider| provider.result_message.clone()),
        attempt_status: attempt_status.map(FlowStatus::Payment),
        connector_transaction_id: Some(payment_id.to_string()),
        network_decline_code: provider
            .and_then(|provider| provider.result_code.as_ref())
            .map(ToString::to_string),
        ..Default::default()
    }
}

// =============================================================================
// PSync
// =============================================================================

/// `POST /v2/payment/status` body.
#[derive(Debug, Clone, Serialize)]
pub struct EcommpayStatusRequest {
    pub general: EcommpayGeneral,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for EcommpayStatusRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        // The status is addressed by the general.payment_id Authorize sent, which
        // Authorize returned as the connector transaction id.
        let payment_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => non_empty(Some(id.clone())),
            ResponseId::EncodedData(_) | ResponseId::NoResponseId => None,
        }
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingConnectorTransactionID {
                context: context(
                    Some(
                        "Pass the connector_transaction_id returned by Authorize (the Ecommpay \
                         general.payment_id).",
                    ),
                    STATUS_DOC_URL,
                    "Ecommpay /v2/payment/status addresses the payment by general.payment_id; \
                     no connector transaction id was supplied"
                        .to_string(),
                ),
            })
        })?;
        let auth = EcommpayAuthType::try_from(&router_data.connector_config)?;
        let mut body = Self {
            general: EcommpayGeneral {
                project_id: auth.project_id.clone(),
                payment_id,
                signature: None,
                merchant_callback_url: None,
            },
        };
        body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
        Ok(body)
    }
}

/// The error of a list of Gate error items: the first item's code and every item's message,
/// so no message is dropped. An empty list yields `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`.
fn error_from_items(
    items: &[EcommpayErrorItem],
    payment_id: &str,
    http_code: u16,
    attempt_status: Option<AttemptStatus>,
) -> ErrorResponse {
    let details = EcommpayErrorResponse::Items {
        errors: items.to_vec(),
    }
    .details();
    ErrorResponse {
        status_code: http_code,
        code: details.code,
        message: details.message,
        reason: details.reason,
        attempt_status: attempt_status.map(FlowStatus::Payment),
        connector_transaction_id: Some(payment_id.to_string()),
        ..Default::default()
    }
}

/// `errors[]` of a status response, when it holds at least one item.
fn status_errors(response: &EcommpayStatusResponse) -> Option<&[EcommpayErrorItem]> {
    response
        .errors
        .as_deref()
        .filter(|errors| !errors.is_empty())
}

/// Code 3061: the Gate has not registered the payment. Payment processing is asynchronous, so
/// this is registration lag (or a payment rejected before registration), not a decline.
fn is_transaction_not_found(response: &EcommpayStatusResponse) -> bool {
    response.errors.iter().flatten().any(|item| {
        item.code
            .as_ref()
            .is_some_and(|code| code.to_string() == TRANSACTION_NOT_FOUND_CODE)
    })
}

/// A non-empty URL, or `None` with a warning naming the redirect source (never its value).
fn redirect_endpoint(url: &str, source: &'static str) -> Option<String> {
    let endpoint = non_empty(Some(url.to_string()));
    if endpoint.is_none() {
        tracing::warn!(
            source,
            "Ecommpay: 3DS redirect URL is empty; this redirect source is ignored"
        );
    }
    endpoint
}

/// The customer redirect of a payment awaiting 3-D Secure / a redirect, in the order the Gate
/// documents them: `threeds2.redirect` (3DS 2 challenge), `acs` (3DS 1 / hosted ACS page),
/// then the generic `redirect_data`.
fn status_redirect_form(response: &EcommpayStatusResponse) -> Option<RedirectForm> {
    if let Some(redirect) = response
        .threeds2
        .as_ref()
        .and_then(|threeds2| threeds2.redirect.as_ref())
    {
        if let Some(endpoint) = redirect_endpoint(&redirect.url, "threeds2.redirect") {
            return Some(RedirectForm::Form {
                endpoint,
                method: Method::Post,
                form_fields: redirect
                    .params
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone().expose()))
                    .collect(),
            });
        }
    }
    if let Some(acs) = response.acs.as_ref() {
        if let Some(endpoint) = redirect_endpoint(&acs.acs_url, "acs") {
            let mut form_fields = HashMap::from([
                ("PaReq".to_string(), acs.pa_req.clone().expose()),
                ("MD".to_string(), acs.md.clone().expose()),
            ]);
            if let Some(term_url) = non_empty(acs.term_url.clone()) {
                form_fields.insert("TermUrl".to_string(), term_url);
            }
            return Some(RedirectForm::Form {
                endpoint,
                method: Method::Post,
                form_fields,
            });
        }
    }
    let redirect_data = response.redirect_data.as_ref()?;
    let endpoint = redirect_endpoint(&redirect_data.url, "redirect_data")?;
    let method = match redirect_data.method.as_deref().map(str::trim) {
        Some(method) if method.eq_ignore_ascii_case("GET") => Method::Get,
        Some(method) if method.eq_ignore_ascii_case("POST") => Method::Post,
        None => Method::Post,
        Some(_) => {
            tracing::warn!(
                field = "redirect_data.method",
                "Ecommpay: unrecognised redirect method; the redirect is posted"
            );
            Method::Post
        }
    };
    let form_fields = match redirect_data.body.as_ref() {
        None | Some(Value::Null) => HashMap::new(),
        Some(Value::Object(body)) => body
            .iter()
            .filter_map(|(key, value)| match value {
                Value::String(text) => Some((key.clone(), text.clone())),
                Value::Number(number) => Some((key.clone(), number.to_string())),
                Value::Bool(flag) => Some((key.clone(), flag.to_string())),
                Value::Null | Value::Array(_) | Value::Object(_) => {
                    tracing::warn!(
                        field = "redirect_data.body",
                        key = key.as_str(),
                        "Ecommpay: non-scalar redirect body field cannot be a form field; dropped"
                    );
                    None
                }
            })
            .collect(),
        Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_)) => {
            tracing::warn!(
                field = "redirect_data.body",
                "Ecommpay: redirect body is not an object; no form fields are posted"
            );
            HashMap::new()
        }
    };
    Some(RedirectForm::Form {
        endpoint,
        method,
        form_fields,
    })
}

/// Card details the status response reports: AVS result, the issuer's auth code and the ECI.
fn status_connector_response(response: &EcommpayStatusResponse) -> Option<ConnectorResponseData> {
    let operation = creating_operation(response);
    let auth_code = operation
        .and_then(|operation| operation.provider.as_ref())
        .and_then(|provider| non_empty(provider.auth_code.clone()));
    let eci = operation.and_then(|operation| non_empty(operation.eci.clone()));
    let avs_result = response
        .avs_data
        .as_ref()
        .and_then(|avs| non_empty(avs.avs_result.clone()));
    if auth_code.is_none() && eci.is_none() && avs_result.is_none() {
        return None;
    }
    Some(ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: eci.map(|eci| serde_json::json!({ "eci": eci })),
            payment_checks: avs_result.map(|avs| serde_json::json!({ "avs_result": avs })),
            card_network: None,
            domestic_network: None,
            auth_code,
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

impl TryFrom<ResponseRouterData<EcommpayRawStatusResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EcommpayRawStatusResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        // Nothing from an unverified body is used.
        let response = verified_status_response(
            &item.response,
            &item.router_data.connector_config,
            http_code,
        )?;
        let sent_payment_id = match &item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => non_empty(Some(id.clone())),
            ResponseId::EncodedData(_) | ResponseId::NoResponseId => None,
        };
        // An unregistered payment's body has no payment.id: fall back to the id that was asked.
        let payment_id = non_empty(response.payment.id.clone())
            .or(sent_payment_id)
            .ok_or_else(|| {
                error_stack::report!(utils::response_deserialization_fail(
                    http_code,
                    "Ecommpay status response carries no payment.id and none was sent",
                ))
            })?;

        match response.payment.status {
            // A declined payment keeps the creating operation's code, message and
            // provider result.
            EcommpayPaymentStatus::Decline => {
                let error = match (creating_operation(&response), status_errors(&response)) {
                    (Some(operation), _) => error_from_operation(
                        operation,
                        &payment_id,
                        http_code,
                        Some(AttemptStatus::Failure),
                    ),
                    (None, Some(errors)) => error_from_items(
                        errors,
                        &payment_id,
                        http_code,
                        Some(AttemptStatus::Failure),
                    ),
                    (None, None) => {
                        error_from_items(&[], &payment_id, http_code, Some(AttemptStatus::Failure))
                    }
                };
                Ok(Self {
                    response: Err(error),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            // `error` fails the attempt, except 3061 "Transaction not found", which
            // the status request returns while an asynchronous payment is not yet registered —
            // the attempt status is then left as it was.
            EcommpayPaymentStatus::Error => {
                let not_found = is_transaction_not_found(&response);
                let attempt_status = (!not_found).then_some(AttemptStatus::Failure);
                let error = match (status_errors(&response), creating_operation(&response)) {
                    (Some(errors), _) => {
                        error_from_items(errors, &payment_id, http_code, attempt_status)
                    }
                    (None, Some(operation)) => {
                        error_from_operation(operation, &payment_id, http_code, attempt_status)
                    }
                    (None, None) => error_from_items(&[], &payment_id, http_code, attempt_status),
                };
                let status = if not_found {
                    item.router_data.resource_common_data.status
                } else {
                    AttemptStatus::Failure
                };
                Ok(Self {
                    response: Err(error),
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            EcommpayPaymentStatus::Processing
            | EcommpayPaymentStatus::Awaiting3dsResult
            | EcommpayPaymentStatus::AwaitingRedirectResult
            | EcommpayPaymentStatus::AwaitingCustomerAction
            | EcommpayPaymentStatus::AwaitingMerchantAuth
            | EcommpayPaymentStatus::AwaitingClarification
            | EcommpayPaymentStatus::AwaitingCustomer
            | EcommpayPaymentStatus::AwaitingCapture
            | EcommpayPaymentStatus::Success
            | EcommpayPaymentStatus::Canceled
            | EcommpayPaymentStatus::PartiallyReversed
            | EcommpayPaymentStatus::Reversed
            | EcommpayPaymentStatus::PartiallyRefunded
            | EcommpayPaymentStatus::Refunded
            | EcommpayPaymentStatus::ScheduledRecurringProcessing
            | EcommpayPaymentStatus::Unknown => {
                let status = map_payment_status(response.payment.status);
                let redirection_data = if status == AttemptStatus::AuthenticationPending {
                    let form = status_redirect_form(&response);
                    if form.is_none() {
                        // An iframe-only 3DS state has no redirect UCS can hand back.
                        tracing::warn!(
                            "Ecommpay: payment awaits 3DS/redirect but the status response \
                             carries no threeds2.redirect, acs or redirect_data; no redirect \
                             is returned"
                        );
                    }
                    form.map(Box::new)
                } else {
                    None
                };
                // The platform-stored credential of a verification or of
                // a purchase that registered a COF series.
                let registers_credential = response.payment.payment_type
                    == Some(EcommpayPaymentType::AccountVerification)
                    || response.payment.status == EcommpayPaymentStatus::Success;
                let mandate_reference = response
                    .recurring
                    .as_ref()
                    .filter(|_| registers_credential)
                    .map(|recurring| {
                        Box::new(MandateReference {
                            connector_mandate_id: Some(recurring.id.to_string()),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                            mandate_metadata: None,
                        })
                    });
                let network_txn_id = non_empty(
                    response
                        .payment
                        .scheme_id
                        .clone()
                        .map(ExposeInterface::expose),
                );
                let network_txn_link_id = non_empty(
                    response
                        .payment
                        .transaction_link_id
                        .clone()
                        .map(ExposeInterface::expose),
                );
                let connector_response = status_connector_response(&response)
                    .or(item.router_data.resource_common_data.connector_response);
                Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(payment_id.clone()),
                        redirection_data,
                        connector_metadata: None,
                        mandate_reference,
                        network_txn_id,
                        network_txn_link_id,
                        connector_response_reference_id: Some(payment_id),
                        incremental_authorization_allowed: None,
                        splits: None,
                        status_code: http_code,
                        payment_account_reference: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}

// =============================================================================
// RSync
// =============================================================================

/// RSync reads the refund from `POST /v2/payment/status` of the original purchase, so its body
/// is the PSync body; the alias gives the RSync flow its own name in the connector macros.
pub type EcommpayRefundSyncRequest = EcommpayStatusRequest;

/// The RSync response is the signed status body of the original purchase.
pub type EcommpayRefundSyncResponse = EcommpayRawStatusResponse;

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EcommpayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for EcommpayStatusRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EcommpayRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        // The refund is found by the request_id its acceptance returned; without it
        // no operation of the payment can be identified as this refund.
        if non_empty(Some(request.connector_refund_id.clone())).is_none() {
            return Err(error_stack::report!(
                IntegrationError::MissingConnectorRefundID {
                    context: context(
                        Some(
                            "Pass the connector_refund_id returned by Refund (the Ecommpay \
                             request_id of the refund acceptance).",
                        ),
                        STATUS_DOC_URL,
                        "Ecommpay has no refund-status endpoint; the refund is matched by \
                         operations[].request_id and no connector refund id was supplied"
                            .to_string(),
                    ),
                }
            ));
        }
        // The status is addressed by the original purchase's general.payment_id.
        let payment_id =
            non_empty(Some(request.connector_transaction_id.clone())).ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingConnectorTransactionID {
                    context: context(
                        Some(
                            "Pass the connector_transaction_id returned by Authorize (the \
                             Ecommpay general.payment_id of the refunded purchase).",
                        ),
                        STATUS_DOC_URL,
                        "Ecommpay refund status is read from /v2/payment/status of the original \
                         purchase; no connector transaction id was supplied"
                            .to_string(),
                    ),
                })
            })?;
        let auth = EcommpayAuthType::try_from(&router_data.connector_config)?;
        let mut body = Self {
            general: EcommpayGeneral {
                project_id: auth.project_id.clone(),
                payment_id,
                signature: None,
                merchant_callback_url: None,
            },
        };
        body.general.signature = Some(sign_request(&body, &auth.secret_key)?);
        Ok(body)
    }
}

/// The refund operation of a status body: type `refund` or `reversal` **and** `request_id`
/// equal to the connector refund id — never selected by array position.
fn refund_operation<'a>(
    response: &'a EcommpayStatusResponse,
    connector_refund_id: &str,
) -> Option<&'a EcommpayOperation> {
    let is_this_refund = |operation: &&EcommpayOperation| {
        matches!(
            operation.operation_type,
            EcommpayOperationType::Refund | EcommpayOperationType::Reversal
        ) && operation.request_id.as_deref() == Some(connector_refund_id)
    };
    response
        .operations
        .as_ref()
        .and_then(|operations| operations.iter().find(is_this_refund))
        .or_else(|| response.operation.as_ref().filter(is_this_refund))
}

/// RSync status table. Exhaustive on purpose: a new upstream status must be classified, never
/// absorbed by a catch-all. Only `success` and `decline`/`error` are final. Shared with the
/// refund callback.
pub fn map_refund_operation_status(status: EcommpayOperationStatus) -> RefundStatus {
    match status {
        EcommpayOperationStatus::Success => RefundStatus::Success,
        EcommpayOperationStatus::Decline | EcommpayOperationStatus::Error => RefundStatus::Failure,
        EcommpayOperationStatus::Processing
        | EcommpayOperationStatus::Awaiting3dsResult
        | EcommpayOperationStatus::AwaitingRedirectResult
        | EcommpayOperationStatus::AwaitingClarification
        | EcommpayOperationStatus::AwaitingCustomerAction
        | EcommpayOperationStatus::AwaitingMerchantAuth
        | EcommpayOperationStatus::AwaitingCustomer
        | EcommpayOperationStatus::AwaitingCapture => RefundStatus::Pending,
        // Undocumented: keep the refund pollable rather than guess a terminal state.
        EcommpayOperationStatus::Unknown => RefundStatus::Pending,
    }
}

/// A refund error carries the refund status, never a payment attempt status: a failed refund
/// must not move the payment.
fn refund_failure(mut error: ErrorResponse) -> ErrorResponse {
    error.attempt_status = Some(FlowStatus::Refund(RefundStatus::Failure));
    error
}

impl TryFrom<ResponseRouterData<EcommpayRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EcommpayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        // Nothing from an unverified body is used.
        let response = verified_status_response(
            &item.response,
            &item.router_data.connector_config,
            http_code,
        )?;
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();

        let (refund_status, error) = match refund_operation(&response, &connector_refund_id) {
            Some(operation) => {
                let refund_status = map_refund_operation_status(operation.status);
                // Review rule 2: a declined refund keeps the operation's own code, message and
                // provider result.
                let error = (refund_status == RefundStatus::Failure).then(|| {
                    refund_failure(error_from_operation(
                        operation,
                        &connector_refund_id,
                        http_code,
                        None,
                    ))
                });
                (refund_status, error)
            }
            None => match response
                .last_failed_request
                .as_ref()
                .filter(|failed| failed.id.as_deref() == Some(connector_refund_id.as_str()))
            {
                // The Gate rejected this refund request without creating an operation (e.g.
                // 3283 "Refund amount more than init amount"): every error item is kept.
                Some(failed) => (
                    RefundStatus::Failure,
                    Some(refund_failure(error_from_items(
                        failed.errors.as_deref().unwrap_or(&[]),
                        &connector_refund_id,
                        http_code,
                        None,
                    ))),
                ),
                // Requirement row 30: the refund operation is not listed yet (asynchronous
                // registration) — a sync miss is never terminal.
                None => {
                    tracing::warn!(
                        "Ecommpay: no refund/reversal operation with the connector refund id \
                         in the status response; the refund stays pending"
                    );
                    (RefundStatus::Pending, None)
                }
            },
        };

        let response = match error {
            Some(error) => Err(error),
            None => Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: http_code,
                acquirer_reference_number: None,
            }),
        };
        Ok(Self {
            response,
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// Callbacks
// =============================================================================
//
// Ecommpay posts three unrelated body shapes to the merchant: payment/operation callbacks
// (signed with a top-level `signature`), card-token callbacks (`general` + `request`, signed
// under `general.signature`) and chargeback callbacks (a separate channel, unsigned by design).
// A callback carries no event name: the event is `operation.type` + `operation.status`.

/// HTTP status recorded on the details of a callback (the callback itself is the response).
const CALLBACK_STATUS_CODE: u16 = 200;

/// `event` of a chargeback callback (techspec "Chargeback callbacks").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EcommpayChargebackEvent {
    NewChargebacksSummary,
    NewPreArbitrationSummary,
    NewArbitrationSummary,
    NewChargebackDetails,
    NewPreArbitrationDetails,
    NewArbitrationDetails,
    ChargebackCancelledByIssuer,
    ChargebackLost,
    ChargebackWon,
    #[serde(other)]
    Unknown,
}

/// Chargeback callback: only the event is read. Its `chargebacks[].operation_id` is an
/// Ecommpay operation id, not a payment id, so it is never used as a reference.
#[derive(Debug, Clone, Deserialize)]
pub struct EcommpayChargebackCallback {
    pub event: EcommpayChargebackEvent,
}

/// Card-token callback (`request.action` `tokenize` / `token_revoke`); nothing in it is read.
#[derive(Debug, Clone, Deserialize)]
pub struct EcommpayTokenCallback {
    pub general: serde::de::IgnoredAny,
    pub request: serde::de::IgnoredAny,
}

/// Every body Ecommpay posts to a merchant callback URL. Order matters: a chargeback is the
/// only shape with `event`, a token callback the only one with `general` + `request`; the
/// payment callback is the status-response shape with a singular `operation`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum EcommpayWebhookBody {
    Chargeback(EcommpayChargebackCallback),
    Token(EcommpayTokenCallback),
    Payment(Box<EcommpayStatusResponse>),
}

pub fn parse_webhook_body(
    body: &[u8],
) -> Result<EcommpayWebhookBody, error_stack::Report<errors::WebhookError>> {
    serde_json::from_slice::<EcommpayWebhookBody>(body)
        .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable(
            "Ecommpay callback is neither a payment, a token nor a chargeback callback",
        )
}

/// The outcome class of an operation status.
///
/// `ActionRequired` is Ecommpay's "waiting for the customer": `awaiting 3ds result`,
/// `awaiting redirect result`, `awaiting customer action` and `awaiting merchant auth` (the
/// customer must authenticate, or finish a step at the payment system). `Processing` is
/// everything still in flight where nothing is asked of the customer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationOutcome {
    Succeeded,
    Failed,
    ActionRequired,
    Processing,
}

/// Exhaustive on purpose: a new upstream status must be classified, never absorbed.
fn operation_outcome(status: EcommpayOperationStatus) -> OperationOutcome {
    match status {
        EcommpayOperationStatus::Success => OperationOutcome::Succeeded,
        EcommpayOperationStatus::Decline | EcommpayOperationStatus::Error => {
            OperationOutcome::Failed
        }
        EcommpayOperationStatus::Awaiting3dsResult
        | EcommpayOperationStatus::AwaitingRedirectResult
        | EcommpayOperationStatus::AwaitingCustomerAction
        | EcommpayOperationStatus::AwaitingMerchantAuth => OperationOutcome::ActionRequired,
        EcommpayOperationStatus::Processing
        | EcommpayOperationStatus::AwaitingClarification
        | EcommpayOperationStatus::AwaitingCustomer
        | EcommpayOperationStatus::AwaitingCapture => OperationOutcome::Processing,
        // Undocumented: classified as in progress, never as a terminal state.
        EcommpayOperationStatus::Unknown => {
            tracing::warn!(
                "Ecommpay: callback carries an undocumented operation.status; treated as \
                 processing"
            );
            OperationOutcome::Processing
        }
    }
}

/// `refund` and `reversal` are the two refund operations (a reversal is a same-day refund).
pub fn is_refund_operation(operation: &EcommpayOperation) -> bool {
    matches!(
        operation.operation_type,
        EcommpayOperationType::Refund | EcommpayOperationType::Reversal
    )
}

/// Whether the callback is about an individual COF debit rather than the payment that
/// registered the series.
pub fn is_recurring_operation(operation: &EcommpayOperation) -> bool {
    matches!(operation.operation_type, EcommpayOperationType::Recurring)
}

/// Event of a payment/operation callback, by `operation.type` + `operation.status` (techspec
/// "Webhook Events"). Exhaustive on purpose: no catch-all arm.
fn operation_event_type(operation: &EcommpayOperation) -> EventType {
    let outcome = operation_outcome(operation.status);
    match operation.operation_type {
        EcommpayOperationType::Sale
        | EcommpayOperationType::Recurring
        | EcommpayOperationType::AccountVerification => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentSuccess,
            OperationOutcome::Failed => EventType::PaymentIntentFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Auth => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentAuthorizationSuccess,
            OperationOutcome::Failed => EventType::PaymentIntentAuthorizationFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Capture => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentCaptureSuccess,
            OperationOutcome::Failed => EventType::PaymentIntentCaptureFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Cancel => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentCancelled,
            OperationOutcome::Failed => EventType::PaymentIntentCancelFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        // Review rule 1: a refund callback is mapped by its operation status, never by the
        // operation name alone.
        EcommpayOperationType::Refund | EcommpayOperationType::Reversal => match outcome {
            OperationOutcome::Succeeded => EventType::RefundSuccess,
            OperationOutcome::Failed => EventType::RefundFailure,
            OperationOutcome::ActionRequired | OperationOutcome::Processing => {
                EventType::RefundProcessing
            }
        },
        // No event row in the techspec for these operations: they never move the payment to a
        // terminal state from a callback.
        EcommpayOperationType::Incremental
        | EcommpayOperationType::PaymentConfirmation
        | EcommpayOperationType::RecurringCancel => match outcome {
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Succeeded
            | OperationOutcome::Failed
            | OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Unknown => {
            tracing::warn!(
                "Ecommpay: callback carries an undocumented operation.type; treated as \
                 processing"
            );
            EventType::PaymentIntentProcessing
        }
    }
}

/// Chargeback callbacks become dispute events; they are unsigned and carry no reference.
fn chargeback_event_type(event: EcommpayChargebackEvent) -> EventType {
    match event {
        EcommpayChargebackEvent::NewChargebacksSummary
        | EcommpayChargebackEvent::NewPreArbitrationSummary
        | EcommpayChargebackEvent::NewArbitrationSummary
        | EcommpayChargebackEvent::NewChargebackDetails
        | EcommpayChargebackEvent::NewPreArbitrationDetails
        | EcommpayChargebackEvent::NewArbitrationDetails => EventType::DisputeOpened,
        EcommpayChargebackEvent::ChargebackWon => EventType::DisputeWon,
        EcommpayChargebackEvent::ChargebackLost => EventType::DisputeLost,
        EcommpayChargebackEvent::ChargebackCancelledByIssuer => EventType::DisputeCancelled,
        EcommpayChargebackEvent::Unknown => {
            tracing::warn!(
                "Ecommpay: chargeback callback carries an undocumented event; left unspecified"
            );
            EventType::IncomingWebhookEventUnspecified
        }
    }
}

/// Event type of any callback body (ParseEvent phase, stateless).
pub fn webhook_event_type(body: &EcommpayWebhookBody) -> EventType {
    match body {
        EcommpayWebhookBody::Chargeback(callback) => chargeback_event_type(callback.event),
        EcommpayWebhookBody::Token(_) => EventType::IncomingWebhookEventUnspecified,
        EcommpayWebhookBody::Payment(response) => match response.operation.as_ref() {
            Some(operation) => operation_event_type(operation),
            None => {
                tracing::warn!(
                    "Ecommpay: payment callback carries no operation object; treated as \
                     processing"
                );
                EventType::PaymentIntentProcessing
            }
        },
    }
}

fn callback_payment_id(
    response: &EcommpayStatusResponse,
) -> Result<String, error_stack::Report<errors::WebhookError>> {
    non_empty(response.payment.id.clone()).ok_or_else(|| {
        error_stack::report!(errors::WebhookError::WebhookReferenceIdNotFound)
            .attach_printable("Ecommpay payment callback carries no payment.id")
    })
}

/// The refund operation of a callback and its `request_id` (the connector refund id that the
/// refund acceptance returned).
fn callback_refund_operation(
    response: &EcommpayStatusResponse,
) -> Result<(&EcommpayOperation, String), error_stack::Report<errors::WebhookError>> {
    let operation = response
        .operation
        .as_ref()
        .filter(|operation| is_refund_operation(operation))
        .ok_or_else(|| {
            error_stack::report!(errors::WebhookError::WebhookResourceObjectNotFound)
                .attach_printable("Ecommpay callback carries no refund or reversal operation")
        })?;
    let connector_refund_id = non_empty(operation.request_id.clone()).ok_or_else(|| {
        error_stack::report!(errors::WebhookError::WebhookReferenceIdNotFound)
            .attach_printable("Ecommpay refund callback carries no operation.request_id")
    })?;
    Ok((operation, connector_refund_id))
}

/// Reference of a callback (ParseEvent phase). Payment callbacks are keyed by `payment.id`
/// (the merchant `general.payment_id`, which UCS returns as connector_transaction_id); refund
/// callbacks by `operation.request_id`, with `payment.id` kept as the payment it belongs to.
/// Chargeback and token callbacks have no reference.
pub fn webhook_reference(
    body: &EcommpayWebhookBody,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
    match body {
        EcommpayWebhookBody::Chargeback(_) | EcommpayWebhookBody::Token(_) => Ok(None),
        EcommpayWebhookBody::Payment(response) => {
            let payment_id = callback_payment_id(response)?;
            let is_refund = response.operation.as_ref().is_some_and(is_refund_operation);
            if is_refund {
                let (_, connector_refund_id) = callback_refund_operation(response)?;
                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(connector_refund_id),
                        merchant_refund_id: None,
                        connector_transaction_id: Some(payment_id),
                        merchant_transaction_id: None,
                    },
                )))
            } else {
                Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(payment_id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    },
                )))
            }
        }
    }
}

/// Payment callback details (HandleEvent phase). The status comes from the shared
/// `payment.status` table, exactly as PSync reads it; a declined or failed operation keeps
/// its own code, message and provider result.
pub fn payment_webhook_details(
    response: &EcommpayStatusResponse,
) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
    let payment_id = callback_payment_id(response)?;
    // `payment.status` describes the payment as a whole, and a COF series sits at
    // `scheduled recurring processing` while individual debits come and go -- so for a
    // recurring-operation callback it cannot say whether *this* debit was taken. Ecommpay
    // documents a declined `recurring` operation under that payment status, which the
    // payment-level table would have reported as `Charged` while the event derived from the
    // same operation is `PaymentIntentFailure`. The operation is this attempt, so it decides.
    // The registration payment (sale / auth / account verification) keeps the payment status,
    // which is also what PSync of that payment returns.
    let recurring_operation = response
        .operation
        .as_ref()
        .filter(|operation| is_recurring_operation(operation));
    let status = match recurring_operation {
        Some(operation) => match operation_outcome(operation.status) {
            OperationOutcome::Succeeded => AttemptStatus::Charged,
            OperationOutcome::Failed => AttemptStatus::Failure,
            OperationOutcome::ActionRequired => AttemptStatus::AuthenticationPending,
            OperationOutcome::Processing => AttemptStatus::Pending,
        },
        None => map_payment_status(response.payment.status),
    };
    let error = response
        .operation
        .as_ref()
        .filter(|operation| operation_outcome(operation.status) == OperationOutcome::Failed)
        .map(|operation| {
            error_from_operation(
                operation,
                &payment_id,
                CALLBACK_STATUS_CODE,
                Some(AttemptStatus::Failure),
            )
        });
    // Same rule as PSync: the platform-stored credential of a verification or of a purchase
    // that registered a COF series.
    let registers_credential = response.payment.payment_type
        == Some(EcommpayPaymentType::AccountVerification)
        || response.payment.status == EcommpayPaymentStatus::Success;
    let mandate_reference = response
        .recurring
        .as_ref()
        .filter(|_| registers_credential)
        .map(|recurring| {
            Box::new(MandateReference {
                connector_mandate_id: Some(recurring.id.to_string()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });
    let network_txn_id = non_empty(
        response
            .payment
            .scheme_id
            .clone()
            .map(ExposeInterface::expose),
    );
    let (error_code, error_message, error_reason) = match error {
        Some(error) => (Some(error.code), Some(error.message), error.reason),
        None => (None, None, None),
    };
    // `payment.sum` is the amount currently standing on the payment, so on a charged payment it
    // is what was actually captured -- a partial capture reports the captured part rather than
    // the full authorised amount. It is NOT the captured amount once money has been sent back:
    // a refund reduces `payment.sum` while `payment.status` stays `success`, so a refund
    // operation is excluded instead of under-reporting the capture.
    // `payment.sum` is the parent payment's standing amount, so it is not an individual COF
    // debit's captured amount either: a recurring-operation callback reports none.
    let settles_capture = status == AttemptStatus::Charged
        && recurring_operation.is_none()
        && !response.operation.as_ref().is_some_and(is_refund_operation);
    let amount_captured = response
        .payment
        .sum
        .as_ref()
        .filter(|_| settles_capture)
        .map(|sum| Money {
            amount: sum.amount,
            currency: sum.currency,
        });
    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(payment_id.clone())),
        status,
        connector_response_reference_id: Some(payment_id),
        connector_request_reference_id: None,
        mandate_reference,
        error_code,
        error_message,
        error_reason,
        // The callback body carries customer and account data; it is not echoed back.
        raw_connector_response: None,
        status_code: CALLBACK_STATUS_CODE,
        response_headers: None,
        amount_captured,
        network_txn_id,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// Refund callback details (HandleEvent phase), mapped with the RSync operation table.
pub fn refund_webhook_details(
    response: &EcommpayStatusResponse,
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
    let (operation, connector_refund_id) = callback_refund_operation(response)?;
    let status = map_refund_operation_status(operation.status);
    let (error_code, error_message) = if status == RefundStatus::Failure {
        let error =
            error_from_operation(operation, &connector_refund_id, CALLBACK_STATUS_CODE, None);
        (Some(error.code), Some(error.message))
    } else {
        (None, None)
    };
    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(connector_refund_id),
        merchant_transaction_id: None,
        status,
        connector_response_reference_id: non_empty(response.payment.merchant_refund_id.clone()),
        error_code,
        error_message,
        raw_connector_response: None,
        status_code: CALLBACK_STATUS_CODE,
        response_headers: None,
    })
}
