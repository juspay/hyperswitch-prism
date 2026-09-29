use crate::{
    connectors::{hipay::HipayRouterData, macros::GetFormData},
    types::ResponseRouterData,
};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii::SecretSerdeValue,
    request::MultipartData,
    types::{AmountConvertor, MinorUnit, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PaymentMethodToken, RSync, Refund, RepeatPayment, SetupMandate,
        Void,
    },
    connector_types::{
        EventType, MandateIds, MandateReference, MandateReferenceId, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    mandates::{MandateData, MandateDataType},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct HipayAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for HipayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Hipay {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send a HipayConfig with api_key and api_secret for this connector."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "HiPay authenticates with HTTP Basic credentials from the merchant's \
                             back office."
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

/// HiPay types the envelope `code` as an integer and its published catalogue runs to seven
/// digits (`4010103` Insufficient Funds, `4010312` Soft Declined), so the carrier must be wide
/// enough for the whole range. It is modelled as integer-or-string because the maintenance and
/// Secure Vault endpoints quote the same code as a JSON string.
/// spec:## HTTP Codes and Errors ### Error Response Body Format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HipayErrorCode {
    Numeric(i64),
    Text(String),
}

impl std::fmt::Display for HipayErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Numeric(code) => write!(f, "{code}"),
            Self::Text(code) => write!(f, "{code}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayErrorResponse {
    pub code: HipayErrorCode,
    pub message: String,
    /// Actionable detail HiPay returns alongside `message`; surfaced as `ErrorResponse.reason`.
    #[serde(default)]
    pub description: Option<String>,
}

impl HipayErrorResponse {
    /// The one place a HiPay error envelope becomes an `ErrorResponse`, used by the maintenance
    /// flows when HiPay sends that envelope under HTTP 200 (see [`HipayMaintenanceEnvelope`]).
    /// All three parsed fields are read: `code` and `message` verbatim, and `description` as
    /// `reason` when it is not HiPay's empty-string placeholder — a 200 error envelope must not
    /// reach the merchant with its cause dropped (repeat_issues.md "Fields parsed but never
    /// used"). `attempt_status` is supplied by the caller so each flow reports its own failure
    /// variant (`CaptureFailed`, `VoidFailed`, `RefundStatus::Failure`) rather than a shared
    /// `Failure`.
    fn into_error_response(
        self,
        status_code: u16,
        attempt_status: FlowStatus,
        connector_transaction_id: Option<String>,
    ) -> domain_types::router_data::ErrorResponse {
        let reason = self
            .description
            .map(|description| description.trim().to_owned())
            .filter(|description| !description.is_empty());
        domain_types::router_data::ErrorResponse {
            code: self.code.to_string(),
            message: self.message,
            reason,
            status_code,
            attempt_status: Some(attempt_status),
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
}

/// Transaction-level decline detail. This is the only carrier of the acquirer / issuer reason
/// on an in-band 2xx decline.
/// spec:### Transaction-level decline detail (reason object)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HipayReason {
    pub code: String,
    #[serde(default)]
    pub message: String,
}

/// HiPay emits `"reason": ""` — an empty **string** — on a response that carries no decline
/// detail (observed on an approved `/v1/order`: `state: completed`, `status: "118"`,
/// `message: "Captured"`), and a `reason` **object** only when there is one. A plain
/// `Option<HipayReason>` therefore fails to deserialize every successful authorization with
/// `invalid type: string "", expected struct HipayReason`. This is the same either-shape
/// tolerance [`HipayErrorCode`] gives its integer-or-string `code`, expressed as a
/// `deserialize_with` because the two shapes collapse to one `Option`.
///
/// The placeholder string, a `null`, an absent key and an object whose `code` is blank all mean
/// the same thing — no decline detail — and all map to `None`, so
/// [`network_fields_from_reason`] and the `ErrorResponse` mappings below see exactly one
/// representation of it. A `reason` object that is present but malformed is still an error:
/// silently dropping it would lose the only carrier of the acquirer / issuer decline code.
/// spec:### Transaction-level decline detail (reason object)
fn deserialize_optional_reason<'de, D>(deserializer: D) -> Result<Option<HipayReason>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(deserialize_optional_object::<D, HipayReason>(deserializer)?
        .filter(|reason| !reason.code.trim().is_empty()))
}

/// The either-shape tolerance [`deserialize_optional_reason`] describes, as one rule the module
/// applies wherever HiPay quotes a nested object that may arrive as the empty-string placeholder
/// instead — the maintenance `reason` and the v3 consultation `reason` both do. Keeping it in one
/// place is deliberate: a second, slightly different tolerance is how one of the two shapes ends
/// up hard-failing a flow nobody exercised yet.
/// spec:### Transaction-level decline detail (reason object)
fn deserialize_optional_object<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    match <Option<serde_json::Value> as Deserialize>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(placeholder)) if placeholder.trim().is_empty() => Ok(None),
        Some(value) => serde_json::from_value::<T>(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// The same tolerance as [`deserialize_optional_object`], adapted to the v3 consultation's
/// **non-optional** `reason` field: PSync reads `Reason` by reference downstream
/// (`network_fields_from_sync_reason`), so an absent or empty-string placeholder becomes
/// `Reason::default()` rather than `None`. This is an adapter over the one rule, not a second
/// tolerance — `HipayRefundSyncJsonResponse` already applies that rule to the same endpoint's
/// `reason`, and PSync failing to deserialize a body RSync accepts is exactly the divergence
/// keeping the tolerance in one place is meant to prevent.
///
/// The `unwrap_or_default()` is not an INV-17 fallback on an error field: `Reason::default()` is
/// `{ reason: None, code: None }` — literally "no decline detail", the same thing the `Option`
/// form says — so the error builder still falls back to `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`
/// rather than reporting a fabricated empty code.
/// spec:#### 3. Transaction Lookup (PSync / RSync)
fn deserialize_optional_reason_v3<'de, D>(deserializer: D) -> Result<Reason, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(deserialize_optional_object::<D, Reason>(deserializer)?.unwrap_or_default())
}

/// The `attempt_status` an `ErrorResponse` carries for a soft decline.
///
/// HiPay signals a soft decline twice at once — status `178` (v3 `78`) and reason code
/// `4010312` — and both mean the same thing: retry **with** 3-D Secure. The payment status
/// stays `Failure` (this attempt really did fail), but the error object reports
/// `AuthenticationFailed` so the retry-with-3DS signal survives alongside the
/// `network_decline_code`; a bare `Failure` on both would collapse the two into one.
/// spec:## 9. Error / decline reason codes; spec:### ThreeDS > Authentication statuses
fn soft_decline_attempt_status(mapped: AttemptStatus, soft_declined: bool) -> AttemptStatus {
    if soft_declined {
        AttemptStatus::AuthenticationFailed
    } else {
        mapped
    }
}

/// `(network_decline_code, network_error_message)` from a HiPay `reason` object.
///
/// Only the `40xxxxx` band is an acquirer / issuer decline; `10xxxxx` and `30xxxxx` are HiPay-side
/// configuration or maintenance errors and must not be reported to the merchant as network
/// declines. spec:## 9. Error / decline reason codes ### Recommended mapping / Range semantics
pub fn network_fields_from_reason(
    reason: Option<&HipayReason>,
) -> (Option<String>, Option<String>) {
    match reason {
        Some(reason) => match reason.code.trim().parse::<u32>() {
            Ok(code) if (4_000_000..5_000_000).contains(&code) => {
                (Some(reason.code.clone()), Some(reason.message.clone()))
            }
            _ => (None, None),
        },
        None => (None, None),
    }
}

// HiPay Payment Status Enum - Type-safe status codes from HiPay API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HipayPaymentStatus {
    #[serde(rename = "109")]
    AuthenticationFailed,
    #[serde(rename = "110")]
    Blocked,
    #[serde(rename = "111")]
    Denied,
    #[serde(rename = "112")]
    AuthorizedAndPending,
    #[serde(rename = "113")]
    Refused,
    #[serde(rename = "114")]
    Expired,
    #[serde(rename = "115")]
    Cancelled,
    #[serde(rename = "116")]
    Authorized,
    #[serde(rename = "117")]
    CaptureRequested,
    #[serde(rename = "118")]
    Captured,
    #[serde(rename = "119")]
    PartiallyCaptured,
    #[serde(rename = "129")]
    ChargedBack,
    #[serde(rename = "173")]
    CaptureRefused,
    #[serde(rename = "174")]
    AwaitingTerminal,
    #[serde(rename = "175")]
    AuthorizationCancellationRequested,
    #[serde(rename = "177")]
    ChallengeRequested,
    #[serde(rename = "178")]
    SoftDeclined,
    #[serde(rename = "200")]
    PendingPayment,
    #[serde(rename = "101")]
    Created,
    #[serde(rename = "105")]
    UnableToAuthenticate,
    #[serde(rename = "106")]
    CardholderAuthenticated,
    #[serde(rename = "107")]
    AuthenticationAttempted,
    #[serde(rename = "108")]
    CouldNotAuthenticate,
    #[serde(rename = "120")]
    Collected,
    #[serde(rename = "121")]
    PartiallyCollected,
    #[serde(rename = "122")]
    Settled,
    #[serde(rename = "123")]
    PartiallySettled,
    #[serde(rename = "140")]
    AuthenticationRequested,
    #[serde(rename = "141")]
    Authenticated,
    #[serde(rename = "151")]
    AcquirerNotFound,
    #[serde(rename = "161")]
    RiskAccepted,
    #[serde(rename = "163")]
    AuthorizationRefused,
    #[serde(rename = "103")]
    CardholderEnrolled,
    #[serde(rename = "104")]
    CardholderNotEnrolled,
    #[serde(rename = "131")]
    Debited,
    #[serde(rename = "132")]
    PartiallyDebited,
    #[serde(rename = "134")]
    DisputeLost,
    #[serde(rename = "142")]
    AuthorizationRequested,
    #[serde(rename = "143")]
    AuthorizationCancelled,
    #[serde(rename = "144")]
    ReferenceRendered,
    #[serde(rename = "150")]
    AcquirerFound,
    #[serde(rename = "160")]
    CardholderEnrollmentUnknown,
    #[serde(rename = "166")]
    Debited166,
    #[serde(rename = "168")]
    Debited168,
    #[serde(rename = "169")]
    CreditRequested,
    #[serde(rename = "172")]
    InProgress,
    #[serde(rename = "180")]
    PartiallyChargeback,
    #[serde(rename = "181")]
    Chargeback,
    /// Any status HiPay adds after this integration was written. Mapped to
    /// `AttemptStatus::Unknown` so the caller keeps the status it already had rather than
    /// the connector inventing a terminal one.
    /// spec:## Status Mappings > Gap vs. current implementation
    #[serde(other)]
    Unknown,
}

impl From<HipayPaymentStatus> for AttemptStatus {
    fn from(status: HipayPaymentStatus) -> Self {
        match status {
            HipayPaymentStatus::AuthenticationFailed => Self::AuthenticationFailed,
            HipayPaymentStatus::Blocked
            | HipayPaymentStatus::Refused
            | HipayPaymentStatus::Denied => Self::Failure,
            // 114 Expired — an authorization that lapsed before capture. It is not a decline,
            // and AttemptStatus has a dedicated terminal variant for it.
            // spec:## Status Mappings ### Notified statuses
            HipayPaymentStatus::Expired => Self::Expired,
            HipayPaymentStatus::AuthorizedAndPending => Self::Pending,
            HipayPaymentStatus::Cancelled => Self::Voided,
            HipayPaymentStatus::Authorized => Self::Authorized,
            HipayPaymentStatus::CaptureRequested => Self::CaptureInitiated,
            HipayPaymentStatus::Captured => Self::Charged,
            HipayPaymentStatus::PartiallyCaptured => Self::PartialCharged,
            HipayPaymentStatus::CaptureRefused => Self::CaptureFailed,
            HipayPaymentStatus::AwaitingTerminal => Self::Pending,
            HipayPaymentStatus::AuthorizationCancellationRequested => Self::VoidInitiated,
            HipayPaymentStatus::ChallengeRequested => Self::AuthenticationPending,
            HipayPaymentStatus::SoftDeclined => Self::Failure,
            HipayPaymentStatus::PendingPayment => Self::Pending,
            HipayPaymentStatus::ChargedBack => Self::Failure,
            HipayPaymentStatus::Created => Self::Started,
            HipayPaymentStatus::UnableToAuthenticate | HipayPaymentStatus::CouldNotAuthenticate => {
                Self::AuthenticationFailed
            }
            HipayPaymentStatus::CardholderAuthenticated => Self::AuthenticationSuccessful,
            HipayPaymentStatus::AuthenticationAttempted => Self::AuthenticationPending,
            HipayPaymentStatus::CardholderEnrolled
            | HipayPaymentStatus::CardholderNotEnrolled
            | HipayPaymentStatus::CardholderEnrollmentUnknown => Self::AuthenticationPending,
            HipayPaymentStatus::Debited
            | HipayPaymentStatus::PartiallyDebited
            | HipayPaymentStatus::AuthorizationRequested
            | HipayPaymentStatus::ReferenceRendered
            | HipayPaymentStatus::AcquirerFound
            | HipayPaymentStatus::CreditRequested
            | HipayPaymentStatus::InProgress => Self::Pending,
            HipayPaymentStatus::AuthorizationCancelled => Self::Voided,
            HipayPaymentStatus::Debited166 | HipayPaymentStatus::Debited168 => Self::Charged,
            HipayPaymentStatus::DisputeLost
            | HipayPaymentStatus::PartiallyChargeback
            | HipayPaymentStatus::Chargeback => Self::Failure,
            HipayPaymentStatus::Unknown => Self::Unknown,
            HipayPaymentStatus::Collected
            | HipayPaymentStatus::PartiallySettled
            | HipayPaymentStatus::PartiallyCollected
            | HipayPaymentStatus::Settled => Self::Charged,
            HipayPaymentStatus::AuthenticationRequested => Self::AuthenticationPending,
            HipayPaymentStatus::Authenticated => Self::AuthenticationSuccessful,
            HipayPaymentStatus::AcquirerNotFound => Self::Failure,
            HipayPaymentStatus::RiskAccepted => Self::Pending,
            HipayPaymentStatus::AuthorizationRefused => Self::Failure,
        }
    }
}

// HiPay Refund Status Enum - Type-safe refund status codes
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HipayRefundStatus {
    #[serde(rename = "124")]
    RefundRequested,
    #[serde(rename = "125")]
    Refunded,
    #[serde(rename = "126")]
    PartiallyRefunded,
    #[serde(rename = "165")]
    RefundRefused,
    /// RDR (Rapid Dispute Resolution) refunds arrive as 182 / 183 and are settled refunds.
    /// spec:### Refunds (Success statuses 124/125/126; RDR refunds arrive as 182 / 183)
    #[serde(rename = "182")]
    PartiallyRefundByRdr,
    #[serde(rename = "183")]
    RefundByRdr,
    /// Any refund status HiPay adds later: the caller keeps the status it already had.
    #[serde(other)]
    Unknown,
}

impl From<HipayRefundStatus> for RefundStatus {
    fn from(item: HipayRefundStatus) -> Self {
        match item {
            HipayRefundStatus::RefundRequested => Self::Pending,
            HipayRefundStatus::Refunded
            | HipayRefundStatus::PartiallyRefunded
            | HipayRefundStatus::PartiallyRefundByRdr
            | HipayRefundStatus::RefundByRdr => Self::Success,
            HipayRefundStatus::RefundRefused => Self::Failure,
            HipayRefundStatus::Unknown => Self::Unknown,
        }
    }
}

// Sync Response Types
// Reason struct for PSync response - matches v3 API format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Reason {
    pub reason: Option<String>,
    pub code: Option<u64>,
}

// HiPay v3 PSync Response - flat structure matching v3 transaction API
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HipaySyncResponse {
    Response {
        id: i64,
        status: i32,
        /// Tolerant of HiPay's empty-string placeholder, the same way
        /// `HipayRefundSyncJsonResponse::reason` is for this very endpoint: a v3 consultation
        /// that carries `"reason": ""` must not fail to deserialize a PSync.
        /// spec:#### 3. Transaction Lookup (PSync / RSync)
        #[serde(default, deserialize_with = "deserialize_optional_reason_v3")]
        reason: Reason,
        #[serde(flatten)]
        extra: std::collections::HashMap<String, serde_json::Value>,
    },
    Error {
        message: String,
        code: u32,
    },
}

/// HiPay v3 Refund Sync Response — the **same** `GET {third_base_url}/v3/transaction/{ref}`
/// consultation PSync uses, addressed by the ORIGINAL transaction reference. The v3 body is
/// snake_case and carries no per-operation array, so the refund state is read from `status`
/// plus the aggregate `refunded_amount` (P-Refunds-04 / P-Refunds-05, UD-04).
///
/// `id` is the consultation service's own identifier and is deliberately **not** reported back
/// as `connector_refund_id`: Execute wrote the original transaction reference, and a sync that
/// answered with a different value would give one refund two identities (TH-16, HP-19).
/// spec:### Refunds > RSync endpoint; spec:### Refunds > DOCUMENTATION GAP — no refund-level identifier
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayRefundSyncJsonResponse {
    pub id: i64,
    pub status: i32,
    /// Acquirer / issuer decline detail for a refused refund; the only carrier of the reason on
    /// the RSync path. Tolerant of HiPay's empty-string placeholder, so a consultation that
    /// carries no decline detail does not fail to deserialize.
    /// spec:#### 3. Transaction Lookup (PSync / RSync)
    #[serde(default, deserialize_with = "deserialize_optional_object")]
    pub reason: Option<Reason>,
    /// Cumulative refunded amount across every refund on this transaction, a **major-unit**
    /// decimal string (`StringMajorUnit`), e.g. `"9.99"`.
    /// spec:### Refunds > DOCUMENTATION GAP — no refund-level identifier
    #[serde(default, alias = "refundedAmount")]
    pub refunded_amount: Option<StringMajorUnit>,
}

// Type alias for backward compatibility
pub type HipayRefundSyncResponse = HipayRefundSyncJsonResponse;

// HiPay Operation Enum - Type-safe operation codes for maintenance requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HipayOperation {
    Capture,
    Refund,
    Cancel,
}

impl std::fmt::Display for HipayOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capture => write!(f, "capture"),
            Self::Refund => write!(f, "refund"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Authorization,
    Sale,
}

