//! Kafka layer implementation that reuses log_utils formatting.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use log_utils::{
    AdditionalFieldsPlacement, JsonFormattingLayer, JsonFormattingLayerConfig, LoggerError,
};
use tracing::Subscriber;
use tracing_subscriber::Layer;

use crate::{
    builder::KafkaWriterBuilder,
    writer::{KafkaWriter, KafkaWriterError},
};

/// Tracing layer that sends JSON-formatted logs to Kafka
///
/// Wraps log_utils' JsonFormattingLayer
pub struct KafkaLayer {
    inner: JsonFormattingLayer<KafkaWriter, serde_json::ser::CompactFormatter>,
}

impl KafkaLayer {
    /// Creates a new builder for configuring a KafkaLayer.
    pub fn builder() -> KafkaLayerBuilder {
        KafkaLayerBuilder::new()
    }

    /// Creates a new KafkaLayer from a pre-configured KafkaWriter.
    /// This is primarily used internally by the builder.
    pub(crate) fn from_writer(
        kafka_writer: KafkaWriter,
        static_fields: HashMap<String, serde_json::Value>,
    ) -> Result<Self, KafkaLayerError> {
        let config = JsonFormattingLayerConfig {
            static_top_level_fields: static_fields,
            top_level_keys: HashSet::new(),
            log_span_lifecycles: true,
            additional_fields_placement: AdditionalFieldsPlacement::TopLevel,
        };

        let inner: JsonFormattingLayer<KafkaWriter, serde_json::ser::CompactFormatter> =
            JsonFormattingLayer::new(config, kafka_writer, serde_json::ser::CompactFormatter)?;

        Ok(Self { inner })
    }
}

impl<S> Layer<S> for KafkaLayer
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_event(event, ctx);
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_new_span(attrs, id, ctx);
    }

    fn on_enter(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_enter(id, ctx);
    }

    fn on_exit(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_exit(id, ctx);
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_close(id, ctx);
    }
}

impl KafkaLayer {
    /// Boxes the layer, making it easier to compose with other layers.
    pub fn boxed<S>(self) -> Box<dyn Layer<S> + Send + Sync + 'static>
    where
        Self: Layer<S> + Sized + Send + Sync + 'static,
        S: Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
    {
        Box::new(self)
    }
}

/// Errors that can occur when creating a KafkaLayer.
#[derive(Debug, thiserror::Error)]
pub enum KafkaLayerError {
    #[error("Kafka writer error: {0}")]
    Writer(#[from] KafkaWriterError),

    #[error("Logger configuration error: {0}")]
    Logger(#[from] LoggerError),

    #[error("Missing brokers configuration")]
    MissingBrokers,

    #[error("Missing topic configuration")]
    MissingTopic,
}

/// Builder for creating a KafkaLayer with custom configuration.
#[derive(Debug, Clone, Default)]
pub struct KafkaLayerBuilder {
    writer_builder: KafkaWriterBuilder,
    static_fields: HashMap<String, serde_json::Value>,
}

impl KafkaLayerBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Kafka brokers to connect to.
    pub fn brokers(mut self, brokers: &[&str]) -> Self {
        self.writer_builder = self
            .writer_builder
            .brokers(brokers.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Sets the Kafka topic to send logs to.
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.writer_builder = self.writer_builder.topic(topic);
        self
    }

    /// Sets the batch size for buffering messages before sending.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.writer_builder = self.writer_builder.batch_size(size);
        self
    }

    /// Sets the linger time in milliseconds.
    pub fn linger_ms(mut self, ms: u64) -> Self {
        self.writer_builder = self.writer_builder.linger_ms(ms);
        self
    }

    /// Sets the linger time as a Duration.
    pub fn linger(mut self, duration: Duration) -> Self {
        self.writer_builder = self.writer_builder.linger(duration);
        self
    }

    /// Sets the maximum number of messages to buffer in the producer's queue.
    pub fn queue_buffering_max_messages(mut self, size: usize) -> Self {
        self.writer_builder = self.writer_builder.queue_buffering_max_messages(size);
        self
    }

    /// Sets the maximum size of the producer's queue in kilobytes.
    pub fn queue_buffering_max_kbytes(mut self, size: usize) -> Self {
        self.writer_builder = self.writer_builder.queue_buffering_max_kbytes(size);
        self
    }

    /// Sets the reconnect backoff times.
    pub fn reconnect_backoff(mut self, min: Duration, max: Duration) -> Self {
        self.writer_builder = self.writer_builder.reconnect_backoff(min, max);
        self
    }

    /// Adds static fields that will be included in every log entry.
    /// These fields are added at the top level of the JSON output.
    pub fn static_fields(mut self, fields: HashMap<String, serde_json::Value>) -> Self {
        self.static_fields = fields;
        self
    }

    /// Adds a single static field that will be included in every log entry.
    pub fn add_static_field(mut self, key: String, value: serde_json::Value) -> Self {
        self.static_fields.insert(key, value);
        self
    }

    /// Builds the KafkaLayer with the configured settings.
    pub fn build(self) -> Result<KafkaLayer, KafkaLayerError> {
        let kafka_writer = self.writer_builder.build()?;
        KafkaLayer::from_writer(kafka_writer, self.static_fields)
    }
}
