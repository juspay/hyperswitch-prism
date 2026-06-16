pub mod config;

pub mod setup;
pub use setup::setup;

#[cfg(feature = "otel")]
pub mod metrics;

pub mod env;

pub use tracing::{debug, error, event as log, info, warn};
pub use tracing_attributes::instrument;
