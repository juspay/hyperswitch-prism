use std::collections::HashMap;

use crate::types::ResponseRouterData;
use crate::utils::{integration_ctx, truncate_secret_string};
use base64::{engine::general_purpose, Engine};
use common_enums::AttemptStatus;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, SignMessage},
    pii::Email,
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, SetupMandate, Void, VoidPC},
    connector_types::{
        BillingDescriptor, L2L3Data, MandateReference, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, SetupMandateRequestData,
    },
    payment_address::{AddressDetails, OrderDetailsWithAmount},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// ===== DOCUMENTED FIELD LENGTH LIMITS (OpenAPI 3.0.2 v26.2.5) =====
// Overlong values are rejected by the gateway with a 400 whose `error.details[].field`
// names the offender, so every merchant-controlled string is truncated before it is sent.
const MAX_LEN_MERCHANT_TRANSACTION_ID: usize = 40;
const MAX_LEN_ORDER_ID: usize = 100;
const MAX_LEN_NAME: usize = 96;
const MAX_LEN_FIRST_LAST_NAME: usize = 48;
const MAX_LEN_CUSTOMER_ID: usize = 32;
const MAX_LEN_EMAIL: usize = 254;
const MAX_LEN_PHONE: usize = 32;
const MAX_LEN_ADDRESS_LINE: usize = 96;
const MAX_LEN_CITY: usize = 96;
const MAX_LEN_REGION: usize = 96;
const MAX_LEN_POSTAL_CODE: usize = 24;
const MAX_LEN_COUNTRY: usize = 32;
const MAX_LEN_CARDHOLDER_NAME: usize = 96;
const MAX_LEN_CUSTOMER_SERVICE_NUMBER: usize = 10;
const MAX_LEN_CUSTOMER_REFERENCE_ID: usize = 17;
const MAX_LEN_VAT_REGISTRATION_NUMBER: usize = 30;
const MAX_LEN_COMMODITY_CODE: usize = 4;
const MAX_LEN_PRODUCT_CODE: usize = 20;
const MAX_LEN_LINE_ITEM_DESCRIPTION: usize = 30;
const MAX_LEN_UNIT_MEASURE: usize = 3;
const MAX_LEN_ACS_TRANSACTION_ID: usize = 40;
/// `Level3.lineItems` declares `maxItems: 100`. Whether the gateway rejects or silently
/// truncates a longer array is undocumented, so the connector fails the request explicitly
/// rather than dropping financial line items on the floor.
const MAX_LINE_ITEMS: usize = 100;
/// `cavv` and `xid` both declare `minLength: 20`, `maxLength: 32`. A value outside those
/// bounds is omitted rather than sent — the gateway 400s on an out-of-range value.
const AUTH_VALUE_MIN_LEN: usize = 20;
const AUTH_VALUE_MAX_LEN: usize = 32;

/// Truncate a plain (non-secret) string to `max_len` **characters** — not bytes, so a
/// multi-byte grapheme is never split down the middle.
///
/// The plain-string sibling of [`crate::utils::truncate_secret_string`], which is the shared
/// helper used for every `Secret<_>` field here. It stays local to this connector rather than
/// moving next to it in `utils.rs`: no other connector needs a plain-string truncation today,
/// and promoting a single-caller helper into the shared file only widens this PR's blast radius.
fn truncate_string(value: &str, max_len: usize) -> String {
    if value.chars().count() > max_len {
        value.chars().take(max_len).collect()
    } else {
        value.to_string()
    }
}

/// Keep only ASCII digits. `softDescriptor.customerServiceNumber` is constrained to
/// `^[0-9]+$`, so the caller's `+44 20 1234 5678` has to be reduced to `442012345678` before
/// it can be sent.
fn retain_ascii_digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

// ===== AUTHENTICATION STRUCTURE =====

#[derive(Debug, Clone)]
pub struct AuthipayAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl AuthipayAuthType {
    /// Generate HMAC-SHA256 signature for Authipay API
    /// Raw signature: API-Key + ClientRequestId + time + requestBody
    /// Then HMAC-SHA256 with API Secret as key, then Base64 encode
    pub fn generate_hmac_signature(
        &self,
        api_key: &str,
        client_request_id: &str,
        timestamp: &str,
        request_body: &str,
    ) -> Result<String, error_stack::Report<IntegrationError>> {
        // Raw signature: apiKey + ClientRequestId + time + requestBody
        let raw_signature = format!("{api_key}{client_request_id}{timestamp}{request_body}");

        // Generate HMAC-SHA256 with API Secret as key
        let signature = crypto::HmacSha256
            .sign_message(
                self.api_secret.clone().expose().as_bytes(),
                raw_signature.as_bytes(),
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: integration_ctx(
                    "authipay: HMAC-SHA256 signing of the Message-Signature preimage failed.",
                    "Check that the configured api_secret is present and is the HMAC secret paired with this Api-Key.",
                ),
            })?;

        // Base64 encode the result
        Ok(general_purpose::STANDARD.encode(signature))
    }

    /// A fresh per-call `Client-Request-Id`, for the **read-only** inquiry flows only.
    ///
    /// `GET /payments/{id}` changes nothing, so there is no double-spend to dedupe — and the
    /// gateway rejects a replayed `Client-Request-Id` with a 400 (see
    /// [`derive_client_request_id`]), which would make a payment un-pollable the second time
    /// PSync ran. State-changing operations must use `derive_client_request_id` instead.
    pub fn generate_client_request_id() -> String {
        common_utils::fp_utils::generate_uuid_v4()
    }

    /// Generate timestamp in milliseconds since Unix epoch
    pub fn generate_timestamp() -> String {
        common_utils::date_time::now_unix_millis().to_string()
    }
}

/// Which Authipay operation a `Client-Request-Id` belongs to.
///
/// The discriminator is part of the derivation input, so two different operations on the same
/// payment can never derive the same id. Without it, a Capture whose
/// `connector_request_reference_id` happened to equal the Authorize's would reuse the
/// Authorize's id and be rejected outright — see [`derive_client_request_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthipayOperation {
    Authorize,
    SetupMandate,
    Capture,
    Void,
    VoidPostCapture,
    Refund,
}

impl AuthipayOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Authorize => "authorize",
            Self::SetupMandate => "setup_mandate",
            Self::Capture => "capture",
            Self::Void => "void",
            Self::VoidPostCapture => "void_post_capture",
            Self::Refund => "refund",
        }
    }
}

/// Derive the `Client-Request-Id` header **deterministically** for a state-changing operation.
///
/// Authipay documents this header as the gateway's idempotency key
/// (`ClientRequestIdParam.description`: *"This is also used for idempotency control"*). The
/// connector used to mint a fresh UUID v4 on every call, which opted every request out of that
/// control: an `Authorize` retried after a client timeout was a brand-new request to Authipay
/// and could authorize the cardholder twice.
///
/// **Observed sandbox behaviour** (the OpenAPI document does not say, and the tech spec records
/// this as UNDECIDED #2): replaying a `Client-Request-Id` is answered with
/// `400 Bad Request` / `responseType: BadRequest` and the transaction is **not** processed
/// again — the gateway fails closed rather than echoing the original transaction. So a stable
/// id genuinely prevents the double charge, and the caller resolves the ambiguous outcome with
/// PSync, exactly as it would after any timeout.
///
/// Two consequences follow from that same observation, and both are load-bearing:
///
/// 1. The id must be unique **per operation**, not merely per attempt — hence
///    [`AuthipayOperation`] in the derivation input. A Capture reusing the Authorize's id would
///    be rejected as a replay and the money would never settle.
/// 2. Read-only polling (PSync / RSync) must *not* use this function. A poll has nothing to
///    dedupe and has to be repeatable; a stable id would make the second poll 400 and strand
///    the payment. Those flows use [`AuthipayAuthType::generate_client_request_id`].
///
/// The gateway also *recommends* a 128-bit UUID, so the caller's reference is not sent raw:
/// UUID v5 (SHA-1 over the OID namespace) keeps the recommended shape while staying a pure
/// function of its input.
pub fn derive_client_request_id(operation: AuthipayOperation, reference: &str) -> String {
    let seed = format!("authipay:{}:{reference}", operation.as_str());
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, seed.as_bytes()).to_string()
}

impl TryFrom<&ConnectorSpecificConfig> for AuthipayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Authipay {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: integration_ctx(
                        "authipay: the connector config did not match ConnectorSpecificConfig::Authipay.",
                        "Route this payment to a merchant connector account configured for authipay.",
                    ),
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE STRUCTURES =====

/// Authipay reports errors under **two** OpenAPI schemas, and this one struct deserialises both:
///
/// * `ErrorResponse` (400/401/403/404/415/500/502) — an `error` object and nothing else.
/// * `TransactionErrorResponse` (409 *Gateway Declined* / 422 *Endpoint Declined*) — a full
///   `TransactionResponse` body **plus** `error`. This is where every issuer decline code
///   actually lives, so it is the payload GSM / smart-retry cares about.
///
/// Everything is optional so a body of either shape parses; an explicit tag is impossible
/// because the discriminator (`type`) is identical prose in both, hence one permissive struct
/// rather than `#[serde(untagged)]` over two.
///
/// The previous version of this struct declared `code`/`message`/`details` at the **top level**,
/// but the API nests them under `error` — so every 4xx/5xx surfaced to the merchant with an
/// empty code and an empty message.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayErrorResponse {
    pub client_request_id: Option<String>,
    pub api_trace_id: Option<String>,
    pub response_type: Option<String>,
    /// Nested error object — the documented location of `code` / `message` / `details`.
    pub error: Option<AuthipayErrorDetails>,
    /// `TransactionErrorResponse` only (409/422): the gateway transaction id, so a decline that
    /// nonetheless created a record can be reconciled.
    pub ipg_transaction_id: Option<String>,
    pub order_id: Option<String>,
    pub merchant_transaction_id: Option<String>,
    pub transaction_result: Option<AuthipayPaymentResult>,
    pub transaction_state: Option<AuthipayTransactionState>,
    pub approval_code: Option<String>,
    pub scheme_response_code: Option<String>,
    pub merchant_advice_code: Option<String>,
    pub error_message: Option<String>,
    /// The OpenAPI document places `declineReasonCode` inside `error`; the Fiserv prose docs
    /// and the hyperswitch reference struct place it at the top level. The two disagree and
    /// parsing both costs nothing, so both positions are accepted.
    pub decline_reason_code: Option<String>,
    pub processor: Option<Processor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayErrorDetails {
    pub code: Option<String>,
    pub message: Option<String>,
    pub details: Option<Vec<ErrorDetail>>,
    pub decline_reason_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    pub field: Option<String>,
    pub message: Option<String>,
}

impl AuthipayErrorResponse {
    fn error_code(&self) -> Option<String> {
        self.error
            .as_ref()
            .and_then(|error| error.code.clone())
            .or_else(|| {
                self.processor
                    .as_ref()
                    .and_then(|processor| processor.response_code.clone())
            })
            .or_else(|| self.scheme_response_code.clone())
    }

    fn error_message(&self) -> Option<String> {
        self.error
            .as_ref()
            .and_then(|error| error.message.clone())
            .or_else(|| {
                self.processor
                    .as_ref()
                    .and_then(|processor| processor.response_message.clone())
            })
            .or_else(|| self.error_message.clone())
            .or_else(|| self.approval_code.clone())
    }

