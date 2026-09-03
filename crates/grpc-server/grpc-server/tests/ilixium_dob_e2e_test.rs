#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::as_conversions)]
#![allow(clippy::print_stdout)]
#![allow(clippy::panic)]
#![allow(clippy::large_futures)]

use cards::CardNumber;
use grpc_api_types::payments::{
    payment_method, payment_service_client::PaymentServiceClient, Address, AuthenticationType,
    CaptureMethod, CardDetails, Currency, PaymentAddress, PaymentMethod,
    PaymentServiceAuthorizeRequest,
};
use grpc_server::app;
use hyperswitch_masking::Secret;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tonic::{transport::Channel, Request};
use ucs_env::configs;
mod common;

/// Shaped after Ilixium's own `/direct/auth` success example.
const ILIXIUM_AUTH_RESPONSE: &str = r#"{
  "version": 2,
  "type": "AUTH_CAP",
  "transaction": { "merchantRef": "dobprobe123456", "gatewayRef": "33412341234123", "currency": "GBP" },
  "status": { "code": "SUCCESS", "message": "Approved" },
  "paymentHistory": {
    "paymentAttempt": [{
      "order": 1,
      "code": "SUCCESS",
      "paymentMethodType": "CARD",
      "cardResponse": { "cardBin": "411111", "cardLastFour": "1111", "authCode": "86394" }
    }]
  }
}"#;

/// Stands in for Ilixium and records every request body it is sent, so a test can assert on what
/// UCS actually put on the wire to the connector.
async fn spawn_recording_stub(captured: Arc<Mutex<Vec<String>>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub");
    let addr = listener.local_addr().expect("stub addr");

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                continue;
            };
            let captured = Arc::clone(&captured);
            tokio::spawn(async move {
                let mut buf = vec![0_u8; 16384];
                if let Ok(n) = socket.read(&mut buf).await {
                    let raw = String::from_utf8_lossy(buf.get(..n).unwrap_or_default()).to_string();
                    if let Some(body) = raw.split("\r\n\r\n").nth(1) {
                        captured.lock().expect("lock").push(body.to_string());
                    }
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    ILIXIUM_AUTH_RESPONSE.len(),
                    ILIXIUM_AUTH_RESPONSE
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });

    format!("http://{addr}/")
}

/// An Ilixium card authorisation carrying `customer.date_of_birth` — the field Hyperswitch
/// populates from the payment request's `customer` object.
fn authorize_request(date_of_birth: &str) -> Request<PaymentServiceAuthorizeRequest> {
    let mut request = Request::new(PaymentServiceAuthorizeRequest {
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: 1000,
            currency: Currency::Gbp as i32,
        }),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(Secret::new("test@test.com".to_string())),
            first_name: Some("Test".to_string()),
            last_name: Some("Client".to_string()),
            date_of_birth: Some(Secret::new(date_of_birth.to_string())),
            ..Default::default()
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),
                card_exp_month: Some(Secret::new("06".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("111".to_string())),
                ..Default::default()
            })),
        }),
        address: Some(PaymentAddress {
            shipping_address: None,
            billing_address: Some(Address {
                first_name: Some(Secret::new("Test".to_string())),
                last_name: Some(Secret::new("Client".to_string())),
                line1: Some(Secret::new("123 Street".to_string())),
                city: Some(Secret::new("Guildford".to_string())),
                zip_code: Some(Secret::new("GU2 2YG".to_string())),
                country_alpha2_code: Some(grpc_api_types::payments::CountryAlpha2::Gb as i32),
                email: Some(Secret::new("test@test.com".to_string())),
                phone_number: Some(Secret::new("01234123123".to_string())),
                ..Default::default()
            }),
        }),
        auth_type: AuthenticationType::NoThreeDs as i32,
        capture_method: Some(CaptureMethod::Automatic as i32),
        merchant_transaction_id: Some("dobprobe123456".to_string()),
        return_url: Some("https://hyperswitch.io/".to_string()),
        ..Default::default()
    });

    let md = request.metadata_mut();
    md.insert("x-connector", "ilixium".parse().expect("valid header"));
    md.insert("x-auth", "signature-key".parse().expect("valid header"));
    md.insert("x-api-key", "probe_api_key".parse().expect("valid header"));
    md.insert("x-key1", "1000003".parse().expect("valid header"));
    md.insert(
        "x-api-secret",
        "probe_api_secret".parse().expect("valid header"),
    );
    request
}

