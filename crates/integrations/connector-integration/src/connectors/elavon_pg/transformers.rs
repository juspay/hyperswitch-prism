//! Elavon Payment Gateway (EPG) — request/response transformers.
//!
//! EPG is Elavon's JSON REST gateway (`api.sandbox.elavonpayments.com`). It is a
//! completely different product from the `elavon` connector in this repo, which
//! integrates Elavon Converge (an XML/form-encoded API). No shape is shared.
//!
//! Everything is a `Transaction`: `POST /transactions` creates one and the `type`
//! discriminator selects `sale`, `refund` or `void`. Capture is expressed either as
//! an update on the sale (`POST /transactions/{id}` with `{"doCapture": true}`) or
//! as a `PartialCapture` resource (`POST /partial-captures`).

use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{consts, pii::Email, request::Method, types::StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateOrder, PSync, PreAuthenticate, RSync, Refund, Void,
    },
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::elavon_pg::ElavonPgRouterData, types::ResponseRouterData};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Value of the `Accept-Version` header. EPG assumes "current" when the header is
/// absent and answers with a `Warning` header; pinning the major protects us from
/// a future breaking release (spec §2.4).
pub const ELAVON_PG_ACCEPT_VERSION: &str = "1";

/// `Content-Type`/`Accept` media type. EPG answers `415 unsupportedMediaType` when
/// the request `Content-Type` does not specify `application/json` (spec §2.4).
pub const ELAVON_PG_MEDIA_TYPE: &str = "application/json";

/// Connector id, used in error contexts and `ConnectorCommon::id`.
pub const ELAVON_PG_CONNECTOR_ID: &str = "elavon_pg";

/// EPG's `ThreeDSecure.authenticationValue` is a 20-byte base64 CAVV, i.e. exactly
/// 28 characters. Any other length is rejected with `fieldValidationFailure`, so a
/// differently-sized value is dropped (and warned about) rather than sent.
const ELAVON_PG_CAVV_LENGTH: usize = 28;

/// Separator used when folding EPG's `failures[]` array into `ErrorResponse::reason`.
const ELAVON_PG_FAILURE_JOIN: &str = "; ";

/// EPG's `ThreeDSecure.transStatusReason` must be exactly two numeric digits
/// (pattern `^[0-9]{2}$`); anything else is a `fieldValidationFailure`.
const ELAVON_PG_TRANS_STATUS_REASON_LENGTH: usize = 2;

/// `PaymentSessionInputOptions.doThreeDSecure` — *"Determines whether or not the
/// HPP will perform 3-D secure validation"*. Always `true`: authenticating the
/// shopper is the entire reason this connector opens a hosted payment session
/// (spec §5.2).
const ELAVON_PG_HPP_DO_THREE_D_SECURE: bool = true;

/// `PaymentSessionInputOptions.doCreateTransaction` — when `false`, EPG turns what
/// the shopper typed into the HPP into a single-use **HostedCard** instead of
/// creating the `Transaction` itself, leaving the settle to
/// `POST /transactions {"paymentSession": …}`. That is the completion call this
/// connector's Authorize makes, so the switch is deliberately `false`.
///
/// The published documentation example writes this as the *string* `"true"`, but
/// the OpenAPI `PaymentSessionInputOptions` schema types it `"type": "boolean"`
/// with `"default": false`. The schema is authoritative: it goes on the wire as a
/// JSON boolean.
const ELAVON_PG_HPP_DO_CREATE_TRANSACTION: bool = false;

/// Path segment of an EPG payment-session resource URL
/// (`https://api…/payment-sessions/{id}`).
///
/// The hosted-payment-page href is threaded from PreAuthenticate to the settle
/// Authorize inside `AuthenticationData.threeds_server_transaction_id` — the only
/// field Hyperswitch carries across the shopper redirect. That same field legitimately
/// holds a real 3DS-server transaction id on the external/pass-through 3DS path, so
/// the two are told apart by shape: only an absolute URL naming this collection is
/// treated as a payment session.
const ELAVON_PG_PAYMENT_SESSIONS_SEGMENT: &str = "/payment-sessions/";

/// URL schemes EPG resource URLs use (`href` is `"format": "url"`, and the
/// `returnUrl`/`cancelUrl` pattern is `https?://[^/]{2,}.*`).
const ELAVON_PG_URL_SCHEMES: [&str; 2] = ["https://", "http://"];

/// EPG resource names, used only to make the "href missing, using id" warning say
/// which resource it is talking about.
const ELAVON_PG_ORDER_RESOURCE: &str = "Order";

/// Diagnostic for the one unverifiable assumption in the hosted-payment-page flow:
/// that `POST /payment-sessions` really does return the shopper `url`.
const ELAVON_PG_MISSING_HOSTED_PAGE_URL: &str =
    "elavon_pg: PreAuthenticate created the payment session but the 201 carried no `url`. \
     `url` (\"URL that shoppers will use\") is the hosted payment page the shopper must be \
     redirected to; it is documented on the PaymentSession schema but omitted from Elavon's \
     published example and has not been verified against a live sandbox. Nothing can be \
     substituted for it, so the attempt fails here rather than redirecting the shopper to a \
     guessed URL.";

/// The payment-session reference must be an absolute `href`. The settle Authorize
/// recognises a hosted session only by an absolute URL containing `/payment-sessions/`
/// (see `elavon_pg_payment_session_href`), so a bare id stashed here would be silently
/// ignored downstream: the settle would fall through to the card branch and fail with an
/// unrelated "unsupported payment method". Failing here names the real cause instead.
const ELAVON_PG_MISSING_PAYMENT_SESSION_HREF: &str =
    "elavon_pg: PreAuthenticate created the payment session but the 201 carried no `href`. \
     The settle Authorize references the session by its absolute resource URL and cannot use \
     a bare id, so the attempt fails here rather than stashing a reference the settle would \
     silently ignore.";

// =============================================================================
// AUTHENTICATION
// =============================================================================

/// EPG's only security scheme is HTTP Basic: the merchant alias is the username and
/// the secret API key (`sk_…`) is the password (spec §2.1–§2.3). The public key
/// (`pk_…`) may only touch hosted cards and would fail every flow here.
#[derive(Debug, Clone)]
pub struct ElavonPgAuthType {
    /// Merchant alias — the HTTP Basic username.
    pub merchant_alias: Secret<String>,
    /// Secret API key (`sk_…`) — the HTTP Basic password.
    pub secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ElavonPgAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::ElavonPg { api_key, key1, .. } => Ok(Self {
                merchant_alias: api_key.clone(),
                secret_key: key1.clone(),
            }),
            _ => Err(error_stack::report!(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway requires ConnectorSpecificConfig::ElavonPg with \
                         api_key = merchant alias and key1 = secret API key (sk_…) for HTTP Basic auth"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })),
        }
    }
}

// =============================================================================
// SHARED WIRE TYPES
// =============================================================================

