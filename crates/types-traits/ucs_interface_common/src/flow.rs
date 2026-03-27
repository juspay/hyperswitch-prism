use common_utils::events::FlowName;
use domain_types::connector_flow::{
    Accept, Authenticate, Authorize, Capture, CreateOrder, CreateSessionToken, DefendDispute,
    IncrementalAuthorization, MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate,
    PreAuthenticate, RSync, Refund, RepeatPayment, SdkSessionToken, SetupMandate, SubmitEvidence,
    Void, VoidPC,
};
use ucs_env::configs;

pub fn service_type_str(service_type: &configs::ServiceType) -> &'static str {
    match service_type {
        configs::ServiceType::Grpc => "grpc",
        configs::ServiceType::Http => "http",
    }
}

/// Maps a flow marker type (compile-time generic) to its corresponding FlowName enum variant.
pub fn flow_marker_to_flow_name<F>() -> FlowName
where
    F: 'static,
{
    let type_id = std::any::TypeId::of::<F>();

    if type_id == std::any::TypeId::of::<Authorize>() {
        FlowName::Authorize
    } else if type_id == std::any::TypeId::of::<PSync>() {
        FlowName::Psync
    } else if type_id == std::any::TypeId::of::<RSync>() {
        FlowName::Rsync
    } else if type_id == std::any::TypeId::of::<Void>() {
        FlowName::Void
    } else if type_id == std::any::TypeId::of::<VoidPC>() {
        FlowName::VoidPostCapture
    } else if type_id == std::any::TypeId::of::<Refund>() {
        FlowName::Refund
    } else if type_id == std::any::TypeId::of::<Capture>() {
        FlowName::Capture
    } else if type_id == std::any::TypeId::of::<SetupMandate>() {
        FlowName::SetupMandate
    } else if type_id == std::any::TypeId::of::<RepeatPayment>() {
        FlowName::RepeatPayment
    } else if type_id == std::any::TypeId::of::<CreateOrder>() {
        FlowName::CreateOrder
    } else if type_id == std::any::TypeId::of::<CreateSessionToken>() {
        FlowName::CreateSessionToken
    } else if type_id == std::any::TypeId::of::<Accept>() {
        FlowName::AcceptDispute
    } else if type_id == std::any::TypeId::of::<DefendDispute>() {
        FlowName::DefendDispute
    } else if type_id == std::any::TypeId::of::<SubmitEvidence>() {
        FlowName::SubmitEvidence
    } else if type_id == std::any::TypeId::of::<PaymentMethodToken>() {
        FlowName::PaymentMethodToken
    } else if type_id == std::any::TypeId::of::<PreAuthenticate>() {
        FlowName::PreAuthenticate
    } else if type_id == std::any::TypeId::of::<Authenticate>() {
        FlowName::Authenticate
    } else if type_id == std::any::TypeId::of::<PostAuthenticate>() {
        FlowName::PostAuthenticate
    } else if type_id == std::any::TypeId::of::<SdkSessionToken>() {
        FlowName::SdkSessionToken
    } else if type_id == std::any::TypeId::of::<IncrementalAuthorization>() {
        FlowName::IncrementalAuthorization
    } else if type_id == std::any::TypeId::of::<MandateRevoke>() {
        FlowName::MandateRevoke
    } else {
        tracing::warn!("Unknown flow marker type: {}", std::any::type_name::<F>());
        FlowName::Unknown
    }
}
