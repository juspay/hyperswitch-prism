#![allow(unused_variables, unused_assignments)]

use crate::router_data::ErrorResponse;
use common_enums;
use common_utils::errors::ErrorSwitch;
use error_stack::Report;
// use api_models::errors::types::{ Extra};
#[derive(Debug, thiserror::Error, PartialEq, Clone, strum::AsRefStr)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiClientError {
    #[error("Header map construction failed")]
    HeaderMapConstructionFailed,
    #[error("Invalid proxy configuration")]
    InvalidProxyConfiguration,
    #[error("Client construction failed")]
    ClientConstructionFailed,
    #[error("Certificate decode failed")]
    CertificateDecodeFailed,
    #[error("Request body serialization failed")]
    BodySerializationFailed,
    #[error("Unexpected state reached/Invariants conflicted")]
    UnexpectedState,
    #[error("Url Parsing Failed")]
    UrlParsingFailed,
    #[error("URL encoding of request payload failed")]
    UrlEncodingFailed,
    #[error("Failed to send request to connector {0}")]
    RequestNotSent(String),
    #[error("Failed to decode response")]
    ResponseDecodingFailed,
    #[error("Server responded with Request Timeout")]
    RequestTimeoutReceived,
    #[error("connection closed before a message could complete")]
    ConnectionClosedIncompleteMessage,
    #[error("Server responded with Internal Server Error")]
    InternalServerErrorReceived,
    #[error("Server responded with Bad Gateway")]
    BadGatewayReceived,
    #[error("Server responded with Service Unavailable")]
    ServiceUnavailableReceived,
    #[error("Server responded with Gateway Timeout")]
    GatewayTimeoutReceived,
    #[error("Server responded with unexpected response")]
    UnexpectedServerResponse,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ApiError {
    pub sub_code: String,
    pub error_identifier: u16,
    pub error_message: String,
    pub error_object: Option<serde_json::Value>,
}

impl ApiError {
    pub fn missing_amount(message: impl Into<String>) -> Self {
        Self {
            sub_code: "MISSING_AMOUNT".to_owned(),
            error_identifier: 400,
            error_message: message.into(),
            error_object: None,
        }
    }
}

/// Fields used when mapping request-phase connector errors to gRPC `IntegrationError`.
/// Does not depend on generated proto types.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntegrationErrorContext {
    /// Human-readable remediation (maps to `IntegrationError.suggested_action`).
    pub suggested_action: Option<String>,
    /// Optional documentation URL (maps to `IntegrationError.doc_url`).
    pub doc_url: Option<String>,
    /// Connector- or flow-specific detail; **appended** to the base error message when building
    /// `IntegrationError.error_message` — see [`combine_error_message_with_context`].
    pub additional_context: Option<String>,
}

/// Fields used when mapping response-phase connector errors to
/// `ConnectorError`.
///
/// For rare cases (e.g. HTTP status unknown **and** [`Self::additional_context`] set), build
/// [`ConnectorError`] with a struct literal instead of adding more constructor helpers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseTransformationErrorContext {
    /// HTTP status from the connector response when known.
    pub http_status_code: Option<u16>,
    /// Connector-specific detail; **appended** to the base error message for
    /// `ConnectorError.error_message` — see [`combine_error_message_with_context`].
    pub additional_context: Option<String>,
}

/// Combines the base error string with optional extra context for gRPC `error_message`.
///
/// **Rule:** If `additional_context` is `Some` and non-empty after trim, returns
/// `"{trimmed_base}. {trimmed_context}"`. Otherwise returns `trimmed_base` only.
pub fn combine_error_message_with_context(
    base_message: impl AsRef<str>,
    additional_context: Option<&str>,
) -> String {
    let base = base_message.as_ref().trim_end();
    match additional_context.map(str::trim).filter(|s| !s.is_empty()) {
        None => base.to_string(),
        Some(ctx) => format!("{base}. {ctx}"),
    }
}

/// Errors that occur on the request transformationside:
/// - proto → domain (`ForeignTryFrom`)
/// - domain → connector bytes (`build_request_v2`)
/// - request building variants from `ApiClientError` (`HeaderMapConstruction`, etc.)
#[derive(Debug, thiserror::Error, PartialEq, Clone, strum::AsRefStr)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum IntegrationError {
    #[error("Error while obtaining URL for the integration")]
    FailedToObtainIntegrationUrl { context: IntegrationErrorContext },
    #[error("Failed to encode connector request")]
    RequestEncodingFailed { context: IntegrationErrorContext },
    #[error("Header map construction failed")]
    HeaderMapConstructionFailed { context: IntegrationErrorContext },
    #[error("Request body serialization failed")]
    BodySerializationFailed { context: IntegrationErrorContext },
    #[error("Url parsing failed")]
    UrlParsingFailed { context: IntegrationErrorContext },
    #[error("URL encoding of request payload failed")]
    UrlEncodingFailed { context: IntegrationErrorContext },
    #[error("Missing required field: {field_name}")]
    MissingRequiredField {
        field_name: &'static str,
        context: IntegrationErrorContext,
    },
    #[error("Missing required fields: {field_names:?}")]
    MissingRequiredFields {
        field_names: Vec<&'static str>,
        context: IntegrationErrorContext,
    },
    #[error("Failed to obtain authentication type")]
    FailedToObtainAuthType { context: IntegrationErrorContext },
    #[error("Invalid connector configuration: {config}")]
    InvalidConnectorConfig {
        config: &'static str,
        context: IntegrationErrorContext,
    },
    #[error("Connector metadata not found")]
    NoConnectorMetaData { context: IntegrationErrorContext },
    #[error("Invalid data format: {field_name}")]
    InvalidDataFormat {
        field_name: &'static str,
        context: IntegrationErrorContext,
    },
    #[error("An invalid wallet was used")]
    InvalidWallet { context: IntegrationErrorContext },
    #[error("Failed to parse {wallet_name} wallet token")]
    InvalidWalletToken {
        wallet_name: String,
        context: IntegrationErrorContext,
    },
    #[error("Payment Method Type not found")]
    MissingPaymentMethodType { context: IntegrationErrorContext },
    #[error("Payment method data / type / experience mismatch")]
    MismatchedPaymentData { context: IntegrationErrorContext },
    #[error("Field {fields} doesn't match with the ones used during mandate creation")]
    MandatePaymentDataMismatch {
        fields: String,
        context: IntegrationErrorContext,
    },
    #[error("Missing apple pay tokenization data")]
    MissingApplePayTokenData { context: IntegrationErrorContext },
    #[error("This feature is not implemented: {0}")]
    NotImplemented(String, IntegrationErrorContext),
    #[error("{message} is not supported by {connector}")]
    NotSupported {
        message: String,
        connector: &'static str,
        context: IntegrationErrorContext,
    },
    #[error("{flow} flow not supported by {connector} connector")]
    FlowNotSupported {
        flow: String,
        connector: String,
        context: IntegrationErrorContext,
    },
    #[error("Capture method not supported")]
    CaptureMethodNotSupported { context: IntegrationErrorContext },
    #[error("The given currency is not configured with the given connector")]
    CurrencyNotSupported {
        message: String,
        connector: &'static str,
        context: IntegrationErrorContext,
    },
    #[error("Failed to convert amount to required type")]
    AmountConversionFailed { context: IntegrationErrorContext },
    #[error("Missing connector transaction ID")]
    MissingConnectorTransactionID { context: IntegrationErrorContext },
    #[error("Missing connector refund ID")]
    MissingConnectorRefundID { context: IntegrationErrorContext },
    #[error("Missing connector mandate ID")]
    MissingConnectorMandateID { context: IntegrationErrorContext },
    #[error("Missing connector mandate metadata")]
    MissingConnectorMandateMetadata { context: IntegrationErrorContext },
    #[error("Missing connector related transaction ID: {id}")]
    MissingConnectorRelatedTransactionID {
        id: String,
        context: IntegrationErrorContext,
    },
    #[error("Field '{field_name}' is too long for connector '{connector}'")]
    MaxFieldLengthViolated {
        connector: String,
        field_name: String,
        max_length: usize,
        received_length: usize,
        context: IntegrationErrorContext,
    },
    #[error("Failed to verify request source (signature, webhook, etc.)")]
    SourceVerificationFailed { context: IntegrationErrorContext },
    /// Config/auth/metadata validation failures (e.g. invalid config override, missing header).
    #[error("{message}")]
    ConfigurationError {
        code: String,
        message: String,
        context: IntegrationErrorContext,
    },
}

