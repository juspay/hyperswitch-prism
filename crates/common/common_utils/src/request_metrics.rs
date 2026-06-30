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
}

impl ConnectorLatencyTracker {
    pub fn add_connector_time(&self, elapsed: Duration) {
        let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.nanos.fetch_add(nanos, Ordering::Relaxed);
    }

    pub fn connector_time(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::Relaxed))
    }
}
