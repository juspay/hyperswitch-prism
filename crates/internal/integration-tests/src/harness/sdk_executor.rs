use std::collections::HashMap;

use connector_service_ffi::bindings::uniffi as ffi_bindings;
use grpc_api_types::payments::{
    self, connector_specific_config, ffi_result, ConnectorResponseTransformationError,
    ConnectorSpecificConfig, Environment, FfiConnectorHttpRequest, FfiConnectorHttpResponse,
    FfiOptions, FfiResult, IntegrationError,
};
use prost::Message;
use reqwest::{blocking::Client, Method};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::harness::{
    credentials::{load_connector_config, ConnectorConfig},
    scenario_api::parse_tonic_payload,
    scenario_types::ScenarioError,
};

type RequestTransformer = fn(Vec<u8>, Vec<u8>) -> Vec<u8>;
type ResponseTransformer = fn(Vec<u8>, Vec<u8>, Vec<u8>) -> Vec<u8>;

/// Returns whether a suite is currently wired for SDK/FFI execution.
pub fn supports_sdk_suite(suite: &str) -> bool {
    matches!(
        suite,
        "server_authentication_token"
            | "server_session_authentication_token"
            | "client_authentication_token"
            | "create_customer"
            | "authorize"
            | "capture"
            | "void"
            | "refund"
            | "get"
            | "setup_recurring"
            | "recurring_charge"
    )
}

/// Returns whether a connector has SDK transformer/auth support in this harness.
pub fn supports_sdk_connector(connector: &str) -> bool {
    matches!(connector, "stripe" | "authorizedotnet" | "paypal")
}

/// Executes one scenario via SDK FFI request/response transformers.
pub fn execute_sdk_request_from_payload(
    suite: &str,
    scenario: &str,
    grpc_req: &Value,
    connector: &str,
) -> Result<String, ScenarioError> {
    // SDK path still uses the same credential loader as grpcurl/tonic paths.
    let config =
        load_connector_config(connector).map_err(|error| ScenarioError::CredentialLoad {
            connector: connector.to_string(),
            message: error.to_string(),
        })?;

    let options = build_ffi_options(connector, &config)?;
    let options_bytes = options.encode_to_vec();

    match suite {
        "server_authentication_token" => execute_sdk_flow::<
            payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
            payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::create_server_authentication_token_req_transformer,
            ffi_bindings::create_server_authentication_token_res_transformer,
        ),
        "server_session_authentication_token" => execute_sdk_flow::<
            payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
            payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::create_server_session_authentication_token_req_transformer,
            ffi_bindings::create_server_session_authentication_token_res_transformer,
        ),
        "client_authentication_token" => execute_sdk_flow::<
            payments::MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
            payments::MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::create_client_authentication_token_req_transformer,
            ffi_bindings::create_client_authentication_token_res_transformer,
        ),
        "create_customer" => execute_sdk_flow::<
            payments::CustomerServiceCreateRequest,
            payments::CustomerServiceCreateResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::create_req_transformer,
            ffi_bindings::create_res_transformer,
        ),
        "authorize" => execute_sdk_flow::<
            payments::PaymentServiceAuthorizeRequest,
            payments::PaymentServiceAuthorizeResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::authorize_req_transformer,
            ffi_bindings::authorize_res_transformer,
        ),
        "capture" => execute_sdk_flow::<
            payments::PaymentServiceCaptureRequest,
            payments::PaymentServiceCaptureResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::capture_req_transformer,
            ffi_bindings::capture_res_transformer,
        ),
        "void" => execute_sdk_flow::<
            payments::PaymentServiceVoidRequest,
            payments::PaymentServiceVoidResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::void_req_transformer,
            ffi_bindings::void_res_transformer,
        ),
        "refund" => {
            execute_sdk_flow::<payments::PaymentServiceRefundRequest, payments::RefundResponse>(
                suite,
                scenario,
                connector,
                grpc_req,
                &options_bytes,
                ffi_bindings::refund_req_transformer,
                ffi_bindings::refund_res_transformer,
            )
        }
        "get" => execute_sdk_flow::<
            payments::PaymentServiceGetRequest,
            payments::PaymentServiceGetResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::get_req_transformer,
            ffi_bindings::get_res_transformer,
        ),
        "setup_recurring" => execute_sdk_flow::<
            payments::PaymentServiceSetupRecurringRequest,
            payments::PaymentServiceSetupRecurringResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::setup_recurring_req_transformer,
            ffi_bindings::setup_recurring_res_transformer,
        ),
        "recurring_charge" => execute_sdk_flow::<
            payments::RecurringPaymentServiceChargeRequest,
            payments::RecurringPaymentServiceChargeResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::charge_req_transformer,
            ffi_bindings::charge_res_transformer,
        ),
        _ => Err(ScenarioError::UnsupportedSuite {
            suite: suite.to_string(),
        }),
    }
}

