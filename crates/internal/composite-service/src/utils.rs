use std::str::FromStr;

use common_utils::consts::{
    X_AUTHENTICATOR_CONNECTOR_NAME, X_CONNECTOR_NAME, X_FRM_CONNECTOR_NAME,
    X_SURCHARGE_CONNECTOR_NAME,
};
use domain_types::connector_types::{
    AuthenticatorConnectorEnum, ConnectorEnum, ConnectorVariant, FrmConnectorEnum,
    SurchargeConnectorEnum,
};
use grpc_api_types::payments::{
    AccessToken, CustomerServiceCreateResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse, PaymentStatus,
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

pub fn frm_connector_from_composite_frm_metadata(
    metadata: &tonic::metadata::MetadataMap,
) -> Result<Option<FrmConnectorEnum>, Box<tonic::Status>> {
    metadata
        .get(X_FRM_CONNECTOR_NAME)
        .map(|connector| {
            connector
                .to_str()
                .map_err(|_| {
                    Box::new(tonic::Status::invalid_argument(
                        "invalid x-frm-connector metadata value",
                    ))
                })
                .and_then(|connector_from_metadata| {
                    FrmConnectorEnum::from_str(connector_from_metadata).map_err(|err| {
                        Box::new(tonic::Status::invalid_argument(format!(
                            "FRM connector not supported: {err}"
                        )))
                    })
                })
        })
        .transpose()
}

pub fn surcharge_connector_from_composite_surcharge_metadata(
    metadata: &tonic::metadata::MetadataMap,
) -> Result<Option<SurchargeConnectorEnum>, Box<tonic::Status>> {
    metadata
        .get(X_SURCHARGE_CONNECTOR_NAME)
        .map(|connector| {
            connector
                .to_str()
                .map_err(|_| {
                    Box::new(tonic::Status::invalid_argument(
                        "invalid x-surcharge-connector metadata value",
                    ))
                })
                .and_then(|connector_from_metadata| {
                    SurchargeConnectorEnum::from_str(connector_from_metadata).map_err(|err| {
                        Box::new(tonic::Status::invalid_argument(format!(
                            "Surcharge connector not supported: {err}"
                        )))
                    })
                })
        })
        .transpose()
}

pub fn authenticator_connector_from_composite_authorize_metadata(
    metadata: &tonic::metadata::MetadataMap,
) -> Result<Option<AuthenticatorConnectorEnum>, Box<tonic::Status>> {
    metadata
        .get(X_AUTHENTICATOR_CONNECTOR_NAME)
        .map(|connector| {
            connector
                .to_str()
                .map_err(|_| {
                    Box::new(tonic::Status::invalid_argument(
                        "invalid x-auth-connector metadata value",
                    ))
                })
                .and_then(|connector_from_metadata| {
                    AuthenticatorConnectorEnum::from_str(connector_from_metadata).map_err(|err| {
                        Box::new(tonic::Status::invalid_argument(format!(
                            "Authenticator connector not supported: {err}"
                        )))
                    })
                })
        })
        .transpose()
}

/// Resolves the connector variant from composite metadata headers.
/// Priority: x-frm-connector → x-surcharge-connector → x-auth-connector → x-connector (payment).
/// Returns `Err` when a specialised header is present but malformed/unknown.
pub fn connector_variant_from_composite_metadata(
    metadata: &tonic::metadata::MetadataMap,
) -> Result<ConnectorVariant, Box<tonic::Status>> {
    if let Some(connector) = frm_connector_from_composite_frm_metadata(metadata)? {
        return Ok(ConnectorVariant::Frm(connector));
    }

    if let Some(connector) = surcharge_connector_from_composite_surcharge_metadata(metadata)? {
        return Ok(ConnectorVariant::Surcharge(connector));
    }

    if let Some(connector) = authenticator_connector_from_composite_authorize_metadata(metadata)? {
        return Ok(ConnectorVariant::Authenticator(connector));
    }

    connector_from_composite_authorize_metadata(metadata).map(ConnectorVariant::Payment)
}

pub fn grpc_connector_from_connector_variant(connector: &ConnectorVariant) -> i32 {
    let grpc_connector_name = connector.get_connector_name().to_ascii_uppercase();
    let grpc_connector =
        grpc_api_types::payments::Connector::from_str_name(grpc_connector_name.as_str())
            .unwrap_or(grpc_api_types::payments::Connector::Unspecified);
    i32::from(grpc_connector)
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

pub fn access_token_from_create_server_authentication_token_response(
    access_token_response: Option<
        &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    >,
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
    access_token_response: Option<
        &MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    >,
) -> Option<AccessToken> {
    access_token_from_request.or_else(|| {
        access_token_from_create_server_authentication_token_response(access_token_response)
    })
}

pub fn get_session_token(
    session_token_from_request: Option<String>,
    session_token_response: Option<
        &MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    >,
) -> Option<String> {
    session_token_from_request
        .or_else(|| session_token_response.map(|response| response.session_token.clone()))
}

/// Check if payment status indicates a terminal state (success or failure)
pub fn is_terminal_payment_status(status: i32) -> bool {
    matches!(
        PaymentStatus::try_from(status).unwrap_or_default(),
        PaymentStatus::Charged
            | PaymentStatus::Authorized
            | PaymentStatus::PartialCharged
            | PaymentStatus::AuthenticationFailed
            | PaymentStatus::AuthorizationFailed
            | PaymentStatus::Failure
    )
}

/// Check if payment status indicates a failure state
pub fn is_failure_payment_status(status: i32) -> bool {
    matches!(
        PaymentStatus::try_from(status).unwrap_or_default(),
        PaymentStatus::AuthenticationFailed
            | PaymentStatus::AuthorizationFailed
            | PaymentStatus::Failure
    )
}
