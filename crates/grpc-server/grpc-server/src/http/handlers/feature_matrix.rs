use std::sync::Arc;

use axum::{body::Bytes, extract::Extension, http::StatusCode, Json};
use domain_types::feature_matrix::feature_matrix_types::{
    FeatureMatrixError, FeatureMatrixResponse,
};
use serde::Deserialize;
use ucs_env::configs::Config;

use crate::{feature_matrix::build_feature_matrix, http::error::HttpError};

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
    let connector_names = parse_connector_names(body.as_ref())?;

    build_feature_matrix(connector_names, &config)
        .map(Json)
        .map_err(http_error_from_feature_matrix_error)
}

fn parse_connector_names(body: &[u8]) -> Result<Vec<String>, HttpError> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(Vec::new());
    }

    serde_json::from_slice::<FeatureMatrixRequest>(body)
        .map(FeatureMatrixRequest::connector_names)
        .map_err(|error| HttpError {
            status: StatusCode::BAD_REQUEST,
            message: error.to_string(),
            details: None,
        })
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
