use common_enums::KafkaClientError;
use common_utils::{
    connector_request_kafka::ConnectorRequestKafkaConfig,
    request::{KafkaRecord, RequestContent},
    CustomResult,
};
use domain_types::router_response_types::Response;
use error_stack::ResultExt;
use hyperswitch_masking::ExposeInterface;
use once_cell::sync::OnceCell;
use rdkafka::{
    config::ClientConfig,
    error::{KafkaError, RDKafkaErrorCode},
    message::{Header, OwnedHeaders, OwnedMessage},
    producer::{FutureProducer, FutureRecord, Producer},
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

static KAFKA_PRODUCER: OnceCell<Arc<FutureProducer>> = OnceCell::new();

pub fn init_kafka_producer(
    config: &ConnectorRequestKafkaConfig,
) -> CustomResult<(), KafkaClientError> {
    if config.brokers.is_empty() {
        Err(KafkaClientError::InvalidConfiguration {
            message: "connector_request_kafka.brokers cannot be empty when Kafka connector request publishing is enabled".to_string(),
        })?
    }

    if config.message_timeout_ms < config.request_timeout_ms {
        Err(KafkaClientError::InvalidConfiguration {
            message: "connector_request_kafka.message_timeout_ms must be >= connector_request_kafka.request_timeout_ms".to_string(),
        })?
    }

    let mut client_config = ClientConfig::new();

    client_config
        .set("bootstrap.servers", config.brokers.join(","))
        .set("acks", &config.acks)
        .set("request.timeout.ms", config.request_timeout_ms.to_string())
        .set("message.timeout.ms", config.message_timeout_ms.to_string());

    if let Some(ref protocol) = config.security_protocol {
        client_config.set("security.protocol", protocol);
    }
    if let Some(ref mechanism) = config.sasl_mechanism {
        client_config.set("sasl.mechanisms", mechanism);
    }
    if let Some(ref username) = config.sasl_username {
        client_config.set("sasl.username", username);
    }
    if let Some(ref password) = config.sasl_password {
        client_config.set("sasl.password", password);
    }

    let producer: FutureProducer = client_config
        .create()
        .change_context(KafkaClientError::ProducerConstructionFailed)?;

    producer
        .client()
        .fetch_metadata(None, Duration::from_secs(5))
        .change_context(KafkaClientError::MetadataFetchFailed)?;

    let _ = KAFKA_PRODUCER.set(Arc::new(producer));

    tracing::info!(brokers = %config.brokers.join(","), "Kafka producer for publishing connector requests initialized successfully");

    Ok(())
}

pub async fn publish_to_kafka(
    kafka_record: KafkaRecord,
) -> error_stack::Result<Result<Response, Response>, KafkaClientError> {
    let producer = KAFKA_PRODUCER
        .get()
        .ok_or(KafkaClientError::ProducerNotInitialized)?;

    // Build OwnedHeaders from the KafkaRecord headers.
    let mut owned_headers = OwnedHeaders::new();

    for (key, value) in &kafka_record.headers {
        owned_headers = owned_headers.insert(Header {
            key: key.as_str(),
            value: Some(value.clone().into_inner().as_str()),
        });
    }

    let payload_bytes = match kafka_record.payload {
        Some(
            ref content @ (RequestContent::Json(_)
            | RequestContent::FormUrlEncoded(_)
            | RequestContent::Xml(_)),
        ) => content.get_inner_value().expose().into_bytes(),
        Some(RequestContent::RawBytes(bytes)) => bytes,
        Some(RequestContent::FormData(_)) => Err(KafkaClientError::UnsupportedPayloadFormat {
            format: "form_data".to_string(),
        })?,
        None => Vec::new(),
    };

    let timeout = Duration::from_secs(5);
    let delivery_result = match kafka_record.key.as_deref() {
        Some(key) => {
            producer
                .send(
                    FutureRecord::to(&kafka_record.topic)
                        .payload(&payload_bytes)
                        .key(key)
                        .headers(owned_headers),
                    timeout,
                )
                .await
        }
        None => {
            producer
                .send(
                    FutureRecord::<(), _>::to(&kafka_record.topic)
                        .payload(&payload_bytes)
                        .headers(owned_headers),
                    timeout,
                )
                .await
        }
    };

    Ok(classify_kafka_delivery_result(
        delivery_result,
        &kafka_record.topic,
    ))
}

/// Map a raw rdkafka delivery result to a synthetic [`Response`].
#[allow(clippy::result_large_err)]
fn classify_kafka_delivery_result(
    delivery_result: Result<(i32, i64), (KafkaError, OwnedMessage)>,
    topic: &str,
) -> Result<Response, Response> {
    match delivery_result {
        // Broker acknowledged — message is confirmed in the queue.
        Ok((_partition, _offset)) => {
            let body = json!({ "status": "queued", "topic": topic });

            Ok(Response {
                headers: None,
                response: body.to_string().into_bytes().into(),
                status_code: 200,
            })
        }

        // Delivery failed. Classify into confirmed-failure (200) vs unknown (500).
        Err((kafka_error, _original_message)) => {
            tracing::error!(
                %kafka_error,
                topic,
                "Kafka connector request publish failed"
            );

            let kafka_error_str = kafka_error.to_string();

            let (error_code, error_message) = kafka_error_str
                .split_once(": ")
                .unwrap_or((kafka_error_str.as_str(), kafka_error_str.as_str()));

            match &kafka_error {
                // Confirmed failures — broker explicitly rejected the message.
                // The message is definitively NOT in the queue.
                KafkaError::MessageProduction(
                    RDKafkaErrorCode::MessageSizeTooLarge
                    | RDKafkaErrorCode::InvalidMessageSize
                    | RDKafkaErrorCode::InvalidTopic
                    | RDKafkaErrorCode::TopicAuthorizationFailed
                    | RDKafkaErrorCode::InvalidRequiredAcks
                    | RDKafkaErrorCode::ClusterAuthorizationFailed
                    | RDKafkaErrorCode::UnknownTopicOrPartition
                    | RDKafkaErrorCode::NotEnoughReplicas
                    | RDKafkaErrorCode::UnsupportedForMessageFormat
                    | RDKafkaErrorCode::InvalidMessage
                    | RDKafkaErrorCode::PolicyViolation,
                ) => {
                    let body = json!({
                        "status": "rejected",
                        "error_code": error_code,
                        "error_message": error_message,
                        "topic": topic
                    });

                    Ok(Response {
                        headers: None,
                        response: body.to_string().into_bytes().into(),
                        status_code: 200,
                    })
                }
                // All other errors (timeouts, network, leader changes, queue full, …)
                // — outcome is unknown; treat as 5xx so the caller can retry or
                // implement idempotency checks.
                _ => {
                    let body = json!({
                        "status": "unknown",
                        "error_code": error_code,
                        "error_message": error_message,
                        "topic": topic
                    });

                    Err(Response {
                        headers: None,
                        response: body.to_string().into_bytes().into(),
                        status_code: 500,
                    })
                }
            }
        }
    }
}