/// G-Payments-04 — resolve HiPay's `operation` from the caller's `capture_method`.
///
/// HiPay's order `operation` enum has exactly two members: `Sale` settles immediately and
/// `Authorization` holds an authorisation for a later `capture` maintenance operation
/// (spec:### Core order fields, spec:#### 2). `Automatic`/`SequentialAutomatic`/absent map to
/// `Sale` and `Manual` to `Authorization`; nothing on the wire distinguishes an authorisation
/// that will be settled once from one the caller intends to settle in several parts or on a
/// schedule, and the maintenance guides document a full and a partial `capture` without ever
/// stating that a second capture on the same transaction is accepted (spec:#### 2, and the
/// DOCUMENTATION GAP on partial maintenances under spec:### Void). `ManualMultiple` and
/// `Scheduled` therefore have no representation HiPay has committed to honouring.
///
/// They are refused here, before the request is built, rather than folded into `Sale` by a
/// wildcard. That wildcard was a charging defect, not a rounding of intent: HiPay would settle
/// the full amount at once and answer `118 Captured`, while the caller believes it holds an
/// uncaptured authorisation it can still capture in parts. Hyperswitch's own HiPay connector
/// refuses the same two variants through `PaymentsAuthorizeRequestData::is_auto_capture()`
/// (hs:crates/hyperswitch_connectors/src/utils.rs), and every other UCS connector groups
/// `ManualMultiple` with `Manual` rather than with `Automatic`, so refusing keeps UCS and
/// Hyperswitch answering the same thing for the same request.
///
/// `PaymentsAuthorizeData::is_auto_capture()` in `domain_types` is documented as a pure getter
/// that folds `ManualMultiple`/`Scheduled` into "manual" and explicitly tells connectors that
/// cannot honour a capture method to validate explicitly, which is what this does. It is shared
/// by Authorize, the SetupMandate CIT and RepeatPayment so a single decision covers all three
/// charging paths.
fn hipay_operation(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<Operation, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::Manual) => Ok(Operation::Authorization),
        None
        | Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic) => Ok(Operation::Sale),
        Some(unsupported @ common_enums::CaptureMethod::ManualMultiple)
        | Some(unsupported @ common_enums::CaptureMethod::Scheduled) => Err(error_stack::report!(
            IntegrationError::CaptureMethodNotSupported {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send capture_method MANUAL to hold a HiPay authorisation and \
                             capture it once, or AUTOMATIC to settle immediately; route \
                             multi-capture or scheduled settlement to a connector that \
                             supports it."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "HiPay's order `operation` enum is Sale|Authorization only, and its \
                             maintenance guides document no second capture on a transaction, \
                             so capture_method {unsupported:?} cannot be expressed on POST \
                             /v1/order. Refused before the request is built so the payment is \
                             not silently settled in full as a Sale."
                    )),
                },
            }
        )),
    }
}

/// The card and card-adjacent values of HiPay's published `payment_product` enum. Anything
/// outside this list is rejected by HiPay with `1020003 Unsupported Payment Product`, so a
/// network we cannot map is refused locally rather than sent.
/// spec:### payment_product code table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HipayPaymentProduct {
    Visa,
    Mastercard,
    Maestro,
    AmericanExpress,
    Cb,
    Bcmc,
    Diners,
    Jcb,
    Discover,
    Unionpay,
    Cup,
    Dankort,
    Postepay,
    EloCard,
    Hipercard,
}

impl HipayPaymentProduct {
    /// Resolves a HiPay wire name (the Secure Vault `domestic_network` or `brand`) to the
    /// published enum. The comparison is case-insensitive and tolerates the space/underscore
    /// spellings HiPay uses for the brand field.
    fn from_wire_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().replace([' ', '_'], "-").as_str() {
            "visa" => Some(Self::Visa),
            "mastercard" => Some(Self::Mastercard),
            "maestro" => Some(Self::Maestro),
            "american-express" | "amex" => Some(Self::AmericanExpress),
            "cb" | "cartes-bancaires" => Some(Self::Cb),
            "bcmc" | "bancontact" => Some(Self::Bcmc),
            "diners" | "diners-club" => Some(Self::Diners),
            "jcb" => Some(Self::Jcb),
            "discover" => Some(Self::Discover),
            "unionpay" => Some(Self::Unionpay),
            "cup" => Some(Self::Cup),
            "dankort" => Some(Self::Dankort),
            "postepay" => Some(Self::Postepay),
            "elo-card" | "elo" => Some(Self::EloCard),
            "hipercard" => Some(Self::Hipercard),
            _ => None,
        }
    }

    /// Maps a Hyperswitch card network onto HiPay's enum. Networks HiPay does not route
    /// (Interac, RuPay, Star, Pulse, …) deliberately return `None` so `payment_product_for`
    /// refuses instead of emitting a value HiPay rejects.
    fn from_card_network(network: &common_enums::CardNetwork) -> Option<Self> {
        match network {
            common_enums::CardNetwork::Visa => Some(Self::Visa),
            common_enums::CardNetwork::Mastercard => Some(Self::Mastercard),
            common_enums::CardNetwork::Maestro => Some(Self::Maestro),
            common_enums::CardNetwork::AmericanExpress => Some(Self::AmericanExpress),
            common_enums::CardNetwork::CartesBancaires => Some(Self::Cb),
            common_enums::CardNetwork::DinersClub => Some(Self::Diners),
            common_enums::CardNetwork::JCB => Some(Self::Jcb),
            common_enums::CardNetwork::Discover => Some(Self::Discover),
            // spec:### payment_product code table lists `unionpay` (UnionPay) and `cup`
            // (China UnionPay) as two distinct HiPay products; CardNetwork::UnionPay is the
            // former. `Cup` stays reachable from a wire name (`from_wire_name`) and from a
            // caller-declared payment_product, so nothing is lost.
            common_enums::CardNetwork::UnionPay => Some(Self::Unionpay),
            common_enums::CardNetwork::Interac
            | common_enums::CardNetwork::RuPay
            | common_enums::CardNetwork::Star
            | common_enums::CardNetwork::Pulse
            | common_enums::CardNetwork::Accel
            | common_enums::CardNetwork::Nyce
            | common_enums::CardNetwork::Prop
            | common_enums::CardNetwork::PrivateLabel
            | common_enums::CardNetwork::Dinacard => None,
        }
    }
}

/// P-Payments-12 / UD-09 — reads a caller-declared `payment_product` out of one of the
/// connector-specific JSON carriers (`connector_feature_data`, then `metadata`). The
/// PaymentMethodToken arm carries no card network of its own — `tokenized_authorize_to_base`
/// builds `PaymentMethod::Token` with `token_payment_method_type: None` and drops
/// `payment_method` entirely — and HiPay's own answer for a token payment (the vault
/// response's `domestic_network`) has no carrier in the UCS Authorize contract, so the value
/// has to be declared by the caller. `merchant_account_id` is read by key out of the same
/// carrier (ucs:crates/types-traits/domain_types/src/types.rs:4758-4767).
/// A present-but-blank value is treated as absent so the refusal still fires (INV-17).
/// spec:## Field Dependency Analysis (payment_product — from the token's domestic_network for
/// token payments; otherwise derived from the card network)
fn declared_payment_product(carrier: Option<&SecretSerdeValue>) -> Option<String> {
    carrier
        .and_then(|carrier| carrier.peek().get("payment_product").cloned())
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// G-Payments-02 — resolve `payment_product` in HiPay's documented precedence:
/// the Secure Vault `domestic_network` (the co-badge HiPay itself picked), then the vault
/// `brand`, then a caller-declared product (`connector_feature_data.payment_product`, then
/// `metadata.payment_product` — the PaymentMethodToken arm's only source, P-Payments-12),
/// then the caller's `card_network`. An unresolvable network is refused before the
/// HTTP call rather than sent as the empty string.
/// spec:#### 4. Tokenization (Secure Vault) > Note (domestic_network); spec:### payment_product code table
pub fn payment_product_for(
    domestic_network: Option<&str>,
    token_brand: Option<&str>,
    declared_product: Option<&str>,
    card_network: Option<&common_enums::CardNetwork>,
) -> Result<HipayPaymentProduct, error_stack::Report<IntegrationError>> {
    domestic_network
        .and_then(HipayPaymentProduct::from_wire_name)
        .or_else(|| token_brand.and_then(HipayPaymentProduct::from_wire_name))
        .or_else(|| declared_product.and_then(HipayPaymentProduct::from_wire_name))
        .or_else(|| card_network.and_then(HipayPaymentProduct::from_card_network))
        .ok_or_else(|| {
            let described = domestic_network
                .or(token_brand)
                .or(declared_product)
                .map(ToOwned::to_owned)
                .or_else(|| card_network.map(|network| format!("{network:?}")))
                .unwrap_or_else(|| "<none supplied>".to_string());
            error_stack::report!(IntegrationError::NotSupported {
                message: format!("HiPay has no payment product for card network {described}"),
                connector: "hipay",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Route this card network to a connector that supports it, or send a \
                         card_network — or, on a token payment, a connector_feature_data or \
                         metadata `payment_product` — from HiPay's published payment_product \
                         enum (visa, mastercard, maestro, american-express, cb, bcmc, diners, \
                         jcb, discover, unionpay, cup, dankort, postepay, elo-card, hipercard)."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "HiPay requires payment_product on POST /v1/order and answers an \
                         unlisted value with 1020003 Unsupported Payment Product."
                            .to_string(),
                    ),
                },
            })
        })
}

// =========================================================================================
// PSD2 / 3DS2 request blocks
//
// HiPay runs its **own** MPI inside `POST /v1/order`: there is no PreAuthenticate /
// Authenticate / PostAuthenticate endpoint to call (LEG COUNT = 0). `authentication_indicator`
// asks HiPay to authenticate, the PSD2 blocks below are the inputs it authenticates with, and a
// challenge comes back as `forwardUrl` on the very same response.
// spec:### ThreeDS (LEG COUNT = 0); spec:#### PSD2 / 3DS2 data blocks
// =========================================================================================

/// `authentication_indicator` — how hard HiPay should try to authenticate the cardholder.
///
/// Derived from the request's `authentication_type`. Hyperswitch derives the same two values from
/// `is_three_ds()` but types the field as a bare `u8`
/// (hs:crates/hyperswitch_connectors/src/connectors/hipay/transformers.rs:79,173), so it cannot
/// express HiPay's third documented value, `1` (authenticate if the card is enrolled).
/// spec:## 11. External 3DS passthrough #### authentication_indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayAuthenticationIndicator {
    /// `0` — bypass 3-D Secure.
    #[serde(rename = "0")]
    Bypass,
    /// `1` — authenticate if the card is enrolled.
    #[serde(rename = "1")]
    IfAvailable,
    /// `2` — 3-D Secure authentication mandatory.
    #[serde(rename = "2")]
    Mandatory,
}

impl From<common_enums::AuthenticationType> for HipayAuthenticationIndicator {
    fn from(auth_type: common_enums::AuthenticationType) -> Self {
        match auth_type {
            common_enums::AuthenticationType::ThreeDs => Self::Mandatory,
            common_enums::AuthenticationType::NoThreeDs => Self::Bypass,
        }
    }
}

/// `device_channel` — the 3DS2 requestor channel.
///
/// UCS drives card payments server-to-server from a browser checkout, so `2` (BRW) is the
/// documented value and the only one this connector sends; `1` (APP) and `3` (3RI) complete
/// HiPay's published set so an unlisted integer can never be produced here.
/// spec:#### PSD2 / 3DS2 data blocks (device_channel 2 Browser, default)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayDeviceChannel {
    /// `1` — App-based (APP).
    #[serde(rename = "1")]
    App,
    /// `2` — Browser (BRW). HiPay's own default and the UCS card flow's channel.
    #[serde(rename = "2")]
    Browser,
    /// `3` — 3DS Requestor Initiated (3RI).
    #[serde(rename = "3")]
    ThreeRi,
}

/// `browser_info` — the 3DS2 browser data HiPay's MPI authenticates with.
///
/// Sent as `browser_info[language]=en-GB`, never as one opaque field: the shared
/// `build_form_from_struct` collapses a nested object to the empty string, which is exactly why
/// Hyperswitch's HiPay connector demands `browser_info` and then delivers it empty. The
/// bracket expansion lives in [`push_bracketed`].
/// spec:### browser_info (nested object, PSD2); UD-06
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayBrowserInfo {
    /// The one sub-field HiPay reports as required; G-ThreeDS-02 refuses the order without it.
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub javascript_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipaddr: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_accept: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_user_agent: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<u32>,
    /// UTC offset in minutes, as a string (`-120`) — HiPay types this one as a string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// The `ioBB` black box, duplicated here from the top-level `device_fingerprint`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_fingerprint: Option<Secret<String>>,
}

/// `merchant_risk_statement` — the merchant's PSD2 statement about the order.
///
/// HiPay accepts no `shipto_email`; the delivery email is carried here instead, which is the
/// one sub-field the specification names.
/// spec:## 2. Shipping / delivery (NOT SUPPORTED BY HIPAY: `shipto_email` … the PSD2 block
/// carries the delivery email instead, as `merchant_risk_statement.email_delivery_address`)
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayMerchantRiskStatement {
    pub email_delivery_address: common_utils::pii::Email,
}

/// The PSD2 / 3DS2 blocks of `POST /v1/order`, flattened into [`HipayPaymentsRequest`].
/// spec:#### PSD2 / 3DS2 data blocks
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HipayThreeDsData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_channel: Option<HipayDeviceChannel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_info: Option<HipayBrowserInfo>,
    /// The `ioBB` black box produced by HiPay's fraud JS on the merchant's page. It has no
    /// field on the UCS authorize contract, so it travels in the merchant-supplied carriers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_fingerprint: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_risk_statement: Option<HipayMerchantRiskStatement>,
    /// The customer's account history on the merchant's own site. HiPay documents the block
    /// but names no sub-field of it, and UCS carries no account-history type, so it is passed
    /// through from the merchant carrier exactly as supplied and bracket-encoded like every
    /// other nested object. spec:#### PSD2 / 3DS2 data blocks (`account_info` object)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_info: Option<serde_json::Value>,
}

/// One merchant-supplied value under `key`, read from `connector_feature_data` and then
/// `metadata` — the same two carriers [`declared_payment_product`] reads, used here for the
/// PSD2 inputs that have no field on the UCS authorize contract.
fn declared_value(
    carriers: [Option<&SecretSerdeValue>; 2],
    key: &str,
) -> Option<serde_json::Value> {
    carriers
        .into_iter()
        .flatten()
        .find_map(|carrier| carrier.peek().get(key).cloned())
        .filter(|value| !value.is_null())
}

/// The merchant-supplied `device_fingerprint` (`ioBB`) as a non-blank secret.
fn declared_device_fingerprint(carriers: [Option<&SecretSerdeValue>; 2]) -> Option<Secret<String>> {
    declared_value(carriers, "device_fingerprint")
        .and_then(|value| value.as_str().map(|text| text.trim().to_string()))
        .filter(|text| !text.is_empty())
        .map(Secret::new)
}

/// The merchant-supplied `description` read out of the same two connector-specific carriers
/// [`declared_payment_product`] uses (`connector_feature_data`, then `metadata`). HiPay documents
/// `description` as a required parameter of `POST /v1/order`, but neither
/// `PaymentServiceTokenSetupRecurringRequest` nor the recurring-charge request message has a
/// `description` field of its own, so on those two paths `metadata.description` is the only carrier
/// the caller has — exactly the [`declared_payment_product`] precedent (P-Payments-12).
/// A present-but-blank value is treated as absent so the missing-field refusal still fires.
/// spec:### Core order fields (description Required Yes)
fn declared_description(carriers: [Option<&SecretSerdeValue>; 2]) -> Option<String> {
    declared_value(carriers, "description")
        .and_then(|value| value.as_str().map(|text| text.trim().to_string()))
        .filter(|text| !text.is_empty())
}

/// One line item of HiPay's `basket` parameter. `basket` is a single form field carrying a
/// JSON-encoded **array** (never a map — the order of the lines is part of the data).
/// spec:## 5. Level 2 / Level 3 data ### Line items — the basket parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayBasketItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub european_article_numbering: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_reference: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub quantity: u16,
    pub unit_price: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_rate: Option<f64>,
    pub total_amount: StringMajorUnit,
}

