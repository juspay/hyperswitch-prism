//! Flow-specific status rules.
//!
//! `FlowStatusRules` declares what `AttemptStatus` values a given flow may legally
//! produce, and which of those count as terminal success or failure.
//! Implementations live on the flow marker types in `connector_flow`.
//!
//! Used by:
//! - `impl_flow_status_mapping!` — const assertions that verify declared terminals
//!   are in the flow's `TERMINAL_SUCCESS_SET` / `TERMINAL_FAILURE_SET`.
//! - `PaymentFlowData::set_status_for_flow` — validates at runtime that a status
//!   being written is in the flow's `ALLOWED` set.

use common_enums::{AttemptStatus, DisputeStatus, PayoutStatus, RefundStatus};

use crate::connector_flow;

// ── Core trait ────────────────────────────────────────────────────────────

/// Implemented on flow marker structs.  Declares the status contract for a flow.
pub trait FlowStatusRules {
    /// Human-readable flow name used in error messages (e.g. `"Capture"`, `"Void"`).
    const NAME: &'static str;

    /// All statuses that represent a successful terminal for this flow.
    ///
    /// Single-terminal flows (Capture → `[Charged]`, Void → `[Voided]`) have one
    /// element; the macro's set-membership assertion is then as strict as equality.
    ///
    /// Multi-terminal flows (Authorize: `Authorized` for manual capture, `Charged`
    /// for auto-capture) list all valid success outcomes.
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus];

    /// All statuses that represent a terminal failure for this flow.
    ///
    /// Multiple entries cover distinct failure modes, e.g. Authorize can end with
    /// `AuthorizationFailed` (auth declined) or `AuthenticationFailed` (3DS failed).
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus];

    /// All status values this flow may legally produce.  Any status NOT in this list
    /// is rejected by `set_status_for_flow`.  Must be a superset of both terminal sets.
    const ALLOWED: &'static [AttemptStatus];
}

// ── const helpers ─────────────────────────────────────────────────────────

/// `const`-compatible slice membership test.
pub const fn const_contains(slice: &[AttemptStatus], target: AttemptStatus) -> bool {
    let mut i = 0;
    while i < slice.len() {
        if slice[i] as u32 == target as u32 {
            return true;
        }
        i += 1;
    }
    false
}

/// `const`-compatible "every element of `subset` is in `superset`" check.
/// Used by `assert_flow_rules!` to verify terminal sets are subsets of ALLOWED.
pub const fn const_all_in(subset: &[AttemptStatus], superset: &[AttemptStatus]) -> bool {
    let mut i = 0;
    while i < subset.len() {
        if !const_contains(superset, subset[i]) {
            return false;
        }
        i += 1;
    }
    true
}

// ── FlowStatusRules impls ─────────────────────────────────────────────────

impl FlowStatusRules for connector_flow::Authorize {
    const NAME: &'static str = "Authorize";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] = &[
        AttemptStatus::Authorized,          // manual-capture path
        AttemptStatus::Charged,             // auto-capture path
        AttemptStatus::PartialCharged,      // auto-capture partial amount
        AttemptStatus::PartiallyAuthorized, // partial authorization
    ];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] = &[
        AttemptStatus::AuthorizationFailed,  // auth declined
        AttemptStatus::AuthenticationFailed, // 3DS challenge failed
        AttemptStatus::Failure,
        AttemptStatus::IntegrityFailure,
    ];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::Started,
        AttemptStatus::AuthenticationPending,
        AttemptStatus::AuthenticationSuccessful,
        AttemptStatus::AuthenticationFailed,
        AttemptStatus::Authorized,
        AttemptStatus::PartiallyAuthorized,
        AttemptStatus::AuthorizationFailed,
        AttemptStatus::Authorizing,
        AttemptStatus::Charged,
        AttemptStatus::PartialCharged,
        AttemptStatus::PartialChargedAndChargeable,
        AttemptStatus::Voided,
        AttemptStatus::AutoRefunded,
        AttemptStatus::Expired,
        AttemptStatus::Unresolved,
        AttemptStatus::Unspecified,
        AttemptStatus::Unknown,
        AttemptStatus::Pending,
        AttemptStatus::Failure,
        AttemptStatus::PaymentMethodAwaited,
        AttemptStatus::ConfirmationAwaited,
        AttemptStatus::DeviceDataCollectionPending,
        AttemptStatus::IntegrityFailure,
    ];
}

