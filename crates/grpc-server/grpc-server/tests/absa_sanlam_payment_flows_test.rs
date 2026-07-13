#![cfg(feature = "connector-request-kafka")]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use grpc_api_types::payments::{
    payment_method, payment_service_client::PaymentServiceClient, AuthenticationType, BankNames,
    BankType, BillingDescriptor, Currency, Eft, Money, PaymentAddress, PaymentMethod,
    PaymentServiceAuthorizeRequest, PaymentStatus,
};
use grpc_server::app;
use hyperswitch_masking::Secret;
use rdkafka::{
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedHeaders, Headers, Message},
};
use serde_json::Value;
use serial_test::serial;
use tokio::time::timeout;
use tonic::{transport::Channel, Request};
use ucs_env::configs;
use uuid::Uuid;

mod common;

const CONNECTOR_NAME: &str = "absa_sanlam";
const KAFKA_BROKER: &str = "localhost:9092";
const TEST_API_KEY: &str = "test_absa_sanlam_api_key";
const TEST_MERCHANT_ID: &str = "test_absa_sanlam_merchant";
const TEST_ACCOUNT_NUMBER: &str = "12345678910";
const TEST_BRANCH_CODE: &str = "632005";
const TEST_ACCOUNT_HOLDER: &str = "Sanlam Test User";
const TEST_AMOUNT: i64 = 1250;

fn kafka_consumer() -> StreamConsumer {
    ClientConfig::new()
        .set("bootstrap.servers", KAFKA_BROKER)
        .set(
            "group.id",
            format!("absa-sanlam-authorize-test-{}", Uuid::new_v4()),
        )
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .create()
        .expect("Failed to create Kafka consumer")
}

fn kafka_header_value(headers: &BorrowedHeaders, key: &str) -> Option<String> {
    (0..headers.count()).find_map(|index| {
        let header = headers.get(index);
        (header.key == key).then(|| {
            header
                .value
                .map(|value| String::from_utf8_lossy(value).to_string())
                .unwrap_or_default()
        })
    })
}

async fn consume_authorize_message(
    consumer: &StreamConsumer,
    merchant_transaction_id: &str,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);

    loop {
        let now = Instant::now();
        assert!(
            now < deadline,
            "Timed out waiting for Kafka authorize message"
        );

        let remaining = deadline.saturating_duration_since(now);
        let message = timeout(remaining, consumer.recv())
            .await
            .expect("Timed out waiting for Kafka message")
            .expect("Failed to consume Kafka message");

        let payload = message
            .payload_view::<str>()
            .expect("Kafka message should have a payload")
            .expect("Kafka payload should be valid UTF-8");

        let payload: Value =
            serde_json::from_str(payload).expect("Kafka payload should be valid JSON");

        if payload
            .get("user_reference")
            .and_then(Value::as_str)
            .is_some_and(|user_reference| user_reference == merchant_transaction_id)
        {
            let headers = message
                .headers()
                .expect("Kafka message should include headers");
            assert_eq!(
                kafka_header_value(headers, "Authorization").as_deref(),
                Some(TEST_API_KEY)
            );
            assert_eq!(
                kafka_header_value(headers, "Merchant-Id").as_deref(),
                Some(TEST_MERCHANT_ID)
            );
            assert_eq!(
                kafka_header_value(headers, "Content-Type").as_deref(),
                Some("application/json")
            );
            return payload;
        }
    }
}

fn add_absa_sanlam_metadata<T>(request: &mut Request<T>) {
    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("Failed to parse x-connector"),
    );
    request.metadata_mut().append(
        "x-auth",
        "body-key".parse().expect("Failed to parse x-auth"),
    );
    request.metadata_mut().append(
        "x-api-key",
        TEST_API_KEY.parse().expect("Failed to parse x-api-key"),
    );
    request.metadata_mut().append(
        "x-key1",
        TEST_MERCHANT_ID.parse().expect("Failed to parse x-key1"),
    );
    request.metadata_mut().append(
        "x-merchant-id",
        "test_merchant"
            .parse()
            .expect("Failed to parse x-merchant-id"),
    );
    request.metadata_mut().append(
        "x-request-id",
        format!("absa_sanlam_authorize_{}", Uuid::new_v4())
            .parse()
            .expect("Failed to parse x-request-id"),
    );
    request.metadata_mut().append(
        "x-tenant-id",
        "default".parse().expect("Failed to parse x-tenant-id"),
    );
}