/// EPG's discriminator on `POST /transactions` (spec §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElavonPgTransactionType {
    Sale,
    Refund,
    Void,
    /// Forward compatibility only. EPG documents that it ships new values for
    /// enumerated fields without a version bump, so an unrecognised `type` on a
    /// *response* must not fail deserialization. Never emitted on a request.
    #[serde(other)]
    Unrecognized,
}

/// `shopperInteraction` (spec §4.2). Every flow in scope is a card e-commerce
/// payment, so `ecommerce` is the only value we ever send: claiming `mailOrder` or
/// `telephoneOrder` to dodge EPG's 3DS enforcement would misrepresent the
/// transaction to the schemes. EPG also rejects `threeDSecure` on anything but
/// `ecommerce` (`"3DS is only allowed on e-commerce transactions"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ElavonPgShopperInteraction {
    Ecommerce,
}

/// `PositiveAmountAndCurrency` — EPG carries amounts as decimal strings in the
/// currency's major units (spec §3), never as a flat `amount`/`currency` pair.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgAmount {
    pub amount: StringMajorUnit,
    pub currency_code: common_enums::Currency,
}

/// `Contact` — EPG's billing/shipping address shape (spec §4.1.3).
/// `countryCode` is ISO 3166-1 **alpha-3**; alpha-2 violates its `minLength: 3`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgContact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<common_enums::CountryAlpha3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
}

impl ElavonPgContact {
    /// Returns `None` when UCS holds no billing data at all, so that `card.billTo`
    /// is omitted rather than serialized as an empty object. EPG scores AVS on what
    /// it is given (`avsStreetMismatch` / `avsPostalCodeMismatch` are decline
    /// codes), so nothing here is ever synthesised — we send exactly what UCS has.
    fn from_billing(flow_data: &PaymentFlowData) -> Option<Self> {
        let contact = Self {
            full_name: flow_data.get_optional_billing_full_name(),
            street1: flow_data.get_optional_billing_line1(),
            street2: flow_data.get_optional_billing_line2(),
            city: flow_data.get_optional_billing_city(),
            region: flow_data.get_optional_billing_state(),
            postal_code: flow_data.get_optional_billing_zip(),
            country_code: flow_data
                .get_optional_billing_country()
                .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
            email: flow_data.get_optional_billing_email(),
        };
        if contact.full_name.is_none()
            && contact.street1.is_none()
            && contact.street2.is_none()
            && contact.city.is_none()
            && contact.region.is_none()
            && contact.postal_code.is_none()
            && contact.country_code.is_none()
            && contact.email.is_none()
        {
            None
        } else {
            Some(contact)
        }
    }
}

/// EPG's `Card` request object (spec §4.1.1).
/// `expirationMonth`/`expirationYear` are JSON **integers** (1–12 and 2000–2099),
/// not zero-padded strings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgCard<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder_name: Option<Secret<String>>,
    pub number: RawCardNumber<T>,
    pub expiration_month: u8,
    pub expiration_year: u16,
    pub security_code: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_to: Option<ElavonPgContact>,
}

/// `ThreeDSecure` — EPG's external / pass-through 3DS v2 object (spec §5.1).
///
/// EPG runs no 3DS of its own on the direct API: authentication happens in the
/// separate UCS authentication leg and only its *results* travel here. The three
/// fields without `skip_serializing_if` are `required` by the schema, so the object
/// is constructed all-or-nothing — a partial object is a hard `400`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgThreeDSecure {
    pub directory_server_transaction_id: String,
    pub transaction_status: ElavonPgThreeDsTransactionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trans_status_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub electronic_commerce_indicator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_value: Option<Secret<String>>,
    pub protocol_version: String,
}

/// EPG accepts only `Y`, `N`, `U` or `A` for `threeDSecure.transactionStatus`
/// (pattern `[YNUA]`). `C`/`D` mean the challenge is still outstanding and `R`
/// means the issuer refused authorization outright — none of them may be presented
/// as a finished authentication (spec §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ElavonPgThreeDsTransactionStatus {
    Y,
    N,
    U,
    A,
}

impl TryFrom<common_enums::TransactionStatus> for ElavonPgThreeDsTransactionStatus {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(status: common_enums::TransactionStatus) -> Result<Self, Self::Error> {
        match status {
            common_enums::TransactionStatus::Success => Ok(Self::Y),
            common_enums::TransactionStatus::Failure => Ok(Self::N),
            common_enums::TransactionStatus::VerificationNotPerformed => Ok(Self::U),
            common_enums::TransactionStatus::NotVerified => Ok(Self::A),
            common_enums::TransactionStatus::Rejected
            | common_enums::TransactionStatus::ChallengeRequired
            | common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication
            | common_enums::TransactionStatus::InformationOnly => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: format!(
                        "3DS transaction status {status:?} on authentication_data.trans_status"
                    ),
                    connector: ELAVON_PG_CONNECTOR_ID,
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon Payment Gateway accepts only a completed 3DS outcome on \
                             threeDSecure.transactionStatus (Y, N, U or A). A challenge-required \
                             or issuer-rejected authentication must be finished — or abandoned — \
                             before the sale is attempted."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })
                .attach_printable("elavon_pg: unusable 3DS transaction status on Authorize"))
            }
        }
    }
}

// =============================================================================
// REQUESTS
// =============================================================================

/// The Authorize body. EPG has two shapes for the same `POST /transactions`
/// endpoint and this connector emits exactly one of them per attempt:
///
/// * [`ElavonPgAuthorizeRequest::Card`] — the raw-card sale, optionally carrying
///   pass-through 3DS results (spec §4.1). This is the shape used by the card
///   no-3DS and external-3DS paths and it is unchanged by the hosted-payment-page
///   work.
/// * [`ElavonPgAuthorizeRequest::PaymentSession`] — the settle leg of the hosted
///   payment page, where the shopper already entered the PAN on EPG's own page
///   (spec §5.2 step 5).
///
/// `untagged` is safe here because this enum is **serialize-only**: there is no
/// deserialization to misattribute, and each variant already carries its own EPG
/// discriminator (`type` on the card sale, `paymentSession` on the session settle).
/// The card variant is boxed so the two variants stay comparable in size.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ElavonPgAuthorizeRequest<T: PaymentMethodDataTypes> {
    Card(Box<ElavonPgCardSaleRequest<T>>),
    PaymentSession(ElavonPgPaymentSessionSaleRequest),
}

/// The settle leg of the hosted-payment-page flow: `POST /transactions` whose only
/// member is the payment session's resource URL (spec §5.2 step 5).
///
/// The EPG documentation is explicit that nothing else belongs here: *"the body of
/// the post only requires the payment session's resource URL […] For forward
/// compatibility, do not include hostedCard in the request."* No `card`, no
/// `threeDSecure` (the HPP already ran the authentication and EPG rejects
/// authenticating in both places), no `type`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgPaymentSessionSaleRequest {
    /// `PaymentSession` resource URL minted by the PreAuthenticate leg.
    pub payment_session: String,
}