impl IntegrationError {
    /// Create a configuration/auth/metadata error with a standardized code.
    pub fn config_error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::config_error_with_context(code, message, IntegrationErrorContext::default())
    }

    /// Like [`Self::config_error`], but allows connector-specific [`IntegrationErrorContext`]
    /// (merged with central defaults in `ucs_env`).
    pub fn config_error_with_context(
        code: impl Into<String>,
        message: impl Into<String>,
        context: IntegrationErrorContext,
    ) -> Self {
        Self::ConfigurationError {
            code: code.into(),
            message: message.into(),
            context,
        }
    }

    /// Connector feature not implemented; uses default empty [`IntegrationErrorContext`].
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::not_implemented_with_context(message, IntegrationErrorContext::default())
    }

    /// Like [`Self::not_implemented`], but allows connector-specific [`IntegrationErrorContext`]
    /// (merged with central defaults in `ucs_env`).
    pub fn not_implemented_with_context(
        message: impl Into<String>,
        context: IntegrationErrorContext,
    ) -> Self {
        Self::NotImplemented(message.into(), context)
    }

    /// Optional connector-specific guidance for gRPC [`IntegrationError`] (overrides merged in `ucs_env`).
    pub fn integration_context(&self) -> &IntegrationErrorContext {
        match self {
            Self::FailedToObtainIntegrationUrl { context }
            | Self::RequestEncodingFailed { context }
            | Self::HeaderMapConstructionFailed { context }
            | Self::BodySerializationFailed { context }
            | Self::UrlParsingFailed { context }
            | Self::UrlEncodingFailed { context }
            | Self::MissingRequiredField { context, .. }
            | Self::MissingRequiredFields { context, .. }
            | Self::FailedToObtainAuthType { context }
            | Self::InvalidConnectorConfig { context, .. }
            | Self::NoConnectorMetaData { context }
            | Self::InvalidDataFormat { context, .. }
            | Self::InvalidWallet { context }
            | Self::InvalidWalletToken { context, .. }
            | Self::MissingPaymentMethodType { context }
            | Self::MismatchedPaymentData { context }
            | Self::MandatePaymentDataMismatch { context, .. }
            | Self::MissingApplePayTokenData { context }
            | Self::NotImplemented(_, context)
            | Self::NotSupported { context, .. }
            | Self::FlowNotSupported { context, .. }
            | Self::CaptureMethodNotSupported { context }
            | Self::CurrencyNotSupported { context, .. }
            | Self::AmountConversionFailed { context }
            | Self::MissingConnectorTransactionID { context }
            | Self::MissingConnectorRefundID { context }
            | Self::MissingConnectorMandateID { context }
            | Self::MissingConnectorMandateMetadata { context }
            | Self::MissingConnectorRelatedTransactionID { context, .. }
            | Self::MaxFieldLengthViolated { context, .. }
            | Self::SourceVerificationFailed { context }
            | Self::ConfigurationError { context, .. } => context,
        }
    }

    /// Machine-readable error code (SCREAMING_SNAKE_CASE from variant name, or explicit `code` for ConfigurationError).
    pub fn error_code(&self) -> &str {
        match self {
            Self::ConfigurationError { code, .. } => code,
            _ => self.as_ref(),
        }
    }
}

/// Direct conversion from domain IntegrationError to proto IntegrationError (lossless).
impl ErrorSwitch<grpc_api_types::payments::IntegrationError> for IntegrationError {
    fn switch(&self) -> grpc_api_types::payments::IntegrationError {
        let context = self.integration_context();
        let base_message = self.to_string();
        let error_message = combine_error_message_with_context(
            &base_message,
            context.additional_context.as_deref(),
        );

        grpc_api_types::payments::IntegrationError {
            error_message,
            error_code: self.error_code().to_string(),
            suggested_action: context.suggested_action.clone(),
            doc_url: doc_url_for_error_code(self.error_code()),
        }
    }
}

/// Errors that occur on the response side of a connector call:
/// - UCS-side: connector bytes → domain (`handle_response_v2`), domain → proto (`generate_payment_*_response`)
/// - Connector-side: connector returned a 4xx/5xx HTTP error response (parsed by `get_error_response_v2` / `get_5xx_error_response`)
#[derive(Debug, thiserror::Error, Clone, strum::AsRefStr)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorError {
    #[error("Failed to deserialize connector response")]
    ResponseDeserializationFailed {
        /// Always present: set `http_status_code` to `Some` when the connector HTTP response is known.
        context: ResponseTransformationErrorContext,
    },
    #[error("Failed to handle connector response")]
    ResponseHandlingFailed {
        context: ResponseTransformationErrorContext,
    },
    #[error("The connector returned an unexpected response")]
    UnexpectedResponseError {
        context: ResponseTransformationErrorContext,
    },
    #[error("Integrity check failed for fields: {field_names}")]
    IntegrityCheckFailed {
        context: ResponseTransformationErrorContext,
        field_names: String,
        connector_transaction_id: Option<String>,
    },
    /// Connector returned a 4xx or 5xx HTTP error response.
    /// The `ErrorResponse` is fully parsed by the connector's own `get_error_response_v2` /
    /// `get_5xx_error_response` / `build_error_response` implementation.
    /// `error_response.status_code` carries the actual HTTP status (4xx or 5xx).
    #[error("Connector returned an error response with status {}", _0.status_code)]
    ConnectorErrorResponse(ErrorResponse),
}

/// Returns documentation URL for error codes.
/// Points to the comprehensive error code reference page.
pub fn doc_url_for_error_code(_error_code: &str) -> Option<String> {
    Some("https://docs.hyperswitch.io/prism/architecture/concepts/error-codes".to_string())
}

impl ConnectorError {
    /// Machine-readable error code (SCREAMING_SNAKE_CASE from variant name via `strum::AsRefStr`).
    pub fn error_code(&self) -> &str {
        self.as_ref()
    }

    /// HTTP status code from the connector response (`None` when not applicable).
    pub fn http_status_code(&self) -> Option<u16> {
        match self {
            Self::ResponseDeserializationFailed { context }
            | Self::ResponseHandlingFailed { context }
            | Self::UnexpectedResponseError { context }
            | Self::IntegrityCheckFailed { context, .. } => context.http_status_code,
            Self::ConnectorErrorResponse(error_response) => Some(error_response.status_code),
        }
    }

    /// Optional connector-specific detail (appended to proto `error_message`).
    pub fn additional_context(&self) -> Option<&str> {
        match self {
            Self::ResponseDeserializationFailed { context }
            | Self::ResponseHandlingFailed { context }
            | Self::UnexpectedResponseError { context }
            | Self::IntegrityCheckFailed { context, .. } => context.additional_context.as_deref(),
            Self::ConnectorErrorResponse(error_response) => error_response.reason.as_deref(),
        }
    }

    /// Build a [`ResponseTransformationErrorContext`] for gRPC mapping.
    /// For `ConnectorErrorResponse`, synthesises a context from the parsed `ErrorResponse`.
    pub fn response_transformation_context(&self) -> ResponseTransformationErrorContext {
        match self {
            Self::ResponseDeserializationFailed { context }
            | Self::ResponseHandlingFailed { context }
            | Self::UnexpectedResponseError { context }
            | Self::IntegrityCheckFailed { context, .. } => context.clone(),
            Self::ConnectorErrorResponse(error_response) => ResponseTransformationErrorContext {
                http_status_code: Some(error_response.status_code),
                additional_context: error_response.reason.clone(),
            },
        }
    }

