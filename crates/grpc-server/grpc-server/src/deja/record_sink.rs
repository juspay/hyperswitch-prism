//! Déjà recording sink: Kafka is THE record transport.
//!
//! A port of the reference (hyperswitch) sink — the envelope shapes are a **cross-repo
//! contract** with the déjà compactor, so field names and versions must match exactly.
//! The sink owns a DEDICATED `rdkafka` producer (never the audit/event publisher),
//! hardened for durability:
//!
//!   acks=all + enable.idempotence  → no acked-then-lost, no broker-side dupes
//!   bounded buffering              → backpressure surfaces as enqueue errors
//!                                    instead of unbounded memory
//!   flush = short poll             → cadence flushes never park the writer behind a
//!                                    slow broker; only the eof marker drains fully
//!
//! Envelopes, all on the ONE topic: `deja_artifact_record`/v2 (boundary events),
//! `deja_graph_node`/v1 (execution-graph nodes), `deja_sink_marker`/v2
//! (checkpoint / eof / dropped loss accounting).
//!
//! Partition key: `correlation_id` when present, else `{recording_run_id}:{global_sequence}`.
//! Headers carry sequence/run/boundary/method so a Vector consumer routes without parsing.
//!
//! Delivery semantics: enqueue errors surface as `io::Error` to the async writer, which
//! accounts the affected batch as dropped and keeps going. Request threads are never
//! failed by instrumentation.

use std::{io, time::Duration};

use rdkafka::{
    config::FromClientConfig,
    message::{Header, OwnedHeaders},
    producer::{BaseRecord, DefaultProducerContext, Producer, ThreadedProducer},
};
use serde::Serialize;

const SCHEMA_VERSION: u32 = 2;
const ARTIFACT_TYPE: &str = "deja_artifact_record";
const GRAPH_SCHEMA_VERSION: u32 = 1;
const GRAPH_ARTIFACT_TYPE: &str = "deja_graph_node";
const MARKER_ARTIFACT_TYPE: &str = "deja_sink_marker";
/// Cadence flushes are a short bounded poll: the threaded producer delivers on its own
/// background thread; a long drain here would park the writer behind a slow broker.
const CADENCE_FLUSH_POLL: Duration = Duration::from_millis(50);
/// End-of-run drain: the eof marker means "everything before this landed".
const EOF_FLUSH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize)]
struct Capture<'a> {
    mode: &'static str,
    session_id: &'a str,
}

#[derive(Serialize)]
struct Code<'a> {
    sha: Option<&'a str>,
    deja_version: &'static str,
}

#[derive(Serialize)]
struct Envelope<'a> {
    schema_version: u32,
    artifact_type: &'static str,
    instance_id: &'a str,
    recording_run_id: &'a str,
    correlation_id: Option<&'a str>,
    event_time_ns: u64,
    capture: Capture<'a>,
    code: Code<'a>,
    event: &'a deja::BoundaryEvent,
}

/// Graph-node envelope: nested under `node` — the node carries its own
/// `recording_run_id` / `global_sequence`, so flattening would collide.
#[derive(Serialize)]
struct GraphEnvelope<'a> {
    schema_version: u32,
    artifact_type: &'static str,
    instance_id: &'a str,
    recording_run_id: &'a str,
    capture: Capture<'a>,
    code: Code<'a>,
    node: &'a deja_core::ExecutionGraphNode,
}

/// Marker envelope: same stream, same session identity, no event payload.
#[derive(Serialize)]
struct MarkerEnvelope<'a> {
    schema_version: u32,
    artifact_type: &'static str,
    instance_id: &'a str,
    recording_run_id: &'a str,
    capture: Capture<'a>,
    code: Code<'a>,
    marker: MarkerBody<'a>,
}

#[derive(Serialize)]
struct MarkerBody<'a> {
    kind: &'static str,
    #[serde(flatten)]
    payload: &'a serde_json::Value,
}