/// `SaleTransaction` — the raw-card Authorize body (spec §4.1).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgCardSaleRequest<T: PaymentMethodDataTypes> {
    #[serde(rename = "type")]
    pub transaction_type: ElavonPgTransactionType,
    pub total: ElavonPgAmount,
    pub card: ElavonPgCard<T>,
    pub shopper_interaction: ElavonPgShopperInteraction,
    /// `true` = sale (auto capture), `false` = authorize-only. EPG defaults it to
    /// `true`; we always send it explicitly so the wire shape is unambiguous.
    pub do_capture: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_secure: Option<ElavonPgThreeDSecure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shopper_email_address: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shopper_ip_address: Option<Secret<String, common_utils::pii::IpAddress>>,
    /// The only merchant-controlled, filterable field on a transaction. It is the
    /// reconciliation handle if a `201` is ever lost (spec §4.1.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// `VoidTransaction` — `POST /transactions` with `type: "void"` (spec §8.3).
/// A void is always full, so it carries no `total`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgVoidRequest {
    #[serde(rename = "type")]
    pub transaction_type: ElavonPgTransactionType,
    pub parent_transaction: String,
    pub shopper_interaction: ElavonPgShopperInteraction,
}

/// `RefundTransaction` — `POST /transactions` with `type: "refund"` (spec §8.4).
/// `total` is optional on the wire but always sent: an explicit amount is safer
/// than relying on EPG's implicit full refund.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgRefundRequest {
    #[serde(rename = "type")]
    pub transaction_type: ElavonPgTransactionType,
    pub total: ElavonPgAmount,
    pub parent_transaction: String,
    pub shopper_interaction: ElavonPgShopperInteraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_reference: Option<String>,
}

/// Capture — `POST /partial-captures` (spec §8.2.b).
///
/// EPG has no `/capture` endpoint. It offers two ways to capture, and this
/// connector deliberately uses only one of them:
///
/// * `POST /transactions/{id}` with `{"doCapture": true}` flips the sale to
///   auto-capture and therefore captures the **entire authorized amount**. It
///   carries no amount, so it is only safe when the capture amount is known to
///   equal the authorization.
/// * `POST /partial-captures` states `total` explicitly and can never capture more
///   than it is asked for.
///
/// UCS's Capture flow does not carry the original authorized amount —
/// `PaymentFlowData::amount` is `None` on this path — so the equality that would
/// justify the update endpoint can never be established. Routing every capture
/// through the partial-capture resource is the only variant that cannot
/// over-capture; a "full" capture is simply one whose `total` happens to equal the
/// authorization, with `isFinal` releasing any remainder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgCaptureRequest {
    /// Reference to the parent sale. EPG parses either an `href` or a bare `id`.
    pub transaction: String,
    pub total: ElavonPgAmount,
    /// When `true`, EPG may reverse any authorized amount left uncaptured.
    pub is_final: bool,
}

/// `OrderInput` — the CreateOrder body (spec §5.2 step 1).
///
/// An EPG `Order` describes what the shopper is paying for, and it is the only
/// resource a `PaymentSession` is required to reference. `total` is the schema's
/// single required member; every other `OrderInput` field is descriptive and would
/// only duplicate what the sale itself already carries.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgCreateOrderRequest {
    pub total: ElavonPgAmount,
}

/// `Order` — the CreateOrder response (spec §5.2 step 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgCreateOrderResponse {
    pub id: String,
    /// Self link. `PaymentSessionInput.order` is a `ResourceURL<Order>`, so the
    /// href is what the PreAuthenticate leg wants. EPG parses a reference as
    /// *either* an href or a bare id, which is why an absent href degrades to `id`
    /// rather than failing.
    pub href: Option<String>,
}

/// `PaymentSessionInputRedirect` — the PreAuthenticate body (spec §5.2 step 2).
///
/// `hppType` is omitted: `PaymentSessionInput` documents it as *"defaults to
/// fullPageRedirect"*, which is the variant this connector uses, and EPG's own
/// published example omits it too.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgPreAuthenticateRequest {
    /// `Order` resource URL produced by the CreateOrder leg.
    pub order: String,
    /// Where EPG sends the shopper once the hosted page has collected the card and
    /// finished 3DS. Required for `hppType = fullPageRedirect`.
    pub return_url: String,
    /// Where EPG sends the shopper if they abandon the hosted page. Also required
    /// for `hppType = fullPageRedirect`.
    pub cancel_url: String,
    pub do_three_d_secure: bool,
    pub do_create_transaction: bool,
    /// Capture intent for the sale the hosted page will create.
    ///
    /// EPG defaults this to `true`, so it must be sent explicitly (spec §4.2): a
    /// `capture_method = manual` payment would otherwise be auto-captured on the
    /// hosted page and the merchant's later Capture would fail against an
    /// already-captured transaction. The settle call carries only the payment-session
    /// URL, so this is the one place capture intent can be expressed on this path.
    pub do_capture: bool,
}

/// `PaymentSession` — the PreAuthenticate `201` (spec §5.2 step 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgPreAuthenticateResponse {
    pub id: String,
    /// Self link; this is what the settle `POST /transactions` references. As with
    /// the order href, an absent value degrades to `id` because EPG parses either
    /// form.
    pub href: Option<String>,
    /// *"URL that shoppers will use"* — the hosted payment page the browser is sent
    /// to.
    ///
    /// **UNVERIFIED AGAINST A LIVE SANDBOX.** `url` is documented on the
    /// `PaymentSession` schema but absent from EPG's published `201` example; no
    /// EPG credentials exist for this integration, so it has only been exercised
    /// against a mock. It is modelled as optional and its absence is reported as an
    /// actionable error rather than papered over with a guessed URL.
    pub url: Option<String>,
    /// Session expiry (EPG mints these with a 30-minute lifetime).
    pub expires_at: Option<String>,
}

// =============================================================================
// RESPONSES
// =============================================================================

/// `TransactionState` (spec §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElavonPgTransactionState {
    Declined,
    Authorized,
    Captured,
    Voided,
    Settled,
    Expired,
    SettlementDelayed,
    Rejected,
    HeldForReview,
    AuthorizationPending,
    Unknown,
    /// Forward compatibility: EPG documents that it ships new values for enumerated
    /// fields without a version bump. An unrecognised state is treated exactly like
    /// `unknown` — `Pending`, so the payment is re-synced — never as a success or a
    /// failure (spec §6.5).
    #[serde(other)]
    Unrecognized,
}

/// One entry of EPG's `failures[]` array (spec §7.1). It appears both inside a
/// `FailureWrapper` on a non-2xx and inline on a `201` that carries a decline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElavonPgFailure {
    pub code: Option<String>,
    pub description: Option<String>,
    pub field: Option<String>,
}

/// `rawProcessorResponseInfo` — the processor's own verbatim response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgRawProcessorResponseInfo {
    pub processor_response_code: Option<String>,
    pub processor_response_message: Option<String>,
}

