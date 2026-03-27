//! Kafka writer implementation for sending formatted log messages to Kafka.

use std::{
    io::{self, Write},
    sync::Arc,
    time::Duration,
};

use rdkafka::{
    config::ClientConfig,
    error::{KafkaError, RDKafkaErrorCode},
    message::OwnedHeaders,
    producer::{BaseRecord, DeliveryResult, Producer, ProducerContext, ThreadedProducer},
    ClientContext,
};

#[cfg(feature = "kafka-metrics")]
use super::metrics::{
    KAFKA_AUDIT_DROPS_MSG_TOO_LARGE, KAFKA_AUDIT_DROPS_OTHER, KAFKA_AUDIT_DROPS_QUEUE_FULL,
    KAFKA_AUDIT_DROPS_TIMEOUT, KAFKA_AUDIT_EVENTS_DROPPED, KAFKA_AUDIT_EVENTS_SENT,
    KAFKA_AUDIT_EVENT_QUEUE_SIZE, KAFKA_DROPS_MSG_TOO_LARGE, KAFKA_DROPS_OTHER,
    KAFKA_DROPS_QUEUE_FULL, KAFKA_DROPS_TIMEOUT, KAFKA_LOGS_DROPPED, KAFKA_LOGS_SENT,
    KAFKA_QUEUE_SIZE,
};

/// A `ProducerContext` that handles delivery callbacks to increment metrics.
#[derive(Clone)]
struct MetricsProducerContext;

impl ClientContext for MetricsProducerContext {}

impl ProducerContext for MetricsProducerContext {
    type DeliveryOpaque = Box<KafkaMessageType>;

    fn delivery(&self, delivery_result: &DeliveryResult<'_>, opaque: Self::DeliveryOpaque) {
        let message_type = *opaque;
        let is_success = delivery_result.is_ok();

        #[cfg(feature = "kafka-metrics")]
        {
            match (message_type, is_success) {
                (KafkaMessageType::Event, true) => KAFKA_AUDIT_EVENTS_SENT.inc(),
                (KafkaMessageType::Event, false) => KAFKA_AUDIT_EVENTS_DROPPED.inc(),
                (KafkaMessageType::Log, true) => KAFKA_LOGS_SENT.inc(),
                (KafkaMessageType::Log, false) => KAFKA_LOGS_DROPPED.inc(),
            }
        }

        if let Err((kafka_error, _)) = delivery_result {
            #[cfg(feature = "kafka-metrics")]
            match (message_type, &kafka_error) {
                (
                    KafkaMessageType::Event,
                    KafkaError::MessageProduction(RDKafkaErrorCode::QueueFull),
                ) => {
                    KAFKA_AUDIT_DROPS_QUEUE_FULL.inc();
                }
                (
                    KafkaMessageType::Event,
                    KafkaError::MessageProduction(RDKafkaErrorCode::MessageSizeTooLarge),
                ) => {
                    KAFKA_AUDIT_DROPS_MSG_TOO_LARGE.inc();
                }
                (
                    KafkaMessageType::Event,
                    KafkaError::MessageProduction(RDKafkaErrorCode::MessageTimedOut),
                ) => {
                    KAFKA_AUDIT_DROPS_TIMEOUT.inc();
                }
                (KafkaMessageType::Event, _) => {
                    KAFKA_AUDIT_DROPS_OTHER.inc();
                }
                (
                    KafkaMessageType::Log,
                    KafkaError::MessageProduction(RDKafkaErrorCode::QueueFull),
                ) => {
                    KAFKA_DROPS_QUEUE_FULL.inc();
                }
                (
                    KafkaMessageType::Log,
                    KafkaError::MessageProduction(RDKafkaErrorCode::MessageSizeTooLarge),
                ) => {
                    KAFKA_DROPS_MSG_TOO_LARGE.inc();
                }
                (
                    KafkaMessageType::Log,
                    KafkaError::MessageProduction(RDKafkaErrorCode::MessageTimedOut),
                ) => {
                    KAFKA_DROPS_TIMEOUT.inc();
                }
                (KafkaMessageType::Log, _) => {
                    KAFKA_DROPS_OTHER.inc();
                }
            }
        }
    }
}

/// This enum helps the callback distinguish between logs and events.
#[derive(Clone, Copy, Debug)]
enum KafkaMessageType {
    Event,
    Log,
}

/// Kafka writer that implements std::io::Write for seamless integration with tracing
#[derive(Clone)]
pub struct KafkaWriter {
    producer: Arc<ThreadedProducer<MetricsProducerContext>>,
    topic: String,
}

impl std::fmt::Debug for KafkaWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KafkaWriter")
            .field("topic", &self.topic)
            .finish()
    }
}

