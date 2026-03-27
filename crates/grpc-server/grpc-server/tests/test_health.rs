#![allow(clippy::expect_used)]

use grpc_server::app;
use ucs_env::configs;
mod common;
use grpc_api_types::health_check::{health_client::HealthClient, HealthCheckRequest};
use tonic::{transport::Channel, Request};

#[tokio::test]
async fn test_health() {
    grpc_test!(client, HealthClient<Channel>, {
        let response = client
            .check(Request::new(HealthCheckRequest {
                service: "connector_service".to_string(),
            }))
            .await
            .expect("Failed to call health check")
            .into_inner();

        assert_eq!(
            response.status(),
            grpc_api_types::health_check::health_check_response::ServingStatus::Serving
        );
    });
}
