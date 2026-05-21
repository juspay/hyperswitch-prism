use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::types::{PaymentIntent, Refund};

#[derive(Clone)]
pub struct AppState {
    pub require_token: Option<String>,
    pub payment_intents: Arc<DashMap<String, (PaymentIntent, Instant)>>,
    pub refunds: Arc<DashMap<String, (Refund, Instant)>>,
    pub attempts: Arc<DashMap<String, AttemptOutcome>>,
}

#[derive(Clone, Debug)]
pub struct AttemptOutcome {
    pub payment_intent_id: String,
    pub on_complete_status: String,
    pub error_message: Option<String>,
}

impl AppState {
    pub fn new(require_token: Option<String>) -> Self {
        Self {
            require_token,
            payment_intents: Arc::new(DashMap::new()),
            refunds: Arc::new(DashMap::new()),
            attempts: Arc::new(DashMap::new()),
        }
    }

    pub fn spawn_ttl_sweeper(&self) {
        let pis = self.payment_intents.clone();
        let rfs = self.refunds.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                let cutoff = Instant::now() - Duration::from_secs(3600);
                pis.retain(|_, (_, ts)| *ts > cutoff);
                rfs.retain(|_, (_, ts)| *ts > cutoff);
            }
        });
    }
}
