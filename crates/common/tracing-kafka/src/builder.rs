//! Builder pattern implementation for KafkaWriter

use std::time::Duration;

use super::writer::{KafkaWriter, KafkaWriterError};

/// Builder for creating a KafkaWriter with custom configuration
#[derive(Debug, Clone, Default)]
pub struct KafkaWriterBuilder {
    brokers: Option<Vec<String>>,
    topic: Option<String>,
    batch_size: Option<usize>,
    linger_ms: Option<u64>,
    queue_buffering_max_messages: Option<usize>,
    queue_buffering_max_kbytes: Option<usize>,
    reconnect_backoff_min_ms: Option<u64>,
    reconnect_backoff_max_ms: Option<u64>,
}

impl KafkaWriterBuilder {
    /// Creates a new builder with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Kafka brokers to connect to
    pub fn brokers(mut self, brokers: Vec<String>) -> Self {
        self.brokers = Some(brokers);
        self
    }

    /// Sets the Kafka topic to send logs to
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Sets the batch size for buffering messages before sending
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = Some(size);
        self
    }

    /// Sets the linger time in milliseconds
    pub fn linger_ms(mut self, ms: u64) -> Self {
        self.linger_ms = Some(ms);
        self
    }

    /// Sets the linger time as a Duration
    pub fn linger(mut self, duration: Duration) -> Self {
        self.linger_ms = duration.as_millis().try_into().ok();
        self
    }

    /// Sets the maximum number of messages to buffer in the producer's queue
    pub fn queue_buffering_max_messages(mut self, size: usize) -> Self {
        self.queue_buffering_max_messages = Some(size);
        self
    }

    /// Sets the maximum size of the producer's queue in kilobytes
    pub fn queue_buffering_max_kbytes(mut self, size: usize) -> Self {
        self.queue_buffering_max_kbytes = Some(size);
        self
    }

    /// Sets the reconnect backoff times
    pub fn reconnect_backoff(mut self, min: Duration, max: Duration) -> Self {
        self.reconnect_backoff_min_ms = min.as_millis().try_into().ok();
        self.reconnect_backoff_max_ms = max.as_millis().try_into().ok();
        self
    }

    /// Builds the KafkaWriter with the configured settings
    pub fn build(self) -> Result<KafkaWriter, KafkaWriterError> {
        let brokers = self.brokers.ok_or_else(|| {
            KafkaWriterError::ProducerCreation(rdkafka::error::KafkaError::ClientCreation(
                "No brokers specified. Use .brokers()".to_string(),
            ))
        })?;

        let topic = self.topic.ok_or_else(|| {
            KafkaWriterError::ProducerCreation(rdkafka::error::KafkaError::ClientCreation(
                "No topic specified. Use .topic()".to_string(),
            ))
        })?;

        KafkaWriter::new(
            brokers,
            topic,
            self.batch_size,
            self.linger_ms,
            self.queue_buffering_max_messages,
            self.queue_buffering_max_kbytes,
            self.reconnect_backoff_min_ms,
            self.reconnect_backoff_max_ms,
        )
    }
}