/// `amount`, `shipping` and `tax` are HiPay **major-unit decimal strings** (`9.99`), produced by
/// the framework `StringMajorUnit` converter from the request's minor-unit amounts.
/// spec:### Core order fields
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayPaymentsRequest {
    pub payment_product: HipayPaymentProduct,
    pub orderid: String,
    pub operation: Operation,
    pub description: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    /// HiPay rejects raw PANs on POST /v1/order; the card must be vaulted first.
    pub cardtoken: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decline_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception_url: Option<String>,
    /// Overrides the account default **server-to-server** notification URL. Sourced from
    /// `webhook_url`, never from the browser return URL.
    /// spec:### Redirect / notification URLs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<String>,
    // --- Billing details (ordinary order parameters; accepted on non-3DS orders too and the
    // only inputs that produce an avsResult). spec:## 1. Billing details — exact field names
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streetaddress: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streetaddress2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zipcode: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    // --- Shipping / delivery. HiPay does NOT accept shipto_email, shipto_streetaddress3,
    // shipto_house_extension or shipto_company. spec:## 2. Shipping / delivery
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_firstname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_lastname: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_recipientinfo: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_house_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_streetaddress: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_streetaddress2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_zipcode: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipto_country: Option<common_enums::CountryAlpha2>,
    // --- Level 2 order-level amounts + the Level 3-ish basket. True Level 3 (duty, commodity
    // code, unit of measure, line-level tax amount) does not exist at HiPay.
    // spec:## 5. Level 2 / Level 3 data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<StringMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax: Option<StringMajorUnit>,
    /// Order-level tax rate as a decimal percentage string, not a currency amount.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_rate: Option<String>,
    /// JSON-encoded `Vec<HipayBasketItem>` in one form field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basket: Option<String>,
    /// PSD2 authentication indicator, derived from the request's `authentication_type` so no
    /// hardcoded literal decides the SCA preference. spec:#### authentication_indicator
    pub authentication_indicator: HipayAuthenticationIndicator,
    /// The PSD2 / 3DS2 blocks. Flattened so `browser_info` and its siblings serialise as
    /// HiPay's bracket-notation form fields rather than as one nested value.
    #[serde(flatten)]
    pub three_ds: HipayThreeDsData,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HipayPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
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
        let currency = request.currency;

        // G-Payments-04 (Card, PaymentMethodToken) — resolve `operation` from `capture_method`
        // first, so a capture method HiPay cannot express is refused before any other field is
        // resolved and the refusal is observable on every arm rather than shadowed by the
        // vault-first cardtoken refusal of G-Payments-01. spec:### Core order fields
        let operation = hipay_operation(request.capture_method)?;

        // G-ThreeDS-01 (Card, PaymentMethodToken) — HiPay performs 3-D Secure with its own MPI
        // and publishes no request-side `cavv`, `xid`, `ds_transaction_id`, `acs_transaction_id`
        // or protocol-version parameter, so a merchant-performed authentication cannot be
        // passed through. Refusing here, before the request is built, is the only way the
        // payment is not silently downgraded to a non-authenticated charge; the request `eci`
        // is a channel indicator (1,2,3,4,7,9,10), not a 3DS result ECI, so it is not a
        // substitute. spec:## 11. External 3DS passthrough ### Verdict
        if let Some(authentication) = request.authentication_data.as_ref() {
            let externally_authenticated = authentication.cavv.is_some()
                || authentication.eci.is_some()
                || authentication.ds_trans_id.is_some()
                || authentication.acs_transaction_id.is_some()
                || authentication.threeds_server_transaction_id.is_some()
                || authentication.message_version.is_some();
            if externally_authenticated {
                return Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "HiPay performs 3-D Secure with its own MPI and accepts no external \
                              authentication result on POST /v1/order"
                        .to_string(),
                    connector: "hipay",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the payment without authentication_data and let HiPay \
                             authenticate it — authentication_indicator is derived from \
                             authentication_type — or route an externally authenticated \
                             payment to a connector that accepts a CAVV."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "POST /v1/order carries no cavv, xid, ds_transaction_id, \
                             acs_transaction_id or three_d_secure_version parameter in any \
                             casing; authentication_indicator only toggles HiPay's own MPI."
                                .to_string(),
                        ),
                    },
                }));
            }
        }

        // The arm decides both where a Secure Vault cardtoken can come from and where
        // `payment_product` can be resolved from (P-Payments-12):
        //   * Card — raw card data, no token carrier at all on
        //     `PaymentServiceAuthorizeRequest`, so no cardtoken can exist; the network is the
        //     caller's `card_network`.
        //   * PaymentMethodToken — the vault token the caller obtained from
        //     `PaymentMethodService/Tokenize`; UCS drops the card network on this arm, so
        //     `payment_product` has to be declared (`declared_payment_product` below).
        // spec:#### 1. Create Order and Transaction
        let (cardtoken, card_network, arm_declares_product): (
            Option<Secret<String>>,
            Option<&common_enums::CardNetwork>,
            bool,
        ) = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => (None, card_data.card_network.as_ref(), false),
            PaymentMethodData::PaymentMethodToken(token) => (Some(token.token.clone()), None, true),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "HiPay supports card payments only; this payment method is not offered on \
                         POST /v1/order"
                        .to_string(),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Route this payment method to a connector that offers it.".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "HiPay's payment_product enum lists card and card-adjacent \
                                 products only."
                                .to_string(),
                        ),
                    },
                )))
            }
        };

        // ---- PSD2 / 3DS2 ----------------------------------------------------------------
        // HiPay authenticates inside this very call, so everything 3-D Secure needs is built
        // here rather than in a separate leg. spec:### ThreeDS (LEG COUNT = 0)
        let is_three_ds = common.is_three_ds();
        let carriers = [
            request.connector_feature_data.as_ref(),
            request.metadata.as_ref(),
        ];

        // G-ThreeDS-02 (Card, PaymentMethodToken) — HiPay's 3DS2 MPI authenticates with the
        // PSD2 `browser_info` block, and `browser_info.language` is the sub-field it reports as
        // required. Hyperswitch demands the block and then loses it to the shared form builder,
        // so HiPay receives it empty; refusing before the HTTP call is the fail-closed reading.
        // spec:### browser_info (nested object, PSD2)
        let browser_info = request.browser_info.as_ref();
        let language = browser_info.and_then(|info| info.language.clone());
        let language = match (is_three_ds, language) {
            (true, None) => {
                return Err(
                    error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "browser_info",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Collect the PSD2 browser_info block client-side and send it on the \
                             authorize request; browser_info.language is mandatory."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "HiPay's 3DS2 MPI requires the PSD2 browser_info block; \
                             browser_info.language is the reported required sub-field."
                                .to_string(),
                        ),
                    },
                }),
                )
            }
            (_, language) => language,
        };

        let device_fingerprint = declared_device_fingerprint(carriers);
        // Only a 3DS order carries the PSD2 blocks: on a bypassed order they are noise HiPay
        // has no use for, and `device_channel` is meaningless without an authentication.
        let three_ds = if is_three_ds {
            HipayThreeDsData {
                device_channel: Some(HipayDeviceChannel::Browser),
                browser_info: language
                    .zip(browser_info)
                    .map(|(language, info)| HipayBrowserInfo {
                        language,
                        java_enabled: info.java_enabled,
                        javascript_enabled: info.java_script_enabled,
                        ipaddr: info
                            .ip_address
                            .map(|address| Secret::new(address.to_string())),
                        http_accept: info.accept_header.clone(),
                        http_user_agent: info.user_agent.clone().map(Secret::new),
                        color_depth: info.color_depth,
                        screen_height: info.screen_height,
                        screen_width: info.screen_width,
                        timezone: info.time_zone.map(|offset| offset.to_string()),
                        device_fingerprint: device_fingerprint.clone(),
                    }),
                device_fingerprint,
                merchant_risk_statement: common
                    .address
                    .get_shipping()
                    .and_then(|shipping| shipping.email.clone())
                    .or_else(|| {
                        common
                            .address
                            .get_payment_billing()
                            .and_then(|billing| billing.email.clone())
                    })
                    .or_else(|| request.email.clone())
                    .map(|email_delivery_address| HipayMerchantRiskStatement {
                        email_delivery_address,
                    }),
                account_info: declared_value(carriers, "account_info"),
            }
        } else {
            HipayThreeDsData::default()
        };

        // G-Payments-02 (Card, PaymentMethodToken), evaluated BEFORE the cardtoken so the more
        // specific refusal wins and each guard stays separately observable (P-Payments-12).
        // Documented precedence is the Secure Vault domestic_network, then the vault brand,
        // then the caller-declared product, then the caller's card_network. The vault response
        // fields have no carrier on the UCS Authorize contract, so on the PaymentMethodToken
        // arm — where UCS drops the card network — the declared product is the only source.
        let declared_product = if arm_declares_product {
            declared_payment_product(request.connector_feature_data.as_ref())
                .or_else(|| declared_payment_product(request.metadata.as_ref()))
        } else {
            None
        };
        let payment_product =
            payment_product_for(None, None, declared_product.as_deref(), card_network)?;

        // G-Payments-01: the Card arm can never carry a Secure Vault cardtoken, and a blank
        // token on the PaymentMethodToken arm is the same defect; a non-empty token is the
        // money path and is accepted unchanged.
        let cardtoken = cardtoken
            .filter(|token| !token.peek().trim().is_empty())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "cardtoken",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Vault the card at POST {secondary_base_url}/create \
                             (PaymentMethodService/Tokenize) and send the returned token as the \
                             payment method token."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "HiPay rejects raw PANs on POST /v1/order and answers 'CardToken is \
                             required'."
                                .to_string(),
                        ),
                    },
                })
            })?;

        // HiPay expects a major-unit decimal string (9.99) for every amount on this call.
        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the payment currency is one HiPay's amount exponent table \
                         covers."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "HiPay's `amount` order parameter is a major-unit decimal string."
                            .to_string(),
                    ),
                },
            })?;

        // `operation` was resolved from `capture_method` at the top of this function
        // (G-Payments-04); UD-07 keeps the lowercase spelling of the enum.

        // Browser return URLs. HiPay's notify_url is the *server-to-server* notification
        // endpoint and must not be pointed at the browser return URL, or the webhook becomes
        // undeliverable. spec:### Redirect / notification URLs
        let redirect_base = request
            .complete_authorize_url
            .clone()
            .map(|url| url.replace("/redirect/complete/hipay", "/redirect/response/hipay"));

        // description is a documented required order parameter; fail closed rather than send a
        // placeholder literal. spec:### Core order fields (description Required Yes)
        let description = common.description.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "description",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send a payment description on the authorize request.".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "HiPay documents `description` as a required parameter of POST /v1/order."
                            .to_string(),
                    ),
                },
            })
        })?;

        let billing = common.address.get_payment_billing();
        let billing_details = billing.and_then(|address| address.address.as_ref());
        let shipping = common.address.get_shipping();
        let shipping_details = shipping.and_then(|address| address.address.as_ref());

        let shipping_amount = request
            .shipping_cost
            .map(|cost| item.connector.amount_converter.convert(cost, currency))
            .transpose()
            .change_context(IntegrationError::AmountConversionFailed {
                context: amount_context("shipping"),
            })?;
        let tax_amount = request
            .order_tax_amount
            .map(|tax| item.connector.amount_converter.convert(tax, currency))
            .transpose()
            .change_context(IntegrationError::AmountConversionFailed {
                context: amount_context("tax"),
            })?;

        let order_details = common
            .l2_l3_data
            .as_ref()
            .and_then(|data| data.order_info.as_ref())
            .and_then(|order| order.order_details.clone())
            .or_else(|| common.order_details.clone());

        let basket = build_basket(
            order_details.as_deref(),
            item.connector.amount_converter,
            currency,
        )?;
        let tax_rate = order_details
            .as_deref()
            .and_then(|items| items.first())
            .and_then(|line| line.tax_rate)
            .map(|rate| rate.to_string());

        Ok(Self {
            payment_product,
            orderid: common.connector_request_reference_id.clone(),
            operation,
            description,
            currency,
            amount,
            cardtoken,
            accept_url: redirect_base.clone(),
            decline_url: redirect_base.clone(),
            pending_url: redirect_base.clone(),
            cancel_url: redirect_base.clone(),
            exception_url: redirect_base,
            notify_url: request.webhook_url.clone(),
            cid: request
                .customer_id
                .as_ref()
                .map(|customer_id| customer_id.get_string_repr().to_string()),
            soft_descriptor: request
                .billing_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.statement_descriptor.clone()),
            // HiPay echoes custom_data back on the notification; the merchant's own order
            // reference is the documented payload for it. spec:## 3. orderid / merchant order reference
            custom_data: request.merchant_order_id.as_ref().map(|merchant_order_id| {
                serde_json::json!({ "merchant_order_id": merchant_order_id }).to_string()
            }),
            firstname: billing_details.and_then(|details| details.first_name.clone()),
            lastname: billing_details.and_then(|details| details.last_name.clone()),
            streetaddress: billing_details.and_then(|details| details.line1.clone()),
            streetaddress2: billing_details.and_then(|details| details.line2.clone()),
            city: billing_details.and_then(|details| details.city.clone()),
            state: billing_details.and_then(|details| details.state.clone()),
            zipcode: billing_details.and_then(|details| details.zip.clone()),
            country: billing_details.and_then(|details| details.country),
            email: billing
                .and_then(|address| address.email.clone())
                .or_else(|| request.email.clone()),
            phone: billing.and_then(|address| address.get_optional_phone_number()),
            shipto_firstname: shipping_details.and_then(|details| details.first_name.clone()),
            shipto_lastname: shipping_details.and_then(|details| details.last_name.clone()),
            shipto_recipientinfo: shipping_details
                .and_then(|details| details.get_optional_full_name()),
            shipto_house_number: shipping_details.and_then(|details| details.line3.clone()),
            shipto_streetaddress: shipping_details.and_then(|details| details.line1.clone()),
            shipto_streetaddress2: shipping_details.and_then(|details| details.line2.clone()),
            shipto_city: shipping_details.and_then(|details| details.city.clone()),
            shipto_state: shipping_details.and_then(|details| details.state.clone()),
            shipto_zipcode: shipping_details.and_then(|details| details.zip.clone()),
            shipto_country: shipping_details.and_then(|details| details.country),
            shipping: shipping_amount,
            tax: tax_amount,
            tax_rate,
            basket,
            authentication_indicator: HipayAuthenticationIndicator::from(common.auth_type),
            three_ds,
        })
    }
}

/// Error context for a failed amount conversion on one of HiPay's optional Level 2 amounts.
fn amount_context(field: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(format!(
            "Check that the {field} amount and the payment currency agree."
        )),
        doc_url: None,
        additional_context: Some(format!(
            "HiPay's `{field}` order parameter is a major-unit decimal string."
        )),
    }
}

/// Builds HiPay's `basket` form field: a JSON-encoded array of line items, in request order.
/// spec:## 5. Level 2 / Level 3 data ### Line items — the basket parameter
fn build_basket(
    order_details: Option<&[domain_types::payment_address::OrderDetailsWithAmount]>,
    amount_converter: &(dyn AmountConvertor<Output = StringMajorUnit> + Sync),
    currency: common_enums::Currency,
) -> Result<Option<String>, error_stack::Report<IntegrationError>> {
    let Some(order_details) = order_details.filter(|details| !details.is_empty()) else {
        return Ok(None);
    };

    let mut items = Vec::with_capacity(order_details.len());
    for line in order_details {
        let unit_price = amount_converter
            .convert(line.amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: amount_context("basket unit_price"),
            })?;
        let total_amount = match line.total_amount {
            Some(total) => amount_converter.convert(total, currency).change_context(
                IntegrationError::AmountConversionFailed {
                    context: amount_context("basket total_amount"),
                },
            )?,
            None => unit_price.clone(),
        };
        let discount = line
            .unit_discount_amount
            .map(|discount| amount_converter.convert(discount, currency))
            .transpose()
            .change_context(IntegrationError::AmountConversionFailed {
                context: amount_context("basket discount"),
            })?;

        items.push(HipayBasketItem {
            european_article_numbering: line.upc.clone(),
            product_reference: line.product_id.clone().or_else(|| line.sku.clone()),
            name: line.product_name.clone(),
            item_type: "good".to_string(),
            quantity: line.quantity,
            unit_price,
            discount,
            tax_rate: line.tax_rate,
            total_amount,
        });
    }

    serde_json::to_string(&items).map(Some).change_context(
        IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Check the order line items for values that cannot be JSON encoded."
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "HiPay's `basket` parameter is one form field holding a JSON array."
                        .to_string(),
                ),
            },
        },
    )
}

// Response Structures aligned with Hyperswitch
// Top-level order response fields are camelCase; the nested `order` object is snake_case.
// spec:#### 1. Create Order and Transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOrder {
    /// The merchant's own `orderid`, echoed back. Optional for the same reason
    /// [`HipayWebhookOrder::id`] is: it is a convenience echo, not the payment's identity —
    /// that is `transactionReference` (TH-16) — and HiPay quotes the whole `order` object only
    /// on the responses that have one. Read through
    /// [`HipayPaymentsResponse::response_reference_id`], which falls back to the transaction
    /// reference. Live: `"order":{"id":"pay_z9F06g0t65uJAykZERIs_1",…}` (env/ucs.log:4221).
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub amount_to_capture: Option<StringMajorUnit>,
    #[serde(default)]
    pub sca_preference: Option<String>,
}

/// AVS check result. spec:## 6. AVS ### AVS result code table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayAvsResult {
    #[serde(rename = "Y")]
    AddressAndPostalCodeMatch,
    #[serde(rename = "A")]
    AddressMatchPostalCodeMismatch,
    #[serde(rename = "P")]
    PostalCodeMatchAddressMismatch,
    #[serde(rename = "N")]
    NoMatch,
    #[serde(rename = "C")]
    NotChecked,
    #[serde(rename = "E")]
    NotApplicable,
    #[serde(rename = "U")]
    Unavailable,
    #[serde(rename = "R")]
    RetryLater,
    #[serde(rename = "S")]
    NotSupported,
    /// HiPay sends a blank string when the acquirer ran no address check at all.
    #[serde(other)]
    Blank,
}

/// CVC / CVV check result. spec:## 7. CVC / CVV ### CVC result code table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayCvcResult {
    #[serde(rename = "M")]
    Match,
    #[serde(rename = "N")]
    NoMatch,
    #[serde(rename = "P")]
    NotProcessed,
    #[serde(rename = "S")]
    ShouldHaveBeenPresent,
    #[serde(rename = "U")]
    IssuerUnableToProcess,
    /// HiPay sends a blank string when no CVC check was performed.
    #[serde(other)]
    Blank,
}

/// The vaulted instrument HiPay echoes on the order response.
///
/// Only the four elements the caller is actually given are declared. The live body also carries
/// `cardId`, `cardHolder`, `pan`, `cardExpiryMonth` and `cardExpiryYear`
/// (`grace/runs/hipay-3d6812/env/ucs.log:4221`); none of them reaches the response, and the two
/// expiry elements could never have carried a value in the first place — they were declared
/// snake_case while HiPay quotes them camelCase, so they deserialized to `None` on every
/// successful authorization this run captured. Declaring card PII only to drop it is what
/// review objects to (repeat_issues.md "Fields parsed but never used"), so they are left
/// unparsed rather than renamed; `serde` ignores unknown keys.
/// spec:#### 1. Create Order and Transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayResponsePaymentMethod {
    #[serde(default)]
    pub token: Option<Secret<String>>,
    #[serde(default)]
    pub brand: Option<String>,
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub country: Option<common_enums::CountryAlpha2>,
}

/// The recurring agreement HiPay issues for a CIT that carried `recurring_payment=1`.
/// spec:### Mandates (M2 response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayDebitAgreement {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// `threeDSecure.enrollmentStatus` — whether the card is enrolled in 3-D Secure.
/// spec:### The 3DS response block (`Y`, `N`, `U`, `E`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayEnrollmentStatus {
    /// `Y` — cardholder enrolled.
    #[serde(rename = "Y")]
    Enrolled,
    /// `N` — cardholder not enrolled.
    #[serde(rename = "N")]
    NotEnrolled,
    /// `U` — enrolment could not be determined.
    #[serde(rename = "U")]
    Undetermined,
    /// `E` — the directory server returned an error.
    #[serde(rename = "E")]
    Error,
    /// Any value HiPay adds after this integration was written.
    #[serde(other)]
    Unrecognised,
}

/// `threeDSecure.authenticationStatus` — the outcome of the authentication itself.
/// spec:### The 3DS response block (`Y`, `A`, `U`, `N`, `E`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HipayAuthenticationStatus {
    /// `Y` — authenticated (ECI 5).
    #[serde(rename = "Y")]
    Authenticated,
    /// `A` — authentication attempted (ECI 6).
    #[serde(rename = "A")]
    Attempted,
    /// `U` — unable to authenticate (ECI 7).
    #[serde(rename = "U")]
    Unable,
    /// `N` — not authenticated.
    #[serde(rename = "N")]
    NotAuthenticated,
    /// `E` — authentication error.
    #[serde(rename = "E")]
    Error,
    /// Any value HiPay adds after this integration was written.
    #[serde(other)]
    Unrecognised,
}

/// The authentication results HiPay's own MPI returns on the order response. These are the
/// **only** 3-D Secure outputs HiPay publishes: there is no `dsTransactionID` and no protocol
/// version, and the CAVV/AAV travels under the name `authenticationToken`.
/// spec:### The 3DS response block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayThreeDSecure {
    /// The ECI resulting from the authentication (distinct from the request-side `eci`, which
    /// is a channel indicator).
    #[serde(default)]
    pub eci: Option<String>,
    #[serde(default, rename = "enrollmentStatus")]
    pub enrollment_status: Option<HipayEnrollmentStatus>,
    #[serde(default, rename = "enrollmentMessage")]
    pub enrollment_message: Option<String>,
    #[serde(default, rename = "authenticationStatus")]
    pub authentication_status: Option<HipayAuthenticationStatus>,
    #[serde(default, rename = "authenticationMessage")]
    pub authentication_message: Option<String>,
    /// The CAVV / AAV carrier — HiPay publishes no field literally named `cavv`.
    #[serde(default, rename = "authenticationToken")]
    pub authentication_token: Option<Secret<String>>,
    /// The 3-D Secure transaction identifier. TH-16: carried as metadata only — the payment's
    /// single reference stays `transactionReference`.
    #[serde(default)]
    pub xid: Option<String>,
}