/// EPG's `Transaction` resource (spec §4.4). The same shape is returned by
/// `POST /transactions` (sale, void and refund alike) and by
/// `GET /transactions/{id}`, so one struct serves Authorize, PSync, Void, Refund
/// and RSync. Deserialization is deliberately lenient: live responses carry fields
/// absent from the published OpenAPI document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgTransactionResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub transaction_type: Option<ElavonPgTransactionType>,
    pub state: ElavonPgTransactionState,
    pub is_authorized: Option<bool>,
    pub is_held_for_review: Option<bool>,
    /// Echo of the request's `doCapture`. It is the only thing that distinguishes an
    /// auto-capture sale from an auth-only when `state` reads `authorized`.
    pub do_capture: Option<bool>,
    pub authorization_code: Option<String>,
    pub processor_reference: Option<String>,
    pub scheme_reference: Option<String>,
    pub issuer_response_code: Option<String>,
    pub raw_processor_response_info: Option<ElavonPgRawProcessorResponseInfo>,
    #[serde(default)]
    pub failures: Vec<ElavonPgFailure>,
}

/// PSync reads the very same resource as Authorize; the alias exists only because
/// the connector macros key their bridge types off the response type's name.
pub type ElavonPgPsyncResponse = ElavonPgTransactionResponse;
/// Void creates a *new* `Transaction` of type `void` (spec §8.3).
pub type ElavonPgVoidResponse = ElavonPgTransactionResponse;
/// Refund creates a *new* `Transaction` of type `refund` (spec §8.4).
pub type ElavonPgRefundResponse = ElavonPgTransactionResponse;
/// RSync reads that refund transaction back (spec §8.5).
pub type ElavonPgRsyncResponse = ElavonPgTransactionResponse;

/// EPG's `PartialCapture` resource (spec §8.2.b). `PartialCaptureState`
/// (`authorized`, `captured`, `settled`, `declined`, `unknown`) is a strict subset
/// of `TransactionState`, so the shared state enum and the shared status mapping
/// both apply. A `PartialCapture` has no `isAuthorized` field at all, hence the
/// `Option`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElavonPgCaptureResponse {
    pub id: String,
    pub state: ElavonPgTransactionState,
    pub is_authorized: Option<bool>,
    pub is_held_for_review: Option<bool>,
    pub processor_reference: Option<String>,
    #[serde(default)]
    pub failures: Vec<ElavonPgFailure>,
}

/// `FailureWrapper` — the body of every non-2xx EPG response (spec §7.1).
/// 5xx responses may carry no body at all, hence every field is optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ElavonPgErrorResponse {
    pub status: Option<i32>,
    #[serde(default)]
    pub failures: Vec<ElavonPgFailure>,
}

// =============================================================================
// STATUS MAPPING
// =============================================================================

/// Everything needed to resolve a `sale` transaction (or a capture resource) into an
/// `AttemptStatus`, per the table in spec §6.3.
pub struct ElavonPgSaleStatus<'a> {
    pub state: &'a ElavonPgTransactionState,
    /// `None` on a `PartialCapture`, which has no such field.
    pub is_authorized: Option<bool>,
    pub is_held_for_review: bool,
    /// Derived from the response's `doCapture` echo wherever EPG returns one.
    pub is_auto_capture: bool,
}

impl From<ElavonPgSaleStatus<'_>> for AttemptStatus {
    fn from(status: ElavonPgSaleStatus<'_>) -> Self {
        match status.state {
            // EPG's `authorized` means "the issuer approved it". On a `doCapture:
            // true` sale the money is already committed to settlement and no further
            // action is possible, so it is `Charged`, not `Authorized` — reporting
            // `Authorized` would invite a Capture that EPG rejects.
            ElavonPgTransactionState::Authorized => {
                if status.is_held_for_review {
                    Self::Pending
                } else {
                    match status.is_authorized {
                        Some(false) => Self::Failure,
                        // `isAuthorized` absent: only a `PartialCapture` omits it, and
                        // there `state` alone is authoritative.
                        Some(true) | None => {
                            if status.is_auto_capture {
                                Self::Charged
                            } else {
                                Self::Authorized
                            }
                        }
                    }
                }
            }
            // Frozen by a fraud rule: authorized but no further processing happens
            // until a human releases it, so it is neither success nor failure.
            ElavonPgTransactionState::HeldForReview => Self::Pending,
            ElavonPgTransactionState::Captured
            | ElavonPgTransactionState::Settled
            | ElavonPgTransactionState::SettlementDelayed => Self::Charged,
            ElavonPgTransactionState::Declined
            | ElavonPgTransactionState::Rejected
            | ElavonPgTransactionState::Expired => Self::Failure,
            ElavonPgTransactionState::Voided => Self::Voided,
            // Async authorization (added for BLIK) that has not resolved yet.
            ElavonPgTransactionState::AuthorizationPending => Self::Pending,
            // Soft-fail: re-sync rather than declare an outcome we cannot read.
            ElavonPgTransactionState::Unknown | ElavonPgTransactionState::Unrecognized => {
                Self::Pending
            }
        }
    }
}

/// A `void` or `refund` is itself a new `Transaction` whose success reads
/// `state: "authorized"` / `isAuthorized: true` — the *parent* is what moves to
/// `voided`. Running such a response through the sale table would map a successful
/// void to `Charged`, so it gets its own mapping (spec §6.4).
pub struct ElavonPgChildStatus<'a> {
    pub state: &'a ElavonPgTransactionState,
    pub is_authorized: Option<bool>,
}

impl From<ElavonPgChildStatus<'_>> for AttemptStatus {
    fn from(status: ElavonPgChildStatus<'_>) -> Self {
        if status.is_authorized == Some(false) {
            return Self::VoidFailed;
        }
        match status.state {
            // The gateway authorized the void; the parent is now reversed.
            ElavonPgTransactionState::Authorized
            | ElavonPgTransactionState::Captured
            | ElavonPgTransactionState::Settled
            | ElavonPgTransactionState::SettlementDelayed
            | ElavonPgTransactionState::Voided => Self::Voided,
            ElavonPgTransactionState::Declined
            | ElavonPgTransactionState::Rejected
            | ElavonPgTransactionState::Expired => Self::VoidFailed,
            ElavonPgTransactionState::HeldForReview
            | ElavonPgTransactionState::AuthorizationPending
            | ElavonPgTransactionState::Unknown
            | ElavonPgTransactionState::Unrecognized => Self::Pending,
        }
    }
}

