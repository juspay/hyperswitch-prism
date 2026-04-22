use hyperswitch_masking::ErasedMaskSerialize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::errors::EventPublisherError;
use crate::{
    global_id::{
        customer::GlobalCustomerId,
        payment::GlobalPaymentId,
        payment_methods::{GlobalPaymentMethodId, GlobalPaymentMethodSessionId},
        refunds::GlobalRefundId,
        token::GlobalTokenId,
    },
    id_type::{self, ApiKeyId, MerchantConnectorAccountId, ProfileAcquirerId},
    lineage,
    types::TimeRange,
};

/// Wrapper type that enforces masked serialization for Serde values
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct MaskedSerdeValue {
    inner: serde_json::Value,
}

impl MaskedSerdeValue {
    pub fn from_masked<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        let masked_value = hyperswitch_masking::masked_serialize(value)?;
        Ok(Self {
            inner: masked_value,
        })
    }

    pub fn from_masked_optional<T: Serialize>(value: &T, context: &str) -> Option<Self> {
        hyperswitch_masking::masked_serialize(value)
            .map(|masked_value| Self {
                inner: masked_value,
            })
            .inspect_err(|e| {
                tracing::error!(
                    error_category = ?e.classify(),
                    context = context,
                    "Failed to mask serialize data"
                );
            })
            .ok()
    }

    pub fn inner(&self) -> &serde_json::Value {
        &self.inner
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "flow_type", rename_all = "snake_case")]
pub enum ApiEventsType {
    Payout {
        payout_id: String,
    },

    Payment {
        payment_id: GlobalPaymentId,
    },

    Refund {
        payment_id: Option<GlobalPaymentId>,
        refund_id: GlobalRefundId,
    },

    PaymentMethod {
        payment_method_id: GlobalPaymentMethodId,
        payment_method_type: Option<common_enums::PaymentMethod>,
        payment_method_subtype: Option<common_enums::PaymentMethodType>,
    },

    PaymentMethodCreate,

    Customer {
        customer_id: Option<GlobalCustomerId>,
    },

    BusinessProfile {
        profile_id: id_type::ProfileId,
    },
    ApiKey {
        key_id: ApiKeyId,
    },
    User {
        user_id: String,
    },
    PaymentMethodList {
        payment_id: Option<String>,
    },

    PaymentMethodListForPaymentMethods {
        payment_method_id: GlobalPaymentMethodId,
    },

    Webhooks {
        connector: MerchantConnectorAccountId,
        payment_id: Option<GlobalPaymentId>,
    },
    Routing,
    ResourceListAPI,

    PaymentRedirectionResponse {
        payment_id: GlobalPaymentId,
    },
    Gsm,
    // TODO: This has to be removed once the corresponding apiEventTypes are created
    Miscellaneous,
    Keymanager,
    RustLocker,
    ApplePayCertificatesMigration,
    FraudCheck,
    Recon,
    ExternalServiceAuth,
    Dispute {
        dispute_id: String,
    },
    Events {
        merchant_id: id_type::MerchantId,
    },
    PaymentMethodCollectLink {
        link_id: String,
    },
    Poll {
        poll_id: String,
    },
    Analytics,

    ClientSecret {
        key_id: id_type::ClientSecretId,
    },

    PaymentMethodSession {
        payment_method_session_id: GlobalPaymentMethodSessionId,
    },

    Token {
        token_id: Option<GlobalTokenId>,
    },
    ProcessTracker,
    ProfileAcquirer {
        profile_acquirer_id: ProfileAcquirerId,
    },
    ThreeDsDecisionRule,
}

pub trait ApiEventMetric {
    fn get_api_event_type(&self) -> Option<ApiEventsType> {
        None
    }
}

impl ApiEventMetric for serde_json::Value {}
impl ApiEventMetric for () {}

impl ApiEventMetric for GlobalPaymentId {
    fn get_api_event_type(&self) -> Option<ApiEventsType> {
        Some(ApiEventsType::Payment {
            payment_id: self.clone(),
        })
    }
}

impl<Q: ApiEventMetric, E> ApiEventMetric for Result<Q, E> {
    fn get_api_event_type(&self) -> Option<ApiEventsType> {
        match self {
            Ok(q) => q.get_api_event_type(),
            Err(_) => None,
        }
    }
}

// TODO: Ideally all these types should be replaced by newtype responses
impl<T> ApiEventMetric for Vec<T> {
    fn get_api_event_type(&self) -> Option<ApiEventsType> {
        Some(ApiEventsType::Miscellaneous)
    }
}

