use std::str::FromStr;

use common_utils::consts::X_CONNECTOR_NAME;
use domain_types::connector_types::ConnectorEnum;
use grpc_api_types::payments::{
    AccessToken, CustomerServiceCreateResponse,
    MerchantAuthenticationServiceCreateAccessTokenResponse,
};

pub fn connector_from_composite_authorize_metadata(
    metadata: &tonic::metadata::MetadataMap,
) -> Result<ConnectorEnum, Box<tonic::Status>> {
    metadata
        .get(X_CONNECTOR_NAME)
        .ok_or_else(|| {
            Box::new(tonic::Status::invalid_argument(
                "missing x-connector metadata",
            ))
        })
        .and_then(|connector| {
            connector.to_str().map_err(|_| {
                Box::new(tonic::Status::invalid_argument(
                    "invalid x-connector metadata value",
                ))
            })
        })
        .and_then(|connector_from_metadata| {
            ConnectorEnum::from_str(connector_from_metadata).map_err(|err| {
                Box::new(tonic::Status::invalid_argument(format!(
                    "Connector not supported: {err}"
                )))
            })
        })
}

pub fn grpc_connector_from_connector_enum(connector: &ConnectorEnum) -> i32 {
    let grpc_connector_name = connector.to_string().to_ascii_uppercase();
    let grpc_connector =
        grpc_api_types::payments::Connector::from_str_name(grpc_connector_name.as_str())
            .unwrap_or(grpc_api_types::payments::Connector::Unspecified);
    i32::from(grpc_connector)
}

pub fn get_connector_customer_id(
    connector_customer_id_from_request: Option<String>,
    create_connector_customer_response: Option<&CustomerServiceCreateResponse>,
) -> Option<String> {
    connector_customer_id_from_request
        .or_else(|| create_connector_customer_response.map(|res| res.connector_customer_id.clone()))
}

pub fn access_token_from_create_access_token_response(
    access_token_response: Option<&MerchantAuthenticationServiceCreateAccessTokenResponse>,
) -> Option<AccessToken> {
    access_token_response.and_then(|response| {
        response.access_token.clone().map(|token| AccessToken {
            token: Some(token),
            token_type: response.token_type.clone(),
            expires_in_seconds: response.expires_in_seconds,
        })
    })
}

pub fn get_access_token(
    access_token_from_request: Option<AccessToken>,
    access_token_response: Option<&MerchantAuthenticationServiceCreateAccessTokenResponse>,
) -> Option<AccessToken> {
    access_token_from_request
        .or_else(|| access_token_from_create_access_token_response(access_token_response))
}