impl From<ElavonPgChildStatus<'_>> for RefundStatus {
    fn from(status: ElavonPgChildStatus<'_>) -> Self {
        if status.is_authorized == Some(false) {
            return Self::Failure;
        }
        match status.state {
            ElavonPgTransactionState::Authorized
            | ElavonPgTransactionState::Captured
            | ElavonPgTransactionState::Settled
            | ElavonPgTransactionState::SettlementDelayed => Self::Success,
            // A refund that is declined, rejected, expired or reversed is terminal:
            // it must not be left in `Pending` where it can never resolve.
            ElavonPgTransactionState::Declined
            | ElavonPgTransactionState::Rejected
            | ElavonPgTransactionState::Expired
            | ElavonPgTransactionState::Voided => Self::Failure,
            ElavonPgTransactionState::HeldForReview
            | ElavonPgTransactionState::AuthorizationPending
            | ElavonPgTransactionState::Unknown
            | ElavonPgTransactionState::Unrecognized => Self::Pending,
        }
    }
}

// =============================================================================
// ERROR MAPPING
// =============================================================================

/// Renders EPG's `failures[]` into `(code, message, reason)` (spec §7.4).
///
/// The first failure names *where* in the gateway → processor → issuer chain the
/// transaction failed (`declinedByProcessor`, `badRequest`, …); the remaining ones
/// carry the actual cause (`cardExpired`, `total.currencyCode must not be null`),
/// so all of them are folded into `reason` rather than dropped.
pub fn summarize_failures(failures: &[ElavonPgFailure]) -> (String, String, Option<String>) {
    let code = failures
        .first()
        .and_then(|failure| failure.code.clone())
        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
    let message = failures
        .first()
        .and_then(|failure| failure.description.clone())
        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
    let reason = if failures.is_empty() {
        None
    } else {
        Some(
            failures
                .iter()
                .map(|failure| {
                    let code = failure.code.as_deref().unwrap_or(consts::NO_ERROR_CODE);
                    let description = failure
                        .description
                        .as_deref()
                        .unwrap_or(consts::NO_ERROR_MESSAGE);
                    match failure.field.as_deref() {
                        Some(field) => format!("{code} ({field}): {description}"),
                        None => format!("{code}: {description}"),
                    }
                })
                .collect::<Vec<_>>()
                .join(ELAVON_PG_FAILURE_JOIN),
        )
    };
    (code, message, reason)
}

/// Builds the `ErrorResponse` for an in-band decline — a `201`/`200` whose body is a
/// perfectly valid `Transaction` carrying `state: "declined"` and a populated
/// `failures[]` (spec §4.5). This never goes through `build_error_response`, which
/// only ever sees non-2xx `FailureWrapper` bodies.
fn in_band_error(
    failures: &[ElavonPgFailure],
    http_code: u16,
    connector_transaction_id: String,
    attempt_status: FlowStatus,
    issuer_response_code: Option<String>,
    processor_message: Option<String>,
) -> ErrorResponse {
    let (code, message, reason) = summarize_failures(failures);
    ErrorResponse {
        status_code: http_code,
        code,
        message,
        reason,
        attempt_status: Some(attempt_status),
        connector_transaction_id: Some(connector_transaction_id),
        // EPG has no dedicated network-advice field; the issuer's raw response code
        // is the closest machine-readable decline code it exposes.
        network_decline_code: issuer_response_code,
        network_advice_code: None,
        network_error_message: processor_message,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_response: None,
        typed_connector_request: None,
    }
}

// =============================================================================
// REQUEST BUILDING HELPERS
// =============================================================================

/// Extracts the card, rejecting every other payment method by name. EPG's direct
/// API also accepts hosted cards, stored cards and wallets, but none of those are
/// in scope for this connector.
fn get_card<T: PaymentMethodDataTypes + std::fmt::Debug>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<&Card<T>, error_stack::Report<IntegrationError>> {
    match payment_method_data {
        PaymentMethodData::Card(card) => Ok(card),
        other => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("Payment method {other:?}"),
            connector: ELAVON_PG_CONNECTOR_ID,
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Elavon Payment Gateway is integrated for raw Card payments only \
                     (payment_method_data.card). Wallets, bank transfers, BNPL and stored \
                     credentials are not implemented for this connector."
                        .to_string(),
                ),
                ..Default::default()
            },
        })
        .attach_printable("elavon_pg: unsupported payment method on Authorize")),
    }
}

/// EPG's `Card.expirationMonth` is an integer in 1–12, never a zero-padded string.
fn expiration_month<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<u8, error_stack::Report<IntegrationError>> {
    card.card_exp_month
        .peek()
        .trim()
        .parse::<u8>()
        .ok()
        .filter(|month| (1..=12).contains(month))
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "payment_method_data.card.card_exp_month",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway expects Card.expirationMonth as an integer \
                         between 1 and 12"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("elavon_pg: card expiry month is not an integer in 1..=12")
        })
}

/// EPG's `Card.expirationYear` is a 4-digit integer in 2000–2099, so a 2-digit UCS
/// expiry is expanded before it is parsed.
fn expiration_year<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<u16, error_stack::Report<IntegrationError>> {
    card.get_expiry_year_4_digit()
        .peek()
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|year| (2000..=2099).contains(year))
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "payment_method_data.card.card_exp_year",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway expects Card.expirationYear as a four-digit \
                         integer between 2000 and 2099"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("elavon_pg: card expiry year is not a four-digit year in 2000..=2099")
        })
}

/// `Card.securityCode` is `required` by EPG's schema, so a missing CVC is rejected
/// here — naming the caller-facing field — rather than sent as an incomplete card.
fn security_code<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    if card.card_cvc.peek().trim().is_empty() {
        Err(
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data.card.card_cvc",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway declares Card.securityCode required on every \
                         card sale (3 digits for Visa/Mastercard/Discover, 4 for Amex)"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("elavon_pg: card CVC absent on Authorize"),
        )
    } else {
        Ok(card.card_cvc.clone())
    }
}