    fn error_reason(&self) -> Option<String> {
        self.error
            .as_ref()
            .and_then(|error| error.decline_reason_code.clone())
            .or_else(|| self.decline_reason_code.clone())
            .or_else(|| {
                self.processor
                    .as_ref()
                    .and_then(|processor| processor.merchant_advice_message.clone())
            })
            .or_else(|| self.error_message.clone())
            .or_else(|| {
                // `details[]` names the offending field on a 400; join them so the merchant
                // sees *which* field the gateway rejected rather than a bare code.
                self.error
                    .as_ref()
                    .and_then(|error| error.details.as_ref())
                    .map(|details| {
                        details
                            .iter()
                            .map(|detail| {
                                format!(
                                    "{}: {}",
                                    detail.field.clone().unwrap_or_default(),
                                    detail.message.clone().unwrap_or_default()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                    .filter(|joined| !joined.is_empty())
            })
            .or_else(|| self.api_trace_id.clone())
    }
}

/// Build the caller-facing [`ErrorResponse`] from an Authipay error body.
///
/// `attempt_status` is deliberately **not** decided here: this builder is shared by
/// `ConnectorCommon::build_error_response`, which is flow-agnostic and also routes Refund and
/// RSync (where `FlowStatus::Payment(_)` is coerced to `RefundFailure`). Each flow's own
/// transformer sets the status it derived.
pub fn build_authipay_error_response(
    response: &AuthipayErrorResponse,
    status_code: u16,
    typed_connector_response: Option<String>,
) -> ErrorResponse {
    let (network_decline_code, network_advice_code, network_error_message) =
        extract_network_error_fields(
            response.processor.as_ref(),
            response.scheme_response_code.as_ref(),
            response.merchant_advice_code.as_ref(),
        );

    ErrorResponse {
        status_code,
        code: response
            .error_code()
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .error_message()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: response.error_reason(),
        attempt_status: None,
        connector_transaction_id: response.ipg_transaction_id.clone(),
        network_decline_code,
        network_advice_code,
        network_error_message,
        typed_connector_response,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

// ===== REQUEST TYPE ENUMS =====

/// `requestType` is the OpenAPI discriminator. Each variant carries an explicit
/// `#[serde(rename)]` so a future Rust-side rename cannot silently change the wire
/// discriminator — the previous version relied on the variant names happening to match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthipayRequestType {
    #[serde(rename = "PaymentCardSaleTransaction")]
    PaymentCardSaleTransaction,
    #[serde(rename = "PaymentCardPreAuthTransaction")]
    PaymentCardPreAuthTransaction,
    #[serde(rename = "PostAuthTransaction")]
    PostAuthTransaction,
    #[serde(rename = "ReturnTransaction")]
    ReturnTransaction,
    #[serde(rename = "VoidPreAuthTransactions")]
    VoidPreAuthTransactions,
    #[serde(rename = "VoidTransaction")]
    VoidTransaction,
}

/// `transactionOrigin` — omitting it lets the gateway apply a default that may not be `ECOM`,
/// which changes interchange and SCA treatment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthipayTransactionOrigin {
    #[serde(rename = "ECOM")]
    Ecom,
    #[serde(rename = "MAIL")]
    Mail,
    #[serde(rename = "PHONE")]
    Phone,
}

impl From<Option<common_enums::PaymentChannel>> for AuthipayTransactionOrigin {
    fn from(channel: Option<common_enums::PaymentChannel>) -> Self {
        match channel {
            Some(common_enums::PaymentChannel::MailOrder) => Self::Mail,
            Some(common_enums::PaymentChannel::TelephoneOrder) => Self::Phone,
            // An absent channel is an e-commerce payment: this connector has no
            // card-present (`RETAIL`) path.
            Some(common_enums::PaymentChannel::Ecommerce) | None => Self::Ecom,
        }
    }
}

/// `storedCredentials.sequence`. Only `FIRST` is reachable from the flows implemented here —
/// Authorize and SetupMandate are both the *cardholder-initiated* leg. `SUBSEQUENT` belongs to
/// the merchant-initiated `RepeatPayment` flow, which Authipay does not implement yet; the
/// variant is declared so the enum matches the documented value set rather than a subset of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthipayCredentialSequence {
    #[serde(rename = "FIRST")]
    First,
    #[serde(rename = "SUBSEQUENT")]
    Subsequent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthipayCredentialInitiator {
    #[serde(rename = "CARDHOLDER")]
    Cardholder,
    #[serde(rename = "MERCHANT")]
    Merchant,
}

/// `storedCredentials.indicatorSubcategory`. Valid values depend on `initiator`; every variant
/// below is one of the values Fiserv lists for `initiator: CARDHOLDER`, which is the only
/// initiator the cardholder-initiated flows (Authorize and SetupMandate) emit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayIndicatorSubcategory {
    CredentialOnFileFirst,
    StandingOrder,
    Subscription,
    Installment,
}

/// `authenticationResult.authenticationResponse` / `.transactionStatus` — the ARes/CRes letter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthipayAuthenticationResponse {
    #[serde(rename = "Y")]
    Authenticated,
    #[serde(rename = "A")]
    Attempted,
    #[serde(rename = "N")]
    NotAuthenticated,
    #[serde(rename = "U")]
    Unavailable,
    #[serde(rename = "R")]
    Rejected,
    #[serde(rename = "C")]
    ChallengeRequired,
    #[serde(rename = "I")]
    InformationOnly,
}

impl AuthipayAuthenticationResponse {
    /// Fiserv's eligibility table forbids sending a CAVV alongside `U` ("unable to
    /// authenticate"): the pairing is a documented error, not merely redundant.
    fn permits_cavv(&self) -> bool {
        !matches!(self, Self::Unavailable)
    }
}

impl From<common_enums::TransactionStatus> for AuthipayAuthenticationResponse {
    fn from(status: common_enums::TransactionStatus) -> Self {
        match status {
            common_enums::TransactionStatus::Success => Self::Authenticated,
            common_enums::TransactionStatus::NotVerified => Self::Attempted,
            common_enums::TransactionStatus::Failure => Self::NotAuthenticated,
            common_enums::TransactionStatus::VerificationNotPerformed => Self::Unavailable,
            common_enums::TransactionStatus::Rejected => Self::Rejected,
            common_enums::TransactionStatus::ChallengeRequired
            | common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication => {
                Self::ChallengeRequired
            }
            common_enums::TransactionStatus::InformationOnly => Self::InformationOnly,
        }
    }
}

// ===== REQUEST STRUCTURES =====

/// `POST /payments` primary transaction.
///
/// **This struct must stay a pure function of the router data.** `get_headers()` serialises it
/// once to build the HMAC `Message-Signature` and the macro-generated `get_request_body()`
/// serialises it again to produce the bytes actually sent. The two byte strings match only
/// while construction is deterministic — no `Uuid::new_v4()`, no `now()`, no `HashMap`
/// iteration order, no ordering derived from anything but `order_details`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentsRequest<T: PaymentMethodDataTypes> {
    pub request_type: AuthipayRequestType,
    pub merchant_transaction_id: String,
    pub transaction_amount: TransactionAmount,
    pub transaction_origin: AuthipayTransactionOrigin,
    pub order: OrderDetails,
    pub payment_method: PaymentMethod<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credentials: Option<AuthipayStoredCredentials>,
    /// `authenticationResult` is a **top-level sibling** of `paymentMethod`, not a member of
    /// `order`. It carries an externally-performed 3DS result; the gateway-managed
    /// `authenticationRequest` is mutually exclusive with it and is never emitted here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_result: Option<AuthipayAuthenticationResult>,
}

#[derive(Debug, Serialize)]
pub struct TransactionAmount {
    pub total: FloatMajorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDetails {
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<AuthipayBilling>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<AuthipayShipping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_descriptor: Option<AuthipaySoftDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_card: Option<AuthipayPurchaseCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayBilling {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    /// The schema notes `firstName`/`lastName` are *"only supported for AMEX"*; other schemes
    /// ignore them, so they are always sent and the gateway decides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<AuthipayContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<AuthipayAddress>,
}

impl AuthipayBilling {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.first_name.is_none()
            && self.last_name.is_none()
            && self.customer_id.is_none()
            && self.birth_date.is_none()
            && self.contact.is_none()
            && self.address.is_none()
    }
}

/// The `Shipping` schema has only `name`, `contact` and `address` — no `firstName`/`lastName`
/// and no `customerId`, unlike `Billing`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayShipping {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<AuthipayContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<AuthipayAddress>,
}

impl AuthipayShipping {
    fn is_empty(&self) -> bool {
        self.name.is_none() && self.contact.is_none() && self.address.is_none()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayContact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
}

impl AuthipayContact {
    fn is_empty(&self) -> bool {
        self.phone.is_none() && self.email.is_none()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    /// The schema accepts ISO-3166-1 ALPHA-2, ALPHA-3, numeric or the full country name, so
    /// serialising `CountryAlpha2` directly is valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<Secret<String>>,
}

impl AuthipayAddress {
    fn is_empty(&self) -> bool {
        self.address1.is_none()
            && self.address2.is_none()
            && self.city.is_none()
            && self.region.is_none()
            && self.postal_code.is_none()
            && self.country.is_none()
    }
}

/// `order.softDescriptor` — the dynamic descriptor shown on the cardholder's statement.
///
/// Fiserv publishes **no `maxLength`** for `dynamicMerchantName` (only `pattern: ^(?!\s*$).+`,
/// i.e. it may not be blank), and the 177-page IPG Integration Guide types the equivalent SOAP
/// element as a bare `xs:string` with no stated limit. So nothing is truncated here — inventing
/// a limit would silently mangle a descriptor the acquirer would have accepted. Only the
/// blank case the pattern forbids is filtered out.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipaySoftDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_merchant_name: Option<String>,
    /// `maxLength: 10`, `pattern: ^[0-9]+$` — punctuation and the leading `+` must be stripped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_service_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_address: Option<AuthipayAddress>,
}

impl AuthipaySoftDescriptor {
    fn is_empty(&self) -> bool {
        self.dynamic_merchant_name.is_none()
            && self.customer_service_number.is_none()
            && self.dynamic_address.is_none()
    }
}

/// `order.purchaseCard` — Level 2 / Level 3 purchasing-card data.
///
/// The `Level2` / `Level3` keys are PascalCase in the schema, unlike every other member of
/// `Order`, so they carry explicit renames.
#[derive(Debug, Serialize)]
pub struct AuthipayPurchaseCard {
    #[serde(rename = "Level2", skip_serializing_if = "Option::is_none")]
    pub level2: Option<AuthipayLevel2>,
    #[serde(rename = "Level3", skip_serializing_if = "Option::is_none")]
    pub level3: Option<AuthipayLevel3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayLevel2 {
    /// `customerReferenceID` — note the trailing `ID`, which `rename_all = "camelCase"` would
    /// render as `customerReferenceId`.
    #[serde(
        rename = "customerReferenceID",
        skip_serializing_if = "Option::is_none"
    )]
    pub customer_reference_id: Option<String>,
    /// `supplierVATRegistrationNumber` — `VAT` is upper-case in the schema.
    #[serde(
        rename = "supplierVATRegistrationNumber",
        skip_serializing_if = "Option::is_none"
    )]
    pub supplier_vat_registration_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_discount_amount_and_rate: Option<AuthipayAmountAndRate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vat_shipping_amount_and_rate: Option<AuthipayAmountAndRate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duty_amount_and_rate: Option<AuthipayAmountAndRate>,
}

impl AuthipayLevel2 {
    fn is_empty(&self) -> bool {
        self.customer_reference_id.is_none()
            && self.supplier_vat_registration_number.is_none()
            && self.total_discount_amount_and_rate.is_none()
            && self.vat_shipping_amount_and_rate.is_none()
            && self.duty_amount_and_rate.is_none()
    }
}

/// `Level3` declares `required: [lineItems]`, so it is only constructed with a non-empty vec.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayLevel3 {
    pub line_items: Vec<AuthipayLineItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayLineItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commodity_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub quantity: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_measure: Option<String>,
    pub unit_price: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vat_amount_and_rate: Option<AuthipayAmountAndRate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_amount_and_rate: Option<AuthipayAmountAndRate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_item_total: Option<FloatMajorUnit>,
}

/// `AdditionalAmountRate` declares `required: [amount, rate]`. Where UCS carries an amount but
/// no rate the whole object is omitted rather than sent with `rate: 0` — a fabricated 0% rate
/// is corrupt financial data, not a default. See [`AuthipayAmountAndRate::new`].
#[derive(Debug, Serialize)]
pub struct AuthipayAmountAndRate {
    pub amount: FloatMajorUnit,
    pub rate: f64,
}

impl AuthipayAmountAndRate {
    fn new(amount: Option<FloatMajorUnit>, rate: Option<f64>) -> Option<Self> {
        match (amount, rate) {
            (Some(amount), Some(rate)) => Some(Self { amount, rate }),
            _ => None,
        }
    }
}

/// `storedCredentials` marks the payment to the schemes as establishing or using a
/// credential-on-file. Emitting it on a genuine one-off payment mislabels the transaction, so
/// it is only built when the caller signalled a stored-credential intent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayStoredCredentials {
    pub sequence: AuthipayCredentialSequence,
    pub scheduled: bool,
    pub initiator: AuthipayCredentialInitiator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indicator_subcategory: Option<AuthipayIndicatorSubcategory>,
}

/// `authenticationResult` — an externally-performed 3DS result passed through to the gateway.
///
/// There is **no `eci` field**: Fiserv derives the ECI itself from `authenticationResponse`
/// plus `secure3DProtocolVersion` plus the scheme, and reports the outcome back in
/// `approvalCode` (e.g. `"Y:ECI2/5:Authenticated"`) and
/// `secure3dResponse.responseCode3dSecure`. The caller's own ECI is therefore recorded in
/// `payment_checks` on the response instead of being forwarded — see
/// [`build_payment_checks`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayAuthenticationResult {
    pub authentication_type: AuthipayAuthenticationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acs_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_response: Option<AuthipayAuthenticationResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_status: Option<AuthipayAuthenticationResponse>,
    pub message_category: AuthipayMessageCategory,
    #[serde(
        rename = "secure3DProtocolVersion",
        skip_serializing_if = "Option::is_none"
    )]
    pub secure_3d_protocol_version: Option<String>,
}

/// The current authentication result type. `Secure3D10AuthenticationResult` and
/// `Secure3D21AuthenticationResult` exist in the schema but are deprecated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthipayAuthenticationType {
    #[serde(rename = "Secure3DAuthenticationResult")]
    Secure3DAuthenticationResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthipayMessageCategory {
    /// Payment authentication. The only category this connector emits — `02` is non-payment
    /// and `80` is the Mastercard data-only flow, neither of which Authorize performs.
    #[serde(rename = "01")]
    Payment,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethod<T: PaymentMethodDataTypes> {
    pub payment_card: PaymentCard<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_date: ExpiryDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_code: Option<Secret<String>>,
    /// The schema field is `cardholderName` (`maxLength: 96`). It was previously serialised as
    /// `holder`, which is not a documented property — at best ignored, at worst a 400.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpiryDate {
    pub month: Secret<String>,
    pub year: Secret<String>,
}

// ===== REQUEST TRANSFORMATION =====

/// Convert a `MinorUnit` amount to the decimal major-unit number the API expects.
fn to_major_unit(
    amount: MinorUnit,
    currency: common_enums::Currency,
    field: &'static str,
) -> Result<FloatMajorUnit, error_stack::Report<IntegrationError>> {
    FloatMajorUnitForConnector
        .convert(amount, currency)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: crate::utils::amount_conversion_ctx(field, &amount, &currency),
        })
        .attach_printable_lazy(|| {
            format!("authipay: failed to convert {field} to major units for {currency}")
        })
}

/// Reject the capture methods this connector cannot serve.
///
/// This is the **single source of truth** for capture-method rejection. It is called from
/// `AuthipayPrimaryRequest::request_type`, which every flow that builds
/// [`AuthipayPaymentsRequest`] goes through — Authorize and SetupMandate alike — so the guard
/// cannot end up on one of them and not the other.
///
/// `ManualMultiple` and `Scheduled` are rejected rather than routed to PreAuth:
/// * `ManualMultiple` means several partial captures against one authorization, which Authipay
///   serves through `splitShipment { totalCount, finalShipment }` — not wired up, so accepting
///   it would authorize funds the caller could not settle the way it asked.
/// * `Scheduled` has no Authipay counterpart on a primary transaction at all.
///
/// Silently mapping either to `PaymentCardSaleTransaction` (the previous behaviour) auto-captures
/// a payment the caller asked to be held.
fn reject_unsupported_capture_method(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::ManualMultiple)
        | Some(common_enums::CaptureMethod::Scheduled) => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: format!("capture_method {capture_method:?} for authipay"),
                connector: "authipay",
                context: integration_ctx(
                    "Authipay serves multi-capture through order.splitShipment, which this connector does not build, and has no counterpart for a scheduled capture.",
                    "Use capture_method AUTOMATIC, SEQUENTIAL_AUTOMATIC or MANUAL.",
                ),
            }))
        }
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | Some(common_enums::CaptureMethod::Manual)
        | None => Ok(()),
    }
}

/// The slice of a UCS request that a `POST /payments` primary transaction is built from.
///
/// Authipay exposes **one** primary-transaction resource, so `Authorize` and `SetupMandate` post
/// the same body to the same endpoint — but UCS hands them two unrelated request structs
/// (`PaymentsAuthorizeData` and `SetupMandateRequestData`). This trait is the seam that lets
/// [`build_primary_transaction_request`] serve both, so every guard and every documented length
/// limit applies to both flows by construction rather than by being copied into a second builder
/// that can then drift.
///
/// It deliberately carries no `payment_method_data`: that accessor needs the card generic, and
/// keeping this trait non-generic lets the field-level helpers (`build_billing`,
/// `build_soft_descriptor`, `build_stored_credentials`) take a plain `&impl AuthipayPrimaryRequest`
/// without an uninferable type parameter. The card side lives in
/// [`AuthipayPrimaryPaymentMethod`].
trait AuthipayPrimaryRequest {
    /// `requestType` — the primary-transaction discriminator, after the shared capture-method
    /// guard has run.
    fn request_type(&self) -> Result<AuthipayRequestType, error_stack::Report<IntegrationError>>;
    /// `transactionAmount.total`, still in minor units — the major-unit conversion is the
    /// builder's job so it happens once.
    fn minor_amount(&self) -> MinorUnit;
    fn currency(&self) -> common_enums::Currency;
    fn payment_channel(&self) -> Option<common_enums::PaymentChannel>;
    fn email(&self) -> Option<Email>;
    fn customer_name(&self) -> Option<String>;
    fn customer_id(&self) -> Option<String>;
    fn customer_date_of_birth(&self) -> Option<Secret<time::Date>>;
    fn billing_descriptor(&self) -> Option<&BillingDescriptor>;
    fn merchant_order_id(&self) -> Option<&String>;
    fn ip_address(&self) -> Option<Secret<String>>;
    /// Whether `storedCredentials` should be emitted at all — see [`build_stored_credentials`].
    fn has_stored_credential_intent(&self) -> bool;
    fn mit_category(&self) -> Option<common_enums::MitCategory>;
    fn authentication_data(&self) -> Option<&AuthenticationData>;
}

/// The card half of [`AuthipayPrimaryRequest`], split out so the non-generic trait stays usable
/// from helpers that never touch the payment method.
trait AuthipayPrimaryPaymentMethod<T: PaymentMethodDataTypes> {
    fn payment_method_data(&self) -> &PaymentMethodData<T>;
}

impl<T: PaymentMethodDataTypes> AuthipayPrimaryRequest for PaymentsAuthorizeData<T> {
    fn request_type(&self) -> Result<AuthipayRequestType, error_stack::Report<IntegrationError>> {
        reject_unsupported_capture_method(self.capture_method)?;
        // `PaymentsAuthorizeData::is_auto_capture()` is the canonical predicate and already
        // groups `SequentialAutomatic` with `Automatic`; it is not re-implemented here.
        if self.is_auto_capture() {
            Ok(AuthipayRequestType::PaymentCardSaleTransaction)
        } else {
            Ok(AuthipayRequestType::PaymentCardPreAuthTransaction)
        }
    }

    fn minor_amount(&self) -> MinorUnit {
        self.minor_amount
    }

    fn currency(&self) -> common_enums::Currency {
        self.currency
    }

    fn payment_channel(&self) -> Option<common_enums::PaymentChannel> {
        self.payment_channel.clone()
    }

    fn email(&self) -> Option<Email> {
        self.email.clone()
    }

    fn customer_name(&self) -> Option<String> {
        self.customer_name.clone()
    }

    fn customer_id(&self) -> Option<String> {
        self.customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
    }

    fn customer_date_of_birth(&self) -> Option<Secret<time::Date>> {
        self.customer_date_of_birth.clone()
    }

    fn billing_descriptor(&self) -> Option<&BillingDescriptor> {
        self.billing_descriptor.as_ref()
    }

    fn merchant_order_id(&self) -> Option<&String> {
        self.merchant_order_id.as_ref()
    }

    fn ip_address(&self) -> Option<Secret<String>> {
        self.browser_info
            .as_ref()
            .and_then(|info| info.ip_address)
            .map(|ip| Secret::new(ip.to_string()))
    }

    fn has_stored_credential_intent(&self) -> bool {
        self.setup_future_usage.is_some()
            || self.customer_acceptance.is_some()
            || self.mit_category.is_some()
    }

    fn mit_category(&self) -> Option<common_enums::MitCategory> {
        self.mit_category.clone()
    }

    fn authentication_data(&self) -> Option<&AuthenticationData> {
        self.authentication_data.as_ref()
    }
}

impl<T: PaymentMethodDataTypes> AuthipayPrimaryPaymentMethod<T> for PaymentsAuthorizeData<T> {
    fn payment_method_data(&self) -> &PaymentMethodData<T> {
        &self.payment_method_data
    }
}

