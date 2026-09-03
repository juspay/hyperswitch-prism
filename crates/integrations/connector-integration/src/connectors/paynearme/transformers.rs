//! PayNearMe API v3.0 transformers.
//!
//! PayNearMe is an RPC-over-JSON gateway: every operation is a `POST` to
//! `{base_url}/{operation}` with `Content-Type: application/json`. There is no
//! `Authorization` header — each request carries a per-request HMAC-SHA256
//! `signature` field computed over the alphabetically sorted, `key + value`
//! concatenation of the body it is about to be sent with (see
//! [`paynearme_signature`]).
//!
//! Scope of this module: Card, one-time payments only.
//! * `CreateOrder` -> `POST /create_order`
//! * `Authorize`   -> `POST /create_payment_method` with `send_payment=true`
//!   (tokenises the card and charges it in one round trip)
//! * `PSync`       -> `POST /find_payment`
//! * `Void`        -> `POST /cancel_payment`
//! * `Refund`      -> `POST /refund_payment`
//! * `RSync`       -> `POST /find_payment` (reads the nested `refund` object)
//!
//! `Capture` has **no** counterpart in the PayNearMe API: card payments are
//! sale / auto-capture. The flow is wired as `not_implemented` in `paynearme.rs`
//! and must never be routed at `/make_payment`, which would charge a second time.
//!
//! 3-D Secure does not exist anywhere in the PayNearMe API surface, so a
//! `ThreeDs` authorize is rejected outright rather than silently downgraded.

use common_enums::{AttemptStatus, AuthenticationType, Currency, RefundStatus};
use common_utils::{crypto::SignMessage, types::StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, CreateOrder, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Deserializer, Serialize};

use crate::connectors::paynearme::{PaynearmeAmountConvertor, PaynearmeRouterData};
use crate::types::ResponseRouterData;
/// Connector id, reused in every `NotSupported` error.
pub(super) const PAYNEARME: &str = "paynearme";

/// The API version this integration speaks. The value must match the version the
/// API key pair was issued for; a `3.0` key signs with SHA-256 (older 1.8 / 2.0
/// keys sign with MD5, which is deliberately not implemented).
const PAYNEARME_API_VERSION: &str = "3.0";

/// Parameters that must never enter the signature input, per the reference
/// implementations on the Authentication page.
const SIGNATURE_EXEMPT_FIELDS: [&str; 3] = ["format", "signature", "call"];

/// Parameters that must always be present in the signature input.
const SIGNATURE_REQUIRED_FIELDS: [&str; 3] = ["site_identifier", "timestamp", "version"];

/// `order_type` for a one-time payment of a known amount (`any` | `exact` | `up-to`).
const ORDER_TYPE_EXACT: &str = "exact";
/// A standing order is a repeatedly-payable balance — out of scope here.
const ORDER_IS_STANDING_FALSE: &str = "false";
/// The only `payment_method_type` in scope. PayNearMe classifies credit vs debit
/// from the BIN and reports it back in `payment_type`.
const PAYMENT_METHOD_TYPE_CARD: &str = "card";
/// The only `site_channel` in scope (the `*_recurring` values are out of scope).
const SITE_CHANNEL_CONSUMER: &str = "consumer";
/// Tokenise **and** charge in the same `/create_payment_method` call.
const SEND_PAYMENT_TRUE: &str = "true";
/// Keep `payments[]` down to the single entry this Authorize just created.
const LAST_PMT_ONLY_TRUE: &str = "true";
/// `/create_order` returns only the identifiers we need when this is set.
const RETURN_MINIMAL_INFO_TRUE: &str = "true";

/// `status` value of a successful response envelope.
const ENVELOPE_STATUS_OK: &str = "ok";
/// `response_code` value meaning "Success".
const RESPONSE_CODE_SUCCESS: &str = "0";

fn context() -> IntegrationErrorContext {
    IntegrationErrorContext::default()
}

fn not_supported(message: impl Into<String>) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: message.into(),
        connector: PAYNEARME,
        context: context(),
    })
}

// =============================================================================
// AUTH
// =============================================================================

/// PayNearMe issues one key pair per site: a *Site/Key Identifier* (public, sent
/// as the `site_identifier` body field) and an *API Secret Key* (never
/// transmitted, used only as the HMAC key). There is no third credential, hence
/// `BodyKey` rather than `SignatureKey`.
#[derive(Debug, Clone)]
pub struct PaynearmeAuthType {
    /// PayNearMe **API Secret Key** — the HMAC-SHA256 key. Never serialised.
    pub api_secret_key: Secret<String>,
    /// PayNearMe **Site Identifier**, e.g. `"S2411573363"`.
    pub site_identifier: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PaynearmeAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paynearme { api_key, key1, .. } => Ok(Self {
                api_secret_key: api_key.to_owned(),
                site_identifier: key1.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType { context: context() }
            )),
        }
    }
}

// =============================================================================
// SIGNING
// =============================================================================

/// Current Unix epoch **seconds**, as the decimal string PayNearMe expects.
fn current_timestamp() -> String {
    time::OffsetDateTime::now_utc().unix_timestamp().to_string()
}

