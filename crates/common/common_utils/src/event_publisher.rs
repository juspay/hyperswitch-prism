use std::sync::Arc;

use once_cell::sync::OnceCell;
use rdkafka::message::{Header, OwnedHeaders};
use serde_json;
use tracing_kafka::{builder::KafkaWriterBuilder, KafkaWriter};

use crate::{
    events::{
        extract_from_request, process_event_with_config, set_nested_value, Event, EventConfig,
    },
    CustomResult, EventPublisherError,
};

const PARTITION_KEY_METADATA: &str = "partitionKey";

/// Global static EventPublisher instance.
/// `None` means initialization was attempted but failed (Kafka unavailable).
/// `Some(p)` means the publisher is healthy and ready.
static EVENT_PUBLISHER: OnceCell<Option<EventPublisher>> = OnceCell::new();

/// An event publisher that sends events directly to Kafka.
#[derive(Clone)]
pub struct EventPublisher {
    writer: Arc<KafkaWriter>,
}

impl EventPublisher {
    /// Creates a new EventPublisher, initializing the KafkaWriter.
    pub fn new(config: &EventConfig) -> CustomResult<Self, EventPublisherError> {
        // Validate configuration before attempting to create writer
        if config.brokers.is_empty() {
            return Err(error_stack::Report::new(
                EventPublisherError::InvalidConfiguration {
                    message: "brokers list cannot be empty".to_string(),
                },
            ));
        }

        if config.topic.is_empty() {
            return Err(error_stack::Report::new(
                EventPublisherError::InvalidConfiguration {
                    message: "topic cannot be empty".to_string(),
                },
            ));
        }

        tracing::debug!(
          brokers = ?config.brokers,
          topic = %config.topic,
          "Creating EventPublisher with configuration"
        );

        let writer = KafkaWriterBuilder::new()
            .brokers(config.brokers.clone())
            .topic(config.topic.clone())
            .build()
            .map_err(|e| {
                error_stack::Report::new(EventPublisherError::KafkaWriterInitializationFailed)
                    .attach_printable(format!("KafkaWriter build failed: {e}"))
                    .attach_printable(format!(
                        "Brokers: {:?}, Topic: {}",
                        config.brokers, config.topic
                    ))
            })?;

        tracing::info!("EventPublisher created successfully");

        Ok(Self {
            writer: Arc::new(writer),
        })
    }

    /// Publishes a single event to Kafka with custom metadata.
    pub fn publish_event_with_metadata(
        &self,
        event: serde_json::Value,
        topic: &str,
        partition_key_field: &str,
        metadata: OwnedHeaders,
    ) -> CustomResult<(), EventPublisherError> {
        tracing::debug!(
            topic = %topic,
            partition_key_field = %partition_key_field,
            "Starting event publication to Kafka"
        );

        let mut headers = metadata;

        let key = if let Some(partition_key_value) =
            event.get(partition_key_field).and_then(|v| v.as_str())
        {
            headers = headers.insert(Header {
                key: PARTITION_KEY_METADATA,
                value: Some(partition_key_value.as_bytes()),
            });
            Some(partition_key_value)
        } else {
            tracing::warn!(
                partition_key_field = %partition_key_field,
                "Partition key field not found in event, message will be published without key"
            );
            None
        };

        let event_bytes = serde_json::to_vec(&event).map_err(|e| {
            error_stack::Report::new(EventPublisherError::EventSerializationFailed)
                .attach_printable(format!("Failed to serialize Event to JSON bytes: {e}"))
        })?;

        self.writer
            .publish_event(topic, key, &event_bytes, Some(headers))
            .map_err(|e| {
                let event_json = serde_json::to_string(&event).unwrap_or_default();
                error_stack::Report::new(EventPublisherError::EventPublishFailed)
                    .attach_printable(format!("Kafka publish failed: {e}"))
                    .attach_printable(format!(
                        "Topic: {}, Event size: {} bytes",
                        topic,
                        event_bytes.len()
                    ))
                    .attach_printable(format!("Failed event: {event_json}"))
            })?;

        Ok(())
    }

    fn build_kafka_metadata(&self, event: &Event) -> OwnedHeaders {
        let mut headers = OwnedHeaders::new();

        // Add lineage headers from Event.lineage_ids
        for (key, value) in event.lineage_ids.inner() {
            headers = headers.insert(Header {
                key: &key,
                value: Some(value.as_bytes()),
            });
        }

        let ref_id_option = event
            .additional_fields
            .get("reference_id")
            .and_then(|ref_id_value| ref_id_value.inner().as_str());
        let resource_id_option = event
            .additional_fields
            .get("resource_id")
            .and_then(|resource_id_value| resource_id_value.inner().as_str());

        // Add reference_id from Event.additional_fields
        if let Some(ref_id_str) = ref_id_option {
            headers = headers.insert(Header {
                key: "reference_id",
                value: Some(ref_id_str.as_bytes()),
            });
        }
        // Add resource_id from Event.additional_fields
        if let Some(resource_id_str) = resource_id_option {
            headers = headers.insert(Header {
                key: "resource_id",
                value: Some(resource_id_str.as_bytes()),
            });
        }

        headers
    }
}

/// Initialize the global EventPublisher with the given configuration.
/// If Kafka is unreachable, stores `None` and logs a warning instead of failing.
/// Subsequent emits will be silently dropped until the process is restarted with Kafka available.
pub fn init_event_publisher(config: &EventConfig) {
    tracing::info!(
        enabled = config.enabled,
        "Initializing global EventPublisher"
    );

    let value = match EventPublisher::new(config) {
        Ok(publisher) => {
            tracing::info!("Global EventPublisher initialized successfully");
            Some(publisher)
        }
        Err(e) => {
            tracing::warn!(
                error = ?e,
                brokers = ?config.brokers,
                topic = %config.topic,
                "Failed to initialize EventPublisher (Kafka may be unavailable); \
                 events will be dropped until the service is restarted with Kafka reachable"
            );
            None
        }
    };

    // Ignore AlreadyInitialized — can happen in tests; first writer wins.
    let _ = EVENT_PUBLISHER.set(value);
}

/// Returns the global EventPublisher if it was successfully initialized, otherwise `None`.
fn get_event_publisher() -> Option<&'static EventPublisher> {
    EVENT_PUBLISHER.get().and_then(|opt| opt.as_ref())
}

/// Publish a processed event to Kafka if enabled. Called from emit_event_with_config.
pub fn publish_event_to_kafka(
    event: &Event,
    processed_event: serde_json::Value,
    config: &EventConfig,
) {
    if config.enabled {
        if let Some(publisher) = get_event_publisher() {
            let metadata = publisher.build_kafka_metadata(event);
            let _ = publisher
                .publish_event_with_metadata(
                    processed_event,
                    &config.topic,
                    &config.partition_key_field,
                    metadata,
                )
                .inspect_err(|e| {
                    tracing::error!(error = ?e, "Failed to publish event to Kafka");
                });
        } else {
            tracing::warn!("EventPublisher not available; audit event dropped");
        }
    }
}