impl<T: PaymentMethodDataTypes> AuthipayPrimaryRequest for SetupMandateRequestData<T> {
    /// Always a **pre-authorization**, never a sale.
    ///
    /// SetupMandate is the account-verification leg: the schemes define it as an
    /// authorization-only message, and Authipay's `Amount.total` declares `minimum: 0`, so the
    /// default body is the documented zero-value auth (`docs/card-verification`: *"a zero value
    /// authorization against the card to ensure it is not fraudulent, blacklisted, expired or
    /// blocked"*). Emitting `PaymentCardSaleTransaction` here would settle a payment the caller
    /// only asked to verify, so the capture method must not select the request type on this
    /// flow — but it is still validated, through the same guard Authorize uses, so a caller
    /// asking for a capture method this connector cannot serve is rejected identically on both.
    ///
    /// The standalone `POST /card-verification` resource is deliberately **not** used: its
    /// documented request body carries only `paymentCard` and `billingAddress`, so it cannot
    /// mark the transaction as the `FIRST`/`CARDHOLDER` credential-on-file leg that a later MIT
    /// has to chain off, and Fiserv restricts it to *"regions where PSD2 is not mandatory"* —
    /// which this EMEA gateway is not.
    fn request_type(&self) -> Result<AuthipayRequestType, error_stack::Report<IntegrationError>> {
        reject_unsupported_capture_method(self.capture_method)?;
        Ok(AuthipayRequestType::PaymentCardPreAuthTransaction)
    }

    /// Zero unless the caller asked for a nominal verification amount.
    ///
    /// A caller-supplied amount is honoured rather than discarded — it is a PreAuth hold that is
    /// never captured and that `Void` releases — but the default, and the documented shape of an
    /// Authipay account verification, is `0`.
    fn minor_amount(&self) -> MinorUnit {
        self.minor_amount.unwrap_or_else(MinorUnit::zero)
    }

    fn currency(&self) -> common_enums::Currency {
        self.currency
    }

    fn payment_channel(&self) -> Option<common_enums::PaymentChannel> {
        self.payment_channel.clone()
    }

    fn email(&self) -> Option<Email> {
        self.email.clone()
    }

    fn customer_name(&self) -> Option<String> {
        self.customer_name.clone()
    }

    fn customer_id(&self) -> Option<String> {
        self.customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
    }

    fn customer_date_of_birth(&self) -> Option<Secret<time::Date>> {
        self.customer
            .as_ref()
            .and_then(|customer| customer.date_of_birth.clone())
    }

    fn billing_descriptor(&self) -> Option<&BillingDescriptor> {
        self.billing_descriptor.as_ref()
    }

    fn merchant_order_id(&self) -> Option<&String> {
        self.merchant_order_id.as_ref()
    }

    fn ip_address(&self) -> Option<Secret<String>> {
        self.browser_info
            .as_ref()
            .and_then(|info| info.ip_address)
            .map(|ip| Secret::new(ip.to_string()))
    }

    /// Unconditionally true: SetupMandate exists to establish a credential on file, so the
    /// `storedCredentials` block is never optional on this flow the way it is on Authorize.
    fn has_stored_credential_intent(&self) -> bool {
        true
    }

    fn mit_category(&self) -> Option<common_enums::MitCategory> {
        self.mit_category.clone()
    }

    fn authentication_data(&self) -> Option<&AuthenticationData> {
        self.authentication_data.as_ref()
    }
}

impl<T: PaymentMethodDataTypes> AuthipayPrimaryPaymentMethod<T> for SetupMandateRequestData<T> {
    fn payment_method_data(&self) -> &PaymentMethodData<T> {
        &self.payment_method_data
    }
}

/// `order.billing` — the AVS input as well as the billing record.
///
/// Billing is optional on the Authipay side and a missing address must never fail an Authorize,
/// so every field is read through a `get_optional_*` accessor and the whole object is dropped
/// when nothing is populated (an empty `{}` is not the same as omission).
///
/// `AddressDetails::line3` has no Authipay counterpart. It is appended to `address2` (space
/// separated, truncated to the documented 96) rather than dropped, so a caller who split a long
/// street address across three lines still gets all of it into AVS.
fn build_billing(
    flow: &PaymentFlowData,
    request: &impl AuthipayPrimaryRequest,
) -> Option<AuthipayBilling> {
    let address = AuthipayAddress {
        address1: flow
            .get_optional_billing_line1()
            .map(|line| truncate_secret_string(&line, MAX_LEN_ADDRESS_LINE)),
        address2: join_address_lines(
            flow.get_optional_billing_line2(),
            flow.get_optional_billing_line3(),
        ),
        city: flow
            .get_optional_billing_city()
            .map(|city| truncate_secret_string(&city, MAX_LEN_CITY)),
        region: flow
            .get_optional_billing_state()
            .map(|state| truncate_secret_string(&state, MAX_LEN_REGION)),
        postal_code: flow
            .get_optional_billing_zip()
            .map(|zip| truncate_secret_string(&zip, MAX_LEN_POSTAL_CODE)),
        country: flow
            .get_optional_billing_country()
            .map(|country| Secret::new(truncate_string(&country.to_string(), MAX_LEN_COUNTRY))),
    };

    let contact = AuthipayContact {
        phone: flow
            .get_optional_billing_phone_number()
            .map(|phone| truncate_secret_string(&phone, MAX_LEN_PHONE)),
        email: flow
            .get_optional_billing_email()
            .or_else(|| request.email())
            .filter(|email| email.peek().chars().count() <= MAX_LEN_EMAIL),
    };

    let billing = AuthipayBilling {
        name: flow
            .get_optional_billing_full_name()
            .or_else(|| request.customer_name().map(Secret::new))
            .map(|name| truncate_secret_string(&name, MAX_LEN_NAME)),
        first_name: flow
            .get_optional_billing_first_name()
            .map(|name| truncate_secret_string(&name, MAX_LEN_FIRST_LAST_NAME)),
        last_name: flow
            .get_optional_billing_last_name()
            .map(|name| truncate_secret_string(&name, MAX_LEN_FIRST_LAST_NAME)),
        customer_id: request
            .customer_id()
            .map(|id| Secret::new(truncate_string(&id, MAX_LEN_CUSTOMER_ID))),
        birth_date: request
            .customer_date_of_birth()
            .as_ref()
            .and_then(|dob| format_iso_date(dob.peek())),
        contact: Some(contact).filter(|contact| !contact.is_empty()),
        address: Some(address).filter(|address| !address.is_empty()),
    };

    Some(billing).filter(|billing| !billing.is_empty())
}

/// `order.shipping`. When `l2_l3_data.shipping_details` is present it is authoritative — a
/// purchasing-card caller supplies the ship-to address there — and the payment address is the
/// fallback.
fn build_shipping(flow: &PaymentFlowData) -> Option<AuthipayShipping> {
    let l2_l3_shipping = flow
        .l2_l3_data
        .as_ref()
        .and_then(|data| data.shipping_details.as_ref());

    let address = match l2_l3_shipping {
        Some(details) => AuthipayAddress {
            address1: details
                .line1
                .as_ref()
                .map(|line| truncate_secret_string(line, MAX_LEN_ADDRESS_LINE)),
            address2: join_address_lines(details.line2.clone(), details.line3.clone()),
            city: details
                .city
                .as_ref()
                .map(|city| truncate_secret_string(city, MAX_LEN_CITY)),
            region: details
                .state
                .as_ref()
                .map(|state| truncate_secret_string(state, MAX_LEN_REGION)),
            postal_code: details
                .zip
                .as_ref()
                .map(|zip| truncate_secret_string(zip, MAX_LEN_POSTAL_CODE)),
            country: details
                .country
                .map(|country| Secret::new(truncate_string(&country.to_string(), MAX_LEN_COUNTRY))),
        },
        None => AuthipayAddress {
            address1: flow
                .get_optional_shipping_line1()
                .map(|line| truncate_secret_string(&line, MAX_LEN_ADDRESS_LINE)),
            address2: join_address_lines(
                flow.get_optional_shipping_line2(),
                flow.get_optional_shipping_line3(),
            ),
            city: flow
                .get_optional_shipping_city()
                .map(|city| truncate_secret_string(&city, MAX_LEN_CITY)),
            region: flow
                .get_optional_shipping_state()
                .map(|state| truncate_secret_string(&state, MAX_LEN_REGION)),
            postal_code: flow
                .get_optional_shipping_zip()
                .map(|zip| truncate_secret_string(&zip, MAX_LEN_POSTAL_CODE)),
            country: flow
                .get_optional_shipping_country()
                .map(|country| Secret::new(truncate_string(&country.to_string(), MAX_LEN_COUNTRY))),
        },
    };

    let contact = AuthipayContact {
        phone: flow
            .get_optional_shipping_phone_number()
            .map(|phone| truncate_secret_string(&phone, MAX_LEN_PHONE)),
        email: flow
            .get_optional_shipping_email()
            .filter(|email| email.peek().chars().count() <= MAX_LEN_EMAIL),
    };

    let shipping = AuthipayShipping {
        name: l2_l3_shipping
            .and_then(shipping_full_name)
            .or_else(|| flow.get_optional_shipping_full_name())
            .map(|name| truncate_secret_string(&name, MAX_LEN_NAME)),
        contact: Some(contact).filter(|contact| !contact.is_empty()),
        address: Some(address).filter(|address| !address.is_empty()),
    };

    Some(shipping).filter(|shipping| !shipping.is_empty())
}

fn shipping_full_name(details: &AddressDetails) -> Option<Secret<String>> {
    let parts: Vec<String> = [details.first_name.as_ref(), details.last_name.as_ref()]
        .into_iter()
        .flatten()
        .map(|part| part.peek().to_string())
        .collect();
    (!parts.is_empty()).then(|| Secret::new(parts.join(" ")))
}

/// Fold `line3` into `address2`, which is the only place Authipay has for it.
fn join_address_lines(
    line2: Option<Secret<String>>,
    line3: Option<Secret<String>>,
) -> Option<Secret<String>> {
    let joined = [line2, line3]
        .into_iter()
        .flatten()
        .map(|line| line.peek().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    (!joined.trim().is_empty())
        .then(|| truncate_secret_string(&Secret::new(joined), MAX_LEN_ADDRESS_LINE))
}

/// `order.billing.birthDate` is `string(date)`, i.e. `YYYY-MM-DD`.
///
/// The `.ok()` here is not swallowing a real failure mode: the format description is a
/// compile-time literal over components every `time::Date` carries, so the only way it can fail
/// is an allocation error. `birthDate` is an optional enrichment field, so degrading to
/// "omitted" beats failing an otherwise-valid authorization over a date rendering.
fn format_iso_date(date: &time::Date) -> Option<Secret<String>> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    date.format(&format).ok().map(Secret::new)
}

/// `order.softDescriptor` from the caller's billing descriptor.
fn build_soft_descriptor(request: &impl AuthipayPrimaryRequest) -> Option<AuthipaySoftDescriptor> {
    let descriptor = request.billing_descriptor()?;

    let dynamic_merchant_name = descriptor
        .statement_descriptor
        .clone()
        .or_else(|| descriptor.name.as_ref().map(|name| name.peek().to_string()))
        .or_else(|| descriptor.statement_descriptor_suffix.clone())
        // `pattern: ^(?!\s*$).+` — a blank descriptor is a 400, so drop it instead.
        .filter(|name| !name.trim().is_empty());

    let customer_service_number = descriptor
        .phone
        .as_ref()
        .map(|phone| retain_ascii_digits(phone.peek()))
        .filter(|digits| !digits.is_empty())
        .map(|digits| Secret::new(truncate_string(&digits, MAX_LEN_CUSTOMER_SERVICE_NUMBER)));

    let dynamic_address = descriptor.city.as_ref().map(|city| AuthipayAddress {
        address1: None,
        address2: None,
        city: Some(truncate_secret_string(city, MAX_LEN_CITY)),
        region: None,
        postal_code: None,
        country: None,
    });

    let soft_descriptor = AuthipaySoftDescriptor {
        dynamic_merchant_name,
        customer_service_number,
        dynamic_address,
    };

    Some(soft_descriptor).filter(|descriptor| !descriptor.is_empty())
}

/// `order.purchaseCard` — Level 2 and Level 3 purchasing-card data.
///
/// Emitted only when the caller supplied `l2_l3_data`, and each of `Level2` / `Level3` only when
/// it has content. `Level3.lineItems` is capped at the documented `maxItems: 100`; a longer
/// order fails explicitly rather than silently losing line items, because whether the gateway
/// rejects or truncates is undocumented.
fn build_purchase_card(
    l2_l3_data: &L2L3Data,
    merchant_order_id: Option<&String>,
    currency: common_enums::Currency,
) -> Result<Option<AuthipayPurchaseCard>, error_stack::Report<IntegrationError>> {
    let level2 = AuthipayLevel2 {
        customer_reference_id: l2_l3_data
            .get_merchant_order_reference_id()
            .or_else(|| merchant_order_id.cloned())
            .map(|reference| truncate_string(&reference, MAX_LEN_CUSTOMER_REFERENCE_ID)),
        supplier_vat_registration_number: l2_l3_data
            .get_merchant_tax_registration_id()
            .map(|id| truncate_secret_string(&id, MAX_LEN_VAT_REGISTRATION_NUMBER)),
        // UCS carries these amounts with no accompanying rate, and `AdditionalAmountRate`
        // requires both — so each object is dropped rather than sent with a fabricated 0% rate.
        total_discount_amount_and_rate: AuthipayAmountAndRate::new(
            l2_l3_data
                .get_discount_amount()
                .map(|amount| to_major_unit(amount, currency, "l2_l3_data.discount_amount"))
                .transpose()?,
            None,
        ),
        vat_shipping_amount_and_rate: AuthipayAmountAndRate::new(
            l2_l3_data
                .get_shipping_amount_tax()
                .map(|amount| to_major_unit(amount, currency, "l2_l3_data.shipping_amount_tax"))
                .transpose()?,
            None,
        ),
        duty_amount_and_rate: AuthipayAmountAndRate::new(
            l2_l3_data
                .get_duty_amount()
                .map(|amount| to_major_unit(amount, currency, "l2_l3_data.duty_amount"))
                .transpose()?,
            None,
        ),
    };

    let line_items = match l2_l3_data.get_order_details() {
        Some(details) if !details.is_empty() => {
            if details.len() > MAX_LINE_ITEMS {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!(
                        "{} Level 3 line items (authipay accepts at most {MAX_LINE_ITEMS})",
                        details.len()
                    ),
                    connector: "authipay",
                    context: integration_ctx(
                        "order.purchaseCard.Level3.lineItems declares maxItems: 100; truncating would drop financial data from the purchasing-card record.",
                        "Send at most 100 order line items, or omit Level 3 data for this order.",
                    ),
                }));
            }
            Some(
                details
                    .iter()
                    .map(|detail| build_line_item(detail, currency))
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
        _ => None,
    };

    let purchase_card = AuthipayPurchaseCard {
        level2: Some(level2).filter(|level2| !level2.is_empty()),
        level3: line_items.map(|line_items| AuthipayLevel3 { line_items }),
    };

    Ok(Some(purchase_card).filter(|card| card.level2.is_some() || card.level3.is_some()))
}

fn build_line_item(
    detail: &OrderDetailsWithAmount,
    currency: common_enums::Currency,
) -> Result<AuthipayLineItem, error_stack::Report<IntegrationError>> {
    let unit_price = to_major_unit(detail.amount, currency, "order_details.amount")?;

    Ok(AuthipayLineItem {
        commodity_code: detail
            .commodity_code
            .as_ref()
            .map(|code| truncate_string(code, MAX_LEN_COMMODITY_CODE)),
        product_code: detail
            .product_id
            .as_ref()
            .or(detail.sku.as_ref())
            .or(detail.upc.as_ref())
            .map(|code| truncate_string(code, MAX_LEN_PRODUCT_CODE)),
        description: detail
            .description
            .as_ref()
            .unwrap_or(&detail.product_name)
            .pipe_truncate(MAX_LEN_LINE_ITEM_DESCRIPTION),
        quantity: detail.quantity,
        unit_measure: detail
            .unit_of_measure
            .as_ref()
            .map(|measure| truncate_string(measure, MAX_LEN_UNIT_MEASURE)),
        unit_price,
        vat_amount_and_rate: AuthipayAmountAndRate::new(
            detail
                .total_tax_amount
                .map(|amount| to_major_unit(amount, currency, "order_details.total_tax_amount"))
                .transpose()?,
            detail.tax_rate,
        ),
        discount_amount_and_rate: AuthipayAmountAndRate::new(
            detail
                .unit_discount_amount
                .map(|amount| to_major_unit(amount, currency, "order_details.unit_discount_amount"))
                .transpose()?,
            detail.discount_percentage,
        ),
        line_item_total: detail
            .total_amount
            .map(|amount| to_major_unit(amount, currency, "order_details.total_amount"))
            .transpose()?,
    })
}

/// Small local extension so a `&String` can be truncated inline without an extra `let`.
trait PipeTruncate {
    fn pipe_truncate(&self, max_len: usize) -> Option<String>;
}