impl KafkaWriter {
    /// Creates a new KafkaWriter with the specified brokers and topic.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        brokers: Vec<String>,
        topic: String,
        batch_size: Option<usize>,
        linger_ms: Option<u64>,
        queue_buffering_max_messages: Option<usize>,
        queue_buffering_max_kbytes: Option<usize>,
        reconnect_backoff_min_ms: Option<u64>,
        reconnect_backoff_max_ms: Option<u64>,
    ) -> Result<Self, KafkaWriterError> {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", brokers.join(","));

        if let Some(min_backoff) = reconnect_backoff_min_ms {
            config.set("reconnect.backoff.ms", min_backoff.to_string());
        }
        if let Some(max_backoff) = reconnect_backoff_max_ms {
            config.set("reconnect.backoff.max.ms", max_backoff.to_string());
        }
        if let Some(max_messages) = queue_buffering_max_messages {
            config.set("queue.buffering.max.messages", max_messages.to_string());
        }
        if let Some(max_kbytes) = queue_buffering_max_kbytes {
            config.set("queue.buffering.max.kbytes", max_kbytes.to_string());
        }
        if let Some(size) = batch_size {
            config.set("batch.size", size.to_string());
        }
        if let Some(ms) = linger_ms {
            config.set("linger.ms", ms.to_string());
        }

        let producer: ThreadedProducer<MetricsProducerContext> = config
            .create_with_context(MetricsProducerContext)
            .map_err(KafkaWriterError::ProducerCreation)?;

        producer
            .client()
            .fetch_metadata(Some(&topic), Duration::from_secs(5))
            .map_err(KafkaWriterError::MetadataFetch)?;

        Ok(Self {
            producer: Arc::new(producer),
            topic,
        })
    }

    /// Publishes a single event to Kafka. This method is non-blocking.
    /// Returns an error if the message cannot be enqueued to the producer's buffer.
    pub fn publish_event(
        &self,
        topic: &str,
        key: Option<&str>,
        payload: &[u8],
        headers: Option<OwnedHeaders>,
    ) -> Result<(), KafkaError> {
        #[cfg(feature = "kafka-metrics")]
        {
            let queue_size = self.producer.in_flight_count();
            KAFKA_AUDIT_EVENT_QUEUE_SIZE.set(queue_size.into());
        }

        let mut record = BaseRecord::with_opaque_to(topic, Box::new(KafkaMessageType::Event))
            .payload(payload)
            .timestamp(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis().try_into().unwrap_or(0))
                    .unwrap_or(0),
            );

        if let Some(k) = key {
            record = record.key(k);
        }

        if let Some(h) = headers {
            record = record.headers(h);
        }

        match self.producer.send(record) {
            Ok(_) => Ok(()),
            Err((kafka_error, _)) => {
                #[cfg(feature = "kafka-metrics")]
                {
                    KAFKA_AUDIT_EVENTS_DROPPED.inc();

                    // Only QUEUE_FULL can happen during send() - others happen during delivery
                    match &kafka_error {
                        KafkaError::MessageProduction(RDKafkaErrorCode::QueueFull) => {
                            KAFKA_AUDIT_DROPS_QUEUE_FULL.inc();
                        }
                        _ => {
                            KAFKA_AUDIT_DROPS_OTHER.inc();
                        }
                    }
                }
                Err(kafka_error)
            }
        }
    }

    /// Creates a new builder for constructing a KafkaWriter
    pub fn builder() -> crate::builder::KafkaWriterBuilder {
        crate::builder::KafkaWriterBuilder::new()
    }
}

impl Write for KafkaWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[cfg(feature = "kafka-metrics")]
        {
            let queue_size = self.producer.in_flight_count();
            KAFKA_QUEUE_SIZE.set(queue_size.into());
        }

        let record = BaseRecord::with_opaque_to(&self.topic, Box::new(KafkaMessageType::Log))
            .payload(buf)
            .timestamp(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis().try_into().unwrap_or(0))
                    .unwrap_or(0),
            );

        if let Err((kafka_error, _)) = self.producer.send::<(), [u8]>(record) {
            #[cfg(feature = "kafka-metrics")]
            {
                KAFKA_LOGS_DROPPED.inc();

                match &kafka_error {
                    KafkaError::MessageProduction(RDKafkaErrorCode::QueueFull) => {
                        KAFKA_DROPS_QUEUE_FULL.inc();
                    }
                    _ => {
                        KAFKA_DROPS_OTHER.inc();
                    }
                }
            }
        }

        // Return Ok to not block the application. The actual delivery result
        // is handled by the callback in the background.
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.producer
            .flush(rdkafka::util::Timeout::After(Duration::from_secs(5)))
            .map_err(|e: KafkaError| io::Error::other(format!("Kafka flush failed: {e}")))
    }
}

/// Errors that can occur when creating or using a KafkaWriter.
#[derive(Debug, thiserror::Error)]
pub enum KafkaWriterError {
    #[error("Failed to create Kafka producer: {0}")]
    ProducerCreation(KafkaError),
    #[error("Failed to fetch Kafka metadata: {0}")]
    MetadataFetch(KafkaError),
}

/// Make KafkaWriter compatible with tracing_appender's MakeWriter trait.
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for KafkaWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Graceful shutdown - flush pending messages when dropping
impl Drop for KafkaWriter {
    fn drop(&mut self) {
        // Only flush if this is the last reference to the producer
        if Arc::strong_count(&self.producer) == 1 {
            // Try to flush pending messages with a 5 second timeout
            let _ = self
                .producer
                .flush(rdkafka::util::Timeout::After(Duration::from_secs(5)));
        }
    }
}