/// Generic SDK execution pipeline:
/// 1. parse JSON payload into protobuf request
/// 2. run request transformer (proto -> FfiResult wrapping FfiConnectorHttpRequest)
/// 3. unwrap FfiResult to get FfiConnectorHttpRequest
/// 4. execute HTTP call
/// 5. run response transformer (FfiConnectorHttpResponse -> FfiResult wrapping proto response in body)
/// 6. unwrap FfiResult to decode the proto response from body bytes
/// 7. serialize proto response to pretty JSON
fn execute_sdk_flow<Req, Res>(
    suite: &str,
    scenario: &str,
    connector: &str,
    grpc_req: &Value,
    options_bytes: &[u8],
    req_transformer: RequestTransformer,
    res_transformer: ResponseTransformer,
) -> Result<String, ScenarioError>
where
    Req: Message + Default + DeserializeOwned,
    Res: Message + Default + Serialize,
{
    let request_payload: Req = parse_sdk_payload(suite, scenario, connector, grpc_req)?;
    let request_bytes = request_payload.encode_to_vec();

    // The req transformer always returns FfiResult bytes.
    let req_ffi_result_bytes = req_transformer(request_bytes.clone(), options_bytes.to_vec());

    let ffi_http_request =
        decode_ffi_result_as_http_request(suite, scenario, req_ffi_result_bytes)?;

    let ffi_http_response = execute_connector_http_request(ffi_http_request, suite, scenario)?;
    let ffi_http_response_bytes = ffi_http_response.encode_to_vec();

    // The res transformer always returns FfiResult bytes.
    let res_ffi_result_bytes = res_transformer(
        ffi_http_response_bytes,
        request_bytes,
        options_bytes.to_vec(),
    );

    let proto_response: Res =
        decode_ffi_result_as_proto_response(suite, scenario, res_ffi_result_bytes)?;

    serde_json::to_string_pretty(&proto_response)
        .map_err(|source| ScenarioError::JsonSerialize { source })
}

/// Decodes a `FfiResult` buffer returned by a request transformer and extracts
/// the inner `FfiConnectorHttpRequest`.  Returns a `ScenarioError` on any
/// decode failure or if the result type signals an error.
fn decode_ffi_result_as_http_request(
    suite: &str,
    scenario: &str,
    bytes: Vec<u8>,
) -> Result<FfiConnectorHttpRequest, ScenarioError> {
    let ffi_result = FfiResult::decode(bytes.as_slice()).map_err(|decode_error| {
        ScenarioError::SdkExecution {
            message: format!(
                "sdk req transformer returned undecodable bytes for '{}/{}': {}",
                suite, scenario, decode_error
            ),
        }
    })?;

    match ffi_result.payload {
        Some(ffi_result::Payload::HttpRequest(http_request)) => Ok(http_request),
        Some(ffi_result::Payload::IntegrationError(e)) => Err(map_integration_error(
            "request transformer",
            suite,
            scenario,
            e,
        )),
        Some(ffi_result::Payload::ConnectorResponseTransformationError(e)) => Err(
            map_response_transformation_error("request transformer", suite, scenario, e),
        ),
        other => Err(ScenarioError::SdkExecution {
            message: format!(
                "sdk req transformer returned unexpected FfiResult payload for '{}/{}': {:?}",
                suite, scenario, other
            ),
        }),
    }
}