    /// Create ResponseHandlingFailed with the connector HTTP status from [`Response::status_code`].
    pub fn response_handling_failed(http_status: u16) -> Self {
        Self::ResponseHandlingFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(http_status),
                additional_context: None,
            },
        }
    }

    /// Use only when there is **no** HTTP response (e.g. base64 decode); prefer
    /// [`Self::response_handling_failed`] with a real status from [`router_response_types::Response`].
    pub fn response_handling_failed_http_status_unknown() -> Self {
        Self::ResponseHandlingFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: None,
            },
        }
    }

    /// [`Self::response_handling_failed`] plus optional appended context for proto.
    pub fn response_handling_failed_with_context(
        http_status: u16,
        additional_context: Option<String>,
    ) -> Self {
        Self::ResponseHandlingFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(http_status),
                additional_context,
            },
        }
    }

    pub fn response_deserialization_failed(http_status: u16) -> Self {
        Self::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(http_status),
                additional_context: None,
            },
        }
    }

    pub fn response_deserialization_failed_http_status_unknown() -> Self {
        Self::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: None,
            },
        }
    }

    pub fn response_deserialization_failed_with_context(
        http_status: u16,
        additional_context: Option<String>,
    ) -> Self {
        Self::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(http_status),
                additional_context,
            },
        }
    }

    pub fn unexpected_response_error(http_status: u16) -> Self {
        Self::UnexpectedResponseError {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(http_status),
                additional_context: None,
            },
        }
    }

    pub fn unexpected_response_error_http_status_unknown() -> Self {
        Self::UnexpectedResponseError {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: None,
            },
        }
    }

    pub fn unexpected_response_error_with_context(
        http_status: u16,
        additional_context: Option<String>,
    ) -> Self {
        Self::UnexpectedResponseError {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(http_status),
                additional_context,
            },
        }
    }
}

/// Direct conversion from domain ConnectorError to proto (lossless).
impl ErrorSwitch<grpc_api_types::payments::ConnectorError> for ConnectorError {
    fn switch(&self) -> grpc_api_types::payments::ConnectorError {
        match self {
            Self::ConnectorErrorResponse(error_response) => {
                // Pack all fields that cannot be surfaced in proto into error_message,
                // since proto ConnectorError has no dedicated fields for them.
                // Format: "<message> | code:<connector_code> | txn:<id> | network_decline:<code> | network_advice:<code>"
                let mut parts = vec![error_response.message.clone()];
                if let Some(reason) = &error_response.reason {
                    if reason != &error_response.message {
                        parts.push(reason.clone());
                    }
                }
                if error_response.code != common_utils::consts::NO_ERROR_CODE {
                    parts.push(format!("code:{}", error_response.code));
                }
                if let Some(txn_id) = &error_response.connector_transaction_id {
                    parts.push(format!("txn:{}", txn_id));
                }
                if let Some(network_decline) = &error_response.network_decline_code {
                    parts.push(format!("network_decline:{}", network_decline));
                }
                if let Some(network_advice) = &error_response.network_advice_code {
                    parts.push(format!("network_advice:{}", network_advice));
                }
                if let Some(network_error) = &error_response.network_error_message {
                    parts.push(format!("network_error:{}", network_error));
                }
                if let Some(attempt_status) = &error_response.attempt_status {
                    parts.push(format!("attempt_status:{:?}", attempt_status));
                }
                grpc_api_types::payments::ConnectorError {
                    error_message: parts.join(" | "),
                    error_code: self.error_code().to_string(),
                    http_status_code: Some(error_response.status_code as u32),
                }
            }
            _ => {
                let context = self.response_transformation_context();
                let base_message = self.to_string();
                let error_message = combine_error_message_with_context(
                    &base_message,
                    context.additional_context.as_deref(),
                );
                grpc_api_types::payments::ConnectorError {
                    error_message,
                    error_code: self.error_code().to_string(),
                    http_status_code: context.http_status_code.map(|code| code as u32),
                }
            }
        }
    }
}