/// Build the `string_to_sign` from an already-serialised request body.
///
/// 1. Drop the exempt parameters (`format`, `signature`, `call`).
/// 2. Drop absent parameters — a field skipped by
///    `skip_serializing_if = "Option::is_none"` is not on the wire either.
/// 3. Sort what remains alphabetically by key (plain lexicographic sort).
/// 4. Concatenate `key + value` with **no** separators.
fn paynearme_string_to_sign(
    body: &serde_json::Value,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let object = body
        .as_object()
        .ok_or_else(|| IntegrationError::RequestEncodingFailed { context: context() })?;

    for required in SIGNATURE_REQUIRED_FIELDS {
        if !object.contains_key(required) {
            return Err(error_stack::report!(
                IntegrationError::MissingRequiredField {
                    field_name: match required {
                        "site_identifier" => "site_identifier",
                        "timestamp" => "timestamp",
                        _ => "version",
                    },
                    context: context(),
                }
            ));
        }
    }

    let mut pairs: Vec<(&str, String)> = Vec::with_capacity(object.len());
    for (key, value) in object.iter() {
        if SIGNATURE_EXEMPT_FIELDS.contains(&key.as_str()) {
            continue;
        }
        let rendered = match value {
            // Absent is absent: nothing to sign and nothing on the wire.
            serde_json::Value::Null => continue,
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Bool(flag) => flag.to_string(),
            serde_json::Value::Number(number) => number.to_string(),
            other => other.to_string(),
        };
        pairs.push((key.as_str(), rendered));
    }
    pairs.sort_by(|left, right| left.0.cmp(right.0));

    Ok(pairs
        .into_iter()
        .map(|(key, value)| format!("{key}{value}"))
        .collect())
}

/// `signature = hex_lowercase(HMAC_SHA256(API_SECRET_KEY, string_to_sign))`.
///
/// The signature is computed over **exactly** the map that will be serialised, so
/// the request struct is built with an empty `signature` first, signed, and only
/// then emitted. Every request field is therefore typed `String` (or a
/// string-serialising newtype such as [`StringMajorUnit`]) so that serialisation
/// and signing agree byte for byte.
pub fn paynearme_signature<R: Serialize>(
    api_secret_key: &Secret<String>,
    request: &R,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let body = serde_json::to_value(request)
        .map_err(|_| IntegrationError::RequestEncodingFailed { context: context() })?;
    let string_to_sign = paynearme_string_to_sign(&body)?;

    let digest = common_utils::crypto::HmacSha256
        .sign_message(api_secret_key.peek().as_bytes(), string_to_sign.as_bytes())
        .map_err(|_| IntegrationError::RequestEncodingFailed { context: context() })?;

    Ok(Secret::new(hex::encode(digest)))
}

/// PayNearMe prices everything in USD; every `*_currency` field is documented as
/// `USD` and no other currency is accepted.
///
/// Returns [`Currency`] rather than a `String` so the type survives all the way
/// on to the wire. `Currency` carries `#[serde(rename_all = "UPPERCASE")]`
/// (`common_enums/src/enums.rs:30`), so it serialises to exactly `"USD"` — the
/// same bytes the previous `Currency::USD.to_string()` produced, which matters
/// because the HMAC is computed over the serialised body (see
/// [`paynearme_string_to_sign`]).
fn require_usd(currency: Currency) -> Result<Currency, error_stack::Report<IntegrationError>> {
    if currency == Currency::USD {
        Ok(Currency::USD)
    } else {
        Err(not_supported(format!("Currency {currency}")))
    }
}

// =============================================================================
// CREATE ORDER — `POST /create_order`
// =============================================================================

/// `/create_order` request. "With PayNearMe, an order is required any time money
/// moves or is scheduled to move", so this runs ahead of every Authorize.
#[derive(Debug, Serialize)]
pub struct PaynearmeCreateOrderRequest {
    pub site_identifier: Secret<String>,
    pub timestamp: String,
    pub version: String,
    pub signature: Secret<String>,
    pub order_amount: StringMajorUnit,
    pub order_currency: Currency,
    pub site_customer_identifier: String,
    pub order_type: String,
    pub order_is_standing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_order_identifier: Option<String>,
    /// Returns only `pnm_order_identifier` / `pnm_customer_identifier`, which is
    /// all this flow consumes, and keeps the cash/slip payload out of the response.
    pub return_minimal_info: String,
}

type CreateOrderRouterData =
    RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<PaynearmeRouterData<CreateOrderRouterData, T>> for PaynearmeCreateOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: PaynearmeRouterData<CreateOrderRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PaynearmeAuthType::try_from(&router_data.connector_config)?;
        let common = &router_data.resource_common_data;

        let order_currency = require_usd(router_data.request.currency)?;
        let order_amount =
            PaynearmeAmountConvertor::convert(router_data.request.amount, Currency::USD)?;

        // `site_customer_identifier` is required and is a client-created unique
        // string; fall back to the attempt reference when no customer is attached.
        let site_customer_identifier = common
            .customer_id
            .as_ref()
            .map(|customer_id| customer_id.get_string_repr().to_string())
            .unwrap_or_else(|| common.connector_request_reference_id.clone());

        let mut request = Self {
            site_identifier: auth.site_identifier,
            timestamp: current_timestamp(),
            version: PAYNEARME_API_VERSION.to_string(),
            signature: Secret::new(String::new()),
            order_amount,
            order_currency,
            site_customer_identifier,
            order_type: ORDER_TYPE_EXACT.to_string(),
            order_is_standing: ORDER_IS_STANDING_FALSE.to_string(),
            site_order_identifier: Some(common.connector_request_reference_id.clone()),
            return_minimal_info: RETURN_MINIMAL_INFO_TRUE.to_string(),
        };
        request.signature = paynearme_signature(&auth.api_secret_key, &request)?;
        Ok(request)
    }
}

// =============================================================================
// AUTHORIZE — `POST /create_payment_method` with `send_payment=true`
// =============================================================================

