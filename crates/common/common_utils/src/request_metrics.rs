use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

/// Marks granular requests invoked by composite orchestration. The composite boundary owns
/// incoming audit transformations; nested services retain their diagnostic logs.
#[derive(Clone, Copy, Debug)]
pub struct InternalCompositeCall;

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
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.nanos.fetch_add(nanos, Ordering::Relaxed);
    }

    pub fn connector_time(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::Relaxed))
    }
}
