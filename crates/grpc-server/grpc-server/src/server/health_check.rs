use grpc_api_types::health_check::{self, health_server};
use tonic::{Request, Response, Status};

pub struct HealthCheck;

#[tonic::async_trait]
impl health_server::Health for HealthCheck {
    async fn check(
        &self,
        request: Request<health_check::HealthCheckRequest>,
    ) -> Result<Response<health_check::HealthCheckResponse>, Status> {
        tracing::debug!(?request, "health_check request");

        let response = health_check::HealthCheckResponse {
            status: health_check::health_check_response::ServingStatus::Serving.into(),
        };
        tracing::info!(?response, "health_check response");

        Ok(Response::new(response))
    }
}