impl FlowStatusRules for connector_flow::Capture {
    const NAME: &'static str = "Capture";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] = &[
        AttemptStatus::Charged,
        AttemptStatus::PartialCharged, // connectors that support partial capture
    ];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] = &[
        AttemptStatus::CaptureFailed,
        AttemptStatus::Failure,
        AttemptStatus::IntegrityFailure,
    ];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::CaptureInitiated,
        AttemptStatus::Charged,
        AttemptStatus::CaptureFailed,
        AttemptStatus::PartialCharged,
        AttemptStatus::PartialChargedAndChargeable,
        AttemptStatus::Pending,
        AttemptStatus::Failure,
        AttemptStatus::IntegrityFailure,
    ];
}

impl FlowStatusRules for connector_flow::Void {
    const NAME: &'static str = "Void";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] = &[AttemptStatus::Voided];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] =
        &[AttemptStatus::VoidFailed, AttemptStatus::Failure];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::VoidInitiated,
        AttemptStatus::Voided,
        AttemptStatus::VoidFailed,
        AttemptStatus::Pending,
        AttemptStatus::Failure,
    ];
}

impl FlowStatusRules for connector_flow::VoidPC {
    const NAME: &'static str = "VoidPC";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] = &[AttemptStatus::VoidedPostCapture];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] = &[AttemptStatus::Failure];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::VoidPostCaptureInitiated,
        AttemptStatus::VoidedPostCapture,
        AttemptStatus::Failure,
        AttemptStatus::Pending,
    ];
}

impl FlowStatusRules for connector_flow::SetupMandate {
    const NAME: &'static str = "SetupMandate";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] = &[AttemptStatus::Charged];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] =
        &[AttemptStatus::Failure, AttemptStatus::AuthorizationFailed];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::Started,
        AttemptStatus::AuthenticationPending,
        AttemptStatus::Pending,
        AttemptStatus::Charged,
        AttemptStatus::Failure,
        AttemptStatus::AuthorizationFailed,
    ];
}

impl FlowStatusRules for connector_flow::RepeatPayment {
    const NAME: &'static str = "RepeatPayment";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] =
        &[AttemptStatus::Charged, AttemptStatus::PartialCharged];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] = &[
        AttemptStatus::Failure,
        AttemptStatus::AuthorizationFailed,
        AttemptStatus::IntegrityFailure,
    ];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::Started,
        AttemptStatus::AuthenticationPending,
        AttemptStatus::Authorized,
        AttemptStatus::PartiallyAuthorized,
        AttemptStatus::AuthorizationFailed,
        AttemptStatus::Authorizing,
        AttemptStatus::Charged,
        AttemptStatus::PartialCharged,
        AttemptStatus::PartialChargedAndChargeable,
        AttemptStatus::Pending,
        AttemptStatus::Failure,
        AttemptStatus::IntegrityFailure,
    ];
}

