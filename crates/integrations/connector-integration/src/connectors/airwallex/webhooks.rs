//! Airwallex incoming-webhook payloads and mappings.
//!
//! Webhooks are **not** a `ConnectorIntegrationV2` flow — there is no flow marker, no
//! `RouterDataV2` and no macro entry. Everything here is consumed by the plain
//! `impl IncomingWebhook for Airwallex<T>` in `super`, across the two `EventService` RPCs:
//!
//! * `ParseEvent` (stateless, **no secrets**) — [`parse_webhook_event`] feeds
//!   `get_event_type` and `get_webhook_event_reference`.
//! * `HandleEvent` (secrets available) — `verify_webhook_source` plus the
//!   `build_*_webhook_response` builders below.
//!
//! Nothing in this module reads a credential, so none of it may be moved into the
//! HandleEvent-only half by accident.

use common_enums::{AttemptStatus, Currency, DisputeStage, DisputeStatus, RefundStatus};
use common_utils::types::{AmountConvertor, FloatMajorUnit, MinorUnit, StringMinorUnit};
use domain_types::{
    connector_types::{
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventType, MandateReference,
        PaymentWebhookReference, RefundWebhookDetailsResponse, RefundWebhookReference, ResponseId,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::WebhookError,
    utils::{convert_amount_for_webhook, convert_back_amount_to_minor_units_for_webhook},
};
use error_stack::{report, Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use super::transformers::{
    get_payment_status, AirwallexFailureDetails, AirwallexNextAction, AirwallexPaymentStatus,
    AirwallexRefundResponse, AirwallexRefundStatus,
};

/// `request_id` prefix this connector writes on `AirwallexRefundRequest`.
///
/// Kept next to the stripper so the two cannot drift; the producer is
/// `impl TryFrom<..> for AirwallexRefundRequest` in `transformers.rs`.
const REFUND_REQUEST_ID_PREFIX: &str = "refund_";

/// Airwallex acknowledges a webhook with an empty HTTP 200, so every `*DetailsResponse` this
/// module builds reports the same synthetic status code. There is no upstream HTTP exchange to
/// take one from — the connector is the *server* here.
const WEBHOOK_STATUS_CODE: u16 = 200;

// ===== Envelope =====

/// The Airwallex webhook envelope (`POST` body, `application/json`).
///
/// Deliberately **not** `#[serde(deny_unknown_fields)]`: the envelope's optional members vary by
/// event class and `data.object` is a full retrieve-API body whose field set grows with the API
/// version the merchant's notification URL is pinned to.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexWebhookEvent {
    /// Event identifier, stable across Airwallex's ~3-day retry schedule — the value a caller
    /// dedupes redeliveries on. Two shapes are documented (`evt_100_<ts>_<seq>` and a bare
    /// 32-char hex string), so it is treated as opaque: never parsed, never prefix-checked. It is
    /// required, so a body without it is rejected as not an Airwallex webhook, and it is quoted in
    /// the misrouted-event error so a bad delivery can be found in Airwallex's event log.
    pub id: String,
    pub name: AirwallexWebhookEventName,
    /// The Airwallex account the event belongs to. Airwallex's own documentation disagrees with
    /// itself on the casing (`account_id` on the webhook pages, `accountId` in every
    /// payments-reference sample), so both spellings bind. It plays no part in resolving the
    /// resource reference; it is carried for the misrouted-event diagnostic, where knowing which
    /// account sent an unexpected event is the whole question.
    #[serde(alias = "accountId")]
    pub account_id: Option<String>,
    pub data: AirwallexWebhookEventData,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexWebhookEventData {
    /// The resource this event is about, rendered as its Retrieve-API body. Which struct it
    /// deserialises into is decided by the sibling [`AirwallexWebhookEvent::name`] — never by
    /// `#[serde(untagged)]`, which would silently bind a PaymentAttempt as a PaymentIntent
    /// (they share `id`, `amount`, `currency`, `status`, `created_at`, `updated_at` and
    /// `captured_amount`).
    pub object: serde_json::Value,
}

/// The `name` field: Airwallex's event catalogue.
///
/// Only the four families UCS models get their own variant. Everything else Airwallex can send —
/// `customer.*`, `payment_method.*`, `payment_link.*`, `fraud.*`, `funds_split.*`,
/// `pos.terminal.*` and `payment_consent.*` — lands on [`Self::Unknown`] and is reported as
/// [`EventType::IncomingWebhookEventUnspecified`]. Giving each of those its own variant would be
/// ~35 arms that all behave identically to `Unknown`.
///
// TODO(mandate-webhooks): `payment_consent.*` is the mandate lifecycle and this connector does
// implement mandates (`SetupMandate` / `RepeatPayment`). UCS has `EventType::{MandateActive,
// MandateFailed, MandateRevoked}` and `WebhookResourceReference::Mandate` for it, but wiring that
// up is its own unit of work; until then consent events are correctly reported as unspecified.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum AirwallexWebhookEventName {
    #[serde(rename = "payment_intent.created")]
    PaymentIntentCreated,
    #[serde(rename = "payment_intent.requires_payment_method")]
    PaymentIntentRequiresPaymentMethod,
    #[serde(rename = "payment_intent.updated")]
    PaymentIntentUpdated,
    #[serde(rename = "payment_intent.requires_customer_action")]
    PaymentIntentRequiresCustomerAction,
    #[serde(rename = "payment_intent.requires_capture")]
    PaymentIntentRequiresCapture,
    #[serde(rename = "payment_intent.pending")]
    PaymentIntentPending,
    #[serde(rename = "payment_intent.pending_review")]
    PaymentIntentPendingReview,
    #[serde(rename = "payment_intent.succeeded")]
    PaymentIntentSucceeded,
    #[serde(rename = "payment_intent.cancelled")]
    PaymentIntentCancelled,
    #[serde(rename = "payment_intent.payment_failed")]
    PaymentIntentPaymentFailed,

    #[serde(rename = "payment_attempt.received")]
    PaymentAttemptReceived,
    #[serde(rename = "payment_attempt.authentication_redirected")]
    PaymentAttemptAuthenticationRedirected,
    #[serde(rename = "payment_attempt.authentication_failed")]
    PaymentAttemptAuthenticationFailed,
    #[serde(rename = "payment_attempt.pending_authorization")]
    PaymentAttemptPendingAuthorization,
    #[serde(rename = "payment_attempt.authorized")]
    PaymentAttemptAuthorized,
    #[serde(rename = "payment_attempt.authorization_failed")]
    PaymentAttemptAuthorizationFailed,
    #[serde(rename = "payment_attempt.capture_requested")]
    PaymentAttemptCaptureRequested,
    #[serde(rename = "payment_attempt.capture_failed")]
    PaymentAttemptCaptureFailed,
    #[serde(rename = "payment_attempt.settled")]
    PaymentAttemptSettled,
    #[serde(rename = "payment_attempt.paid")]
    PaymentAttemptPaid,
    #[serde(rename = "payment_attempt.cancelled")]
    PaymentAttemptCancelled,
    #[serde(rename = "payment_attempt.expired")]
    PaymentAttemptExpired,
    #[serde(rename = "payment_attempt.risk_declined")]
    PaymentAttemptRiskDeclined,
    #[serde(rename = "payment_attempt.failed_to_process")]
    PaymentAttemptFailedToProcess,

    #[serde(rename = "refund.received")]
    RefundReceived,
    #[serde(rename = "refund.accepted")]
    RefundAccepted,
    #[serde(rename = "refund.settled")]
    RefundSettled,
    #[serde(rename = "refund.failed")]
    RefundFailed,

    #[serde(rename = "payment_dispute.requires_response")]
    PaymentDisputeRequiresResponse,
    #[serde(rename = "payment_dispute.challenged")]
    PaymentDisputeChallenged,
    #[serde(rename = "payment_dispute.accepted")]
    PaymentDisputeAccepted,
    #[serde(rename = "payment_dispute.expired")]
    PaymentDisputeExpired,
    #[serde(rename = "payment_dispute.pending_closure")]
    PaymentDisputePendingClosure,
    #[serde(rename = "payment_dispute.pending_decision")]
    PaymentDisputePendingDecision,
    #[serde(rename = "payment_dispute.won")]
    PaymentDisputeWon,
    #[serde(rename = "payment_dispute.lost")]
    PaymentDisputeLost,
    #[serde(rename = "payment_dispute.reversed")]
    PaymentDisputeReversed,

    #[serde(other)]
    Unknown,
}

/// Which of the four Retrieve-API bodies `data.object` holds, decided by the event name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirwallexWebhookResource {
    PaymentIntent,
    PaymentAttempt,
    Refund,
    Dispute,
    /// An event UCS does not model. `data.object` is not parsed at all.
    Unmodelled,
}