/// `/create_payment_method` request, card variant, charging in the same call.
///
/// The credit-card and debit-card `oneOf` variants of this endpoint are
/// field-identical (both `payment_method_type: "card"`); PayNearMe decides which
/// it is from the BIN.
#[derive(Debug, Serialize)]
pub struct PaynearmeAuthorizeRequest {
    pub site_identifier: Secret<String>,
    pub timestamp: String,
    pub version: String,
    pub signature: Secret<String>,
    /// The order created by the `CreateOrder` flow.
    pub pnm_order_identifier: String,
    pub payment_method_type: String,
    /// PAN, plain digits, no separators.
    pub payment_method_card_number_pii: Secret<String>,
    /// `MM/YYYY` per the field's normative description. (The docs' own examples
    /// show `MM/YY`; `accounts.expiration_date` in responses uses `MM/YYYY`.)
    pub payment_method_card_expiry_pii: Secret<String>,
    pub payment_method_cvv_pii: Secret<String>,
    pub payment_method_billing_name: Secret<String>,
    pub payment_method_billing_address: Secret<String>,
    pub payment_method_billing_zipcode: Secret<String>,
    pub payment_method_billing_phone: Secret<String>,
    pub send_payment: String,
    pub payment_amount: StringMajorUnit,
    pub payment_currency: Currency,
    pub site_channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_payment_identifier: Option<String>,
    /// Response shaping only: keeps `payments[]` to the entry this call created.
    pub last_pmt_only: String,
}

type AuthorizeRouterData<T> =
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>;

/// `MM/YYYY`, built from the two-digit month and the four-digit year.
fn card_expiry_mm_yyyy<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let month = card.get_card_expiry_month_2_digit()?;
    let year = card.get_expiry_year_4_digit();
    Ok(Secret::new(format!("{}/{}", month.peek(), year.peek())))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<PaynearmeRouterData<AuthorizeRouterData<T>, T>> for PaynearmeAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: PaynearmeRouterData<AuthorizeRouterData<T>, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let common = &router_data.resource_common_data;
        let request = &router_data.request;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported by paynearme".to_string(),
                    context(),
                )))
            }
        };

        // The PayNearMe API v3.0 has no 3-D Secure surface at all: no enrolment or
        // verification endpoint, no CAVV/ECI/XID/dsTransId field, no ACS redirect.
        // Refuse rather than silently downgrade to a non-3DS charge.
        //
        // The test is `auth_type` (per the spec) plus the presence of real
        // authentication results. It deliberately does NOT look at
        // `enrolled_for_3ds`: that flag reports whether the *merchant* is
        // enrolled in 3DS, not whether *this* payment is a 3DS one, and
        // Hyperswitch hardcodes it to `true` on every card authorize
        // (`router/src/core/payments/transformers.rs`), so keying off it
        // rejected every single PayNearMe payment that came from Hyperswitch.
        if common.auth_type == AuthenticationType::ThreeDs || request.authentication_data.is_some()
        {
            return Err(not_supported("Three DS payments"));
        }

        // Mandates / MIT / stored credentials are out of scope for this
        // integration: `SetupMandate` and `RepeatPayment` are `not_implemented`
        // and `mandates` is declared `NotSupported` in `paynearme.rs`. Nothing
        // here reads `mandate_id` / `setup_mandate_details` / `setup_future_usage`,
        // so without this guard an off-session or credential-storing authorize
        // would be charged as a plain one-off and come back with
        // `mandate_reference: None` — the merchant would believe a credential
        // was stored when none was. Refuse, for the same reason 3DS is refused
        // above rather than silently downgraded.
        if request.is_mandate_payment() {
            return Err(not_supported("Mandates / stored credentials"));
        }

        // There is no capture endpoint anywhere in the API (see `Capture` in
        // `paynearme.rs`): card payments are sale / auto-capture only, so any
        // capture method that would need a second call is refused.
        // `is_auto_capture()` is false for exactly Manual / ManualMultiple /
        // Scheduled; `capture_method` is read only to name the offender.
        if !request.is_auto_capture() {
            return Err(not_supported(match request.capture_method {
                Some(method) => format!("{method} capture"),
                None => "This capture method".to_string(),
            }));
        }

        let auth = PaynearmeAuthType::try_from(&router_data.connector_config)?;

        let payment_currency = require_usd(request.currency)?;
        let payment_amount =
            PaynearmeAmountConvertor::convert(request.minor_amount, Currency::USD)?;

        // Written by the CreateOrder flow (or supplied by the caller as
        // `connector_order_id`); `/create_payment_method` cannot run without it.
        let pnm_order_identifier =
            common
                .connector_order_id
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_order_id",
                    context: context(),
                })?;

        // `payment_method_billing_name` is a required field on
        // `/create_payment_method`, so the billing full name is required here.
        // It is read straight off the billing address — no cardholder-name
        // fallback — so a caller that sends neither gets a precise
        // missing-field error instead of a gateway 400.
        let billing_name = common.get_billing_full_name()?;

        Ok({
            let mut built = Self {
                site_identifier: auth.site_identifier,
                timestamp: current_timestamp(),
                version: PAYNEARME_API_VERSION.to_string(),
                signature: Secret::new(String::new()),
                pnm_order_identifier,
                payment_method_type: PAYMENT_METHOD_TYPE_CARD.to_string(),
                payment_method_card_number_pii: Secret::new(card.card_number.peek().to_string()),
                payment_method_card_expiry_pii: card_expiry_mm_yyyy(card)?,
                payment_method_cvv_pii: card.card_cvc.clone(),
                payment_method_billing_name: billing_name,
                payment_method_billing_address: common.get_billing_line1()?,
                payment_method_billing_zipcode: common.get_billing_zip()?,
                // Documented as required. It can be made optional per site, but
                // surfacing the gap here beats a 400 from the gateway.
                payment_method_billing_phone: common.get_billing_phone_number()?,
                send_payment: SEND_PAYMENT_TRUE.to_string(),
                payment_amount,
                payment_currency,
                site_channel: SITE_CHANNEL_CONSUMER.to_string(),
                site_payment_identifier: Some(common.connector_request_reference_id.clone()),
                last_pmt_only: LAST_PMT_ONLY_TRUE.to_string(),
            };
            built.signature = paynearme_signature(&auth.api_secret_key, &built)?;
            built
        })
    }
}

