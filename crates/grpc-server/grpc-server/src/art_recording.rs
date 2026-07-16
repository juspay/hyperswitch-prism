use std::fmt::Display;

use art_recorder::schema::CsvRecording;
use thiserror::Error;
use ucs_env::configs::ArtRecordingConfig;

pub trait ArtRecordingSink {
    type Error: Display;

    fn publish(&self, topic: &str, key: &str, payload: &[u8]) -> Result<(), Self::Error>;
}

pub fn publish_rows_to_sink<S>(
    rows: &[CsvRecording],
    topic: &str,
    sink: &S,
) -> Result<usize, ArtRecordingPublishError>
where
    S: ArtRecordingSink,
{
    for row in rows {
        let payload = serde_json::to_vec(row)
            .map_err(|source| ArtRecordingPublishError::SerializeRow { source })?;

        sink.publish(topic, &row.sess_id, &payload)
            .map_err(|error| ArtRecordingPublishError::Publish {
                message: error.to_string(),
            })?;
    }

    Ok(rows.len())
}

pub fn init_art_recording_publisher(config: &ArtRecordingConfig) {
    imp::init_art_recording_publisher(config);
}

pub fn publish_art_recording_rows(rows: &[CsvRecording], config: &ArtRecordingConfig) {
    imp::publish_art_recording_rows(rows, config);
}

#[derive(Debug, Error)]
pub enum ArtRecordingPublishError {
    #[error("failed to serialize ART recording row")]
    SerializeRow { source: serde_json::Error },
    #[error("failed to publish ART recording row: {message}")]
    Publish { message: String },
}

#[cfg(feature = "art-recording-kafka")]
mod imp {
    use std::sync::Arc;

    use once_cell::sync::OnceCell;
    use tracing_kafka::{builder::KafkaWriterBuilder, KafkaWriter};

    use super::{publish_rows_to_sink, ArtRecordingSink};
    use art_recorder::schema::CsvRecording;
    use ucs_env::configs::ArtRecordingConfig;

    static ART_RECORDING_PUBLISHER: OnceCell<Option<ArtRecordingPublisher>> = OnceCell::new();

    #[derive(Clone)]
    struct KafkaArtRecordingSink {
        writer: Arc<KafkaWriter>,
    }

    impl KafkaArtRecordingSink {
        fn new(config: &ArtRecordingConfig) -> Result<Self, String> {
            if config.kafka_brokers.is_empty() {
                return Err("art_recording.kafka_brokers cannot be empty".to_string());
            }

            if config.kafka_topic.is_empty() {
                return Err("art_recording.kafka_topic cannot be empty".to_string());
            }

            let writer = KafkaWriterBuilder::new()
                .brokers(config.kafka_brokers.clone())
                .topic(config.kafka_topic.clone())
                .custom_config(config.kafka_properties.clone())
                .build()
                .map_err(|error| error.to_string())?;

            Ok(Self {
                writer: Arc::new(writer),
            })
        }
    }

    impl ArtRecordingSink for KafkaArtRecordingSink {
        type Error = String;

        fn publish(&self, topic: &str, key: &str, payload: &[u8]) -> Result<(), Self::Error> {
            self.writer
                .publish_event(topic, Some(key), payload, None)
                .map_err(|error| error.to_string())
        }
    }

    #[derive(Clone)]
    struct ArtRecordingPublisher {
        topic: String,
        sink: KafkaArtRecordingSink,
    }

    pub fn init_art_recording_publisher(config: &ArtRecordingConfig) {
        if !config.enabled {
            tracing::info!("ART recording publisher disabled in configuration");
            return;
        }

        let value = match KafkaArtRecordingSink::new(config) {
            Ok(sink) => {
                tracing::info!(
                    topic = %config.kafka_topic,
                    brokers = ?config.kafka_brokers,
                    "ART recording publisher initialized successfully"
                );
                Some(ArtRecordingPublisher {
                    topic: config.kafka_topic.clone(),
                    sink,
                })
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    brokers = ?config.kafka_brokers,
                    topic = %config.kafka_topic,
                    "Failed to initialize ART recording publisher; recordings will be dropped"
                );
                None
            }
        };