/// Decodes a `FfiResult` buffer returned by a response transformer and extracts
/// the proto response from `FfiConnectorHttpResponse.body`.
fn decode_ffi_result_as_proto_response<Res: Message + Default>(
    suite: &str,
    scenario: &str,
    bytes: Vec<u8>,
) -> Result<Res, ScenarioError> {
    let ffi_result = FfiResult::decode(bytes.as_slice()).map_err(|decode_error| {
        ScenarioError::SdkExecution {
            message: format!(
                "sdk res transformer returned undecodable bytes for '{}/{}': {}",
                suite, scenario, decode_error
            ),
        }
    })?;

    match ffi_result.payload {
        Some(ffi_result::Payload::HttpResponse(http_response)) => {
            // The res transformer encodes the proto response into body.
            Res::decode(http_response.body.as_slice()).map_err(|decode_error| {
                ScenarioError::SdkExecution {
                    message: format!(
                        "sdk proto response decode failed for '{}/{}': {}",
                        suite, scenario, decode_error
                    ),
                }
            })
        }
        Some(ffi_result::Payload::ConnectorResponseTransformationError(e)) => Err(
            map_response_transformation_error("response transformer", suite, scenario, e),
        ),
        Some(ffi_result::Payload::IntegrationError(e)) => Err(map_integration_error(
            "response transformer",
            suite,
            scenario,
            e,
        )),
        other => Err(ScenarioError::SdkExecution {
            message: format!(
                "sdk res transformer returned unexpected FfiResult payload for '{}/{}': {:?}",
                suite, scenario, other
            ),
        }),
    }
}

/// Performs the raw HTTP call described by FFI transformed request.
fn execute_connector_http_request(
    request: FfiConnectorHttpRequest,
    suite: &str,
    scenario: &str,
) -> Result<FfiConnectorHttpResponse, ScenarioError> {
    let method = Method::from_bytes(request.method.as_bytes()).map_err(|error| {
        ScenarioError::SdkExecution {
            message: format!(
                "sdk invalid HTTP method for '{}'/'{}': {}",
                suite, scenario, error
            ),
        }
    })?;

    let client = Client::builder()
        .build()
        .map_err(|error| ScenarioError::SdkExecution {
            message: format!(
                "sdk HTTP client initialization failed for '{}'/'{}': {}",
                suite, scenario, error
            ),
        })?;

    let mut builder = client.request(method, &request.url);
    // Preserve connector headers exactly as produced by the transformer.
    for (key, value) in &request.headers {
        builder = builder.header(key, value);
    }

    if let Some(body) = request.body {
        builder = builder.body(body);
    }

    let response = builder
        .send()
        .map_err(|error| ScenarioError::SdkExecution {
            message: format!(
                "sdk HTTP request failed for '{}'/'{}': {}",
                suite, scenario, error
            ),
        })?;

    let status_code = u32::from(response.status().as_u16());
    let mut headers = HashMap::new();
    for (name, value) in response.headers() {
        if let Ok(value) = value.to_str() {
            headers.insert(name.to_string(), value.to_string());
        }
    }

    let body = response
        .bytes()
        .map_err(|error| ScenarioError::SdkExecution {
            message: format!(
                "sdk HTTP response read failed for '{}'/'{}': {}",
                suite, scenario, error
            ),
        })?
        .to_vec();

    Ok(FfiConnectorHttpResponse {
        status_code,
        headers,
        body,
    })
}

/// Parses scenario JSON payload into a strongly typed protobuf request.
fn parse_sdk_payload<T: DeserializeOwned>(
    suite: &str,
    scenario: &str,
    connector: &str,
    grpc_req: &Value,
) -> Result<T, ScenarioError> {
    parse_tonic_payload(suite, scenario, connector, grpc_req).map_err(convert_sdk_error_label)
}

/// Builds FFI options bundle used by all request/response transformers.
fn build_ffi_options(
    connector: &str,
    connector_config: &ConnectorConfig,
) -> Result<FfiOptions, ScenarioError> {
    let proto_config = build_proto_connector_config(connector, connector_config)?;

    Ok(FfiOptions {
        environment: environment_discriminant(ffi_environment()),
        connector_config: Some(proto_config),
    })
}

fn environment_discriminant(environment: Environment) -> i32 {
    match environment {
        Environment::Unspecified => 0,
        Environment::Sandbox => 1,
        Environment::Production => 2,
    }
}

