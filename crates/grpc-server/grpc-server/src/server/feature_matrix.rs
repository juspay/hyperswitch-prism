use std::str::FromStr;

use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::ConnectorEnum,
    feature_matrix::feature_matrix_types::{
        FeatureMatrixConnector, FeatureMatrixError,
        FeatureMatrixResponse as DomainFeatureMatrixResponse,
    },
    payment_method_data::DefaultPCIHolder,
    utils::ForeignTryFrom,
};
use grpc_api_types::payments::{
    connector_capability_service_server::ConnectorCapabilityService, Connector,
    FeatureMatrixRequest, FeatureMatrixResponse as GrpcFeatureMatrixResponse,
};
use interfaces::connector_types::ConnectorServiceTrait;
use strum::IntoEnumIterator;
use tonic::{Request, Response, Status};
use ucs_env::{
    configs::Config,
    error::{IntoGrpcStatus, ResultExtGrpc},
};

use crate::utils;

pub(crate) fn build_feature_matrix(
    requested_connectors: Vec<ConnectorEnum>,
    config: &Config,
) -> Result<DomainFeatureMatrixResponse, FeatureMatrixError> {
    let connectors = if requested_connectors.is_empty() {
        ConnectorEnum::iter()
            .filter_map(|connector_name| {
                build_feature_matrix_connector(connector_name, config).ok()
            })
            .collect()
    } else {
        requested_connectors
            .into_iter()
            .map(|connector_name| build_feature_matrix_connector(connector_name, config))
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(DomainFeatureMatrixResponse::new(connectors))
}

pub(crate) fn parse_connector_name(
    connector_name: &str,
) -> Result<ConnectorEnum, FeatureMatrixError> {
    let normalized_connector_name = connector_name.trim().to_ascii_lowercase().replace('-', "_");

    ConnectorEnum::from_str(&normalized_connector_name).map_err(|error| {
        tracing::error!(
            ?error,
            connector_name,
            normalized_connector_name,
            "failed to parse feature matrix connector name"
        );
        FeatureMatrixError::InvalidConnectorName(connector_name.to_string())
    })
}

fn build_feature_matrix_connector(
    connector_name: ConnectorEnum,
    config: &Config,
) -> Result<FeatureMatrixConnector, FeatureMatrixError> {
    let connector_data = ConnectorData::<DefaultPCIHolder>::get_connector_by_name(&connector_name);
    let connector = *connector_data.connector;

    build_connector_response(
        connector_name,
        connector,
        connector.base_url(&config.connectors),
    )
}

fn build_connector_response(
    connector_name: ConnectorEnum,
    connector: &(dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync),
    base_url: &str,
) -> Result<FeatureMatrixConnector, FeatureMatrixError> {
    let connector_info = connector
        .get_connector_about()
        .ok_or(FeatureMatrixError::ConnectorNotConfigured(connector_name))?;

    Ok(FeatureMatrixConnector::from_connector_details(
        connector_name,
        connector_info,
        base_url,
        connector.get_supported_payment_methods(),
        connector.get_supported_webhook_flows(),
    ))
}

#[derive(Debug, Clone)]
pub struct ConnectorCapability;

#[tonic::async_trait]
impl ConnectorCapabilityService for ConnectorCapability {
    async fn get_feature_matrix(
        &self,
        request: Request<FeatureMatrixRequest>,
    ) -> Result<Response<GrpcFeatureMatrixResponse>, Status> {
        let config = utils::get_config_from_request(&request).into_grpc_status()?;
        let request = request.into_inner();
        let connectors = request
            .connectors
            .into_iter()
            .map(connector_from_proto)
            .collect::<Result<Vec<_>, _>>()?;

        build_feature_matrix(connectors, &config)
            .map(Into::into)
            .map(Response::new)
            .map_err(Status::from)
    }
}

fn connector_from_proto(connector: i32) -> Result<ConnectorEnum, Status> {
    let connector = Connector::try_from(connector).map_err(|_| {
        Status::invalid_argument(format!("Invalid connector enum value: {connector}"))
    })?;

    ConnectorEnum::foreign_try_from(connector).map_err(IntoGrpcStatus::into_grpc_status)
}