/// Errors that occur during webhook processing
#[derive(Debug, thiserror::Error, PartialEq, Clone, strum::AsRefStr)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhookError {
    #[error("Webhooks not implemented for this connector ({operation})")]
    WebhooksNotImplemented { operation: &'static str },
    #[error("Failed to decode webhook event body")]
    WebhookBodyDecodingFailed,
    #[error("Signature not found for incoming webhook")]
    WebhookSignatureNotFound,
    #[error("Failed to verify webhook source")]
    WebhookSourceVerificationFailed,
    #[error("Merchant secret for webhook verification not found")]
    WebhookVerificationSecretNotFound,
    #[error("Failed while processing webhook")]
    WebhookProcessingFailed,
    #[error("Failed to convert amount for webhook: {reason}")]
    WebhookAmountConversionFailed { reason: String },
    #[error("Merchant secret for webhook verification is invalid")]
    WebhookVerificationSecretInvalid,
    #[error("Incoming webhook object reference ID not found")]
    WebhookReferenceIdNotFound,
    #[error("Incoming webhook event type not found")]
    WebhookEventTypeNotFound,
    #[error("Incoming webhook event resource object not found")]
    WebhookResourceObjectNotFound,
    #[error("Failed to encode webhook response")]
    WebhookResponseEncodingFailed,
    #[error("Missing required EventContext field '{field}' for this connector's webhook handling. Pass {field} from your original {origin} request in EventContext.")]
    WebhookMissingRequiredContext {
        field: &'static str,
        origin: &'static str,
    },
    #[error("Missing required field '{field}' in webhook request")]
    WebhookMissingRequiredField { field: &'static str },
}

impl ErrorSwitch<grpc_api_types::payments::IntegrationError> for WebhookError {
    fn switch(&self) -> grpc_api_types::payments::IntegrationError {
        grpc_api_types::payments::IntegrationError {
            error_message: self.to_string(),
            error_code: self.as_ref().to_string(),
            suggested_action: None,
            doc_url: None,
        }
    }
}

/// Wrapper enum used by `execute_connector_processing_step` (gRPC unified path)
/// which performs all three phases in one call.
/// SDK uses `IntegrationError` / `ConnectorError` directly.
///
/// `ConnectorFlowError::Response` carries both UCS-side transformation failures
/// and connector-side 4xx/5xx error responses — distinguished by the
/// `ConnectorError` variant inside.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConnectorFlowError {
    #[error("Connector Request Transformation error: {0}")]
    Request(#[from] IntegrationError),
    #[error("Client error: {0}")]
    Client(#[from] ApiClientError),
    #[error("Kafka client error: {0}")]
    KafkaClient(common_enums::KafkaClientError),
    #[error("Connector error: {0}")]
    Response(#[from] ConnectorError),
}

impl From<common_enums::ApiClientError> for ApiClientError {
    fn from(value: common_enums::ApiClientError) -> Self {
        match value {
            common_enums::ApiClientError::HeaderMapConstructionFailed => {
                Self::HeaderMapConstructionFailed
            }
            common_enums::ApiClientError::InvalidProxyConfiguration => {
                Self::InvalidProxyConfiguration
            }
            common_enums::ApiClientError::ClientConstructionFailed => {
                Self::ClientConstructionFailed
            }
            common_enums::ApiClientError::CertificateDecodeFailed => Self::CertificateDecodeFailed,
            common_enums::ApiClientError::BodySerializationFailed => Self::BodySerializationFailed,
            common_enums::ApiClientError::UnexpectedState => Self::UnexpectedState,
            common_enums::ApiClientError::UrlParsingFailed => Self::UrlParsingFailed,
            common_enums::ApiClientError::UrlEncodingFailed => Self::UrlEncodingFailed,
            common_enums::ApiClientError::RequestNotSent(s) => Self::RequestNotSent(s),
            common_enums::ApiClientError::ResponseDecodingFailed => Self::ResponseDecodingFailed,
            common_enums::ApiClientError::RequestTimeoutReceived => Self::RequestTimeoutReceived,
            common_enums::ApiClientError::ConnectionClosedIncompleteMessage => {
                Self::ConnectionClosedIncompleteMessage
            }
            common_enums::ApiClientError::InternalServerErrorReceived => {
                Self::InternalServerErrorReceived
            }
            common_enums::ApiClientError::BadGatewayReceived => Self::BadGatewayReceived,
            common_enums::ApiClientError::ServiceUnavailableReceived => {
                Self::ServiceUnavailableReceived
            }
            common_enums::ApiClientError::GatewayTimeoutReceived => Self::GatewayTimeoutReceived,
            common_enums::ApiClientError::UnexpectedServerResponse => {
                Self::UnexpectedServerResponse
            }
        }
    }
}

/// Map a request-phase error report into `ConnectorFlowError::Request`.
pub fn report_connector_request_to_flow(
    report: Report<IntegrationError>,
) -> Report<ConnectorFlowError> {
    let ctx = report.current_context().clone();
    report.change_context(ConnectorFlowError::Request(ctx))
}

/// Map a response-phase error report into `ConnectorFlowError::Response`.
pub fn report_connector_response_to_flow(
    report: Report<ConnectorError>,
) -> Report<ConnectorFlowError> {
    let ctx = report.current_context().clone();
    report.change_context(ConnectorFlowError::Response(ctx))
}

/// Map transport-layer `common_enums::ApiClientError` reports into `ConnectorFlowError::Client`.
pub fn report_common_api_client_to_flow(
    report: Report<common_enums::ApiClientError>,
) -> Report<ConnectorFlowError> {
    let ctx: ApiClientError = report.current_context().clone().into();
    report.change_context(ConnectorFlowError::Client(ctx))
}

/// Map `common_enums::KafkaClientError` reports into `ConnectorFlowError::KafkaClient`.
pub fn report_kafka_client_to_flow(
    report: Report<common_enums::KafkaClientError>,
) -> Report<ConnectorFlowError> {
    let ctx = report.current_context().clone();
    report.change_context(ConnectorFlowError::KafkaClient(ctx))
}

#[derive(Debug, thiserror::Error)]
pub enum ParsingError {
    #[error("Failed to parse struct: {0}")]
    StructParseFailure(&'static str),
    #[error("Failed to serialize to {0} format")]
    EncodeError(&'static str),
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    InvalidRequestError,
    ObjectNotFound,
    RouterError,
    ProcessingError,
    BadGateway,
    ServerNotAvailable,
    DuplicateRequest,
    ValidationError,
    ConnectorError,
    LockTimeout,
}

// CE	Connector Error	Errors originating from connector's end
// HE	Hyperswitch Error	Errors originating from Hyperswitch's end
// IR	Invalid Request Error	Error caused due to invalid fields and values in API request
// WE	Webhook Error	Errors related to Webhooks
#[derive(Debug, Clone, router_derive::ApiError)]
#[error(error_type_enum = ErrorType)]
#[allow(unused_variables, unused_assignments)]
pub enum ApiErrorResponse {
    #[error(error_type = ErrorType::ConnectorError, code = "CE_00", message = "{code}: {message}", ignore = "status_code")]
    ExternalConnectorError {
        code: String,
        message: String,
        _connector: String,
        _status_code: u16,
        _reason: Option<String>,
    },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_01", message = "Payment failed during authorization with connector. Retry payment")]
    PaymentAuthorizationFailed { _data: Option<serde_json::Value> },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_02", message = "Payment failed during authentication with connector. Retry payment")]
    PaymentAuthenticationFailed { _data: Option<serde_json::Value> },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_03", message = "Capture attempt failed while processing with connector")]
    PaymentCaptureFailed { _data: Option<serde_json::Value> },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_04", message = "The card data is invalid")]
    InvalidCardData { _data: Option<serde_json::Value> },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_05", message = "The card has expired")]
    CardExpired { _data: Option<serde_json::Value> },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_06", message = "Refund failed while processing with connector. Retry refund")]
    RefundFailed { _data: Option<serde_json::Value> },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_07", message = "Verification failed while processing with connector. Retry operation")]
    VerificationFailed { _data: Option<serde_json::Value> },
    #[error(error_type = ErrorType::ProcessingError, code = "CE_08", message = "Dispute operation failed while processing with connector. Retry operation")]
    DisputeFailed { _data: Option<serde_json::Value> },

    #[error(error_type = ErrorType::LockTimeout, code = "HE_00", message = "Resource is busy. Please try again later.")]
    ResourceBusy,
    #[error(error_type = ErrorType::ServerNotAvailable, code = "HE_00", message = "Something went wrong")]
    InternalServerError,
    #[error(error_type = ErrorType::ServerNotAvailable, code= "HE_00", message = "{component} health check is failing with error: {message}")]
    HealthCheckError {
        component: &'static str,
        message: String,
    },
    #[error(error_type = ErrorType::ValidationError, code = "HE_00", message = "Failed to convert currency to minor unit")]
    CurrencyConversionFailed,
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "Duplicate refund request. Refund already attempted with the refund ID")]
    DuplicateRefundRequest,
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "Duplicate mandate request. Mandate already attempted with the Mandate ID")]
    DuplicateMandate,
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "The merchant account with the specified details already exists in our records")]
    DuplicateMerchantAccount,
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "The merchant connector account with the specified profile_id '{profile_id}' and connector_label '{connector_label}' already exists in our records")]
    DuplicateMerchantConnectorAccount {
        profile_id: String,
        connector_label: String,
    },
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "The payment method with the specified details already exists in our records")]
    DuplicatePaymentMethod,
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "The payment with the specified payment_id already exists in our records")]
    DuplicatePayment {
        _payment_id: common_utils::id_type::PaymentId,
    },
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "The payout with the specified payout_id '{payout_id}' already exists in our records")]
    DuplicatePayout { payout_id: String },
    #[error(error_type = ErrorType::DuplicateRequest, code = "HE_01", message = "The config with the specified key already exists in our records")]
    DuplicateConfig,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Refund does not exist in our records")]
    RefundNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Payment Link does not exist in our records")]
    PaymentLinkNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Customer does not exist in our records")]
    CustomerNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Config key does not exist in our records.")]
    ConfigNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Payment does not exist in our records")]
    PaymentNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Payment method does not exist in our records")]
    PaymentMethodNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Merchant account does not exist in our records")]
    MerchantAccountNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Merchant connector account does not exist in our records")]
    MerchantConnectorAccountNotFound { _id: String },
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Business profile with the given id  '{id}' does not exist in our records")]
    ProfileNotFound { id: String },
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Profile acquirer with id '{profile_acquirer_id}' not found for profile '{profile_id}'.")]
    ProfileAcquirerNotFound {
        profile_acquirer_id: String,
        profile_id: String,
    },
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Poll with the given id  '{id}' does not exist in our records")]
    PollNotFound { id: String },
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Resource ID does not exist in our records")]
    ResourceIdNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Mandate does not exist in our records")]
    MandateNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Authentication does not exist in our records")]
    AuthenticationNotFound { _id: String },
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Failed to update mandate")]
    MandateUpdateFailed,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "API Key does not exist in our records")]
    ApiKeyNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Payout does not exist in our records")]
    PayoutNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_02", message = "Event does not exist in our records")]
    EventNotFound,
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "Invalid mandate id passed from connector")]
    MandateSerializationFailed,
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "Unable to parse the mandate identifier passed from connector")]
    MandateDeserializationFailed,
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "Return URL is not configured and not passed in payments request")]
    ReturnUrlUnavailable,
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "This refund is not possible through Hyperswitch. Please raise the refund through {connector} dashboard")]
    RefundNotPossible { connector: String },
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "Mandate Validation Failed" )]
    MandateValidationFailed { _reason: String },
    #[error(error_type= ErrorType::ValidationError, code = "HE_03", message = "The payment has not succeeded yet. Please pass a successful payment to initiate refund")]
    PaymentNotSucceeded,
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "The specified merchant connector account is disabled")]
    MerchantConnectorAccountDisabled,
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "{code}: {message}")]
    PaymentBlockedError {
        code: u16,
        message: String,
        _status: String,
        _reason: String,
    },
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "File validation failed")]
    FileValidationFailed { _reason: String },
    #[error(error_type = ErrorType::ValidationError, code = "HE_03", message = "Dispute status validation failed")]
    DisputeStatusValidationFailed { _reason: String },
    #[error(error_type= ErrorType::ObjectNotFound, code = "HE_04", message = "Successful payment not found for the given payment id")]
    SuccessfulPaymentNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_04", message = "The connector provided in the request is incorrect or not available")]
    IncorrectConnectorNameGiven,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_04", message = "Address does not exist in our records")]
    AddressNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_04", message = "Dispute does not exist in our records")]
    DisputeNotFound { _dispute_id: String },
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_04", message = "File does not exist in our records")]
    FileNotFound,
    #[error(error_type = ErrorType::ObjectNotFound, code = "HE_04", message = "File not available")]
    FileNotAvailable,
    #[error(error_type = ErrorType::ProcessingError, code = "HE_05", message = "Missing tenant id")]
    MissingTenantId,
    #[error(error_type = ErrorType::ProcessingError, code = "HE_05", message = "Invalid tenant id: {tenant_id}")]
    InvalidTenant { tenant_id: String },
    #[error(error_type = ErrorType::ValidationError, code = "HE_06", message = "Failed to convert amount to {amount_type} type")]
    AmountConversionFailed { amount_type: &'static str },
    #[error(error_type = ErrorType::ServerNotAvailable, code = "IR_00", message = "{message:?}")]
    NotImplemented { message: NotImplementedMessage },
    #[error(
        error_type = ErrorType::InvalidRequestError, code = "IR_01",
        message = "API key not provided or invalid API key used"
    )]
    Unauthorized,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_02", message = "Unrecognized request URL")]
    InvalidRequestUrl,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_03", message = "The HTTP method is not applicable for this API")]
    InvalidHttpMethod,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_04", message = "Missing required param: {field_name}")]
    MissingRequiredField { field_name: &'static str },
    #[error(
        error_type = ErrorType::InvalidRequestError, code = "IR_05",
        message = "{field_name} contains invalid data. Expected format is {expected_format}"
    )]
    InvalidDataFormat {
        field_name: String,
        expected_format: String,
    },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_06", message = "{message}")]
    InvalidRequestData { message: String },
    /// Typically used when a field has invalid value, or deserialization of the value contained in a field fails.
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_07", message = "Invalid value provided: {field_name}")]
    InvalidDataValue { field_name: &'static str },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_08", message = "Client secret was not provided")]
    ClientSecretNotGiven,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_08", message = "Client secret has expired")]
    ClientSecretExpired,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_09", message = "The client_secret provided does not match the client_secret associated with the Payment")]
    ClientSecretInvalid,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_10", message = "Customer has active mandate/subsciption")]
    MandateActive,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_11", message = "Customer has already been redacted")]
    CustomerRedacted,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_12", message = "Reached maximum refund attempts")]
    MaximumRefundCount,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_13", message = "The refund amount exceeds the amount captured")]
    RefundAmountExceedsPaymentAmount,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_14", message = "This Payment could not be {current_flow} because it has a {field_name} of {current_value}. The expected state is {states}")]
    PaymentUnexpectedState {
        current_flow: String,
        field_name: String,
        current_value: String,
        states: String,
    },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_15", message = "Invalid Ephemeral Key for the customer")]
    InvalidEphemeralKey,
    /// Typically used when information involving multiple fields or previously provided information doesn't satisfy a condition.
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_16", message = "{message}")]
    PreconditionFailed { message: String },
    #[error(
        error_type = ErrorType::InvalidRequestError, code = "IR_17",
        message = "Access forbidden, invalid JWT token was used"
    )]
    InvalidJwtToken,
    #[error(
        error_type = ErrorType::InvalidRequestError, code = "IR_18",
        message = "{message}",
    )]
    GenericUnauthorized { message: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_19", message = "{message}")]
    NotSupported { message: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_20", message = "{flow} flow not supported by the {connector} connector")]
    FlowNotSupported { flow: String, connector: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_21", message = "Missing required params")]
    MissingRequiredFields { _field_names: Vec<&'static str> },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_22", message = "Access forbidden. Not authorized to access this resource {resource}")]
    AccessForbidden { resource: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_23", message = "{message}")]
    FileProviderNotSupported { message: String },
    #[error(
        error_type = ErrorType::ProcessingError, code = "IR_24",
        message = "Invalid {wallet_name} wallet token"
    )]
    InvalidWalletToken { wallet_name: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_25", message = "Cannot delete the default payment method")]
    PaymentMethodDeleteFailed,
    #[error(
        error_type = ErrorType::InvalidRequestError, code = "IR_26",
        message = "Invalid Cookie"
    )]
    InvalidCookie,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_27", message = "Extended card info does not exist")]
    ExtendedCardInfoNotFound,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_28", message = "{message}")]
    CurrencyNotSupported { message: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_29", message = "{message}")]
    UnprocessableEntity { message: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_30", message = "Merchant connector account is configured with invalid {config}")]
    InvalidConnectorConfiguration { config: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_31", message = "Card with the provided iin does not exist")]
    InvalidCardIin,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_32", message = "The provided card IIN length is invalid, please provide an iin with 6 or 8 digits")]
    InvalidCardIinLength,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_33", message = "File not found / valid in the request")]
    MissingFile,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_34", message = "Dispute id not found in the request")]
    MissingDisputeId,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_35", message = "File purpose not found in the request or is invalid")]
    MissingFilePurpose,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_36", message = "File content type not found / valid")]
    MissingFileContentType,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_37", message = "{message}")]
    GenericNotFoundError { message: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_38", message = "{message}")]
    GenericDuplicateError { message: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_39", message = "required payment method is not configured or configured incorrectly for all configured connectors")]
    IncorrectPaymentMethodConfiguration,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_40", message = "{message}")]
    LinkConfigurationError { message: String },
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_41", message = "Payout validation failed")]
    PayoutFailed { _data: Option<serde_json::Value> },
    #[error(
        error_type = ErrorType::InvalidRequestError, code = "IR_42",
        message = "Cookies are not found in the request"
    )]
    CookieNotFound,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_43", message = "API does not support platform account operation")]
    PlatformAccountAuthNotSupported,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_44", message = "Invalid platform account operation")]
    InvalidPlatformOperation,
    #[error(error_type = ErrorType::InvalidRequestError, code = "IR_45", message = "External vault failed during processing with connector")]
    ExternalVaultFailed,
    #[error(error_type = ErrorType::InvalidRequestError, code = "WE_01", message = "Failed to authenticate the webhook")]
    WebhookAuthenticationFailed,
    #[error(error_type = ErrorType::InvalidRequestError, code = "WE_02", message = "Bad request received in webhook")]
    WebhookBadRequest,
    #[error(error_type = ErrorType::RouterError, code = "WE_03", message = "There was some issue processing the webhook")]
    WebhookProcessingFailure,
    #[error(error_type = ErrorType::ObjectNotFound, code = "WE_04", message = "Webhook resource not found")]
    WebhookResourceNotFound,
    #[error(error_type = ErrorType::InvalidRequestError, code = "WE_05", message = "Unable to process the webhook body")]
    WebhookUnprocessableEntity,
    #[error(error_type = ErrorType::InvalidRequestError, code = "WE_06", message = "Merchant Secret set my merchant for webhook source verification is invalid")]
    WebhookInvalidMerchantSecret,
    #[error(error_type = ErrorType::ServerNotAvailable, code = "IE", message = "{reason} as data mismatched for {field_names}")]
    IntegrityCheckFailed {
        reason: String,
        field_names: String,
        _connector_transaction_id: Option<String>,
    },
}

#[derive(Clone)]
pub enum NotImplementedMessage {
    Reason(String),
    Default,
}

impl std::fmt::Debug for NotImplementedMessage {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reason(message) => write!(fmt, "{message} is not implemented"),
            Self::Default => {
                write!(
                    fmt,
                    "This API is under development and will be made available soon."
                )
            }
        }
    }
}

impl ::core::fmt::Display for ApiErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"{{"error":{}}}"#,
            serde_json::to_string(self).unwrap_or_else(|_| "API error response".to_string())
        )
    }
}

