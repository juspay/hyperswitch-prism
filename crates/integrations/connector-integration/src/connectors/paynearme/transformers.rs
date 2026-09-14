//! PayNearMe API v3.0 transformers.
//!
//! PayNearMe is an RPC-over-JSON gateway: every operation is a `POST` to
//! `{base_url}/{operation}` with `Content-Type: application/json`. There is no
//! `Authorization` header — each request carries a per-request HMAC-SHA256
//! `signature` field computed over the alphabetically sorted, `key + value`
//! concatenation of the body it is about to be sent with (see
//! [`paynearme_signature`]).
//!
//! Scope of this module: Card — one-time payments, plus storing a card for later
//! merchant-initiated use.
//! * `CreateOrder` -> `POST /create_order` (a standing order on opt-in, see
//!   [`PaynearmeCreateOrderFeatureData`])
//! * `Authorize`   -> `POST /create_payment_method` with `send_payment=true`
//!   (tokenises the card and charges it in one round trip)
//! * `SetupMandate` -> `POST /create_payment_method` without `send_payment`
//!   (tokenises only; the token becomes the mandate reference, see
//!   [`PaynearmeMandateReference`])
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
    connector_flow::{Authorize, CreateOrder, PSync, RSync, Refund, SetupMandate, Void},
    connector_types::{
        MandateReference, PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
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
/// `order_type` for a standing order that holds a stored card: later
/// merchant-initiated charges need not equal the order amount. This is the value
/// PayNearMe's own standing-order recipe pairs with `order_is_standing="true"`.
const ORDER_TYPE_ANY: &str = "any";
/// A one-time order: paid once, then done.
const ORDER_IS_STANDING_FALSE: &str = "false";
/// "If the order can be repeatedly paid for, enter `true`" — required for an order
/// a stored card will be charged against more than once.
const ORDER_IS_STANDING_TRUE: &str = "true";
/// Hyperswitch persists `connector_mandate_id` in a `VARCHAR(128)` column. A
/// longer reference is refused rather than truncated: a truncated reference would
/// be stored as valid and fail on the first merchant-initiated charge.
const CONNECTOR_MANDATE_ID_MAX_LEN: usize = 128;
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
    common_utils::date_time::now_unix_timestamp().to_string()
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

/// Opt-in settings for `/create_order`, read from `connector_feature_data`:
/// `{"order_is_standing": true, "site_customer_identifier": "<customer id>"}`.
///
/// Why this exists: CreateOrder has no other way to learn that the order will hold
/// a stored card. `PaymentCreateOrderData` carries no setup-future-usage signal,
/// and `PaymentServiceCreateOrderRequest` carries no customer id
/// (`PaymentFlowData.customer_id` is always `None` on this flow). Yet an order for
/// a SetupMandate needs two things a one-time order does not:
/// * `order_is_standing="true"` ("If the order can be repeatedly paid for"), so
///   RepeatPayment can `/make_payment` against it more than once, with
///   `order_type="any"` because those charges need not equal the order amount;
/// * a stable `site_customer_identifier`, because PayNearMe links a stored card to
///   the order's customer, and the per-order fallback a one-time order uses would
///   mint a fresh PayNearMe customer for every mandate.
///
/// When absent, the order is the same one-time `exact` order as before.
/// SetupMandate checks both properties on its response and fails otherwise.
#[derive(Debug, Default, Deserialize)]
pub struct PaynearmeCreateOrderFeatureData {
    #[serde(default)]
    pub order_is_standing: bool,
    #[serde(default)]
    pub site_customer_identifier: Option<String>,
}

impl TryFrom<&PaymentFlowData> for PaynearmeCreateOrderFeatureData {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(common: &PaymentFlowData) -> Result<Self, Self::Error> {
        match &common.connector_feature_data {
            None => Ok(Self::default()),
            // These settings decide what goes on the wire, so a value that does
            // not parse is an error rather than a silent one-time order.
            Some(feature_data) => {
                serde_json::from_value(feature_data.clone().expose()).map_err(|error| {
                    error_stack::report!(IntegrationError::InvalidDataFormat {
                        field_name: "connector_feature_data",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Send a JSON object such as {\"order_is_standing\": true, \
                                 \"site_customer_identifier\": \"cus_123\"}"
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(format!(
                                "PayNearMe CreateOrder settings did not parse: {error}"
                            )),
                        },
                    })
                })
            }
        }
    }
}

/// `/create_order` request. "With PayNearMe, an order is required any time money
/// moves or is scheduled to move", so this runs ahead of every Authorize, and a
/// caller runs it ahead of SetupMandate (passing the id on as `order_id`).
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

        let PaynearmeCreateOrderFeatureData {
            order_is_standing: standing,
            site_customer_identifier: feature_customer_identifier,
        } = PaynearmeCreateOrderFeatureData::try_from(common)?;

        // A customer id the caller actually supplied: `connector_feature_data`
        // first (CreateOrder requests carry no `customer.id`), then the flow's own
        // customer id should one ever be threaded through.
        let supplied_customer_identifier = feature_customer_identifier
            .filter(|identifier| !identifier.is_empty())
            .or_else(|| {
                common
                    .customer_id
                    .as_ref()
                    .map(|customer_id| customer_id.get_string_repr().to_string())
            });

        let (order_type, order_is_standing, site_customer_identifier) = if standing {
            // A standing order holds a stored card for one customer; see
            // `PaynearmeCreateOrderFeatureData`. No per-order fallback here.
            let site_customer_identifier = supplied_customer_identifier.ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.site_customer_identifier",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the stable customer id (the same value later sent as \
                             customer.id on SetupRecurring) as \
                             connector_feature_data.site_customer_identifier"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "A standing PayNearMe order holds stored cards for one customer"
                                .to_string(),
                        ),
                    },
                })
            })?;
            (
                ORDER_TYPE_ANY,
                ORDER_IS_STANDING_TRUE,
                site_customer_identifier,
            )
        } else {
            // `site_customer_identifier` is required and is a client-created unique
            // string; a one-time order falls back to the attempt reference when no
            // customer is attached.
            (
                ORDER_TYPE_EXACT,
                ORDER_IS_STANDING_FALSE,
                supplied_customer_identifier
                    .unwrap_or_else(|| common.connector_request_reference_id.clone()),
            )
        };

        let mut request = Self {
            site_identifier: auth.site_identifier,
            timestamp: current_timestamp(),
            version: PAYNEARME_API_VERSION.to_string(),
            signature: Secret::new(String::new()),
            order_amount,
            order_currency,
            site_customer_identifier,
            order_type: order_type.to_string(),
            order_is_standing: order_is_standing.to_string(),
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
#[derive(Debug, Serialize)]
pub struct PaynearmeAuthorizeRequest {
    pub site_identifier: Secret<String>,
    pub timestamp: String,
    pub version: String,
    pub signature: Secret<String>,
    /// The order created by the `CreateOrder` flow.
    pub pnm_order_identifier: String,
    #[serde(flatten)]
    pub card: PaynearmeCardPaymentMethod,
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

/// The card and billing fields of `/create_payment_method`, shared by Authorize
/// (tokenise and charge) and SetupMandate (tokenise only). Both calls take the same
/// fields from the same sources, so they are built in one place. Flattened into
/// each request, they serialise, and therefore sign, exactly as separate fields
/// would.
///
/// The credit-card and debit-card `oneOf` variants of the endpoint are
/// field-identical (both `payment_method_type: "card"`); PayNearMe decides which it
/// is from the BIN.
#[derive(Debug, Serialize)]
pub struct PaynearmeCardPaymentMethod {
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
}

impl<T: PaymentMethodDataTypes> TryFrom<(&Card<T>, &PaymentFlowData)>
    for PaynearmeCardPaymentMethod
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((card, common): (&Card<T>, &PaymentFlowData)) -> Result<Self, Self::Error> {
        Ok(Self {
            payment_method_type: PAYMENT_METHOD_TYPE_CARD.to_string(),
            payment_method_card_number_pii: Secret::new(card.card_number.peek().to_string()),
            payment_method_card_expiry_pii: card_expiry_mm_yyyy(card)?,
            payment_method_cvv_pii: card.card_cvc.clone(),
            // `payment_method_billing_name` is a required field on
            // `/create_payment_method`, so the billing full name is required here.
            // It is read straight off the billing address — no cardholder-name
            // fallback — so a caller that sends neither gets a precise
            // missing-field error instead of a gateway 400.
            payment_method_billing_name: common.get_billing_full_name()?,
            payment_method_billing_address: common.get_billing_line1()?,
            payment_method_billing_zipcode: common.get_billing_zip()?,
            // Documented as required. It can be made optional per site, but
            // surfacing the gap here beats a 400 from the gateway.
            //
            // The bare national number, **not** `get_billing_phone_number()`:
            // that helper prefixes the country code including the `+`
            // (`payment_address.rs:380`), producing `+14695555878`, while
            // PayNearMe's own worked example for this field is
            // `"469-555-5878"` (§8.2.1) — a US national number with no
            // country code. `PhoneDetails::get_number()`
            // (`payment_address.rs:375`) is the framework accessor for it.
            payment_method_billing_phone: common.get_billing_phone()?.get_number()?,
        })
    }
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

        // Storing a card is SetupMandate's job (tokenise-only
        // `/create_payment_method`), and charging a stored card is RepeatPayment's,
        // which is still `not_implemented`. This Authorize never reads
        // `mandate_id` / `setup_mandate_details` / `setup_future_usage` and never
        // returns a `mandate_reference`, so without this guard an off-session or
        // credential-storing authorize
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

        let card = PaynearmeCardPaymentMethod::try_from((card, common))?;

        Ok({
            let mut built = Self {
                site_identifier: auth.site_identifier,
                timestamp: current_timestamp(),
                version: PAYNEARME_API_VERSION.to_string(),
                signature: Secret::new(String::new()),
                pnm_order_identifier,
                card,
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
// SETUP MANDATE — `POST /create_payment_method`, tokenise only
// =============================================================================

/// `/create_payment_method` request, card variant, **without** `send_payment`:
/// PayNearMe tokenises the card (running issuer AVS/CVV validation) and charges
/// nothing. The `payment_method_identifier` it creates is what a later
/// `/make_payment` charges.
///
/// Optional fields deliberately not sent:
/// * `send_payment` / `payment_amount` / `payment_currency`: nothing is charged,
///   and a non-zero setup amount is refused (see the `TryFrom`).
/// * `site_channel` / `pricing_schedule_name`: both describe the channel of a
///   *payment* ("The payment channel where this payment was created"), and a
///   tokenise-only call creates none. The recurring channel belongs on the
///   `/make_payment` that RepeatPayment sends.
/// * `return_minimal_info`: it would strip the `accounts[]` the token is read from.
#[derive(Debug, Serialize)]
pub struct PaynearmeSetupMandateRequest {
    pub site_identifier: Secret<String>,
    pub timestamp: String,
    pub version: String,
    pub signature: Secret<String>,
    /// The standing order a prior CreateOrder created (opted in through
    /// [`PaynearmeCreateOrderFeatureData`]), passed to SetupRecurring as `order_id`.
    pub pnm_order_identifier: String,
    #[serde(flatten)]
    pub card: PaynearmeCardPaymentMethod,
}

type SetupMandateRouterData<T> =
    RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<PaynearmeRouterData<SetupMandateRouterData<T>, T>> for PaynearmeSetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaynearmeRouterData<SetupMandateRouterData<T>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let common = &router_data.resource_common_data;
        let request = &router_data.request;

        let card = match &request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card mandates are supported by paynearme".to_string(),
                    context(),
                )))
            }
        };

        // No 3-D Secure surface exists in the API; refused for the same reason
        // Authorize refuses it.
        if common.auth_type == AuthenticationType::ThreeDs || request.authentication_data.is_some()
        {
            return Err(not_supported("Three DS mandate setup"));
        }

        // Tokenise only. Charging a non-zero setup amount would need
        // `send_payment="true"` and would make this a payment, whose outcome this
        // flow does not report; ignoring the amount would leave the merchant
        // believing the card was charged. Refuse instead.
        if request
            .minor_amount
            .is_some_and(|amount| amount.get_amount_as_i64() != 0)
        {
            return Err(not_supported("SetupMandate with a non-zero amount"));
        }

        // PayNearMe binds a stored card to the customer of the order it is created
        // on, and the response is checked against this id. Require it before any
        // card data is sent.
        common.customer_id.as_ref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "customer.id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the stable customer id that was sent as \
                             connector_feature_data.site_customer_identifier on CreateOrder"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "A stored PayNearMe card belongs to the order's customer".to_string(),
                    ),
                },
            })
        })?;

        // `/create_payment_method` attaches the card to an existing order, and
        // SetupRecurring does not create one (the order-create pre-step only runs
        // ahead of Authorize), so the caller must supply it.
        let pnm_order_identifier = common.connector_order_id.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "order_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Call PaymentService/CreateOrder with connector_feature_data \
                         {\"order_is_standing\": true, \"site_customer_identifier\": \
                         \"<customer.id>\"} and pass its connector_order_id as order_id"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "PayNearMe stores a card against an existing order".to_string(),
                    ),
                },
            })
        })?;

        let auth = PaynearmeAuthType::try_from(&router_data.connector_config)?;
        let card = PaynearmeCardPaymentMethod::try_from((card, common))?;

        let mut built = Self {
            site_identifier: auth.site_identifier,
            timestamp: current_timestamp(),
            version: PAYNEARME_API_VERSION.to_string(),
            signature: Secret::new(String::new()),
            pnm_order_identifier,
            card,
        };
        built.signature = paynearme_signature(&auth.api_secret_key, &built)?;
        Ok(built)
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
            // PayNearMe reports `authorized` on a payment it has not settled
            // yet. It is **not** mapped to `AttemptStatus::Authorized`: that
            // state expects a capture to move it on, and this connector has no
            // capture (`supported_capture_methods` is `[Automatic]` and the
            // Capture flow is `not_implemented` in `paynearme.rs` — the API has
            // no capture endpoint at all, §8.7). An `Authorized` attempt would
            // therefore be a dead end that nothing can ever advance or reap.
            // `Pending` keeps PSync polling until PayNearMe moves the payment
            // to `approved` (or `rejected`), which is the only way it resolves.
            //
            // This is a deliberate deviation from the spec's status table
            // (§10.1, "emit as-is"), which is self-defeating on a connector
            // with no capture: the spec's own note for the row says the payment
            // "will settle on its own or be resolved by PSync", and `Pending`
            // is the status that lets PSync do exactly that.
            Self::Authorized => AttemptStatus::Pending,
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
    /// The generic HTTP-error path (`Paynearme::build_error_response` in
    /// `paynearme.rs`), which every flow reaches through
    /// `get_error_response_v2` for any non-2xx response that is not a 5xx.
    ///
    /// `attempt_status` is deliberately left `None` here — the same choice
    /// `ilixium.rs:190` makes, and the same thing the framework's own
    /// `get_5xx_error_response` default does
    /// (`interfaces/src/connector_integration_v2.rs:232`). This handler sees
    /// clock-skew 400s, signature rejections (§14.5 lists 401/403/404/429 as
    /// undocumented but expected) and rate limits: none of them carries a
    /// verdict on the payment. Forcing a status here would report an already
    /// CHARGED payment as `Failure` the first time a PSync hit a 429, and —
    /// because `ForeignFrom<FlowStatus> for RefundStatus` maps `Payment(_)` to
    /// `RefundFailure` (`domain_types/src/types.rs:7817`) — would mark a live
    /// refund terminally failed. Leaving it `None` keeps whatever status the
    /// payment already had.
    ///
    /// The in-band handlers below, which have read an actual PayNearMe verdict
    /// off a 2xx body, still pass an explicit status. §11.4 scopes
    /// `attempt_status := Some(Failure)` to exactly those ("for declines on
    /// Authorize").
    pub fn to_error_response(&self, status_code: u16) -> ErrorResponse {
        build_error_response(
            status_code,
            None,
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
    flow_status: Option<FlowStatus>,
    response_code: Option<&str>,
    errors: &[PaynearmeErrorItem],
    connector_transaction_id: Option<String>,
) -> ErrorResponse {
    let rendered: Vec<String> = errors.iter().map(PaynearmeErrorItem::render).collect();
    let code = response_code
        .filter(|code| *code != RESPONSE_CODE_SUCCESS)
        .map(str::to_string)
        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
    let message = rendered.first().cloned().unwrap_or_else(|| {
        match response_code.filter(|code| *code != RESPONSE_CODE_SUCCESS) {
            // A non-zero `response_code` *is* PayNearMe's verdict on the card
            // (§11.3: every code but `0` is a decline reason), so naming it a
            // decline is accurate even when `errors[]` was empty.
            Some(code) => format!("Payment declined by Paynearme (response_code {code})"),
            // Neither an `errors[]` entry nor a `response_code`: PayNearMe
            // returned no verdict at all, which is what a signature rejection,
            // a rate limit or a gateway error looks like (§14.5). Calling that
            // a decline sends the merchant chasing the cardholder for what is
            // an integration or availability problem, so report the transport
            // condition instead.
            None => format!("Paynearme request failed with HTTP status {status_code}"),
        }
    });
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
        attempt_status: flow_status,
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
    /// With `return_minimal_info="true"`, which CreateOrder sends, `/create_order`
    /// answers `{status, pnm_order_identifier, pnm_customer_identifier}`: the
    /// order id sits at the top level and there is no `orders` object at all.
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub pnm_order_identifier: Option<String>,
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
                // The minimal shape is what `return_minimal_info` asks for; the
                // full `orders` object is the documented shape without it.
                response.pnm_order_identifier.clone().or_else(|| {
                    response
                        .orders
                        .as_ref()
                        .and_then(|order| order.pnm_order_identifier.clone())
                })
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
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
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
        // The order this Authorize charged against: read back off the response
        // when PayNearMe echoed it, otherwise the id we sent as
        // `pnm_order_identifier` (written by the CreateOrder flow). Both are the
        // same value; the fallback only covers a response that omits it.
        let order_identifier = order
            .and_then(|order| order.pnm_order_identifier.clone())
            .or_else(|| {
                item.router_data
                    .resource_common_data
                    .connector_order_id
                    .clone()
            });

        let declined = !response.is_ok()
            || payment
                .and_then(|payment| payment.payment_status.as_ref())
                .map(|status| *status == PaynearmePaymentStatus::Rejected)
                .unwrap_or(false);

        if declined {
            return Ok(Self {
                response: Err(build_error_response(
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
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
                // `pnm_order_identifier`, per §7.3. PSync and Void report the
                // same value (see below), so one payment has exactly one
                // reference id whichever flow last spoke to PayNearMe.
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
// SETUP MANDATE RESPONSE
// =============================================================================

/// What `connector_mandate_id` carries for a card stored at PayNearMe.
///
/// A merchant-initiated charge (`/make_payment`, the RepeatPayment flow) needs two
/// PayNearMe identifiers: the card token (`payment_method_identifier`) and an order
/// to charge it against (`pnm_order_identifier`, the standing order the card was
/// stored on). The RecurringCharge request carries no connector order id
/// (`PaymentFlowData.connector_order_id` is `None` on that path), so both have to
/// travel inside the mandate reference. They travel together in
/// `connector_mandate_id` because:
/// * it is the mandate field every caller persists and sends back, and what
///   RepeatPayment reads through `get_connector_mandate_id()`;
///   `connector_mandate_request_reference_id` means something else (a
///   merchant-side request reference) and is not guaranteed to be round-tripped;
/// * a token without its order cannot be charged, so splitting the two across
///   fields only adds a way to receive one without the other.
///
/// The keys are three letters so the JSON stays well inside
/// [`CONNECTOR_MANDATE_ID_MAX_LEN`]: with the documented id shapes (a 13-character
/// token, an 11-digit order) it is 44 characters.
#[derive(Debug, Serialize, Deserialize)]
pub struct PaynearmeMandateReference {
    #[serde(rename = "pmi")]
    pub payment_method_identifier: Secret<String>,
    #[serde(rename = "oid")]
    pub pnm_order_identifier: String,
}

impl PaynearmeMandateReference {
    /// Serialises the reference, refusing (never truncating) one that would not fit
    /// Hyperswitch's `connector_mandate_id` column.
    fn to_connector_mandate_id(&self) -> Result<String, String> {
        let encoded = serde_json::to_string(self).map_err(|error| {
            format!("the PayNearMe mandate reference could not be encoded: {error}")
        })?;
        if encoded.len() > CONNECTOR_MANDATE_ID_MAX_LEN {
            return Err(format!(
                "the PayNearMe mandate reference is {} characters, but \
                 connector_mandate_id holds at most {CONNECTOR_MANDATE_ID_MAX_LEN}",
                encoded.len()
            ));
        }
        Ok(encoded)
    }
}

/// [`deserialize_string_or_number`], masked: stored-account fields are card data.
fn deserialize_optional_secret_string<'de, D>(
    deserializer: D,
) -> Result<Option<Secret<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_string_or_number(deserializer).map(|value| value.map(Secret::new))
}

/// `true` / `false` as either a JSON boolean or a string: `order_is_standing` is
/// `true` in one documented `/create_payment_method` example and `"true"` in the
/// other. Anything else reads as unknown.
fn deserialize_optional_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(serde_json::Value::Bool(flag)) => Some(flag),
        Some(serde_json::Value::String(text)) => match text.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

/// `accounts[].status` of a stored payment method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaynearmeAccountStatus {
    Active,
    /// Deactivated through `/update_payment_method`: permanently unusable.
    Inactive,
    #[serde(other)]
    Unknown,
}

/// `electronic_payments.payment_methods[].type`, the bucket stored accounts are
/// listed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaynearmeStoredMethodType {
    Ach,
    AchPush,
    ApplePayCredit,
    ApplePayDebit,
    CashApp,
    Credit,
    Debit,
    Gpay,
    Paypal,
    PaypalPush,
    PushDebit,
    Venmo,
    VenmoPush,
    #[serde(other)]
    Unknown,
}

impl PaynearmeStoredMethodType {
    /// A card sent to `/create_payment_method` is listed under `credit` or `debit`,
    /// as PayNearMe classifies it from the BIN.
    fn holds_cards(&self) -> bool {
        match self {
            Self::Credit | Self::Debit => true,
            Self::Ach
            | Self::AchPush
            | Self::ApplePayCredit
            | Self::ApplePayDebit
            | Self::CashApp
            | Self::Gpay
            | Self::Paypal
            | Self::PaypalPush
            | Self::PushDebit
            | Self::Venmo
            | Self::VenmoPush
            | Self::Unknown => false,
        }
    }
}

/// One stored account, reduced to what identifies a card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeStoredAccount {
    /// The token `/make_payment` charges.
    #[serde(default, deserialize_with = "deserialize_optional_secret_string")]
    pub payment_method_identifier: Option<Secret<String>>,
    pub status: Option<PaynearmeAccountStatus>,
    /// The card's last four digits.
    #[serde(default, deserialize_with = "deserialize_optional_secret_string")]
    pub number: Option<Secret<String>>,
    /// `MM/YYYY`.
    #[serde(default, deserialize_with = "deserialize_optional_secret_string")]
    pub expiration_date: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeStoredPaymentMethods {
    #[serde(rename = "type")]
    pub method_type: Option<PaynearmeStoredMethodType>,
    #[serde(default)]
    pub accounts: Option<Vec<PaynearmeStoredAccount>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeStoredElectronicPayments {
    #[serde(default)]
    pub payment_methods: Option<Vec<PaynearmeStoredPaymentMethods>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeOrderCustomer {
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub site_customer_identifier: Option<String>,
}

/// The `order` of a `/create_payment_method` response, reduced to what SetupMandate
/// reads. Kept apart from [`PaynearmeOrder`] so stored-account parsing cannot put
/// the Authorize and CreateOrder responses at risk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeStoredCredentialOrder {
    #[serde(default, deserialize_with = "deserialize_optional_bool")]
    pub order_is_standing: Option<bool>,
    pub customer: Option<PaynearmeOrderCustomer>,
    pub electronic_payments: Option<PaynearmeStoredElectronicPayments>,
}

impl PaynearmeStoredCredentialOrder {
    /// Picks the token `/create_payment_method` just created.
    ///
    /// The response lists every stored account visible on the order ("Payment
    /// methods created for one order appear in other orders with the same Customer
    /// ID"), not only the new one, and nothing in it marks which account this call
    /// created. So the card is found by what identifies it: an `active` account
    /// under `credit` or `debit` whose last four digits and `MM/YYYY` expiry equal
    /// the card that was sent.
    ///
    /// Exactly one distinct token must match. None means the card was not stored,
    /// or not as `active`. More than one means the customer already holds a card
    /// with the same last four and expiry, and guessing between them could point
    /// every future charge at the wrong card. Both are errors.
    fn stored_card_token(
        &self,
        last4: &str,
        expiry_mm_yyyy: &str,
    ) -> Result<Secret<String>, String> {
        let mut matches: Vec<&Secret<String>> = Vec::new();
        let card_buckets = self
            .electronic_payments
            .iter()
            .flat_map(|electronic| electronic.payment_methods.iter().flatten())
            .filter(|bucket| {
                bucket
                    .method_type
                    .as_ref()
                    .is_some_and(PaynearmeStoredMethodType::holds_cards)
            });
        for account in card_buckets.flat_map(|bucket| bucket.accounts.iter().flatten()) {
            let same_card = account.status == Some(PaynearmeAccountStatus::Active)
                && account
                    .number
                    .as_ref()
                    .is_some_and(|number| number.peek() == last4)
                && account
                    .expiration_date
                    .as_ref()
                    .is_some_and(|expiry| expiry.peek() == expiry_mm_yyyy);
            if let (true, Some(token)) = (same_card, account.payment_method_identifier.as_ref()) {
                if !matches.iter().any(|known| known.peek() == token.peek()) {
                    matches.push(token);
                }
            }
        }
        match matches.as_slice() {
            [token] => Ok((*token).clone()),
            [] => Err(
                "PayNearMe reported success but lists no active credit or debit account \
                 matching the card's last four digits and expiry"
                    .to_string(),
            ),
            several => Err(format!(
                "PayNearMe lists {} active accounts matching the card's last four digits and \
                 expiry, so the card just stored cannot be told apart",
                several.len()
            )),
        }
    }
}

/// `/create_payment_method` response for a tokenise-only call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynearmeSetupMandateResponse {
    pub status: Option<String>,
    pub response_code: Option<String>,
    #[serde(alias = "orders")]
    pub order: Option<PaynearmeStoredCredentialOrder>,
    #[serde(default)]
    pub errors: Vec<PaynearmeErrorItem>,
}

/// Last four digits of the PAN that was sent, to find its account in `accounts[]`,
/// which reports only the last four.
fn card_last4<T: PaymentMethodDataTypes>(card: &Card<T>) -> Option<String> {
    let digits: Vec<char> = card
        .card_number
        .peek()
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let start = digits.len().checked_sub(4)?;
    digits.get(start..).map(|last4| last4.iter().collect())
}

/// Everything a successful SetupMandate must establish, returned as
/// `(connector_mandate_id, pnm_order_identifier)`, or the reason it could not.
fn stored_credential_reference<T: PaymentMethodDataTypes>(
    response: &PaynearmeSetupMandateResponse,
    router_data: &SetupMandateRouterData<T>,
) -> Result<(String, String), String> {
    let common = &router_data.resource_common_data;

    // The order the card was attached to is the one the request named.
    let pnm_order_identifier = common
        .connector_order_id
        .clone()
        .ok_or_else(|| "the request carried no PayNearMe order id".to_string())?;
    let order = response
        .order
        .as_ref()
        .ok_or_else(|| "PayNearMe returned no order object".to_string())?;

    // RepeatPayment charges this order again for every merchant-initiated payment;
    // a one-time order would accept one such charge at most.
    if order.order_is_standing != Some(true) {
        return Err(format!(
            "PayNearMe order {pnm_order_identifier} is not a standing order, so the stored \
             card could not be charged against it again; create the order with \
             connector_feature_data {{\"order_is_standing\": true, \
             \"site_customer_identifier\": \"<customer.id>\"}}"
        ));
    }

    // The card belongs to the order's customer. An order keyed on a per-order
    // fallback, or on another customer, would orphan it.
    let expected_customer = common
        .customer_id
        .as_ref()
        .map(|customer_id| customer_id.get_string_repr())
        .ok_or_else(|| "the request carried no customer.id".to_string())?;
    let order_customer = order
        .customer
        .as_ref()
        .and_then(|customer| customer.site_customer_identifier.as_deref());
    if order_customer != Some(expected_customer) {
        return Err(format!(
            "PayNearMe order {pnm_order_identifier} does not belong to customer.id; its \
             site_customer_identifier must equal the customer.id sent on SetupRecurring"
        ));
    }

    let card = match &router_data.request.payment_method_data {
        PaymentMethodData::Card(card) => card,
        _ => return Err("the stored payment method is not a card".to_string()),
    };
    let last4 =
        card_last4(card).ok_or_else(|| "the card number has fewer than four digits".to_string())?;
    let expiry = card_expiry_mm_yyyy(card)
        .map_err(|_| "the card expiry could not be formatted as MM/YYYY".to_string())?;

    let connector_mandate_id = PaynearmeMandateReference {
        payment_method_identifier: order.stored_card_token(&last4, expiry.peek())?,
        pnm_order_identifier: pnm_order_identifier.clone(),
    }
    .to_connector_mandate_id()?;

    Ok((connector_mandate_id, pnm_order_identifier))
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaynearmeSetupMandateResponse, Self>>
    for SetupMandateRouterData<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaynearmeSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;

        // A declared error, `status: "error"` or a non-zero `response_code` such as
        // 1003 (AVS mismatch) or 1007 (invalid security code), is PayNearMe
        // refusing to store the card. SetupMandate is a write, so that is terminal.
        let outcome = if envelope_is_ok(
            response.status.as_deref(),
            response.response_code.as_deref(),
        ) {
            // An ok envelope still has to yield a usable reference. When it does
            // not, the attempt fails: nothing was charged, and succeeding with
            // `mandate_reference: None` would tell the merchant a card was stored
            // for later use when none can be charged.
            stored_credential_reference(&response, &router_data).map_err(|reason| {
                build_error_response(
                    item.http_code,
                    Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    None,
                    &[PaynearmeErrorItem::Message(reason)],
                    None,
                )
            })
        } else {
            Err(build_error_response(
                item.http_code,
                Some(FlowStatus::Payment(AttemptStatus::Failure)),
                response.response_code.as_deref(),
                &response.errors,
                None,
            ))
        };

        match outcome {
            Ok((connector_mandate_id, pnm_order_identifier)) => Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    // A tokenise-only call creates no payment, so there is no
                    // transaction id. Reporting the token here instead would invite
                    // a PSync to look it up with `/find_payment`, which takes
                    // payment ids only.
                    resource_id: ResponseId::NoResponseId,
                    redirection_data: None,
                    mandate_reference: Some(Box::new(MandateReference {
                        connector_mandate_id: Some(connector_mandate_id),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })),
                    connector_metadata: None,
                    // PayNearMe returns no scheme transaction id on tokenisation.
                    network_txn_id: None,
                    network_txn_link_id: None,
                    // `pnm_order_identifier`, the reference id every flow reports.
                    connector_response_reference_id: Some(pnm_order_identifier),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: item.http_code,
                    payment_account_reference: None,
                }),
                resource_common_data: PaymentFlowData {
                    // The terminal success status zero-amount SetupMandate flows
                    // report (NMI, Finix): the setup is complete, nothing to poll.
                    status: AttemptStatus::Charged,
                    ..router_data.resource_common_data
                },
                ..router_data
            }),
            Err(error_response) => Ok(Self {
                response: Err(error_response),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data
                },
                ..router_data
            }),
        }
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
                        Some(FlowStatus::Payment(AttemptStatus::Pending)),
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
        // `pnm_order_identifier`, the same reference id Authorize reported
        // (§7.3), taken from `connector_order_id` — which the sync request
        // carries as `connector_order_reference_id`
        // (`domain_types/src/types.rs:6004`). It is deliberately **not**
        // `payment.site_payment_identifier`: that is merely the echo of our own
        // attempt reference, and reporting it here made the same payment come
        // back with a different `connector_response_reference_id` depending on
        // whether Authorize or PSync was the last call. When the caller sent no
        // order reference there is nothing new to report, and `None` says so
        // rather than contradicting what Authorize already recorded.
        let order_identifier = item
            .router_data
            .resource_common_data
            .connector_order_id
            .clone();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: payment.connector_metadata(),
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
                    Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
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
                // The same `pnm_order_identifier` reference id as Authorize and
                // PSync (§7.3), never the echoed `site_payment_identifier`. The
                // void request carries no order reference today
                // (`domain_types/src/types.rs:6083` sets `connector_order_id:
                // None`), so this is normally `None` — no new reference rather
                // than a second, different one.
                connector_response_reference_id: item
                    .router_data
                    .resource_common_data
                    .connector_order_id
                    .clone(),
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
            // PayNearMe declared an error on a 2xx body — `status: "error"` or a
            // non-zero `response_code` (§11.2 / §11.3). Note that transport
            // failures never reach here: a non-2xx response is routed to
            // `get_error_response_v2` / `get_5xx_error_response` instead
            // (`external-services/src/service.rs:484-494`), so this arm is
            // exactly "PayNearMe read the request and refused it".
            //
            // That refusal means **no refund was created**, so nothing will ever
            // appear under `payment.refund` for RSync to read — reporting
            // `Pending` here (as this arm used to, lumped in with the ambiguous
            // case below) left the refund polling a refund that does not exist,
            // for ever. It is a terminal failure.
            (false, _) => {
                return Ok(Self {
                    response: Err(build_error_response(
                        item.http_code,
                        Some(FlowStatus::Refund(RefundStatus::Failure)),
                        response.response_code.as_deref(),
                        &response.errors,
                        response.transaction_id(),
                    )),
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            // Genuinely ambiguous: PayNearMe accepted the call (`status: "ok"`)
            // but returned no `payment` object. The refund may well have been
            // created, so this must **not** be failed — a false failure invites
            // the merchant to refund a second time. `Pending`, resolved by
            // RSync, is the safe reading.
            (true, None) => {
                return Ok(Self {
                    response: Err(build_error_response(
                        item.http_code,
                        Some(FlowStatus::Refund(RefundStatus::Pending)),
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
            // Deliberately NOT split the way the Refund handler above is.
            // Refund is a write: a declared error there is PayNearMe refusing to
            // create the refund, which is terminal. RSync is a *read*, and it
            // looks up `connector_transaction_id` — the payment id (§8.6.3: the
            // payment id doubles as `connector_refund_id`, so there is no refund
            // identifier that could be unknown). A declared error here is
            // therefore about reading the *payment* and says nothing about
            // whether the refund exists; failing the refund on it would be a
            // false failure, and a false failure invites a second refund.
            // `Pending` matches what PSync does with this same condition.
            _ => {
                return Ok(Self {
                    response: Err(build_error_response(
                        item.http_code,
                        Some(FlowStatus::Refund(RefundStatus::Pending)),
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

// =============================================================================
// TESTS
// =============================================================================

/// The signing helper is the only piece of novel logic in this connector: get it
/// wrong and every single request is rejected, and nothing else in the codebase
/// exercises it. Both worked vectors published on the Authentication page
/// (spec §3.1) are pinned here — the second is the one the docs explicitly hand
/// out as a unit test, because it exercises the `format` exemption and the
/// alphabetical ordering.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Worked example #1 (spec §3.1, the `/create_order` sample): the full
    /// envelope, including the empty `signature` field that must not sign
    /// itself.
    #[test]
    fn signs_the_create_order_worked_example() {
        let body = serde_json::json!({
            "site_identifier": "S2155373459",
            "timestamp": "1636142061",
            "version": "3.0",
            "signature": "",
            "order_amount": "500",
            "order_currency": "USD",
            "site_customer_identifier": "11223344",
            "order_type": "any",
            "order_is_standing": "true",
        });

        let string_to_sign = paynearme_string_to_sign(&body).expect("string_to_sign");
        assert_eq!(
            string_to_sign,
            // Split at the currency/`order_is_standing` boundary: concatenated
            // end-to-end, those two run together into a token the spell checker
            // flags as a misspelling. `concat!` keeps the runtime value identical.
            concat!(
                "order_amount500order_currencyUSD",
                "order_is_standingtrueorder_typeany",
                "site_customer_identifier11223344site_identifierS2155373459",
                "timestamp1636142061version3.0",
            )
        );

        let signature =
            paynearme_signature(&Secret::new("ab7b539ea1317cca67c63c552".to_string()), &body)
                .expect("signature");
        assert_eq!(
            signature.peek(),
            "65c694f4b632187aa02fc4144cdd374373307f0a200448c9e53f2e47d84b3b82"
        );
    }

    /// Worked example #2 (spec §3.1), the vector shipped in every reference
    /// implementation: `format` is dropped from the input even though it is sent
    /// on the wire, and the remaining keys are concatenated in alphabetical —
    /// not insertion — order.
    #[test]
    fn signs_the_documented_unit_test_vector() {
        let body = serde_json::json!({
            "version": "3.0",
            "site_identifier": "MySiteID",
            "timestamp": "1582149833",
            "hello": "world",
            "foo": "bar",
            "format": "json",
        });

        let string_to_sign = paynearme_string_to_sign(&body).expect("string_to_sign");
        assert_eq!(
            string_to_sign,
            "foobarhelloworldsite_identifierMySiteIDtimestamp1582149833version3.0"
        );

        let signature =
            paynearme_signature(&Secret::new("abc123".to_string()), &body).expect("signature");
        assert_eq!(
            signature.peek(),
            "7986b4e59c15cd22fd496113c916f9739f619778812bf9ab8943af80749aadcc"
        );
    }

    /// `version`, `site_identifier` and `timestamp` are mandatory inputs to the
    /// signature (§3.1 step 2) — a body missing one must be refused rather than
    /// signed into a request PayNearMe will reject with a bare 401.
    #[test]
    fn refuses_to_sign_without_the_required_envelope_fields() {
        let body = serde_json::json!({
            "site_identifier": "MySiteID",
            "version": "3.0",
        });
        assert!(paynearme_string_to_sign(&body).is_err());
    }

    /// A field that is skipped by `skip_serializing_if = "Option::is_none"` is
    /// absent from the wire, so it must be absent from the signature too — the
    /// full-refund shape, where `refund_amount` / `refund_currency` are omitted,
    /// depends on this.
    #[test]
    fn omitted_fields_do_not_enter_the_signature() {
        let with_null = serde_json::json!({
            "site_identifier": "MySiteID",
            "timestamp": "1582149833",
            "version": "3.0",
            "refund_amount": serde_json::Value::Null,
        });
        let without = serde_json::json!({
            "site_identifier": "MySiteID",
            "timestamp": "1582149833",
            "version": "3.0",
        });
        assert_eq!(
            paynearme_string_to_sign(&with_null).expect("string_to_sign"),
            paynearme_string_to_sign(&without).expect("string_to_sign")
        );
    }

    /// The SetupMandate body, built from the spec's "tokenize only" example values
    /// (expiry in the normative `MM/YYYY`). The flattened card block must sign as
    /// plain top-level fields, and nothing a charge would add (`send_payment`,
    /// `payment_amount`, `payment_currency`, `site_channel`) may reach the
    /// preimage. The expected digest was computed outside this codebase (Python
    /// `hmac` + `hashlib`) with the secret from worked example #1.
    #[test]
    fn signs_the_tokenize_only_setup_mandate_body() {
        let request = PaynearmeSetupMandateRequest {
            site_identifier: Secret::new("S2411573363".to_string()),
            timestamp: "1702333839".to_string(),
            version: "3.0".to_string(),
            signature: Secret::new(String::new()),
            pnm_order_identifier: "85011138740".to_string(),
            card: PaynearmeCardPaymentMethod {
                payment_method_type: "card".to_string(),
                payment_method_card_number_pii: Secret::new("9999916516806651".to_string()),
                payment_method_card_expiry_pii: Secret::new("01/2027".to_string()),
                payment_method_cvv_pii: Secret::new("416".to_string()),
                payment_method_billing_name: Secret::new("John Smith".to_string()),
                payment_method_billing_address: Secret::new("123 Fake Street".to_string()),
                payment_method_billing_zipcode: Secret::new("75013".to_string()),
                payment_method_billing_phone: Secret::new("469-555-5878".to_string()),
            },
        };

        let body = serde_json::to_value(&request).expect("body");
        assert_eq!(
            paynearme_string_to_sign(&body).expect("string_to_sign"),
            concat!(
                "payment_method_billing_address",
                "123 Fake Street",
                "payment_method_billing_name",
                "John Smith",
                "payment_method_billing_phone",
                "469-555-5878",
                "payment_method_billing_zipcode",
                "75013",
                "payment_method_card_expiry_pii",
                "01/2027",
                "payment_method_card_number_pii",
                "9999916516806651",
                "payment_method_cvv_pii",
                "416",
                "payment_method_type",
                "card",
                "pnm_order_identifier",
                "85011138740",
                "site_identifier",
                "S2411573363",
                "timestamp",
                "1702333839",
                "version",
                "3.0",
            )
        );

        let signature = paynearme_signature(
            &Secret::new("ab7b539ea1317cca67c63c552".to_string()),
            &request,
        )
        .expect("signature");
        assert_eq!(
            signature.peek(),
            "fc2a7784d9a2c329a106e844db94f465e0768fa1e32dd56798a4da437ab4dbcc"
        );
    }

    /// The spec's verbatim "Create a Card Payment Method" `order`, trimmed to the
    /// fields SetupMandate reads: a `debit` card account next to an unrelated
    /// `ach` account.
    fn documented_card_order() -> PaynearmeStoredCredentialOrder {
        serde_json::from_value(serde_json::json!({
            "order_is_standing": true,
            "customer": { "site_customer_identifier": "470070000" },
            "electronic_payments": {
                "payment_methods": [
                    { "type": "apple_pay_debit", "fee_amount": "4.99", "accounts": [] },
                    {
                        "type": "debit",
                        "accounts": [{
                            "payment_method_identifier": "a95ac4a03ef38",
                            "status": "active",
                            "account_type": "Debit",
                            "number": "7641",
                            "expiration_date": "06/2024",
                            "card_brand": "PULSE"
                        }]
                    },
                    {
                        "type": "ach",
                        "accounts": [{
                            "payment_method_identifier": "534be6d8afcf4",
                            "status": "active",
                            "number": "7016"
                        }]
                    }
                ]
            }
        }))
        .expect("documented order")
    }

    #[test]
    fn finds_the_stored_card_by_last_four_and_expiry() {
        let order = documented_card_order();
        assert_eq!(
            order
                .stored_card_token("7641", "06/2024")
                .expect("token")
                .peek(),
            "a95ac4a03ef38"
        );
        // Right last four, wrong expiry; and an ACH account's last four.
        assert!(order.stored_card_token("7641", "07/2024").is_err());
        assert!(order.stored_card_token("7016", "06/2024").is_err());
    }

    /// Two different active tokens for what looks like the same card must not be
    /// guessed between; an inactive duplicate does not count.
    #[test]
    fn refuses_to_guess_between_matching_cards() {
        let account = |token: &str, status: &str| {
            serde_json::json!({
                "payment_method_identifier": token,
                "status": status,
                "number": "7641",
                "expiration_date": "06/2024"
            })
        };
        let order = |accounts: serde_json::Value| -> PaynearmeStoredCredentialOrder {
            serde_json::from_value(serde_json::json!({
                "electronic_payments": {
                    "payment_methods": [{ "type": "credit", "accounts": accounts }]
                }
            }))
            .expect("order")
        };

        let ambiguous = order(serde_json::json!([
            account("a95ac4a03ef38", "active"),
            account("3b7ac9d4c86e6", "active")
        ]));
        assert!(ambiguous.stored_card_token("7641", "06/2024").is_err());

        let one_retired = order(serde_json::json!([
            account("a95ac4a03ef38", "active"),
            account("3b7ac9d4c86e6", "inactive")
        ]));
        assert_eq!(
            one_retired
                .stored_card_token("7641", "06/2024")
                .expect("token")
                .peek(),
            "a95ac4a03ef38"
        );
    }

    /// The reference fits Hyperswitch's `VARCHAR(128)` `connector_mandate_id`, and
    /// one that would not is refused rather than truncated.
    #[test]
    fn mandate_reference_fits_the_connector_mandate_id_column() {
        let reference = PaynearmeMandateReference {
            payment_method_identifier: Secret::new("a95ac4a03ef38".to_string()),
            pnm_order_identifier: "86383382942".to_string(),
        };
        assert_eq!(
            reference.to_connector_mandate_id().expect("encoded"),
            r#"{"pmi":"a95ac4a03ef38","oid":"86383382942"}"#
        );

        let oversized = PaynearmeMandateReference {
            payment_method_identifier: Secret::new("f".repeat(CONNECTOR_MANDATE_ID_MAX_LEN)),
            pnm_order_identifier: "86383382942".to_string(),
        };
        assert!(oversized.to_connector_mandate_id().is_err());
    }
}