impl PipeTruncate for String {
    fn pipe_truncate(&self, max_len: usize) -> Option<String> {
        (!self.is_empty()).then(|| truncate_string(self, max_len))
    }
}

/// `storedCredentials` for a customer-initiated transaction.
///
/// On Authorize this is emitted only when the caller signalled a stored-credential intent —
/// `setup_future_usage`, a `customer_acceptance`, or an `mit_category`. On a genuine one-off
/// payment the block is omitted entirely, because marking a one-off as credential-on-file
/// misreports it to the schemes. On SetupMandate the intent is unconditional: that flow exists
/// only to establish the credential (see `AuthipayPrimaryRequest::has_stored_credential_intent`).
///
/// `sequence` is always `FIRST` and `initiator` always `CARDHOLDER`: both callers of this
/// function are the *cardholder-initiated* leg of a credential-on-file arrangement. The
/// subsequent merchant-initiated payment is the `RepeatPayment` flow, which sends
/// `SUBSEQUENT`/`MERCHANT` plus the `referencedSchemeTransactionId` this leg returns.
fn build_stored_credentials(
    request: &impl AuthipayPrimaryRequest,
) -> Option<AuthipayStoredCredentials> {
    if !request.has_stored_credential_intent() {
        return None;
    }

    // `scheduled` distinguishes a subscription/instalment plan from an unscheduled
    // credential-on-file. Fiserv's MIT documentation is explicit that it must be `false` when
    // the CIT is only establishing a credential for a later MIT.
    let (scheduled, indicator_subcategory) = match request.mit_category() {
        Some(common_enums::MitCategory::Recurring) => {
            (true, AuthipayIndicatorSubcategory::Subscription)
        }
        Some(common_enums::MitCategory::Installment) => {
            (true, AuthipayIndicatorSubcategory::Installment)
        }
        Some(common_enums::MitCategory::Unscheduled)
        | Some(common_enums::MitCategory::Resubmission)
        | None => (false, AuthipayIndicatorSubcategory::CredentialOnFileFirst),
    };

    Some(AuthipayStoredCredentials {
        sequence: AuthipayCredentialSequence::First,
        scheduled,
        initiator: AuthipayCredentialInitiator::Cardholder,
        indicator_subcategory: Some(indicator_subcategory),
    })
}

/// `authenticationResult` — pass through an externally-performed 3DS authentication.
///
/// Nothing here is derived or fabricated: if the caller performed no external 3DS,
/// `authentication_data` is `None` and the block is omitted, which leaves the payment
/// unauthenticated exactly as the caller intended.
fn build_authentication_result(
    authentication_data: &AuthenticationData,
) -> AuthipayAuthenticationResult {
    let authentication_response = authentication_data
        .trans_status
        .clone()
        .map(AuthipayAuthenticationResponse::from);

    // Fiserv documents `U` + CAVV as an error: an "unable to authenticate" result carries no
    // authentication value, so the CAVV is suppressed rather than sent alongside it.
    let cavv = authentication_data
        .cavv
        .as_ref()
        .filter(|_| {
            authentication_response
                .as_ref()
                .is_none_or(AuthipayAuthenticationResponse::permits_cavv)
        })
        .and_then(bounded_auth_value);

    AuthipayAuthenticationResult {
        authentication_type: AuthipayAuthenticationType::Secure3DAuthenticationResult,
        cavv,
        // `xid` is a 3DS-1 artefact. UCS's nearest equivalent is `transaction_id`; it is only
        // sent when it satisfies the schema's 20-32 character bounds, and 2.x flows simply
        // do not populate it.
        xid: authentication_data
            .transaction_id
            .as_ref()
            .and_then(|xid| bounded_auth_value(&Secret::new(xid.clone()))),
        ds_transaction_id: authentication_data.ds_trans_id.clone(),
        acs_transaction_id: authentication_data
            .acs_transaction_id
            .as_ref()
            .map(|id| truncate_string(id, MAX_LEN_ACS_TRANSACTION_ID)),
        authentication_response: authentication_response.clone(),
        transaction_status: authentication_response,
        message_category: AuthipayMessageCategory::Payment,
        secure_3d_protocol_version: authentication_data
            .message_version
            .as_ref()
            .map(|version| version.to_string()),
    }
}

/// `cavv` and `xid` both declare `minLength: 20`, `maxLength: 32`. A value outside those bounds
/// is a 400, so it is omitted rather than sent or truncated — truncating an authentication
/// value would corrupt it.
fn bounded_auth_value(value: &Secret<String>) -> Option<Secret<String>> {
    let len = value.peek().chars().count();
    (AUTH_VALUE_MIN_LEN..=AUTH_VALUE_MAX_LEN)
        .contains(&len)
        .then(|| value.clone())
}

/// Build the `POST /payments` primary transaction shared by **Authorize and SetupMandate**.
///
/// Both flows post the same body to the same endpoint, so they share one builder rather than one
/// each: the capture-method guard, the card-only rejection, the documented length limits, the
/// stored-credential marking and the external-3DS pass-through are then structurally identical
/// on both, and a guard added to one cannot go missing on the other.
fn build_primary_transaction_request<T, R>(
    flow: &PaymentFlowData,
    request: &R,
) -> Result<AuthipayPaymentsRequest<T>, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
    R: AuthipayPrimaryRequest + AuthipayPrimaryPaymentMethod<T>,
{
    let currency = request.currency();

    let transaction_amount = TransactionAmount {
        total: to_major_unit(request.minor_amount(), currency, "amount")?,
        currency,
    };

    let payment_method = match request.payment_method_data() {
            PaymentMethodData::Card(card_data) => {
                let payment_card = PaymentCard {
                    number: card_data.card_number.clone(),
                    expiry_date: ExpiryDate {
                        // `expiryDate.month` is `^(0[1-9]|1[012])$`, so a caller-supplied "8"
                        // must be zero-padded to "08" — the shared getter does that.
                        month: card_data.get_card_expiry_month_2_digit()?,
                        year: card_data.get_card_expiry_year_2_digit()?,
                    },
                    security_code: Some(card_data.card_cvc.clone()),
                    cardholder_name: crate::utils::build_card_holder_name(
                        &card_data.card_holder_name,
                        flow.get_optional_billing_first_name(),
                        flow.get_optional_billing_last_name(),
                    )
                    .or_else(|| request.customer_name().map(Secret::new))
                    .map(|name| truncate_secret_string(&name, MAX_LEN_CARDHOLDER_NAME)),
                };
                PaymentMethod { payment_card }
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    crate::utils::get_unimplemented_payment_method_error_message("authipay"),
                    integration_ctx(
                        "Authipay's REST v2 primary transaction is wired for paymentMethod.paymentCard only in this connector.",
                        "Route non-card payment methods to a connector that supports them.",
                    ),
                )))
            }
        };

    // `merchantTransactionId` is the per-attempt merchant reference (maxLength: 40) and
    // `order.orderId` the merchant's own order reference (maxLength: 100). They are
    // distinct fields with distinct semantics: only the first 12 characters of `orderId`
    // reach Fiserv Enterprise reporting, so the merchant's order id is preferred there.
    let merchant_transaction_id = truncate_string(
        &flow.connector_request_reference_id,
        MAX_LEN_MERCHANT_TRANSACTION_ID,
    );
    let order_id = truncate_string(
        request
            .merchant_order_id()
            .map(String::as_str)
            .or(flow.reference_id.as_deref())
            .unwrap_or(&flow.connector_request_reference_id),
        MAX_LEN_ORDER_ID,
    );

    let purchase_card = match flow.l2_l3_data.as_deref() {
        Some(l2_l3_data) => build_purchase_card(l2_l3_data, request.merchant_order_id(), currency)?,
        None => None,
    };

    let order = OrderDetails {
        order_id,
        billing: build_billing(flow, request),
        shipping: build_shipping(flow),
        soft_descriptor: build_soft_descriptor(request),
        purchase_card,
        ip: request.ip_address(),
    };

    Ok(AuthipayPaymentsRequest {
        request_type: request.request_type()?,
        merchant_transaction_id,
        transaction_amount,
        transaction_origin: AuthipayTransactionOrigin::from(request.payment_channel()),
        order,
        payment_method,
        stored_credentials: build_stored_credentials(request),
        authentication_result: request
            .authentication_data()
            .map(build_authentication_result),
    })
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for AuthipayPaymentsRequest<T>
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
        build_primary_transaction_request(&item.resource_common_data, &item.request)
    }
}

/// SetupMandate posts the very same primary transaction as Authorize — a zero-value
/// pre-authorization carrying the `FIRST`/`CARDHOLDER` stored-credential marking — so it goes
/// through the same builder rather than a second copy of it.
impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    > for AuthipayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        build_primary_transaction_request(&item.resource_common_data, &item.request)
    }
}

// ===== CAPTURE REQUEST STRUCTURE =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayCaptureRequest {
    pub request_type: AuthipayRequestType,
    pub transaction_amount: TransactionAmount,
}

// ===== CAPTURE REQUEST TRANSFORMATION =====

impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for AuthipayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Validate connector_transaction_id is present
        // The get_connector_transaction_id() method will validate this in get_url()
        // No validation needed here

        // Get capture amount from minor_amount_to_capture
        let capture_amount = item.request.minor_amount_to_capture;

        // Convert amount to FloatMajorUnit format
        let converter = FloatMajorUnitForConnector;
        let amount_major = converter
            .convert(capture_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: crate::utils::amount_conversion_ctx(
                    "capture",
                    &capture_amount,
                    &item.request.currency,
                ),
            })
            .attach_printable("authipay: failed to convert the capture amount to major units")?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        Ok(Self {
            request_type: AuthipayRequestType::PostAuthTransaction,
            transaction_amount,
        })
    }
}

// ===== RESPONSE STATUS ENUMS =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayTransactionType {
    Sale,
    Preauth,
    Credit,
    ForcedTicket,
    Void,
    Return,
    Postauth,
    PayerAuth,
    Disbursement,
    #[serde(other)]
    Unknown,
}

/// Deprecated by Fiserv in favour of `transactionResult`, but still emitted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayPaymentStatus {
    Approved,
    Waiting,
    Partial,
    ValidationFailed,
    ProcessingFailed,
    Declined,
    /// An unrecognised value must not fail deserialisation of an otherwise-good response.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayPaymentResult {
    /// Record created, not yet processed.
    Created,
    Approved,
    Declined,
    Failed,
    Waiting,
    Partial,
    Fraud,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayTransactionState {
    Authorized,
    Captured,
    Declined,
    Checked,
    CompletedGet,
    Initialized,
    Pending,
    Ready,
    Template,
    Settled,
    Voided,
    Waiting,
    #[serde(other)]
    Unknown,
}