#[macro_export]
macro_rules! impl_api_event_type {
    ($event: ident, ($($type:ty),+))=> {
        $(
            impl ApiEventMetric for $type {
                fn get_api_event_type(&self) -> Option<ApiEventsType> {
                    Some(ApiEventsType::$event)
                }
            }
        )+
     };
}

impl_api_event_type!(
    Miscellaneous,
    (
        String,
        id_type::MerchantId,
        (Option<i64>, Option<i64>, String),
        (Option<i64>, Option<i64>, id_type::MerchantId),
        bool
    )
);

impl<T: ApiEventMetric> ApiEventMetric for &T {
    fn get_api_event_type(&self) -> Option<ApiEventsType> {
        T::get_api_event_type(self)
    }
}

impl ApiEventMetric for TimeRange {}

fn serialize_method<S: serde::Serializer>(
    method: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(method.as_deref().unwrap_or(""))
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub request_id: String,
    pub timestamp: i128,
    pub flow_type: FlowName,
    pub connector: String,
    pub url: Option<String>,
    /// HTTP verb for outbound connector calls; empty string for gRPC-only audit events.
    #[serde(serialize_with = "serialize_method")]
    pub method: Option<String>,
    pub stage: EventStage,
    pub latency_ms: Option<u64>,
    pub status_code: Option<i32>,
    pub request_data: Option<MaskedSerdeValue>,
    pub response_data: Option<MaskedSerdeValue>,
    pub error: Option<MaskedSerdeValue>,
    pub headers: HashMap<String, String>,
    #[serde(flatten)]
    pub additional_fields: HashMap<String, MaskedSerdeValue>,
    #[serde(flatten)]
    pub lineage_ids: lineage::LineageIds<'static>,
}

impl Event {
    pub fn add_reference_id(&mut self, reference_id: Option<&str>) {
        reference_id
            .and_then(|ref_id| {
                MaskedSerdeValue::from_masked_optional(&ref_id.to_string(), "reference_id")
            })
            .map(|masked_ref| {
                self.additional_fields
                    .insert("reference_id".to_string(), masked_ref);
            });
    }

    pub fn add_resource_id(&mut self, resource_id: Option<&str>) {
        resource_id
            .and_then(|res_id| {
                MaskedSerdeValue::from_masked_optional(&res_id.to_string(), "resource_id")
            })
            .map(|masked_res| {
                self.additional_fields
                    .insert("resource_id".to_string(), masked_res);
            });
    }

    pub fn add_service_type(&mut self, service_type: &str) {
        MaskedSerdeValue::from_masked_optional(&service_type.to_string(), "service_type").map(
            |masked_type| {
                self.additional_fields
                    .insert("service_type".to_string(), masked_type);
            },
        );
    }

    pub fn add_service_name(&mut self, service_name: &str) {
        MaskedSerdeValue::from_masked_optional(&service_name.to_string(), "service_name").map(
            |masked_name| {
                self.additional_fields
                    .insert("service_name".to_string(), masked_name);
            },
        );
    }

    pub fn add_tenant_id(&mut self, tenant_id: &str) {
        MaskedSerdeValue::from_masked_optional(&tenant_id.to_string(), "tenant_id").map(|masked| {
            self.additional_fields
                .insert("tenant_id".to_string(), masked);
        });
    }

    pub fn set_grpc_error_response(&mut self, tonic_error: &tonic::Status) {
        self.status_code = Some(tonic_error.code().into());
        let error_body = serde_json::json!({
            "grpc_code": i32::from(tonic_error.code()),
            "grpc_code_name": format!("{:?}", tonic_error.code())
        });
        self.response_data =
            MaskedSerdeValue::from_masked_optional(&error_body, "grpc_error_response");
    }

    pub fn set_grpc_success_response<R: Serialize>(&mut self, response: &R) {
        self.status_code = Some(0);
        self.response_data =
            MaskedSerdeValue::from_masked_optional(response, "grpc_success_response");
    }

    pub fn set_connector_response<R: Serialize>(&mut self, response: &R) {
        self.response_data = MaskedSerdeValue::from_masked_optional(response, "connector_response");
    }

    pub fn set_error_response<R: Serialize>(&mut self, error: &R) {
        self.error = MaskedSerdeValue::from_masked_optional(error, "error_response");
    }
}