// impl ErrorSwitch<api_models::errors::types::ApiErrorResponse> for ApiErrorResponse {
//     fn switch(&self) -> api_models::errors::types::ApiErrorResponse {
//         use api_models::errors::types::{ApiError, ApiErrorResponse as AER};

//         match self {
//             Self::ExternalConnectorError {
//                 code,
//                 message,
//                 connector,
//                 reason,
//                 status_code,
//             } => AER::ConnectorError(ApiError::new("CE", 0, format!("{code}: {message}"), Some(Extra {connector: Some(connector.clone()), reason: reason.to_owned(), ..Default::default()})), StatusCode::from_u16(*status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)),
//             Self::PaymentAuthorizationFailed { data } => {
//                 AER::BadRequest(ApiError::new("CE", 1, "Payment failed during authorization with connector. Retry payment", Some(Extra { data: data.clone(), ..Default::default()})))
//             }
//             Self::PaymentAuthenticationFailed { data } => {
//                 AER::BadRequest(ApiError::new("CE", 2, "Payment failed during authentication with connector. Retry payment", Some(Extra { data: data.clone(), ..Default::default()})))
//             }
//             Self::PaymentCaptureFailed { data } => {
//                 AER::BadRequest(ApiError::new("CE", 3, "Capture attempt failed while processing with connector", Some(Extra { data: data.clone(), ..Default::default()})))
//             }
//             Self::InvalidCardData { data } => AER::BadRequest(ApiError::new("CE", 4, "The card data is invalid", Some(Extra { data: data.clone(), ..Default::default()}))),
//             Self::CardExpired { data } => AER::BadRequest(ApiError::new("CE", 5, "The card has expired", Some(Extra { data: data.clone(), ..Default::default()}))),
//             Self::RefundFailed { data } => AER::BadRequest(ApiError::new("CE", 6, "Refund failed while processing with connector. Retry refund", Some(Extra { data: data.clone(), ..Default::default()}))),
//             Self::VerificationFailed { data } => {
//                 AER::BadRequest(ApiError::new("CE", 7, "Verification failed while processing with connector. Retry operation", Some(Extra { data: data.clone(), ..Default::default()})))
//             },
//             Self::DisputeFailed { data } => {
//                 AER::BadRequest(ApiError::new("CE", 8, "Dispute operation failed while processing with connector. Retry operation", Some(Extra { data: data.clone(), ..Default::default()})))
//             }