/// Converts harness credential shape into connector-specific protobuf config oneof.
///
/// `connector_config.header_value()` returns the fully-normalised JSON that is
/// sent as the `x-connector-config` gRPC header.  Its shape is:
/// ```json
/// {"config":{"Stripe":{"api_key":"sk_test_..."}}}
/// ```
/// The `{"value":"..."}` wrappers are already unwrapped by
/// `credentials::load_connector_config`, so we must NOT try to read `.value`.
fn build_proto_connector_config(
    connector: &str,
    connector_config: &ConnectorConfig,
) -> Result<ConnectorSpecificConfig, ScenarioError> {
    // header_value() is {"config":{"<PascalConnector>":{...flat auth fields...}}}
    let header_json: Value =
        serde_json::from_str(connector_config.header_value()).map_err(|e| {
            ScenarioError::SdkExecution {
                message: format!("Failed to parse connector config JSON: {}", e),
            }
        })?;

    // Navigate to the connector-specific auth object.
    // pascal_name mirrors credentials::pascal_connector_name.
    let pascal_name: String = {
        let mut chars = connector.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    };
    let auth = &header_json["config"][&pascal_name];

    match connector {
        "stripe" => {
            let api_key = auth["api_key"]
                .as_str()
                .ok_or_else(|| ScenarioError::SdkExecution {
                    message: "Missing api_key in stripe config".to_string(),
                })?;
            Ok(ConnectorSpecificConfig {
                config: Some(connector_specific_config::Config::Stripe(
                    payments::StripeConfig {
                        api_key: Some(api_key.to_string().into()),
                        base_url: None,
                    },
                )),
            })
        }
        "authorizedotnet" => {
            let name = auth["api_key"]
                .as_str()
                .ok_or_else(|| ScenarioError::SdkExecution {
                    message: "Missing api_key in authorizedotnet config".to_string(),
                })?;
            let transaction_key =
                auth["key1"]
                    .as_str()
                    .ok_or_else(|| ScenarioError::SdkExecution {
                        message: "Missing key1 in authorizedotnet config".to_string(),
                    })?;
            Ok(ConnectorSpecificConfig {
                config: Some(connector_specific_config::Config::Authorizedotnet(
                    payments::AuthorizedotnetConfig {
                        name: Some(name.to_string().into()),
                        transaction_key: Some(transaction_key.to_string().into()),
                        base_url: None,
                    },
                )),
            })
        }
        "paypal" => {
            // Support both field-name conventions (key1/client_id, api_key/client_secret, etc.)
            let client_id = auth["key1"]
                .as_str()
                .or_else(|| auth["client_id"].as_str())
                .ok_or_else(|| ScenarioError::SdkExecution {
                    message: "Missing key1/client_id in paypal config".to_string(),
                })?;
            let client_secret = auth["api_key"]
                .as_str()
                .or_else(|| auth["client_secret"].as_str())
                .ok_or_else(|| ScenarioError::SdkExecution {
                    message: "Missing api_key/client_secret in paypal config".to_string(),
                })?;
            let payer_id = auth["api_secret"]
                .as_str()
                .or_else(|| auth["payer_id"].as_str())
                .map(|s| s.to_string().into());
            Ok(ConnectorSpecificConfig {
                config: Some(connector_specific_config::Config::Paypal(
                    payments::PaypalConfig {
                        client_id: Some(client_id.to_string().into()),
                        client_secret: Some(client_secret.to_string().into()),
                        payer_id,
                        base_url: None,
                    },
                )),
            })
        }
        _ => Err(ScenarioError::CredentialLoad {
            connector: connector.to_string(),
            message: "unsupported connector auth shape for SDK harness".to_string(),
        }),
    }
}

/// SDK environment selector (defaults to sandbox for safety).
fn ffi_environment() -> Environment {
    let env = std::env::var("UCS_SDK_ENVIRONMENT")
        .unwrap_or_default()
        .to_ascii_lowercase();

    if env == "production" || env == "prod" {
        Environment::Production
    } else {
        Environment::Sandbox
    }
}

fn map_integration_error(
    stage: &str,
    suite: &str,
    scenario: &str,
    error: IntegrationError,
) -> ScenarioError {
    let mut details = Vec::new();
    details.push(error.error_message);
    details.push(format!("code={}", error.error_code));

    if let Some(suggested_action) = error.suggested_action.filter(|msg| !msg.is_empty()) {
        details.push(format!("suggested_action={}", suggested_action));
    }

    let detail_text = details.join(", ");

    ScenarioError::SdkExecution {
        message: format!(
            "sdk {} failed for '{}/{}': {}",
            stage, suite, scenario, detail_text
        ),
    }
}