// PSync mirrors whatever state the payment is in — intentionally broad.
impl FlowStatusRules for connector_flow::PSync {
    const NAME: &'static str = "PSync";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] = &[
        AttemptStatus::Authorized,
        AttemptStatus::Charged,
        AttemptStatus::PartialCharged,
        AttemptStatus::PartiallyAuthorized,
        AttemptStatus::Voided,
        AttemptStatus::AutoRefunded,
        AttemptStatus::VoidedPostCapture,
    ];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] = &[
        AttemptStatus::AuthorizationFailed,
        AttemptStatus::AuthenticationFailed,
        AttemptStatus::CaptureFailed,
        AttemptStatus::VoidFailed,
        AttemptStatus::Failure,
        AttemptStatus::IntegrityFailure,
    ];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::Started,
        AttemptStatus::AuthenticationPending,
        AttemptStatus::AuthenticationSuccessful,
        AttemptStatus::AuthenticationFailed,
        AttemptStatus::Authorized,
        AttemptStatus::PartiallyAuthorized,
        AttemptStatus::AuthorizationFailed,
        AttemptStatus::Authorizing,
        AttemptStatus::Charged,
        AttemptStatus::CaptureInitiated,
        AttemptStatus::CaptureFailed,
        AttemptStatus::PartialCharged,
        AttemptStatus::PartialChargedAndChargeable,
        AttemptStatus::Voided,
        AttemptStatus::VoidFailed,
        AttemptStatus::VoidInitiated,
        AttemptStatus::VoidPostCaptureInitiated,
        AttemptStatus::VoidedPostCapture,
        AttemptStatus::AutoRefunded,
        AttemptStatus::Expired,
        AttemptStatus::Unresolved,
        AttemptStatus::Unspecified,
        AttemptStatus::Unknown,
        AttemptStatus::Pending,
        AttemptStatus::Failure,
        AttemptStatus::PaymentMethodAwaited,
        AttemptStatus::ConfirmationAwaited,
        AttemptStatus::DeviceDataCollectionPending,
        AttemptStatus::IntegrityFailure,
        AttemptStatus::CodInitiated,
    ];
}

impl FlowStatusRules for connector_flow::IncrementalAuthorization {
    const NAME: &'static str = "IncrementalAuthorization";
    const TERMINAL_SUCCESS_SET: &'static [AttemptStatus] =
        &[AttemptStatus::Authorized, AttemptStatus::Charged];
    const TERMINAL_FAILURE_SET: &'static [AttemptStatus] = &[
        AttemptStatus::AuthorizationFailed,
        AttemptStatus::Failure,
        AttemptStatus::IntegrityFailure,
    ];
    const ALLOWED: &'static [AttemptStatus] = &[
        AttemptStatus::AuthenticationPending,
        AttemptStatus::Authorized,
        AttemptStatus::AuthorizationFailed,
        AttemptStatus::Authorizing,
        AttemptStatus::Charged,
        AttemptStatus::Voided,
        AttemptStatus::Pending,
        AttemptStatus::Failure,
        AttemptStatus::ConfirmationAwaited,
        AttemptStatus::IntegrityFailure,
    ];
}

// ── FlowStatusRules self-validation ──────────────────────────────────────
// Verifies that every flow's terminal sets are subsets of its ALLOWED set.
// Fires once at compile time — no per-connector repetition needed.

macro_rules! assert_flow_rules {
    ($flow:ty) => {
        const _: () = assert!(
            const_all_in(
                <$flow as FlowStatusRules>::TERMINAL_SUCCESS_SET,
                <$flow as FlowStatusRules>::ALLOWED,
            ),
            concat!(
                stringify!($flow),
                ": TERMINAL_SUCCESS_SET contains a value not in ALLOWED"
            ),
        );
        const _: () = assert!(
            const_all_in(
                <$flow as FlowStatusRules>::TERMINAL_FAILURE_SET,
                <$flow as FlowStatusRules>::ALLOWED,
            ),
            concat!(
                stringify!($flow),
                ": TERMINAL_FAILURE_SET contains a value not in ALLOWED"
            ),
        );
    };
}

assert_flow_rules!(connector_flow::Authorize);
assert_flow_rules!(connector_flow::Capture);
assert_flow_rules!(connector_flow::Void);
assert_flow_rules!(connector_flow::VoidPC);
assert_flow_rules!(connector_flow::SetupMandate);
assert_flow_rules!(connector_flow::RepeatPayment);
assert_flow_rules!(connector_flow::PSync);
assert_flow_rules!(connector_flow::IncrementalAuthorization);

// ── Refund flow rules ─────────────────────────────────────────────────────

pub trait RefundFlowStatusRules {
    const TERMINAL_SUCCESS: RefundStatus;
    const TERMINAL_FAILURE: RefundStatus;
    const ALLOWED: &'static [RefundStatus];
}

