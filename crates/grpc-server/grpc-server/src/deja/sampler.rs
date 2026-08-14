//! Per-request recording sampler seam.
//!
//! The déjà library does not ship this trait (it is the integrator's policy), so it is
//! declared here. The concrete Superposition-backed implementation and its installation
//! land with the boot/sink change; until then the ingress layer records every request in
//! record mode (a `None` sampler).

use std::{future::Future, pin::Pin};

/// Facts the sampler decides on. Deliberately minimal and cheap to build.
pub struct RequestRecordingFacts {
    pub request_id: String,
    pub rpc: String,
}

/// Decides whether a given request should be recorded. Consulted once per request in
/// record mode, before the handler runs.
pub trait RequestRecordingSampler: Send + Sync {
    fn should_record(
        &self,
        facts: RequestRecordingFacts,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}
