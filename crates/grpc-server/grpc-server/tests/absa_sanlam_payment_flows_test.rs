#![cfg(feature = "connector-request-kafka")]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use common_utils::request::{KafkaRecord, RequestContent};
use connector_request_kafka::{KafkaPublish, KafkaPublishResult};
use domain_types::router_response_types::Response;
use grpc_api_types::payments::{
    payment_method, payment_service_client::PaymentServiceClient, AuthenticationType, BankNames,
    BankType, BillingDescriptor, Currency, Eft, Money, PaymentAddress, PaymentMethod,
    PaymentServiceAuthorizeRequest, PaymentStatus,
};
use grpc_server::app;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde_json::{json, Value};
use tonic::{transport::Channel, Request};
use ucs_env::configs;
use uuid::Uuid;

mod common;

const CONNECTOR_NAME: &str = "absa_sanlam";
const TEST_API_KEY: &str = "test_absa_sanlam_api_key";
const TEST_MERCHANT_ID: &str = "test_absa_sanlam_merchant";
const TEST_ACCOUNT_NUMBER: &str = "12345678910";
const TEST_BRANCH_CODE: &str = "632005";
const TEST_ACCOUNT_HOLDER: &str = "Sanlam Test User";
const TEST_AMOUNT: i64 = 1250;

#[derive(Clone, Default)]
struct RecordingPublisher {
    records: Arc<Mutex<Vec<KafkaRecord>>>,
}

#[tonic::async_trait]
impl KafkaPublish for RecordingPublisher {
    async fn publish(&self, record: KafkaRecord) -> KafkaPublishResult {
        let topic = record.topic.clone();
        self.records
            .lock()
            .expect("Recording publisher lock should not be poisoned")
            .push(record);

        Ok(Ok(Response {
            headers: None,
            response: json!({ "status": "queued", "topic": topic })
                .to_string()
                .into_bytes()
                .into(),
            status_code: 200,
        }))
    }
}

fn kafka_header_value(record: &KafkaRecord, key: &str) -> Option<String> {
    record
        .headers
        .iter()
        .find_map(|(header_key, header_value)| {
            (header_key == key).then(|| header_value.clone().into_inner())
        })
}

fn kafka_payload_json(record: &KafkaRecord) -> Value {
    let payload = match record.payload.as_ref() {
        Some(
            content @ (RequestContent::Json(_)
            | RequestContent::FormUrlEncoded(_)
            | RequestContent::Xml(_)),
        ) => content.get_inner_value().expose().into_bytes(),
        Some(RequestContent::RawBytes(bytes)) => bytes.to_vec(),
        Some(RequestContent::FormData(_)) => panic!("Kafka payload should not be form data"),
        None => Vec::new(),
    };
    serde_json::from_slice(&payload).expect("Failed to parse Kafka payload as JSON")
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

fn default_absa_sanlam_topic() -> String {
    let config = configs::Config::new().expect("Failed while parsing config");
    format!("{}_payments_queue", config.connectors.absa_sanlam.base_url)
}

#[tokio::test]
async fn test_absa_sanlam_authorize_publishes_eft_debit_to_kafka() {
    let merchant_transaction_id = format!("absa_sanlam_authorize_{}", Uuid::new_v4().simple());
    let topic = default_absa_sanlam_topic();
    let publisher = RecordingPublisher::default();

    assert!(
        connector_request_kafka::set_publisher(Arc::new(publisher.clone())),
        "Kafka publisher should be installed once for this test binary"
    );

    grpc_test!(client, PaymentServiceClient<Channel>, {
        let mut request = Request::new(create_authorize_request(&merchant_transaction_id));
        add_absa_sanlam_metadata(&mut request);

        let response = Box::pin(client.authorize(request))
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        assert_eq!(response.status, i32::from(PaymentStatus::Pending));
        assert_eq!(response.status_code, 200);

        let records = publisher
            .records
            .lock()
            .expect("Recording publisher lock should not be poisoned");
        assert_eq!(records.len(), 1, "Expected one Kafka record");

        let record = records.first().expect("Expected Kafka record");
        assert_eq!(record.topic, topic);
        assert_eq!(
            kafka_header_value(record, "Authorization").as_deref(),
            Some(TEST_API_KEY)
        );
        assert_eq!(
            kafka_header_value(record, "Merchant-Id").as_deref(),
            Some(TEST_MERCHANT_ID)
        );
        assert_eq!(
            kafka_header_value(record, "Content-Type").as_deref(),
            Some("application/json")
        );

        let payload = kafka_payload_json(record);
        assert_eq!(
            payload.get("user_reference").and_then(Value::as_str),
            Some(merchant_transaction_id.as_str())
        );
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