impl RefundFlowStatusRules for connector_flow::Refund {
    const TERMINAL_SUCCESS: RefundStatus = RefundStatus::Success;
    const TERMINAL_FAILURE: RefundStatus = RefundStatus::Failure;
    const ALLOWED: &'static [RefundStatus] = &[
        RefundStatus::Pending,
        RefundStatus::Success,
        RefundStatus::Failure,
        RefundStatus::ManualReview,
        RefundStatus::TransactionFailure,
    ];
}

impl RefundFlowStatusRules for connector_flow::RSync {
    const TERMINAL_SUCCESS: RefundStatus = RefundStatus::Success;
    const TERMINAL_FAILURE: RefundStatus = RefundStatus::Failure;
    const ALLOWED: &'static [RefundStatus] = &[
        RefundStatus::Pending,
        RefundStatus::Success,
        RefundStatus::Failure,
        RefundStatus::ManualReview,
        RefundStatus::TransactionFailure,
    ];
}

// ── Dispute flow rules ────────────────────────────────────────────────────

pub trait DisputeFlowStatusRules {
    const TERMINAL_SUCCESS: DisputeStatus;
    const TERMINAL_FAILURE: DisputeStatus;
    const ALLOWED: &'static [DisputeStatus];
}

impl DisputeFlowStatusRules for connector_flow::Accept {
    const TERMINAL_SUCCESS: DisputeStatus = DisputeStatus::DisputeAccepted;
    const TERMINAL_FAILURE: DisputeStatus = DisputeStatus::DisputeLost;
    const ALLOWED: &'static [DisputeStatus] = &[DisputeStatus::DisputeAccepted];
}

impl DisputeFlowStatusRules for connector_flow::SubmitEvidence {
    const TERMINAL_SUCCESS: DisputeStatus = DisputeStatus::DisputeChallenged;
    const TERMINAL_FAILURE: DisputeStatus = DisputeStatus::DisputeLost;
    const ALLOWED: &'static [DisputeStatus] = &[DisputeStatus::DisputeChallenged];
}

impl DisputeFlowStatusRules for connector_flow::DefendDispute {
    const TERMINAL_SUCCESS: DisputeStatus = DisputeStatus::DisputeWon;
    const TERMINAL_FAILURE: DisputeStatus = DisputeStatus::DisputeLost;
    const ALLOWED: &'static [DisputeStatus] =
        &[DisputeStatus::DisputeWon, DisputeStatus::DisputeLost];
}

// ── Payout flow rules ─────────────────────────────────────────────────────

pub trait PayoutFlowStatusRules {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus];
    const ALLOWED: &'static [PayoutStatus];
}

impl PayoutFlowStatusRules for connector_flow::PayoutTransfer {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus] = &[PayoutStatus::Success];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus] =
        &[PayoutStatus::Failure]; // EXPIRED, REVERSED
    const ALLOWED: &'static [PayoutStatus] = &[
        PayoutStatus::Initiated,
        PayoutStatus::Pending,
        PayoutStatus::Success,
        PayoutStatus::Failure,
        PayoutStatus::Reversed,
        PayoutStatus::Ineligible,
    ];
}

impl PayoutFlowStatusRules for connector_flow::PayoutGet {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus] = &[PayoutStatus::Success];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus] = &[PayoutStatus::Failure];
    // PayoutGet is a poll/sync flow — all statuses are valid responses.
    const ALLOWED: &'static [PayoutStatus] = &[
        PayoutStatus::Success,
        PayoutStatus::Failure,
        PayoutStatus::Cancelled,
        PayoutStatus::Initiated,
        PayoutStatus::Expired,
        PayoutStatus::Reversed,
        PayoutStatus::Pending,
        PayoutStatus::Ineligible,
        PayoutStatus::NotPermitted,
        PayoutStatus::RequiresCreation,
        PayoutStatus::RequiresConfirmation,
        PayoutStatus::RequiresPayoutMethodData,
        PayoutStatus::RequiresFulfillment,
        PayoutStatus::RequiresVendorAccountCreation,
    ];
}

impl PayoutFlowStatusRules for connector_flow::PayoutVoid {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus] = &[PayoutStatus::Cancelled];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus] = &[PayoutStatus::Failure];
    const ALLOWED: &'static [PayoutStatus] = &[
        PayoutStatus::Pending,
        PayoutStatus::Cancelled,
        PayoutStatus::Reversed, // REVERSED
    ];
}

