use grpc_api_types::payments::{
    connector_info_service_server::ConnectorInfoService, FeatureMatrixRequest,
    FeatureMatrixResponse,
};
use tonic::{Request, Response, Status};

use crate::{
    feature_matrix::{build_feature_matrix, FeatureMatrixError},
    utils,
};

#[derive(Debug, Clone)]
pub struct ConnectorInfo;

#[tonic::async_trait]
impl ConnectorInfoService for ConnectorInfo {
    async fn feature_matrix(
        &self,
        request: Request<FeatureMatrixRequest>,
    ) -> Result<Response<FeatureMatrixResponse>, Status> {
        let config = utils::get_config_from_request(&request)?;
        let request = request.into_inner();

        build_feature_matrix(request.connectors, &config)
            .map(Into::into)
            .map(Response::new)
            .map_err(status_from_feature_matrix_error)
    }
}

fn status_from_feature_matrix_error(error: FeatureMatrixError) -> Status {
    match &error {
        FeatureMatrixError::InvalidConnectorName(_) => Status::invalid_argument(error.message()),
        FeatureMatrixError::ConnectorNotConfigured(_) => Status::unimplemented(error.message()),
    }
}