// Authorize Response - matches HiPay's order API response (camelCase from HiPay API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayPaymentsResponse {
    pub status: HipayPaymentStatus,
    pub message: String,
    /// The merchant order echo. **Every** nested object on this response is optional and
    /// arrives as HiPay's empty-string placeholder rather than `null` when it is absent, so it
    /// carries the one shared tolerance ([`deserialize_optional_object`]) like `reason` does.
    /// Read through [`HipayPaymentsResponse::response_reference_id`].
    #[serde(default, deserialize_with = "deserialize_optional_object")]
    pub order: Option<PaymentOrder>,
    #[serde(default)]
    #[serde(rename = "forwardUrl")]
    pub forward_url: String,
    #[serde(rename = "transactionReference")]
    pub transaction_reference: String,
    #[serde(default, deserialize_with = "deserialize_optional_reason")]
    pub reason: Option<HipayReason>,
    #[serde(default, rename = "authorizationCode")]
    pub authorization_code: Option<String>,
    #[serde(default, rename = "avsResult")]
    pub avs_result: Option<HipayAvsResult>,
    #[serde(default, rename = "cvcResult")]
    pub cvc_result: Option<HipayCvcResult>,
    #[serde(default)]
    pub eci: Option<String>,
    #[serde(
        default,
        rename = "paymentMethod",
        deserialize_with = "deserialize_optional_object"
    )]
    pub payment_method: Option<HipayResponsePaymentMethod>,
    #[serde(
        default,
        rename = "debitAgreement",
        deserialize_with = "deserialize_optional_object"
    )]
    pub debit_agreement: Option<HipayDebitAgreement>,
    /// The authentication result of HiPay's own MPI.
    ///
    /// HiPay sends `"threeDSecure": ""` — the empty **string** placeholder — on an order that
    /// ran no 3-D Secure authentication, exactly as it does for `reason`. A plain
    /// `Option<HipayThreeDSecure>` therefore fails the whole response with
    /// `invalid type: string "", expected struct HipayThreeDSecure`, which is what rejected a
    /// live `118 Captured` (`transactionReference 800454817129`, 1.00 EUR captured;
    /// `grace/runs/hipay-3d6812/env/ucs.log:4221`, error at column 1021). The same shared
    /// tolerance applies. spec:### The 3DS response block
    #[serde(
        default,
        rename = "threeDSecure",
        deserialize_with = "deserialize_optional_object"
    )]
    pub three_d_secure: Option<HipayThreeDSecure>,
}

impl HipayPaymentsResponse {
    /// `connector_response_reference_id` for every `POST /v1/order` leg — Authorize,
    /// SetupMandate and RepeatPayment.
    ///
    /// HiPay's `order[id]` is the merchant's own `orderid` echoed back, and it is present only
    /// when HiPay quotes the `order` object at all. The fallback is `transactionReference`,
    /// which is always present and is the value every later Capture / Void / Refund / PSync
    /// acts on, so a response with no `order` still reports a reference the caller can
    /// correlate — rather than failing, or reporting nothing. Consistent with TH-16: the
    /// payment's single identity stays the transaction reference.
    /// spec:#### 1. Create Order and Transaction
    pub fn response_reference_id(&self) -> String {
        self.order
            .as_ref()
            .and_then(|order| order.id.as_deref())
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| self.transaction_reference.clone())
    }

    /// `order[amount_to_capture]`, when HiPay quoted an `order` object carrying one.
    fn order_amount_to_capture(&self) -> Option<StringMajorUnit> {
        self.order
            .as_ref()
            .and_then(|order| order.amount_to_capture.clone())
    }

    /// `order[sca_preference]`, when HiPay quoted an `order` object carrying one.
    fn order_sca_preference(&self) -> Option<String> {
        self.order
            .as_ref()
            .and_then(|order| order.sca_preference.clone())
    }
}

// Generic Maintenance Response for Capture/Void/Refund operations (camelCase from HiPay API)
// spec:#### 2. Maintenance (Capture / Refund / Void) > Response 200 fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayMaintenanceResponse<S> {
    pub status: S,
    pub message: String,
    #[serde(rename = "transactionReference")]
    pub transaction_reference: String,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default, rename = "authorizationCode")]
    pub authorization_code: Option<String>,
    /// The typed optionals below carry the one shared empty-value tolerance
    /// ([`deserialize_optional_object`]) for the same reason `reason` does: HiPay renders an
    /// absent value on this body as an empty **string**, which neither `StringMajorUnit` nor
    /// `Currency` can parse, so a plain `Option<_>` fails the whole response.
    #[serde(
        default,
        rename = "authorizedAmount",
        deserialize_with = "deserialize_optional_object"
    )]
    pub authorized_amount: Option<StringMajorUnit>,
    #[serde(
        default,
        rename = "capturedAmount",
        deserialize_with = "deserialize_optional_object"
    )]
    pub captured_amount: Option<StringMajorUnit>,
    /// HiPay issues no refund-level identifier; the cumulative refunded amount is the
    /// documented handle for refund correlation.
    /// spec:### Refunds > DOCUMENTATION GAP — no refund-level identifier
    #[serde(
        default,
        rename = "refundedAmount",
        deserialize_with = "deserialize_optional_object"
    )]
    pub refunded_amount: Option<StringMajorUnit>,
    #[serde(default, deserialize_with = "deserialize_optional_object")]
    pub currency: Option<common_enums::Currency>,
    #[serde(default, deserialize_with = "deserialize_optional_reason")]
    pub reason: Option<HipayReason>,
}

impl<S> HipayMaintenanceResponse<S> {
    /// Every documented field of the maintenance 200 response, carried to the caller as
    /// `connector_metadata` so nothing parsed is left unread.
    fn connector_metadata(&self) -> Option<serde_json::Value> {
        let metadata = serde_json::json!({
            "operation": self.operation,
            "authorization_code": self.authorization_code,
            "authorized_amount": self.authorized_amount,
            "captured_amount": self.captured_amount,
            "refunded_amount": self.refunded_amount,
            "currency": self.currency,
        });
        metadata
            .as_object()
            .filter(|fields| fields.values().any(|value| !value.is_null()))
            .map(|fields| serde_json::Value::Object(fields.clone()))
    }
}

/// What `POST /v1/maintenance/transaction/{ref}` actually answers with.
///
/// HiPay returns its **error envelope** (`code` / `message` / `description`) under **HTTP 200**
/// when it will not act on a maintenance request — captured live on all three flows as
/// `{"code":"3000002","message":"Transaction not found","description":""}`
/// (`grace/runs/hipay-3d6812/env/ucs.log`, Capture / Void / Refund alike). Because the status is
/// 2xx the framework never reaches `build_error_response`, so the success shape had to parse that
/// body and could not — the caller saw `RESPONSE_DESERIALIZATION_FAILED` / grpc `Internal`
/// instead of the connector error HiPay actually sent.
///
/// The two shapes are disjoint (`transactionReference` + `status` only on the success arm, `code`
/// only on the error arm), which is what makes `untagged` unambiguous here — the same either-shape
/// tolerance [`HipayErrorCode`] already gives its integer-or-string `code`.
/// spec:## HTTP Codes and Errors ### Error Response Body Format;
/// spec:#### 2. Maintenance (Capture / Refund / Void) > Response 200 fields
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HipayMaintenanceEnvelope<S> {
    Success(HipayMaintenanceResponse<S>),
    Error(HipayErrorResponse),
}

// Type aliases for different flows - operation-specific types
pub type HipayAuthorizeResponse = HipayPaymentsResponse;
pub type HipayCaptureResponse = HipayMaintenanceEnvelope<HipayPaymentStatus>;
pub type HipayVoidResponse = HipayMaintenanceEnvelope<HipayPaymentStatus>;
pub type HipayRefundResponse = HipayMaintenanceEnvelope<HipayRefundStatus>;
pub type HipayPSyncResponse = HipaySyncResponse;
pub type HipayRSyncResponse = HipayRefundSyncResponse;

/// Does this mapped status mean the attempt failed and must be reported as an `ErrorResponse`?
///
/// One predicate for **every** path that reads a `POST /v1/order` response body — Authorize,
/// SetupMandate, RepeatPayment and the v3 PSync consultation — because the same connector status
/// answering differently on two paths is the defect this connector has already been reviewed for
/// once (review_themes.md TH-07; repeat_issues.md "Status mapping is wrong or incomplete").
///
/// `CaptureFailed` is in the set: `173 Capture Refused` arrives on the order response itself when
/// `operation=Sale` (HiPay captures in the same call), and on a v3 consultation as `73`. Without
/// it that response returns `Ok` and the parsed `reason` / `network_decline_code` are dropped —
/// the caller sees a non-charged payment with no decline detail at all. The `ErrorResponse` each
/// caller builds keeps its own flow-specific `attempt_status`, so widening this test does not
/// flatten `CaptureFailed` into a shared `Failure`.
///
/// `Expired` and `Voided` stay on the `Ok` branch: they are terminal but not declines, and HiPay
/// documents no `reason` for them.
/// spec:## Status Mappings
fn is_order_response_failure(status: AttemptStatus) -> bool {
    matches!(
        status,
        AttemptStatus::Failure | AttemptStatus::AuthenticationFailed | AttemptStatus::CaptureFailed
    )
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HipayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // A 2xx from HiPay can still carry a decline, so the failure test is on the mapped
        // status, not on the HTTP code. spec:## Status Mappings
        let is_failure = is_order_response_failure(status);

        // Every documented response field is read here: AVS, CVC, the acquirer authorization
        // code, the ECI and the vaulted instrument all reach the caller as connector metadata.
        // spec:## 6. AVS / ## 7. CVC / ## 8. Network / authorization transaction id
        let connector_metadata = serde_json::json!({
            "avs_result": item.response.avs_result,
            "cvc_result": item.response.cvc_result,
            "authorization_code": item.response.authorization_code,
            "eci": item.response.eci,
            "payment_product_brand": item
                .response
                .payment_method
                .as_ref()
                .and_then(|payment_method| payment_method.brand.clone()),
            "card_issuer": item
                .response
                .payment_method
                .as_ref()
                .and_then(|payment_method| payment_method.issuer.clone()),
            "card_issuing_country": item
                .response
                .payment_method
                .as_ref()
                .and_then(|payment_method| payment_method.country),
            "debit_agreement_id": item
                .response
                .debit_agreement
                .as_ref()
                .and_then(|agreement| agreement.id.clone()),
            "amount_to_capture": item.response.order_amount_to_capture(),
            "sca_preference": item.response.order_sca_preference(),
            // P-ThreeDS-03: the authentication results of HiPay's own MPI. `xid` and the
            // authentication token stay here — neither is the payment's identity (TH-16).
            "three_d_secure": item.response.three_d_secure.as_ref().map(|three_ds| {
                serde_json::json!({
                    "eci": three_ds.eci,
                    "enrollment_status": three_ds.enrollment_status,
                    "enrollment_message": three_ds.enrollment_message,
                    "authentication_status": three_ds.authentication_status,
                    "authentication_message": three_ds.authentication_message,
                    "authentication_token": three_ds.authentication_token,
                    "xid": three_ds.xid,
                })
            }),
        });
        let connector_metadata = connector_metadata
            .as_object()
            .filter(|fields| fields.values().any(|value| !value.is_null()))
            .map(|fields| serde_json::Value::Object(fields.clone()));

        let response = if is_failure {
            let (network_decline_code, network_error_message) =
                network_fields_from_reason(item.response.reason.as_ref());
            Err(domain_types::router_data::ErrorResponse {
                // The 7-digit HiPay reason code is what the merchant needs; the envelope
                // message is the fallback only when no reason object was returned.
                code: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.code.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.message.clone())
                    .unwrap_or_else(|| item.response.message.clone()),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                // Flow-aware: Authorize reports the status the response actually mapped to,
                // except on a soft decline — see `soft_decline_attempt_status`.
                attempt_status: Some(FlowStatus::Payment(soft_decline_attempt_status(
                    status,
                    matches!(item.response.status, HipayPaymentStatus::SoftDeclined),
                ))),
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code,
                network_advice_code: None,
                network_error_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            // UD-05: the redirect is driven off forwardUrl being non-empty, not off a state
            // literal — a completed non-3DS order returns forwardUrl as the empty string.
            let redirection_data = if item.response.forward_url.is_empty() {
                None
            } else {
                Some(Box::new(
                    domain_types::router_response_types::RedirectForm::Uri {
                        uri: item.response.forward_url.clone(),
                    },
                ))
            };

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.response_reference_id()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
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

// Tokenization Structures
// UD-01: this request is sent as application/x-www-form-urlencoded. The vault answers the
// equivalent multipart POST with an Apache-level 400 page ("Your browser sent a request that
// this server could not understand"), i.e. it is rejected before HiPay's application layer,
// while a form-urlencoded POST to the same URL with the same credentials returns 201.
// spec:#### 4. Tokenization (Secure Vault)
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayTokenRequest<T: PaymentMethodDataTypes> {
    pub card_number: domain_types::payment_method_data::RawCardNumber<T>,
    pub card_expiry_month: Secret<String>,
    pub card_expiry_year: Secret<String>,
    /// Documented Required. Taken from the card itself, never defaulted from an unrelated
    /// billing name and never sent as the empty string.
    pub card_holder: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<Secret<String>>,
    /// Required for recurring / network-token eligibility: the difference between a replayable
    /// token and a single-use one. spec:### Mandates M1
    pub multi_use: u8,
    pub generate_request_id: u8,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for HipayTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => Ok(Self {
                card_number: card_data.card_number.clone(),
                card_expiry_month: card_data.card_exp_month.clone(),
                // HiPay types card_expiry_year as a full year; a caller's 2-digit value would
                // vault "25" instead of "2025" and the instrument would read as long expired.
                // spec:#### 4. Tokenization (Secure Vault)
                card_expiry_year: card_data.get_expiry_year_4_digit(),
                card_holder: card_data.card_holder_name.clone().ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "card_holder",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Send card.card_holder_name on the tokenize request.".to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "HiPay documents card_holder as Required on POST \
                                 {secondary_base_url}/create."
                                    .to_string(),
                            ),
                        },
                    })
                })?,
                cvc: Some(card_data.card_cvc.clone()),
                multi_use: 1,
                generate_request_id: 1,
            }),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotImplemented(
                    "HiPay supports card payments only; this payment method cannot be vaulted \
                     at POST {secondary_base_url}/create"
                        .to_string(),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Route this payment method to a connector that offers it.".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "HiPay's Secure Vault stores card credentials only.".to_string(),
                        ),
                    },
                )))
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HipayTokenResponse {
    pub token: Secret<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub card_id: Option<String>,
    pub brand: String,
    /// The co-badge HiPay itself resolved; it takes precedence over `brand` when replayed as
    /// `payment_product`. spec:#### 4. Tokenization (Secure Vault) > Note
    #[serde(default)]
    pub domestic_network: Option<String>,
    pub pan: Secret<String>,
    pub card_holder: Secret<String>,
    pub card_expiry_month: Secret<String>,
    pub card_expiry_year: Secret<String>,
    pub issuer: Option<String>,
    pub country: Option<common_enums::CountryAlpha2>,
    #[serde(default)]
    pub card_type: Option<String>,
    #[serde(default)]
    pub card_category: Option<String>,
    #[serde(default)]
    pub forbidden_issuer_country: Option<bool>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayTokenResponse, Self>) -> Result<Self, Self::Error> {
        use hyperswitch_masking::ExposeInterface;
        // The vault's own view of the card network — domestic_network first, then brand — is
        // what HiPay expects replayed as payment_product on the order call, so it is carried
        // back rather than dropped. spec:#### 4. Tokenization (Secure Vault) > Note
        let resolved_network = item
            .response
            .domestic_network
            .clone()
            .filter(|network| !network.trim().is_empty())
            .unwrap_or_else(|| item.response.brand.clone());
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.token.expose(),
                connector_payment_method_id: Some(resolved_network),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Helper function to map v3 API integer status codes to AttemptStatus
// Matches Hyperswitch's get_sync_status function
fn get_sync_status(state: i32) -> AttemptStatus {
    match state {
        9 => AttemptStatus::AuthenticationFailed,
        10 => AttemptStatus::Failure,
        11 => AttemptStatus::Failure,
        12 => AttemptStatus::Pending,
        13 => AttemptStatus::Failure,
        // v3 `14` is status `114 Expired`, which `HipayPaymentStatus::Expired` maps to
        // `AttemptStatus::Expired` on the order and notification paths. An authorization that
        // lapsed before capture is not a decline, and the same connector status must not have
        // two answers on two paths. spec:## Status Mappings ### Notified statuses
        14 => AttemptStatus::Expired,
        15 => AttemptStatus::Voided,
        16 => AttemptStatus::Authorized,
        17 => AttemptStatus::CaptureInitiated,
        18 => AttemptStatus::Charged,
        19 => AttemptStatus::PartialCharged,
        29 => AttemptStatus::Failure,
        73 => AttemptStatus::CaptureFailed,
        74 => AttemptStatus::Pending,
        75 => AttemptStatus::VoidInitiated,
        77 => AttemptStatus::AuthenticationPending,
        78 => AttemptStatus::Failure,
        200 => AttemptStatus::Pending,
        1 => AttemptStatus::Started,
        // P-ThreeDS-04: the authentication arms, identical to the `HipayPaymentStatus` map the
        // Authorize path uses — a status mapping applied on one emitting path and not the
        // others is the operator's most frequent review finding.
        // spec:## Status Mappings; spec:### ThreeDS > Authentication statuses
        3 => AttemptStatus::AuthenticationPending,
        4 => AttemptStatus::AuthenticationPending,
        5 => AttemptStatus::AuthenticationFailed,
        6 => AttemptStatus::AuthenticationSuccessful,
        7 => AttemptStatus::AuthenticationPending,
        8 => AttemptStatus::AuthenticationFailed,
        60 => AttemptStatus::AuthenticationPending,
        20 => AttemptStatus::Charged,
        21 => AttemptStatus::Charged,
        22 => AttemptStatus::Charged,
        23 => AttemptStatus::Charged,
        40 => AttemptStatus::AuthenticationPending,
        41 => AttemptStatus::AuthenticationSuccessful,
        51 => AttemptStatus::Failure,
        61 => AttemptStatus::Pending,
        63 => AttemptStatus::Failure,
        // The v3 consultation reports the same statuses as the order API with the leading `1`
        // dropped. These are the codes this integration added to `HipayPaymentStatus`; without
        // them PSync answers `Unknown` for a transaction HiPay considers finished — a chargeback
        // or a settled cardholder credit would never surface on the sync path. Each mapping is
        // the one `From<HipayPaymentStatus> for AttemptStatus` already gives its 1xx form, so
        // the sync and order paths cannot disagree. spec:## Status Mappings
        31 | 32 => AttemptStatus::Pending, // 131 Debited / 132 Partially Debited
        34 => AttemptStatus::Failure,      // 134 Dispute Lost
        42 => AttemptStatus::Pending,      // 142 Authorization Requested
        43 => AttemptStatus::Voided,       // 143 Authorization Cancelled
        44 => AttemptStatus::Pending,      // 144 Reference Rendered
        50 => AttemptStatus::Pending,      // 150 Acquirer Found
        66 | 68 => AttemptStatus::Charged, // 166 / 168 Debited (cardholder credit)
        69 => AttemptStatus::Pending,      // 169 Credit Requested
        72 => AttemptStatus::Pending,      // 172 In Progress
        // 180 Partially Chargeback / 181 Chargeback. The RDR *refund* pair 182 / 183 is
        // deliberately absent: it belongs to `get_refund_sync_status`, where it already is.
        80 | 81 => AttemptStatus::Failure,
        // A code this table does not list is a documented-unknown, not a decline: the caller
        // keeps the status the attempt already had rather than terminally failing a payment
        // that may well be Charged. review_themes.md TH-07
        _ => AttemptStatus::Unknown,
    }
}

/// `(network_decline_code, network_error_message)` from the v3 consultation `reason` object.
/// Same `40xxxxx` range rule as [`network_fields_from_reason`].
/// spec:#### 3. Transaction Lookup (PSync / RSync)
fn network_fields_from_sync_reason(reason: &Reason) -> (Option<String>, Option<String>) {
    match reason.code {
        Some(code) if (4_000_000..5_000_000).contains(&code) => (
            Some(code.to_string()),
            reason.reason.clone().or_else(|| Some(String::new())),
        ),
        _ => (None, None),
    }
}

// Payment Sync Response Implementation
// Uses HipaySyncResponse enum with v3 API flat structure
impl TryFrom<ResponseRouterData<HipayPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayPSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Handle sync response - could be Response or Error variant
        // TH-16: the transaction reference the caller already holds is the single identity of
        // this payment across Authorize, Capture, Void, Refund and PSync. The v3 consultation
        // `id` is the consultation service's own identifier, so restating it here would give
        // one payment two references.
        let transaction_reference = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .ok();

        match item.response {
            HipaySyncResponse::Response {
                id, status, reason, ..
            } => {
                // Convert i32 status code to AttemptStatus using mapping function
                let attempt_status = get_sync_status(status);
                let (network_decline_code, network_error_message) =
                    network_fields_from_sync_reason(&reason);

                let connector_metadata = serde_json::json!({
                    "hipay_consultation_id": id,
                    "reason_code": reason.code,
                    "reason": reason.reason,
                });

                // Same predicate as every other path that reads a HiPay transaction status, so
                // a `73` (173 Capture Refused) consultation reports its decline detail here too
                // instead of answering `Ok` with the reason only in `connector_metadata`.
                let response = if is_order_response_failure(attempt_status) {
                    Err(domain_types::router_data::ErrorResponse {
                        code: reason
                            .code
                            .map(|code| code.to_string())
                            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                        message: reason
                            .reason
                            .clone()
                            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                        reason: reason.reason.clone(),
                        status_code: item.http_code,
                        // Same soft-decline rule as the Authorize path (P-ThreeDS-04): v3 `78`
                        // is status 178, and the retry-with-3DS signal must survive here too.
                        attempt_status: Some(FlowStatus::Payment(soft_decline_attempt_status(
                            attempt_status,
                            status == 78,
                        ))),
                        connector_transaction_id: transaction_reference.clone(),
                        network_decline_code,
                        network_advice_code: None,
                        network_error_message,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    })
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: transaction_reference
                            .clone()
                            .map(ResponseId::ConnectorTransactionId)
                            .unwrap_or(ResponseId::NoResponseId),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: Some(connector_metadata),
                        network_txn_id: None,
                        network_txn_link_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    })
                };

                Ok(Self {
                    response,
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            // TH-07: a request-level v3 error (bad reference, service unavailable) says nothing
            // about the payment, so the attempt keeps the status it came in with instead of
            // being terminally failed.
            HipaySyncResponse::Error { message, code } => {
                let incoming_status = item.router_data.resource_common_data.status;
                Ok(Self {
                    response: Err(domain_types::router_data::ErrorResponse {
                        code: code.to_string(),
                        message: message.clone(),
                        reason: Some(message),
                        status_code: item.http_code,
                        attempt_status: Some(FlowStatus::Payment(incoming_status)),
                        connector_transaction_id: transaction_reference,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                    ..item.router_data
                })
            }
        }
    }
}

// Capture Request Structure
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayCaptureRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for HipayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Always send `amount` + `currency`, so HiPay settles exactly the amount the caller
        // asked for. spec:#### 2. Maintenance (Capture / Refund / Void).
        //
        // Why this, and not the previous "omit the amount on a full capture" (B039):
        //
        // The capture contract cannot prove a capture is full. `PaymentsCaptureData` carries
        // `minor_amount_to_capture` but never the amount that was authorized
        // (`domain_types/src/connector_types.rs:3687-3704`), and
        // `PaymentFlowData::minor_amount_authorized` is a response-reporting field that every
        // request-path constructor sets to `None` (`domain_types/src/types.rs`, the Capture
        // constructor at `:12073` among them). `multiple_capture_data` only arrives when the
        // caller explicitly asks for a *multiple* capture, so a **lone** partial capture — a
        // plain `Manual` capture for less than the authorization — carried neither signal and
        // was indistinguishable from a full one. Omitting the amount on it made HiPay settle the
        // whole authorization: live references `800454817220` / `800454817221` were captured for
        // 60.00 EUR after Hyperswitch asked for 20.00 and recorded `amount_received` 2000, so the
        // two ledgers disagreed by 40.00 EUR and the suite still reported success.
        //
        // Sending the amount unconditionally removes the undecidable branch instead of guessing
        // at it: a full capture is simply a partial capture of the full amount. HiPay's own
        // `amount` parameter is documented "Required for partial operations; omit for a full
        // capture" — *omit*, not *forbidden* — and Hyperswitch's own shipped HiPay connector
        // sends `amount` + `currency` on every capture
        // (`hs:crates/hyperswitch_connectors/src/connectors/hipay/transformers.rs:430-439`),
        // which is the parity report's D-10.
        //
        // The one documented rejection, `3020103 "You cannot partially capture this type of
        // transaction"` (spec:#### Maintenance operations (capture / refund / void)), is keyed on
        // the **transaction type**, not on the amount being equal to the authorization. On a type
        // that cannot be partially captured it is the *correct* answer to a partial capture, and
        // HiPay returns it in the ERROR envelope that the response `TryFrom` below already maps
        // to a `CaptureFailed` `ErrorResponse`. So every outcome is now truthful: either HiPay
        // settles precisely the requested amount, or it refuses and the caller sees an error.
        // The previous behaviour was the only one that could report success while moving a
        // different amount of money. Verified live in this run — see the run's decisions file.
        //
        // Unit: `minor_amount_to_capture` is minor units, scoped to this one capture call;
        // HiPay's wire `amount` is a major-unit decimal string, so it goes through the
        // connector's single framework converter rather than any local arithmetic.
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: amount_context("capture amount"),
            })?;

        Ok(Self {
            operation: HipayOperation::Capture,
            // Sent together with `amount`: HiPay requires the currency to match the original
            // authorization and answers `3020105 Currency Mismatch` when it does not.
            currency: Some(item.router_data.request.currency),
            amount: Some(amount),
        })
    }
}

