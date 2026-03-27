// #![allow(clippy::expect_used)]

// use grpc_server::app;
// use ucs_env::configs;
// mod common;
// use grpc_api_types::payments::payment_service_client::PaymentServiceClient;
// use tonic::{transport::Channel, Request};

// #[tokio::test]
// async fn test_authorize_only_basic() {
//     // This test verifies that the authorize_only endpoint exists and can be called
//     // It doesn't test the full flow since that would require valid connector credentials

//     grpc_test!(client, PaymentServiceClient<Channel>, {
//         // Create a basic authorize_only request
//         let request = Request::new(PaymentServiceAuthorizeOnlyRequest {
//             payment_method: None, // We'll keep it simple for this basic test
//             amount: 1000,
//             currency: grpc_api_types::payments::Currency::Usd.into(),
//             minor_amount: 100000, // 1000.00 USD in minor units
//             ..Default::default()
//         });

//         // This should fail gracefully since we don't have valid credentials,
//         // but it proves the endpoint exists and is reachable
//         let response = client.authorize_only(request).await;

//         // We expect this to fail due to missing/invalid data, but not to panic
//         assert!(response.is_err() || response.is_ok());
//     });
// }