        let _ = ART_RECORDING_PUBLISHER.set(value);
    }

    pub fn publish_art_recording_rows(rows: &[CsvRecording], config: &ArtRecordingConfig) {
        if !config.enabled || rows.is_empty() {
            return;
        }

        if let Some(publisher) = ART_RECORDING_PUBLISHER
            .get()
            .and_then(|value| value.as_ref())
        {
            if let Err(error) = publish_rows_to_sink(rows, &publisher.topic, &publisher.sink) {
                tracing::error!(error = ?error, "Failed to publish ART recording rows");
            }
        } else {
            tracing::warn!("ART recording publisher not available; recording rows dropped");
        }
    }
}

#[cfg(not(feature = "art-recording-kafka"))]
mod imp {
    use art_recorder::schema::CsvRecording;
    use ucs_env::configs::ArtRecordingConfig;

    pub fn init_art_recording_publisher(config: &ArtRecordingConfig) {
        if config.enabled {
            tracing::warn!(
                "ART recording is enabled but grpc-server was built without the art-recording-kafka feature; recordings will be dropped"
            );
        }
    }

    pub fn publish_art_recording_rows(rows: &[CsvRecording], config: &ArtRecordingConfig) {
        if config.enabled && !rows.is_empty() {
            tracing::warn!(
                row_count = rows.len(),
                "ART recording rows dropped because grpc-server was built without the art-recording-kafka feature"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use art_recorder::schema::CsvRecording;

    use super::{publish_rows_to_sink, ArtRecordingSink};

    #[derive(Default)]
    struct RecordingSink {
        messages: RefCell<Vec<PublishedMessage>>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct PublishedMessage {
        topic: String,
        key: String,
        payload: Vec<u8>,
    }

    impl ArtRecordingSink for RecordingSink {
        type Error = String;

        fn publish(&self, topic: &str, key: &str, payload: &[u8]) -> Result<(), Self::Error> {
            self.messages.borrow_mut().push(PublishedMessage {
                topic: topic.to_string(),
                key: key.to_string(),
                payload: payload.to_vec(),
            });
            Ok(())
        }
    }

    #[test]
    fn publish_rows_to_sink_serializes_each_row_and_keys_by_session_id() {
        let sink = RecordingSink::default();
        let rows = vec![CsvRecording {
            sess_id: "req_123".to_string(),
            merch_id: "merchant_123".to_string(),
            ord_id: "order_123".to_string(),
            counter: 1,
            val_type: "UUIDEntryT".to_string(),
            rec_entry: "{\"tag\":\"UUIDEntryT\"}".to_string(),
        }];

        let published =
            publish_rows_to_sink(&rows, "art-recordings", &sink).expect("rows should publish");

        assert_eq!(published, 1);
        let messages = sink.messages.borrow();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].topic, "art-recordings");
        assert_eq!(messages[0].key, "req_123");

        let payload = serde_json::from_slice::<serde_json::Value>(&messages[0].payload)
            .expect("payload should be JSON");
        assert_eq!(
            payload,
            serde_json::json!({
                "sessId": "req_123",
                "merchId": "merchant_123",
                "ordId": "order_123",
                "counter": 1,
                "valType": "UUIDEntryT",
                "recEntry": "{\"tag\":\"UUIDEntryT\"}"
            })
        );
    }

    #[test]
    fn publish_rows_to_sink_stops_on_first_publish_error() {
        struct FailingSink;

        impl ArtRecordingSink for FailingSink {
            type Error = &'static str;

            fn publish(
                &self,
                _topic: &str,
                _key: &str,
                _payload: &[u8],
            ) -> Result<(), Self::Error> {
                Err("queue full")
            }
        }

        let rows = vec![CsvRecording {
            sess_id: "req_123".to_string(),
            merch_id: "merchant_123".to_string(),
            ord_id: "order_123".to_string(),
            counter: 1,
            val_type: "UUIDEntryT".to_string(),
            rec_entry: "{}".to_string(),
        }];

        let error = publish_rows_to_sink(&rows, "art-recordings", &FailingSink)
            .expect_err("publish error should surface");

        assert!(
            error.to_string().contains("queue full"),
            "unexpected error: {error}"
        );
    }
}