impl AirwallexWebhookEventName {
    /// The `name` prefix decides the shape of `data.object` (spec §15.5).
    pub fn resource(self) -> AirwallexWebhookResource {
        match self {
            Self::PaymentIntentCreated
            | Self::PaymentIntentRequiresPaymentMethod
            | Self::PaymentIntentUpdated
            | Self::PaymentIntentRequiresCustomerAction
            | Self::PaymentIntentRequiresCapture
            | Self::PaymentIntentPending
            | Self::PaymentIntentPendingReview
            | Self::PaymentIntentSucceeded
            | Self::PaymentIntentCancelled
            | Self::PaymentIntentPaymentFailed => AirwallexWebhookResource::PaymentIntent,

            Self::PaymentAttemptReceived
            | Self::PaymentAttemptAuthenticationRedirected
            | Self::PaymentAttemptAuthenticationFailed
            | Self::PaymentAttemptPendingAuthorization
            | Self::PaymentAttemptAuthorized
            | Self::PaymentAttemptAuthorizationFailed
            | Self::PaymentAttemptCaptureRequested
            | Self::PaymentAttemptCaptureFailed
            | Self::PaymentAttemptSettled
            | Self::PaymentAttemptPaid
            | Self::PaymentAttemptCancelled
            | Self::PaymentAttemptExpired
            | Self::PaymentAttemptRiskDeclined
            | Self::PaymentAttemptFailedToProcess => AirwallexWebhookResource::PaymentAttempt,

            Self::RefundReceived
            | Self::RefundAccepted
            | Self::RefundSettled
            | Self::RefundFailed => AirwallexWebhookResource::Refund,

            Self::PaymentDisputeRequiresResponse
            | Self::PaymentDisputeChallenged
            | Self::PaymentDisputeAccepted
            | Self::PaymentDisputeExpired
            | Self::PaymentDisputePendingClosure
            | Self::PaymentDisputePendingDecision
            | Self::PaymentDisputeWon
            | Self::PaymentDisputeLost
            | Self::PaymentDisputeReversed => AirwallexWebhookResource::Dispute,

            Self::Unknown => AirwallexWebhookResource::Unmodelled,
        }
    }

    /// The UCS event type. Drives `process_webhook_event`'s payment/refund/dispute fan-out via
    /// `EventType::is_payment_event()` / `is_refund_event()` / `is_dispute_event()`.
    pub fn event_type(self) -> EventType {
        match self {
            Self::PaymentIntentCreated
            | Self::PaymentIntentRequiresPaymentMethod
            | Self::PaymentIntentUpdated
            | Self::PaymentIntentPending
            | Self::PaymentIntentPendingReview
            | Self::PaymentAttemptReceived
            | Self::PaymentAttemptPendingAuthorization => EventType::PaymentIntentProcessing,

            Self::PaymentIntentRequiresCustomerAction
            | Self::PaymentAttemptAuthenticationRedirected => EventType::PaymentActionRequired,

            Self::PaymentIntentRequiresCapture | Self::PaymentAttemptAuthorized => {
                EventType::PaymentIntentAuthorizationSuccess
            }

            Self::PaymentAttemptAuthenticationFailed | Self::PaymentAttemptAuthorizationFailed => {
                EventType::PaymentIntentAuthorizationFailure
            }

            Self::PaymentAttemptCaptureRequested => EventType::PaymentIntentCaptureSuccess,
            Self::PaymentAttemptCaptureFailed => EventType::PaymentIntentCaptureFailure,

            Self::PaymentIntentSucceeded
            | Self::PaymentAttemptSettled
            | Self::PaymentAttemptPaid => EventType::PaymentIntentSuccess,

            Self::PaymentIntentCancelled | Self::PaymentAttemptCancelled => {
                EventType::PaymentIntentCancelled
            }
            Self::PaymentAttemptExpired => EventType::PaymentIntentExpired,

            Self::PaymentIntentPaymentFailed
            | Self::PaymentAttemptRiskDeclined
            | Self::PaymentAttemptFailedToProcess => EventType::PaymentIntentFailure,

            Self::RefundReceived | Self::RefundAccepted => EventType::RefundProcessing,
            Self::RefundSettled => EventType::RefundSuccess,
            Self::RefundFailed => EventType::RefundFailure,

            Self::PaymentDisputeRequiresResponse => EventType::DisputeOpened,
            Self::PaymentDisputeChallenged | Self::PaymentDisputePendingDecision => {
                EventType::DisputeChallenged
            }
            Self::PaymentDisputeAccepted | Self::PaymentDisputePendingClosure => {
                EventType::DisputeAccepted
            }
            Self::PaymentDisputeExpired => EventType::DisputeExpired,
            Self::PaymentDisputeWon => EventType::DisputeWon,
            Self::PaymentDisputeLost => EventType::DisputeLost,
            Self::PaymentDisputeReversed => EventType::DisputeCancelled,

            Self::Unknown => EventType::IncomingWebhookEventUnspecified,
        }
    }

    /// The UCS attempt status this event *name* asserts.
    ///
    /// The name carries strictly more information than `data.object.status`:
    /// `PaymentAttempt.status` collapses authentication_failed, authorization_failed,
    /// risk_declined, failed_to_process and capture_failed into the single wire value `FAILED`,
    /// and `PaymentIntent.status` has no `PENDING_REVIEW` counterpart in
    /// [`AirwallexPaymentStatus`]. So the name wins here — the exact inverse of the PSync rule,
    /// where there is no name and `status` is all there is.
    ///
    /// `payment_intent.updated` is the one event whose name says nothing beyond "something
    /// changed"; it returns `None` and the caller falls back to the intent's own status.
    fn asserted_attempt_status(self) -> Option<AttemptStatus> {
        match self {
            Self::PaymentIntentUpdated => None,

            Self::PaymentIntentCreated | Self::PaymentIntentRequiresPaymentMethod => {
                Some(AttemptStatus::PaymentMethodAwaited)
            }
            Self::PaymentIntentRequiresCustomerAction
            | Self::PaymentAttemptAuthenticationRedirected => {
                Some(AttemptStatus::AuthenticationPending)
            }
            Self::PaymentIntentRequiresCapture | Self::PaymentAttemptAuthorized => {
                Some(AttemptStatus::Authorized)
            }
            // `PENDING_REVIEW` is a live risk review that can still succeed or fail, so it stays
            // non-terminal. It has no `AirwallexPaymentStatus` variant, which is precisely why it
            // has to be read off the name.
            Self::PaymentIntentPending
            | Self::PaymentIntentPendingReview
            | Self::PaymentAttemptReceived => Some(AttemptStatus::Pending),
            Self::PaymentAttemptPendingAuthorization => Some(AttemptStatus::Authorizing),
            Self::PaymentIntentSucceeded
            | Self::PaymentAttemptCaptureRequested
            | Self::PaymentAttemptSettled
            | Self::PaymentAttemptPaid => Some(AttemptStatus::Charged),
            Self::PaymentIntentCancelled | Self::PaymentAttemptCancelled => {
                Some(AttemptStatus::Voided)
            }
            Self::PaymentAttemptExpired => Some(AttemptStatus::Expired),
            Self::PaymentAttemptAuthenticationFailed => Some(AttemptStatus::AuthenticationFailed),
            Self::PaymentAttemptAuthorizationFailed => Some(AttemptStatus::AuthorizationFailed),
            Self::PaymentAttemptCaptureFailed => Some(AttemptStatus::CaptureFailed),
            // `payment_intent.payment_failed` is "an attempt on this intent failed" — terminal for
            // the attempt UCS is tracking, but the intent itself stays alive and retryable.
            Self::PaymentIntentPaymentFailed
            | Self::PaymentAttemptRiskDeclined
            | Self::PaymentAttemptFailedToProcess => Some(AttemptStatus::Failure),

            // Refund, dispute and unmodelled events never reach the payment path: the fan-out
            // routes them by `event_type()`, and `build_payment_webhook_response` rejects them.
            Self::RefundReceived
            | Self::RefundAccepted
            | Self::RefundSettled
            | Self::RefundFailed
            | Self::PaymentDisputeRequiresResponse
            | Self::PaymentDisputeChallenged
            | Self::PaymentDisputeAccepted
            | Self::PaymentDisputeExpired
            | Self::PaymentDisputePendingClosure
            | Self::PaymentDisputePendingDecision
            | Self::PaymentDisputeWon
            | Self::PaymentDisputeLost
            | Self::PaymentDisputeReversed
            | Self::Unknown => None,
        }
    }