pub struct UcsKafkaRecordSinkConfig<'a> {
    pub brokers: &'a [String],
    pub topic: &'a str,
    pub recording_run_id: &'a str,
    pub instance_id: String,
    pub code_sha: Option<String>,
    pub client_id: Option<&'a str>,
    pub acks: &'a str,
    pub enable_idempotence: bool,
    pub compression: Option<&'a str>,
    pub linger_ms: Option<u64>,
    pub message_timeout_ms: u64,
    pub queue_buffering_max_messages: u64,
    pub queue_buffering_max_kbytes: u64,
}

/// `RecordSink<deja::DejaRecord>` over a déjà-owned, durability-hardened Kafka producer.
pub struct UcsKafkaRecordSink {
    producer: ThreadedProducer<DefaultProducerContext>,
    topic: String,
    recording_run_id: String,
    instance_id: String,
    code_sha: Option<String>,
}

impl UcsKafkaRecordSink {
    /// Build the sink with its own hardened producer. Deliberately no constructor
    /// metadata probe: an unreachable broker must fail open at boot, not hang it.
    pub fn new(config: UcsKafkaRecordSinkConfig<'_>) -> io::Result<Self> {
        let mut producer_config = rdkafka::ClientConfig::new();
        producer_config
            .set("bootstrap.servers", config.brokers.join(","))
            .set("acks", config.acks)
            .set(
                "enable.idempotence",
                if config.enable_idempotence {
                    "true"
                } else {
                    "false"
                },
            )
            .set("message.timeout.ms", config.message_timeout_ms.to_string())
            // Bounded buffering: a dead broker turns into enqueue errors (counted and
            // ledgered by the writer), not unbounded memory.
            .set(
                "queue.buffering.max.messages",
                config.queue_buffering_max_messages.to_string(),
            )
            .set(
                "queue.buffering.max.kbytes",
                config.queue_buffering_max_kbytes.to_string(),
            );

        if let Some(client_id) = config.client_id.filter(|value| !value.is_empty()) {
            producer_config.set("client.id", client_id);
        }
        if let Some(compression) = config.compression.filter(|value| !value.is_empty()) {
            producer_config.set("compression.type", compression);
        }
        if let Some(linger_ms) = config.linger_ms {
            producer_config.set("linger.ms", linger_ms.to_string());
        }

        let producer = ThreadedProducer::from_config(&producer_config)
            .map_err(|error| io::Error::other(format!("deja kafka producer: {error}")))?;
        Ok(Self {
            producer,
            topic: config.topic.to_owned(),
            recording_run_id: config.recording_run_id.to_owned(),
            instance_id: config.instance_id,
            code_sha: config.code_sha,
        })
    }

    fn send(&self, key: &str, payload: &[u8], headers: OwnedHeaders) -> io::Result<()> {
        self.producer
            .send(
                BaseRecord::to(&self.topic)
                    .key(key)
                    .payload(payload)
                    .headers(headers),
            )
            .map_err(|(error, _record)| io::Error::other(format!("kafka send: {error}")))
    }

    fn write_boundary_event(&self, event: &deja::BoundaryEvent) -> io::Result<()> {
        let envelope = Envelope {
            schema_version: SCHEMA_VERSION,
            artifact_type: ARTIFACT_TYPE,
            instance_id: &self.instance_id,
            recording_run_id: &self.recording_run_id,
            correlation_id: event.correlation_id.as_deref(),
            event_time_ns: event.timestamp_ns,
            capture: Capture {
                // Session capture is the only mode today.
                mode: "session",
                session_id: &self.recording_run_id,
            },
            code: Code {
                sha: self.code_sha.as_deref(),
                deja_version: deja::PKG_VERSION,
            },
            event,
        };
        let payload = serde_json::to_vec(&envelope).map_err(io::Error::other)?;

        let key = match &event.correlation_id {
            Some(correlation_id) => correlation_id.clone(),
            None => format!("{}:{}", self.recording_run_id, event.global_sequence),
        };

        let global_seq = event.global_sequence.to_string();
        let request_seq = event.request_sequence.to_string();
        let headers = OwnedHeaders::new()
            .insert(Header {
                key: "global_sequence",
                value: Some(global_seq.as_str()),
            })
            .insert(Header {
                key: "request_sequence",
                value: Some(request_seq.as_str()),
            })
            .insert(Header {
                key: "recording_run_id",
                value: Some(self.recording_run_id.as_str()),
            })
            .insert(Header {
                key: "boundary",
                value: Some(event.boundary.as_str()),
            })
            .insert(Header {
                key: "method_name",
                value: Some(event.method_name.as_str()),
            });

        self.send(&key, &payload, headers)
    }