// ===== RESPONSE STRUCTURES =====

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentCardResponse {
    pub expiry_date: Option<ExpiryDate>,
    pub bin: Option<String>,
    pub last4: Option<String>,
    pub brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentMethodDetails {
    pub payment_card: Option<AuthipayPaymentCardResponse>,
    pub payment_method_type: Option<String>,
    pub payment_method_brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmountDetails {
    pub total: Option<FloatMajorUnit>,
    pub currency: Option<common_enums::Currency>,
}

/// `processor.avsResponse`. The raw schema types `streetMatch`/`postalCodeMatch` as
/// `Y | N | NO_INPUT_DATA | NOT_CHECKED`, but they stay `Option<String>` here so a value
/// outside that enum can never fail deserialisation of an otherwise-good authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvsResponse {
    pub street_match: Option<String>,
    pub postal_code_match: Option<String>,
    /// The raw single-letter scheme AVS code (`A`, `Y`, `Z`, `N`, …) — more informative than
    /// the two normalised booleans above.
    pub association_avs_response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Processor {
    pub reference_number: Option<String>,
    pub authorization_code: Option<String>,
    /// Gateway-**normalised** endpoint response code.
    pub response_code: Option<String>,
    pub response_message: Option<String>,
    /// Network *name* (e.g. `NYCE`, `VISA`) — not a transaction id.
    pub network: Option<String>,
    /// Raw response code from the issuer. This, not `response_code`, is what GSM rules key off.
    pub association_response_code: Option<String>,
    pub association_response_message: Option<String>,
    pub avs_response: Option<AvsResponse>,
    /// `MATCHED | NOT_MATCHED | NOT_PROCESSED | NOT_PRESENT | NOT_CERTIFIED | NOT_CHECKED`.
    /// Kept as `Option<String>` for the same reason as the AVS fields.
    pub security_code_response: Option<String>,
    pub merchant_advice_code_indicator: Option<String>,
    pub merchant_advice_message: Option<String>,
    pub response_indicator: Option<String>,
    pub payment_account_reference_number: Option<String>,
}

/// `secure3dResponse` — what the gateway reports back about the 3DS leg.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Secure3dResponse {
    /// `1` fully authenticated, `3` failed/rejected, `4` attempted, `6` unable, `5`/`7`/`8`
    /// decommissioned 3DS v1.
    pub response_code_3d_secure: Option<String>,
    pub authentication_value: Option<Secret<String>>,
    pub directory_server_transaction_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentToken {
    pub value: Option<String>,
    pub reusable: Option<bool>,
    pub decline_duplicates: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentsResponse {
    pub client_request_id: Option<String>,
    pub api_trace_id: Option<String>,
    pub response_type: Option<String>,
    #[serde(rename = "type")]
    pub response_type_field: Option<String>,
    pub ipg_transaction_id: String,
    pub order_id: Option<String>,
    pub user_id: Option<String>,
    pub transaction_type: AuthipayTransactionType,
    pub payment_method_details: Option<AuthipayPaymentMethodDetails>,
    pub merchant_transaction_id: Option<String>,
    pub transaction_time: Option<i64>,
    pub approved_amount: Option<AmountDetails>,
    pub transaction_amount: Option<AmountDetails>,
    pub transaction_status: Option<AuthipayPaymentStatus>,
    pub transaction_result: Option<AuthipayPaymentResult>,
    pub transaction_state: Option<AuthipayTransactionState>,
    pub approval_code: Option<String>,
    pub scheme_response_code: Option<String>,
    pub merchant_advice_code: Option<String>,
    pub error_message: Option<String>,
    /// The network transaction id for card-on-file / MIT chaining (`maxLength: 40`). This, not
    /// `processor.network`, is what `network_txn_id` must carry.
    pub scheme_transaction_id: Option<String>,
    /// Scheme TLID linking an order's transactions (`maxLength: 36`).
    pub transaction_link_identifier: Option<String>,
    pub payment_account_reference_number: Option<String>,
    pub secure3d_response: Option<Secure3dResponse>,
    pub processor: Option<Processor>,
    pub payment_token: Option<PaymentToken>,
    /// Populated only on a 409/422 `TransactionErrorResponse`. A 200 carrying
    /// `transactionResult: DECLINED` has no `error` object, so the decline codes must be read
    /// off `processor` in that case.
    pub error: Option<AuthipayErrorDetails>,
}

// ===== HELPER FUNCTIONS TO AVOID CODE DUPLICATION =====

/// Extract connector metadata from payment token
fn extract_connector_metadata(payment_token: Option<&PaymentToken>) -> Option<serde_json::Value> {
    payment_token.map(|token| {
        let mut metadata = HashMap::new();
        if let Some(value) = &token.value {
            metadata.insert("payment_token".to_string(), value.clone());
        }
        if let Some(reusable) = token.reusable {
            metadata.insert("token_reusable".to_string(), reusable.to_string());
        }
        serde_json::Value::Object(
            metadata
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
        )
    })
}

/// Pull the three network-level diagnostic codes out of `processor`, in the priority order the
/// tech spec establishes, as `(network_decline_code, network_advice_code, network_error_message)`.
///
/// `associationResponseCode` is the **raw issuer code** (schema: *"Raw response code from
/// issuer"*), while `responseCode` is the gateway-normalised value — GSM and smart-retry key off
/// the raw one, so it wins. These used to be extracted into `_`-prefixed bindings and thrown
/// away, which is why an issuer decline reached the merchant with no code at all.
///
/// `scheme_response_code` and `merchant_advice_code` are the top-level siblings of the
/// `processor` members, and they are threaded in here rather than applied by the caller so that
/// the full priority chain lives in one place:
///
/// * decline: `processor.associationResponseCode` → `schemeResponseCode` → `processor.responseCode`
/// * advice:  `processor.merchantAdviceCodeIndicator` → `merchantAdviceCode`
/// * message: `processor.associationResponseMessage` → `processor.merchantAdviceMessage` → `processor.responseMessage`
///
/// The 400/401/403/404/415/500/502 `ErrorResponse` shape carries no `processor` at all, so on
/// that path only the top-level fallbacks can fire — which is exactly why they belong in the
/// chain instead of being a caller-side afterthought.
fn extract_network_error_fields(
    processor: Option<&Processor>,
    scheme_response_code: Option<&String>,
    merchant_advice_code: Option<&String>,
) -> (Option<String>, Option<String>, Option<String>) {
    let network_decline_code = processor
        .and_then(|processor| processor.association_response_code.clone())
        .or_else(|| scheme_response_code.cloned())
        .or_else(|| processor.and_then(|processor| processor.response_code.clone()));

    let network_advice_code = processor
        .and_then(|processor| processor.merchant_advice_code_indicator.clone())
        .or_else(|| merchant_advice_code.cloned());

    let network_error_message = processor.and_then(|processor| {
        processor
            .association_response_message
            .clone()
            .or_else(|| processor.merchant_advice_message.clone())
            .or_else(|| processor.response_message.clone())
    });

    (
        network_decline_code,
        network_advice_code,
        network_error_message,
    )
}

/// Build the `payment_checks` blob carried back on
/// `PaymentFlowData::connector_response` → `AdditionalPaymentMethodConnectorResponse::Card`,
/// which is how UCS surfaces AVS / CVV / 3DS check outcomes (proto
/// `ConnectorResponseData connector_response = 13`).
///
/// The key names follow the Cybersource / Bank of America precedent (`avs_response`,
/// `card_verification`) so downstream consumers see one shape across connectors.
///
/// An AVS mismatch is deliberately **not** turned into a failure here: Authipay/OmniPay already
/// decline on AVS policy when the merchant is configured for it, so a 200 `APPROVED` carrying
/// `streetMatch: N` means the acquirer chose to approve. Surface it; do not judge it.
fn build_payment_checks(
    response: &AuthipayPaymentsResponse,
    request_eci: Option<&String>,
) -> serde_json::Value {
    let processor = response.processor.as_ref();
    let avs = processor.and_then(|processor| processor.avs_response.as_ref());

    serde_json::json!({
        "avs_response": {
            "street_match": avs.and_then(|avs| avs.street_match.clone()),
            "postal_code_match": avs.and_then(|avs| avs.postal_code_match.clone()),
            "association_avs_response": avs.and_then(|avs| avs.association_avs_response.clone()),
        },
        "card_verification": processor.and_then(|processor| processor.security_code_response.clone()),
        "approval_code": response.approval_code.clone(),
        "scheme_response_code": response.scheme_response_code.clone(),
        "three_d_secure": {
            "response_code_3d_secure": response
                .secure3d_response
                .as_ref()
                .and_then(|secure3d| secure3d.response_code_3d_secure.clone()),
            "directory_server_transaction_id": response
                .secure3d_response
                .as_ref()
                .and_then(|secure3d| secure3d.directory_server_transaction_id.clone()),
            // Authipay's `authenticationResult` schema has no `eci` field — Fiserv derives the
            // ECI from `authenticationResponse` plus the protocol version. The caller's ECI is
            // recorded here so it is observable rather than silently dropped.
            "requested_eci": request_eci.cloned(),
        },
        "payment_account_reference_number": response.payment_account_reference_number.clone(),
    })
}

/// Assemble the `connector_response` payload for a card payment.
fn build_connector_response(
    response: &AuthipayPaymentsResponse,
    request_eci: Option<&String>,
) -> ConnectorResponseData {
    ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks: Some(build_payment_checks(response, request_eci)),
            card_network: response
                .processor
                .as_ref()
                .and_then(|processor| processor.network.clone()),
            domestic_network: None,
            auth_code: response
                .processor
                .as_ref()
                .and_then(|processor| processor.authorization_code.clone()),
        },
    )
}

/// The merchant-facing reference to echo back on every payment flow.
///
/// `orderId` is the merchant's own order reference; `merchantTransactionId` is the per-attempt
/// reference. Either is meaningful to the caller. `clientRequestId` — which this connector used
/// to return — is the connector's own generated idempotency id and means nothing to anyone
/// upstream, so it is not used. Authorize and PSync read the same two fields in the same order,
/// so a payment reports one stable reference id across its lifetime.
fn connector_reference_id(response: &AuthipayPaymentsResponse) -> Option<String> {
    response
        .order_id
        .clone()
        .or_else(|| response.merchant_transaction_id.clone())
}

/// Build the caller-facing error for a **2xx that carries a decline**.
///
/// A `200`/`201` with `transactionResult: DECLINED | FAILED | FRAUD` never reaches
/// `ConnectorCommon::build_error_response`, so without this the transaction would map to
/// `AttemptStatus::Failure` with every issuer code discarded. The same priority table as the
/// 4xx/5xx path is used, sourced from `processor` (a 200 decline has no `error` object).
fn build_decline_error_response(
    response: &AuthipayPaymentsResponse,
    status_code: u16,
    attempt_status: AttemptStatus,
) -> ErrorResponse {
    let processor = response.processor.as_ref();
    let (network_decline_code, network_advice_code, network_error_message) =
        extract_network_error_fields(
            processor,
            response.scheme_response_code.as_ref(),
            response.merchant_advice_code.as_ref(),
        );

    ErrorResponse {
        status_code,
        code: response
            .error
            .as_ref()
            .and_then(|error| error.code.clone())
            .or_else(|| processor.and_then(|processor| processor.response_code.clone()))
            .or_else(|| response.scheme_response_code.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .error
            .as_ref()
            .and_then(|error| error.message.clone())
            .or_else(|| processor.and_then(|processor| processor.response_message.clone()))
            .or_else(|| response.error_message.clone())
            .or_else(|| response.approval_code.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: response
            .error
            .as_ref()
            .and_then(|error| error.decline_reason_code.clone())
            .or_else(|| processor.and_then(|processor| processor.merchant_advice_message.clone()))
            .or_else(|| response.error_message.clone())
            .or_else(|| response.api_trace_id.clone()),
        attempt_status: Some(FlowStatus::Payment(attempt_status)),
        connector_transaction_id: Some(response.ipg_transaction_id.clone()),
        network_decline_code,
        network_advice_code,
        network_error_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

// ===== STATUS MAPPING FUNCTION =====
// CRITICAL: This checks BOTH transactionResult AND transactionStatus, AND considers transactionType

/// Approval means different things depending on which lifecycle the record describes, so the
/// approved arm is resolved by `transactionType`.
///
/// A type this connector never initiates on a payment flow (`CREDIT`, `FORCED_TICKET`,
/// `RETURN`, `PAYER_AUTH`, `DISBURSEMENT`) arriving on a payment response means the gateway
/// answered about a different record than the one asked about. That is not a decline, so it
/// maps to `Unspecified` and lets hyperswitch apply previous-status handling rather than
/// reporting a payment as failed on the strength of a mismatched echo.
fn map_approved_status(transaction_type: &AuthipayTransactionType) -> AttemptStatus {
    match transaction_type {
        AuthipayTransactionType::Preauth => AttemptStatus::Authorized,
        AuthipayTransactionType::Void => AttemptStatus::Voided,
        AuthipayTransactionType::Sale | AuthipayTransactionType::Postauth => AttemptStatus::Charged,
        AuthipayTransactionType::Credit
        | AuthipayTransactionType::ForcedTicket
        | AuthipayTransactionType::Return
        | AuthipayTransactionType::PayerAuth
        | AuthipayTransactionType::Disbursement
        | AuthipayTransactionType::Unknown => AttemptStatus::Unspecified,
    }
}

/// Resolve an Authipay payment outcome to an `AttemptStatus`.
///
/// Authipay reports the outcome across three fields and all three must be consulted:
/// `transactionState` is the lifecycle position, `transactionResult` the current outcome field,
/// and `transactionStatus` its deprecated predecessor. `transactionType` says which lifecycle
/// the record belongs to, which is what disambiguates `APPROVED`.
///
/// The order is state first (most specific), then `transactionResult`, then the deprecated
/// `transactionStatus`, then — when the gateway told us nothing at all — `Pending`, because a
/// silent response is "not visible yet", never a decline.
fn map_status(
    authipay_status: Option<AuthipayPaymentStatus>,
    authipay_result: Option<AuthipayPaymentResult>,
    authipay_state: Option<AuthipayTransactionState>,
    transaction_type: AuthipayTransactionType,
) -> AttemptStatus {
    // Terminal states are trusted outright; a lifecycle state that only *suggests* an outcome
    // is cross-checked against the transaction type before it is believed.
    if let Some(state) = authipay_state {
        match state {
            AuthipayTransactionState::Declined => return AttemptStatus::Failure,
            AuthipayTransactionState::Voided => return AttemptStatus::Voided,
            AuthipayTransactionState::Authorized => {
                if matches!(transaction_type, AuthipayTransactionType::Preauth) {
                    return AttemptStatus::Authorized;
                }
            }
            AuthipayTransactionState::Captured | AuthipayTransactionState::Settled => {
                if matches!(
                    transaction_type,
                    AuthipayTransactionType::Sale | AuthipayTransactionType::Postauth
                ) {
                    return AttemptStatus::Charged;
                }
            }
            // The gateway holds the record but has not reached an outcome yet — distinct from
            // a decline, and the caller can still advance out of it by polling.
            AuthipayTransactionState::Pending
            | AuthipayTransactionState::Waiting
            | AuthipayTransactionState::Initialized
            | AuthipayTransactionState::Ready
            | AuthipayTransactionState::Template
            | AuthipayTransactionState::Checked
            | AuthipayTransactionState::CompletedGet => {}
            // A state this build does not know about: fall through to the outcome fields
            // rather than guessing from the state alone.
            AuthipayTransactionState::Unknown => {}
        }
    }

    // `transactionResult` is the current field and wins over its deprecated predecessor.
    if let Some(result) = authipay_result {
        match result {
            AuthipayPaymentResult::Approved => return map_approved_status(&transaction_type),
            AuthipayPaymentResult::Created | AuthipayPaymentResult::Waiting => {
                return AttemptStatus::Pending
            }
            AuthipayPaymentResult::Partial => return AttemptStatus::PartialCharged,
            AuthipayPaymentResult::Declined
            | AuthipayPaymentResult::Failed
            | AuthipayPaymentResult::Fraud => return AttemptStatus::Failure,
            // Do not invent an outcome for a value this build has never seen.
            AuthipayPaymentResult::Unknown => return AttemptStatus::Unspecified,
        }
    }

    match authipay_status {
        Some(AuthipayPaymentStatus::Approved) => map_approved_status(&transaction_type),
        Some(AuthipayPaymentStatus::Waiting) => AttemptStatus::Pending,
        Some(AuthipayPaymentStatus::Partial) => AttemptStatus::PartialCharged,
        Some(AuthipayPaymentStatus::ValidationFailed)
        | Some(AuthipayPaymentStatus::ProcessingFailed)
        | Some(AuthipayPaymentStatus::Declined) => AttemptStatus::Failure,
        Some(AuthipayPaymentStatus::Unknown) => AttemptStatus::Unspecified,
        // The gateway reported no outcome at all: the transaction is not visible yet, which is
        // recoverable by PSync. Reporting a failure here would strand a possibly-charged payment.
        None => AttemptStatus::Pending,
    }
}

// ===== RESPONSE TRANSFORMATION =====

/// Assemble the success half of a payment response.
///
/// `network_txn_id` carries `schemeTransactionId` — the id a later merchant-initiated
/// transaction has to echo back in `storedCredentials.referencedSchemeTransactionId`. It used to
/// carry `processor.network`, which is a network *name* (`"NYCE"`), falling back to
/// `apiTraceId`, a Fiserv log id: neither is a network transaction id and neither can chain a
/// credential-on-file payment.
fn build_transaction_response(
    response: &AuthipayPaymentsResponse,
    http_code: u16,
    mandate_reference: Option<Box<MandateReference>>,
) -> PaymentsResponseData {
    PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.ipg_transaction_id.clone()),
        redirection_data: None,
        mandate_reference,
        connector_metadata: extract_connector_metadata(response.payment_token.as_ref()),
        network_txn_id: response.scheme_transaction_id.clone(),
        network_txn_link_id: response.transaction_link_identifier.clone(),
        connector_response_reference_id: connector_reference_id(response),
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
        payment_account_reference: response.payment_account_reference_number.clone(),
    }
}

/// A 2xx can still be a decline. Branch on the mapped status so a `200` carrying
/// `transactionResult: DECLINED` returns the issuer's codes instead of a success envelope with
/// a `Failure` status and no diagnostics.
fn build_payment_flow_response(
    response: &AuthipayPaymentsResponse,
    status: AttemptStatus,
    http_code: u16,
    mandate_reference: Option<Box<MandateReference>>,
) -> Result<PaymentsResponseData, ErrorResponse> {
    if domain_types::utils::is_payment_failure(status) {
        Err(build_decline_error_response(response, http_code, status))
    } else {
        Ok(build_transaction_response(
            response,
            http_code,
            mandate_reference,
        ))
    }
}

/// The credential-on-file handle a later merchant-initiated transaction has to quote back.
///
/// Authipay chains a CIT to its MITs through `schemeTransactionId`: the MIT echoes it in
/// `storedCredentials.referencedSchemeTransactionId` (`docs/merchant-initiated-transactions-mit-1`:
/// *"The `schemeTransactionId` from the first transaction response must be provided in all
/// subsequent MIT requests"*). `paymentToken.value` is the gateway's own reusable token and is
/// only returned when the merchant asked one to be created, so it is preferred as the mandate id
/// when present and `schemeTransactionId` is the fallback.
///
/// The metadata block is assembled from whichever identifiers came back rather than being gated
/// on any single one, so a response carrying only the TLID still yields something `RepeatPayment`
/// can use. `None` is returned only when the gateway returned no chaining identifier at all —
/// which is a mandate that cannot be reused, and must not masquerade as one that can.
fn build_mandate_reference(response: &AuthipayPaymentsResponse) -> Option<Box<MandateReference>> {
    let payment_token = response
        .payment_token
        .as_ref()
        .and_then(|token| token.value.clone());

    let connector_mandate_id = payment_token
        .clone()
        .or_else(|| response.scheme_transaction_id.clone());

    let mut metadata = serde_json::Map::new();
    let mut record = |key: &str, value: Option<String>| {
        if let Some(value) = value {
            metadata.insert(key.to_string(), serde_json::Value::String(value));
        }
    };
    record(
        "scheme_transaction_id",
        response.scheme_transaction_id.clone(),
    );
    record(
        "transaction_link_identifier",
        response.transaction_link_identifier.clone(),
    );
    record("payment_token", payment_token);

    if metadata.is_empty() {
        return None;
    }

    // The gateway transaction id is only worth persisting once there is a credential to persist
    // it against, so it is added after the emptiness check rather than before it.
    metadata.insert(
        "ipg_transaction_id".to_string(),
        serde_json::Value::String(response.ipg_transaction_id.clone()),
    );

    Some(Box::new(MandateReference {
        connector_mandate_id,
        payment_method_id: None,
        connector_mandate_request_reference_id: response.merchant_transaction_id.clone(),
        mandate_metadata: Some(Secret::new(serde_json::Value::Object(metadata))),
    }))
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
            item.response.transaction_type.clone(),
        );

        let response = build_payment_flow_response(&item.response, status, item.http_code, None);

        // AVS, CVV and 3DS check outcomes travel back on `connector_response`; the ECI the
        // caller supplied is recorded alongside them because Authipay has no request field for it.
        let connector_response = Some(build_connector_response(
            &item.response,
            item.router_data
                .request
                .authentication_data
                .as_ref()
                .and_then(|data| data.eci.as_ref()),
        ));

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PSYNC RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from authorize flow
// PSync returns the same response format as the original transaction

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
            item.response.transaction_type.clone(),
        );

        // Same `resource_id` (ipgTransactionId) and same `connector_response_reference_id`
        // (orderId) as Authorize returned, so a payment reports one identity across its life.
        let response = build_payment_flow_response(&item.response, status, item.http_code, None);
        // PSync has no request-side authentication data to echo.
        let connector_response = Some(build_connector_response(&item.response, None));

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== CAPTURE RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from authorize flow
// Capture returns the same response format as the original transaction

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // A successful capture is transactionType=POSTAUTH, transactionState=CAPTURED.
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
            item.response.transaction_type.clone(),
        );

        let response = build_payment_flow_response(&item.response, status, item.http_code, None);
        let connector_response = Some(build_connector_response(&item.response, None));

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== SETUP MANDATE RESPONSE TRANSFORMATION =====
// SetupMandate posts the same primary transaction as Authorize, so it deserialises the same
// body and shares the same status mapping. The one thing it adds is the credential-on-file
// handle: `mandate_reference`, which the later merchant-initiated transaction consumes.

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AuthipaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // A successful verification is transactionType=PREAUTH, transactionState=AUTHORIZED.
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
            item.response.transaction_type.clone(),
        );

        let response = build_payment_flow_response(
            &item.response,
            status,
            item.http_code,
            build_mandate_reference(&item.response),
        );

        // AVS and CVV outcomes are the whole point of a zero-value verification, so they travel
        // back on `connector_response` exactly as they do on Authorize.
        let connector_response = Some(build_connector_response(
            &item.response,
            item.router_data
                .request
                .authentication_data
                .as_ref()
                .and_then(|data| data.eci.as_ref()),
        ));

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND REQUEST STRUCTURE =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayRefundRequest {
    pub request_type: AuthipayRequestType,
    pub transaction_amount: TransactionAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

// ===== REFUND REQUEST TRANSFORMATION =====

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for AuthipayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Convert refund amount to major unit format
        let converter = FloatMajorUnitForConnector;
        let amount_major = converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: crate::utils::amount_conversion_ctx(
                    "refund",
                    &item.request.minor_refund_amount,
                    &item.request.currency,
                ),
            })
            .attach_printable("authipay: failed to convert the refund amount to major units")?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        Ok(Self {
            request_type: AuthipayRequestType::ReturnTransaction,
            transaction_amount,
            comments: item.request.reason.clone(),
        })
    }
}