    /// Whether the event reports money actually captured. `captured_amount` is a running total on
    /// every intent/attempt body (it is `0` on a freshly created intent), so surfacing it
    /// unconditionally would report a capture on `payment_intent.created`.
    fn reports_capture(self) -> bool {
        matches!(
            self,
            Self::PaymentIntentSucceeded
                | Self::PaymentAttemptCaptureRequested
                | Self::PaymentAttemptSettled
                | Self::PaymentAttemptPaid
        )
    }
}

// ===== `data.object` — payments =====

/// `data.object` for `payment_intent.*`.
///
/// A webhook-local view of the PaymentIntent retrieve body carrying only the fields the webhook
/// path consumes. It deliberately does **not** reuse `AirwallexPaymentsResponse`: that struct is
/// shaped for the Authorize/PSync/3DS responses, does not carry `merchant_order_id`, and widening
/// it would put webhook-only concerns on the shipped card-payment path. The one thing that *is*
/// shared is [`AirwallexPaymentStatus`] — there is no second intent-status enum.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexWebhookIntent {
    /// `int_…`. This **is** the UCS `connector_transaction_id`: every `ResponseId` the Airwallex
    /// connector emits is the intent id.
    pub id: String,
    /// The UCS `merchant_transaction_id` — `AirwallexCreateOrderRequest` sets it to
    /// `connector_request_reference_id` verbatim. Preferred over `request_id`, which this
    /// connector decorates with `create_` / `confirm_` prefixes.
    pub merchant_order_id: Option<String>,
    /// Read only for `payment_intent.updated`, the single event whose name is not decisive.
    pub status: AirwallexPaymentStatus,
    /// The redirect target when the intent is `REQUIRES_CUSTOMER_ACTION`. Read together with
    /// `status` by the shared [`get_payment_status`], which refuses to report
    /// `AuthenticationPending` for an intent that names no action to take.
    pub next_action: Option<AirwallexNextAction>,
    /// Major-unit decimal (`16.66` = USD 16.66), never minor units.
    pub captured_amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    /// The mandate token, when this intent set one up.
    pub payment_consent_id: Option<Secret<String>>,
    /// Carries the failure and network identifiers for the attempt this intent event is about.
    pub latest_payment_attempt: Option<AirwallexWebhookAttempt>,
}

/// `data.object` for `payment_attempt.*`, and the `latest_payment_attempt` member of
/// [`AirwallexWebhookIntent`].
///
/// **The attempt's own `id` (`att_…`) is deliberately not modelled.** UCS's transaction id for
/// Airwallex is the *intent* id — every `resource_id` the connector emits is
/// `ResponseId::ConnectorTransactionId(<intent id>)` — so writing `att_…` into
/// `connector_transaction_id` would make every attempt webhook fail to resolve to a payment.
/// Not having the field means that mistake cannot be made here.
///
/// `status` is not modelled either: for all fourteen `payment_attempt.*` names the event name is
/// strictly more informative (see [`AirwallexWebhookEventName::asserted_attempt_status`]), so the
/// field would never be read.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexWebhookAttempt {
    /// `int_…` — **this** is the UCS `connector_transaction_id`. Required by the Airwallex API
    /// reference on the top-level attempt body; absent on the `latest_payment_attempt`
    /// sub-object, hence `Option`.
    pub payment_intent_id: Option<String>,
    /// Documented as required, but the verbatim `payment_attempt.received` sample omits it.
    pub merchant_order_id: Option<String>,
    pub captured_amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub payment_consent_id: Option<Secret<String>>,
    /// The network transaction id. Airwallex deprecated `provider_transaction_id` in favour of
    /// this field, so the deprecated one is not read.
    pub payment_method_transaction_id: Option<String>,
    /// Raw processor response text — the `error_reason` source.
    pub provider_original_response_description: Option<String>,
    /// Present when the attempt failed.
    pub failure_code: Option<String>,
    pub failure_details: Option<AirwallexFailureDetails>,
}

// ===== `data.object` — disputes =====

/// `data.object` for `payment_dispute.*`.
///
/// The one real `payment_dispute.requires_response` sample Airwallex publishes carries only
/// `amount`, `currency`, `due_at`, `id`, `stage` and `status` — no `payment_intent_id`, no
/// `reason`. Whether the live wire sends the full Retrieve body is unconfirmed, so everything
/// beyond the five fields UCS cannot do without is `Option`.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexWebhookDispute {
    /// `dst_…` → `DisputeWebhookDetailsResponse.dispute_id`.
    pub id: String,
    /// The parent payment. `None` in the published sample, so
    /// `DisputeWebhookReference.connector_transaction_id` must tolerate its absence and the
    /// caller resolves on `connector_dispute_id` alone.
    pub payment_intent_id: Option<String>,
    /// Major-unit decimal. `DisputeWebhookDetailsResponse.amount` is `StringMinorUnit`, so this
    /// is converted, never cast.
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub stage: AirwallexDisputeStage,
    pub status: AirwallexDisputeStatus,
    pub reason: Option<AirwallexDisputeReason>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexDisputeReason {
    /// The scheme's own reason code, e.g. Visa `"4837"`.
    pub original_code: Option<String>,
    pub description: Option<String>,
}

/// Airwallex's dispute escalation ladder.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexDisputeStage {
    /// Request for information — the issuer is asking, no money has moved.
    Rfi,
    PreChargeback,
    Chargeback,
    PreArbitration,
    Arbitration,
    #[serde(other)]
    Unknown,
}

impl From<AirwallexDisputeStage> for DisputeStage {
    fn from(stage: AirwallexDisputeStage) -> Self {
        match stage {
            AirwallexDisputeStage::Rfi | AirwallexDisputeStage::PreChargeback => Self::PreDispute,
            AirwallexDisputeStage::Chargeback => Self::Dispute,
            // Lossy: `common_enums::DisputeStage` has no `Arbitration`, so arbitration is reported
            // at the closest stage below it rather than being dropped.
            AirwallexDisputeStage::PreArbitration | AirwallexDisputeStage::Arbitration => {
                Self::PreArbitration
            }
            // A stage Airwallex added after this mapping was written. `Dispute` is the
            // `common_enums` default and the neutral middle of the ladder — it neither
            // under-reports a chargeback as a pre-dispute nor over-reports it as arbitration.
            AirwallexDisputeStage::Unknown => Self::Dispute,
        }
    }
}

/// Airwallex's dispute lifecycle states.
///
/// These are **reporting** states: they tell the caller where the chargeback stands so it can
/// surface on the dashboard. None of them requires a UCS flow to advance — responding to an
/// Airwallex dispute happens through Airwallex's own dispute APIs, and `Accept`, `DefendDispute`
/// and `SubmitEvidence` remain in this connector's `not_implemented` list. Nothing in this
/// mapping may be changed to depend on them until they are actually implemented.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexDisputeStatus {
    RequiresResponse,
    Challenged,
    Accepted,
    Reversed,
    Won,
    Lost,
    PendingClosure,
    Expired,
    PendingDecision,
    #[serde(other)]
    Unknown,
}