// =============================================================================
// PSYNC / RSYNC / VOID — payment-keyed requests
// =============================================================================

/// `/find_payment` (PSync and RSync) and `/cancel_payment` (Void) share the exact
/// same body: the envelope plus `pnm_payment_identifier`.
#[derive(Debug, Serialize)]
pub struct PaynearmePaymentLookupRequest {
    pub site_identifier: Secret<String>,
    pub timestamp: String,
    pub version: String,
    pub signature: Secret<String>,
    pub pnm_payment_identifier: String,
}

impl PaynearmePaymentLookupRequest {
    fn build(
        auth: &PaynearmeAuthType,
        pnm_payment_identifier: String,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let mut request = Self {
            site_identifier: auth.site_identifier.clone(),
            timestamp: current_timestamp(),
            version: PAYNEARME_API_VERSION.to_string(),
            signature: Secret::new(String::new()),
            pnm_payment_identifier,
        };
        request.signature = paynearme_signature(&auth.api_secret_key, &request)?;
        Ok(request)
    }
}

/// PSync request — `POST /find_payment`.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct PaynearmeSyncRequest(pub PaynearmePaymentLookupRequest);

type SyncRouterData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<PaynearmeRouterData<SyncRouterData, T>> for PaynearmeSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: PaynearmeRouterData<SyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PaynearmeAuthType::try_from(&router_data.connector_config)?;
        let pnm_payment_identifier = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .map_err(|_| IntegrationError::MissingConnectorTransactionID { context: context() })?;
        Ok(Self(PaynearmePaymentLookupRequest::build(
            &auth,
            pnm_payment_identifier,
        )?))
    }
}

/// Void request — `POST /cancel_payment`. Only unprocessed payments can be
/// cancelled; for cards the window closes five minutes before the card network's
/// cutoff, after which a Refund is the correct operation.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct PaynearmeVoidRequest(pub PaynearmePaymentLookupRequest);

type VoidRouterData = RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<PaynearmeRouterData<VoidRouterData, T>> for PaynearmeVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: PaynearmeRouterData<VoidRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PaynearmeAuthType::try_from(&router_data.connector_config)?;
        Ok(Self(PaynearmePaymentLookupRequest::build(
            &auth,
            router_data.request.connector_transaction_id.clone(),
        )?))
    }
}

/// RSync request — the same `POST /find_payment` as PSync, keyed on the payment
/// id, because PayNearMe mints no refund identifier of its own (§8.6.3) and the
/// refund is surfaced as a nested object on the payment record.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct PaynearmeRefundSyncRequest(pub PaynearmePaymentLookupRequest);

type RefundSyncRouterData =
    RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<PaynearmeRouterData<RefundSyncRouterData, T>> for PaynearmeRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: PaynearmeRouterData<RefundSyncRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PaynearmeAuthType::try_from(&router_data.connector_config)?;
        Ok(Self(PaynearmePaymentLookupRequest::build(
            &auth,
            router_data.request.connector_transaction_id.clone(),
        )?))
    }
}

// =============================================================================
// REFUND — `POST /refund_payment`
// =============================================================================

/// Refund request. `refund_amount` / `refund_currency` "should only be included
/// for partial-amount refunds", so a full refund omits both.
#[derive(Debug, Serialize)]
pub struct PaynearmeRefundRequest {
    pub site_identifier: Secret<String>,
    pub timestamp: String,
    pub version: String,
    pub signature: Secret<String>,
    pub pnm_payment_identifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_amount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_currency: Option<Currency>,
}

type RefundRouterData = RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<PaynearmeRouterData<RefundRouterData, T>> for PaynearmeRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: PaynearmeRouterData<RefundRouterData, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let auth = PaynearmeAuthType::try_from(&router_data.connector_config)?;

        let currency = require_usd(request.currency)?;
        let (refund_amount, refund_currency) =
            if request.minor_refund_amount == request.minor_payment_amount {
                (None, None)
            } else {
                (
                    Some(PaynearmeAmountConvertor::convert(
                        request.minor_refund_amount,
                        Currency::USD,
                    )?),
                    Some(currency),
                )
            };

        let mut built = Self {
            site_identifier: auth.site_identifier,
            timestamp: current_timestamp(),
            version: PAYNEARME_API_VERSION.to_string(),
            signature: Secret::new(String::new()),
            pnm_payment_identifier: request.connector_transaction_id.clone(),
            refund_amount,
            refund_currency,
        };
        built.signature = paynearme_signature(&auth.api_secret_key, &built)?;
        Ok(built)
    }
}

// =============================================================================
// RESPONSE PRIMITIVES
// =============================================================================

/// PayNearMe quotes identifiers and amounts inconsistently: `pnm_order_identifier`
/// is `85237034088` in one documented example and `"86383382942"` in another, and
/// `payment_amount` is both `100` and `"504.99"`. Anything typed as a bare
/// `String` or a bare `u64` will fail on real traffic, so every such field goes
/// through this.
fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(serde_json::Value::String(text)) if !text.is_empty() => Some(text),
        Some(serde_json::Value::Number(number)) => Some(number.to_string()),
        Some(serde_json::Value::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    })
}

