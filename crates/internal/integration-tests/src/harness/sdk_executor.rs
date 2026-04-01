use std::collections::HashMap;

use connector_service_ffi::bindings::uniffi as ffi_bindings;
use grpc_api_types::payments::{
    self, ffi_result, ConnectorSpecificConfig, Environment, FfiConnectorHttpRequest,
    FfiConnectorHttpResponse, FfiOptions, FfiResult, ResponseError,
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
        "create_access_token"
            | "create_customer"
            | "authorize"
            | "complete_authorize"
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
    // SDK path loads credentials via the unified connector config loader.
    let config =
        load_connector_config(connector).map_err(|error| ScenarioError::CredentialLoad {
            connector: connector.to_string(),
            message: error.to_string(),
        })?;

    let options = build_ffi_options(connector, &config)?;
    let options_bytes = options.encode_to_vec();

    match suite {
        "create_access_token" => execute_sdk_flow::<
            payments::MerchantAuthenticationServiceCreateAccessTokenRequest,
            payments::MerchantAuthenticationServiceCreateAccessTokenResponse,
        >(
            suite,
            scenario,
            connector,
            grpc_req,
            &options_bytes,
            ffi_bindings::create_access_token_req_transformer,
            ffi_bindings::create_access_token_res_transformer,
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
        "authorize" | "complete_authorize" => execute_sdk_flow::<
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
/// 2. run request transformer (proto -> connector HTTP request)
/// 3. execute HTTP call
/// 4. run response transformer (HTTP response -> proto)
/// 5. serialize proto response to pretty JSON
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

    // Run request transformer — returns encoded FfiResult bytes.
    let req_result_bytes = req_transformer(request_bytes.clone(), options_bytes.to_vec());
    let req_result = FfiResult::decode(req_result_bytes.as_slice()).map_err(|e| {
        ScenarioError::SdkExecution {
            message: format!(
                "sdk request transformer returned invalid FfiResult for '{}/{}': {}",
                suite, scenario, e
            ),
        }
    })?;

    let ffi_http_request = match req_result.payload {
        Some(ffi_result::Payload::HttpRequest(http_request)) => http_request,
        Some(ffi_result::Payload::IntegrationError(ie)) => {
            return Err(ScenarioError::SdkExecution {
                message: format!(
                    "sdk request transformer failed for '{}/{}': {} (code: {})",
                    suite, scenario, ie.error_message, ie.error_code
                ),
            });
        }
        other => {
            return Err(ScenarioError::SdkExecution {
                message: format!(
                    "sdk request transformer returned unexpected FfiResult variant for '{}/{}': {:?}",
                    suite, scenario, other
                ),
            });
        }
    };

    let ffi_http_response = execute_connector_http_request(ffi_http_request, suite, scenario)?;
    let ffi_http_response_bytes = ffi_http_response.encode_to_vec();

    // Run response transformer — also returns encoded FfiResult bytes.
    let res_result_bytes = res_transformer(
        ffi_http_response_bytes,
        request_bytes,
        options_bytes.to_vec(),
    );
    let res_result = FfiResult::decode(res_result_bytes.as_slice()).map_err(|e| {
        ScenarioError::SdkExecution {
            message: format!(
                "sdk response transformer returned invalid FfiResult for '{}/{}': {}",
                suite, scenario, e
            ),
        }
    })?;

    let ffi_http_response_inner = match res_result.payload {
        Some(ffi_result::Payload::HttpResponse(http_response)) => http_response,
        Some(ffi_result::Payload::ConnectorResponseTransformationError(cre)) => {
            return Err(ScenarioError::SdkExecution {
                message: format!(
                    "sdk response transformer failed for '{}/{}': {} (code: {})",
                    suite, scenario, cre.error_message, cre.error_code
                ),
            });
        }
        other => {
            return Err(ScenarioError::SdkExecution {
                message: format!(
                    "sdk response transformer returned unexpected FfiResult variant for '{}/{}': {:?}",
                    suite, scenario, other
                ),
            });
        }
    };

    // The body of the HTTP response payload contains the encoded proto response.
    let proto_response =
        Res::decode(ffi_http_response_inner.body.as_slice()).map_err(|decode_error| {
            if let Ok(response_error) =
                ResponseError::decode(ffi_http_response_inner.body.as_slice())
            {
                return map_response_error("response transformer", suite, scenario, response_error);
            }

            ScenarioError::SdkExecution {
                message: format!(
                    "sdk decode failed for '{}'/'{}' response bytes: {}",
                    suite, scenario, decode_error
                ),
            }
        })?;

    serde_json::to_string_pretty(&proto_response)
        .map_err(|source| ScenarioError::JsonSerialize { source })
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
///
/// Deserializes the `x-connector-config` JSON from [`ConnectorConfig`] directly
/// into the proto [`ConnectorSpecificConfig`] type.  This avoids a separate
/// auth-type → proto-field mapping layer.
fn build_ffi_options(
    connector: &str,
    connector_config: &ConnectorConfig,
) -> Result<FfiOptions, ScenarioError> {
    let proto_config: ConnectorSpecificConfig =
        serde_json::from_str(connector_config.header_value()).map_err(|error| {
            ScenarioError::CredentialLoad {
                connector: connector.to_string(),
                message: format!("failed to deserialize connector config JSON into proto: {error}"),
            }
        })?;

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

// Removed: dead code after FfiResult refactor. The function was previously used
// for error mapping before structured protobuf oneof was introduced.

fn map_response_error(
    stage: &str,
    suite: &str,
    scenario: &str,
    error: ResponseError,
) -> ScenarioError {
    let mut details = Vec::new();
    if let Some(message) = error.error_message.filter(|msg| !msg.is_empty()) {
        details.push(message);
    }
    if let Some(code) = error.error_code.filter(|code| !code.is_empty()) {
        details.push(format!("code={code}"));
    }
    if let Some(status_code) = error.status_code {
        details.push(format!("status_code={status_code}"));
    }

    let detail_text = if details.is_empty() {
        "unknown ffi response error".to_string()
    } else {
        details.join(", ")
    };

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
    use super::{build_ffi_options, parse_sdk_payload, supports_sdk_connector, supports_sdk_suite};
    use crate::harness::credentials::ConnectorConfig;
    use crate::harness::scenario_api::get_the_grpc_req_for_connector;
    use grpc_api_types::payments::connector_specific_config;
    use grpc_api_types::payments::identifier;
    use grpc_api_types::payments::{self, payment_method};

    fn make_config(json: &str) -> ConnectorConfig {
        ConnectorConfig::from_header_json(json.to_string())
    }

    #[test]
    fn sdk_support_matrix_matches_current_scope() {
        assert!(supports_sdk_connector("stripe"));
        assert!(supports_sdk_connector("paypal"));
        assert!(supports_sdk_connector("authorizedotnet"));
        assert!(!supports_sdk_connector("adyen"));

        assert!(supports_sdk_suite("authorize"));
        assert!(supports_sdk_suite("create_access_token"));
        assert!(!supports_sdk_suite("refund_sync"));
    }

    #[test]
    fn stripe_config_json_deserializes_to_proto_shape() {
        let config = make_config(r#"{"config":{"Stripe":{"api_key":"sk_test_123"}}}"#);
        let opts = build_ffi_options("stripe", &config).expect("stripe config should build");
        assert!(matches!(
            opts.connector_config
                .expect("connector_config should be set")
                .config,
            Some(connector_specific_config::Config::Stripe(_))
        ));
    }

    #[test]
    fn paypal_config_json_deserializes_to_proto_shape() {
        let config =
            make_config(r#"{"config":{"Paypal":{"client_id":"cid","client_secret":"csec"}}}"#);
        let opts = build_ffi_options("paypal", &config).expect("paypal config should build");
        assert!(matches!(
            opts.connector_config
                .expect("connector_config should be set")
                .config,
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