#[derive(strum::Display)]
#[strum(serialize_all = "snake_case")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowName {
    Authorize,
    Refund,
    Capture,
    Void,
    VoidPostCapture,
    Psync,
    Rsync,
    AcceptDispute,
    SubmitEvidence,
    DefendDispute,
    Dsync,
    IncomingWebhook,
    VerifyRedirectResponse,
    SetupMandate,
    RepeatPayment,
    CreateOrder,
    ServerSessionAuthenticationToken,
    ServerAuthenticationToken,
    CreateConnectorCustomer,
    PaymentMethodToken,
    PreAuthenticate,
    Authenticate,
    PostAuthenticate,
    ClientAuthenticationToken,
    MandateRevoke,
    Unknown,
    IncrementalAuthorization,
    PayoutCreate,
    PayoutTransfer,
    PayoutGet,
    PayoutVoid,
    PayoutStage,
    PayoutCreateLink,
    PayoutCreateRecipient,
    PayoutEnrollDisburseAccount,
}

impl FlowName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authorize => "Authorize",
            Self::Refund => "Refund",
            Self::Capture => "Capture",
            Self::Void => "Void",
            Self::VoidPostCapture => "VoidPostCapture",
            Self::Psync => "Psync",
            Self::Rsync => "Rsync",
            Self::AcceptDispute => "AcceptDispute",
            Self::SubmitEvidence => "SubmitEvidence",
            Self::DefendDispute => "DefendDispute",
            Self::Dsync => "Dsync",
            Self::IncomingWebhook => "IncomingWebhook",
            Self::VerifyRedirectResponse => "VerifyRedirectResponse",
            Self::SetupMandate => "SetupMandate",
            Self::RepeatPayment => "RepeatPayment",
            Self::CreateOrder => "CreateOrder",
            Self::PaymentMethodToken => "PaymentMethodToken",
            Self::ServerSessionAuthenticationToken => "ServerSessionAuthenticationToken",
            Self::ServerAuthenticationToken => "ServerAuthenticationToken",
            Self::CreateConnectorCustomer => "CreateConnectorCustomer",
            Self::PreAuthenticate => "PreAuthenticate",
            Self::Authenticate => "Authenticate",
            Self::PostAuthenticate => "PostAuthenticate",
            Self::ClientAuthenticationToken => "ClientAuthenticationToken",
            Self::IncrementalAuthorization => "IncrementalAuthorization",
            Self::PayoutCreate => "PayoutCreate",
            Self::PayoutTransfer => "PayoutTransfer",
            Self::PayoutGet => "PayoutGet",
            Self::PayoutVoid => "PayoutVoid",
            Self::PayoutStage => "PayoutStage",
            Self::PayoutCreateLink => "PayoutCreateLink",
            Self::PayoutCreateRecipient => "PayoutCreateRecipient",
            Self::PayoutEnrollDisburseAccount => "PayoutEnrollDisburseAccount",
            Self::MandateRevoke => "MandateRevoke",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventStage {
    ConnectorCall,
    GrpcRequest,
}

impl EventStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ConnectorCall => "CONNECTOR_CALL",
            Self::GrpcRequest => "GRPC_REQUEST",
        }
    }
}

/// Configuration for events system
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct EventConfig {
    pub enabled: bool,
    pub topic: String,
    pub brokers: Vec<String>,
    pub partition_key_field: String,
    #[serde(default)]
    pub transformations: HashMap<String, String>, // target_path → source_field
    #[serde(default)]
    pub static_values: HashMap<String, String>, // target_path → static_value
    #[serde(default)]
    pub extractions: HashMap<String, String>, // target_path → extraction_path
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            topic: "events".to_string(),
            brokers: vec!["localhost:9092".to_string()],
            partition_key_field: "request_id".to_string(),
            transformations: HashMap::new(),
            static_values: HashMap::new(),
            extractions: HashMap::new(),
        }
    }
}

/// Emit an event: always processes and logs; publishes to Kafka only when kafka feature is enabled.
pub fn emit_event_with_config(event: Event, config: &EventConfig) {
    let processed_event = match process_event_with_config(&event, config) {
        Ok(processed) => processed,
        Err(e) => {
            tracing::error!(error = ?e, "Failed to process event");
            return;
        }
    };
    let event_json = serde_json::to_string(&processed_event)
        .unwrap_or_else(|e| format!("{{\"error\":\"Failed to serialize event: {}\"}}", e));
    tracing::info!(
        events_enabled = config.enabled,
        "Event processed (Kafka publishing: {}) - Event JSON: {}",
        if config.enabled {
            "enabled"
        } else {
            "disabled"
        },
        event_json
    );
    #[cfg(feature = "kafka")]
    crate::event_publisher::publish_event_to_kafka(&event, processed_event, config);
}