/// Amount fields, as [`StringMajorUnit`].
///
/// [`StringMajorUnit`] derives a plain `Deserialize` over its inner `String`
/// (`common_utils/src/types.rs:373`), so it rejects a JSON number outright —
/// which PayNearMe does send (`payment_amount` is documented as
/// `number | string` and appears as both `100` and `"504.99"`). This normalises
/// either shape to the major-unit string first. `StringMajorUnit::new` is
/// private, so the value is constructed by deserialising a `Value::String`.
///
/// An absent, null, empty or otherwise unusable value is reported as `None`
/// rather than failing the whole response body: these fields are informational
/// (`net_payment_amount` is a settlement figure, not the captured amount) and
/// none of them gates a status decision.
fn deserialize_optional_string_major_unit<'de, D>(
    deserializer: D,
) -> Result<Option<StringMajorUnit>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let amount = match value {
        Some(serde_json::Value::String(text)) if !text.is_empty() => text,
        Some(serde_json::Value::Number(number)) => number.to_string(),
        _ => return Ok(None),
    };
    Ok(StringMajorUnit::deserialize(serde_json::Value::String(amount)).ok())
}

/// Response `*_currency` fields, as [`Currency`].
///
/// PayNearMe documents every response currency as `USD`, but an unrecognised or
/// oddly-cased code must not take the whole response down with it — the field is
/// optional and purely informational, whereas failing here would turn a
/// perfectly good refund response into a deserialisation error. Unknown codes
/// therefore degrade to `None`.
fn deserialize_optional_currency<'de, D>(deserializer: D) -> Result<Option<Currency>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(serde_json::Value::String(text)) if !text.is_empty() => {
            Currency::deserialize(serde_json::Value::String(text.to_uppercase())).ok()
        }
        _ => None,
    })
}

/// `payment_type` — how the consumer actually paid, as classified by PayNearMe.
///
/// Output only: the request always sends `payment_method_type: "card"` and
/// PayNearMe decides `credit` vs `debit` from the BIN (spec §10.5). The
/// remaining variants belong to payment methods this integration does not offer
/// but which can still appear on a payment record read back by `/find_payment`,
/// so they are modelled rather than rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaynearmePaymentType {
    Ach,
    AchPush,
    Cash,
    CashApp,
    /// In scope — a credit card, classified from the BIN.
    Credit,
    /// In scope — a debit card, classified from the BIN.
    Debit,
    Paypal,
    #[serde(rename = "paypal-push")]
    PaypalPush,
    Pin4,
    #[serde(rename = "push-debit")]
    PushDebit,
    Venmo,
    #[serde(rename = "venmo-push")]
    VenmoPush,
    /// Anything PayNearMe adds later. Never fail a response over an
    /// informational field.
    #[serde(other)]
    Unknown,
}

/// As [`PaynearmePaymentType`], but a non-string JSON value degrades to `None`
/// instead of failing the body — the field it guards previously went through
/// [`deserialize_string_or_number`] and that tolerance is deliberately kept.
fn deserialize_optional_payment_type<'de, D>(
    deserializer: D,
) -> Result<Option<PaynearmePaymentType>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(payment_type @ serde_json::Value::String(_)) => Some(
            PaynearmePaymentType::deserialize(payment_type)
                .unwrap_or(PaynearmePaymentType::Unknown),
        ),
        _ => None,
    })
}

/// `payment_status`. The OpenAPI enum declares `canceled`; the `/cancel_payment`
/// example returns `cancelled`. Both spellings must deserialise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaynearmePaymentStatus {
    Approved,
    Authorized,
    #[serde(alias = "cancelled")]
    Canceled,
    Refunded,
    Rejected,
    #[serde(rename = "waiting for review")]
    WaitingForReview,
    #[serde(other)]
    Unknown,
}

impl PaynearmePaymentStatus {
    /// Never guess a terminal state: anything undocumented resolves to `Pending`
    /// and is settled by PSync.
    fn attempt_status(&self) -> AttemptStatus {
        match self {
            // Terminal success. PayNearMe card payments are sale / auto-capture.
            Self::Approved => AttemptStatus::Charged,
            // Emitted as-is; there is no capture endpoint to move it forward.
            Self::Authorized => AttemptStatus::Authorized,
            Self::Canceled => AttemptStatus::Voided,
            // The *payment* succeeded; the refund is tracked as a RefundStatus.
            Self::Refunded => AttemptStatus::Charged,
            Self::Rejected => AttemptStatus::Failure,
            Self::WaitingForReview | Self::Unknown => AttemptStatus::Pending,
        }
    }
}

/// `refund.refund_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaynearmeRefundStatus {
    /// Accepted; funds take 5+ banking days to reach the consumer.
    Started,
    Completed,
    #[serde(other)]
    Unknown,
}

impl PaynearmeRefundStatus {
    fn refund_status(&self) -> RefundStatus {
        match self {
            Self::Completed => RefundStatus::Success,
            Self::Started | Self::Unknown => RefundStatus::Pending,
        }
    }
}

/// The nested refund object. PayNearMe returns no refund identifier, so this is
/// the only refund state the API exposes and it hangs off the payment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeRefundObject {
    pub refund_status: Option<PaynearmeRefundStatus>,
    #[serde(default, deserialize_with = "deserialize_optional_string_major_unit")]
    pub refund_amount: Option<StringMajorUnit>,
    #[serde(default, deserialize_with = "deserialize_optional_currency")]
    pub refund_currency: Option<Currency>,
}