/// Builds EPG's `threeDSecure` object from the external-authentication results UCS
/// carries on the Authorize request (spec §5).
///
/// EPG's direct API supports pass-through 3DS only: there is no challenge, no
/// redirect and no completion leg. When the caller asked for 3DS but no
/// authentication results are present, this errors rather than silently degrading
/// to a no-3DS sale — EPG would decline that with `3dsEnforcedOnEcommerceSales` on
/// any 3DS-enforcing merchant, and a decline is a far worse diagnostic than a
/// validation error.
fn build_three_d_secure(
    authentication_data: Option<&AuthenticationData>,
    is_three_ds: bool,
) -> Result<Option<ElavonPgThreeDSecure>, error_stack::Report<IntegrationError>> {
    let authentication_data = match authentication_data {
        Some(authentication_data) => authentication_data,
        None => {
            if is_three_ds {
                return Err(
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "authentication_data",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "auth_type is three_ds but the Authorize request carries no \
                                 external 3DS authentication results. Elavon Payment Gateway \
                                 performs no 3DS of its own on the direct API: it needs \
                                 authentication_data.ds_trans_id, \
                                 authentication_data.trans_status and \
                                 authentication_data.message_version from the authentication leg."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })
                    .attach_printable(
                        "elavon_pg: three_ds Authorize without external authentication results",
                    ),
                );
            }
            return Ok(None);
        }
    };

    // EPG's pattern for `directoryServerTransactionId` accepts lowercase hex only.
    let directory_server_transaction_id = authentication_data
        .ds_trans_id
        .as_ref()
        .map(|id| id.to_lowercase())
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.ds_trans_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway requires \
                         threeDSecure.directoryServerTransactionId (an RFC 4122 UUID) on every \
                         3DS sale"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("elavon_pg: 3DS directory server transaction id absent")
        })?;

    let transaction_status = authentication_data
        .trans_status
        .clone()
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.trans_status",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway requires threeDSecure.transactionStatus \
                         (Y, N, U or A) on every 3DS sale"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("elavon_pg: 3DS transaction status absent")
        })
        .and_then(ElavonPgThreeDsTransactionStatus::try_from)?;

    let protocol_version = authentication_data
        .message_version
        .as_ref()
        .map(ToString::to_string)
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.message_version",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway requires threeDSecure.protocolVersion in \
                         <major>.<minor>.<patch> form (3DS 1.x is no longer accepted)"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
            .attach_printable("elavon_pg: 3DS protocol version absent")
        })?;

    // EPG rejects any CAVV that is not exactly 28 characters with a hard
    // `fieldValidationFailure`. Dropping an out-of-spec value keeps the sale alive
    // on transactionStatus + ECI alone, which is what the schema allows, but it is
    // never done silently.
    let authentication_value = match authentication_data.cavv.as_ref() {
        Some(cavv) if cavv.peek().len() == ELAVON_PG_CAVV_LENGTH => Some(cavv.clone()),
        Some(_) => {
            tracing::warn!(
                connector = ELAVON_PG_CONNECTOR_ID,
                expected_length = ELAVON_PG_CAVV_LENGTH,
                "dropping authentication_data.cavv: Elavon Payment Gateway accepts \
                 threeDSecure.authenticationValue only at exactly 28 characters"
            );
            None
        }
        None => None,
    };

    // The 3DS `transStatusReason` reaches UCS as the challenge-code reason. EPG's
    // pattern is exactly two numeric digits, so a value in any other shape is
    // dropped (and warned about) rather than sent into a `fieldValidationFailure`.
    let trans_status_reason = match authentication_data.challenge_code_reason.as_ref() {
        Some(reason)
            if reason.len() == ELAVON_PG_TRANS_STATUS_REASON_LENGTH
                && reason.chars().all(|character| character.is_ascii_digit()) =>
        {
            Some(reason.clone())
        }
        Some(_) => {
            tracing::warn!(
                connector = ELAVON_PG_CONNECTOR_ID,
                "dropping authentication_data.challenge_code_reason: Elavon Payment Gateway \
                 accepts threeDSecure.transStatusReason only as exactly two numeric digits"
            );
            None
        }
        None => None,
    };

    Ok(Some(ElavonPgThreeDSecure {
        directory_server_transaction_id,
        transaction_status,
        trans_status_reason,
        electronic_commerce_indicator: authentication_data.eci.clone(),
        authentication_value,
        protocol_version,
    }))
}

// =============================================================================
// HOSTED PAYMENT PAGE (gateway 3DS) — SHARED HELPERS
// =============================================================================

/// EPG resource references parse as **either** an `href` or a bare `id` —
/// *"Resource reference could not be parsed as either an href or an id"* is the
/// failure when neither does. A `201` that omits its self link is therefore still
/// usable, by referencing the resource with its id. Every published example uses
/// the href, so taking the id is a deliberate, warned-about fallback rather than a
/// silent one.
pub(crate) fn elavon_pg_resource_reference(
    href: Option<String>,
    id: &str,
    resource: &str,
) -> String {
    match href {
        Some(href) => href,
        None => {
            tracing::warn!(
                connector = ELAVON_PG_CONNECTOR_ID,
                resource = resource,
                resource_id = id,
                "Elavon Payment Gateway response omitted the resource href; referencing the \
                 resource by its bare id instead"
            );
            id.to_string()
        }
    }
}

/// Threads the hosted-payment-page session href from PreAuthenticate to the settle
/// Authorize inside `AuthenticationData.threeds_server_transaction_id`.
///
/// That is the only field Hyperswitch carries across the shopper redirect, and EPG
/// runs the 3DS itself on this path so there is no real 3DS-server transaction id to
/// displace. It is the same channel Paysafe uses for its `paymentHandleToken`.
fn elavon_pg_payment_session_authentication_data(
    payment_session_href: String,
) -> AuthenticationData {
    AuthenticationData {
        threeds_server_transaction_id: Some(payment_session_href),
        trans_status: None,
        eci: None,
        cavv: None,
        ucaf_collection_indicator: None,
        message_version: None,
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
    }
}

/// Reads back the payment-session href stashed by
/// [`elavon_pg_payment_session_authentication_data`].
///
/// `threeds_server_transaction_id` is shared with the external/pass-through 3DS
/// path, where it legitimately carries a real 3DS-server transaction id (a UUID), so
/// a value is only claimed here when it is shaped like an EPG payment-session
/// resource URL. Anything else falls through to the card branch of Authorize, which
/// is what keeps the two already-proven card paths byte-for-byte unchanged.
pub(crate) fn elavon_pg_payment_session_href(
    authentication_data: Option<&AuthenticationData>,
) -> Option<String> {
    authentication_data
        .and_then(|data| data.threeds_server_transaction_id.as_deref())
        .filter(|reference| {
            ELAVON_PG_URL_SCHEMES
                .iter()
                .any(|scheme| reference.starts_with(scheme))
                && reference.contains(ELAVON_PG_PAYMENT_SESSIONS_SEGMENT)
        })
        .map(str::to_owned)
}

