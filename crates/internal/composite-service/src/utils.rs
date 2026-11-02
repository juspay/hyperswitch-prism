use std::str::FromStr;

/// Log a composite request at the same service-method boundary used by granular services.
/// Extensions are forwarded unchanged except for the marker identifying nested internal calls.
pub async fn composite_grpc_logging_wrapper<T, F, Fut, R>(
    mut request: tonic::Request<T>,
    service_name: &str,
    flow_name: common_utils::events::FlowName,
    handler: F,
) -> Result<tonic::Response<R>, tonic::Status>
where
    T: serde::Serialize,
    F: FnOnce(tonic::Request<T>) -> Fut,
    Fut: std::future::Future<Output = Result<tonic::Response<R>, tonic::Status>>,
    R: serde::Serialize,
{
    use common_utils::events::record_json_fields_on_span;
    use ucs_interface_common::metadata::{
        connector_variant_from_metadata, merchant_id_from_metadata, request_id_from_metadata,
        tenant_id_from_metadata,
    };

    let span = tracing::Span::current();
    let start = std::time::Instant::now();
    let config = request
        .extensions()
        .get::<std::sync::Arc<ucs_env::configs::Config>>()
        .cloned();
    span.record("service_name", service_name);
    span.record("service_method", flow_name.as_str());
    span.record("flow", flow_name.as_str());
    if let Ok(body) = hyperswitch_masking::masked_serialize(request.get_ref()) {
        record_json_fields_on_span(vec![("request_body", body)]);
    }
    let metadata = request.metadata();
    if let Ok(value) = request_id_from_metadata(metadata) {
        span.record("request_id", value);
    }
    if let Ok(value) = merchant_id_from_metadata(metadata) {
        span.record("merchant_id", value);
    }
    if let Ok(value) = tenant_id_from_metadata(metadata) {
        span.record("tenant_id", value);
    }
    if let Ok(connector) = connector_variant_from_metadata(metadata) {
        span.record("gateway", connector.get_connector_name());
    }
    tracing::info!("Golden Log Line (incoming - request)");

    request
        .extensions_mut()
        .insert(common_utils::request_metrics::InternalCompositeCall);
    let result = handler(request).await;
    let duration = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    span.record("response_time", duration);
    record_json_fields_on_span(vec![("latency_ms", serde_json::json!(duration))]);
    let status_code = match &result {
        Ok(response) => {
            if let Ok(body) = hyperswitch_masking::masked_serialize(response.get_ref()) {
                record_json_fields_on_span(vec![("response_body", body)]);
            }
            200
        }
        Err(status) => {
            span.record("error_message", status.message());
            span.record("status_code", status.code().to_string());
            ucs_env::error::http_status_for_status(status)
        }
    };
    record_json_fields_on_span(vec![("res_code", serde_json::json!(status_code))]);

    // Apply transformations after orchestration so nested logs never inherit INCOMING_API.
    #[cfg(feature = "log-transformations")]
    if let Some(config) = config.filter(|config| config.log_fields.enabled) {
        common_utils::events::apply_log_fields(&config.log_fields.incoming, None);
    }
    #[cfg(not(feature = "log-transformations"))]
    let _ = config;

    tracing::info!("Golden Log Line (incoming - response)");
    result
}

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
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    PaymentMethodServiceTokenizeResponse, PaymentStatus,
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
/// Priority: x-connector (payment) → x-surcharge-connector → x-frm-connector → x-auth-connector.
/// Matches the ordering in ucs_interface_common/src/metadata.rs (connector → surcharge → frm → auth).
/// Payout is omitted because composite never handles payout flows.
/// Returns `Err` when a header is present but malformed/unknown, or when no header is found.
pub fn connector_variant_from_composite_metadata(
    metadata: &tonic::metadata::MetadataMap,
) -> Result<ConnectorVariant, Box<tonic::Status>> {
    if metadata.get(X_CONNECTOR_NAME).is_some() {
        return connector_from_composite_authorize_metadata(metadata)
            .map(ConnectorVariant::Payment);
    }

    if let Some(connector) = surcharge_connector_from_composite_surcharge_metadata(metadata)? {
        return Ok(ConnectorVariant::Surcharge(connector));
    }

    if let Some(connector) = frm_connector_from_composite_frm_metadata(metadata)? {
        return Ok(ConnectorVariant::Frm(connector));
    }

    if let Some(connector) = authenticator_connector_from_composite_authorize_metadata(metadata)? {
        return Ok(ConnectorVariant::Authenticator(connector));
    }

    Err(Box::new(tonic::Status::invalid_argument(
        "missing connector metadata: one of x-connector, x-surcharge-connector, x-frm-connector, or x-auth-connector is required",
    )))
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

pub fn get_payment_method_token(
    payment_method_token_from_request: Option<String>,
    payment_method_tokenize_response: Option<&PaymentMethodServiceTokenizeResponse>,
) -> Option<String> {
    payment_method_token_from_request.or_else(|| {
        payment_method_tokenize_response
            .map(|resp| resp.payment_method_token.clone())
            .filter(|token| !token.is_empty())
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
            | PaymentStatus::Conflicted
    )
}

/// Check if payment status indicates a failure state
pub fn is_failure_payment_status(status: i32) -> bool {
    matches!(
        PaymentStatus::try_from(status).unwrap_or_default(),
        PaymentStatus::AuthenticationFailed
            | PaymentStatus::AuthorizationFailed
            | PaymentStatus::Failure
            | PaymentStatus::Conflicted
    )
}
