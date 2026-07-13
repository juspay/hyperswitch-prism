use grpc_api_types::payments::{
    connector_capability_service_server::ConnectorCapabilityService, FeatureMatrixRequest,
    FeatureMatrixResponse,
};
use tonic::{Request, Response, Status};

use crate::{feature_matrix::build_feature_matrix, utils};

#[derive(Debug, Clone)]
pub struct ConnectorCapability;

#[tonic::async_trait]
impl ConnectorCapabilityService for ConnectorCapability {
    async fn get_feature_matrix(
        &self,
        request: Request<FeatureMatrixRequest>,
    ) -> Result<Response<FeatureMatrixResponse>, Status> {
        let config = utils::get_config_from_request(&request)?;
        let request = request.into_inner();

        build_feature_matrix(request.connectors, &config)
            .map(Into::into)
            .map(Response::new)
            .map_err(Status::from)
    }
}