/// Process an event by applying masking, transformations, static values, and extractions.
/// This function does not require Kafka and is used for both logging and publishing.
pub(crate) fn process_event_with_config(
    event: &Event,
    config: &EventConfig,
) -> crate::CustomResult<serde_json::Value, EventPublisherError> {
    let mut result = event.masked_serialize().map_err(|e| {
        error_stack::Report::new(EventPublisherError::EventSerializationFailed)
            .attach_printable(format!("Event masked serialization failed: {e}"))
    })?;

    // Helper function to normalize paths (replace _DOT_ and _dot_ with .)
    let normalize_path =
        |path: &str| -> String { path.replace("_DOT_", ".").replace("_dot_", ".") };

    // Process transformations
    for (target_path, source_field) in &config.transformations {
        if let Some(value) = result.get(source_field).cloned() {
            let normalized_path = normalize_path(target_path);
            if let Err(e) = set_nested_value(&mut result, &normalized_path, value) {
                tracing::warn!(
                    target_path = %target_path,
                    normalized_path = %normalized_path,
                    source_field = %source_field,
                    error = %e,
                    "Failed to set transformation, continuing with event processing"
                );
            }
        }
    }

    // Process static values - log warnings but continue processing
    for (target_path, static_value) in &config.static_values {
        let normalized_path = normalize_path(target_path);
        let value = serde_json::json!(static_value);
        if let Err(e) = set_nested_value(&mut result, &normalized_path, value) {
            tracing::warn!(
                target_path = %target_path,
                normalized_path = %normalized_path,
                static_value = %static_value,
                error = %e,
                "Failed to set static value, continuing with event processing"
            );
        }
    }

    // Process extraction
    for (target_path, extraction_path) in &config.extractions {
        if let Some(value) = extract_from_request(&result, extraction_path) {
            let normalized_path = normalize_path(target_path);
            if let Err(e) = set_nested_value(&mut result, &normalized_path, value) {
                tracing::warn!(
                    target_path = %target_path,
                    normalized_path = %normalized_path,
                    extraction_path = %extraction_path,
                    error = %e,
                    "Failed to set extraction, continuing with event processing"
                );
            }
        }
    }

    // Stringify JSON object fields that CKH expects as String columns
    for field in &["request_data", "response_data", "error"] {
        if let Some(obj) = result.as_object_mut() {
            if let Some(val) = obj.get(*field) {
                if val.is_object() || val.is_array() {
                    let stringified = val.to_string();
                    obj.insert(field.to_string(), serde_json::Value::String(stringified));
                }
            }
        }
    }

    Ok(result)
}

pub(crate) fn extract_from_request(
    event_value: &serde_json::Value,
    extraction_path: &str,
) -> Option<serde_json::Value> {
    let mut path_parts = extraction_path.split('.');

    let first_part = path_parts.next()?;

    let source = match first_part {
        "req" => event_value.get("request_data")?.clone(),
        _ => return None,
    };

    let mut current = &source;
    for part in path_parts {
        current = current.get(part)?;
    }

    Some(current.clone())
}

pub(crate) fn set_nested_value(
    target: &mut serde_json::Value,
    path: &str,
    value: serde_json::Value,
) -> crate::CustomResult<(), EventPublisherError> {
    let path_parts: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();

    if path_parts.is_empty() {
        return Err(error_stack::Report::new(EventPublisherError::InvalidPath {
            path: path.to_string(),
        }));
    }

    if path_parts.len() == 1 {
        let key = path_parts.first().ok_or_else(|| {
            error_stack::Report::new(EventPublisherError::InvalidPath {
                path: path.to_string(),
            })
        })?;
        target[key] = value;
        return Ok(());
    }

    let result = path_parts.iter().enumerate().try_fold(
        target,
        |current,
         (index, &part)|
         -> crate::CustomResult<&mut serde_json::Value, EventPublisherError> {
            if index == path_parts.len() - 1 {
                current[part] = value.clone();
                Ok(current)
            } else {
                if !current[part].is_object() {
                    current[part] = serde_json::json!({});
                }
                current.get_mut(part).ok_or_else(|| {
                    error_stack::Report::new(EventPublisherError::InvalidPath {
                        path: format!("{path}.{part}"),
                    })
                })
            }
        },
    );

    result.map(|_| ())
}