/// The Payments object, returned by `/find_payment`, `/cancel_payment`,
/// `/refund_payment`, and nested inside the order of `/create_payment_method`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmePayment {
    pub payment_status: Option<PaynearmePaymentStatus>,
    /// The connector transaction id — the key for PSync, Void, Refund and RSync.
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub pnm_payment_identifier: Option<String>,
    /// The card token, retained in `connector_metadata` so a later
    /// `/make_payment` against the saved instrument stays possible.
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub payment_method_identifier: Option<String>,
    /// Total charged, **including** PayNearMe's convenience fee.
    #[serde(default, deserialize_with = "deserialize_optional_string_major_unit")]
    pub payment_amount: Option<StringMajorUnit>,
    /// Merchant settlement amount (payment minus fees) — not the captured amount.
    #[serde(default, deserialize_with = "deserialize_optional_string_major_unit")]
    pub net_payment_amount: Option<StringMajorUnit>,
    #[serde(default, deserialize_with = "deserialize_optional_payment_type")]
    pub payment_type: Option<PaynearmePaymentType>,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub site_payment_identifier: Option<String>,
    pub refund: Option<PaynearmeRefundObject>,
}

impl PaynearmePayment {
    fn attempt_status(&self) -> AttemptStatus {
        self.payment_status
            .as_ref()
            .map(PaynearmePaymentStatus::attempt_status)
            .unwrap_or(AttemptStatus::Pending)
    }

    fn resource_id(&self) -> Option<ResponseId> {
        self.pnm_payment_identifier
            .clone()
            .map(ResponseId::ConnectorTransactionId)
    }

    fn connector_metadata(&self) -> Option<serde_json::Value> {
        self.connector_metadata_with_order(None)
    }

    /// Retains the card token so a later `/make_payment` against the saved
    /// instrument stays possible, plus the order it was created against when the
    /// response carried one (`/create_payment_method` does; `/find_payment` and
    /// the post-authorization endpoints do not).
    fn connector_metadata_with_order(
        &self,
        pnm_order_identifier: Option<&String>,
    ) -> Option<serde_json::Value> {
        if self.payment_method_identifier.is_none() && pnm_order_identifier.is_none() {
            return None;
        }
        Some(serde_json::json!({
            "payment_method_identifier": self.payment_method_identifier,
            "pnm_order_identifier": pnm_order_identifier,
        }))
    }
}

/// `electronic_payments` — where the ACH example puts `payments[]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeElectronicPayments {
    #[serde(default)]
    pub payments: Option<Vec<PaynearmePayment>>,
}

/// The Orders object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeOrder {
    /// The connector order id.
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub pnm_order_identifier: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub order_status: Option<String>,
    /// The card example nests the created payment here …
    #[serde(default)]
    pub payments: Option<Vec<PaynearmePayment>>,
    /// … while the ACH example nests it one level deeper.
    pub electronic_payments: Option<PaynearmeElectronicPayments>,
}

impl PaynearmeOrder {
    /// The payment this order carries, as read back from a `/create_order` or
    /// `/create_payment_method` response.
    ///
    /// **Which envelope it reads.** PayNearMe nests `payments[]` in two
    /// different places depending on the example: the card flow puts it directly
    /// on the order (`order.payments[]`), while the ACH flow puts it one level
    /// deeper, under `order.electronic_payments.payments[]`. Neither location is
    /// documented as canonical, so both are tried — `payments` first, then
    /// `electronic_payments.payments` — and the first **non-empty** array wins.
    /// The emptiness check matters: an order that serialises `"payments": []`
    /// alongside a populated `electronic_payments` would otherwise resolve to
    /// the empty array and report no payment at all.
    ///
    /// **Why `.last()`.** Authorize sends `last_pmt_only: "true"`
    /// ([`LAST_PMT_ONLY_TRUE`]), so the array is expected to hold exactly the
    /// one payment the call just created and `.last()` is simply "that one".
    /// Should PayNearMe ignore the flag, or should the order already carry
    /// earlier attempts, the array is in chronological order and the most recent
    /// entry is the one this request produced — taking `.first()` there would
    /// report the status of an older, unrelated attempt.
    ///
    /// **When it returns `None`.** No `payments` key and no
    /// `electronic_payments.payments` key; or both present but empty. That is
    /// the "tokenised but not charged" shape, and the Authorize response handler
    /// deliberately maps it to `Pending` with the order id as the resource id so
    /// PSync can resolve it, rather than claiming `Charged`.
    fn last_payment(&self) -> Option<&PaynearmePayment> {
        self.payments
            .as_ref()
            .filter(|payments| !payments.is_empty())
            .or_else(|| {
                self.electronic_payments
                    .as_ref()
                    .and_then(|electronic| electronic.payments.as_ref())
                    .filter(|payments| !payments.is_empty())
            })
            .and_then(|payments| payments.last())
    }
}

/// An `errors[]` entry is either a plain string or an object with a
/// `description` field; both shapes are documented.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PaynearmeErrorItem {
    Message(String),
    Detailed { description: String },
}

impl PaynearmeErrorItem {
    fn render(&self) -> String {
        match self {
            Self::Message(message) => message.clone(),
            Self::Detailed { description } => description.clone(),
        }
    }
}

/// The documented `400` body. Also parsed defensively on the undocumented
/// failure modes (bad signature, expired key, rate limiting, 5xx).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeErrorResponse {
    pub status: Option<String>,
    #[serde(default)]
    pub errors: Vec<PaynearmeErrorItem>,
    pub response_code: Option<String>,
}