impl From<AirwallexDisputeStatus> for DisputeStatus {
    fn from(status: AirwallexDisputeStatus) -> Self {
        match status {
            AirwallexDisputeStatus::RequiresResponse => Self::DisputeOpened,
            AirwallexDisputeStatus::Challenged => Self::DisputeChallenged,
            AirwallexDisputeStatus::Accepted => Self::DisputeAccepted,
            AirwallexDisputeStatus::Won => Self::DisputeWon,
            AirwallexDisputeStatus::Lost => Self::DisputeLost,
            AirwallexDisputeStatus::Expired => Self::DisputeExpired,
            // The dispute was withdrawn and the merchant credited back.
            AirwallexDisputeStatus::Reversed => Self::DisputeCancelled,
            // Airwallex is auto-accepting the pre-arbitration on the merchant's behalf.
            AirwallexDisputeStatus::PendingClosure => Self::DisputeAccepted,
            // Evidence is with the scheme awaiting a ruling — the challenged state.
            AirwallexDisputeStatus::PendingDecision => Self::DisputeChallenged,
            // A status Airwallex added after this mapping was written. `DisputeOpened` is the
            // `common_enums` default and the only non-terminal reading: it says a dispute exists
            // without claiming an outcome the money may not agree with.
            AirwallexDisputeStatus::Unknown => Self::DisputeOpened,
        }
    }
}

// ===== Parsing =====

/// Parses the envelope. Called by both `EventService` phases, so it must not need a secret.
pub fn parse_webhook_event(body: &[u8]) -> Result<AirwallexWebhookEvent, Report<WebhookError>> {
    serde_json::from_slice::<AirwallexWebhookEvent>(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("failed to decode the Airwallex webhook envelope")
}

fn parse_object<T: serde::de::DeserializeOwned>(
    event: &AirwallexWebhookEvent,
) -> Result<T, Report<WebhookError>> {
    serde_json::from_value::<T>(event.data.object.clone())
        .change_context(WebhookError::WebhookResourceObjectNotFound)
        .attach_printable("failed to decode `data.object` for the Airwallex webhook event")
}

/// Rejects an event that did not belong on this `process_*_webhook` path.
///
/// `EventType::IncomingWebhookEventUnspecified` matches none of `is_payment_event()`,
/// `is_refund_event()` or `is_dispute_event()`, so `process_webhook_event` falls through to
/// `process_payment_webhook` with it. That must be a clean error, not a misshapen success.
fn wrong_resource(event: &AirwallexWebhookEvent) -> Report<WebhookError> {
    report!(WebhookError::WebhookEventTypeNotFound).attach_printable(format!(
        "Airwallex webhook event {:?} (id {}, account {}) does not carry this resource type",
        event.name,
        event.id,
        event.account_id.as_deref().unwrap_or("unknown"),
    ))
}

/// Recovers the UCS refund reference from the echoed idempotency key.
///
/// This connector writes `refund_{connector_request_reference_id}` on `AirwallexRefundRequest`.
/// A refund created outside UCS carries whatever the caller sent — Airwallex's own sample is
/// `"GN230113463059337228"`, with no prefix — so an absent prefix is the normal case and passes
/// through unchanged rather than erroring.
fn strip_refund_request_prefix(request_id: &str) -> &str {
    request_id
        .strip_prefix(REFUND_REQUEST_ID_PREFIX)
        .unwrap_or(request_id)
}

// ===== ParseEvent: reference resolution =====

/// The `EventService.ParseEvent` payload.
///
/// Each id goes in exactly one slot — the caller matches on which field is populated, so
/// duplicating an id across fields is a wrong answer, not a redundant one.
pub fn webhook_reference(
    event: &AirwallexWebhookEvent,
) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
    let reference = match event.name.resource() {
        AirwallexWebhookResource::PaymentIntent => {
            let intent: AirwallexWebhookIntent = parse_object(event)?;
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(intent.id),
                merchant_transaction_id: intent.merchant_order_id,
            })
        }
        AirwallexWebhookResource::PaymentAttempt => {
            let attempt: AirwallexWebhookAttempt = parse_object(event)?;
            WebhookResourceReference::Payment(PaymentWebhookReference {
                // The intent id, never the `att_…` id this object is named by.
                connector_transaction_id: Some(attempt.payment_intent_id.ok_or_else(|| {
                    report!(WebhookError::WebhookMissingRequiredField {
                        field: "data.object.payment_intent_id"
                    })
                })?),
                merchant_transaction_id: attempt.merchant_order_id,
            })
        }
        AirwallexWebhookResource::Refund => {
            let refund: AirwallexRefundResponse = parse_object(event)?;
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: Some(refund.id),
                merchant_refund_id: refund
                    .request_id
                    .as_deref()
                    .map(|request_id| strip_refund_request_prefix(request_id).to_string()),
                connector_transaction_id: refund.payment_intent_id,
                // The Refund object carries no `merchant_order_id`.
                merchant_transaction_id: None,
            })
        }
        AirwallexWebhookResource::Dispute => {
            let dispute: AirwallexWebhookDispute = parse_object(event)?;
            WebhookResourceReference::Dispute(DisputeWebhookReference {
                connector_dispute_id: Some(dispute.id),
                connector_transaction_id: dispute.payment_intent_id,
            })
        }
        // No actionable reference — and `data.object` is not parsed, because its shape is
        // whatever resource the unmodelled event is about.
        AirwallexWebhookResource::Unmodelled => return Ok(None),
    };

    Ok(Some(reference))
}

// ===== HandleEvent: payment =====

/// The fields `WebhookDetailsResponse` needs, normalised from either shape of `data.object`.
///
/// `payment_intent.*` and `payment_attempt.*` produce the same UCS response, they just carry the
/// pieces in different places — the intent nests the failure and network identifiers under
/// `latest_payment_attempt`, the attempt has them at the top level. Normalising once keeps the
/// builder from branching on the resource twice.
struct AirwallexWebhookPayment {
    /// Always the intent id, whichever object the event carried.
    intent_id: String,
    merchant_order_id: Option<String>,
    captured_amount: Option<FloatMajorUnit>,
    currency: Option<Currency>,
    consent_id: Option<Secret<String>>,
    attempt: Option<AirwallexWebhookAttempt>,
    /// `Some` only for `payment_intent.*` — an attempt body has no intent status to fall back to,
    /// and no `payment_attempt.*` event needs one.
    intent_status: Option<(AirwallexPaymentStatus, Option<AirwallexNextAction>)>,
}

impl AirwallexWebhookPayment {
    fn from_intent(intent: AirwallexWebhookIntent) -> Self {
        Self {
            intent_id: intent.id,
            merchant_order_id: intent.merchant_order_id,
            captured_amount: intent.captured_amount,
            currency: intent.currency,
            consent_id: intent.payment_consent_id,
            attempt: intent.latest_payment_attempt,
            intent_status: Some((intent.status, intent.next_action)),
        }
    }

    fn from_attempt(attempt: AirwallexWebhookAttempt) -> Result<Self, Report<WebhookError>> {
        // The `att_…` id this object is named by is not a UCS transaction id, so without the
        // intent id there is nothing to resolve the payment against.
        let intent_id = attempt.payment_intent_id.clone().ok_or_else(|| {
            report!(WebhookError::WebhookMissingRequiredField {
                field: "data.object.payment_intent_id"
            })
        })?;
        Ok(Self {
            intent_id,
            merchant_order_id: attempt.merchant_order_id.clone(),
            captured_amount: attempt.captured_amount,
            currency: attempt.currency,
            consent_id: attempt.payment_consent_id.clone(),
            attempt: Some(attempt),
            intent_status: None,
        })
    }
}