//             Self::ResourceBusy => {
//                 AER::Unprocessable(ApiError::new("HE", 0, "There was an issue processing the webhook body", None))
//             }
//             Self::CurrencyConversionFailed => {
//                 AER::Unprocessable(ApiError::new("HE", 0, "Failed to convert currency to minor unit", None))
//             }
//             Self::InternalServerError => {
//                 AER::InternalServerError(ApiError::new("HE", 0, "Something went wrong", None))
//             },
//             Self::HealthCheckError { message,component } => {
//                 AER::InternalServerError(ApiError::new("HE",0,format!("{} health check failed with error: {}",component,message),None))
//             },
//             Self::DuplicateRefundRequest => AER::BadRequest(ApiError::new("HE", 1, "Duplicate refund request. Refund already attempted with the refund ID", None)),
//             Self::DuplicateMandate => AER::BadRequest(ApiError::new("HE", 1, "Duplicate mandate request. Mandate already attempted with the Mandate ID", None)),
//             Self::DuplicateMerchantAccount => AER::BadRequest(ApiError::new("HE", 1, "The merchant account with the specified details already exists in our records", None)),
//             Self::DuplicateMerchantConnectorAccount { profile_id, connector_label: connector_name } => {
//                 AER::BadRequest(ApiError::new("HE", 1, format!("The merchant connector account with the specified profile_id '{profile_id}' and connector_label '{connector_name}' already exists in our records"), None))
//             }
//             Self::DuplicatePaymentMethod => AER::BadRequest(ApiError::new("HE", 1, "The payment method with the specified details already exists in our records", None)),
//             Self::DuplicatePayment { payment_id } => {
//                 AER::BadRequest(ApiError::new("HE", 1, "The payment with the specified payment_id already exists in our records", Some(Extra {reason: Some(format!("{payment_id:?} already exists")), ..Default::default()})))
//             }
//             Self::DuplicatePayout { payout_id } => {
//                 AER::BadRequest(ApiError::new("HE", 1, format!("The payout with the specified payout_id '{payout_id}' already exists in our records"), None))
//             }
//             Self::DuplicateConfig => {
//                 AER::BadRequest(ApiError::new("HE", 1, "The config with the specified key already exists in our records", None))
//             }
//             Self::RefundNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Refund does not exist in our records.", None))
//             }
//             Self::PaymentLinkNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Payment Link does not exist in our records", None))
//             }
//             Self::CustomerNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Customer does not exist in our records", None))
//             }
//             Self::ConfigNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Config key does not exist in our records.", None))
//             },
//             Self::PaymentNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Payment does not exist in our records", None))
//             }
//             Self::PaymentMethodNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Payment method does not exist in our records", None))
//             }
//             Self::MerchantAccountNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Merchant account does not exist in our records", None))
//             }
//             Self::MerchantConnectorAccountNotFound {id } => {
//                 AER::NotFound(ApiError::new("HE", 2, "Merchant connector account does not exist in our records", Some(Extra {reason: Some(format!("{id} does not exist")), ..Default::default()})))
//             }
//             Self::ProfileNotFound { id } => {
//                 AER::NotFound(ApiError::new("HE", 2, format!("Business profile with the given id {id} does not exist"), None))
//             }
//             Self::ProfileAcquirerNotFound { profile_acquirer_id, profile_id } => {
//                 AER::NotFound(ApiError::new("HE", 2, format!("Profile acquirer with id '{profile_acquirer_id}' not found for profile '{profile_id}'."), None))
//             }
//             Self::PollNotFound { .. } => {
//                 AER::NotFound(ApiError::new("HE", 2, "Poll does not exist in our records", None))
//             },
//             Self::ResourceIdNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Resource ID does not exist in our records", None))
//             }
//             Self::MandateNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Mandate does not exist in our records", None))
//             }
//             Self::AuthenticationNotFound { .. } => {
//                 AER::NotFound(ApiError::new("HE", 2, "Authentication does not exist in our records", None))
//             },
//             Self::MandateUpdateFailed => {
//                 AER::InternalServerError(ApiError::new("HE", 2, "Mandate update failed", None))
//             },
//             Self::ApiKeyNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "API Key does not exist in our records", None))
//             }
//             Self::PayoutNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Payout does not exist in our records", None))
//             }
//             Self::EventNotFound => {
//                 AER::NotFound(ApiError::new("HE", 2, "Event does not exist in our records", None))
//             }
//             Self::MandateSerializationFailed | Self::MandateDeserializationFailed => {
//                 AER::InternalServerError(ApiError::new("HE", 3, "Something went wrong", None))
//             },
//             Self::ReturnUrlUnavailable => AER::NotFound(ApiError::new("HE", 3, "Return URL is not configured and not passed in payments request", None)),
//             Self::RefundNotPossible { connector } => {
//                 AER::BadRequest(ApiError::new("HE", 3, format!("This refund is not possible through Hyperswitch. Please raise the refund through {connector} dashboard"), None))
//             }
//             Self::MandateValidationFailed { reason } => {
//                 AER::BadRequest(ApiError::new("HE", 3, "Mandate Validation Failed", Some(Extra { reason: Some(reason.to_owned()), ..Default::default() })))
//             }
//             Self::PaymentNotSucceeded => AER::BadRequest(ApiError::new("HE", 3, "The payment has not succeeded yet. Please pass a successful payment to initiate refund", None)),
//             Self::MerchantConnectorAccountDisabled => {
//                 AER::BadRequest(ApiError::new("HE", 3, "The selected merchant connector account is disabled", None))
//             }
//             Self::PaymentBlockedError {
//                 message,
//                 reason,
//                 ..
//             } => AER::DomainError(ApiError::new("HE", 3, message, Some(Extra { reason: Some(reason.clone()), ..Default::default() }))),
//             Self::FileValidationFailed { reason } => {
//                 AER::BadRequest(ApiError::new("HE", 3, format!("File validation failed {reason}"), None))
//             }
//             Self::DisputeStatusValidationFailed { .. } => {
//                 AER::BadRequest(ApiError::new("HE", 3, "Dispute status validation failed", None))
//             }
//             Self::SuccessfulPaymentNotFound => {
//                 AER::NotFound(ApiError::new("HE", 4, "Successful payment not found for the given payment id", None))
//             }
//             Self::IncorrectConnectorNameGiven => {
//                 AER::NotFound(ApiError::new("HE", 4, "The connector provided in the request is incorrect or not available", None))
//             }
//             Self::AddressNotFound => {
//                 AER::NotFound(ApiError::new("HE", 4, "Address does not exist in our records", None))
//             },
//             Self::DisputeNotFound { .. } => {
//                 AER::NotFound(ApiError::new("HE", 4, "Dispute does not exist in our records", None))
//             },
//             Self::FileNotFound => {
//                 AER::NotFound(ApiError::new("HE", 4, "File does not exist in our records", None))
//             }
//             Self::FileNotAvailable => {
//                 AER::NotFound(ApiError::new("HE", 4, "File not available", None))
//             }
//             Self::MissingTenantId => {
//                 AER::InternalServerError(ApiError::new("HE", 5, "Missing Tenant ID in the request".to_string(), None))
//             }
//             Self::InvalidTenant { tenant_id }  => {
//                 AER::InternalServerError(ApiError::new("HE", 5, format!("Invalid Tenant {tenant_id}"), None))
//             }
//             Self::AmountConversionFailed { amount_type }  => {
//                 AER::InternalServerError(ApiError::new("HE", 6, format!("Failed to convert amount to {amount_type} type"), None))
//             }

