use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

/// Request-scoped accumulator for time spent in outbound connector calls.
///
/// The inner counter is atomic so the same request context can be cloned into
/// concurrent connector work without locks.
#[derive(Clone, Debug, Default)]
pub struct ConnectorLatencyTracker {
    nanos: Arc<AtomicU64>,
    outgoing_millis: Arc<AtomicU64>,
}

impl ConnectorLatencyTracker {
    pub fn add_connector_time(&self, elapsed: Duration) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.nanos.fetch_add(nanos, Ordering::Relaxed);
    }

    pub fn connector_time(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::Relaxed))
    }

    /// Add the exact millisecond value emitted on an outgoing golden log line.
    pub fn add_outgoing_latency_millis(&self, elapsed_millis: u64) {
        // The closure always returns `Some`, so the update cannot fail.
        let _ = self
            .outgoing_millis
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_add(elapsed_millis))
            });
    }

    /// Sum of the `latency_ms` values emitted by outgoing calls in this request.
    pub fn outgoing_latency_millis(&self) -> u64 {
        self.outgoing_millis.load(Ordering::Relaxed)
    }
}