impl PaynearmeErrorResponse {
    pub fn to_error_response(&self, status_code: u16, flow_status: FlowStatus) -> ErrorResponse {
        build_error_response(
            status_code,
            flow_status,
            self.response_code.as_deref(),
            &self.errors,
            None,
        )
    }
}

/// Shared `ErrorResponse` construction: `code` is the numeric `response_code`
/// when PayNearMe supplied a non-success one, `message` is the first rendered
/// `errors[]` entry and `reason` joins them all.
fn build_error_response(
    status_code: u16,
    flow_status: FlowStatus,
    response_code: Option<&str>,
    errors: &[PaynearmeErrorItem],
    connector_transaction_id: Option<String>,
) -> ErrorResponse {
    let rendered: Vec<String> = errors.iter().map(PaynearmeErrorItem::render).collect();
    let code = response_code
        .filter(|code| *code != RESPONSE_CODE_SUCCESS)
        .map(str::to_string)
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
    let message = rendered
        .first()
        .cloned()
        .unwrap_or_else(|| "Payment declined by Paynearme".to_string());
    let reason = if rendered.is_empty() {
        None
    } else {
        Some(rendered.join("; "))
    };

    ErrorResponse {
        status_code,
        code,
        message,
        reason,
        attempt_status: Some(flow_status),
        connector_transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Success is a three-way test, because a decline can arrive as HTTP 400, as
/// `status: "error"`, as `response_code != "0"`, or as
/// `payment_status: "rejected"` on an otherwise fine `201`.
fn envelope_is_ok(status: Option<&str>, response_code: Option<&str>) -> bool {
    let status_ok = status
        .map(|value| value == ENVELOPE_STATUS_OK)
        .unwrap_or(true);
    let code_ok = response_code
        .map(|value| value == RESPONSE_CODE_SUCCESS)
        .unwrap_or(true);
    status_ok && code_ok
}

// =============================================================================
// CREATE ORDER RESPONSE
// =============================================================================

/// `/create_order` answers with the top-level key `orders` (plural), while
/// `/create_payment_method` answers with `order` (singular). Alias so one struct
/// covers both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeOrderEnvelope {
    pub status: Option<String>,
    pub response_code: Option<String>,
    #[serde(alias = "order")]
    pub orders: Option<PaynearmeOrder>,
    #[serde(default)]
    pub errors: Vec<PaynearmeErrorItem>,
}

impl PaynearmeOrderEnvelope {
    fn is_ok(&self) -> bool {
        envelope_is_ok(self.status.as_deref(), self.response_code.as_deref())
    }
}

/// `/create_order` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaynearmeCreateOrderResponse(pub PaynearmeOrderEnvelope);

impl TryFrom<ResponseRouterData<PaynearmeCreateOrderResponse, Self>> for CreateOrderRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaynearmeCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let connector_order_id = response
            .is_ok()
            .then(|| {
                response
                    .orders
                    .as_ref()
                    .and_then(|order| order.pnm_order_identifier.clone())
            })
            .flatten();

        match connector_order_id {
            Some(connector_order_id) => Ok(Self {
                response: Ok(PaymentCreateOrderResponse {
                    connector_order_id: connector_order_id.clone(),
                    session_data: None,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Pending,
                    reference_id: Some(connector_order_id.clone()),
                    connector_order_id: Some(connector_order_id),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
            None => Ok(Self {
                response: Err(build_error_response(
                    item.http_code,
                    FlowStatus::Payment(AttemptStatus::Failure),
                    response.response_code.as_deref(),
                    &response.errors,
                    None,
                )),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

// =============================================================================
// AUTHORIZE RESPONSE
// =============================================================================

/// `/create_payment_method` response — the created payment is nested inside the
/// order payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaynearmeAuthorizeResponse(pub PaynearmeOrderEnvelope);

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaynearmeAuthorizeResponse, Self>>
    for AuthorizeRouterData<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaynearmeAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let order = response.orders.as_ref();
        let payment = order.and_then(PaynearmeOrder::last_payment);
        let order_identifier = order.and_then(|order| order.pnm_order_identifier.clone());

        let declined = !response.is_ok()
            || payment
                .and_then(|payment| payment.payment_status.as_ref())
                .map(|status| *status == PaynearmePaymentStatus::Rejected)
                .unwrap_or(false);

        if declined {
            return Ok(Self {
                response: Err(build_error_response(
                    item.http_code,
                    FlowStatus::Payment(AttemptStatus::Failure),
                    response.response_code.as_deref(),
                    &response.errors,
                    payment.and_then(|payment| payment.pnm_payment_identifier.clone()),
                )),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // `payments[]` absent or empty on an otherwise-ok response means the card
        // was tokenised but not charged; report Pending and let PSync resolve it
        // rather than claiming Charged.
        let (status, resource_id, connector_metadata) = match payment {
            // A money-moved status is only trustworthy if it comes with an
            // identifier to reconcile against. `pnm_payment_identifier` is
            // absent — or an empty string, which `deserialize_string_or_number`
            // also reports as absent — on a malformed 201, and pairing
            // `Charged` with `NoResponseId` would record an unreconcilable,
            // unrefundable charge that PSync, Void and Refund can never key on.
            // Fall back to the order id and report Pending so PSync can resolve
            // it, exactly as the `None` arm below already does.
            Some(payment) => match payment.resource_id() {
                Some(resource_id) => (
                    payment.attempt_status(),
                    resource_id,
                    payment.connector_metadata_with_order(order_identifier.as_ref()),
                ),
                None => (
                    AttemptStatus::Pending,
                    order_identifier
                        .clone()
                        .map(ResponseId::ConnectorTransactionId)
                        .unwrap_or(ResponseId::NoResponseId),
                    payment.connector_metadata_with_order(order_identifier.as_ref()),
                ),
            },
            None => (
                AttemptStatus::Pending,
                order_identifier
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                None,
            ),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                // Never: PayNearMe has no 3DS and no redirect step at all.
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: order_identifier,
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
// PAYMENT-ENVELOPE RESPONSES (PSync, Void, Refund, RSync)
// =============================================================================

/// The `{"status": "...", "payment": { … }}` envelope returned by
/// `/find_payment`, `/cancel_payment` and `/refund_payment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmePaymentEnvelope {
    pub status: Option<String>,
    pub response_code: Option<String>,
    pub payment: Option<PaynearmePayment>,
    #[serde(default)]
    pub errors: Vec<PaynearmeErrorItem>,
}

impl PaynearmePaymentEnvelope {
    fn is_ok(&self) -> bool {
        envelope_is_ok(self.status.as_deref(), self.response_code.as_deref())
    }

    fn transaction_id(&self) -> Option<String> {
        self.payment
            .as_ref()
            .and_then(|payment| payment.pnm_payment_identifier.clone())
    }
}

/// PSync response — `POST /find_payment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaynearmeSyncResponse(pub PaynearmePaymentEnvelope);

impl TryFrom<ResponseRouterData<PaynearmeSyncResponse, Self>> for SyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaynearmeSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let payment = match (response.is_ok(), response.payment.as_ref()) {
            (true, Some(payment)) => payment,
            _ => {
                return Ok(Self {
                    response: Err(build_error_response(
                        item.http_code,
                        FlowStatus::Payment(AttemptStatus::Pending),
                        response.response_code.as_deref(),
                        &response.errors,
                        response.transaction_id(),
                    )),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Pending,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        };

        let status = payment.attempt_status();
        let resource_id = payment
            .resource_id()
            .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: payment.connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: payment.site_payment_identifier.clone(),
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

/// Void response — `POST /cancel_payment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaynearmeVoidResponse(pub PaynearmePaymentEnvelope);

impl TryFrom<ResponseRouterData<PaynearmeVoidResponse, Self>> for VoidRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaynearmeVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;
        let payment = response.payment.as_ref();

        // A `status: "ok"` that still reports `approved` means the cancellation
        // window had already closed and PayNearMe did not cancel anything —
        // that is a VoidFailed, not a success.
        let cancelled = response.is_ok()
            && payment
                .and_then(|payment| payment.payment_status.as_ref())
                .map(|status| *status == PaynearmePaymentStatus::Canceled)
                .unwrap_or(false);

        if !cancelled {
            return Ok(Self {
                response: Err(build_error_response(
                    item.http_code,
                    FlowStatus::Payment(AttemptStatus::VoidFailed),
                    response.response_code.as_deref(),
                    &response.errors,
                    response.transaction_id(),
                )),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: payment.and_then(PaynearmePayment::connector_metadata),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: payment
                    .and_then(|payment| payment.site_payment_identifier.clone()),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// Refund response — `POST /refund_payment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaynearmeRefundResponse(pub PaynearmePaymentEnvelope);

impl TryFrom<ResponseRouterData<PaynearmeRefundResponse, Self>> for RefundRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaynearmeRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let payment = match (response.is_ok(), response.payment.as_ref()) {
            (true, Some(payment)) => payment,
            _ => {
                return Ok(Self {
                    response: Err(build_error_response(
                        item.http_code,
                        FlowStatus::Refund(RefundStatus::Pending),
                        response.response_code.as_deref(),
                        &response.errors,
                        response.transaction_id(),
                    )),
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Pending,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        };

        // PayNearMe mints no refund identifier, so the payment id doubles as
        // `connector_refund_id` — which is exactly what RSync needs to key on.
        let connector_refund_id = payment
            .pnm_payment_identifier
            .clone()
            .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone());

        // `refund` absent on an ok response means the refund was accepted but is
        // not visible yet; report Pending and let RSync resolve it.
        let refund_status = payment
            .refund
            .as_ref()
            .and_then(|refund| refund.refund_status.as_ref())
            .map(PaynearmeRefundStatus::refund_status)
            .unwrap_or(RefundStatus::Pending);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
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

/// RSync response — the same `/find_payment` envelope, read for its nested
/// `refund` object.
///
/// Limitation: `payment.refund` is a single object, so a payment carrying
/// multiple partial refunds exposes only one refund state through this API and
/// concurrent partial refunds cannot be disambiguated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaynearmeRefundSyncResponse(pub PaynearmePaymentEnvelope);

impl TryFrom<ResponseRouterData<PaynearmeRefundSyncResponse, Self>> for RefundSyncRouterData {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaynearmeRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        let payment = match (response.is_ok(), response.payment.as_ref()) {
            (true, Some(payment)) => payment,
            _ => {
                return Ok(Self {
                    response: Err(build_error_response(
                        item.http_code,
                        FlowStatus::Refund(RefundStatus::Pending),
                        response.response_code.as_deref(),
                        &response.errors,
                        response.transaction_id(),
                    )),
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Pending,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        };

        let refund_status = payment
            .refund
            .as_ref()
            .and_then(|refund| refund.refund_status.as_ref())
            .map(PaynearmeRefundStatus::refund_status)
            .unwrap_or(RefundStatus::Pending);

        let connector_refund_id = payment
            .pnm_payment_identifier
            .clone()
            .unwrap_or_else(|| item.router_data.request.connector_refund_id.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
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
