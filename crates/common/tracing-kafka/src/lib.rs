//! A Kafka tracing layer that integrates with the tracing ecosystem.
//!
//! This crate provides a simple way to send tracing logs to Kafka while maintaining
//! consistent JSON formatting through the log_utils infrastructure.
//!
//! # Examples
//! ```no_run
//! use tracing_kafka::KafkaLayer;
//! use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
//!
//! let kafka_layer = KafkaLayer::builder()
//!     .brokers(&["localhost:9092"])
//!     .topic("application-logs")
//!     .build()
//!     .expect("Failed to create Kafka layer");
//!
//! tracing_subscriber::registry()
//!     .with(kafka_layer)
//!     .init();
//! ```
//!
//! # Publishing Custom Events
//!
//! In addition to logging, the `KafkaWriter` can be used to publish custom events to Kafka.
//! The `publish_event` method allows you to send a payload to a specific topic with an optional key and headers.
//!
//! ```no_run
//! use tracing_kafka::KafkaWriter;
//! use rdkafka::message::OwnedHeaders;
//!
//! let writer = KafkaWriter::new(
//!     vec!["localhost:9092".to_string()],
//!     "default-topic".to_string(),
//!     None, None, None, None, None, None
//! ).expect("Failed to create KafkaWriter");
//!
//! let headers = OwnedHeaders::new().add("my-header", "my-value");
//!
//! let result = writer.publish_event(
//!     "custom-events",
//!     Some("event-key"),
//!     b"event-payload",
//!     Some(headers),
//! );
//!
//! if let Err(e) = result {
//!     eprintln!("Failed to publish event: {}", e);
//! }
//! ```

pub mod builder;
mod layer;
mod writer;

pub use layer::{KafkaLayer, KafkaLayerError};
pub use writer::{KafkaWriter, KafkaWriterError};

#[cfg(feature = "kafka-metrics")]
mod metrics;

/// Initializes the metrics for the tracing kafka.
/// This function should be called once at application startup.
#[cfg(feature = "kafka-metrics")]
pub fn init() {
    metrics::initialize_all_metrics();
}

#[cfg(not(feature = "kafka-metrics"))]
pub fn init() {
    tracing::warn!("Kafka metrics feature is not enabled. Metrics will not be collected.");
}