// =============================================================================
// CREATE ORDER — POST /orders
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonPgRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for ElavonPgCreateOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonPgRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        let amount = item
            .connector
            .amount_converter
            .convert(request.amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway carries total.amount as a decimal string in the \
                         currency's major units (at most 9 integer and 4 fractional digits). The \
                         order total must equal the amount the payment session will authenticate: \
                         EPG rejects a mismatch with \"Transaction amount does not match the 3DS \
                         authorized amount\"."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            total: ElavonPgAmount {
                amount,
                currency_code: request.currency,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<ElavonPgCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ElavonPgCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // The order **href** is what travels onward: `PaymentSessionInput.order` is
        // a `ResourceURL<Order>`, not a bare id.
        let order_reference = elavon_pg_resource_reference(
            item.response.href,
            &item.response.id,
            ELAVON_PG_ORDER_RESOURCE,
        );

        Ok(Self {
            response: Ok(PaymentCreateOrderResponse {
                connector_order_id: order_reference.clone(),
                session_data: None,
            }),
            resource_common_data: PaymentFlowData {
                // An order is not yet a payment: nothing has been authorized and no
                // shopper has been asked for anything.
                status: AttemptStatus::Pending,
                connector_order_id: Some(order_reference),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// PRE-AUTHENTICATE — POST /payment-sessions
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonPgRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ElavonPgPreAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonPgRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let order = router_data
            .resource_common_data
            .connector_order_id
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Run PaymentService/CreateOrder first and pass the Order resource URL it \
                         returns as connector_order_id on the PreAuthenticate request."
                            .to_string(),
                    ),
                    additional_context: Some(
                        "Elavon Payment Gateway's hosted payment page is opened against an \
                         existing Order: `order` is the one required member of \
                         PaymentSessionInput and there is no way to create the order inline."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        // The shopper must land back on the complete-authorize continuation so
        // Hyperswitch settles the payment; `return_url` alone only triggers a sync.
        let return_url = request
            .continue_redirection_url
            .as_ref()
            .map(|url| url.to_string())
            .or_else(|| router_data.resource_common_data.get_return_url())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway requires returnUrl on a fullPageRedirect payment \
                         session: it is where the hosted payment page sends the shopper once the \
                         card has been collected and 3DS has run."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        // Cancellation must NOT land on the complete-authorize continuation: the shopper
        // abandoned the hosted page, so there is no session to settle and sending them
        // there would try to charge a card that was never entered. Fall back to the
        // merchant return URL, and only to `return_url` if the merchant configured
        // nothing else — in which case the two are the same URL by the merchant's own
        // choice rather than by our silent default.
        let cancel_url = router_data
            .resource_common_data
            .get_return_url()
            .or_else(|| {
                request
                    .router_return_url
                    .as_ref()
                    .map(|url| url.to_string())
            })
            .unwrap_or_else(|| return_url.clone());

        Ok(Self {
            order,
            return_url,
            cancel_url,
            do_three_d_secure: ELAVON_PG_HPP_DO_THREE_D_SECURE,
            do_create_transaction: ELAVON_PG_HPP_DO_CREATE_TRANSACTION,
            do_capture: request.is_auto_capture()?,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ElavonPgPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ElavonPgPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let response = item.response;

        // No URL means no hosted payment page, and there is nothing to guess: the
        // page lives on an EPG-owned host under an opaque session id. Fail here,
        // naming the field, instead of shipping the shopper somewhere invented.
        let hosted_page_url = response
            .url
            .ok_or_else(|| {
                ConnectorError::unexpected_response_error_with_context(
                    http_code,
                    Some(ELAVON_PG_MISSING_HOSTED_PAGE_URL.to_string()),
                )
            })
            .attach_printable(ELAVON_PG_MISSING_HOSTED_PAGE_URL)?;

        // Deliberately NOT `elavon_pg_resource_reference`: the bare-id fallback is right
        // for the Order (EPG accepts either an href or an id there) but harmful for the
        // PaymentSession, which the settle can only recognise as an absolute URL.
        let payment_session_reference = response
            .href
            .filter(|href| !href.trim().is_empty())
            .ok_or_else(|| {
                ConnectorError::unexpected_response_error_with_context(
                    http_code,
                    Some(ELAVON_PG_MISSING_PAYMENT_SESSION_HREF.to_string()),
                )
            })
            .attach_printable(ELAVON_PG_MISSING_PAYMENT_SESSION_HREF)?;

        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                // Nothing has been created on the card network yet — the Transaction
                // only comes into being on the settle Authorize.
                resource_id: Some(ResponseId::NoResponseId),
                authentication_data: Some(elavon_pg_payment_session_authentication_data(
                    payment_session_reference,
                )),
                redirection_data: Some(Box::new(RedirectForm::Form {
                    endpoint: hosted_page_url,
                    // The hosted page is opened with a plain browser navigation; EPG
                    // carries the session identity in the URL itself.
                    method: Method::Get,
                    form_fields: std::collections::HashMap::new(),
                })),
                connector_response_reference_id: Some(response.id),
                status_code: http_code,
            }),
            resource_common_data: PaymentFlowData {
                // The shopper still has to visit the hosted page and authenticate.
                status: AttemptStatus::AuthenticationPending,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// AUTHORIZE
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonPgRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ElavonPgAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonPgRouterData<
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

        // Precedence (spec §5.2 step 5 vs §4.1/§5.1): a hosted-payment-page session
        // href beats everything, because on that path EPG already holds the card and
        // already ran 3DS — sending `card` or `threeDSecure` alongside it is rejected.
        // Only then does the external/pass-through 3DS shape apply, and failing that,
        // the plain card sale.
        if let Some(payment_session) =
            elavon_pg_payment_session_href(request.authentication_data.as_ref())
        {
            return Ok(Self::PaymentSession(ElavonPgPaymentSessionSaleRequest {
                payment_session,
            }));
        }

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway carries total.amount as a decimal string in the \
                         currency's major units (at most 9 integer and 4 fractional digits)"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let card = get_card(&request.payment_method_data)?;

        Ok(Self::Card(Box::new(ElavonPgCardSaleRequest {
            transaction_type: ElavonPgTransactionType::Sale,
            total: ElavonPgAmount {
                amount,
                currency_code: request.currency,
            },
            card: ElavonPgCard {
                holder_name: card.card_holder_name.clone(),
                number: card.card_number.clone(),
                expiration_month: expiration_month(card)?,
                expiration_year: expiration_year(card)?,
                security_code: security_code(card)?,
                bill_to: ElavonPgContact::from_billing(&router_data.resource_common_data),
            },
            shopper_interaction: ElavonPgShopperInteraction::Ecommerce,
            do_capture: request.is_auto_capture(),
            three_d_secure: build_three_d_secure(
                request.authentication_data.as_ref(),
                router_data.resource_common_data.is_three_ds(),
            )?,
            shopper_email_address: request.email.clone(),
            shopper_ip_address: request
                .browser_info
                .as_ref()
                .and_then(|browser_info| browser_info.ip_address)
                .map(|ip_address| Secret::new(ip_address.to_string())),
            custom_reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: router_data.resource_common_data.description.clone(),
        })))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ElavonPgTransactionResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ElavonPgTransactionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = resolve_auto_capture(
            item.response.do_capture,
            item.router_data.request.is_auto_capture(),
            &item.response.id,
        );
        Ok(build_sale_router_data(
            item.response,
            item.http_code,
            is_auto_capture,
            item.router_data,
        ))
    }
}

// =============================================================================
// PSYNC
// =============================================================================

impl TryFrom<ResponseRouterData<ElavonPgPsyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ElavonPgPsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_auto_capture = resolve_auto_capture(
            item.response.do_capture,
            item.router_data.request.is_auto_capture(),
            &item.response.id,
        );
        Ok(build_sale_router_data(
            item.response,
            item.http_code,
            is_auto_capture,
            item.router_data,
        ))
    }
}

/// `doCapture` is echoed on every `Transaction`, so the capture intent is read from
/// the response — which keeps Authorize and PSync (which has no request-side capture
/// method of the connector's own) on exactly the same code path. The request-side
/// value is only a fallback, and taking it is warned about.
fn resolve_auto_capture(
    response_do_capture: Option<bool>,
    request_is_auto_capture: bool,
    transaction_id: &str,
) -> bool {
    match response_do_capture {
        Some(do_capture) => do_capture,
        None => {
            tracing::warn!(
                connector = ELAVON_PG_CONNECTOR_ID,
                transaction_id = transaction_id,
                "Elavon Payment Gateway response omitted doCapture; falling back to the \
                 request's capture method to classify the transaction state"
            );
            request_is_auto_capture
        }
    }
}

/// Shared Authorize/PSync response shaping. A `201` carrying `state: "declined"` is
/// a successful HTTP response describing a *failed payment*: it is mapped here, from
/// the `Transaction` body, and never routed through `build_error_response` (which
/// only ever sees `FailureWrapper` bodies on non-2xx).
fn build_sale_router_data<F, Req>(
    response: ElavonPgTransactionResponse,
    http_code: u16,
    is_auto_capture: bool,
    router_data: RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
) -> RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData> {
    let status = AttemptStatus::from(ElavonPgSaleStatus {
        state: &response.state,
        is_authorized: response.is_authorized,
        is_held_for_review: response.is_held_for_review.unwrap_or(false),
        is_auto_capture,
    });

    let response_body = if status == AttemptStatus::Failure {
        Err(in_band_error(
            &response.failures,
            http_code,
            response.id.clone(),
            FlowStatus::Payment(status),
            response.issuer_response_code.clone(),
            response
                .raw_processor_response_info
                .as_ref()
                .and_then(|info| info.processor_response_message.clone()),
        ))
    } else {
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            // EPG's direct API never asks us to challenge the shopper: there is no
            // ACS URL, no challenge payload and no completion leg (spec §5.2).
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: response.scheme_reference.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: response.processor_reference.clone(),
            incremental_authorization_allowed: None,
            splits: None,
            status_code: http_code,
            payment_account_reference: None,
        })
    };

    RouterDataV2 {
        response: response_body,
        resource_common_data: PaymentFlowData {
            status,
            ..router_data.resource_common_data
        },
        ..router_data
    }
}