// Capture Response Implementation
// Uses HipayMaintenanceResponse<HipayPaymentStatus> with direct enum conversion
impl TryFrom<ResponseRouterData<HipayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayCaptureResponse, Self>) -> Result<Self, Self::Error> {
        // HiPay answers a maintenance POST it will not act on with its ERROR envelope under
        // HTTP 200, so `build_error_response` is never reached; map that envelope to the
        // connector error it is instead of failing to deserialize it (B030).
        let item = match item.response {
            HipayMaintenanceEnvelope::Error(error) => {
                return Ok(Self {
                    response: Err(error.into_error_response(
                        item.http_code,
                        FlowStatus::Payment(AttemptStatus::CaptureFailed),
                        None,
                    )),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::CaptureFailed,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            HipayMaintenanceEnvelope::Success(response) => ResponseRouterData {
                response,
                router_data: item.router_data,
                http_code: item.http_code,
            },
        };

        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // Check if status indicates failure
        let response = if status == AttemptStatus::Failure || status == AttemptStatus::CaptureFailed
        {
            let (network_decline_code, network_error_message) =
                network_fields_from_reason(item.response.reason.as_ref());
            Err(domain_types::router_data::ErrorResponse {
                code: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.code.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.message.clone())
                    .unwrap_or_else(|| item.response.message.clone()),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                // Flow-aware: a failed capture is CaptureFailed, never a shared Failure.
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code,
                network_advice_code: None,
                network_error_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: item.response.connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        let status = if status == AttemptStatus::Failure {
            AttemptStatus::CaptureFailed
        } else {
            status
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

/// Refund Request Structure — `operation=refund` on the shared maintenance endpoint.
///
/// `amount` and `currency` are **partial-refund only**: the maintenance guide is explicit that a
/// full refund sends `operation=refund` alone, and sending an amount unconditionally risks
/// `3020203` "You cannot partially refund this type of transaction" (P-Refunds-01).
/// `amount` is a **major-unit** decimal string (`StringMajorUnit`), produced from the caller's
/// `MinorUnit` by the connector's `amount_converter`, never by hand.
/// spec:### Refunds; spec:#### 2. Maintenance (Capture / Refund / Void)
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayRefundRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
    /// The merchant's own reference for this individual refund. HiPay issues no refund-level
    /// identifier, and `operation_id` is the **only** documented correlation handle: it is echoed
    /// back in the `operation` element of the notification, which is how a partial refund is
    /// matched to the request that created it (P-Refunds-02, UD-04).
    /// spec:#### 2. Maintenance (operation_id); spec:### IncomingWebhook (Refund webhooks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for HipayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // P-Refunds-01: a full refund is `operation=refund` **alone**; `amount` + `currency` are
        // added only when the caller asks for less than the payment amount. Always sending an
        // amount risks 3020203 "You cannot partially refund this type of transaction".
        // spec:### Refunds (Full refund: send operation=refund only)
        let is_partial_refund = item.router_data.request.minor_refund_amount
            < item.router_data.request.minor_payment_amount;

        // Minor -> major-unit decimal string, through the framework converter only.
        let amount = if is_partial_refund {
            Some(
                item.connector
                    .amount_converter
                    .convert(
                        item.router_data.request.minor_refund_amount,
                        item.router_data.request.currency,
                    )
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: amount_context("refund amount"),
                    })?,
            )
        } else {
            None
        };

        Ok(Self {
            operation: HipayOperation::Refund,
            currency: amount.as_ref().map(|_| item.router_data.request.currency),
            amount,
            // P-Refunds-02: the caller's refund id is HiPay's only correlation handle for an
            // individual refund. An empty one is omitted rather than sent blank.
            operation_id: Some(item.router_data.request.refund_id.clone())
                .filter(|refund_id| !refund_id.trim().is_empty()),
        })
    }
}

/// v3 consultation refund statuses.
///
/// The consultation service quotes HiPay's `1xx` transaction statuses with the leading `1`
/// dropped — 118 Captured arrives as `18`, see [`get_sync_status`] — so the refund band
/// 124 / 125 / 126 / 165 / 182 / 183 arrives as 24 / 25 / 26 / 65 / 82 / 83. The table is the
/// same one [`HipayRefundStatus`] applies to the maintenance response, so the two emitting paths
/// cannot drift apart (P-Refunds-04).
/// spec:### Refunds (Success statuses 124/125/126; RDR refunds arrive as 182 / 183; 165 Refund Refused)
fn get_refund_sync_status(state: i32) -> RefundStatus {
    match state {
        24 => RefundStatus::Pending,
        // 182 / 183 are RDR-initiated refunds and are settled refunds like 125 / 126.
        25 | 26 | 82 | 83 => RefundStatus::Success,
        65 => RefundStatus::Failure,
        // TH-07 / P-Refunds-04: a code this table does not list is a documented-unknown, not a
        // default. The former `_ => Pending` made an unmapped refund sync forever; `Unknown` is
        // the unspecified variant, so the caller keeps the status the refund already had.
        _ => RefundStatus::Unknown,
    }
}

/// Does HiPay's cumulative `refundedAmount` justify a **terminal** refund success?
///
/// HiPay issues no refund-level identifier and its lookup returns a single aggregate
/// `refundedAmount` with no per-operation array, so an individual refund's state cannot be read
/// back directly. The aggregate is still a valid lower bound: the cumulative refunded amount can
/// never be smaller than a single refund that has actually settled. A mapped success whose
/// `refundedAmount` does not cover the amount asked for is therefore reported as `Pending` —
/// non-terminal, so the caller re-syncs — rather than as a settled refund nothing on the wire
/// confirms. This is the check that keeps `refundedAmount` from being parsed and never read.
///
/// Both sides are money: the wire value is a major-unit decimal string and the request carries a
/// `MinorUnit`, so they are compared as `MinorUnit` through the framework converter — never as
/// floats and never as strings.
/// spec:### Refunds > DOCUMENTATION GAP — no refund-level identifier
fn refunded_amount_covers_request(
    refunded_amount: Option<&StringMajorUnit>,
    requested: MinorUnit,
    currency: common_enums::Currency,
) -> bool {
    refunded_amount
        .and_then(|amount| {
            StringMajorUnitForConnector
                .convert_back(amount.clone(), currency)
                .ok()
        })
        .is_some_and(|refunded| refunded >= requested)
}

// Refund Response Implementation
// Uses HipayMaintenanceResponse<HipayRefundStatus> with From trait conversion
impl TryFrom<ResponseRouterData<HipayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayRefundResponse, Self>) -> Result<Self, Self::Error> {
        // HiPay answers a maintenance POST it will not act on with its ERROR envelope under
        // HTTP 200, so `build_error_response` is never reached; map that envelope to the
        // connector error it is instead of failing to deserialize it (B032).
        let item = match item.response {
            HipayMaintenanceEnvelope::Error(error) => {
                return Ok(Self {
                    response: Err(error.into_error_response(
                        item.http_code,
                        FlowStatus::Refund(RefundStatus::Failure),
                        None,
                    )),
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            HipayMaintenanceEnvelope::Success(response) => ResponseRouterData {
                response,
                router_data: item.router_data,
                http_code: item.http_code,
            },
        };

        // Convert HipayRefundStatus enum directly to RefundStatus using From trait
        let mapped_status = RefundStatus::from(item.response.status.clone());

        // P-Refunds-05 / TH-16: HiPay's maintenance response echoes the ORIGINAL
        // transactionReference — it is not a new refund id — and that is the one identifier
        // Execute and RSync both use, so the id a refund is created with is the id it syncs back
        // with. spec:### Refunds > DOCUMENTATION GAP — no refund-level identifier
        let connector_refund_id = item.response.transaction_reference.clone();

        // A terminal success needs the cumulative refundedAmount to cover what was asked for;
        // otherwise the refund stays Pending and the caller re-syncs.
        let refund_status = if mapped_status == RefundStatus::Success
            && !refunded_amount_covers_request(
                item.response.refunded_amount.as_ref(),
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            ) {
            RefundStatus::Pending
        } else {
            mapped_status
        };

        let response = if refund_status == RefundStatus::Failure {
            // P-Refunds-03: a 2xx carrying 165 Refund Refused is a *failure*. Returning it as
            // Ok with a Failure status dropped `reason` entirely, leaving the merchant a failed
            // refund with no cause — the reviewer finding in repeat_issues.md, and the same hole
            // Hyperswitch has. spec:### Transaction-level decline detail (reason object)
            let (network_decline_code, network_error_message) =
                network_fields_from_reason(item.response.reason.as_ref());
            Err(domain_types::router_data::ErrorResponse {
                code: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.code.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.message.clone())
                    .unwrap_or_else(|| item.response.message.clone()),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                // Flow-aware and refund-specific: a refused refund is RefundStatus::Failure,
                // never a shared hardcoded payment failure.
                attempt_status: Some(FlowStatus::Refund(RefundStatus::Failure)),
                connector_transaction_id: Some(connector_refund_id.clone()),
                network_decline_code,
                network_advice_code: None,
                network_error_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
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

// Refund Sync Response Implementation
// Uses HipayRefundSyncResponse JSON structure from v3 API
impl TryFrom<ResponseRouterData<HipayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let mapped_status = get_refund_sync_status(item.response.status);

        // P-Refunds-05 / HP-19: the refund is reported back under the SAME identifier Execute
        // wrote — the original transaction reference the sync was addressed with — not under the
        // v3 consultation's own numeric `id`. Writing two different ids for one refund is the
        // defect Hyperswitch has here. TH-16
        let connector_refund_id = item.router_data.request.connector_refund_id.clone();

        // The refund state is interpreted from `status` + the aggregate `refunded_amount`
        // (UD-04). `refund_money` is the amount this sync is asking about; when the caller did
        // not supply it there is nothing to compare against and the status stands on its own.
        let refund_status = match item.router_data.request.refund_money.as_ref() {
            Some(money)
                if mapped_status == RefundStatus::Success
                    && !refunded_amount_covers_request(
                        item.response.refunded_amount.as_ref(),
                        money.amount,
                        money.currency,
                    ) =>
            {
                RefundStatus::Pending
            }
            _ => mapped_status,
        };

        let response = if refund_status == RefundStatus::Failure {
            // 165 Refund Refused on the sync path carries its cause in the v3 `reason` object;
            // dropping it would leave a refused refund with no reason, exactly as the Execute
            // path used to. spec:#### 3. Transaction Lookup (PSync / RSync)
            let sync_reason = item.response.reason.as_ref();
            let (network_decline_code, network_error_message) = sync_reason
                .map(network_fields_from_sync_reason)
                .unwrap_or((None, None));
            Err(domain_types::router_data::ErrorResponse {
                code: sync_reason
                    .and_then(|reason| reason.code)
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: sync_reason
                    .and_then(|reason| reason.reason.clone())
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: sync_reason.and_then(|reason| reason.reason.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Refund(RefundStatus::Failure)),
                connector_transaction_id: Some(connector_refund_id.clone()),
                network_decline_code,
                network_advice_code: None,
                network_error_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            })
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

// Void Request Structure
// P-Void-01: the documented full-cancel body is `operation=cancel` alone. HiPay documents no
// partial cancel, so neither `amount` nor `currency` is ever written on this path.
// spec:### Void; spec:#### 2. Maintenance (Capture / Refund / Void)
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayVoidRequest {
    pub operation: HipayOperation,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for HipayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // G-Void-02 (B038): HiPay documents no partial cancel — the maintenance guide's only
        // documented cancel body is `operation=cancel` alone, and this transformer always emits
        // exactly that. The guard therefore refuses a *provably* partial cancel and nothing else.
        //
        // The earlier predicate — "the request carries a non-zero amount" — was wider than the
        // invariant it assumed and made Void unreachable from Hyperswitch. HS writes
        // `amount: Some(Money { minor_amount: request.amount.unwrap_or_default(), currency })` on
        // *every* void (hs:crates/router/src/core/unified_connector_service/transformers.rs
        // :8194-8197), and on a full cancel that amount is the full authorized amount, so its
        // presence is no evidence of partiality. UCS answered such a void with gRPC 12
        // UNIMPLEMENTED before any HTTP request was built, which left real authorizations held at
        // status 116 Authorized (B038: 800454817222, 800454817223).
        //
        // Partiality cannot be derived from a non-zero amount, because nothing on the Void
        // contract carries the amount that was authorized to compare it against:
        // `PaymentVoidData` has no authorized amount (ucs:crates/types-traits/domain_types/src/
        // connector_types.rs:1604-1616) and the Void request-path constructor sets
        // `PaymentFlowData::minor_amount_authorized` to `None`
        // (ucs:crates/types-traits/domain_types/src/types.rs:6290). The comparison below is the
        // only shape that *proves* a partial cancel, and it starts binding the day the framework
        // populates the authorized amount; until then every void is issued as the full cancel
        // Hyperswitch is asking for, which is the correct semantics for an uncaptured
        // authorization.
        // spec:### Void > DOCUMENTATION GAP — partial cancel
        let provably_partial_cancel = match (
            item.router_data.request.amount,
            item.router_data
                .resource_common_data
                .minor_amount_authorized,
        ) {
            (Some(requested), Some(authorized)) => {
                requested.get_amount_as_i64() > 0 && requested < authorized
            }
            _ => false,
        };

        if provably_partial_cancel {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "HiPay documents no partial cancel; a Void is full-amount only"
                    .to_string(),
                connector: "hipay",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the void for the full authorized amount to cancel the whole \
                         authorization, or capture the amount you want to keep and refund the \
                         remainder."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "POST /v1/maintenance/transaction/{transactionReference} accepts amount \
                         and currency only for the documented partial capture and partial \
                         refund operations, never for operation=cancel: this request asked to \
                         cancel less than the authorized amount, which HiPay cannot express."
                            .to_string(),
                    ),
                },
            }));
        }

        Ok(Self {
            operation: HipayOperation::Cancel,
        })
    }
}

// Void Response Implementation
// Uses HipayMaintenanceResponse<HipayPaymentStatus> with direct enum conversion
impl TryFrom<ResponseRouterData<HipayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayVoidResponse, Self>) -> Result<Self, Self::Error> {
        // HiPay answers a maintenance POST it will not act on with its ERROR envelope under
        // HTTP 200, so `build_error_response` is never reached; map that envelope to the
        // connector error it is instead of failing to deserialize it (B031).
        let item = match item.response {
            HipayMaintenanceEnvelope::Error(error) => {
                return Ok(Self {
                    response: Err(error.into_error_response(
                        item.http_code,
                        FlowStatus::Payment(AttemptStatus::VoidFailed),
                        None,
                    )),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::VoidFailed,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            HipayMaintenanceEnvelope::Success(response) => ResponseRouterData {
                response,
                router_data: item.router_data,
                http_code: item.http_code,
            },
        };

        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // Check if status indicates void failure
        let response = if status == AttemptStatus::Failure || status == AttemptStatus::VoidFailed {
            let (network_decline_code, network_error_message) =
                network_fields_from_reason(item.response.reason.as_ref());
            Err(domain_types::router_data::ErrorResponse {
                code: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.code.clone())
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .reason
                    .as_ref()
                    .map(|reason| reason.message.clone())
                    .unwrap_or_else(|| item.response.message.clone()),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                // Flow-aware: a failed void is VoidFailed, never a shared Failure.
                attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code,
                network_advice_code: None,
                network_error_message,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: item.response.connector_metadata(),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
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

// =========================================================================================
// Mandates — SetupMandate (initial CIT) and RepeatPayment (MIT)
//
// HiPay has no mandate service: every leg is `POST /v1/order` over the same multipart form the
// Authorize flow uses. The CIT is an ordinary card-on-file authorization marked `eci=7` and
// `recurring_payment=1`; HiPay answers it with a `debitAgreement`, whose `id` a later MIT
// replays as `debit_agreement_id` with `eci=9`. There is no `/mandate`, `/subscription` or
// `/verify` path anywhere in HiPay's published specifications.
// spec:### Mandates (M1–M6); spec:### API Call Sequences ### Mandates (a) and (c)
// =========================================================================================

/// The request-side `eci` (Electronic Commerce Indicator) values HiPay publishes.
///
/// This is a **channel** indicator, not a 3-D Secure result ECI: `7` marks the initial
/// customer-initiated transaction of a card-on-file agreement (and a subsequent one-click
/// payment), `9` marks a merchant-initiated recurring charge against an existing agreement. A
/// value outside the published set is rejected with `1020002 Unsupported ECI`, which is why this
/// is an enum rather than an integer field.
/// spec:### ECI code table (request-side values); spec:## 11. ... #### eci (request side)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipayEci {
    /// `1` — mail order.
    MailOrder,
    /// `2` — telephone order.
    TelephoneOrder,
    /// `7` — secure e-commerce: the initial CIT of a card-on-file agreement, and one-click.
    SecureEcommerce,
    /// `9` — recurring e-commerce: a merchant-initiated charge on an existing agreement.
    RecurringEcommerce,
    /// `10` — instalment payment.
    Instalment,
}

impl HipayEci {
    /// The wire value. HiPay types `eci` as an integer on the request side.
    /// spec:## 11. ... #### eci (request side)
    const fn code(self) -> u8 {
        match self {
            Self::MailOrder => 1,
            Self::TelephoneOrder => 2,
            Self::SecureEcommerce => 7,
            Self::RecurringEcommerce => 9,
            Self::Instalment => 10,
        }
    }
}

impl Serialize for HipayEci {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.code())
    }
}

/// A HiPay boolean order parameter. HiPay spells these as the integers `0` and `1`, never as
/// `true`/`false`, so the flag is typed rather than written as a bare literal at each use.
/// spec:### Mandates (M2, M3, M4 — `recurring_payment=1`, `one_click=1`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipayFlag {
    /// `0` — off.
    Off,
    /// `1` — on.
    On,
}

impl Serialize for HipayFlag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(match self {
            Self::Off => 0,
            Self::On => 1,
        })
    }
}

/// `recurring_info` — the schedule HiPay attaches to the agreement the initial CIT opens.
///
/// DOCUMENTATION GAP: the object is named on HiPay's recurring / card-on-file guide (an
/// expiration date and a frequency) but has **no** field-level schema in `gateway.yaml`, so only
/// the two sub-fields the guide names are ever sent, and only when the caller actually supplied
/// mandate details. It travels in bracket notation (`recurring_info[frequency]`) like every other
/// nested HiPay object. spec:### Mandates > DOCUMENTATION GAP — `recurring_info` field schema; UD-06
#[derive(Debug, Serialize)]
pub struct HipayRecurringInfo {
    /// The last date the agreement may be charged, as an ISO-8601 calendar date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
    /// The billing period the caller declared (`daily`, `weekly`, `monthly`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
}

impl HipayRecurringInfo {
    /// Builds `recurring_info` from the caller's mandate details, or `None` when the caller
    /// declared neither an end date nor a frequency — an empty object would be noise HiPay has
    /// no schema for. spec:### Mandates (M2)
    fn from_mandate_data(mandate_data: Option<&MandateData>) -> Option<Self> {
        let amount_data = match mandate_data.and_then(|data| data.mandate_type.as_ref())? {
            MandateDataType::SingleUse(amount_data) => Some(amount_data),
            MandateDataType::MultiUse(amount_data) => amount_data.as_ref(),
        }?;
        let expiration_date = amount_data.end_date.map(|end| end.date().to_string());
        let frequency = amount_data
            .frequency
            .as_ref()
            .filter(|frequency| !frequency.trim().is_empty())
            .cloned();
        if expiration_date.is_none() && frequency.is_none() {
            return None;
        }
        Some(Self {
            expiration_date,
            frequency,
        })
    }
}

/// G-Mandates-03 — HiPay chains a merchant-initiated transaction by its own **debit agreement**,
/// not by a scheme transaction id. HiPay calls the scheme id SRD (Scheme Reference Data) with a
/// companion TLID, and the only documented carrier for one obtained elsewhere is
/// `POST /v3/debit-agreement/{payment_product}` — an endpoint with no published schema in any
/// HiPay OpenAPI file. `POST /v1/order` has no request parameter for an SRD/NTID in any casing,
/// so a caller-supplied network transaction id is refused here rather than silently dropped: an
/// MIT sent without the credential the caller asked us to chain on is one the scheme cannot link.
/// spec:## 8. Network / authorization transaction id ### The scheme / network transaction id
/// (NOT SUPPORTED BY HIPAY (Gateway API v1)); spec:### Mandates (M5)
fn refuse_network_transaction_id() -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: "HiPay chains MIT by debit agreement, not by scheme transaction id; POST \
                  /v1/order has no request field for an NTID"
            .to_string(),
        connector: "hipay",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Set the mandate up through PaymentService/SetupRecurring so HiPay issues its own \
                 debit agreement, and replay that agreement id on the recurring charge."
                    .to_string(),
            ),
            doc_url: None,
            additional_context: Some(
                "HiPay's equivalent of a network transaction id is Scheme Reference Data (SRD), \
                 accepted only on the unpublished POST /v3/debit-agreement/{payment_product}; it \
                 is not a parameter of POST /v1/order and is not returned on the V1 order \
                 response."
                    .to_string(),
            ),
        },
    })
}