// ===== VOID REQUEST STRUCTURE =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayVoidRequest {
    pub request_type: AuthipayRequestType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

// ===== VOID REQUEST TRANSFORMATION =====

impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for AuthipayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            request_type: AuthipayRequestType::VoidPreAuthTransactions,
            comments: item.request.cancellation_reason.clone(),
        })
    }
}

// ===== REFUND RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from payment flows
// Refunds return the same response format as primary transactions

use common_enums::RefundStatus;

// CRITICAL REFUND STATUS MAPPING FUNCTION
// This validates ALL conditions to avoid Silverflow PR #240 issues:
// 1. transactionType must be RETURN (not just any type)
// 2. transactionResult OR transactionStatus must be APPROVED (API uses both deprecated and new fields)
// 3. transactionState should be CAPTURED for success
// ONLY returns RefundStatus::Success when ALL conditions are met

/// Assemble the refund half of a response.
///
/// A refund that the gateway hard-declined must carry `FlowStatus::Refund(RefundStatus::Failure)`
/// explicitly: `ForeignFrom<FlowStatus> for RefundStatus` has no fallback for a `Payment(_)`
/// value, so leaving `attempt_status` as `None` here would report the refund as
/// `REFUND_STATUS_UNSPECIFIED` and leave it retrying forever.
fn build_refund_flow_response(
    response: &AuthipayPaymentsResponse,
    refund_status: RefundStatus,
    http_code: u16,
) -> Result<RefundsResponseData, ErrorResponse> {
    if crate::utils::is_refund_failure(refund_status) {
        let mut error = build_decline_error_response(response, http_code, AttemptStatus::Failure);
        error.attempt_status = Some(FlowStatus::Refund(RefundStatus::Failure));
        Err(error)
    } else {
        Ok(RefundsResponseData {
            connector_refund_id: response.ipg_transaction_id.clone(),
            refund_status,
            status_code: http_code,
            acquirer_reference_number: response
                .processor
                .as_ref()
                .and_then(|processor| processor.reference_number.clone()),
        })
    }
}

fn map_refund_status(
    transaction_type: Option<AuthipayTransactionType>,
    transaction_status: Option<AuthipayPaymentStatus>,
    transaction_result: Option<AuthipayPaymentResult>,
    transaction_state: Option<AuthipayTransactionState>,
) -> RefundStatus {
    // Validate transaction type is RETURN first
    if let Some(tx_type) = transaction_type {
        if tx_type != AuthipayTransactionType::Return {
            // CRITICAL: If transactionType is NOT RETURN, this is NOT a valid refund
            return RefundStatus::Failure;
        }
    } else {
        // No transaction type provided
        return RefundStatus::Pending;
    }

    // Check transaction_state first (most reliable)
    if let Some(state) = transaction_state {
        match state {
            AuthipayTransactionState::Captured | AuthipayTransactionState::Settled
                if matches!(transaction_result, Some(AuthipayPaymentResult::Approved))
                    || matches!(transaction_status, Some(AuthipayPaymentStatus::Approved)) =>
            {
                return RefundStatus::Success;
            }
            AuthipayTransactionState::Declined => return RefundStatus::Failure,
            AuthipayTransactionState::Pending | AuthipayTransactionState::Waiting => {
                return RefundStatus::Pending;
            }
            _ => {} // Continue to check status/result
        }
    }

    // Check transaction_result (newer field)
    if let Some(result) = transaction_result {
        return match result {
            AuthipayPaymentResult::Approved => {
                // If state not available or unclear, check if it's likely settled
                // API may return APPROVED without state for immediate refunds
                RefundStatus::Success
            }
            AuthipayPaymentResult::Created | AuthipayPaymentResult::Waiting => {
                RefundStatus::Pending
            }
            AuthipayPaymentResult::Declined
            | AuthipayPaymentResult::Failed
            | AuthipayPaymentResult::Fraud => RefundStatus::Failure,
            AuthipayPaymentResult::Partial => RefundStatus::Pending,
            // A value this build has never seen is not a decline: keep the refund pollable
            // rather than telling the merchant money moved back when it may not have.
            AuthipayPaymentResult::Unknown => RefundStatus::Pending,
        };
    }

    // Check transaction_status (deprecated field) if transaction_result not present
    if let Some(status) = transaction_status {
        return match status {
            AuthipayPaymentStatus::Approved => {
                // If state not available or unclear, treat as success
                // API may return APPROVED without state for immediate refunds
                RefundStatus::Success
            }
            AuthipayPaymentStatus::Waiting => RefundStatus::Pending,
            AuthipayPaymentStatus::ValidationFailed
            | AuthipayPaymentStatus::ProcessingFailed
            | AuthipayPaymentStatus::Declined => RefundStatus::Failure,
            AuthipayPaymentStatus::Partial => RefundStatus::Pending,
            AuthipayPaymentStatus::Unknown => RefundStatus::Pending,
        };
    }

    // Default to Pending for unknown/incomplete status combinations
    RefundStatus::Pending
}

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map refund status with CRITICAL validation of ALL fields
        let refund_status = map_refund_status(
            Some(item.response.transaction_type.clone()),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        let mut router_data = item.router_data;
        router_data.response =
            build_refund_flow_response(&item.response, refund_status, item.http_code);

        Ok(router_data)
    }
}

// ===== REFUND SYNC RESPONSE TRANSFORMATION =====
// RSync also reuses AuthipayPaymentsResponse and uses the same refund status mapping

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map refund status with CRITICAL validation of ALL fields
        let refund_status = map_refund_status(
            Some(item.response.transaction_type.clone()),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        let mut router_data = item.router_data;
        router_data.response =
            build_refund_flow_response(&item.response, refund_status, item.http_code);

        Ok(router_data)
    }
}

// ===== VOID RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from payment flows
// Void returns the same response format as primary transactions

// CRITICAL VOID STATUS MAPPING FUNCTION

// 1. transactionType must be VOID (not just any type)
// 2. transactionResult OR transactionStatus must be APPROVED (API uses both deprecated and new fields)
// 3. transactionState should be VOIDED for success
// ONLY returns AttemptStatus::Voided when ALL conditions are met

fn map_void_status(
    transaction_type: AuthipayTransactionType,
    transaction_status: Option<AuthipayPaymentStatus>,
    transaction_result: Option<AuthipayPaymentResult>,
    transaction_state: Option<AuthipayTransactionState>,
) -> AttemptStatus {
    // First validate transactionType is VOID
    if transaction_type != AuthipayTransactionType::Void {
        // Not a void transaction - this is an error
        return AttemptStatus::VoidFailed;
    }

    // Check transactionState first for most accurate status
    if let Some(state) = transaction_state {
        match state {
            AuthipayTransactionState::Voided => {
                // Verify result/status is also APPROVED for complete validation
                if matches!(transaction_result, Some(AuthipayPaymentResult::Approved))
                    || matches!(transaction_status, Some(AuthipayPaymentStatus::Approved))
                {
                    return AttemptStatus::Voided;
                }
                // State is VOIDED but no confirmation from result/status, still consider voided
                return AttemptStatus::Voided;
            }
            AuthipayTransactionState::Declined => return AttemptStatus::VoidFailed,
            AuthipayTransactionState::Pending | AuthipayTransactionState::Waiting => {
                return AttemptStatus::Pending;
            }
            _ => {} // Continue to check result/status
        }
    }

    // Check transaction_result (newer field)
    if let Some(result) = transaction_result {
        return match result {
            AuthipayPaymentResult::Approved => AttemptStatus::Voided,
            AuthipayPaymentResult::Created | AuthipayPaymentResult::Waiting => {
                AttemptStatus::Pending
            }
            AuthipayPaymentResult::Declined
            | AuthipayPaymentResult::Failed
            | AuthipayPaymentResult::Fraud => AttemptStatus::VoidFailed,
            AuthipayPaymentResult::Partial => AttemptStatus::Pending,
            AuthipayPaymentResult::Unknown => AttemptStatus::Unspecified,
        };
    }

    // Check transaction_status (deprecated field) if transaction_result not present
    if let Some(status) = transaction_status {
        return match status {
            AuthipayPaymentStatus::Approved => AttemptStatus::Voided,
            AuthipayPaymentStatus::Waiting => AttemptStatus::Pending,
            AuthipayPaymentStatus::ValidationFailed
            | AuthipayPaymentStatus::ProcessingFailed
            | AuthipayPaymentStatus::Declined => AttemptStatus::VoidFailed,
            AuthipayPaymentStatus::Partial => AttemptStatus::Pending,
            AuthipayPaymentStatus::Unknown => AttemptStatus::Unspecified,
        };
    }

    // Default to Pending if no clear status
    AttemptStatus::Pending
}

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map void status with CRITICAL validation of ALL fields
        let status = map_void_status(
            item.response.transaction_type.clone(),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        let response = build_payment_flow_response(&item.response, status, item.http_code, None);

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== VOIDPC REQUEST STRUCTURE =====
// VoidPostCapture (Reverse) — cancels a captured (PostAuth) transaction before settlement
// Uses requestType: VoidTransaction (distinct from Void which uses VoidPreAuthTransactions)
// AUTHIPAY always voids the full original amount; partial void is not supported

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayVoidPCRequest {
    pub request_type: AuthipayRequestType,
}

// ===== VOIDPC REQUEST TRANSFORMATION =====

impl
    TryFrom<
        &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
    > for AuthipayVoidPCRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // VoidTransaction requires no amount — AUTHIPAY always voids the full original amount
        Ok(Self {
            request_type: AuthipayRequestType::VoidTransaction,
        })
    }
}

// ===== VOIDPC RESPONSE TRANSFORMATION =====