/// Boots the UCS in-process with Ilixium pointed at `stub_url`.
///
/// The stub URL goes on the config itself rather than the `x-config-override` header: the test
/// harness's `ConfigInterceptor` inserts the base config verbatim and ignores that header
/// (production applies it via a tower Layer), so a header-based override would silently leave the
/// connector calling the real Ilixium.
async fn ucs_client(
    stub_url: &str,
) -> (
    impl std::future::Future<Output = ()>,
    PaymentServiceClient<Channel>,
) {
    let mut config = configs::Config::new().expect("Failed while parsing config");
    config.connectors.ilixium.base_url = stub_url.to_string();
    assert!(
        config.connectors.ilixium.base_url.contains("127.0.0.1"),
        "refusing to run: ilixium base_url must point at the local stub"
    );

    let base_config = Arc::new(config);
    let server = app::Service::new(base_config.clone()).await;
    common::server_and_client_stub::<PaymentServiceClient<Channel>>(server, base_config)
        .await
        .expect("Failed to create the server client pair")
}

/// `customer.date_of_birth` must reach Ilixium as `customer.dateOfBirth` in `ddmmyyyy` — inside
/// the `customer` object, not the merchant `metadata` blob it used to be smuggled through.
#[tokio::test]
async fn ilixium_sends_customer_date_of_birth_to_connector() {
    let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stub_url = spawn_recording_stub(Arc::clone(&captured)).await;
    println!("Ilixium stub listening on {stub_url}");

    let (server_fut, mut client) = ucs_client(&stub_url).await;
    let server_fut = Box::pin(server_fut);

    let probe = Box::pin(async {
        let response = Box::pin(client.authorize(authorize_request("1990-01-31"))).await;
        match &response {
            Ok(r) => println!(
                "status={:?} error={:?}",
                r.get_ref().status,
                r.get_ref().error
            ),
            Err(status) => panic!("authorize failed: {status:?}"),
        }

        let bodies = captured.lock().expect("lock").clone();
        assert!(
            !bodies.is_empty(),
            "the connector never called the stub — nothing to assert on"
        );
        for body in &bodies {
            println!("outbound /direct/auth body:\n{body}");
        }

        let joined = bodies.join("\n");
        assert!(
            joined.contains(r#""dateOfBirth":"31011990""#),
            "expected customer.dateOfBirth=31011990 (ddmmyyyy) in the outbound body, got:\n{joined}"
        );
        assert!(
            !joined.contains("ilixium_date_of_birth"),
            "the date of birth must travel in `customer`, not the metadata blob"
        );
        println!("PASS: customer.date_of_birth 1990-01-31 -> dateOfBirth=31011990");
    });

    tokio::select! {
        _ = server_fut => panic!("Server failed"),
        _ = probe => {}
    }
}

/// A malformed date must name the field it came from. Before the shared parser threaded a field
/// name through, this surfaced as `InvalidDataFormat { field_name: "unknown" }`, which told an
/// integrator nothing about which date on the request was rejected.
#[tokio::test]
async fn malformed_date_of_birth_names_the_offending_field() {
    let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stub_url = spawn_recording_stub(Arc::clone(&captured)).await;

    let (server_fut, mut client) = ucs_client(&stub_url).await;
    let server_fut = Box::pin(server_fut);

    let probe = Box::pin(async {
        // `31-01-1990` is the plausible wrong guess: right date, wrong format.
        let response = Box::pin(client.authorize(authorize_request("31-01-1990"))).await;

        let rendered = match &response {
            Ok(r) => format!("{:?}", r.get_ref().error),
            Err(status) => format!("{status:?}"),
        };
        println!("rejection: {rendered}");

        assert!(
            rendered.contains("customer.date_of_birth"),
            "the error must name the offending field, got: {rendered}"
        );
        assert!(
            !rendered.contains("unknown"),
            "the error must not fall back to field_name \"unknown\", got: {rendered}"
        );
        assert!(
            captured.lock().expect("lock").is_empty(),
            "a request with an unparseable date must be rejected before the connector is called"
        );
        println!("PASS: malformed date rejected naming customer.date_of_birth");
    });

    tokio::select! {
        _ = server_fut => panic!("Server failed"),
        _ = probe => {}
    }
}