/// True when the caller handed us a scheme / network transaction id instead of (or alongside) a
/// HiPay handle. Both the payment-method arms that exist only to carry an NTID and a
/// `mandate_id` whose reference is a network id count. spec:## 8. Network / authorization transaction id
fn carries_network_transaction_id<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
    mandate_id: Option<&MandateIds>,
) -> bool {
    let from_payment_method = matches!(
        payment_method_data,
        PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
    );
    let from_mandate_id = matches!(
        mandate_id.and_then(|ids| ids.mandate_reference_id.as_ref()),
        Some(MandateReferenceId::NetworkMandateId(_))
            | Some(MandateReferenceId::NetworkTokenWithNTI(_))
    );
    from_payment_method || from_mandate_id
}

/// The initial customer-initiated transaction that opens a HiPay debit agreement.
///
/// UD-03: HiPay publishes **no** zero-amount authorization — the `operation` enum is only
/// `Sale | Authorization` and `amount` is a required order parameter with a documented minimum of
/// 1 — so SetupMandate is a real-amount CIT, and a zero amount is refused by G-Mandates-01 rather
/// than sent as an invented verification call.
/// spec:### Mandates (M2); spec:### Mandates > DOCUMENTATION GAP — no zero-amount authorization
#[derive(Debug, Serialize)]
pub struct HipaySetupMandateRequest {
    pub payment_product: HipayPaymentProduct,
    pub orderid: String,
    pub operation: Operation,
    pub description: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    /// The **multi-use** Secure Vault token. A single-use token cannot be replayed, so
    /// `multi_use=1` on the vault call is the precondition of the whole mandate family.
    /// spec:### Mandates M1
    pub cardtoken: Secret<String>,
    /// `7` — the initial CIT of a card-on-file agreement.
    pub eci: HipayEci,
    /// `1` — asks HiPay to open a debit agreement for this card.
    pub recurring_payment: HipayFlag,
    /// Derived from the request's `authentication_type`, exactly as on the Authorize call.
    pub authentication_indicator: HipayAuthenticationIndicator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_info: Option<HipayRecurringInfo>,
    /// Overrides the account default **server-to-server** notification URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HipaySetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let common = &item.router_data.resource_common_data;
        let currency = request.currency;

        // G-Mandates-03 (Card, MandatePayment, PaymentMethodToken) — evaluated before the arm
        // match so the NTID refusal wins over the generic payment-method refusal: the caller
        // asked for a scheme-chained mandate, and saying "payment method not implemented" would
        // hide the real reason.
        if carries_network_transaction_id(&request.payment_method_data, request.mandate_id.as_ref())
        {
            return Err(refuse_network_transaction_id());
        }

        // G-Mandates-01 (Card, MandatePayment, PaymentMethodToken) — refused before the arm match
        // so it holds on every arm. HiPay has no $0 account verification: `operation` is only
        // Sale | Authorization and `amount` is required with a documented minimum of 1, so a
        // zero-amount setup cannot be expressed on this API and is refused rather than sent as a
        // 0 HiPay would reject with its own parameter error.
        // spec:### Mandates > DOCUMENTATION GAP — no zero-amount authorization; spec:### Core order fields
        let minor_amount = request
            .minor_amount
            .filter(|amount| amount.get_amount_as_i64() > 0)
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::NotSupported {
                    message: "HiPay has no zero-amount verification: the operation enum is \
                              Sale|Authorization and amount is required with a minimum of 1"
                        .to_string(),
                    connector: "hipay",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Set the mandate up with a real chargeable amount of at least the \
                             currency's smallest documented unit; HiPay's card-on-file CIT is an \
                             ordinary authorization."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "POST /v1/order takes no `verify` operation and no amount=0; the \
                             agreement is opened by the initial charge itself."
                                .to_string(),
                        ),
                    },
                })
            })?;

        // The arm decides where the multi-use Secure Vault token can come from. HiPay rejects raw
        // PANs on POST /v1/order, so the Card arm — which has no token carrier on the recurring
        // request messages — reaches the cardtoken refusal below rather than sending a PAN.
        // spec:### Mandates (M2); spec:#### 1. Create Order and Transaction
        let cardtoken: Option<Secret<String>> = match &request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(token) => Some(token.token.clone()),
            PaymentMethodData::Card(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => None,
        };

        // P-Payments-12 / UD-09 — the recurring request messages carry no card network at all, so
        // `payment_product` is resolved from the caller-declared carriers only. Evaluated before
        // the cardtoken refusal so each guard stays separately observable, exactly as on Authorize.
        // `SetupMandateRequestData` has no `connector_feature_data` field of its own, so the
        // carrier is read from the flow data, which is where the mandate request messages' one
        // lands (ucs:crates/types-traits/domain_types/src/connector_types.rs:829).
        let declared_product = declared_payment_product(common.connector_feature_data.as_ref())
            .or_else(|| declared_payment_product(request.metadata.as_ref()));
        let payment_product = payment_product_for(None, None, declared_product.as_deref(), None)?;

        // G-Payments-01 — the CIT is the money path and needs the vaulted, replayable credential.
        let cardtoken = cardtoken
            .filter(|token| !token.peek().trim().is_empty())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "cardtoken",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Vault the card at POST {secondary_base_url}/create \
                             (PaymentMethodService/Tokenize) with multi_use=1 and set up the \
                             mandate through PaymentService/TokenSetupRecurring with the returned \
                             token."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "HiPay rejects raw PANs on POST /v1/order, and only a multi-use \
                             Secure Vault token can be replayed on a later MIT."
                                .to_string(),
                        ),
                    },
                })
            })?;

        let amount = item
            .connector
            .amount_converter
            .convert(minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the mandate currency is one HiPay's amount exponent table \
                         covers."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "HiPay's `amount` order parameter is a major-unit decimal string."
                            .to_string(),
                    ),
                },
            })?;

        // TH-15 / G-Payments-04 — the CIT honours capture_method like any other order, and
        // refuses the two capture methods HiPay's `operation` enum cannot express rather than
        // settling them in full as a Sale; UD-07 keeps the lowercase spelling of the enum.
        // spec:### API Call Sequences ### Mandates (a) Step 2
        let operation = hipay_operation(request.capture_method)?;

        // `description` is a documented required order parameter; fail closed rather than send a
        // placeholder literal. `PaymentServiceTokenSetupRecurringRequest` has no `description`
        // field, so on that path it arrives through `metadata.description` and is read by
        // `declared_description` below — the same carriers, and the same precedence, this function
        // already uses for `payment_product`. spec:### Core order fields (description Required Yes)
        let description = common
            .description
            .clone()
            .or_else(|| {
                declared_description([
                    common.connector_feature_data.as_ref(),
                    request.metadata.as_ref(),
                ])
            })
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "description",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send a description for the mandate setup: the `description` field on \
                         PaymentService/SetupRecurring, or metadata.description on \
                         PaymentService/TokenSetupRecurring, whose request message has no \
                         description field of its own."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "HiPay documents `description` as a required parameter of POST /v1/order, \
                         and the mandate CIT is an ordinary order."
                            .to_string(),
                    ),
                },
            })
            })?;

        Ok(Self {
            payment_product,
            orderid: common.connector_request_reference_id.clone(),
            operation,
            description,
            currency,
            amount,
            cardtoken,
            eci: HipayEci::SecureEcommerce,
            recurring_payment: HipayFlag::On,
            authentication_indicator: HipayAuthenticationIndicator::from(common.auth_type),
            recurring_info: HipayRecurringInfo::from_mandate_data(
                request.setup_mandate_details.as_ref(),
            ),
            notify_url: request.webhook_url.clone(),
        })
    }
}

/// The merchant-initiated charge against an agreement an earlier CIT opened.
///
/// HiPay accepts **either** handle: its own `debit_agreement_id` (from the CIT response's
/// `debitAgreement.id`) or the multi-use `cardtoken`. There is no cardholder to challenge, so
/// `authentication_indicator` is `0`. spec:### Mandates (M4); spec:### API Call Sequences ### Mandates (c)
#[derive(Debug, Serialize)]
pub struct HipayRepeatPaymentRequest {
    pub payment_product: HipayPaymentProduct,
    pub orderid: String,
    pub operation: Operation,
    pub description: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    /// `9` — a merchant-initiated recurring charge.
    pub eci: HipayEci,
    /// `1` — this charge belongs to an existing agreement.
    pub recurring_payment: HipayFlag,
    /// `0` — no cardholder is present on an MIT, so 3-D Secure is bypassed.
    pub authentication_indicator: HipayAuthenticationIndicator,
    /// HiPay's own agreement handle, the CIT response's `debitAgreement.id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_agreement_id: Option<Secret<String>>,
    /// The alternate handle: the multi-use Secure Vault token the CIT was made with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardtoken: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HipayRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let common = &item.router_data.resource_common_data;
        let currency = request.currency;

        // G-Mandates-03 (Card, MandatePayment, PaymentMethodToken) — a caller-supplied scheme
        // transaction id is refused on the MIT for the same reason as on the CIT, and before the
        // arm match so it holds on every arm.
        let mandate_reference = request.get_mandate_reference();
        if carries_network_transaction_id(&request.payment_method_data, None)
            || matches!(
                mandate_reference,
                MandateReferenceId::NetworkMandateId(_)
                    | MandateReferenceId::NetworkTokenWithNTI(_)
            )
        {
            return Err(refuse_network_transaction_id());
        }

