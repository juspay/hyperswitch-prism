//! Flow-specific typed statuses.
//!
//! Each flow selects distinct success, failure, and non-terminal enums
//! through [`FlowSpec`]. Invalid cross-flow statuses are not representable.

use common_enums::{AttemptStatus, DisputeStatus, PayoutStatus, RefundStatus};

use crate::connector_flow;

pub trait FlowSpec {
    const NAME: &'static str;
    type Status;
    type Success: Copy + std::fmt::Debug + Eq + Into<Self::Status>;
    type Failure: Copy + std::fmt::Debug + Eq + Into<Self::Status>;
    type NonTerminal: Copy + std::fmt::Debug + Eq + Into<Self::Status>;
}

pub trait PaymentFlowSpec: FlowSpec<Status = AttemptStatus> {}
impl<Flow> PaymentFlowSpec for Flow where Flow: FlowSpec<Status = AttemptStatus> {}

pub trait RefundFlowSpec: FlowSpec<Status = RefundStatus> {}
impl<Flow> RefundFlowSpec for Flow where Flow: FlowSpec<Status = RefundStatus> {}

pub trait DisputeFlowSpec: FlowSpec<Status = DisputeStatus> {}
impl<Flow> DisputeFlowSpec for Flow where Flow: FlowSpec<Status = DisputeStatus> {}

pub trait PayoutFlowSpec: FlowSpec<Status = PayoutStatus> {}
impl<Flow> PayoutFlowSpec for Flow where Flow: FlowSpec<Status = PayoutStatus> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectorFlowStatus<Flow: FlowSpec> {
    Success(Flow::Success),
    Failure(Flow::Failure),
    NonTerminal(Flow::NonTerminal),
}

impl<Flow: FlowSpec> ConnectorFlowStatus<Flow> {
    pub fn status(&self) -> Flow::Status {
        match self {
            Self::Success(status) => (*status).into(),
            Self::Failure(status) => (*status).into(),
            Self::NonTerminal(status) => (*status).into(),
        }
    }

    pub fn into_status(self) -> Flow::Status {
        match self {
            Self::Success(status) => status.into(),
            Self::Failure(status) => status.into(),
            Self::NonTerminal(status) => status.into(),
        }
    }
}

impl<Flow: PaymentFlowSpec> From<ConnectorFlowStatus<Flow>> for AttemptStatus {
    fn from(status: ConnectorFlowStatus<Flow>) -> Self {
        status.into_status()
    }
}
impl<Flow: RefundFlowSpec> From<ConnectorFlowStatus<Flow>> for RefundStatus {
    fn from(status: ConnectorFlowStatus<Flow>) -> Self {
        status.into_status()
    }
}
impl<Flow: DisputeFlowSpec> From<ConnectorFlowStatus<Flow>> for DisputeStatus {
    fn from(status: ConnectorFlowStatus<Flow>) -> Self {
        status.into_status()
    }
}
impl<Flow: PayoutFlowSpec> From<ConnectorFlowStatus<Flow>> for PayoutStatus {
    fn from(status: ConnectorFlowStatus<Flow>) -> Self {
        status.into_status()
    }
}

/// Connectors whose framework status is cross-checked against the
/// transformer-derived status (shadow mode) instead of replacing it.
///
/// Entries are the **type-name suffix** of the connector struct, matched
/// against the trailing segment of `std::any::type_name::<Connector>()`
/// (e.g. `...::connectors::tsys_transit::TsysTransit<...>`). Keying on the
/// type rather than a display name makes rename drift impossible: if the
/// struct is renamed the entry stops matching loudly instead of silently
/// skipping shadow mode.
pub const LIVE_STATUS_TRANSFORMER_CONNECTORS: &[&str] = &[
    "Fiservcommercehub",
    "Stripe",
    "Adyen",
    "Datatrans",
    "Cybersource",
    "Paypal",
    "Authorizedotnet",
    "TsysTransit",
];

/// `connector_type_name` is `std::any::type_name::<Connector>()` from the
/// bridge probe, e.g. `connector_integration::connectors::adyen::Adyen<
/// connector_integration::type_mem...::DefaultPCIHolder>`.
pub fn is_live_status_transformer_connector(connector_type_name: &str) -> bool {
    // Strip generic parameters first — they contain `::` themselves
    // (`Adyen<foo::Bar>`) — then take the last path segment.
    let path = connector_type_name
        .split('<')
        .next()
        .unwrap_or(connector_type_name);
    let base = path.rsplit("::").next().unwrap_or(path);
    LIVE_STATUS_TRANSFORMER_CONNECTORS.contains(&base)
}

pub const fn const_contains_str(slice: &[&str], target: &str) -> bool {
    let mut i = 0;
    while i < slice.len() {
        if str_eq(slice[i], target) {
            return true;
        }
        i += 1;
    }
    false
}

pub const fn const_contains_connector_type(slice: &[&str], connector_type: &str) -> bool {
    let mut i = 0;
    while i < slice.len() {
        if const_connector_type_eq(connector_type, slice[i]) {
            return true;
        }
        i += 1;
    }
    false
}