    fn write_graph_node(&self, node: &deja_core::ExecutionGraphNode) -> io::Result<()> {
        let envelope = GraphEnvelope {
            schema_version: GRAPH_SCHEMA_VERSION,
            artifact_type: GRAPH_ARTIFACT_TYPE,
            instance_id: &self.instance_id,
            recording_run_id: &self.recording_run_id,
            capture: Capture {
                mode: "session",
                session_id: &self.recording_run_id,
            },
            code: Code {
                sha: self.code_sha.as_deref(),
                deja_version: deja::PKG_VERSION,
            },
            node,
        };
        let payload = serde_json::to_vec(&envelope).map_err(io::Error::other)?;

        let key = match node.request_id() {
            Some(request_id) => request_id.to_owned(),
            None => format!("{}:{}", self.recording_run_id, node.global_sequence),
        };

        let global_seq = node.global_sequence.to_string();
        let headers = OwnedHeaders::new()
            .insert(Header {
                key: "global_sequence",
                value: Some(global_seq.as_str()),
            })
            .insert(Header {
                key: "recording_run_id",
                value: Some(self.recording_run_id.as_str()),
            })
            .insert(Header {
                key: "span_name",
                value: Some(node.span_name.as_str()),
            });

        self.send(&key, &payload, headers)
    }
}

impl deja::RecordSink<deja::DejaRecord> for UcsKafkaRecordSink {
    fn write_batch(&mut self, records: &[deja::DejaRecord]) -> io::Result<()> {
        for record in records {
            match record {
                deja::DejaRecord::BoundaryEvent(event) => self.write_boundary_event(event)?,
                deja::DejaRecord::GraphNode(node) => self.write_graph_node(node)?,
                // Record mode never produces observations; skip instead of failing the
                // writer if the library contract is breached.
                deja::DejaRecord::Observed(_) => {
                    debug_assert!(false, "observed record reached the record-mode Kafka sink");
                }
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Cadence flush: short bounded poll; in-flight messages when the poll expires are
        // NOT a sink failure — saturation surfaces as enqueue errors in write_batch.
        match self.producer.flush(CADENCE_FLUSH_POLL) {
            Ok(()) => Ok(()),
            Err(rdkafka::error::KafkaError::Flush(
                rdkafka::types::RDKafkaErrorCode::OperationTimedOut,
            )) => Ok(()),
            Err(error) => Err(io::Error::other(format!("kafka flush: {error}"))),
        }
    }

    fn write_marker(
        &mut self,
        kind: deja::MarkerKind,
        payload: &serde_json::Value,
    ) -> io::Result<()> {
        let envelope = MarkerEnvelope {
            schema_version: SCHEMA_VERSION,
            artifact_type: MARKER_ARTIFACT_TYPE,
            instance_id: &self.instance_id,
            recording_run_id: &self.recording_run_id,
            capture: Capture {
                mode: "session",
                session_id: &self.recording_run_id,
            },
            code: Code {
                sha: self.code_sha.as_deref(),
                deja_version: deja::PKG_VERSION,
            },
            marker: MarkerBody {
                kind: kind.as_str(),
                payload,
            },
        };
        let bytes = serde_json::to_vec(&envelope).map_err(io::Error::other)?;
        let key = format!("{}:marker", self.recording_run_id);
        let headers = OwnedHeaders::new()
            .insert(Header {
                key: "recording_run_id",
                value: Some(self.recording_run_id.as_str()),
            })
            .insert(Header {
                key: "marker_kind",
                value: Some(kind.as_str()),
            });
        self.send(&key, &bytes, headers)?;
        // The eof marker bounds delivery audits — drain for real so "eof landed" means
        // everything before it landed too.
        if matches!(kind, deja::MarkerKind::Eof) {
            let _ = self.producer.flush(EOF_FLUSH_TIMEOUT);
        }
        Ok(())
    }
}