//             Self::NotImplemented { message } => {
//                 AER::NotImplemented(ApiError::new("IR", 0, format!("{message:?}"), None))
//             }
//             Self::Unauthorized => AER::Unauthorized(ApiError::new(
//                 "IR",
//                 1,
//                 "API key not provided or invalid API key used", None
//             )),
//             Self::InvalidRequestUrl => {
//                 AER::NotFound(ApiError::new("IR", 2, "Unrecognized request URL", None))
//             }
//             Self::InvalidHttpMethod => AER::MethodNotAllowed(ApiError::new(
//                 "IR",
//                 3,
//                 "The HTTP method is not applicable for this API", None
//             )),
//             Self::MissingRequiredField { field_name } => AER::BadRequest(
//                 ApiError::new("IR", 4, format!("Missing required param: {field_name}"), None),
//             ),
//             Self::InvalidDataFormat {
//                 field_name,
//                 expected_format,
//             } => AER::Unprocessable(ApiError::new(
//                 "IR",
//                 5,
//                 format!(
//                     "{field_name} contains invalid data. Expected format is {expected_format}"
//                 ), None
//             )),
//             Self::InvalidRequestData { message } => {
//                 AER::Unprocessable(ApiError::new("IR", 6, message.to_string(), None))
//             }
//             Self::InvalidDataValue { field_name } => AER::BadRequest(ApiError::new(
//                 "IR",
//                 7,
//                 format!("Invalid value provided: {field_name}"), None
//             )),
//             Self::ClientSecretNotGiven => AER::BadRequest(ApiError::new(
//                 "IR",
//                 8,
//                 "client_secret was not provided", None
//             )),
//             Self::ClientSecretExpired => AER::BadRequest(ApiError::new(
//                 "IR",
//                 8,
//                 "The provided client_secret has expired", None
//             )),
//             Self::ClientSecretInvalid => {
//                 AER::BadRequest(ApiError::new("IR", 9, "The client_secret provided does not match the client_secret associated with the Payment", None))
//             }
//             Self::MandateActive => {
//                 AER::BadRequest(ApiError::new("IR", 10, "Customer has active mandate/subsciption", None))
//             }
//             Self::CustomerRedacted => {
//                 AER::BadRequest(ApiError::new("IR", 11, "Customer has already been redacted", None))
//             }
//             Self::MaximumRefundCount => AER::BadRequest(ApiError::new("IR", 12, "Reached maximum refund attempts", None)),
//             Self::RefundAmountExceedsPaymentAmount => {
//                 AER::BadRequest(ApiError::new("IR", 13, "The refund amount exceeds the amount captured", None))
//             }
//             Self::PaymentUnexpectedState {
//                 current_flow,
//                 field_name,
//                 current_value,
//                 states,
//             } => AER::BadRequest(ApiError::new("IR", 14, format!("This Payment could not be {current_flow} because it has a {field_name} of {current_value}. The expected state is {states}"), None)),
//             Self::InvalidEphemeralKey => AER::Unauthorized(ApiError::new("IR", 15, "Invalid Ephemeral Key for the customer", None)),
//             Self::PreconditionFailed { message } => {
//                 AER::BadRequest(ApiError::new("IR", 16, message.to_string(), None))
//             }
//             Self::InvalidJwtToken => AER::Unauthorized(ApiError::new("IR", 17, "Access forbidden, invalid JWT token was used", None)),
//             Self::GenericUnauthorized { message } => {
//                 AER::Unauthorized(ApiError::new("IR", 18, message.to_string(), None))
//             },
//             Self::NotSupported { message } => {
//                 AER::BadRequest(ApiError::new("IR", 19, "Payment method type not supported", Some(Extra {reason: Some(message.to_owned()), ..Default::default()})))
//             },
//             Self::FlowNotSupported { flow, connector } => {
//                 AER::BadRequest(ApiError::new("IR", 20, format!("{flow} flow not supported"), Some(Extra {connector: Some(connector.to_owned()), ..Default::default()}))) //FIXME: error message
//             }
//             Self::MissingRequiredFields { field_names } => AER::BadRequest(
//                 ApiError::new("IR", 21, "Missing required params".to_string(), Some(Extra {data: Some(serde_json::json!(field_names)), ..Default::default() })),
//             ),
//             Self::AccessForbidden {resource} => {
//                 AER::ForbiddenCommonResource(ApiError::new("IR", 22, format!("Access forbidden. Not authorized to access this resource {resource}"), None))
//             },
//             Self::FileProviderNotSupported { message } => {
//                 AER::BadRequest(ApiError::new("IR", 23, message.to_string(), None))
//             },
//             Self::InvalidWalletToken { wallet_name} => AER::Unprocessable(ApiError::new(
//                 "IR",
//                 24,
//                 format!("Invalid {wallet_name} wallet token"), None
//             )),
//             Self::PaymentMethodDeleteFailed => {
//                 AER::BadRequest(ApiError::new("IR", 25, "Cannot delete the default payment method", None))
//             }
//             Self::InvalidCookie => {
//                 AER::BadRequest(ApiError::new("IR", 26, "Invalid Cookie", None))
//             }
//             Self::ExtendedCardInfoNotFound => {
//                 AER::NotFound(ApiError::new("IR", 27, "Extended card info does not exist", None))
//             }
//             Self::CurrencyNotSupported { message } => {
//                 AER::BadRequest(ApiError::new("IR", 28, message, None))
//             }
//             Self::UnprocessableEntity {message} => AER::Unprocessable(ApiError::new("IR", 29, message.to_string(), None)),
//             Self::InvalidConnectorConfiguration {config} => {
//                 AER::BadRequest(ApiError::new("IR", 30, format!("Merchant connector account is configured with invalid {config}"), None))
//             }
//             Self::InvalidCardIin => AER::BadRequest(ApiError::new("IR", 31, "The provided card IIN does not exist", None)),
//             Self::InvalidCardIinLength  => AER::BadRequest(ApiError::new("IR", 32, "The provided card IIN length is invalid, please provide an IIN with 6 digits", None)),
//             Self::MissingFile => {
//                 AER::BadRequest(ApiError::new("IR", 33, "File not found in the request", None))
//             }
//             Self::MissingDisputeId => {
//                 AER::BadRequest(ApiError::new("IR", 34, "Dispute id not found in the request", None))
//             }
//             Self::MissingFilePurpose => {
//                 AER::BadRequest(ApiError::new("IR", 35, "File purpose not found in the request or is invalid", None))
//             }
//             Self::MissingFileContentType => {
//                 AER::BadRequest(ApiError::new("IR", 36, "File content type not found", None))
//             }
//             Self::GenericNotFoundError { message } => {
//                 AER::NotFound(ApiError::new("IR", 37, message, None))
//             },
//             Self::GenericDuplicateError { message } => {
//                 AER::BadRequest(ApiError::new("IR", 38, message, None))
//             }
//             Self::IncorrectPaymentMethodConfiguration => {
//                 AER::BadRequest(ApiError::new("IR", 39, "No eligible connector was found for the current payment method configuration", None))
//             }
//             Self::LinkConfigurationError { message } => {
//                 AER::BadRequest(ApiError::new("IR", 40, message, None))
//             },
//             Self::PayoutFailed { data } => {
//                 AER::BadRequest(ApiError::new("IR", 41, "Payout failed while processing with connector.", Some(Extra { data: data.clone(), ..Default::default()})))
//             },
//             Self::CookieNotFound => {
//                 AER::Unauthorized(ApiError::new("IR", 42, "Cookies are not found in the request", None))
//             },
//             Self::ExternalVaultFailed => {
//                 AER::BadRequest(ApiError::new("IR", 45, "External Vault failed while processing with connector.", None))
//             },

