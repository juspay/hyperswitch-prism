use serde::{Deserialize, Serialize};

/// Producer configuration for Kafka connector requests
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, config_patch_derive::Patch)]
pub struct ConnectorRequestKafkaConfig {
    pub enabled: bool,
    /// Comma-separated list of `host:port` broker addresses.
    #[serde(default)]
    pub brokers: Vec<String>,
    /// Producer acknowledgement mode passed to `acks` rdkafka config key.
    /// `"all"` (default) requires all in-sync replicas to acknowledge — safest.
    #[serde(default)]
    pub acks: String,
    /// How long (ms) the producer waits for broker acknowledgement before
    /// timing out the request. Maps to `request.timeout.ms`.
    #[serde(default)]
    pub request_timeout_ms: u64,
    /// Per-message delivery timeout (ms). Maps to `message.timeout.ms`.
    /// Should be >= `request_timeout_ms`.
    #[serde(default)]
    pub message_timeout_ms: u64,
    /// Security protocol. Maps to `security.protocol`.
    /// Common values: `"PLAINTEXT"`, `"SSL"`, `"SASL_PLAINTEXT"`, `"SASL_SSL"`.
    #[serde(default)]
    pub security_protocol: Option<String>,
    /// SASL mechanism. Maps to `sasl.mechanisms`.
    /// Common values: `"PLAIN"`, `"SCRAM-SHA-256"`, `"SCRAM-SHA-512"`.
    #[serde(default)]
    pub sasl_mechanism: Option<String>,
    /// SASL username. Maps to `sasl.username`.
    #[serde(default)]
    pub sasl_username: Option<String>,
    /// SASL password. Maps to `sasl.password`.
    #[serde(default)]
    pub sasl_password: Option<String>,
    /// Timeout (ms) for enqueuing a message into the producer's internal queue.
    /// This is the `queue_timeout` passed to `FutureProducer::send`.
    /// If the queue is full for this long, the send fails.
    #[serde(default)]
    pub enqueue_timeout_ms: u64,
}

impl Default for ConnectorRequestKafkaConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            brokers: vec!["localhost:9092".to_string()],
            acks: "all".to_string(),
            request_timeout_ms: 30_000,
            message_timeout_ms: 30_000,
            security_protocol: None,
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
            enqueue_timeout_ms: 5_000,
        }
    }
}