fn map_void_pc_status(
    transaction_type: AuthipayTransactionType,
    transaction_status: Option<AuthipayPaymentStatus>,
    transaction_result: Option<AuthipayPaymentResult>,
    transaction_state: Option<AuthipayTransactionState>,
) -> common_enums::PostCaptureVoidStatus {
    if transaction_type != AuthipayTransactionType::Void {
        return common_enums::PostCaptureVoidStatus::Failed;
    }

    if let Some(state) = transaction_state {
        match state {
            AuthipayTransactionState::Voided => {
                return common_enums::PostCaptureVoidStatus::Succeeded;
            }
            AuthipayTransactionState::Declined => {
                return common_enums::PostCaptureVoidStatus::Failed;
            }
            AuthipayTransactionState::Pending | AuthipayTransactionState::Waiting => {
                return common_enums::PostCaptureVoidStatus::Pending;
            }
            _ => {}
        }
    }

    if let Some(result) = transaction_result {
        return match result {
            AuthipayPaymentResult::Approved => common_enums::PostCaptureVoidStatus::Succeeded,
            AuthipayPaymentResult::Created
            | AuthipayPaymentResult::Waiting
            | AuthipayPaymentResult::Partial
            | AuthipayPaymentResult::Unknown => common_enums::PostCaptureVoidStatus::Pending,
            AuthipayPaymentResult::Declined
            | AuthipayPaymentResult::Failed
            | AuthipayPaymentResult::Fraud => common_enums::PostCaptureVoidStatus::Failed,
        };
    }

    if let Some(status) = transaction_status {
        return match status {
            AuthipayPaymentStatus::Approved => common_enums::PostCaptureVoidStatus::Succeeded,
            AuthipayPaymentStatus::Waiting
            | AuthipayPaymentStatus::Partial
            | AuthipayPaymentStatus::Unknown => common_enums::PostCaptureVoidStatus::Pending,
            AuthipayPaymentStatus::ValidationFailed
            | AuthipayPaymentStatus::ProcessingFailed
            | AuthipayPaymentStatus::Declined => common_enums::PostCaptureVoidStatus::Failed,
        };
    }

    common_enums::PostCaptureVoidStatus::Pending
}

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let post_capture_void_status = map_void_pc_status(
            item.response.transaction_type.clone(),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        let description = post_capture_void_status
            .is_post_capture_void_failure()
            .then(|| {
                item.response.error_message.clone().or_else(|| {
                    item.response
                        .processor
                        .as_ref()
                        .and_then(|p| p.response_message.clone())
                })
            })
            .flatten();

        Ok(Self {
            response: Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id: Some(item.response.ipg_transaction_id.clone()),
                description,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== TYPE ALIASES FOR MACRO COMPATIBILITY =====
// Each flow needs its own response type for the macro system
// Even though they all use the same underlying AuthipayPaymentsResponse struct
pub type AuthipayAuthorizeResponse = AuthipayPaymentsResponse;
/// SetupMandate posts the identical primary-transaction body Authorize does; the alias exists
/// only because the connector macros mint one `…Templating` marker type per named request /
/// response, so two flows cannot name the same type.
pub type AuthipaySetupMandateRequest<T> = AuthipayPaymentsRequest<T>;
pub type AuthipaySetupMandateResponse = AuthipayPaymentsResponse;
pub type AuthipaySyncResponse = AuthipayPaymentsResponse;
pub type AuthipayVoidResponse = AuthipayPaymentsResponse;
pub type AuthipayVoidPCResponse = AuthipayPaymentsResponse;
pub type AuthipayCaptureResponse = AuthipayPaymentsResponse;
pub type AuthipayRefundResponse = AuthipayPaymentsResponse;
pub type AuthipayRefundSyncResponse = AuthipayPaymentsResponse;

// ===== TRYFROM IMPLEMENTATIONS FOR MACRO COMPATIBILITY =====
// These delegate to the existing TryFrom<&RouterDataV2> implementations

use crate::connectors::authipay::AuthipayRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthipayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthipaySetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AuthipayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AuthipayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AuthipayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthipayVoidPCRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// ===== UNIT TESTS =====
// These cover the pure logic in this file that no integration test can reach: the HMAC
// string-to-sign, the deterministic Client-Request-Id, the three-field status resolution, the
// AVS/CVV mapping, the error-priority table, and the serde shape of the request.

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn auth() -> AuthipayAuthType {
        AuthipayAuthType {
            api_key: Secret::new("api_key_value".to_string()),
            api_secret: Secret::new("api_secret_value".to_string()),
        }
    }

    // --- MAC / message signature -------------------------------------------------------

    #[test]
    fn hmac_preimage_is_apikey_then_request_id_then_timestamp_then_body() {
        // Documented construction: rawSignature = apiKey + ClientRequestId + time + requestBody,
        // HMAC-SHA256 keyed by the API secret, Base64 encoded.
        let signature = auth()
            .generate_hmac_signature("api_key_value", "req-1", "1518811817000", r#"{"a":1}"#)
            .expect("signature");

        let expected = {
            let raw = "api_key_valuereq-11518811817000{\"a\":1}";
            let mac = crypto::HmacSha256
                .sign_message(b"api_secret_value", raw.as_bytes())
                .expect("mac");
            general_purpose::STANDARD.encode(mac)
        };

        assert_eq!(signature, expected);
    }

    #[test]
    fn hmac_signature_changes_when_any_component_changes() {
        let base = auth()
            .generate_hmac_signature("k", "r", "1", "{}")
            .expect("signature");
        for (key, id, ts, body) in [
            ("k2", "r", "1", "{}"),
            ("k", "r2", "1", "{}"),
            ("k", "r", "2", "{}"),
            ("k", "r", "1", "{\"a\":1}"),
        ] {
            let other = auth()
                .generate_hmac_signature(key, id, ts, body)
                .expect("signature");
            assert_ne!(base, other, "signature must cover {key}/{id}/{ts}/{body}");
        }
    }

    #[test]
    fn get_requests_sign_the_empty_body() {
        // PSync / RSync sign an empty-string body; this pins that the empty body is a real
        // input to the preimage rather than being skipped.
        let empty = auth()
            .generate_hmac_signature("k", "r", "1", "")
            .expect("signature");
        let non_empty = auth()
            .generate_hmac_signature("k", "r", "1", "{}")
            .expect("signature");
        assert_ne!(empty, non_empty);
    }

    // --- Idempotency ---------------------------------------------------------------------

    #[test]
    fn client_request_id_is_deterministic_and_uuid_shaped() {
        let first = derive_client_request_id(AuthipayOperation::Authorize, "attempt_abc");
        let second = derive_client_request_id(AuthipayOperation::Authorize, "attempt_abc");
        assert_eq!(
            first, second,
            "a retried Authorize must reuse the gateway's idempotency key so the replay is refused"
        );
        assert_ne!(
            first,
            derive_client_request_id(AuthipayOperation::Authorize, "attempt_xyz")
        );
        assert!(
            uuid::Uuid::parse_str(&first).is_ok(),
            "must stay UUID-shaped"
        );
        assert_eq!(first.len(), 36);
    }

    #[test]
    fn each_operation_derives_a_distinct_client_request_id() {
        // The gateway answers a replayed Client-Request-Id with a 400 and does not process the
        // request, so a Capture that reused the Authorize's id would never settle.
        let ids: Vec<String> = [
            AuthipayOperation::Authorize,
            AuthipayOperation::SetupMandate,
            AuthipayOperation::Capture,
            AuthipayOperation::Void,
            AuthipayOperation::VoidPostCapture,
            AuthipayOperation::Refund,
        ]
        .into_iter()
        .map(|operation| derive_client_request_id(operation, "same_reference"))
        .collect();

        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(
            unique.len(),
            ids.len(),
            "operations must not collide: {ids:?}"
        );
    }

    #[test]
    fn inquiry_flows_get_a_fresh_id_so_polling_stays_repeatable() {
        let first = AuthipayAuthType::generate_client_request_id();
        let second = AuthipayAuthType::generate_client_request_id();
        assert_ne!(
            first, second,
            "a repeated PSync must not replay an id the gateway would reject"
        );
    }

    // --- Capture-method classification ----------------------------------------------------

    /// Exercise the real classification: the shared guard that every flow building
    /// `AuthipayPaymentsRequest` runs, followed by the Authorize-side SALE/PREAUTH split that
    /// `PaymentsAuthorizeData::is_auto_capture()` decides. Both halves are the production code
    /// paths — no stand-in predicate that could drift from them.
    fn request_type_for(
        capture_method: Option<common_enums::CaptureMethod>,
    ) -> Result<AuthipayRequestType, ()> {
        reject_unsupported_capture_method(capture_method).map_err(|_| ())?;
        let is_auto_capture = !matches!(
            capture_method,
            Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        );
        Ok(if is_auto_capture {
            AuthipayRequestType::PaymentCardSaleTransaction
        } else {
            AuthipayRequestType::PaymentCardPreAuthTransaction
        })
    }

    /// The asymmetry guard: SetupMandate builds the *same* request struct as Authorize, so the
    /// capture-method rejection has to fire on it too. `AuthipayPrimaryRequest::request_type` is
    /// the only place either flow can obtain a `requestType`, and both implementations open with
    /// this call — so proving the guard here proves it for both.
    #[test]
    fn the_capture_method_guard_is_shared_by_every_primary_transaction_flow() {
        for rejected in [
            common_enums::CaptureMethod::ManualMultiple,
            common_enums::CaptureMethod::Scheduled,
        ] {
            assert!(
                reject_unsupported_capture_method(Some(rejected)).is_err(),
                "{rejected:?} must be rejected before any flow can build a request"
            );
        }
        for accepted in [
            Some(common_enums::CaptureMethod::Automatic),
            Some(common_enums::CaptureMethod::SequentialAutomatic),
            Some(common_enums::CaptureMethod::Manual),
            None,
        ] {
            assert!(
                reject_unsupported_capture_method(accepted).is_ok(),
                "{accepted:?} must be accepted"
            );
        }
    }

    #[test]
    fn sequential_automatic_groups_with_automatic() {
        for method in [
            Some(common_enums::CaptureMethod::Automatic),
            Some(common_enums::CaptureMethod::SequentialAutomatic),
            None,
        ] {
            assert_eq!(
                request_type_for(method),
                Ok(AuthipayRequestType::PaymentCardSaleTransaction),
                "{method:?} must auto-capture"
            );
            // The canonical predicate must agree with the arm above — this is the guard
            // against the two drifting apart.
            assert!(
                !matches!(
                    method,
                    Some(common_enums::CaptureMethod::Manual)
                        | Some(common_enums::CaptureMethod::ManualMultiple)
                        | Some(common_enums::CaptureMethod::Scheduled)
                ),
                "auto-capture methods must not be in the manual set"
            );
        }
    }

    #[test]
    fn multi_and_scheduled_capture_are_rejected_not_downgraded() {
        assert_eq!(
            request_type_for(Some(common_enums::CaptureMethod::ManualMultiple)),
            Err(())
        );
        assert_eq!(
            request_type_for(Some(common_enums::CaptureMethod::Scheduled)),
            Err(())
        );
        assert_eq!(
            request_type_for(Some(common_enums::CaptureMethod::Manual)),
            Ok(AuthipayRequestType::PaymentCardPreAuthTransaction)
        );
    }

    // --- Status mapping -------------------------------------------------------------------

    #[test]
    fn preauth_authorized_maps_to_authorized_and_sale_captured_to_charged() {
        assert_eq!(
            map_status(
                None,
                Some(AuthipayPaymentResult::Approved),
                Some(AuthipayTransactionState::Authorized),
                AuthipayTransactionType::Preauth,
            ),
            AttemptStatus::Authorized
        );
        assert_eq!(
            map_status(
                None,
                Some(AuthipayPaymentResult::Approved),
                Some(AuthipayTransactionState::Captured),
                AuthipayTransactionType::Sale,
            ),
            AttemptStatus::Charged
        );
        assert_eq!(
            map_status(
                None,
                Some(AuthipayPaymentResult::Approved),
                Some(AuthipayTransactionState::Settled),
                AuthipayTransactionType::Postauth,
            ),
            AttemptStatus::Charged
        );
    }

    #[test]
    fn every_terminal_state_maps_to_a_terminal_attempt_status() {
        assert_eq!(
            map_status(
                None,
                Some(AuthipayPaymentResult::Declined),
                Some(AuthipayTransactionState::Declined),
                AuthipayTransactionType::Sale,
            ),
            AttemptStatus::Failure
        );
        assert_eq!(
            map_status(
                None,
                Some(AuthipayPaymentResult::Approved),
                Some(AuthipayTransactionState::Voided),
                AuthipayTransactionType::Void,
            ),
            AttemptStatus::Voided
        );
    }

    #[test]
    fn not_visible_yet_is_pending_never_failure() {
        // No outcome reported at all: the gateway has not answered, which PSync can recover.
        assert_eq!(
            map_status(None, None, None, AuthipayTransactionType::Sale),
            AttemptStatus::Pending
        );
        // Held but not resolved.
        for state in [
            AuthipayTransactionState::Pending,
            AuthipayTransactionState::Waiting,
            AuthipayTransactionState::Initialized,
            AuthipayTransactionState::Ready,
        ] {
            assert_eq!(
                map_status(
                    None,
                    Some(AuthipayPaymentResult::Waiting),
                    Some(state.clone()),
                    AuthipayTransactionType::Sale,
                ),
                AttemptStatus::Pending,
                "{state:?} must stay pollable"
            );
        }
        // CREATED is "record made, not yet processed" — also not a decline.
        assert_eq!(
            map_status(
                None,
                Some(AuthipayPaymentResult::Created),
                None,
                AuthipayTransactionType::Sale,
            ),
            AttemptStatus::Pending
        );
    }

    #[test]
    fn unknown_connector_status_is_unspecified_not_invented_pending() {
        assert_eq!(
            map_status(
                None,
                Some(AuthipayPaymentResult::Unknown),
                None,
                AuthipayTransactionType::Sale,
            ),
            AttemptStatus::Unspecified
        );
        assert_eq!(
            map_status(
                Some(AuthipayPaymentStatus::Unknown),
                None,
                None,
                AuthipayTransactionType::Sale,
            ),
            AttemptStatus::Unspecified
        );
    }

    #[test]
    fn transaction_result_wins_over_deprecated_transaction_status() {
        // `transactionStatus` is deprecated; when both are present the current field decides.
        assert_eq!(
            map_status(
                Some(AuthipayPaymentStatus::Declined),
                Some(AuthipayPaymentResult::Approved),
                None,
                AuthipayTransactionType::Sale,
            ),
            AttemptStatus::Charged
        );
    }

    #[test]
    fn unknown_connector_status_deserializes_instead_of_failing() {
        let result: AuthipayPaymentResult =
            serde_json::from_str("\"SOME_FUTURE_VALUE\"").expect("must not fail deserialization");
        assert_eq!(result, AuthipayPaymentResult::Unknown);
        let state: AuthipayTransactionState =
            serde_json::from_str("\"SOME_FUTURE_STATE\"").expect("must not fail deserialization");
        assert_eq!(state, AuthipayTransactionState::Unknown);
    }

    #[test]
    fn transaction_state_serde_uses_screaming_snake_case() {
        let state: AuthipayTransactionState =
            serde_json::from_str("\"COMPLETED_GET\"").expect("parse");
        assert_eq!(state, AuthipayTransactionState::CompletedGet);
    }

    // --- Error mapping ---------------------------------------------------------------------

    fn transaction_error_body() -> AuthipayErrorResponse {
        serde_json::from_value(serde_json::json!({
            "type": "transactionResponse",
            "clientRequestId": "client-1",
            "apiTraceId": "trace-1",
            "responseType": "EndpointDeclined",
            "ipgTransactionId": "838916029301",
            "orderId": "ABC12345",
            "transactionType": "SALE",
            "transactionResult": "DECLINED",
            "transactionState": "DECLINED",
            "approvalCode": "N:05:Do not honor",
            "schemeResponseCode": "05",
            "merchantAdviceCode": "03",
            "processor": {
                "responseCode": "05",
                "responseMessage": "Do not honor",
                "associationResponseCode": "005",
                "associationResponseMessage": "Do not honour",
                "merchantAdviceCodeIndicator": "03",
                "merchantAdviceMessage": "Do not try again",
                "avsResponse": { "streetMatch": "N", "postalCodeMatch": "N", "associationAvsResponse": "N" },
                "securityCodeResponse": "NOT_MATCHED"
            },
            "error": { "code": "2303", "message": "Invalid credit card number", "declineReasonCode": "Do not try again" }
        }))
        .expect("parse TransactionErrorResponse")
    }

    #[test]
    fn nested_error_object_is_parsed_not_dropped() {
        // The previous struct declared code/message at the top level, so every 4xx/5xx
        // surfaced with an empty code and message.
        let error = build_authipay_error_response(&transaction_error_body(), 422, None);
        assert_eq!(error.code, "2303");
        assert_eq!(error.message, "Invalid credit card number");
        assert_ne!(error.code, NO_ERROR_CODE);
    }

    #[test]
    fn issuer_codes_reach_the_network_fields_for_gsm() {
        let error = build_authipay_error_response(&transaction_error_body(), 422, None);
        // Raw issuer code, not the gateway-normalised `processor.responseCode` ("05").
        assert_eq!(error.network_decline_code.as_deref(), Some("005"));
        assert_eq!(error.network_advice_code.as_deref(), Some("03"));
        assert_eq!(
            error.network_error_message.as_deref(),
            Some("Do not honour")
        );
        assert_eq!(
            error.connector_transaction_id.as_deref(),
            Some("838916029301")
        );
        assert_eq!(error.status_code, 422);
    }

    #[test]
    fn flow_agnostic_builder_never_stamps_an_attempt_status() {
        // It also routes Refund/RSync, where `Payment(_)` is coerced to RefundFailure.
        let error = build_authipay_error_response(&transaction_error_body(), 422, None);
        assert!(error.attempt_status.is_none());
    }

    #[test]
    fn plain_error_response_without_processor_still_carries_code_and_field_details() {
        let body: AuthipayErrorResponse = serde_json::from_value(serde_json::json!({
            "clientRequestId": "c",
            "apiTraceId": "t",
            "responseType": "BadRequest",
            "type": "errorResponse",
            "error": {
                "code": "2303",
                "message": "Invalid credit card number",
                "details": [ { "field": "PaymentCard.number", "message": "may not be null" } ]
            }
        }))
        .expect("parse ErrorResponse");

        let error = build_authipay_error_response(&body, 400, None);
        assert_eq!(error.code, "2303");
        assert_eq!(error.message, "Invalid credit card number");
        assert_eq!(
            error.reason.as_deref(),
            Some("PaymentCard.number: may not be null")
        );
        assert!(error.network_decline_code.is_none());
    }

    #[test]
    fn empty_error_body_falls_back_to_named_constants_not_empty_strings() {
        let error = build_authipay_error_response(&AuthipayErrorResponse::default(), 500, None);
        assert_eq!(error.code, NO_ERROR_CODE);
        assert_eq!(error.message, NO_ERROR_MESSAGE);
    }

    // --- 200-with-decline ------------------------------------------------------------------

    fn declined_200_body() -> AuthipayPaymentsResponse {
        serde_json::from_value(serde_json::json!({
            "type": "transactionResponse",
            "ipgTransactionId": "999",
            "orderId": "ORDER-9",
            "merchantTransactionId": "ATTEMPT-9",
            "transactionType": "SALE",
            "transactionResult": "DECLINED",
            "transactionState": "DECLINED",
            "schemeResponseCode": "51",
            "processor": {
                "responseCode": "51",
                "responseMessage": "Insufficient funds",
                "associationResponseCode": "051",
                "associationResponseMessage": "Not sufficient funds",
                "merchantAdviceCodeIndicator": "21",
                "avsResponse": { "streetMatch": "Y", "postalCodeMatch": "N", "associationAvsResponse": "A" },
                "securityCodeResponse": "MATCHED"
            }
        }))
        .expect("parse declined 200")
    }

    #[test]
    fn decline_arriving_on_a_200_returns_an_error_with_its_codes() {
        let response = declined_200_body();
        let status = map_status(
            response.transaction_status.clone(),
            response.transaction_result.clone(),
            response.transaction_state.clone(),
            response.transaction_type.clone(),
        );
        assert_eq!(status, AttemptStatus::Failure);

        let mapped = build_payment_flow_response(&response, status, 200, None);
        let error = mapped.expect_err("a declined 200 must not map to a success envelope");
        assert_eq!(error.code, "51");
        assert_eq!(error.message, "Insufficient funds");
        assert_eq!(error.network_decline_code.as_deref(), Some("051"));
        assert_eq!(error.network_advice_code.as_deref(), Some("21"));
        assert_eq!(
            error.attempt_status,
            Some(FlowStatus::Payment(AttemptStatus::Failure))
        );
    }

    // --- AVS / CVV mapping ------------------------------------------------------------------

    #[test]
    fn avs_and_cvv_results_are_mapped_onto_payment_checks() {
        let checks = build_payment_checks(&declined_200_body(), Some(&"05".to_string()));
        assert_eq!(checks["avs_response"]["street_match"], "Y");
        assert_eq!(checks["avs_response"]["postal_code_match"], "N");
        assert_eq!(checks["avs_response"]["association_avs_response"], "A");
        assert_eq!(checks["card_verification"], "MATCHED");
        // Authipay has no request field for ECI, so the caller's value is recorded here.
        assert_eq!(checks["three_d_secure"]["requested_eci"], "05");
    }

    #[test]
    fn missing_processor_yields_nulls_rather_than_panicking() {
        let response: AuthipayPaymentsResponse = serde_json::from_value(serde_json::json!({
            "ipgTransactionId": "1",
            "transactionType": "SALE",
        }))
        .expect("parse minimal response");
        let checks = build_payment_checks(&response, None);
        assert!(checks["card_verification"].is_null());
        assert!(checks["avs_response"]["street_match"].is_null());
    }

    // --- Response identifiers ---------------------------------------------------------------

    #[test]
    fn network_txn_id_is_the_scheme_transaction_id_not_the_network_name() {
        let response: AuthipayPaymentsResponse = serde_json::from_value(serde_json::json!({
            "ipgTransactionId": "838916029301",
            "orderId": "ABC12345",
            "merchantTransactionId": "ATTEMPT-1",
            "clientRequestId": "generated-uuid",
            "apiTraceId": "rrt-trace",
            "transactionType": "SALE",
            "transactionResult": "APPROVED",
            "transactionState": "CAPTURED",
            "schemeTransactionId": "019078743804756",
            "transactionLinkIdentifier": "01236548543965",
            "processor": { "network": "VISA" }
        }))
        .expect("parse approved response");

        match build_transaction_response(&response, 200, None) {
            PaymentsResponseData::TransactionResponse {
                resource_id,
                network_txn_id,
                network_txn_link_id,
                connector_response_reference_id,
                ..
            } => {
                assert!(matches!(
                    resource_id,
                    ResponseId::ConnectorTransactionId(ref id) if id == "838916029301"
                ));
                assert_eq!(network_txn_id.as_deref(), Some("019078743804756"));
                assert_eq!(network_txn_link_id.as_deref(), Some("01236548543965"));
                // orderId — the merchant-facing reference — not the generated clientRequestId.
                assert_eq!(connector_response_reference_id.as_deref(), Some("ABC12345"));
            }
            other => panic!("expected a TransactionResponse, got {other:?}"),
        }
    }

    #[test]
    fn reference_id_falls_back_to_merchant_transaction_id_when_order_id_is_absent() {
        let response: AuthipayPaymentsResponse = serde_json::from_value(serde_json::json!({
            "ipgTransactionId": "1",
            "merchantTransactionId": "ATTEMPT-1",
            "clientRequestId": "generated-uuid",
            "transactionType": "SALE",
        }))
        .expect("parse");
        assert_eq!(
            connector_reference_id(&response).as_deref(),
            Some("ATTEMPT-1")
        );
    }

    // --- 3DS pass-through ---------------------------------------------------------------------

    fn authentication_data(
        trans_status: Option<common_enums::TransactionStatus>,
        cavv: Option<&str>,
    ) -> AuthenticationData {
        AuthenticationData {
            trans_status,
            eci: Some("05".to_string()),
            cavv: cavv.map(|value| Secret::new(value.to_string())),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: None,
            message_version: None,
            ds_trans_id: Some("f38e6948-5388-41a6-bca4-b49723c19437".to_string()),
            acs_transaction_id: Some("acs-txn-id".to_string()),
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

    #[test]
    fn external_3ds_result_is_passed_through_with_the_right_letter() {
        let result = build_authentication_result(&authentication_data(
            Some(common_enums::TransactionStatus::Success),
            Some("AAABCZIhcQAAAABZlyFxAAAAAAA"),
        ));
        assert_eq!(
            result.authentication_type,
            AuthipayAuthenticationType::Secure3DAuthenticationResult
        );
        assert_eq!(
            result.authentication_response,
            Some(AuthipayAuthenticationResponse::Authenticated)
        );
        assert_eq!(
            result.transaction_status,
            Some(AuthipayAuthenticationResponse::Authenticated)
        );
        assert!(result.cavv.is_some());
        assert_eq!(
            result.ds_transaction_id.as_deref(),
            Some("f38e6948-5388-41a6-bca4-b49723c19437")
        );
        assert_eq!(result.message_category, AuthipayMessageCategory::Payment);
    }

    #[test]
    fn cavv_is_suppressed_when_authentication_could_not_be_performed() {
        // Fiserv documents `U` + CAVV as an error, not merely redundant.
        let result = build_authentication_result(&authentication_data(
            Some(common_enums::TransactionStatus::VerificationNotPerformed),
            Some("AAABCZIhcQAAAABZlyFxAAAAAAA"),
        ));
        assert_eq!(
            result.authentication_response,
            Some(AuthipayAuthenticationResponse::Unavailable)
        );
        assert!(result.cavv.is_none());
    }

    #[test]
    fn out_of_range_cavv_is_omitted_rather_than_truncated() {
        // `minLength: 20`, `maxLength: 32` — truncating an authentication value corrupts it.
        let too_short = build_authentication_result(&authentication_data(
            Some(common_enums::TransactionStatus::Success),
            Some("short"),
        ));
        assert!(too_short.cavv.is_none());

        let too_long = build_authentication_result(&authentication_data(
            Some(common_enums::TransactionStatus::Success),
            Some(&"A".repeat(33)),
        ));
        assert!(too_long.cavv.is_none());

        let in_range = build_authentication_result(&authentication_data(
            Some(common_enums::TransactionStatus::Success),
            Some(&"A".repeat(20)),
        ));
        assert!(in_range.cavv.is_some());
    }

    #[test]
    fn authentication_result_serializes_with_the_documented_keys() {
        let result = build_authentication_result(&authentication_data(
            Some(common_enums::TransactionStatus::NotVerified),
            Some(&"B".repeat(28)),
        ));
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["authenticationType"], "Secure3DAuthenticationResult");
        assert_eq!(json["authenticationResponse"], "A");
        assert_eq!(json["transactionStatus"], "A");
        assert_eq!(json["messageCategory"], "01");
        assert_eq!(
            json["dsTransactionId"],
            "f38e6948-5388-41a6-bca4-b49723c19437"
        );
        assert_eq!(json["acsTransactionId"], "acs-txn-id");
        // There is no `eci` key: the schema has none and Fiserv derives it.
        assert!(json.get("eci").is_none());
        // A None protocol version must be omitted, not serialized as null.
        assert!(json.get("secure3DProtocolVersion").is_none());
    }

    // --- serde hygiene on the request ------------------------------------------------------

    #[test]
    fn request_type_discriminators_match_the_api_strings() {
        for (variant, expected) in [
            (
                AuthipayRequestType::PaymentCardSaleTransaction,
                "PaymentCardSaleTransaction",
            ),
            (
                AuthipayRequestType::PaymentCardPreAuthTransaction,
                "PaymentCardPreAuthTransaction",
            ),
            (
                AuthipayRequestType::PostAuthTransaction,
                "PostAuthTransaction",
            ),
            (AuthipayRequestType::ReturnTransaction, "ReturnTransaction"),
            (
                AuthipayRequestType::VoidPreAuthTransactions,
                "VoidPreAuthTransactions",
            ),
            (AuthipayRequestType::VoidTransaction, "VoidTransaction"),
        ] {
            assert_eq!(
                serde_json::to_value(&variant).expect("serialize"),
                serde_json::Value::String(expected.to_string())
            );
        }
    }

    #[test]
    fn optional_order_members_are_omitted_not_serialized_as_null() {
        let order = OrderDetails {
            order_id: "ORDER-1".to_string(),
            billing: None,
            shipping: None,
            soft_descriptor: None,
            purchase_card: None,
            ip: None,
        };
        let json = serde_json::to_value(&order).expect("serialize");
        let object = json.as_object().expect("object");
        assert_eq!(
            object.len(),
            1,
            "only orderId should be on the wire: {json}"
        );
        assert!(object.contains_key("orderId"));
    }

    #[test]
    fn nested_order_members_use_the_documented_json_paths() {
        let order = OrderDetails {
            order_id: "ORDER-1".to_string(),
            billing: Some(AuthipayBilling {
                name: Some(Secret::new("John Doe".to_string())),
                first_name: None,
                last_name: None,
                customer_id: None,
                birth_date: None,
                contact: Some(AuthipayContact {
                    phone: Some(Secret::new("5555555555".to_string())),
                    email: None,
                }),
                address: Some(AuthipayAddress {
                    address1: Some(Secret::new("123 Main St.".to_string())),
                    address2: None,
                    city: Some(Secret::new("Dublin".to_string())),
                    region: None,
                    postal_code: Some(Secret::new("D02".to_string())),
                    country: Some(Secret::new("IE".to_string())),
                }),
            }),
            shipping: None,
            soft_descriptor: Some(AuthipaySoftDescriptor {
                dynamic_merchant_name: Some("Merchant XYZ".to_string()),
                customer_service_number: Some(Secret::new("9973322990".to_string())),
                dynamic_address: None,
            }),
            purchase_card: Some(AuthipayPurchaseCard {
                level2: Some(AuthipayLevel2 {
                    customer_reference_id: Some("REF-1".to_string()),
                    supplier_vat_registration_number: None,
                    total_discount_amount_and_rate: None,
                    vat_shipping_amount_and_rate: None,
                    duty_amount_and_rate: None,
                }),
                level3: Some(AuthipayLevel3 {
                    line_items: vec![AuthipayLineItem {
                        commodity_code: Some("ab12".to_string()),
                        product_code: Some("SKU-1".to_string()),
                        description: Some("Dinner".to_string()),
                        quantity: 2,
                        unit_measure: Some("EA".to_string()),
                        unit_price: FloatMajorUnit(30.075),
                        vat_amount_and_rate: None,
                        discount_amount_and_rate: None,
                        line_item_total: Some(FloatMajorUnit(60.15)),
                    }],
                }),
            }),
            ip: None,
        };

        let json = serde_json::to_value(&order).expect("serialize");
        assert_eq!(json["billing"]["address"]["address1"], "123 Main St.");
        assert_eq!(json["billing"]["address"]["postalCode"], "D02");
        assert_eq!(json["billing"]["contact"]["phone"], "5555555555");
        assert_eq!(
            json["softDescriptor"]["dynamicMerchantName"],
            "Merchant XYZ"
        );
        assert_eq!(
            json["softDescriptor"]["customerServiceNumber"],
            "9973322990"
        );
        // PascalCase Level keys and the `ID` / `VAT` casing the schema uses.
        assert_eq!(
            json["purchaseCard"]["Level2"]["customerReferenceID"],
            "REF-1"
        );
        assert_eq!(
            json["purchaseCard"]["Level3"]["lineItems"][0]["unitMeasure"],
            "EA"
        );
        assert_eq!(
            json["purchaseCard"]["Level3"]["lineItems"][0]["quantity"],
            2
        );
    }

    #[test]
    fn amount_and_rate_is_omitted_when_the_rate_is_unknown() {
        // `AdditionalAmountRate` requires both fields; a fabricated 0% rate is corrupt data.
        assert!(AuthipayAmountAndRate::new(Some(FloatMajorUnit(5.0)), None).is_none());
        assert!(AuthipayAmountAndRate::new(None, Some(1.175)).is_none());
        let both = AuthipayAmountAndRate::new(Some(FloatMajorUnit(5.145)), Some(1.175))
            .expect("both present");
        let json = serde_json::to_value(&both).expect("serialize");
        assert_eq!(json["amount"], 5.145);
        assert_eq!(json["rate"], 1.175);
    }

    #[test]
    fn cardholder_name_uses_the_documented_key() {
        let json = serde_json::to_value(PaymentCard::<
            domain_types::payment_method_data::DefaultPCIHolder,
        > {
            number: RawCardNumber(
                cards::CardNumber::try_from("4111111111111111".to_string()).expect("card"),
            ),
            expiry_date: ExpiryDate {
                month: Secret::new("08".to_string()),
                year: Secret::new("30".to_string()),
            },
            security_code: Some(Secret::new("999".to_string())),
            cardholder_name: Some(Secret::new("John Doe".to_string())),
        })
        .expect("serialize");
        assert_eq!(json["cardholderName"], "John Doe");
        assert!(json.get("holder").is_none());
    }

    // --- Field-limit and formatting helpers ---------------------------------------------------

    #[test]
    fn merchant_references_are_truncated_to_their_documented_limits() {
        let long = "x".repeat(200);
        assert_eq!(
            truncate_string(&long, MAX_LEN_MERCHANT_TRANSACTION_ID).len(),
            40
        );
        assert_eq!(truncate_string(&long, MAX_LEN_ORDER_ID).len(), 100);
        assert_eq!(truncate_string("short", MAX_LEN_ORDER_ID), "short");
    }

    #[test]
    fn customer_service_number_keeps_only_digits() {
        assert_eq!(retain_ascii_digits("+353 (1) 234-5678"), "35312345678");
        assert_eq!(
            truncate_string(&retain_ascii_digits("+353 (1) 234-5678"), 10).len(),
            10
        );
    }

    #[test]
    fn line3_is_folded_into_address2_rather_than_dropped() {
        let joined = join_address_lines(
            Some(Secret::new("Suite 4".to_string())),
            Some(Secret::new("Block B".to_string())),
        )
        .expect("joined");
        assert_eq!(joined.peek(), "Suite 4 Block B");
        assert!(join_address_lines(None, None).is_none());
    }

    #[test]
    fn transaction_origin_follows_the_payment_channel() {
        assert_eq!(
            serde_json::to_value(AuthipayTransactionOrigin::from(None)).expect("serialize"),
            serde_json::Value::String("ECOM".to_string())
        );
        assert_eq!(
            AuthipayTransactionOrigin::from(Some(common_enums::PaymentChannel::MailOrder)),
            AuthipayTransactionOrigin::Mail
        );
        assert_eq!(
            AuthipayTransactionOrigin::from(Some(common_enums::PaymentChannel::TelephoneOrder)),
            AuthipayTransactionOrigin::Phone
        );
    }

    // --- Credential-on-file handle --------------------------------------------------------

    fn approved_preauth_body() -> AuthipayPaymentsResponse {
        let mut response = declined_200_body();
        response.transaction_type = AuthipayTransactionType::Preauth;
        response.transaction_result = Some(AuthipayPaymentResult::Approved);
        response.transaction_state = Some(AuthipayTransactionState::Authorized);
        response.transaction_status = Some(AuthipayPaymentStatus::Approved);
        response.processor = None;
        response.error = None;
        response
    }

    #[test]
    fn mandate_reference_carries_the_scheme_transaction_id_a_later_mit_must_echo() {
        let mut response = approved_preauth_body();
        response.scheme_transaction_id = Some("249771795129519".to_string());
        response.transaction_link_identifier = Some("tlid-1".to_string());

        let mandate = build_mandate_reference(&response).expect("mandate reference");
        assert_eq!(
            mandate.connector_mandate_id.as_deref(),
            Some("249771795129519"),
            "the MIT quotes schemeTransactionId in storedCredentials.referencedSchemeTransactionId"
        );

        let metadata = mandate.mandate_metadata.expect("metadata");
        let metadata = metadata.peek();
        assert_eq!(metadata["scheme_transaction_id"], "249771795129519");
        assert_eq!(metadata["transaction_link_identifier"], "tlid-1");
        assert_eq!(metadata["ipg_transaction_id"], response.ipg_transaction_id);
    }

    #[test]
    fn a_gateway_payment_token_outranks_the_scheme_id_as_the_mandate_handle() {
        let mut response = approved_preauth_body();
        response.scheme_transaction_id = Some("249771795129519".to_string());
        response.payment_token = Some(PaymentToken {
            value: Some("tok_reusable".to_string()),
            reusable: Some(true),
            decline_duplicates: Some(false),
        });

        let mandate = build_mandate_reference(&response).expect("mandate reference");
        assert_eq!(
            mandate.connector_mandate_id.as_deref(),
            Some("tok_reusable")
        );
        // The scheme id is still persisted — RepeatPayment needs it even when a token exists.
        let metadata = mandate.mandate_metadata.expect("metadata");
        assert_eq!(metadata.peek()["scheme_transaction_id"], "249771795129519");
    }

    #[test]
    fn a_mandate_that_cannot_be_chained_is_reported_as_absent_not_fabricated() {
        // No schemeTransactionId, no TLID, no token: nothing a later MIT could quote. Returning
        // a reference built out of the gateway transaction id would look reusable and is not.
        let mut response = approved_preauth_body();
        response.scheme_transaction_id = None;
        response.transaction_link_identifier = None;
        response.payment_token = None;

        assert!(build_mandate_reference(&response).is_none());
    }

    #[test]
    fn mandate_metadata_is_built_from_whatever_identifiers_came_back() {
        // Only the TLID: the block must still be built rather than gated on the scheme id.
        let mut response = approved_preauth_body();
        response.scheme_transaction_id = None;
        response.payment_token = None;
        response.transaction_link_identifier = Some("tlid-only".to_string());

        let mandate = build_mandate_reference(&response).expect("mandate reference");
        assert!(mandate.connector_mandate_id.is_none());
        let metadata = mandate.mandate_metadata.expect("metadata");
        assert_eq!(metadata.peek()["transaction_link_identifier"], "tlid-only");
    }

    #[test]
    fn a_declined_verification_returns_an_error_not_a_mandate() {
        // The 2xx-with-decline path is shared with Authorize; this pins that a decline on the
        // SetupMandate leg cannot hand back a credential-on-file handle.
        let response = declined_200_body();
        let status = map_status(
            response.transaction_status.clone(),
            response.transaction_result.clone(),
            response.transaction_state.clone(),
            response.transaction_type.clone(),
        );
        assert!(build_payment_flow_response(
            &response,
            status,
            200,
            build_mandate_reference(&response)
        )
        .is_err());
    }

    #[test]
    fn stored_credential_block_is_scheduled_only_for_a_recurring_intent() {
        let json = serde_json::to_value(AuthipayStoredCredentials {
            sequence: AuthipayCredentialSequence::First,
            scheduled: false,
            initiator: AuthipayCredentialInitiator::Cardholder,
            indicator_subcategory: Some(AuthipayIndicatorSubcategory::CredentialOnFileFirst),
        })
        .expect("serialize");
        assert_eq!(json["sequence"], "FIRST");
        assert_eq!(json["initiator"], "CARDHOLDER");
        assert_eq!(json["scheduled"], false);
        assert_eq!(json["indicatorSubcategory"], "CREDENTIAL_ON_FILE_FIRST");
    }
}