//             Self::WebhookAuthenticationFailed => {
//                 AER::Unauthorized(ApiError::new("WE", 1, "Webhook authentication failed", None))
//             }
//             Self::WebhookBadRequest => {
//                 AER::BadRequest(ApiError::new("WE", 2, "Bad request body received", None))
//             }
//             Self::WebhookProcessingFailure => {
//                 AER::InternalServerError(ApiError::new("WE", 3, "There was an issue processing the webhook", None))
//             },
//             Self::WebhookResourceNotFound => {
//                 AER::NotFound(ApiError::new("WE", 4, "Webhook resource was not found", None))
//             }
//             Self::WebhookUnprocessableEntity => {
//                 AER::Unprocessable(ApiError::new("WE", 5, "There was an issue processing the webhook body", None))
//             },
//             Self::WebhookInvalidMerchantSecret => {
//                 AER::BadRequest(ApiError::new("WE", 6, "Merchant Secret set for webhook source verification is invalid", None))
//             }
//             Self::IntegrityCheckFailed {
//                 reason,
//                 field_names,
//                 connector_transaction_id
//             } => AER::InternalServerError(ApiError::new(
//                 "IE",
//                 0,
//                 format!("{} as data mismatched for {}", reason, field_names),
//                 Some(Extra {
//                     connector_transaction_id: connector_transaction_id.to_owned(),
//                     ..Default::default()
//                 })
//             )),
//             Self::PlatformAccountAuthNotSupported => {
//                 AER::BadRequest(ApiError::new("IR", 43, "API does not support platform operation", None))
//             }
//             Self::InvalidPlatformOperation => {
//                 AER::Unauthorized(ApiError::new("IR", 44, "Invalid platform account operation", None))
//             }
//         }
//     }
// }

// impl actix_web::ResponseError for ApiErrorResponse {
//     fn status_code(&self) -> StatusCode {
//         ErrorSwitch::<api_models::errors::types::ApiErrorResponse>::switch(self).status_code()
//     }

//     fn error_response(&self) -> actix_web::HttpResponse {
//         ErrorSwitch::<api_models::errors::types::ApiErrorResponse>::switch(self).error_response()
//     }
// }

impl From<ApiErrorResponse> for crate::router_data::ErrorResponse {
    fn from(error: ApiErrorResponse) -> Self {
        Self {
            code: error.error_code(),
            message: error.error_message(),
            reason: None,
            status_code: match error {
                ApiErrorResponse::ExternalConnectorError { _status_code, .. } => _status_code,
                _ => 500,
            },
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        }
    }
}

impl ErrorSwitch<IntegrationError> for common_utils::errors::ParsingError {
    fn switch(&self) -> IntegrationError {
        use common_utils::errors::ParsingError as Pe;
        let field_name = match self {
            Pe::EnumParseFailure(name) | Pe::StructParseFailure(name) | Pe::EncodeError(name) => {
                *name
            }
            Pe::UnknownError => "unknown",
            Pe::DateTimeParsingError => "datetime",
            Pe::EmailParsingError => "email",
            Pe::PhoneNumberParsingError => "phone_number",
            Pe::FloatToDecimalConversionFailure => "amount",
            Pe::DecimalToI64ConversionFailure => "integer",
            Pe::StringToFloatConversionFailure => "float",
            Pe::I64ToDecimalConversionFailure => "amount",
            Pe::StringToDecimalConversionFailure { .. } => "decimal",
            Pe::IntegerOverflow => "integer",
        };
        IntegrationError::InvalidDataFormat {
            field_name,
            context: IntegrationErrorContext::default(),
        }
    }
}

// http client errors
#[allow(missing_docs, missing_debug_implementations)]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum HttpClientError {
    #[error("Header map construction failed")]
    HeaderMapConstructionFailed,
    #[error("Invalid proxy configuration")]
    InvalidProxyConfiguration,
    #[error("Client construction failed")]
    ClientConstructionFailed,
    #[error("Certificate decode failed")]
    CertificateDecodeFailed,
    #[error("Request body serialization failed")]
    BodySerializationFailed,
    #[error("Unexpected state reached/Invariants conflicted")]
    UnexpectedState,

    #[error("Failed to parse URL")]
    UrlParsingFailed,
    #[error("URL encoding of request payload failed")]
    UrlEncodingFailed,
    #[error("Failed to send request to connector {0}")]
    RequestNotSent(String),
    #[error("Failed to decode response")]
    ResponseDecodingFailed,

    #[error("Server responded with Request Timeout")]
    RequestTimeoutReceived,

    #[error("connection closed before a message could complete")]
    ConnectionClosedIncompleteMessage,

    #[error("Server responded with Internal Server Error")]
    InternalServerErrorReceived,
    #[error("Server responded with Bad Gateway")]
    BadGatewayReceived,
    #[error("Server responded with Service Unavailable")]
    ServiceUnavailableReceived,
    #[error("Server responded with Gateway Timeout")]
    GatewayTimeoutReceived,
    #[error("Server responded with unexpected response")]
    UnexpectedServerResponse,
}

impl ErrorSwitch<ApiClientError> for HttpClientError {
    fn switch(&self) -> ApiClientError {
        match self {
            Self::HeaderMapConstructionFailed => ApiClientError::HeaderMapConstructionFailed,
            Self::InvalidProxyConfiguration => ApiClientError::InvalidProxyConfiguration,
            Self::ClientConstructionFailed => ApiClientError::ClientConstructionFailed,
            Self::CertificateDecodeFailed => ApiClientError::CertificateDecodeFailed,
            Self::BodySerializationFailed => ApiClientError::BodySerializationFailed,
            Self::UnexpectedState => ApiClientError::UnexpectedState,
            Self::UrlParsingFailed => ApiClientError::UrlParsingFailed,
            Self::UrlEncodingFailed => ApiClientError::UrlEncodingFailed,
            Self::RequestNotSent(reason) => ApiClientError::RequestNotSent(reason.clone()),
            Self::ResponseDecodingFailed => ApiClientError::ResponseDecodingFailed,
            Self::RequestTimeoutReceived => ApiClientError::RequestTimeoutReceived,
            Self::ConnectionClosedIncompleteMessage => {
                ApiClientError::ConnectionClosedIncompleteMessage
            }
            Self::InternalServerErrorReceived => ApiClientError::InternalServerErrorReceived,
            Self::BadGatewayReceived => ApiClientError::BadGatewayReceived,
            Self::ServiceUnavailableReceived => ApiClientError::ServiceUnavailableReceived,
            Self::GatewayTimeoutReceived => ApiClientError::GatewayTimeoutReceived,
            Self::UnexpectedServerResponse => ApiClientError::UnexpectedServerResponse,
        }
    }
}