fn create_authorize_request(merchant_transaction_id: &str) -> PaymentServiceAuthorizeRequest {
    PaymentServiceAuthorizeRequest {
        merchant_transaction_id: Some(merchant_transaction_id.to_string()),
        amount: Some(Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Zar),
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Eft(Eft {
                account_number: Some(Secret::new(TEST_ACCOUNT_NUMBER.to_string())),
                branch_code: Some(Secret::new(TEST_BRANCH_CODE.to_string())),
                bank_account_holder_name: Some(Secret::new(TEST_ACCOUNT_HOLDER.to_string())),
                bank_name: i32::from(BankNames::Absa),
                bank_type: i32::from(BankType::Savings),
            })),
        }),
        capture_method: None,
        address: Some(PaymentAddress::default()),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        metadata: Some(Secret::new(
            r#"{"batch_user_reference":"absa-sanlam-authorize-test"}"#.to_string(),
        )),
        billing_descriptor: Some(BillingDescriptor {
            statement_descriptor: Some("Sanlam debit order".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn set_kafka_config_env() {
    std::env::set_var("CS__CONNECTOR_REQUEST_KAFKA__ENABLED", "true");
    std::env::set_var("CS__CONNECTOR_REQUEST_KAFKA__BROKERS", KAFKA_BROKER);
}

fn default_absa_sanlam_topic() -> String {
    let config = configs::Config::new().expect("Failed while parsing config");
    format!("{}_payments_queue", config.connectors.absa_sanlam.base_url)
}

#[tokio::test]
#[serial]
async fn test_absa_sanlam_authorize_publishes_eft_debit_to_kafka() {
    let merchant_transaction_id = format!("absa_sanlam_authorize_{}", Uuid::new_v4().simple());
    let topic = default_absa_sanlam_topic();

    set_kafka_config_env();
    let consumer = kafka_consumer();
    consumer
        .subscribe(&[topic.as_str()])
        .expect("Failed to subscribe to Kafka topic");

    grpc_test!(client, PaymentServiceClient<Channel>, {
        let mut request = Request::new(create_authorize_request(&merchant_transaction_id));
        add_absa_sanlam_metadata(&mut request);

        let response = Box::pin(client.authorize(request))
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        assert_eq!(response.status, i32::from(PaymentStatus::Pending));
        assert_eq!(response.status_code, 200);

        let payload = consume_authorize_message(&consumer, &merchant_transaction_id).await;
        assert_eq!(
            payload.get("amount").and_then(Value::as_i64),
            Some(TEST_AMOUNT)
        );
        assert_eq!(payload.get("currency").and_then(Value::as_str), Some("ZAR"));
        assert_eq!(
            payload.get("statement_descriptor").and_then(Value::as_str),
            Some("Sanlam debit order")
        );

        let metadata = payload
            .get("metadata")
            .and_then(Value::as_object)
            .expect("Expected metadata payload");
        assert_eq!(
            metadata.get("batch_user_reference").and_then(Value::as_str),
            Some("absa-sanlam-authorize-test")
        );

        let eft = payload
            .get("payment_method")
            .and_then(|payment_method| payment_method.get("eft_debit_order"))
            .expect("Expected eft_debit_order payload");
        assert_eq!(
            eft.get("homing_account").and_then(Value::as_str),
            Some(TEST_ACCOUNT_NUMBER)
        );
        assert_eq!(
            eft.get("homing_branch").and_then(Value::as_str),
            Some(TEST_BRANCH_CODE)
        );
        assert_eq!(
            eft.get("homing_account_name").and_then(Value::as_str),
            Some(TEST_ACCOUNT_HOLDER)
        );
        assert_eq!(eft.get("bank_name").and_then(Value::as_str), Some("absa"));
        assert_eq!(
            eft.get("bank_type").and_then(Value::as_str),
            Some("savings")
        );
    });
}
