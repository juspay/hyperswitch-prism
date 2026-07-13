use std::str::FromStr;

use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::ConnectorEnum,
    feature_matrix::feature_matrix_types::{
        FeatureMatrixConnector, FeatureMatrixError, FeatureMatrixResponse,
    },
    payment_method_data::DefaultPCIHolder,
};
use interfaces::connector_types::ConnectorServiceTrait;
use strum::IntoEnumIterator;
use ucs_env::configs::Config;

pub fn build_feature_matrix(
    connector_names: Vec<String>,
    config: &Config,
) -> Result<FeatureMatrixResponse, FeatureMatrixError> {
    let connectors = if connector_names.is_empty() {
        ConnectorEnum::iter()
            .filter_map(|connector_name| {
                build_feature_matrix_connector(connector_name, config).ok()
            })
            .collect()
    } else {
        connector_names
            .into_iter()
            .map(|connector_name| {
                let connector = parse_connector_name(&connector_name)?;
                build_feature_matrix_connector(connector, config)
            })
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(FeatureMatrixResponse::new(connectors))
}

fn parse_connector_name(connector_name: &str) -> Result<ConnectorEnum, FeatureMatrixError> {
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