// =============================================================================
// CAPTURE
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonPgRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for ElavonPgCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonPgRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount_to_capture, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway carries PartialCapture.total.amount as a decimal \
                         string in the currency's major units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            transaction: request.get_connector_transaction_id().change_context(
                IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Elavon Payment Gateway needs the parent sale's transaction id on \
                             PartialCapture.transaction"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                },
            )?,
            total: ElavonPgAmount {
                amount,
                currency_code: request.currency,
            },
            // A single capture is final, so EPG may release whatever is left of the
            // authorization; when the caller declared multiple captures, the
            // remaining authorization must stay open for the next one.
            is_final: request.multiple_capture_data.is_none(),
        })
    }
}

impl TryFrom<ResponseRouterData<ElavonPgCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ElavonPgCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // A capture that EPG authorized is money committed to settlement, so the
        // shared sale table is evaluated with auto-capture semantics: `authorized`
        // here means `Charged`, never "still needs capturing".
        let status = AttemptStatus::from(ElavonPgSaleStatus {
            state: &item.response.state,
            is_authorized: item.response.is_authorized,
            is_held_for_review: item.response.is_held_for_review.unwrap_or(false),
            is_auto_capture: true,
        });

        let response = if status == AttemptStatus::Failure {
            Err(in_band_error(
                &item.response.failures,
                item.http_code,
                item.response.id.clone(),
                FlowStatus::Payment(status),
                None,
                None,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.processor_reference.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

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

// =============================================================================
// VOID
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonPgRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ElavonPgVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonPgRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            transaction_type: ElavonPgTransactionType::Void,
            parent_transaction: item.router_data.request.connector_transaction_id.clone(),
            shopper_interaction: ElavonPgShopperInteraction::Ecommerce,
        })
    }
}

impl TryFrom<ResponseRouterData<ElavonPgVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<ElavonPgVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(ElavonPgChildStatus {
            state: &item.response.state,
            is_authorized: item.response.is_authorized,
        });

        // The void is a *new* Transaction with its own id; the resource UCS reports
        // back stays the parent payment (the thing that was voided), and the void's
        // own id is surfaced as the connector response reference.
        let parent_transaction_id = item.router_data.request.connector_transaction_id.clone();

        let response = if status == AttemptStatus::VoidFailed {
            Err(in_band_error(
                &item.response.failures,
                item.http_code,
                parent_transaction_id.clone(),
                FlowStatus::Payment(status),
                item.response.issuer_response_code.clone(),
                item.response
                    .raw_processor_response_info
                    .as_ref()
                    .and_then(|info| info.processor_response_message.clone()),
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(parent_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

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

// =============================================================================
// REFUND
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ElavonPgRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for ElavonPgRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ElavonPgRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_refund_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Elavon Payment Gateway carries the refund total.amount as a decimal \
                         string in the currency's major units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            transaction_type: ElavonPgTransactionType::Refund,
            total: ElavonPgAmount {
                amount,
                currency_code: request.currency,
            },
            parent_transaction: request.connector_transaction_id.clone(),
            shopper_interaction: ElavonPgShopperInteraction::Ecommerce,
            custom_reference: Some(request.refund_id.clone()),
        })
    }
}

impl TryFrom<ResponseRouterData<ElavonPgRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ElavonPgRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(build_refund_router_data(
            item.response,
            item.http_code,
            item.router_data,
        ))
    }
}

// =============================================================================
// RSYNC
// =============================================================================

impl TryFrom<ResponseRouterData<ElavonPgRsyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ElavonPgRsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(build_refund_router_data(
            item.response,
            item.http_code,
            item.router_data,
        ))
    }
}

/// Shared Refund/RSync response shaping — the refund transaction resource is the
/// same on create and on read, so one mapping serves both.
fn build_refund_router_data<F, Req>(
    response: ElavonPgTransactionResponse,
    http_code: u16,
    router_data: RouterDataV2<F, RefundFlowData, Req, RefundsResponseData>,
) -> RouterDataV2<F, RefundFlowData, Req, RefundsResponseData> {
    let refund_status = RefundStatus::from(ElavonPgChildStatus {
        state: &response.state,
        is_authorized: response.is_authorized,
    });

    let response_body = if refund_status == RefundStatus::Failure {
        Err(in_band_error(
            &response.failures,
            http_code,
            response.id.clone(),
            FlowStatus::Refund(refund_status),
            response.issuer_response_code.clone(),
            response
                .raw_processor_response_info
                .as_ref()
                .and_then(|info| info.processor_response_message.clone()),
        ))
    } else {
        Ok(RefundsResponseData {
            connector_refund_id: response.id.clone(),
            refund_status,
            status_code: http_code,
            acquirer_reference_number: response.processor_reference.clone(),
        })
    };

    RouterDataV2 {
        response: response_body,
        resource_common_data: RefundFlowData {
            status: refund_status,
            ..router_data.resource_common_data
        },
        ..router_data
    }
}