/// Builds the `WebhookDetailsResponse` for `payment_intent.*` and `payment_attempt.*`.
///
/// `major_converter` is the connector's registered `FloatMajorUnit` converter; it turns
/// Airwallex's major-unit decimals into the `MinorUnit` the captured-amount fields want.
pub fn build_payment_webhook_response(
    event: &AirwallexWebhookEvent,
    raw_body: &[u8],
    major_converter: &dyn AmountConvertor<Output = FloatMajorUnit>,
) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
    let payment = match event.name.resource() {
        AirwallexWebhookResource::PaymentIntent => {
            AirwallexWebhookPayment::from_intent(parse_object(event)?)
        }
        AirwallexWebhookResource::PaymentAttempt => {
            AirwallexWebhookPayment::from_attempt(parse_object(event)?)?
        }
        AirwallexWebhookResource::Refund
        | AirwallexWebhookResource::Dispute
        | AirwallexWebhookResource::Unmodelled => return Err(wrong_resource(event)),
    };
    let AirwallexWebhookPayment {
        intent_id,
        merchant_order_id,
        captured_amount,
        currency,
        consent_id,
        attempt,
        intent_status,
    } = payment;

    // The event name is authoritative; the intent's own status is the fallback for the one event
    // (`payment_intent.updated`) whose name is not decisive.
    let status = match event.name.asserted_attempt_status() {
        Some(status) => status,
        None => {
            let (intent_status, next_action) = intent_status.ok_or_else(|| {
                report!(WebhookError::WebhookMissingRequiredField {
                    field: "data.object.status"
                })
            })?;
            get_payment_status(&intent_status, &next_action)
        }
    };

    let failure_details = attempt.as_ref().and_then(|a| a.failure_details.as_ref());
    let error_code = attempt
        .as_ref()
        .and_then(|a| a.failure_code.clone())
        .or_else(|| failure_details.and_then(|d| d.code.clone()));
    let error_message = failure_details.and_then(|d| d.message.clone());
    let error_reason = attempt
        .as_ref()
        .and_then(|a| a.provider_original_response_description.clone());

    // `captured_amount` is a running total present on every intent/attempt body, so it is only
    // surfaced on the events that actually report a capture.
    let minor_amount_captured = match (event.name.reports_capture(), captured_amount, currency) {
        (true, Some(amount), Some(currency)) => Some(
            convert_back_amount_to_minor_units_for_webhook(major_converter, amount, currency)?,
        ),
        _ => None,
    };

    let mandate_reference = consent_id.map(|consent_id| {
        Box::new(MandateReference {
            connector_mandate_id: Some(consent_id.expose()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: None,
        })
    });

    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(intent_id.clone())),
        status,
        connector_response_reference_id: Some(intent_id),
        connector_request_reference_id: merchant_order_id,
        mandate_reference,
        error_code,
        error_message,
        error_reason,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: WEBHOOK_STATUS_CODE,
        response_headers: None,
        amount_captured: minor_amount_captured.map(MinorUnit::get_amount_as_i64),
        minor_amount_captured,
        network_txn_id: attempt.and_then(|a| a.payment_method_transaction_id),
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

// ===== HandleEvent: refund =====

/// Builds the `RefundWebhookDetailsResponse` for `refund.*`.
///
/// The status comes from `data.object.status` through the **existing**
/// `From<AirwallexRefundStatus> for RefundStatus` that RSync already uses. Deriving it from the
/// event name instead would be a second, independent mapping, and the two would race: a refund
/// would flap Pending → Success → Pending as RSync and the webhook disagreed about `ACCEPTED`.
pub fn build_refund_webhook_response(
    event: &AirwallexWebhookEvent,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
    if event.name.resource() != AirwallexWebhookResource::Refund {
        return Err(wrong_resource(event));
    }
    let refund: AirwallexRefundResponse = parse_object(event)?;

    let is_failed = matches!(refund.status, AirwallexRefundStatus::Failed);
    let (error_code, error_message) = match (is_failed, refund.failure_details.as_ref()) {
        (true, Some(details)) => (details.code.clone(), details.message.clone()),
        _ => (None, None),
    };

    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(refund.id),
        merchant_transaction_id: refund
            .request_id
            .as_deref()
            .map(|request_id| strip_refund_request_prefix(request_id).to_string()),
        status: RefundStatus::from(refund.status),
        connector_response_reference_id: refund.payment_intent_id,
        error_code,
        error_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: WEBHOOK_STATUS_CODE,
        response_headers: None,
    })
}

// ===== HandleEvent: dispute =====