impl PayoutFlowStatusRules for connector_flow::PayoutCreate {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus] = &[PayoutStatus::RequiresFulfillment];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus] = &[PayoutStatus::Failure];
    const ALLOWED: &'static [PayoutStatus] = &[
        PayoutStatus::Pending,
        PayoutStatus::RequiresFulfillment,
        PayoutStatus::Failure,
        PayoutStatus::RequiresPayoutMethodData,
        PayoutStatus::RequiresConfirmation,
    ];
}

impl PayoutFlowStatusRules for connector_flow::PayoutStage {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus] =
        &[PayoutStatus::RequiresFulfillment, PayoutStatus::RequiresCreation];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus] = &[PayoutStatus::Failure];
    const ALLOWED: &'static [PayoutStatus] = &[
        PayoutStatus::Pending,
        PayoutStatus::RequiresFulfillment,
        PayoutStatus::Failure,
        PayoutStatus::RequiresCreation,
    ];
}

impl PayoutFlowStatusRules for connector_flow::PayoutCreateRecipient {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus] = &[PayoutStatus::RequiresCreation];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus] = &[PayoutStatus::Failure];
    const ALLOWED: &'static [PayoutStatus] =
        &[PayoutStatus::RequiresCreation, PayoutStatus::Failure];
}

impl PayoutFlowStatusRules for connector_flow::PayoutEligibility {
    const TERMINAL_SUCCESS_SET: &'static [PayoutStatus] = &[PayoutStatus::RequiresCreation];
    const TERMINAL_FAILURE_SET: &'static [PayoutStatus] = &[PayoutStatus::NotPermitted];
    const ALLOWED: &'static [PayoutStatus] = &[
        PayoutStatus::RequiresFulfillment,
        PayoutStatus::NotPermitted,
        PayoutStatus::RequiresCreation,
    ];
}

/// Per-flow terminal status declaration for a specific connector.
///
/// Scoped to **payment flows** (`FlowStatusRules` uses `AttemptStatus`).
/// Refund, dispute, and payout flows have their own rules traits
/// (`RefundFlowStatusRules`, `DisputeFlowStatusRules`, `PayoutFlowStatusRules`)
/// and will need separate mapping traits if enforcement is extended to them.
///
/// In Phase 3 (flag day), this trait is added as a supertrait of `PaymentCapture`,
/// `PaymentVoidV2`, etc., making it mandatory at compile time for all connectors.
/// Until then, connectors add impls voluntarily flow by flow.
///
/// Implemented by `impl_flow_status_mapping!` (in `domain_types::status_mapping`).
pub trait ConnectorTerminalMapping<Flow: FlowStatusRules> {
    /// The connector-native status type for this flow (e.g. `StripePaymentStatus`).
    type ConnectorStatus;

    /// Extra context passed into `map_attempt_status` for connectors whose mapping
    /// depends on data beyond the connector status alone (e.g. `NexinetsTransactionType`
    /// tells whether the Authorize response was a Preauth → `Authorized` or a Debit →
    /// `Charged`).
    ///
    /// For connectors with no context dependency, set `type MappingContext = ()`.
    type MappingContext;

    /// Returns the connector status that maps to a value in the flow's
    /// `TERMINAL_SUCCESS_SET`.  Verified at test time by `assert_terminal_mapping!`.
    fn success_connector_status() -> Self::ConnectorStatus;

    /// Returns the connector status that maps to a value in the flow's
    /// `TERMINAL_FAILURE_SET`.  Verified at test time by `assert_terminal_mapping!`.
    fn failure_connector_status() -> Self::ConnectorStatus;

    /// The per-flow status mapping function.  Replaces the shared
    /// `From<ConnectorStatus> for AttemptStatus` for this flow.
    ///
    /// `ctx` carries any extra request/response context the connector needs.
    /// Pass `()` for the common case where the connector status alone determines
    /// the `AttemptStatus`.
    fn map_attempt_status(
        status: Self::ConnectorStatus,
        ctx: Self::MappingContext,
    ) -> AttemptStatus;
}
