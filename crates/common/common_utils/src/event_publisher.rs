use std::sync::Arc;

use hyperswitch_masking::ErasedMaskSerialize;
use once_cell::sync::OnceCell;
use rdkafka::message::{Header, OwnedHeaders};
use serde_json;
use tracing_kafka::{builder::KafkaWriterBuilder, KafkaWriter};

use crate::{
    events::{Event, EventConfig},
    CustomResult, EventPublisherError,
};

const PARTITION_KEY_METADATA: &str = "partitionKey";

/// Global static EventPublisher instance
static EVENT_PUBLISHER: OnceCell<EventPublisher> = OnceCell::new();

/// An event publisher that sends events directly to Kafka.
#[derive(Clone)]
pub struct EventPublisher {
    writer: Arc<KafkaWriter>,
    config: EventConfig,
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
            config: config.clone(),
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

/// Process an event by applying masking, transformations, static values, and extractions.
/// This function does not require Kafka initialization and can be used for logging purposes.
fn process_event_with_config(
    event: &Event,
    config: &EventConfig,
) -> CustomResult<serde_json::Value, EventPublisherError> {
    let mut result = event.masked_serialize().map_err(|e| {
        error_stack::Report::new(EventPublisherError::EventSerializationFailed)
            .attach_printable(format!("Event masked serialization failed: {e}"))
    })?;

    // Helper function to normalize paths (replace _DOT_ and _dot_ with .)
    let normalize_path =
        |path: &str| -> String { path.replace("_DOT_", ".").replace("_dot_", ".") };

    // Process transformations
    for (target_path, source_field) in &config.transformations {
        if let Some(value) = result.get(source_field).cloned() {
            let normalized_path = normalize_path(target_path);
            if let Err(e) = set_nested_value(&mut result, &normalized_path, value) {
                tracing::warn!(
                    target_path = %target_path,
                    normalized_path = %normalized_path,
                    source_field = %source_field,
                    error = %e,
                    "Failed to set transformation, continuing with event processing"
                );
            }
        }
    }

    // Process static values - log warnings but continue processing
    for (target_path, static_value) in &config.static_values {
        let normalized_path = normalize_path(target_path);
        let value = serde_json::json!(static_value);
        if let Err(e) = set_nested_value(&mut result, &normalized_path, value) {
            tracing::warn!(
                target_path = %target_path,
                normalized_path = %normalized_path,
                static_value = %static_value,
                error = %e,
                "Failed to set static value, continuing with event processing"
            );
        }
    }

    // Process extraction
    for (target_path, extraction_path) in &config.extractions {
        if let Some(value) = extract_from_request(&result, extraction_path) {
            let normalized_path = normalize_path(target_path);
            if let Err(e) = set_nested_value(&mut result, &normalized_path, value) {
                tracing::warn!(
                    target_path = %target_path,
                    normalized_path = %normalized_path,
                    extraction_path = %extraction_path,
                    error = %e,
                    "Failed to set extraction, continuing with event processing"
                );
            }
        }
    }

    Ok(result)
}

fn extract_from_request(
    event_value: &serde_json::Value,
    extraction_path: &str,
) -> Option<serde_json::Value> {
    let mut path_parts = extraction_path.split('.');

    let first_part = path_parts.next()?;

    let source = match first_part {
        "req" => event_value.get("request_data")?.clone(),
        _ => return None,
    };

    let mut current = &source;
    for part in path_parts {
        current = current.get(part)?;
    }

    Some(current.clone())
}

fn set_nested_value(
    target: &mut serde_json::Value,
    path: &str,
    value: serde_json::Value,
) -> CustomResult<(), EventPublisherError> {
    let path_parts: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();

    if path_parts.is_empty() {
        return Err(error_stack::Report::new(EventPublisherError::InvalidPath {
            path: path.to_string(),
        }));
    }

    if path_parts.len() == 1 {
        let key = path_parts.first().ok_or_else(|| {
            error_stack::Report::new(EventPublisherError::InvalidPath {
                path: path.to_string(),
            })
        })?;
        target[key] = value;
        return Ok(());
    }

    let result = path_parts.iter().enumerate().try_fold(
        target,
        |current, (index, &part)| -> CustomResult<&mut serde_json::Value, EventPublisherError> {
            if index == path_parts.len() - 1 {
                current[part] = value.clone();
                Ok(current)
            } else {
                if !current[part].is_object() {
                    current[part] = serde_json::json!({});
                }
                current.get_mut(part).ok_or_else(|| {
                    error_stack::Report::new(EventPublisherError::InvalidPath {
                        path: format!("{path}.{part}"),
                    })
                })
            }
        },
    );

    result.map(|_| ())
}

/// Initialize the global EventPublisher with the given configuration
pub fn init_event_publisher(config: &EventConfig) -> CustomResult<(), EventPublisherError> {
    tracing::info!(
        enabled = config.enabled,
        "Initializing global EventPublisher"
    );

    let publisher = EventPublisher::new(config)?;

    EVENT_PUBLISHER.set(publisher).map_err(|failed_publisher| {
        error_stack::Report::new(EventPublisherError::AlreadyInitialized)
            .attach_printable("EventPublisher was already initialized")
            .attach_printable(format!(
                "Existing config: brokers={:?}, topic={}",
                failed_publisher.config.brokers, failed_publisher.config.topic
            ))
            .attach_printable(format!(
                "New config: brokers={:?}, topic={}",
                config.brokers, config.topic
            ))
    })?;

    tracing::info!("Global EventPublisher initialized successfully");
    Ok(())
}

/// Get or initialize the global EventPublisher
fn get_event_publisher(
    config: &EventConfig,
) -> CustomResult<&'static EventPublisher, EventPublisherError> {
    EVENT_PUBLISHER.get_or_try_init(|| EventPublisher::new(config))
}

/// Standalone function to emit events using the global EventPublisher.
/// This function always processes and logs events, but only publishes to Kafka when enabled.
pub fn emit_event_with_config(event: Event, config: &EventConfig) {
    // Always process the event to get masked/parsed data for logging
    let processed_event = match process_event_with_config(&event, config) {
        Ok(processed) => processed,
        Err(e) => {
            tracing::error!(
                error = ?e,
                "Failed to process event"
            );
            return;
        }
    };

    // This provides observability even when Kafka publishing is disabled
    let event_json = serde_json::to_string(&processed_event)
        .unwrap_or_else(|e| format!("{{\"error\":\"Failed to serialize event: {}\"}}", e));
    tracing::info!(
        events_enabled = config.enabled,
        "Event processed (Kafka publishing: {}) - Event JSON: {}",
        if config.enabled {
            "enabled"
        } else {
            "disabled"
        },
        event_json
    );

    // Only publish to Kafka if enabled
    if config.enabled {
        let _ = get_event_publisher(config)
            .and_then(|publisher| {
                let metadata = publisher.build_kafka_metadata(&event);
                publisher.publish_event_with_metadata(
                    processed_event,
                    &config.topic,
                    &config.partition_key_field,
                    metadata,
                )
            })
            .inspect_err(|e| {
                tracing::error!(error = ?e, "Failed to publish event to Kafka");
            });
    }
}