        // P-Mandates-03 — the agreement id HiPay issued on the CIT is the primary MIT handle.
        let debit_agreement_id = match mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate) => connector_mandate
                .get_connector_mandate_id()
                .filter(|id| !id.trim().is_empty())
                .map(Secret::new),
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => None,
        };

        // The alternate handle, when the caller replays the multi-use vault token instead.
        let cardtoken = match &request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(token) => Some(token.token.clone()),
            PaymentMethodData::Card(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => None,
        }
        .filter(|token| !token.peek().trim().is_empty());

        // G-Mandates-02 (Card, MandatePayment, PaymentMethodToken) — reached from every arm: an
        // MIT with neither handle is not a recurring charge at all, and HiPay would answer
        // `3040001 Unknown Token`. spec:### Mandates (M4) (either cardtoken or debit_agreement_id)
        if debit_agreement_id.is_none() && cardtoken.is_none() {
            return Err(
                error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "debit_agreement_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Replay the debitAgreement.id HiPay returned on the SetupRecurring CIT as \
                         connector_recurring_payment_id, or send the multi-use Secure Vault token \
                         as the payment method token."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "A HiPay MIT replays the agreement id from the CIT, or the multi-use card \
                         token; POST /v1/order accepts either but needs one of them."
                            .to_string(),
                    ),
                },
            }),
            );
        }

        let declared_product = declared_payment_product(request.connector_feature_data.as_ref())
            .or_else(|| declared_payment_product(request.metadata.as_ref()));
        let payment_product = payment_product_for(None, None, declared_product.as_deref(), None)?;

        let amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Check that the recurring charge currency is one HiPay's amount exponent \
                         table covers."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "HiPay's `amount` order parameter is a major-unit decimal string."
                            .to_string(),
                    ),
                },
            })?;

        // TH-15 / G-Payments-04 — a recurring charge honours capture_method like any other
        // order, and refuses ManualMultiple/Scheduled rather than settling them in full.
        let operation = hipay_operation(request.capture_method)?;

        // Same carrier gap as the CIT above: the recurring-charge request message has no
        // `description` field either, so `metadata.description` is the caller's only carrier.
        let description = common
            .description
            .clone()
            .or_else(|| {
                declared_description([
                    request.connector_feature_data.as_ref(),
                    request.metadata.as_ref(),
                ])
            })
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "description",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send a description on the recurring charge request (metadata.description \
                         on PaymentService/RecurringCharge)."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "HiPay documents `description` as a required parameter of POST /v1/order, \
                         and a recurring charge is an ordinary order."
                            .to_string(),
                    ),
                },
            })
            })?;

        Ok(Self {
            payment_product,
            orderid: common.connector_request_reference_id.clone(),
            operation,
            description,
            currency,
            amount,
            eci: HipayEci::RecurringEcommerce,
            recurring_payment: HipayFlag::On,
            authentication_indicator: HipayAuthenticationIndicator::Bypass,
            debit_agreement_id,
            cardtoken,
            notify_url: request.webhook_url.clone(),
        })
    }
}

/// Both mandate legs are the same `POST /v1/order` call as Authorize, so they answer with the
/// same body. spec:### Mandates (M2, M4)
pub type HipaySetupMandateResponse = HipayPaymentsResponse;
pub type HipayRepeatPaymentResponse = HipayPaymentsResponse;

/// The mandate handles a HiPay CIT hands back, as the domain `MandateReference`.
///
/// `debitAgreement.id` is the primary handle and the value a later MIT replays as
/// `debit_agreement_id`. The multi-use `paymentMethod.token` is the documented alternative, and
/// it is a reusable payment credential, so it is carried in `mandate_metadata` — a
/// `SecretSerdeValue` — rather than in either plaintext `String` field (TH-01).
/// spec:### Mandates (M2 response — the mandate handle); spec:### Mandates (M4)
fn mandate_reference_from_response(
    response: &HipayPaymentsResponse,
) -> Option<Box<MandateReference>> {
    let connector_mandate_id = response
        .debit_agreement
        .as_ref()
        .and_then(|agreement| agreement.id.clone())
        .filter(|id| !id.trim().is_empty());
    let cardtoken = response
        .payment_method
        .as_ref()
        .and_then(|payment_method| payment_method.token.clone())
        .filter(|token| !token.peek().trim().is_empty());
    if connector_mandate_id.is_none() && cardtoken.is_none() {
        return None;
    }
    Some(Box::new(MandateReference {
        connector_mandate_id,
        payment_method_id: None,
        connector_mandate_request_reference_id: None,
        mandate_metadata: cardtoken.map(|token| {
            Secret::new(serde_json::json!({ "hipay_multi_use_cardtoken": token.peek() }))
        }),
    }))
}

/// Everything the order response carries that the mandate legs read, as connector metadata.
/// The vault token is deliberately **not** here — it is a payment credential and travels in
/// `MandateReference.mandate_metadata`, which is `Secret`-typed.
fn mandate_connector_metadata(response: &HipayPaymentsResponse) -> Option<serde_json::Value> {
    let metadata = serde_json::json!({
        "avs_result": response.avs_result,
        "cvc_result": response.cvc_result,
        "authorization_code": response.authorization_code,
        "eci": response.eci,
        "payment_product_brand": response
            .payment_method
            .as_ref()
            .and_then(|payment_method| payment_method.brand.clone()),
        "card_issuer": response
            .payment_method
            .as_ref()
            .and_then(|payment_method| payment_method.issuer.clone()),
        "card_issuing_country": response
            .payment_method
            .as_ref()
            .and_then(|payment_method| payment_method.country),
        "debit_agreement_id": response
            .debit_agreement
            .as_ref()
            .and_then(|agreement| agreement.id.clone()),
        // The agreement lifecycle HiPay reports alongside the id (available, created, error,
        // incomplete, pending, suspended, terminated). spec:### API Call Sequences ### Mandates (a) Step 2
        "debit_agreement_status": response
            .debit_agreement
            .as_ref()
            .and_then(|agreement| agreement.status.clone()),
        "amount_to_capture": response.order_amount_to_capture(),
        "sca_preference": response.order_sca_preference(),
        "three_d_secure": response.three_d_secure.as_ref().map(|three_ds| {
            serde_json::json!({
                "eci": three_ds.eci,
                "enrollment_status": three_ds.enrollment_status,
                "enrollment_message": three_ds.enrollment_message,
                "authentication_status": three_ds.authentication_status,
                "authentication_message": three_ds.authentication_message,
                "authentication_token": three_ds.authentication_token,
                "xid": three_ds.xid,
            })
        }),
    });
    metadata
        .as_object()
        .filter(|fields| fields.values().any(|value| !value.is_null()))
        .map(|fields| serde_json::Value::Object(fields.clone()))
}