const fn const_connector_type_eq(connector_type: &str, expected: &str) -> bool {
    let bytes = connector_type.as_bytes();
    let expected = expected.as_bytes();
    let mut end = 0;

    while end < bytes.len() && bytes[end] != b'<' {
        end += 1;
    }
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    let mut start = end;
    while start > 0 && bytes[start - 1] != b':' {
        start -= 1;
    }
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }

    if end - start != expected.len() {
        return false;
    }

    let mut i = 0;
    while i < expected.len() {
        if bytes[start + i] != expected[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut i = 0;
    while i < left.len() {
        if left[i] != right[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[doc(hidden)]
pub mod __status_mapping {
    use super::*;

    // Payment flow statuses

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AuthorizeSuccessStatus {
        Authorized,
        Charged,
        PartialCharged,
        PartiallyAuthorized,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AuthorizeFailureStatus {
        AuthorizationFailed,
        AuthenticationFailed,
        Failure,
        IntegrityFailure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AuthorizeNonTerminalStatus {
        Started,
        AuthenticationPending,
        AuthenticationSuccessful,
        Authorizing,
        PartialChargedAndChargeable,
        Voided,
        AutoRefunded,
        Expired,
        Unresolved,
        Unspecified,
        Unknown,
        Pending,
        PaymentMethodAwaited,
        ConfirmationAwaited,
        DeviceDataCollectionPending,
    }

    impl From<AuthorizeSuccessStatus> for AttemptStatus {
        fn from(status: AuthorizeSuccessStatus) -> Self {
            match status {
                AuthorizeSuccessStatus::Authorized => Self::Authorized,
                AuthorizeSuccessStatus::Charged => Self::Charged,
                AuthorizeSuccessStatus::PartialCharged => Self::PartialCharged,
                AuthorizeSuccessStatus::PartiallyAuthorized => Self::PartiallyAuthorized,
            }
        }
    }

    impl From<AuthorizeFailureStatus> for AttemptStatus {
        fn from(status: AuthorizeFailureStatus) -> Self {
            match status {
                AuthorizeFailureStatus::AuthorizationFailed => Self::AuthorizationFailed,
                AuthorizeFailureStatus::AuthenticationFailed => Self::AuthenticationFailed,
                AuthorizeFailureStatus::Failure => Self::Failure,
                AuthorizeFailureStatus::IntegrityFailure => Self::IntegrityFailure,
            }
        }
    }

    impl From<AuthorizeNonTerminalStatus> for AttemptStatus {
        fn from(status: AuthorizeNonTerminalStatus) -> Self {
            match status {
                AuthorizeNonTerminalStatus::Started => Self::Started,
                AuthorizeNonTerminalStatus::AuthenticationPending => Self::AuthenticationPending,
                AuthorizeNonTerminalStatus::AuthenticationSuccessful => {
                    Self::AuthenticationSuccessful
                }
                AuthorizeNonTerminalStatus::Authorizing => Self::Authorizing,
                AuthorizeNonTerminalStatus::PartialChargedAndChargeable => {
                    Self::PartialChargedAndChargeable
                }
                AuthorizeNonTerminalStatus::Voided => Self::Voided,
                AuthorizeNonTerminalStatus::AutoRefunded => Self::AutoRefunded,
                AuthorizeNonTerminalStatus::Expired => Self::Expired,
                AuthorizeNonTerminalStatus::Unresolved => Self::Unresolved,
                AuthorizeNonTerminalStatus::Unspecified => Self::Unspecified,
                AuthorizeNonTerminalStatus::Unknown => Self::Unknown,
                AuthorizeNonTerminalStatus::Pending => Self::Pending,
                AuthorizeNonTerminalStatus::PaymentMethodAwaited => Self::PaymentMethodAwaited,
                AuthorizeNonTerminalStatus::ConfirmationAwaited => Self::ConfirmationAwaited,
                AuthorizeNonTerminalStatus::DeviceDataCollectionPending => {
                    Self::DeviceDataCollectionPending
                }
            }
        }
    }

    impl FlowSpec for connector_flow::Authorize {
        const NAME: &'static str = "Authorize";
        type Status = AttemptStatus;
        type Success = AuthorizeSuccessStatus;
        type Failure = AuthorizeFailureStatus;
        type NonTerminal = AuthorizeNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum CaptureSuccessStatus {
        Charged,
        PartialCharged,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum CaptureFailureStatus {
        CaptureFailed,
        Failure,
        IntegrityFailure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum CaptureNonTerminalStatus {
        CaptureInitiated,
        PartialChargedAndChargeable,
        Pending,
    }

    impl From<CaptureSuccessStatus> for AttemptStatus {
        fn from(status: CaptureSuccessStatus) -> Self {
            match status {
                CaptureSuccessStatus::Charged => Self::Charged,
                CaptureSuccessStatus::PartialCharged => Self::PartialCharged,
            }
        }
    }

    impl From<CaptureFailureStatus> for AttemptStatus {
        fn from(status: CaptureFailureStatus) -> Self {
            match status {
                CaptureFailureStatus::CaptureFailed => Self::CaptureFailed,
                CaptureFailureStatus::Failure => Self::Failure,
                CaptureFailureStatus::IntegrityFailure => Self::IntegrityFailure,
            }
        }
    }

    impl From<CaptureNonTerminalStatus> for AttemptStatus {
        fn from(status: CaptureNonTerminalStatus) -> Self {
            match status {
                CaptureNonTerminalStatus::CaptureInitiated => Self::CaptureInitiated,
                CaptureNonTerminalStatus::PartialChargedAndChargeable => {
                    Self::PartialChargedAndChargeable
                }
                CaptureNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::Capture {
        const NAME: &'static str = "Capture";
        type Status = AttemptStatus;
        type Success = CaptureSuccessStatus;
        type Failure = CaptureFailureStatus;
        type NonTerminal = CaptureNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum VoidSuccessStatus {
        Voided,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum VoidFailureStatus {
        VoidFailed,
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum VoidNonTerminalStatus {
        VoidInitiated,
        Pending,
    }

    impl From<VoidSuccessStatus> for AttemptStatus {
        fn from(status: VoidSuccessStatus) -> Self {
            match status {
                VoidSuccessStatus::Voided => Self::Voided,
            }
        }
    }

    impl From<VoidFailureStatus> for AttemptStatus {
        fn from(status: VoidFailureStatus) -> Self {
            match status {
                VoidFailureStatus::VoidFailed => Self::VoidFailed,
                VoidFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<VoidNonTerminalStatus> for AttemptStatus {
        fn from(status: VoidNonTerminalStatus) -> Self {
            match status {
                VoidNonTerminalStatus::VoidInitiated => Self::VoidInitiated,
                VoidNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::Void {
        const NAME: &'static str = "Void";
        type Status = AttemptStatus;
        type Success = VoidSuccessStatus;
        type Failure = VoidFailureStatus;
        type NonTerminal = VoidNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum VoidPCSuccessStatus {
        VoidedPostCapture,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum VoidPCFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum VoidPCNonTerminalStatus {
        VoidPostCaptureInitiated,
        Pending,
    }

    impl From<VoidPCSuccessStatus> for AttemptStatus {
        fn from(status: VoidPCSuccessStatus) -> Self {
            match status {
                VoidPCSuccessStatus::VoidedPostCapture => Self::VoidedPostCapture,
            }
        }
    }

    impl From<VoidPCFailureStatus> for AttemptStatus {
        fn from(status: VoidPCFailureStatus) -> Self {
            match status {
                VoidPCFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<VoidPCNonTerminalStatus> for AttemptStatus {
        fn from(status: VoidPCNonTerminalStatus) -> Self {
            match status {
                VoidPCNonTerminalStatus::VoidPostCaptureInitiated => Self::VoidPostCaptureInitiated,
                VoidPCNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::VoidPC {
        const NAME: &'static str = "VoidPC";
        type Status = AttemptStatus;
        type Success = VoidPCSuccessStatus;
        type Failure = VoidPCFailureStatus;
        type NonTerminal = VoidPCNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum SetupMandateSuccessStatus {
        Charged,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum SetupMandateFailureStatus {
        Failure,
        AuthorizationFailed,
        AuthenticationFailed,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum SetupMandateNonTerminalStatus {
        Started,
        AuthenticationPending,
        AuthenticationSuccessful,
        Pending,
    }

    impl From<SetupMandateSuccessStatus> for AttemptStatus {
        fn from(status: SetupMandateSuccessStatus) -> Self {
            match status {
                SetupMandateSuccessStatus::Charged => Self::Charged,
            }
        }
    }

    impl From<SetupMandateFailureStatus> for AttemptStatus {
        fn from(status: SetupMandateFailureStatus) -> Self {
            match status {
                SetupMandateFailureStatus::Failure => Self::Failure,
                SetupMandateFailureStatus::AuthorizationFailed => Self::AuthorizationFailed,
                SetupMandateFailureStatus::AuthenticationFailed => Self::AuthenticationFailed,
            }
        }
    }

    impl From<SetupMandateNonTerminalStatus> for AttemptStatus {
        fn from(status: SetupMandateNonTerminalStatus) -> Self {
            match status {
                SetupMandateNonTerminalStatus::Started => Self::Started,
                SetupMandateNonTerminalStatus::AuthenticationPending => Self::AuthenticationPending,
                SetupMandateNonTerminalStatus::AuthenticationSuccessful => {
                    Self::AuthenticationSuccessful
                }
                SetupMandateNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::SetupMandate {
        const NAME: &'static str = "SetupMandate";
        type Status = AttemptStatus;
        type Success = SetupMandateSuccessStatus;
        type Failure = SetupMandateFailureStatus;
        type NonTerminal = SetupMandateNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RepeatPaymentSuccessStatus {
        Charged,
        PartialCharged,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RepeatPaymentFailureStatus {
        Failure,
        AuthorizationFailed,
        IntegrityFailure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RepeatPaymentNonTerminalStatus {
        Started,
        AuthenticationPending,
        Authorized,
        PartiallyAuthorized,
        Authorizing,
        PartialChargedAndChargeable,
        Pending,
    }

    impl From<RepeatPaymentSuccessStatus> for AttemptStatus {
        fn from(status: RepeatPaymentSuccessStatus) -> Self {
            match status {
                RepeatPaymentSuccessStatus::Charged => Self::Charged,
                RepeatPaymentSuccessStatus::PartialCharged => Self::PartialCharged,
            }
        }
    }

    impl From<RepeatPaymentFailureStatus> for AttemptStatus {
        fn from(status: RepeatPaymentFailureStatus) -> Self {
            match status {
                RepeatPaymentFailureStatus::Failure => Self::Failure,
                RepeatPaymentFailureStatus::AuthorizationFailed => Self::AuthorizationFailed,
                RepeatPaymentFailureStatus::IntegrityFailure => Self::IntegrityFailure,
            }
        }
    }

    impl From<RepeatPaymentNonTerminalStatus> for AttemptStatus {
        fn from(status: RepeatPaymentNonTerminalStatus) -> Self {
            match status {
                RepeatPaymentNonTerminalStatus::Started => Self::Started,
                RepeatPaymentNonTerminalStatus::AuthenticationPending => {
                    Self::AuthenticationPending
                }
                RepeatPaymentNonTerminalStatus::Authorized => Self::Authorized,
                RepeatPaymentNonTerminalStatus::PartiallyAuthorized => Self::PartiallyAuthorized,
                RepeatPaymentNonTerminalStatus::Authorizing => Self::Authorizing,
                RepeatPaymentNonTerminalStatus::PartialChargedAndChargeable => {
                    Self::PartialChargedAndChargeable
                }
                RepeatPaymentNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::RepeatPayment {
        const NAME: &'static str = "RepeatPayment";
        type Status = AttemptStatus;
        type Success = RepeatPaymentSuccessStatus;
        type Failure = RepeatPaymentFailureStatus;
        type NonTerminal = RepeatPaymentNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum CreateOrderSuccessStatus {
        Charged,
        Authorized,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum CreateOrderFailureStatus {
        Failure,
        AuthenticationFailed,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum CreateOrderNonTerminalStatus {
        Started,
        AuthenticationPending,
        AuthenticationSuccessful,
        Pending,
    }

    impl From<CreateOrderSuccessStatus> for AttemptStatus {
        fn from(status: CreateOrderSuccessStatus) -> Self {
            match status {
                CreateOrderSuccessStatus::Charged => Self::Charged,
                CreateOrderSuccessStatus::Authorized => Self::Authorized,
            }
        }
    }

    impl From<CreateOrderFailureStatus> for AttemptStatus {
        fn from(status: CreateOrderFailureStatus) -> Self {
            match status {
                CreateOrderFailureStatus::Failure => Self::Failure,
                CreateOrderFailureStatus::AuthenticationFailed => Self::AuthenticationFailed,
            }
        }
    }

    impl From<CreateOrderNonTerminalStatus> for AttemptStatus {
        fn from(status: CreateOrderNonTerminalStatus) -> Self {
            match status {
                CreateOrderNonTerminalStatus::Started => Self::Started,
                CreateOrderNonTerminalStatus::AuthenticationPending => Self::AuthenticationPending,
                CreateOrderNonTerminalStatus::AuthenticationSuccessful => {
                    Self::AuthenticationSuccessful
                }
                CreateOrderNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::CreateOrder {
        const NAME: &'static str = "CreateOrder";
        type Status = AttemptStatus;
        type Success = CreateOrderSuccessStatus;
        type Failure = CreateOrderFailureStatus;
        type NonTerminal = CreateOrderNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PreAuthenticateSuccessStatus {
        AuthenticationSuccessful,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PreAuthenticateFailureStatus {
        AuthenticationFailed,
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PreAuthenticateNonTerminalStatus {
        AuthenticationPending,
        Authorized,
        Charged,
        Pending,
    }

    impl From<PreAuthenticateSuccessStatus> for AttemptStatus {
        fn from(status: PreAuthenticateSuccessStatus) -> Self {
            match status {
                PreAuthenticateSuccessStatus::AuthenticationSuccessful => {
                    Self::AuthenticationSuccessful
                }
            }
        }
    }

    impl From<PreAuthenticateFailureStatus> for AttemptStatus {
        fn from(status: PreAuthenticateFailureStatus) -> Self {
            match status {
                PreAuthenticateFailureStatus::AuthenticationFailed => Self::AuthenticationFailed,
                PreAuthenticateFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<PreAuthenticateNonTerminalStatus> for AttemptStatus {
        fn from(status: PreAuthenticateNonTerminalStatus) -> Self {
            match status {
                PreAuthenticateNonTerminalStatus::AuthenticationPending => {
                    Self::AuthenticationPending
                }
                PreAuthenticateNonTerminalStatus::Authorized => Self::Authorized,
                PreAuthenticateNonTerminalStatus::Charged => Self::Charged,
                PreAuthenticateNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::PreAuthenticate {
        const NAME: &'static str = "PreAuthenticate";
        type Status = AttemptStatus;
        type Success = PreAuthenticateSuccessStatus;
        type Failure = PreAuthenticateFailureStatus;
        type NonTerminal = PreAuthenticateNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AuthenticateSuccessStatus {
        AuthenticationSuccessful,
        Authorized,
        Charged,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AuthenticateFailureStatus {
        AuthenticationFailed,
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AuthenticateNonTerminalStatus {
        AuthenticationPending,
        Pending,
    }

    impl From<AuthenticateSuccessStatus> for AttemptStatus {
        fn from(status: AuthenticateSuccessStatus) -> Self {
            match status {
                AuthenticateSuccessStatus::AuthenticationSuccessful => {
                    Self::AuthenticationSuccessful
                }
                AuthenticateSuccessStatus::Authorized => Self::Authorized,
                AuthenticateSuccessStatus::Charged => Self::Charged,
            }
        }
    }

    impl From<AuthenticateFailureStatus> for AttemptStatus {
        fn from(status: AuthenticateFailureStatus) -> Self {
            match status {
                AuthenticateFailureStatus::AuthenticationFailed => Self::AuthenticationFailed,
                AuthenticateFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<AuthenticateNonTerminalStatus> for AttemptStatus {
        fn from(status: AuthenticateNonTerminalStatus) -> Self {
            match status {
                AuthenticateNonTerminalStatus::AuthenticationPending => Self::AuthenticationPending,
                AuthenticateNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::Authenticate {
        const NAME: &'static str = "Authenticate";
        type Status = AttemptStatus;
        type Success = AuthenticateSuccessStatus;
        type Failure = AuthenticateFailureStatus;
        type NonTerminal = AuthenticateNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PostAuthenticateSuccessStatus {
        AuthenticationSuccessful,
        Authorized,
        Charged,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PostAuthenticateFailureStatus {
        AuthenticationFailed,
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PostAuthenticateNonTerminalStatus {
        AuthenticationPending,
        Voided,
        Pending,
    }

    impl From<PostAuthenticateSuccessStatus> for AttemptStatus {
        fn from(status: PostAuthenticateSuccessStatus) -> Self {
            match status {
                PostAuthenticateSuccessStatus::AuthenticationSuccessful => {
                    Self::AuthenticationSuccessful
                }
                PostAuthenticateSuccessStatus::Authorized => Self::Authorized,
                PostAuthenticateSuccessStatus::Charged => Self::Charged,
            }
        }
    }

    impl From<PostAuthenticateFailureStatus> for AttemptStatus {
        fn from(status: PostAuthenticateFailureStatus) -> Self {
            match status {
                PostAuthenticateFailureStatus::AuthenticationFailed => Self::AuthenticationFailed,
                PostAuthenticateFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<PostAuthenticateNonTerminalStatus> for AttemptStatus {
        fn from(status: PostAuthenticateNonTerminalStatus) -> Self {
            match status {
                PostAuthenticateNonTerminalStatus::AuthenticationPending => {
                    Self::AuthenticationPending
                }
                PostAuthenticateNonTerminalStatus::Voided => Self::Voided,
                PostAuthenticateNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::PostAuthenticate {
        const NAME: &'static str = "PostAuthenticate";
        type Status = AttemptStatus;
        type Success = PostAuthenticateSuccessStatus;
        type Failure = PostAuthenticateFailureStatus;
        type NonTerminal = PostAuthenticateNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PSyncSuccessStatus {
        Authorized,
        Charged,
        PartialCharged,
        PartiallyAuthorized,
        Voided,
        AutoRefunded,
        VoidedPostCapture,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PSyncFailureStatus {
        AuthorizationFailed,
        AuthenticationFailed,
        CaptureFailed,
        VoidFailed,
        Failure,
        IntegrityFailure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PSyncNonTerminalStatus {
        Started,
        AuthenticationPending,
        AuthenticationSuccessful,
        Authorizing,
        CaptureInitiated,
        PartialChargedAndChargeable,
        VoidInitiated,
        VoidPostCaptureInitiated,
        Expired,
        Unresolved,
        Unspecified,
        Unknown,
        Pending,
        PaymentMethodAwaited,
        ConfirmationAwaited,
        DeviceDataCollectionPending,
        CodInitiated,
    }

    impl From<PSyncSuccessStatus> for AttemptStatus {
        fn from(status: PSyncSuccessStatus) -> Self {
            match status {
                PSyncSuccessStatus::Authorized => Self::Authorized,
                PSyncSuccessStatus::Charged => Self::Charged,
                PSyncSuccessStatus::PartialCharged => Self::PartialCharged,
                PSyncSuccessStatus::PartiallyAuthorized => Self::PartiallyAuthorized,
                PSyncSuccessStatus::Voided => Self::Voided,
                PSyncSuccessStatus::AutoRefunded => Self::AutoRefunded,
                PSyncSuccessStatus::VoidedPostCapture => Self::VoidedPostCapture,
            }
        }
    }

    impl From<PSyncFailureStatus> for AttemptStatus {
        fn from(status: PSyncFailureStatus) -> Self {
            match status {
                PSyncFailureStatus::AuthorizationFailed => Self::AuthorizationFailed,
                PSyncFailureStatus::AuthenticationFailed => Self::AuthenticationFailed,
                PSyncFailureStatus::CaptureFailed => Self::CaptureFailed,
                PSyncFailureStatus::VoidFailed => Self::VoidFailed,
                PSyncFailureStatus::Failure => Self::Failure,
                PSyncFailureStatus::IntegrityFailure => Self::IntegrityFailure,
            }
        }
    }

    impl From<PSyncNonTerminalStatus> for AttemptStatus {
        fn from(status: PSyncNonTerminalStatus) -> Self {
            match status {
                PSyncNonTerminalStatus::Started => Self::Started,
                PSyncNonTerminalStatus::AuthenticationPending => Self::AuthenticationPending,
                PSyncNonTerminalStatus::AuthenticationSuccessful => Self::AuthenticationSuccessful,
                PSyncNonTerminalStatus::Authorizing => Self::Authorizing,
                PSyncNonTerminalStatus::CaptureInitiated => Self::CaptureInitiated,
                PSyncNonTerminalStatus::PartialChargedAndChargeable => {
                    Self::PartialChargedAndChargeable
                }
                PSyncNonTerminalStatus::VoidInitiated => Self::VoidInitiated,
                PSyncNonTerminalStatus::VoidPostCaptureInitiated => Self::VoidPostCaptureInitiated,
                PSyncNonTerminalStatus::Expired => Self::Expired,
                PSyncNonTerminalStatus::Unresolved => Self::Unresolved,
                PSyncNonTerminalStatus::Unspecified => Self::Unspecified,
                PSyncNonTerminalStatus::Unknown => Self::Unknown,
                PSyncNonTerminalStatus::Pending => Self::Pending,
                PSyncNonTerminalStatus::PaymentMethodAwaited => Self::PaymentMethodAwaited,
                PSyncNonTerminalStatus::ConfirmationAwaited => Self::ConfirmationAwaited,
                PSyncNonTerminalStatus::DeviceDataCollectionPending => {
                    Self::DeviceDataCollectionPending
                }
                PSyncNonTerminalStatus::CodInitiated => Self::CodInitiated,
            }
        }
    }

    impl FlowSpec for connector_flow::PSync {
        const NAME: &'static str = "PSync";
        type Status = AttemptStatus;
        type Success = PSyncSuccessStatus;
        type Failure = PSyncFailureStatus;
        type NonTerminal = PSyncNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum IncrementalAuthorizationSuccessStatus {
        Authorized,
        Charged,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum IncrementalAuthorizationFailureStatus {
        AuthorizationFailed,
        Failure,
        IntegrityFailure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum IncrementalAuthorizationNonTerminalStatus {
        AuthenticationPending,
        Authorizing,
        Voided,
        Pending,
        ConfirmationAwaited,
    }

    impl From<IncrementalAuthorizationSuccessStatus> for AttemptStatus {
        fn from(status: IncrementalAuthorizationSuccessStatus) -> Self {
            match status {
                IncrementalAuthorizationSuccessStatus::Authorized => Self::Authorized,
                IncrementalAuthorizationSuccessStatus::Charged => Self::Charged,
            }
        }
    }

    impl From<IncrementalAuthorizationFailureStatus> for AttemptStatus {
        fn from(status: IncrementalAuthorizationFailureStatus) -> Self {
            match status {
                IncrementalAuthorizationFailureStatus::AuthorizationFailed => {
                    Self::AuthorizationFailed
                }
                IncrementalAuthorizationFailureStatus::Failure => Self::Failure,
                IncrementalAuthorizationFailureStatus::IntegrityFailure => Self::IntegrityFailure,
            }
        }
    }

    impl From<IncrementalAuthorizationNonTerminalStatus> for AttemptStatus {
        fn from(status: IncrementalAuthorizationNonTerminalStatus) -> Self {
            match status {
                IncrementalAuthorizationNonTerminalStatus::AuthenticationPending => {
                    Self::AuthenticationPending
                }
                IncrementalAuthorizationNonTerminalStatus::Authorizing => Self::Authorizing,
                IncrementalAuthorizationNonTerminalStatus::Voided => Self::Voided,
                IncrementalAuthorizationNonTerminalStatus::Pending => Self::Pending,
                IncrementalAuthorizationNonTerminalStatus::ConfirmationAwaited => {
                    Self::ConfirmationAwaited
                }
            }
        }
    }

    impl FlowSpec for connector_flow::IncrementalAuthorization {
        const NAME: &'static str = "IncrementalAuthorization";
        type Status = AttemptStatus;
        type Success = IncrementalAuthorizationSuccessStatus;
        type Failure = IncrementalAuthorizationFailureStatus;
        type NonTerminal = IncrementalAuthorizationNonTerminalStatus;
    }

    // Refund flow statuses

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RefundSuccessStatus {
        Success,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RefundFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RefundNonTerminalStatus {
        Pending,
        ManualReview,
        TransactionFailure,
    }

    impl From<RefundSuccessStatus> for RefundStatus {
        fn from(status: RefundSuccessStatus) -> Self {
            match status {
                RefundSuccessStatus::Success => Self::Success,
            }
        }
    }

    impl From<RefundFailureStatus> for RefundStatus {
        fn from(status: RefundFailureStatus) -> Self {
            match status {
                RefundFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<RefundNonTerminalStatus> for RefundStatus {
        fn from(status: RefundNonTerminalStatus) -> Self {
            match status {
                RefundNonTerminalStatus::Pending => Self::Pending,
                RefundNonTerminalStatus::ManualReview => Self::ManualReview,
                RefundNonTerminalStatus::TransactionFailure => Self::TransactionFailure,
            }
        }
    }

    impl FlowSpec for connector_flow::Refund {
        const NAME: &'static str = "Refund";
        type Status = RefundStatus;
        type Success = RefundSuccessStatus;
        type Failure = RefundFailureStatus;
        type NonTerminal = RefundNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RSyncSuccessStatus {
        Success,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RSyncFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum RSyncNonTerminalStatus {
        Pending,
        ManualReview,
        TransactionFailure,
    }

    impl From<RSyncSuccessStatus> for RefundStatus {
        fn from(status: RSyncSuccessStatus) -> Self {
            match status {
                RSyncSuccessStatus::Success => Self::Success,
            }
        }
    }

    impl From<RSyncFailureStatus> for RefundStatus {
        fn from(status: RSyncFailureStatus) -> Self {
            match status {
                RSyncFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<RSyncNonTerminalStatus> for RefundStatus {
        fn from(status: RSyncNonTerminalStatus) -> Self {
            match status {
                RSyncNonTerminalStatus::Pending => Self::Pending,
                RSyncNonTerminalStatus::ManualReview => Self::ManualReview,
                RSyncNonTerminalStatus::TransactionFailure => Self::TransactionFailure,
            }
        }
    }

    impl FlowSpec for connector_flow::RSync {
        const NAME: &'static str = "RSync";
        type Status = RefundStatus;
        type Success = RSyncSuccessStatus;
        type Failure = RSyncFailureStatus;
        type NonTerminal = RSyncNonTerminalStatus;
    }

    // Dispute flow statuses

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AcceptSuccessStatus {
        DisputeAccepted,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AcceptFailureStatus {
        DisputeLost,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum AcceptNonTerminalStatus {}

    impl From<AcceptSuccessStatus> for DisputeStatus {
        fn from(status: AcceptSuccessStatus) -> Self {
            match status {
                AcceptSuccessStatus::DisputeAccepted => Self::DisputeAccepted,
            }
        }
    }

    impl From<AcceptFailureStatus> for DisputeStatus {
        fn from(status: AcceptFailureStatus) -> Self {
            match status {
                AcceptFailureStatus::DisputeLost => Self::DisputeLost,
            }
        }
    }

    impl From<AcceptNonTerminalStatus> for DisputeStatus {
        fn from(status: AcceptNonTerminalStatus) -> Self {
            match status {}
        }
    }

    impl FlowSpec for connector_flow::Accept {
        const NAME: &'static str = "Accept";
        type Status = DisputeStatus;
        type Success = AcceptSuccessStatus;
        type Failure = AcceptFailureStatus;
        type NonTerminal = AcceptNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum SubmitEvidenceSuccessStatus {
        DisputeChallenged,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum SubmitEvidenceFailureStatus {
        DisputeLost,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum SubmitEvidenceNonTerminalStatus {}

    impl From<SubmitEvidenceSuccessStatus> for DisputeStatus {
        fn from(status: SubmitEvidenceSuccessStatus) -> Self {
            match status {
                SubmitEvidenceSuccessStatus::DisputeChallenged => Self::DisputeChallenged,
            }
        }
    }

    impl From<SubmitEvidenceFailureStatus> for DisputeStatus {
        fn from(status: SubmitEvidenceFailureStatus) -> Self {
            match status {
                SubmitEvidenceFailureStatus::DisputeLost => Self::DisputeLost,
            }
        }
    }

    impl From<SubmitEvidenceNonTerminalStatus> for DisputeStatus {
        fn from(status: SubmitEvidenceNonTerminalStatus) -> Self {
            match status {}
        }
    }

    impl FlowSpec for connector_flow::SubmitEvidence {
        const NAME: &'static str = "SubmitEvidence";
        type Status = DisputeStatus;
        type Success = SubmitEvidenceSuccessStatus;
        type Failure = SubmitEvidenceFailureStatus;
        type NonTerminal = SubmitEvidenceNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum DefendDisputeSuccessStatus {
        DisputeWon,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum DefendDisputeFailureStatus {
        DisputeLost,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum DefendDisputeNonTerminalStatus {}

    impl From<DefendDisputeSuccessStatus> for DisputeStatus {
        fn from(status: DefendDisputeSuccessStatus) -> Self {
            match status {
                DefendDisputeSuccessStatus::DisputeWon => Self::DisputeWon,
            }
        }
    }

    impl From<DefendDisputeFailureStatus> for DisputeStatus {
        fn from(status: DefendDisputeFailureStatus) -> Self {
            match status {
                DefendDisputeFailureStatus::DisputeLost => Self::DisputeLost,
            }
        }
    }

    impl From<DefendDisputeNonTerminalStatus> for DisputeStatus {
        fn from(status: DefendDisputeNonTerminalStatus) -> Self {
            match status {}
        }
    }

    impl FlowSpec for connector_flow::DefendDispute {
        const NAME: &'static str = "DefendDispute";
        type Status = DisputeStatus;
        type Success = DefendDisputeSuccessStatus;
        type Failure = DefendDisputeFailureStatus;
        type NonTerminal = DefendDisputeNonTerminalStatus;
    }

    // Payout flow statuses

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutTransferSuccessStatus {
        Success,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutTransferFailureStatus {
        Failure,
        Expired,
        Reversed,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutTransferNonTerminalStatus {
        Initiated,
        Pending,
        Ineligible,
    }

    impl From<PayoutTransferSuccessStatus> for PayoutStatus {
        fn from(status: PayoutTransferSuccessStatus) -> Self {
            match status {
                PayoutTransferSuccessStatus::Success => Self::Success,
            }
        }
    }

    impl From<PayoutTransferFailureStatus> for PayoutStatus {
        fn from(status: PayoutTransferFailureStatus) -> Self {
            match status {
                PayoutTransferFailureStatus::Failure => Self::Failure,
                PayoutTransferFailureStatus::Expired => Self::Expired,
                PayoutTransferFailureStatus::Reversed => Self::Reversed,
            }
        }
    }

    impl From<PayoutTransferNonTerminalStatus> for PayoutStatus {
        fn from(status: PayoutTransferNonTerminalStatus) -> Self {
            match status {
                PayoutTransferNonTerminalStatus::Initiated => Self::Initiated,
                PayoutTransferNonTerminalStatus::Pending => Self::Pending,
                PayoutTransferNonTerminalStatus::Ineligible => Self::Ineligible,
            }
        }
    }

    impl FlowSpec for connector_flow::PayoutTransfer {
        const NAME: &'static str = "PayoutTransfer";
        type Status = PayoutStatus;
        type Success = PayoutTransferSuccessStatus;
        type Failure = PayoutTransferFailureStatus;
        type NonTerminal = PayoutTransferNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutGetSuccessStatus {
        Success,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutGetFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutGetNonTerminalStatus {
        Cancelled,
        Initiated,
        Expired,
        Reversed,
        Pending,
        Ineligible,
        NotPermitted,
        RequiresCreation,
        RequiresConfirmation,
        RequiresPayoutMethodData,
        RequiresFulfillment,
        RequiresVendorAccountCreation,
    }

    impl From<PayoutGetSuccessStatus> for PayoutStatus {
        fn from(status: PayoutGetSuccessStatus) -> Self {
            match status {
                PayoutGetSuccessStatus::Success => Self::Success,
            }
        }
    }

    impl From<PayoutGetFailureStatus> for PayoutStatus {
        fn from(status: PayoutGetFailureStatus) -> Self {
            match status {
                PayoutGetFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<PayoutGetNonTerminalStatus> for PayoutStatus {
        fn from(status: PayoutGetNonTerminalStatus) -> Self {
            match status {
                PayoutGetNonTerminalStatus::Cancelled => Self::Cancelled,
                PayoutGetNonTerminalStatus::Initiated => Self::Initiated,
                PayoutGetNonTerminalStatus::Expired => Self::Expired,
                PayoutGetNonTerminalStatus::Reversed => Self::Reversed,
                PayoutGetNonTerminalStatus::Pending => Self::Pending,
                PayoutGetNonTerminalStatus::Ineligible => Self::Ineligible,
                PayoutGetNonTerminalStatus::NotPermitted => Self::NotPermitted,
                PayoutGetNonTerminalStatus::RequiresCreation => Self::RequiresCreation,
                PayoutGetNonTerminalStatus::RequiresConfirmation => Self::RequiresConfirmation,
                PayoutGetNonTerminalStatus::RequiresPayoutMethodData => {
                    Self::RequiresPayoutMethodData
                }
                PayoutGetNonTerminalStatus::RequiresFulfillment => Self::RequiresFulfillment,
                PayoutGetNonTerminalStatus::RequiresVendorAccountCreation => {
                    Self::RequiresVendorAccountCreation
                }
            }
        }
    }

    impl FlowSpec for connector_flow::PayoutGet {
        const NAME: &'static str = "PayoutGet";
        type Status = PayoutStatus;
        type Success = PayoutGetSuccessStatus;
        type Failure = PayoutGetFailureStatus;
        type NonTerminal = PayoutGetNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutVoidSuccessStatus {
        Cancelled,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutVoidFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutVoidNonTerminalStatus {
        Pending,
        Reversed,
    }

    impl From<PayoutVoidSuccessStatus> for PayoutStatus {
        fn from(status: PayoutVoidSuccessStatus) -> Self {
            match status {
                PayoutVoidSuccessStatus::Cancelled => Self::Cancelled,
            }
        }
    }

    impl From<PayoutVoidFailureStatus> for PayoutStatus {
        fn from(status: PayoutVoidFailureStatus) -> Self {
            match status {
                PayoutVoidFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<PayoutVoidNonTerminalStatus> for PayoutStatus {
        fn from(status: PayoutVoidNonTerminalStatus) -> Self {
            match status {
                PayoutVoidNonTerminalStatus::Pending => Self::Pending,
                PayoutVoidNonTerminalStatus::Reversed => Self::Reversed,
            }
        }
    }

    impl FlowSpec for connector_flow::PayoutVoid {
        const NAME: &'static str = "PayoutVoid";
        type Status = PayoutStatus;
        type Success = PayoutVoidSuccessStatus;
        type Failure = PayoutVoidFailureStatus;
        type NonTerminal = PayoutVoidNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutCreateSuccessStatus {
        RequiresFulfillment,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutCreateFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutCreateNonTerminalStatus {
        Pending,
        RequiresPayoutMethodData,
        RequiresConfirmation,
    }

    impl From<PayoutCreateSuccessStatus> for PayoutStatus {
        fn from(status: PayoutCreateSuccessStatus) -> Self {
            match status {
                PayoutCreateSuccessStatus::RequiresFulfillment => Self::RequiresFulfillment,
            }
        }
    }

    impl From<PayoutCreateFailureStatus> for PayoutStatus {
        fn from(status: PayoutCreateFailureStatus) -> Self {
            match status {
                PayoutCreateFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<PayoutCreateNonTerminalStatus> for PayoutStatus {
        fn from(status: PayoutCreateNonTerminalStatus) -> Self {
            match status {
                PayoutCreateNonTerminalStatus::Pending => Self::Pending,
                PayoutCreateNonTerminalStatus::RequiresPayoutMethodData => {
                    Self::RequiresPayoutMethodData
                }
                PayoutCreateNonTerminalStatus::RequiresConfirmation => Self::RequiresConfirmation,
            }
        }
    }

    impl FlowSpec for connector_flow::PayoutCreate {
        const NAME: &'static str = "PayoutCreate";
        type Status = PayoutStatus;
        type Success = PayoutCreateSuccessStatus;
        type Failure = PayoutCreateFailureStatus;
        type NonTerminal = PayoutCreateNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutStageSuccessStatus {
        RequiresFulfillment,
        RequiresCreation,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutStageFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutStageNonTerminalStatus {
        Pending,
    }

    impl From<PayoutStageSuccessStatus> for PayoutStatus {
        fn from(status: PayoutStageSuccessStatus) -> Self {
            match status {
                PayoutStageSuccessStatus::RequiresFulfillment => Self::RequiresFulfillment,
                PayoutStageSuccessStatus::RequiresCreation => Self::RequiresCreation,
            }
        }
    }

    impl From<PayoutStageFailureStatus> for PayoutStatus {
        fn from(status: PayoutStageFailureStatus) -> Self {
            match status {
                PayoutStageFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<PayoutStageNonTerminalStatus> for PayoutStatus {
        fn from(status: PayoutStageNonTerminalStatus) -> Self {
            match status {
                PayoutStageNonTerminalStatus::Pending => Self::Pending,
            }
        }
    }

    impl FlowSpec for connector_flow::PayoutStage {
        const NAME: &'static str = "PayoutStage";
        type Status = PayoutStatus;
        type Success = PayoutStageSuccessStatus;
        type Failure = PayoutStageFailureStatus;
        type NonTerminal = PayoutStageNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutCreateRecipientSuccessStatus {
        RequiresCreation,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutCreateRecipientFailureStatus {
        Failure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutCreateRecipientNonTerminalStatus {}

    impl From<PayoutCreateRecipientSuccessStatus> for PayoutStatus {
        fn from(status: PayoutCreateRecipientSuccessStatus) -> Self {
            match status {
                PayoutCreateRecipientSuccessStatus::RequiresCreation => Self::RequiresCreation,
            }
        }
    }

    impl From<PayoutCreateRecipientFailureStatus> for PayoutStatus {
        fn from(status: PayoutCreateRecipientFailureStatus) -> Self {
            match status {
                PayoutCreateRecipientFailureStatus::Failure => Self::Failure,
            }
        }
    }

    impl From<PayoutCreateRecipientNonTerminalStatus> for PayoutStatus {
        fn from(status: PayoutCreateRecipientNonTerminalStatus) -> Self {
            match status {}
        }
    }

    impl FlowSpec for connector_flow::PayoutCreateRecipient {
        const NAME: &'static str = "PayoutCreateRecipient";
        type Status = PayoutStatus;
        type Success = PayoutCreateRecipientSuccessStatus;
        type Failure = PayoutCreateRecipientFailureStatus;
        type NonTerminal = PayoutCreateRecipientNonTerminalStatus;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutEligibilitySuccessStatus {
        RequiresCreation,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutEligibilityFailureStatus {
        NotPermitted,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
    pub enum PayoutEligibilityNonTerminalStatus {
        RequiresFulfillment,
    }

    impl From<PayoutEligibilitySuccessStatus> for PayoutStatus {
        fn from(status: PayoutEligibilitySuccessStatus) -> Self {
            match status {
                PayoutEligibilitySuccessStatus::RequiresCreation => Self::RequiresCreation,
            }
        }
    }

    impl From<PayoutEligibilityFailureStatus> for PayoutStatus {
        fn from(status: PayoutEligibilityFailureStatus) -> Self {
            match status {
                PayoutEligibilityFailureStatus::NotPermitted => Self::NotPermitted,
            }
        }
    }

    impl From<PayoutEligibilityNonTerminalStatus> for PayoutStatus {
        fn from(status: PayoutEligibilityNonTerminalStatus) -> Self {
            match status {
                PayoutEligibilityNonTerminalStatus::RequiresFulfillment => {
                    Self::RequiresFulfillment
                }
            }
        }
    }

    impl FlowSpec for connector_flow::PayoutEligibility {
        const NAME: &'static str = "PayoutEligibility";
        type Status = PayoutStatus;
        type Success = PayoutEligibilitySuccessStatus;
        type Failure = PayoutEligibilityFailureStatus;
        type NonTerminal = PayoutEligibilityNonTerminalStatus;
    }
} // mod __status_mapping

pub trait ConnectorRuntimeStatusMapping<Flow: FlowSpec, Request, Response> {
    fn map_runtime_status<CommonData>(
        common_data: &CommonData,
        request: &Request,
        response: &Response,
        http_status_code: u16,
    ) -> Result<ConnectorFlowStatus<Flow>, crate::ConnectorError>
    where
        CommonData: FlowStatusReader<Flow::Status>;
}

pub trait FlowStatusReader<Status> {
    fn current_mapped_flow_status(&self) -> Status;
    fn connector_request_reference_id(&self) -> Option<&str> {
        None
    }
}

pub trait FlowStatusSetter<Flow, Status> {
    fn set_mapped_flow_status(&mut self, status: Status) -> Result<(), crate::ConnectorError>;
}