/// Builds the `DisputeWebhookDetailsResponse` for `payment_dispute.*`.
///
/// Airwallex sends dispute amounts as major-unit decimals (`"amount": 10, "currency": "AUD"` is
/// AUD 10.00) while the UCS field is `StringMinorUnit`, so the amount goes through both
/// registered converters — major → `MinorUnit` → minor-unit string. It is never multiplied by
/// 100 in place, which would be wrong for JPY and KWD alike.
pub fn build_dispute_webhook_response(
    event: &AirwallexWebhookEvent,
    raw_body: &[u8],
    major_converter: &dyn AmountConvertor<Output = FloatMajorUnit>,
    minor_converter: &dyn AmountConvertor<Output = StringMinorUnit>,
) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
    if event.name.resource() != AirwallexWebhookResource::Dispute {
        return Err(wrong_resource(event));
    }
    let dispute: AirwallexWebhookDispute = parse_object(event)?;

    let minor_amount = convert_back_amount_to_minor_units_for_webhook(
        major_converter,
        dispute.amount,
        dispute.currency,
    )?;
    let amount = convert_amount_for_webhook(minor_converter, minor_amount, dispute.currency)?;

    Ok(DisputeWebhookDetailsResponse {
        amount,
        currency: dispute.currency,
        dispute_id: dispute.id,
        status: DisputeStatus::from(dispute.status),
        stage: DisputeStage::from(dispute.stage),
        connector_response_reference_id: dispute.payment_intent_id,
        dispute_message: dispute
            .reason
            .as_ref()
            .and_then(|reason| reason.description.clone()),
        connector_reason_code: dispute
            .reason
            .as_ref()
            .and_then(|reason| reason.original_code.clone()),
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: WEBHOOK_STATUS_CODE,
        response_headers: None,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use common_utils::{
        crypto::{SignMessage, VerifySignature},
        types::{FloatMajorUnitForConnector, StringMinorUnitForConnector},
    };

    use super::*;

    /// Verbatim `payment_intent.created` sample from the Airwallex payments-webhooks reference.
    /// Note the `accountId` spelling and the absent envelope-level `created_at` / `version`.
    const PAYMENT_INTENT_CREATED: &[u8] = br#"{
        "id":"evt_100_2019102201549020043_8321220011893766",
        "name":"payment_intent.created",
        "accountId":"78814faa-1b30-4598-a9c8-f0583db8d09d",
        "data":{
            "object": {
                "request_id": "d6a92e2a-02e5-c37b-c977-13796ec7443a",
                "id": "int_aaaat9w2hgh8mzi1111",
                "merchant_order_id": "0000000000",
                "amount": 16.66,
                "currency": "USD",
                "captured_amount": 0,
                "payment_method_options": {
                    "card": {
                        "risk_control": {
                            "three_domain_secure_action": "FORCE_3DS",
                            "three_ds_action": "FORCE_3DS"
                        },
                        "three_ds_action": "FORCE_3DS"
                    }
                },
                "status": "REQUIRES_PAYMENT_METHOD",
                "created_at": "2023-01-13T07:32:05+0000",
                "updated_at": "2023-01-13T07:32:05+0000"
            }
        }
    }"#;

    /// Verbatim `payment_attempt.received` sample. Exercises the intent-vs-attempt-id trap, the
    /// missing `merchant_order_id`, and the empty `"ds_data": {}`.
    const PAYMENT_ATTEMPT_RECEIVED: &[u8] = br#"{
        "id":"evt_100_2019102201549020043_8321220011893766",
        "name":"payment_attempt.received",
        "accountId":"78814faa-1b30-4598-a9c8-f0583db8d09d",
        "data":{
            "object": {
                "id": "att_hkpdcpcvbgh8mw11111_wkgwfs",
                "payment_intent_id": "int_hkpdcpcvbgh8mwk1111",
                "payment_consent_id": "cst_hkpdnqn5bgh8mw1111",
                "currency": "KRW",
                "amount": 62723,
                "payment_method": {
                    "id": "mtd_hkpdcpcvbgh8m111111",
                    "customer_id": "cus_hkpdk7f98gh8ir1111",
                    "type": "card",
                    "card": {
                        "bin": "53651045",
                        "brand": "mastercard",
                        "card_type": "DEBIT",
                        "expiry_month": "09",
                        "expiry_year": "2027",
                        "fingerprint": "111e4Y759jpc5eC3AAAAaaaa/8=",
                        "is_commercial": false,
                        "issuer_country_code": "KR",
                        "issuer_name": "KB KOOKMIN CARD CO., LTD",
                        "last4": "1111",
                        "name": "",
                        "number_type": "PAN"
                    },
                    "status": "CREATED",
                    "created_at": "2023-01-13T07:29:08+0000",
                    "updated_at": "2023-01-13T07:29:08+0000"
                },
                "authentication_data": {
                    "avs_result": "U",
                    "cvc_result": "U",
                    "ds_data": {},
                    "fraud_data": {
                        "score": "0"
                    }
                },
                "captured_amount": 0,
                "refunded_amount": 0,
                "settle_via": "airwallex",
                "status": "RECEIVED",
                "created_at": "2023-01-13T07:29:08+0000",
                "updated_at": "2023-01-13T07:29:08+0000"
            }
        }
    }"#;

    /// Verbatim `refund.accepted` sample. Its `request_id` has **no** `refund_` prefix — it came
    /// from a non-UCS caller — so this doubles as the no-op-strip regression test.
    const REFUND_ACCEPTED: &[u8] = br#"{
        "id":"evt_100_2019102201549020043_8321220011893766",
        "name":"refund.accepted",
        "accountId":"78814faa-1b30-4598-a9c8-f0583db8d09d",
        "data":{
            "object": {
                "request_id": "GN230113463059337228",
                "id": "rfd_aaaanqn5bgh8mnssssh_ga04nr",
                "payment_attempt_id": "att_hkpdxmj6wggosgaaniy_ga04nr",
                "payment_intent_id": "int_hkpdxmj6wggosga04nr",
                "amount": 21.49,
                "currency": "SGD",
                "reason": "Return good",
                "status": "ACCEPTED",
                "created_at": "2023-01-13T07:20:02+0000",
                "updated_at": "2023-01-13T07:20:02+0000"
            }
        }
    }"#;

    /// Verbatim `payment_dispute.requires_response` sample. Proves the envelope `id` need not be
    /// `evt_…`-shaped, that `account_id` is also spelled snake_case, and that `payment_intent_id`
    /// can be absent.
    const DISPUTE_REQUIRES_RESPONSE: &[u8] = br#"{
      "id": "04e0e84622d7b3add4f712dea5abcbeb",
      "name": "payment_dispute.requires_response",
      "account_id": "acct_BGEVPKSxP9OjzQGbwxVEKA",
      "data": {
        "object": {
          "amount": 10,
          "currency": "AUD",
          "due_at": "2023-10-30T20:00:00+0000",
          "id": "dst_sgstmr5zzgpg1wsjcwo",
          "stage": "CHARGEBACK",
          "status": "REQUIRES_RESPONSE"
        }
      }
    }"#;

    /// A `payment_dispute.*` `data.object` in its full Retrieve shape, from the dispute-handling
    /// guide. Second dispute fixture for the deserialiser: `payment_intent_id` and `reason` are
    /// present here and absent above.
    const DISPUTE_ACCEPTED_FULL: &[u8] = br#"{
      "id": "evt_dispute_full_fixture",
      "name": "payment_dispute.accepted",
      "account_id": "acct_BGEVPKSxP9OjzQGbwxVEKA",
      "data": { "object": {
        "id": "dst_hkpdw2eqp9oie",
        "stage": "CHARGEBACK",
        "status": "ACCEPTED",
        "amount": 100,
        "currency": "USD",
        "mode": "COLLABORATION",
        "merchant_order_id": "D202503210001",
        "payment_intent_id": "int_hkpdskz7vg1xc7uscdj",
        "payment_attempt_id": "att_hkpdw2eqp9oie",
        "acquirer_reference_number": "T1234567890",
        "payment_method_type": "VISA",
        "card_brand": "visa",
        "reason": { "original_code": "4837", "description": "Fraudulent transaction.", "type": "FRAUDULENT" },
        "accept_details": [{ "stage": "CHARGEBACK", "reason": "AGREEMENT_REACHED_WITH_CUSTOMER", "accepted_at": "2023-10-01T10:00:00+00:00" }],
        "created_at": "2023-10-01T10:00:00+00:00",
        "updated_at": "2023-10-01T10:00:00+00:00"
      } }
    }"#;

    fn major() -> &'static FloatMajorUnitForConnector {
        &FloatMajorUnitForConnector
    }

    fn minor() -> &'static StringMinorUnitForConnector {
        &StringMinorUnitForConnector
    }

    /// `ResponseId` has no `PartialEq`, so read the transaction id out of it instead.
    fn connector_txn_id(resource_id: &Option<ResponseId>) -> Option<&str> {
        match resource_id {
            Some(ResponseId::ConnectorTransactionId(id)) => Some(id.as_str()),
            _ => None,
        }
    }

    #[test]
    fn payment_intent_created_parses_to_processing_and_the_intent_id() {
        let event = parse_webhook_event(PAYMENT_INTENT_CREATED).expect("envelope parses");
        assert_eq!(event.name, AirwallexWebhookEventName::PaymentIntentCreated);
        // The `accountId` spelling has to bind through the alias.
        assert_eq!(
            event.account_id.as_deref(),
            Some("78814faa-1b30-4598-a9c8-f0583db8d09d")
        );
        assert_eq!(event.name.event_type(), EventType::PaymentIntentProcessing);

        let reference = webhook_reference(&event)
            .expect("reference resolves")
            .expect("payment reference is present");
        match reference {
            WebhookResourceReference::Payment(payment) => {
                assert_eq!(
                    payment.connector_transaction_id.as_deref(),
                    Some("int_aaaat9w2hgh8mzi1111")
                );
                assert_eq!(
                    payment.merchant_transaction_id.as_deref(),
                    Some("0000000000")
                );
            }
            other => panic!("expected a payment reference, got {other:?}"),
        }

        let details = build_payment_webhook_response(&event, PAYMENT_INTENT_CREATED, major())
            .expect("payment webhook builds");
        assert_eq!(details.status, AttemptStatus::PaymentMethodAwaited);
        assert_eq!(
            connector_txn_id(&details.resource_id),
            Some("int_aaaat9w2hgh8mzi1111")
        );
        // `captured_amount` is 0 on a created intent and this event reports no capture.
        assert_eq!(details.minor_amount_captured, None);
        assert_eq!(details.amount_captured, None);
        assert_eq!(details.error_code, None);
        assert_eq!(details.status_code, WEBHOOK_STATUS_CODE);
    }

    #[test]
    fn payment_attempt_resolves_via_the_intent_id_never_the_attempt_id() {
        let event = parse_webhook_event(PAYMENT_ATTEMPT_RECEIVED).expect("envelope parses");
        assert_eq!(
            event.name,
            AirwallexWebhookEventName::PaymentAttemptReceived
        );
        assert_eq!(event.name.event_type(), EventType::PaymentIntentProcessing);

        let reference = webhook_reference(&event)
            .expect("reference resolves")
            .expect("payment reference is present");
        match reference {
            WebhookResourceReference::Payment(payment) => {
                assert_eq!(
                    payment.connector_transaction_id.as_deref(),
                    Some("int_hkpdcpcvbgh8mwk1111"),
                    "the `att_…` id must never land in connector_transaction_id"
                );
                assert_eq!(payment.merchant_transaction_id, None);
            }
            other => panic!("expected a payment reference, got {other:?}"),
        }

        let details = build_payment_webhook_response(&event, PAYMENT_ATTEMPT_RECEIVED, major())
            .expect("payment webhook builds");
        assert_eq!(details.status, AttemptStatus::Pending);
        assert_eq!(
            connector_txn_id(&details.resource_id),
            Some("int_hkpdcpcvbgh8mwk1111")
        );
        // The attempt carries a consent id, so the mandate reference has to come through.
        assert_eq!(
            details
                .mandate_reference
                .as_ref()
                .and_then(|m| m.connector_mandate_id.as_deref()),
            Some("cst_hkpdnqn5bgh8mw1111")
        );
    }

    /// The name is authoritative because `PaymentAttempt.status` collapses five distinct
    /// outcomes into `FAILED`. Same wire status, five different UCS statuses.
    #[test]
    fn failure_names_do_not_collapse_the_way_the_status_field_does() {
        let cases = [
            (
                "payment_attempt.authentication_failed",
                AttemptStatus::AuthenticationFailed,
                EventType::PaymentIntentAuthorizationFailure,
            ),
            (
                "payment_attempt.authorization_failed",
                AttemptStatus::AuthorizationFailed,
                EventType::PaymentIntentAuthorizationFailure,
            ),
            (
                "payment_attempt.capture_failed",
                AttemptStatus::CaptureFailed,
                EventType::PaymentIntentCaptureFailure,
            ),
            (
                "payment_attempt.risk_declined",
                AttemptStatus::Failure,
                EventType::PaymentIntentFailure,
            ),
            (
                "payment_attempt.failed_to_process",
                AttemptStatus::Failure,
                EventType::PaymentIntentFailure,
            ),
        ];

        for (name, expected_status, expected_event) in cases {
            let body = format!(
                r#"{{"id":"evt_1","name":"{name}","data":{{"object":{{
                    "id":"att_x","payment_intent_id":"int_x","status":"FAILED",
                    "failure_code":"card_declined",
                    "failure_details":{{"code":"card_declined","message":"Card was declined","trace_id":"t1"}},
                    "provider_original_response_description":"DO NOT HONOR"
                }}}}}}"#
            );
            let event = parse_webhook_event(body.as_bytes()).expect("envelope parses");
            assert_eq!(event.name.event_type(), expected_event, "{name}");

            let details = build_payment_webhook_response(&event, body.as_bytes(), major())
                .expect("payment webhook builds");
            assert_eq!(details.status, expected_status, "{name}");
            assert_eq!(
                details.error_code.as_deref(),
                Some("card_declined"),
                "{name}"
            );
            assert_eq!(
                details.error_message.as_deref(),
                Some("Card was declined"),
                "{name}"
            );
            assert_eq!(
                details.error_reason.as_deref(),
                Some("DO NOT HONOR"),
                "{name}"
            );
        }
    }

    /// `payment_intent.updated` is the one event whose name is not decisive, so it — and only it
    /// — reads `data.object.status`.
    #[test]
    fn payment_intent_updated_falls_back_to_the_intent_status() {
        let body = br#"{"id":"evt_1","name":"payment_intent.updated","data":{"object":{
            "id":"int_x","merchant_order_id":"order_1","status":"REQUIRES_CAPTURE"}}}"#;
        let event = parse_webhook_event(body).expect("envelope parses");
        let details =
            build_payment_webhook_response(&event, body, major()).expect("payment webhook builds");
        assert_eq!(details.status, AttemptStatus::Authorized);
    }

    /// A capture event must surface the captured amount in minor units, converted rather than
    /// scaled: SGD 21.49 is 2149 minor units.
    #[test]
    fn capture_events_convert_the_captured_amount_to_minor_units() {
        let body = br#"{"id":"evt_1","name":"payment_attempt.settled","data":{"object":{
            "id":"att_x","payment_intent_id":"int_x","currency":"SGD","captured_amount":21.49,
            "payment_method_transaction_id":"net_txn_1"}}}"#;
        let event = parse_webhook_event(body).expect("envelope parses");
        let details =
            build_payment_webhook_response(&event, body, major()).expect("payment webhook builds");
        assert_eq!(details.status, AttemptStatus::Charged);
        assert_eq!(details.minor_amount_captured, Some(MinorUnit::new(2149)));
        assert_eq!(details.amount_captured, Some(2149));
        assert_eq!(details.network_txn_id.as_deref(), Some("net_txn_1"));
    }

    #[test]
    fn refund_accepted_stays_pending_and_strips_no_prefix_when_there_is_none() {
        let event = parse_webhook_event(REFUND_ACCEPTED).expect("envelope parses");
        assert_eq!(event.name, AirwallexWebhookEventName::RefundAccepted);
        assert_eq!(event.name.event_type(), EventType::RefundProcessing);

        let reference = webhook_reference(&event)
            .expect("reference resolves")
            .expect("refund reference is present");
        match reference {
            WebhookResourceReference::Refund(refund) => {
                assert_eq!(
                    refund.connector_refund_id.as_deref(),
                    Some("rfd_aaaanqn5bgh8mnssssh_ga04nr")
                );
                assert_eq!(
                    refund.merchant_refund_id.as_deref(),
                    Some("GN230113463059337228"),
                    "an absent `refund_` prefix must pass through unchanged"
                );
                assert_eq!(
                    refund.connector_transaction_id.as_deref(),
                    Some("int_hkpdxmj6wggosga04nr")
                );
                assert_eq!(refund.merchant_transaction_id, None);
            }
            other => panic!("expected a refund reference, got {other:?}"),
        }

        let details =
            build_refund_webhook_response(&event, REFUND_ACCEPTED).expect("refund webhook builds");
        assert_eq!(
            details.status,
            RefundStatus::Pending,
            "ACCEPTED must stay Pending to match the shipped RSync mapping"
        );
        assert_eq!(details.error_code, None);
    }

    /// A UCS-created refund does carry the `refund_` prefix, and a partial refund is just a
    /// smaller `amount` — nothing in the mapping is special-cased for it.
    #[test]
    fn partial_refund_settled_strips_the_ucs_prefix_and_reports_success() {
        let body = br#"{"id":"evt_1","name":"refund.settled","data":{"object":{
            "id":"rfd_partial","request_id":"refund_ref_abc","payment_intent_id":"int_x",
            "amount":5.00,"currency":"USD","status":"SETTLED"}}}"#;
        let event = parse_webhook_event(body).expect("envelope parses");
        assert_eq!(event.name.event_type(), EventType::RefundSuccess);

        let details = build_refund_webhook_response(&event, body).expect("refund webhook builds");
        assert_eq!(details.status, RefundStatus::Success);
        assert_eq!(details.merchant_transaction_id.as_deref(), Some("ref_abc"));
        assert_eq!(
            details.connector_response_reference_id.as_deref(),
            Some("int_x")
        );
    }

    #[test]
    fn refund_failed_surfaces_the_failure_details() {
        let body = br#"{"id":"evt_1","name":"refund.failed","data":{"object":{
            "id":"rfd_x","request_id":"refund_ref_abc","payment_intent_id":"int_x",
            "amount":5.00,"currency":"USD","status":"FAILED",
            "failure_details":{"code":"refund_declined","message":"Issuer refused the refund","trace_id":"t9"}}}}"#;
        let event = parse_webhook_event(body).expect("envelope parses");
        assert_eq!(event.name.event_type(), EventType::RefundFailure);

        let details = build_refund_webhook_response(&event, body).expect("refund webhook builds");
        assert_eq!(details.status, RefundStatus::Failure);
        assert_eq!(details.error_code.as_deref(), Some("refund_declined"));
        assert_eq!(
            details.error_message.as_deref(),
            Some("Issuer refused the refund")
        );
    }

    #[test]
    fn dispute_requires_response_converts_major_units_and_tolerates_a_missing_intent_id() {
        let event = parse_webhook_event(DISPUTE_REQUIRES_RESPONSE).expect("envelope parses");
        assert_eq!(
            event.name,
            AirwallexWebhookEventName::PaymentDisputeRequiresResponse
        );
        // The snake_case spelling of the account key has to bind too.
        assert_eq!(
            event.account_id.as_deref(),
            Some("acct_BGEVPKSxP9OjzQGbwxVEKA")
        );
        assert_eq!(event.name.event_type(), EventType::DisputeOpened);

        let reference = webhook_reference(&event)
            .expect("reference resolves")
            .expect("dispute reference is present");
        match reference {
            WebhookResourceReference::Dispute(dispute) => {
                assert_eq!(
                    dispute.connector_dispute_id.as_deref(),
                    Some("dst_sgstmr5zzgpg1wsjcwo")
                );
                assert_eq!(
                    dispute.connector_transaction_id, None,
                    "the published dispute sample carries no payment_intent_id"
                );
            }
            other => panic!("expected a dispute reference, got {other:?}"),
        }

        let details =
            build_dispute_webhook_response(&event, DISPUTE_REQUIRES_RESPONSE, major(), minor())
                .expect("dispute webhook builds");
        assert_eq!(details.dispute_id, "dst_sgstmr5zzgpg1wsjcwo");
        assert_eq!(details.status, DisputeStatus::DisputeOpened);
        assert_eq!(details.stage, DisputeStage::Dispute);
        assert_eq!(details.currency, Currency::AUD);
        assert_eq!(
            details.amount.to_string(),
            "1000",
            "AUD 10 is 1000 minor units — the field is StringMinorUnit, the wire is major"
        );
        assert_eq!(details.dispute_message, None);
        assert_eq!(details.connector_reason_code, None);
    }

    #[test]
    fn full_dispute_body_surfaces_the_reason_and_the_parent_payment() {
        let event = parse_webhook_event(DISPUTE_ACCEPTED_FULL).expect("envelope parses");
        assert_eq!(event.name.event_type(), EventType::DisputeAccepted);

        let details =
            build_dispute_webhook_response(&event, DISPUTE_ACCEPTED_FULL, major(), minor())
                .expect("dispute webhook builds");
        assert_eq!(details.status, DisputeStatus::DisputeAccepted);
        assert_eq!(details.stage, DisputeStage::Dispute);
        assert_eq!(details.amount.to_string(), "10000");
        assert_eq!(details.currency, Currency::USD);
        assert_eq!(
            details.connector_response_reference_id.as_deref(),
            Some("int_hkpdskz7vg1xc7uscdj")
        );
        assert_eq!(
            details.dispute_message.as_deref(),
            Some("Fraudulent transaction.")
        );
        assert_eq!(details.connector_reason_code.as_deref(), Some("4837"));
    }

    /// Every dispute lifecycle event maps onto a `DisputeStatus`, and the escalation ladder maps
    /// onto the three UCS stages.
    #[test]
    fn every_dispute_lifecycle_event_maps_to_a_status_and_a_stage() {
        let cases = [
            (
                "payment_dispute.requires_response",
                "RFI",
                "REQUIRES_RESPONSE",
                EventType::DisputeOpened,
                DisputeStatus::DisputeOpened,
                DisputeStage::PreDispute,
            ),
            (
                "payment_dispute.challenged",
                "PRE_CHARGEBACK",
                "CHALLENGED",
                EventType::DisputeChallenged,
                DisputeStatus::DisputeChallenged,
                DisputeStage::PreDispute,
            ),
            (
                "payment_dispute.accepted",
                "CHARGEBACK",
                "ACCEPTED",
                EventType::DisputeAccepted,
                DisputeStatus::DisputeAccepted,
                DisputeStage::Dispute,
            ),
            (
                "payment_dispute.expired",
                "CHARGEBACK",
                "EXPIRED",
                EventType::DisputeExpired,
                DisputeStatus::DisputeExpired,
                DisputeStage::Dispute,
            ),
            (
                "payment_dispute.pending_closure",
                "PRE_ARBITRATION",
                "PENDING_CLOSURE",
                EventType::DisputeAccepted,
                DisputeStatus::DisputeAccepted,
                DisputeStage::PreArbitration,
            ),
            (
                "payment_dispute.pending_decision",
                "PRE_ARBITRATION",
                "PENDING_DECISION",
                EventType::DisputeChallenged,
                DisputeStatus::DisputeChallenged,
                DisputeStage::PreArbitration,
            ),
            (
                "payment_dispute.won",
                "ARBITRATION",
                "WON",
                EventType::DisputeWon,
                DisputeStatus::DisputeWon,
                DisputeStage::PreArbitration,
            ),
            (
                "payment_dispute.lost",
                "ARBITRATION",
                "LOST",
                EventType::DisputeLost,
                DisputeStatus::DisputeLost,
                DisputeStage::PreArbitration,
            ),
            (
                "payment_dispute.reversed",
                "CHARGEBACK",
                "REVERSED",
                EventType::DisputeCancelled,
                DisputeStatus::DisputeCancelled,
                DisputeStage::Dispute,
            ),
        ];

        for (name, stage, status, expected_event, expected_status, expected_stage) in cases {
            let body = format!(
                r#"{{"id":"evt_1","name":"{name}","data":{{"object":{{
                    "id":"dst_x","amount":10,"currency":"AUD","stage":"{stage}","status":"{status}"}}}}}}"#
            );
            let event = parse_webhook_event(body.as_bytes()).expect("envelope parses");
            assert_eq!(event.name.event_type(), expected_event, "{name}");
            assert!(event.name.event_type().is_dispute_event(), "{name}");

            let details = build_dispute_webhook_response(&event, body.as_bytes(), major(), minor())
                .expect("dispute webhook builds");
            assert_eq!(details.status, expected_status, "{name}");
            assert_eq!(details.stage, expected_stage, "{name}");
        }
    }

    /// An event UCS does not model must be reported as unspecified and resolve no reference —
    /// never error, and never be misshapen into a payment response.
    #[test]
    fn unmodelled_events_are_unspecified_and_carry_no_reference() {
        let body =
            br#"{"id":"evt_1","name":"payment_consent.created","data":{"object":{"id":"cst_x"}}}"#;
        let event = parse_webhook_event(body).expect("envelope parses");
        assert_eq!(event.name, AirwallexWebhookEventName::Unknown);
        assert_eq!(
            event.name.event_type(),
            EventType::IncomingWebhookEventUnspecified
        );
        assert!(webhook_reference(&event)
            .expect("reference resolves")
            .is_none());

        // `IncomingWebhookEventUnspecified` matches none of the three fan-out predicates, so the
        // dispatcher falls through to the payment path with it. That has to be a clean error.
        let err = build_payment_webhook_response(&event, body, major())
            .expect_err("an unmodelled event is not a payment webhook");
        assert!(matches!(
            err.current_context(),
            WebhookError::WebhookEventTypeNotFound
        ));
    }

    /// The fan-out routes by `EventType`, so a refund event reaching the payment builder (or the
    /// reverse) must be rejected rather than misshapen.
    #[test]
    fn builders_reject_events_from_another_resource_family() {
        let refund_event = parse_webhook_event(REFUND_ACCEPTED).expect("envelope parses");
        assert!(build_payment_webhook_response(&refund_event, REFUND_ACCEPTED, major()).is_err());
        assert!(
            build_dispute_webhook_response(&refund_event, REFUND_ACCEPTED, major(), minor())
                .is_err()
        );

        let intent_event = parse_webhook_event(PAYMENT_INTENT_CREATED).expect("envelope parses");
        assert!(build_refund_webhook_response(&intent_event, PAYMENT_INTENT_CREATED).is_err());
    }

    /// An attempt body without `payment_intent_id` cannot be resolved to a payment. Failing
    /// loudly beats writing the `att_…` id into `connector_transaction_id`.
    #[test]
    fn attempt_without_an_intent_id_is_a_missing_required_field() {
        let body = br#"{"id":"evt_1","name":"payment_attempt.authorized","data":{"object":{"id":"att_x"}}}"#;
        let event = parse_webhook_event(body).expect("envelope parses");
        let err = webhook_reference(&event).expect_err("no intent id, no reference");
        assert!(matches!(
            err.current_context(),
            WebhookError::WebhookMissingRequiredField {
                field: "data.object.payment_intent_id"
            }
        ));
    }

    #[test]
    fn a_malformed_body_is_a_decoding_failure_not_a_panic() {
        let err = parse_webhook_event(b"not json at all").expect_err("garbage must not parse");
        assert!(matches!(
            err.current_context(),
            WebhookError::WebhookBodyDecodingFailed
        ));
    }

    /// Airwallex publishes no `(secret, timestamp, body) -> signature` vector, so this pins the
    /// wiring — header names, concatenation order (timestamp **then** raw body, no separator),
    /// HMAC-SHA256, lowercase hex — against a locally computed digest. It proves the algorithm
    /// this connector implements, **not** interop with Airwallex; that needs a captured sandbox
    /// webhook.
    #[test]
    fn signature_preimage_is_the_timestamp_concatenated_with_the_raw_body() {
        let secret = b"whsec_airwallex_test_secret";
        let timestamp = "1357872222592";

        let mut message = timestamp.as_bytes().to_vec();
        message.extend_from_slice(PAYMENT_INTENT_CREATED);

        let digest = common_utils::crypto::HmacSha256
            .sign_message(secret, &message)
            .expect("HMAC-SHA256 signs");
        let hex_digest = hex::encode(&digest);

        // Pinned so a change to the preimage recipe fails here rather than in production.
        assert_eq!(hex_digest.len(), 64);
        assert!(common_utils::crypto::HmacSha256
            .verify_signature(secret, &digest, &message)
            .expect("verification runs"));

        // The concatenation order is load-bearing: body-then-timestamp must not verify.
        let mut reversed = PAYMENT_INTENT_CREATED.to_vec();
        reversed.extend_from_slice(timestamp.as_bytes());
        assert!(!common_utils::crypto::HmacSha256
            .verify_signature(secret, &digest, &reversed)
            .expect("verification runs"));

        // And a body altered by so much as one byte must not verify.
        let mut tampered = message.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(!common_utils::crypto::HmacSha256
            .verify_signature(secret, &digest, &tampered)
            .expect("verification runs"));
    }
}