/// The `ErrorResponse` a mandate leg reports on an in-band decline.
///
/// P-Mandates-04 — both markers ride the payment status vocabulary, so the error carries the
/// status the response actually mapped to (a soft decline keeps its retry-with-3DS signal) and
/// the acquirer / issuer decline code from the `reason` object. spec:## 9. Error / decline reason codes
fn mandate_error_response(
    response: &HipayPaymentsResponse,
    status: AttemptStatus,
    http_code: u16,
) -> domain_types::router_data::ErrorResponse {
    let (network_decline_code, network_error_message) =
        network_fields_from_reason(response.reason.as_ref());
    domain_types::router_data::ErrorResponse {
        code: response
            .reason
            .as_ref()
            .map(|reason| reason.code.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: response
            .reason
            .as_ref()
            .map(|reason| reason.message.clone())
            .unwrap_or_else(|| response.message.clone()),
        reason: Some(response.message.clone()),
        status_code: http_code,
        attempt_status: Some(FlowStatus::Payment(soft_decline_attempt_status(
            status,
            matches!(response.status, HipayPaymentStatus::SoftDeclined),
        ))),
        connector_transaction_id: Some(response.transaction_reference.clone()),
        network_decline_code,
        network_advice_code: None,
        network_error_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// UD-05 — the redirect is driven off `forwardUrl` being non-empty, not off a state literal.
fn redirection_data_from_response(
    response: &HipayPaymentsResponse,
) -> Option<Box<domain_types::router_response_types::RedirectForm>> {
    if response.forward_url.is_empty() {
        None
    } else {
        Some(Box::new(
            domain_types::router_response_types::RedirectForm::Uri {
                uri: response.forward_url.clone(),
            },
        ))
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HipaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        // A 2xx from HiPay can still carry a decline, so the failure test is on the mapped
        // status — the same predicate the Authorize leg uses, since both read a
        // `POST /v1/order` response.
        let is_failure = is_order_response_failure(status);
        let mandate_reference = mandate_reference_from_response(&item.response);

        // P-Mandates-03 — a settled CIT that carried recurring_payment=1 and came back without a
        // debitAgreement is not a mandate: reporting it as a successful setup would hand the
        // caller a mandate it cannot charge. A still-pending or still-authenticating CIT has no
        // agreement yet, which is not the same thing.
        // spec:### Mandates (M2 response — the mandate handle)
        let settled = matches!(status, AttemptStatus::Charged | AttemptStatus::Authorized);
        let agreement_missing = mandate_reference.is_none();

        let response = if is_failure {
            Err(mandate_error_response(
                &item.response,
                status,
                item.http_code,
            ))
        } else if settled && agreement_missing {
            Err(domain_types::router_data::ErrorResponse {
                code: NO_ERROR_CODE.to_string(),
                message: "HiPay settled the card-on-file authorization but returned no \
                          debitAgreement, so no mandate handle exists to charge later"
                    .to_string(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: redirection_data_from_response(&item.response),
                mandate_reference,
                connector_metadata: mandate_connector_metadata(&item.response),
                // P-Mandates-03 / G-Mandates-03: HiPay returns no scheme transaction id on the V1
                // order response — `scheme_reference_data` exists only on the V3 debitAgreement
                // object — so `None` is the correct value here, not an oversight. The agreement id
                // above is what chains the MIT.
                // spec:## 8. Network / authorization transaction id ### How HiPay actually chains MIT instead
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.response_reference_id()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
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

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HipayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());
        // Same predicate as the Authorize and SetupMandate legs: an MIT is a `POST /v1/order`.
        let is_failure = is_order_response_failure(status);

        let response = if is_failure {
            Err(mandate_error_response(
                &item.response,
                status,
                item.http_code,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: redirection_data_from_response(&item.response),
                // An MIT charges an agreement that already exists; HiPay echoes it, and echoing it
                // back keeps a rotated agreement id visible to the caller.
                mandate_reference: mandate_reference_from_response(&item.response),
                connector_metadata: mandate_connector_metadata(&item.response),
                // See the SetupMandate conversion: HiPay publishes no scheme transaction id on the
                // V1 order response.
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.response_reference_id()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
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

// ========================================================================================
// GetFormData TRAIT IMPLEMENTATIONS
// ========================================================================================
// These implementations enable multipart/form-data request format for HiPay API

/// HiPay's own multipart form builder.
///
/// The shared `build_form_from_struct` collapses every `Value::Object` to the empty string,
/// which is exactly why a nested `browser_info` reaches HiPay empty. HiPay's convention for a
/// nested object in a form body is **bracket notation** (`browser_info[language]=en-GB`) — the
/// same convention it uses in the direction it controls, its notification payloads. The shared
/// helper is deliberately left untouched so no other FormData connector's wire body changes.
/// spec:### browser_info (nested object, PSD2); spec:### Webhook Payload Structure; UD-06
fn hipay_form_from_struct<T: Serialize>(data: &T) -> MultipartData {
    let mut form = MultipartData::new();
    // A request type that does not serialise to a JSON object cannot be expressed as a form
    // body at all; an empty form makes HiPay answer with its own validation error rather than
    // this connector panicking.
    if let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(data) {
        for (key, value) in fields {
            push_bracketed(&mut form, &key, &value);
        }
    }
    form
}

/// Appends one serialised value under `name`, expanding nested objects and arrays with
/// bracket notation and skipping nulls (HiPay treats an absent parameter and an empty one
/// differently, and `Option::None` means absent).
fn push_bracketed(form: &mut MultipartData, name: &str, value: &serde_json::Value) {
    match value {
        serde_json::Value::Null => {}
        serde_json::Value::String(text) => form.add_text(name.to_string(), text.clone()),
        serde_json::Value::Number(number) => form.add_text(name.to_string(), number.to_string()),
        serde_json::Value::Bool(flag) => form.add_text(name.to_string(), flag.to_string()),
        serde_json::Value::Object(fields) => {
            for (key, nested) in fields {
                push_bracketed(form, &format!("{name}[{key}]"), nested);
            }
        }
        serde_json::Value::Array(entries) => {
            for (index, nested) in entries.iter().enumerate() {
                push_bracketed(form, &format!("{name}[{index}]"), nested);
            }
        }
    }
}

// GetFormData implementation for HipayPaymentsRequest
impl GetFormData for HipayPaymentsRequest {
    fn get_form_data(&self) -> MultipartData {
        hipay_form_from_struct(self)
    }
}

// GetFormData implementation for HipayCaptureRequest
impl GetFormData for HipayCaptureRequest {
    fn get_form_data(&self) -> MultipartData {
        hipay_form_from_struct(self)
    }
}

// GetFormData implementation for HipayVoidRequest
impl GetFormData for HipayVoidRequest {
    fn get_form_data(&self) -> MultipartData {
        hipay_form_from_struct(self)
    }
}

// GetFormData implementation for HipayRefundRequest
impl GetFormData for HipayRefundRequest {
    fn get_form_data(&self) -> MultipartData {
        hipay_form_from_struct(self)
    }
}

// GetFormData implementation for HipaySetupMandateRequest
impl GetFormData for HipaySetupMandateRequest {
    fn get_form_data(&self) -> MultipartData {
        hipay_form_from_struct(self)
    }
}

// GetFormData implementation for HipayRepeatPaymentRequest
impl GetFormData for HipayRepeatPaymentRequest {
    fn get_form_data(&self) -> MultipartData {
        hipay_form_from_struct(self)
    }
}

// ============================================================================
// IncomingWebhook — HiPay server-to-server notification
//
// HiPay POSTs an **XML** body to the merchant's `notify_url` (back office
// Integration > Notifications, or the per-order `notify_url` parameter). It is not a
// `ConnectorIntegrationV2` flow: there is no outbound call, no marker and no macro.
//
// Three facts shape everything below.
//
// 1. **The root element is undocumented.** HiPay's published snippet starts at `<state>` and no
//    page names the wrapper or a `Content-Type`. `quick_xml::de` deserializes the document
//    element into the target struct without matching its name, so the parser does not need to
//    know it. (The `<mapi>/<result>` wrapper of the legacy HiPay *Professional* product is a
//    different contract and must not leak in here.)
//    spec:### IncomingWebhook > DOCUMENTATION GAP — Content-Type and XML root
// 2. **The signature covers the raw bytes**, so nothing is re-serialised before hashing and the
//    body is parsed from the same `request.body` slice that was hashed.
//    spec:## 10. Signature / MAC (HMAC) rules #### A
// 3. **Field names are `snake_case`**, unlike the camelCase JSON the synchronous API returns, and
//    nested objects arrive as child elements (the docs spell them with the bracket notation the
//    form-encoded variant uses: `payment_method[token]`, `order[id]`, `operation[id]`).
//    spec:### Webhook Payload Structure
// ============================================================================

/// The coarse `state` of a HiPay notification. It is a cross-check on `status`, not a
/// substitute for it: `status` is the value the whole status vocabulary is expressed in, and the
/// two disagree on the states this connector cares about most (a `115` cancellation arrives as
/// `completed`). It is read to decide whether a payment notification that carries no mapped
/// terminal status should be reported as failed rather than as an unrelated pending.
/// spec:### Webhook Payload Structure (`state`: completed, pending, declined, error)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HipayWebhookState {
    Completed,
    Pending,
    Declined,
    Error,
    /// Any state HiPay adds after this integration was written.
    #[serde(other)]
    Unknown,
}

/// Which of HiPay's three notification classes a numeric `status` belongs to.
///
/// HiPay has **no separate dispute webhook**: a chargeback arrives as an ordinary transaction
/// notification carrying `180`, `181` or `134`, which is why the split is made on the status code
/// rather than on an event-type field (there is none).
/// spec:### IncomingWebhook (Dispute webhooks); spec:## Status Mappings ### Notified statuses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipayWebhookEventClass {
    Payment,
    Refund,
    Dispute,
}

/// The raw numeric `status` element of a notification.
///
/// It is kept as the code HiPay wrote because one element has to be read through three different
/// vocabularies — [`HipayPaymentStatus`], [`HipayRefundStatus`] and [`HipayDisputeStatus`] —
/// and which one applies is decided by [`Self::event_class`]. The three accessors below
/// **re-use the `serde` rename tables of those enums** rather than restating the code-to-variant
/// mapping, so there is exactly one place where `118` means `Captured`. Each of those enums
/// carries `#[serde(other)] Unknown`, so an unrecognised code never errors and never guesses.
/// spec:## Status Mappings ### Notified statuses
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HipayWebhookStatusCode(String);

impl HipayWebhookStatusCode {
    /// Refund statuses, per the notified-status table: 124 Refund Requested, 125 Refunded,
    /// 126 Partially Refunded, 165 Refund Refused, and the RDR refunds 182 / 183.
    const REFUND_CODES: [&'static str; 6] = ["124", "125", "126", "165", "182", "183"];
    /// Chargeback / dispute statuses: 181 Chargeback, 180 Partially Chargeback, 134 Dispute Lost.
    const DISPUTE_CODES: [&'static str; 3] = ["180", "181", "134"];

    fn as_str(&self) -> &str {
        self.0.trim()
    }

    /// Which notification class this status belongs to. Everything that is not a refund or a
    /// chargeback code is a payment notification — the payment vocabulary is open-ended and
    /// `HipayPaymentStatus::Unknown` is what absorbs a code HiPay adds later.
    pub fn event_class(&self) -> HipayWebhookEventClass {
        let code = self.as_str();
        if Self::REFUND_CODES.contains(&code) {
            HipayWebhookEventClass::Refund
        } else if Self::DISPUTE_CODES.contains(&code) {
            HipayWebhookEventClass::Dispute
        } else {
            HipayWebhookEventClass::Payment
        }
    }

    /// The payment vocabulary — the same [`HipayPaymentStatus`] the Authorize, Capture, Void and
    /// PSync paths map, so the webhook path cannot drift from them.
    pub fn payment_status(&self) -> HipayPaymentStatus {
        deserialize_status_code(self.as_str()).unwrap_or(HipayPaymentStatus::Unknown)
    }

    /// The refund vocabulary — the same [`HipayRefundStatus`] the Refund and RSync paths map.
    pub fn refund_status(&self) -> HipayRefundStatus {
        deserialize_status_code(self.as_str()).unwrap_or(HipayRefundStatus::Unknown)
    }

    /// The chargeback vocabulary.
    pub fn dispute_status(&self) -> HipayDisputeStatus {
        deserialize_status_code(self.as_str()).unwrap_or(HipayDisputeStatus::Unknown)
    }
}

/// Read a numeric HiPay status code through a wire status enum's own `serde` rename table.
///
/// This is deliberately *not* a second `match` over the numeric codes: the rename attributes on
/// [`HipayPaymentStatus`], [`HipayRefundStatus`] and [`HipayDisputeStatus`] are the single source
/// of truth for what `118` or `165` means, and restating them here is exactly how the webhook path
/// would drift from the synchronous paths. Each of those enums carries `#[serde(other)] Unknown`,
/// so a code outside its vocabulary deserializes to `Unknown` rather than erroring; the caller
/// still supplies that variant explicitly because the signature is fallible.
fn deserialize_status_code<S>(code: &str) -> Result<S, serde::de::value::Error>
where
    S: serde::de::DeserializeOwned,
{
    use serde::de::IntoDeserializer;
    S::deserialize(code.into_deserializer())
}

/// HiPay's chargeback vocabulary, which is only ever delivered by notification.
///
/// It has exactly three members. There is **no "dispute won" status** anywhere in HiPay's
/// documentation — no representment, no evidence submission, no accepted state — so
/// `DisputeStatus::DisputeWon` is never produced here. Inventing one would report an outcome the
/// connector has no way of learning.
/// spec:### IncomingWebhook > DOCUMENTATION GAP — dispute lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HipayDisputeStatus {
    #[serde(rename = "180")]
    PartiallyChargeback,
    #[serde(rename = "181")]
    Chargeback,
    #[serde(rename = "134")]
    DisputeLost,
    /// A dispute code HiPay adds later. Reported as an opened dispute rather than as a lost one:
    /// the conservative reading of an unknown chargeback code is that the case is still live.
    #[serde(other)]
    #[default]
    Unknown,
}

impl From<HipayDisputeStatus> for common_enums::DisputeStatus {
    fn from(status: HipayDisputeStatus) -> Self {
        match status {
            // 181 Chargeback and 180 Partially Chargeback are both "the issuer has taken the
            // money back and the case is open".
            HipayDisputeStatus::Chargeback
            | HipayDisputeStatus::PartiallyChargeback
            | HipayDisputeStatus::Unknown => Self::DisputeOpened,
            HipayDisputeStatus::DisputeLost => Self::DisputeLost,
        }
    }
}

/// `reason` sub-element of a notification.
///
/// The synchronous paths carry the same decline detail in [`HipayReason`], whose `code` is
/// **required** on purpose: on the JSON side a malformed `reason` object must not be silently
/// dropped, because it is the only carrier of the acquirer / issuer code. XML is different —
/// HiPay's "no decline detail" placeholder is the empty-string `reason` on the JSON side, and an
/// empty `<reason/>` element is its natural XML spelling. With a required `code`, that element
/// would hard-fail the whole notification, so *every approved payment* would stop parsing.
///
/// The fields are therefore optional here and converted to a [`HipayReason`] only when a code is
/// actually present, which keeps `network_fields_from_reason` — the one `40xxxxx`-band rule — as
/// the single decision point for what becomes a network decline code.
/// spec:### Transaction-level decline detail (reason object); spec:### Webhook Payload Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayWebhookReason {
    #[serde(default)]
    pub code: Option<String>,
    /// `#[serde(default)]` on a `String`, exactly as [`HipayReason::message`] carries it on the
    /// JSON side: an absent `<message>` element and an absent `"message"` key are the same
    /// "no text" state, and [`HipayWebhookNotification::network_fields`] maps it back to `None`
    /// so the caller falls through to the notification's own `message` element rather than
    /// reporting a blank.
    #[serde(default)]
    pub message: String,
}

impl HipayWebhookReason {
    /// The shared decline-detail carrier, when this element holds a non-blank code. A `reason`
    /// element with no usable code is `None` — the same thing `deserialize_optional_reason` does
    /// with HiPay's empty-string placeholder on the JSON side.
    fn as_reason(&self) -> Option<HipayReason> {
        let code = self
            .code
            .as_deref()
            .map(str::trim)
            .filter(|code| !code.is_empty())?;
        Some(HipayReason {
            code: code.to_owned(),
            message: self.message.trim().to_owned(),
        })
    }
}

/// `payment_method` sub-object of a notification.
///
/// Every field here is card data, a reusable credential or personal data arriving from an
/// external source, so each is `Secret`-typed on the wire and exposed only where it is written
/// into the response — `token` into `MandateReference.mandate_metadata` (a `SecretSerdeValue`)
/// and the rest into `CardDetailUpdate`.
/// spec:### Webhook Payload Structure (`payment_method[token]`, `[brand]`, `[pan]`, …)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayWebhookPaymentMethod {
    /// The multi-use Secure Vault token — a replayable payment credential.
    #[serde(default)]
    pub token: Option<Secret<String>>,
    #[serde(default)]
    pub brand: Option<String>,
    /// HiPay sends a masked PAN (`411111xxxxxx1111`); it is still card data.
    #[serde(default)]
    pub pan: Option<Secret<String>>,
    #[serde(default)]
    pub card_holder: Option<Secret<String>>,
    #[serde(default)]
    pub card_expiry_month: Option<String>,
    #[serde(default)]
    pub card_expiry_year: Option<String>,
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

/// `order` sub-object. Only `id` is read: it is the merchant's own `orderid`, which the response
/// carries as `connector_response_reference_id` — explicitly **not** as the event reference,
/// which is `transaction_reference`.
/// spec:### Webhook Payload Structure (`order[id]`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayWebhookOrder {
    #[serde(default)]
    pub id: Option<String>,
}

/// `operation` sub-object, present only when the maintenance call supplied `operation_id`. It is
/// the **only** way to attribute a partial refund to the request that asked for it, because HiPay
/// issues no refund-level identifier.
/// spec:### IncomingWebhook (Refund webhooks); spec:### Refunds > DOCUMENTATION GAP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayWebhookOperation {
    #[serde(default)]
    pub id: Option<String>,
}

/// A HiPay server-to-server notification.
///
/// Only the elements that reach the response are declared. The notification also carries
/// `three_d_secure[*]`, `avs_result`, `cvc_result`, `eci`, `fraud_screening[*]`,
/// `authorization_code`, `acquirer_transaction_reference`, `custom_data[*]`, `mid`, `test`,
/// `device_id`, `ip_address` and the `date_*` timestamps; none of the webhook response types has
/// a field for any of them, and a parsed-but-unread element is exactly what review objects to, so
/// they are deliberately left unparsed. `quick_xml` ignores unknown elements.
/// spec:### Webhook Payload Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayWebhookNotification {
    pub state: HipayWebhookState,
    pub status: HipayWebhookStatusCode,
    #[serde(default)]
    pub message: Option<String>,
    /// The HiPay transaction reference — the same value Authorize returned as
    /// `connector_transaction_id`.
    pub transaction_reference: String,
    /// Acquirer / issuer decline detail, converted to the `HipayReason` the synchronous paths
    /// use so `network_fields_from_reason` applies the one `40xxxxx`-band rule on this path too.
    #[serde(default)]
    pub reason: Option<HipayWebhookReason>,
    #[serde(default)]
    pub authorized_amount: Option<StringMajorUnit>,
    #[serde(default)]
    pub captured_amount: Option<StringMajorUnit>,
    #[serde(default)]
    pub refunded_amount: Option<StringMajorUnit>,
    #[serde(default)]
    pub currency: Option<common_enums::Currency>,
    #[serde(default)]
    pub payment_method: Option<HipayWebhookPaymentMethod>,
    #[serde(default)]
    pub order: Option<HipayWebhookOrder>,
    #[serde(default)]
    pub operation: Option<HipayWebhookOperation>,
}

impl HipayWebhookNotification {
    /// `(network_decline_code, network_error_message)` — the shared `40xxxxx`-band rule, applied
    /// unchanged on the notification path.
    pub fn network_fields(&self) -> (Option<String>, Option<String>) {
        let (code, message) = network_fields_from_reason(
            self.reason
                .as_ref()
                .and_then(HipayWebhookReason::as_reason)
                .as_ref(),
        );
        // A `reason` object with a code but no text reports no network message, so the caller
        // falls through to the notification's own `message` element instead of a blank string.
        (code, message.filter(|message| !message.is_empty()))
    }

    /// The transaction reference, refused when blank. An empty reference would resolve against no
    /// stored payment, so it is a checked error rather than an empty string handed onwards.
    pub fn non_empty_transaction_reference(&self) -> Option<&str> {
        Some(self.transaction_reference.trim()).filter(|reference| !reference.is_empty())
    }

    /// The merchant `orderid`, when HiPay echoed one back.
    pub fn order_reference(&self) -> Option<String> {
        self.order
            .as_ref()
            .and_then(|order| order.id.as_ref())
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
    }

    /// The merchant `operation_id` echoed back on a maintenance notification — the partial-refund
    /// correlator.
    pub fn operation_reference(&self) -> Option<String> {
        self.operation
            .as_ref()
            .and_then(|operation| operation.id.as_ref())
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
    }

    /// The captured amount this notification reports, as `MinorUnit`.
    ///
    /// The wire value is a major-unit decimal string (`StringMajorUnit`) — the same unit the
    /// order request sends — converted through the connector's own converter and the payload
    /// `currency`. It is never handled as a `String` and never as a float (INV-19 / TH-02).
    /// spec:### Webhook Payload Structure (`captured_amount`); spec:### Core order fields
    pub fn captured_amount_minor(&self) -> Option<MinorUnit> {
        let currency = self.currency?;
        StringMajorUnitForConnector
            .convert_back(self.captured_amount.clone()?, currency)
            .ok()
    }

    /// The amount a chargeback notification moved, with the payload currency.
    ///
    /// HiPay's notification has no chargeback-specific amount element, so the refunded amount is
    /// read first (a chargeback takes money back the way a refund does) and the captured, then
    /// authorized, amount is the fallback.
    /// spec:### IncomingWebhook (Dispute webhooks); spec:### Webhook Payload Structure
    pub fn disputed_amount_minor(&self) -> Option<(MinorUnit, common_enums::Currency)> {
        let currency = self.currency?;
        let amount = self
            .refunded_amount
            .as_ref()
            .or(self.captured_amount.as_ref())
            .or(self.authorized_amount.as_ref())?;
        StringMajorUnitForConnector
            .convert_back(amount.clone(), currency)
            .ok()
            .map(|minor| (minor, currency))
    }

    /// The card details HiPay reports on the notification, as the framework's own update type.
    /// This is the one place `payment_method[pan]` / `[card_holder]` leave `Secret`.
    pub fn card_detail_update(&self) -> Option<domain_types::connector_types::PaymentMethodUpdate> {
        let payment_method = self.payment_method.as_ref()?;
        let last4_digits = payment_method
            .pan
            .as_ref()
            .map(|pan| pan.peek().trim().to_owned())
            .filter(|pan| !pan.is_empty())
            .map(|pan| pan.chars().rev().take(4).collect::<Vec<_>>())
            .map(|mut tail| {
                tail.reverse();
                tail.into_iter().collect::<String>()
            });
        let card_holder_name = payment_method
            .card_holder
            .as_ref()
            .map(|holder| holder.peek().trim().to_owned())
            .filter(|holder| !holder.is_empty());
        let update = domain_types::connector_types::CardDetailUpdate {
            card_exp_month: non_blank(payment_method.card_expiry_month.as_deref()),
            card_exp_year: non_blank(payment_method.card_expiry_year.as_deref()),
            last4_digits,
            issuer_country: non_blank(payment_method.country.as_deref()),
            card_issuer: non_blank(payment_method.issuer.as_deref()),
            card_network: non_blank(payment_method.brand.as_deref()),
            card_holder_name,
        };
        if update.card_exp_month.is_none()
            && update.card_exp_year.is_none()
            && update.last4_digits.is_none()
            && update.issuer_country.is_none()
            && update.card_issuer.is_none()
            && update.card_network.is_none()
            && update.card_holder_name.is_none()
        {
            return None;
        }
        Some(domain_types::connector_types::PaymentMethodUpdate::Card(
            update,
        ))
    }

    /// The reusable vault token HiPay echoes on the notification, carried the same way the
    /// Mandates legs carry it: inside `MandateReference.mandate_metadata`, which is a
    /// `SecretSerdeValue`, never in a plaintext `String` field (TH-01).
    pub fn mandate_reference(&self) -> Option<Box<MandateReference>> {
        let token = self
            .payment_method
            .as_ref()
            .and_then(|payment_method| payment_method.token.as_ref())
            .filter(|token| !token.peek().trim().is_empty())?;
        Some(Box::new(MandateReference {
            connector_mandate_id: None,
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: Some(Secret::new(
                serde_json::json!({ "hipay_multi_use_cardtoken": token.peek() }),
            )),
        }))
    }
}

/// `Some(trimmed)` for a non-blank value, `None` otherwise — never `""`.
fn non_blank(value: Option<&str>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// The UCS event type a payment notification reports.
///
/// The mapping is taken off the mapped [`AttemptStatus`] rather than off a second table of
/// numeric codes, so the webhook classification cannot disagree with the status the same
/// notification reports — the single defect the repeat-issue checklist names most often.
/// `state` is the documented cross-check: a notification whose status maps to a non-terminal
/// state but whose `state` is `declined` or `error` is reported as a failure rather than as
/// processing.
/// spec:## Status Mappings ### Notified statuses; spec:### Webhook Payload Structure
fn payment_event_type(status: AttemptStatus, state: HipayWebhookState) -> EventType {
    match status {
        // 118 Captured, 119 Partially Captured, 120/121/122/123 collected/settled,
        // 131/132/166/168 debited.
        AttemptStatus::Charged | AttemptStatus::PartialCharged => EventType::PaymentIntentSuccess,
        // 116 Authorized.
        AttemptStatus::Authorized | AttemptStatus::PartiallyAuthorized => {
            EventType::PaymentIntentAuthorizationSuccess
        }
        // 117 Capture Requested, 175 Authorization Cancellation Requested.
        AttemptStatus::CaptureInitiated
        | AttemptStatus::VoidInitiated
        | AttemptStatus::VoidPostCaptureInitiated
        | AttemptStatus::Authorizing
        | AttemptStatus::CodInitiated => EventType::PaymentIntentProcessing,
        // 173 Capture Refused.
        AttemptStatus::CaptureFailed => EventType::PaymentIntentCaptureFailure,
        // 115 Cancelled, 143 Authorization Cancelled.
        AttemptStatus::Voided | AttemptStatus::VoidedPostCapture => {
            EventType::PaymentIntentCancelled
        }
        AttemptStatus::VoidFailed => EventType::PaymentIntentCancelFailure,
        // 114 Expired — an authorization that lapsed before capture.
        AttemptStatus::Expired => EventType::PaymentIntentExpired,
        // 113 Refused, 110 Blocked, 111 Denied, 178 Soft Declined, 151 Acquirer Not Found,
        // 163 Authorization Refused, and the chargeback codes when they arrive on the payment
        // path rather than the dispute one.
        AttemptStatus::Failure | AttemptStatus::RouterDeclined => EventType::PaymentIntentFailure,
        // 109 Authentication Failed, 105/108 could not authenticate.
        AttemptStatus::AuthenticationFailed | AttemptStatus::AuthorizationFailed => {
            EventType::PaymentIntentAuthorizationFailure
        }
        // 177 Challenge Requested, 103/104/160 enrollment, 106/140/141 authentication.
        AttemptStatus::AuthenticationPending
        | AttemptStatus::AuthenticationSuccessful
        | AttemptStatus::DeviceDataCollectionPending => EventType::PaymentActionRequired,
        // Not reachable from `HipayPaymentStatus` — no HiPay status maps to it. Listed so the
        // match stays exhaustive without a `_` arm (INV-09).
        AttemptStatus::AutoRefunded => EventType::RefundSuccess,
        // Non-terminal, or a status HiPay added after this integration was written (mapped to
        // `AttemptStatus::Unknown` by the shared table). `state` is the only other signal the
        // payload carries, so it decides between "still processing" and "this attempt failed";
        // it never invents a success.
        AttemptStatus::Started
        | AttemptStatus::Pending
        | AttemptStatus::PartialChargedAndChargeable
        | AttemptStatus::Unresolved
        | AttemptStatus::Unspecified
        | AttemptStatus::PaymentMethodAwaited
        | AttemptStatus::ConfirmationAwaited
        | AttemptStatus::IntegrityFailure
        | AttemptStatus::Unknown => match state {
            HipayWebhookState::Declined | HipayWebhookState::Error => {
                EventType::PaymentIntentFailure
            }
            HipayWebhookState::Completed
            | HipayWebhookState::Pending
            | HipayWebhookState::Unknown => EventType::PaymentIntentProcessing,
        },
    }
}

/// The UCS event type a refund notification reports, off the mapped [`RefundStatus`] — the same
/// table the Refund and RSync paths use.
/// spec:## Status Mappings ### Notified statuses (124/125/126/165, RDR 182/183)
fn refund_event_type(status: RefundStatus) -> EventType {
    match status {
        RefundStatus::Success => EventType::RefundSuccess,
        RefundStatus::Failure | RefundStatus::TransactionFailure => EventType::RefundFailure,
        RefundStatus::Pending | RefundStatus::ManualReview | RefundStatus::Unknown => {
            EventType::RefundProcessing
        }
    }
}

/// The UCS event type a chargeback notification reports. HiPay's vocabulary has no won or
/// accepted state, so only these two are ever produced.
/// spec:### IncomingWebhook > DOCUMENTATION GAP — dispute lifecycle
fn dispute_event_type(status: common_enums::DisputeStatus) -> EventType {
    match status {
        common_enums::DisputeStatus::DisputeLost => EventType::DisputeLost,
        common_enums::DisputeStatus::DisputeOpened
        | common_enums::DisputeStatus::DisputeExpired
        | common_enums::DisputeStatus::DisputeAccepted
        | common_enums::DisputeStatus::DisputeCancelled
        | common_enums::DisputeStatus::DisputeChallenged
        | common_enums::DisputeStatus::DisputeWon => EventType::DisputeOpened,
    }
}

/// The event type of a notification, from its `status` alone — the stateless ParseEvent contract.
pub fn hipay_webhook_event_type(notification: &HipayWebhookNotification) -> EventType {
    match notification.status.event_class() {
        HipayWebhookEventClass::Payment => payment_event_type(
            AttemptStatus::from(notification.status.payment_status()),
            notification.state,
        ),
        HipayWebhookEventClass::Refund => {
            refund_event_type(RefundStatus::from(notification.status.refund_status()))
        }
        HipayWebhookEventClass::Dispute => dispute_event_type(common_enums::DisputeStatus::from(
            notification.status.dispute_status(),
        )),
    }
}