fn map_response_transformation_error(
    stage: &str,
    suite: &str,
    scenario: &str,
    error: ConnectorResponseTransformationError,
) -> ScenarioError {
    let mut details = Vec::new();
    details.push(error.error_message);
    details.push(format!("code={}", error.error_code));

    if let Some(http_status_code) = error.http_status_code {
        details.push(format!("http_status_code={}", http_status_code));
    }

    let detail_text = details.join(", ");

    ScenarioError::SdkExecution {
        message: format!(
            "sdk {} failed for '{}/{}': {}",
            stage, suite, scenario, detail_text
        ),
    }
}

/// Re-labels generic execution errors into SDK-specific error variant.
fn convert_sdk_error_label(error: ScenarioError) -> ScenarioError {
    match error {
        ScenarioError::GrpcurlExecution { message } => ScenarioError::SdkExecution { message },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_proto_connector_config, parse_sdk_payload, supports_sdk_connector, supports_sdk_suite,
    };
    use crate::harness::credentials::ConnectorConfig;
    use crate::harness::scenario_api::get_the_grpc_req_for_connector;
    use grpc_api_types::payments::connector_specific_config;
    use grpc_api_types::payments::identifier;
    use grpc_api_types::payments::{self, payment_method};

    #[test]
    fn sdk_support_matrix_matches_current_scope() {
        assert!(supports_sdk_connector("stripe"));
        assert!(supports_sdk_connector("paypal"));
        assert!(supports_sdk_connector("authorizedotnet"));
        assert!(!supports_sdk_connector("adyen"));

        assert!(supports_sdk_suite("authorize"));
        assert!(supports_sdk_suite("server_authentication_token"));
        assert!(!supports_sdk_suite("refund_sync"));
    }

    #[test]
    fn stripe_auth_maps_to_proto_shape() {
        let config = ConnectorConfig::from_header_json(
            r#"{"config":{"Stripe":{"api_key":"sk_test_123"}}}"#.to_string(),
        );
        let proto =
            build_proto_connector_config("stripe", &config).expect("stripe auth should map");
        assert!(matches!(
            proto.config,
            Some(connector_specific_config::Config::Stripe(_))
        ));
    }

    #[test]
    fn paypal_auth_accepts_body_and_signature_shapes() {
        let body_config = ConnectorConfig::from_header_json(
            r#"{"config":{"Paypal":{"key1":"client_id","api_key":"client_secret"}}}"#.to_string(),
        );
        let body_proto = build_proto_connector_config("paypal", &body_config)
            .expect("paypal body auth should map");
        assert!(matches!(
            body_proto.config,
            Some(connector_specific_config::Config::Paypal(_))
        ));

        let sig_config = ConnectorConfig::from_header_json(
            r#"{"config":{"Paypal":{"key1":"client_id","api_key":"client_secret","api_secret":"payer_id"}}}"#.to_string(),
        );
        let sig_proto = build_proto_connector_config("paypal", &sig_config)
            .expect("paypal signature auth should map");
        assert!(matches!(
            sig_proto.config,
            Some(connector_specific_config::Config::Paypal(_))
        ));
    }

    #[test]
    fn authorize_scenario_maps_to_card_payment_method() {
        let req = get_the_grpc_req_for_connector(
            "authorize",
            "no3ds_auto_capture_credit_card",
            "authorizedotnet",
        )
        .expect("authorize scenario should load");

        let parsed: payments::PaymentServiceAuthorizeRequest = parse_sdk_payload(
            "authorize",
            "no3ds_auto_capture_credit_card",
            "authorizedotnet",
            &req,
        )
        .expect("sdk payload parse should succeed");

        let payment_method = parsed
            .payment_method
            .expect("payment_method should be present after parsing");
        assert!(
            matches!(
                payment_method.payment_method,
                Some(payment_method::PaymentMethod::Card(_))
            ),
            "unexpected payment_method variant: {:?}",
            payment_method.payment_method
        );
    }

    #[test]
    fn serde_shapes_for_oneof_wrappers_are_nested() {
        let payment_method = payments::PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(
                payments::CardDetails::default(),
            )),
        };
        let payment_method_json =
            serde_json::to_value(payment_method).expect("payment method should serialize");
        assert!(payment_method_json.get("payment_method").is_some());

        let identifier = payments::Identifier {
            id_type: Some(identifier::IdType::Id("id_123".to_string())),
        };
        let identifier_json =
            serde_json::to_value(identifier).expect("identifier should serialize");
        assert!(identifier_json.get("id_type").is_some());
    }
}
