use std::sync::Arc;

use axum::{body::Bytes, extract::Extension, http::StatusCode, Json};
use domain_types::{
    connector_types::ConnectorEnum,
    feature_matrix::feature_matrix_types::{FeatureMatrixError, FeatureMatrixResponse},
};
use serde::Deserialize;
use ucs_env::configs::Config;

use crate::{
    http::error::HttpError,
    server::feature_matrix::{build_feature_matrix, parse_connector_name},
};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum FeatureMatrixRequest {
    ConnectorNames(Vec<String>),
    ConnectorObject {
        #[serde(alias = "connector_names")]
        connectors: Vec<String>,
    },
}

impl FeatureMatrixRequest {
    fn connector_names(self) -> Vec<String> {
        match self {
            Self::ConnectorNames(connectors) | Self::ConnectorObject { connectors } => connectors,
        }
    }
}

pub async fn feature_matrix(
    Extension(config): Extension<Arc<Config>>,
    body: Bytes,
) -> Result<Json<FeatureMatrixResponse>, HttpError> {
    let connectors = parse_connectors(body.as_ref())?;

    build_feature_matrix(connectors, &config)
        .map(Json)
        .map_err(http_error_from_feature_matrix_error)
}

fn parse_connectors(body: &[u8]) -> Result<Vec<ConnectorEnum>, HttpError> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(Vec::new());
    }

    let connector_names = serde_json::from_slice::<FeatureMatrixRequest>(body)
        .map(FeatureMatrixRequest::connector_names)
        .map_err(|error| HttpError {
            status: StatusCode::BAD_REQUEST,
            message: error.to_string(),
            details: None,
        })?;

    connector_names
        .into_iter()
        .map(|connector_name| {
            parse_connector_name(&connector_name).map_err(http_error_from_feature_matrix_error)
        })
        .collect()
}

fn http_error_from_feature_matrix_error(error: FeatureMatrixError) -> HttpError {
    let status = match &error {
        FeatureMatrixError::InvalidConnectorName(_) => StatusCode::BAD_REQUEST,
        FeatureMatrixError::ConnectorNotConfigured(_) => StatusCode::NOT_IMPLEMENTED,
    };

    HttpError {
        status,
        message: error.message(),
        details: None,
    }
}
