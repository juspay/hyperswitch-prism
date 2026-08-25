#[cfg(feature = "log-transformations")]
use std::borrow::Cow;
use std::collections::HashMap;

use hyperswitch_masking::ErasedMaskSerialize;
use serde::{Deserialize, Serialize};

use crate::errors::EventPublisherError;
use crate::types::ExecutionMode;
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

/// Deployment / runtime identity carried on every emitted event.
///
/// `version` is the compiled build (set once in `main` from `ucs_env::git_describe!()`), so it is
/// always present. `application_name` / `deployment_id` / `pod_name` are optional and supplied by
/// the deployment platform via `CS__RUNTIME_METADATA__*` config (e.g. the k8s Downward API:
/// `metadata.name`, deployment labels). When a platform cannot provide one it is omitted rather
/// than inferred; missing optional values never fail startup or event creation.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, config_patch_derive::Patch)]
pub struct RuntimeMetadata {
    /// Compiled build version. Set at startup from the binary, never read from config.
    #[serde(default, skip_deserializing)]
    #[patch(ignore)]
    pub version: String,
    /// Application / microservice name (e.g. `connector-service-http`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_name: Option<String>,
    /// Deployment / rollout identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_id: Option<String>,
    /// Pod name (`metadata.name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod_name: Option<String>,
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
    /// Primary execution or shadow mirror.
    pub execution_mode: ExecutionMode,
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
    /// Deployment / runtime identity, serialized as top-level keys.
    #[serde(flatten)]
    pub runtime_metadata: RuntimeMetadata,
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
    VoidPostRefund,
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
    SurchargeCalculate,
    CreateConnectorCustomer,
    GetConnectorCustomer,
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
    PayoutEligibility,
    NotifyConnector,
    Recharge,
    CreatePaymentMethod,
    GetPaymentMethod,
    RefreshPaymentMethod,
    PreRiskCheck,
    PostRiskCheck,
    PaymentMethodEligibility,
}

impl FlowName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authorize => "Authorize",
            Self::Refund => "Refund",
            Self::VoidPostRefund => "VoidPostRefund",
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
            Self::GetConnectorCustomer => "GetConnectorCustomer",
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
            Self::PayoutEligibility => "PayoutEligibility",
            Self::MandateRevoke => "MandateRevoke",
            Self::SurchargeCalculate => "SurchargeCalculate",
            Self::NotifyConnector => "NotifyConnector",
            Self::Recharge => "Recharge",
            Self::CreatePaymentMethod => "CreatePaymentMethod",
            Self::GetPaymentMethod => "GetPaymentMethod",
            Self::RefreshPaymentMethod => "RefreshPaymentMethod",
            Self::PreRiskCheck => "PreRiskCheck",
            Self::PostRiskCheck => "PostRiskCheck",
            Self::PaymentMethodEligibility => "PaymentMethodEligibility",
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

/// A Kafka topic plus the payload field whose value is used as its partition key.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct KafkaTopicConfig {
    pub topic: String,
    pub partition_key_field: String,
    #[serde(default)]
    pub payload_format: EventPayloadFormat,
}

#[derive(
    Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, config_patch_derive::Patch,
)]
#[serde(rename_all = "snake_case")]
pub enum EventPayloadFormat {
    #[default]
    Json,
    JsonString,
}

/// Controls whether transformations copy or replace the original source fields.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransformationMode {
    /// Add the new key but keep the original source field (default).
    #[default]
    Copy,
    /// Add the new key and remove the original source field after all transformations complete.
    Replace,
}

/// A single field entry for golden log lines.
///
/// Each entry is either a literal value or a reference to a span storage field.
/// - `{ value = "literal" }` — injects a constant string.
/// - `{ source = "span_field" }` — reads the value from the named span storage field at runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum LogFieldEntry {
    /// Read value from the named field in span storage.
    Source { source: String },
    /// Use a literal string value.
    Value { value: String },
}

/// A single compiled log field with pre-split target path segments.
#[derive(Debug, Clone)]
pub struct CompiledLogField {
    /// Pre-split target path segments (e.g., `["api_details", "url"]`).
    /// Segment 0 is the root key; segments 1.. are the nested sub-path.
    pub target_segments: Vec<String>,
    /// Top-level key — segment 0 of the target path.
    pub target_root_key: String,
    /// Resolved entry: either a source field name or a literal value.
    pub entry: CompiledFieldEntry,
}

/// Resolved field entry — either a source field reference or a literal value.
#[derive(Debug, Clone)]
pub enum CompiledFieldEntry {
    /// Read value from this field name in span storage at runtime.
    Source(String),
    /// Use this literal value.
    Value(serde_json::Value),
}

/// Pre-compiled fields for one golden log line type (incoming or outgoing).
#[derive(Debug, Clone, Default)]
pub struct CompiledLogFields {
    pub rules: Vec<CompiledLogField>,
}

impl CompiledLogFields {
    /// Compile raw `target_path → LogFieldEntry` mappings into pre-split rules.
    pub fn compile(raw: &HashMap<String, LogFieldEntry>) -> Self {
        let rules = raw
            .iter()
            .filter_map(|(target_path, field_entry)| {
                let segments: Vec<String> = target_path.split('.').map(String::from).collect();
                let root_key = segments.first()?.clone();
                let entry = match field_entry {
                    LogFieldEntry::Source { source } => {
                        // In prod infra, `.` cannot be used in config keys, so
                        // fields are specified as `request_DOT_body`. Decode
                        // `_DOT_` back to `.` at compile time for correct lookups.
                        CompiledFieldEntry::Source(crate::consts::decode_dot(source))
                    }
                    LogFieldEntry::Value { value } => {
                        CompiledFieldEntry::Value(serde_json::Value::String(value.clone()))
                    }
                };
                Some(CompiledLogField {
                    target_segments: segments,
                    target_root_key: root_key,
                    entry,
                })
            })
            .collect();
        Self { rules }
    }

    /// Returns `true` if no field rules are configured.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Holds compiled fields for both golden log lines.
#[derive(Debug, Clone, Default)]
pub struct CompiledLogFieldsConfig {
    /// Runtime kill-switch: when `false`, `apply_log_fields` is skipped even
    /// though the `log-transformations` feature is compiled in.
    pub enabled: bool,
    pub incoming: CompiledLogFields,
    pub outgoing: CompiledLogFields,
}

impl CompiledLogFieldsConfig {
    pub fn compile(
        enabled: bool,
        incoming: &HashMap<String, LogFieldEntry>,
        outgoing: &HashMap<String, LogFieldEntry>,
    ) -> Self {
        Self {
            enabled,
            incoming: CompiledLogFields::compile(incoming),
            outgoing: CompiledLogFields::compile(outgoing),
        }
    }
}

/// Span storage key for the masked connector reply. A single flat dotted key, matching how
/// `response.body` and `response.headers` are already recorded.
#[cfg(feature = "log-transformations")]
const MASKED_BODY_KEY: &str = "response.masked_body";

/// The connector's raw reply, handed to [`apply_log_fields`] so it can build the masked view.
///
/// These inputs cannot be read back out of span storage: the log formatter serialises every stored
/// key on every event, so an unmasked body parked on the span would leak through any intervening
/// log line. It is passed in directly and never recorded.
#[cfg(feature = "log-transformations")]
pub struct ConnectorResponseForLogging<'a> {
    pub body: &'a [u8],
    pub content_type: Option<&'a str>,
    pub connector_name: &'a str,
    pub masking_keys: &'a crate::connector_response_masking::CompiledMaskingKeys,
}

/// Apply compiled log field rules to the current span's storage.
///
/// For each rule:
/// - `Source` entries read a value from the span storage snapshot.
/// - `Value` entries use the literal value directly.
///
/// Dotted target paths build nested JSON objects. When writing a nested object,
/// it is deep-merged with any existing value for the same root key in the span.
///
/// When `connector_response` is supplied, the masked view of the connector's reply is recorded as
/// `response.masked_body`. That value goes to our own logs and nowhere else — it is deliberately
/// never returned to the caller, so a careless allowlist entry cannot put connector response
/// content on the wire.
#[cfg(feature = "log-transformations")]
pub fn apply_log_fields(
    compiled: &CompiledLogFields,
    connector_response: Option<ConnectorResponseForLogging<'_>>,
) {
    // Two independent jobs behind two independent switches: `[log.fields]` drives the rules,
    // `[connector_response_masking]` drives the masked body. Bail only when neither has work —
    // gating on `compiled` alone would kill masking, since no shipped config defines `[log.fields]`.
    if compiled.is_empty() && connector_response.is_none() {
        return;
    }

    // Build writes: for each rule, resolve the value and group by root key.
    let mut writes: HashMap<String, serde_json::Value> = HashMap::new();

    // Masked first, and outside every span lock: the `tracing::debug!` below re-enters the
    // subscriber, which deadlocks if called from inside `with_current_span*`.
    if let Some(masked) = connector_response.and_then(|response| {
        let masked = crate::connector_response_masking::mask_connector_response(
            response.body,
            response.content_type,
            response.connector_name,
            response.masking_keys,
        )?;
        // Metadata only, and cheap: content type and size are what explain an unexpected
        // `_format: unparsable` stub in the field below.
        tracing::debug!(
            connector = response.connector_name,
            content_type = response.content_type.unwrap_or("<none>"),
            response_bytes = response.body.len(),
            "built masked connector response"
        );
        Some(masked)
    }) {
        writes.insert(MASKED_BODY_KEY.to_string(), masked);
    }

    // Snapshot span storage for `Source` entries. Skipped when no rule reads one: this function
    // now runs on every connector call whenever masking is on, and cloning the whole span map
    // per request to serve zero `Source` rules is pure waste.
    let snapshot: Option<HashMap<Cow<'static, str>, serde_json::Value>> = compiled
        .rules
        .iter()
        .any(|rule| matches!(rule.entry, CompiledFieldEntry::Source(_)))
        .then(|| log_utils::Storage::with_current_span(|storage| storage.values().clone()))
        .flatten();

    for rule in &compiled.rules {
        let resolved_value = match &rule.entry {
            CompiledFieldEntry::Value(v) => v.clone(),
            CompiledFieldEntry::Source(field) => {
                match snapshot
                    .as_ref()
                    .and_then(|s| s.get(field.as_str()))
                    .cloned()
                {
                    Some(v) => v,
                    None => continue, // source field not present in span, skip
                }
            }
        };

        if rule.target_segments.len() == 1 {
            // Flat key: "connector" = { source = "gateway" }
            writes.insert(rule.target_root_key.clone(), resolved_value);
        } else {
            // Nested target: "api_details.url" = { source = "request.url" }
            let root = writes
                .entry(rule.target_root_key.clone())
                .or_insert_with(|| serde_json::json!({}));
            if let Some(rest) = rule.target_segments.get(1..) {
                set_nested_value_from_segments(root, rest, resolved_value);
            }
        }
    }

    // Filter out reserved keys BEFORE entering the span lock.
    // `record_value` calls `tracing::warn!` for reserved keys, which re-enters
    // the subscriber and deadlocks when called inside `with_current_span_mut`.
    let writes: Vec<_> = writes
        .into_iter()
        .filter(|(key, value)| {
            if log_utils::Storage::is_reserved(key) {
                tracing::warn!(
                    "Log field target key `{key}` is reserved by the logging infrastructure, skipping"
                );
                false
            } else {
                // Span storage alone is invisible under `log_format = "default"`: that formatter is
                // stock tracing-subscriber and never reads it. Emit the value as its own event too,
                // matching `record_json_fields_on_span`.
                tracing::info!(%value, "{key}");
                true
            }
        })
        .collect();

    if writes.is_empty() {
        return;
    }

    // Write to span storage, deep-merging nested objects with existing values.
    log_utils::Storage::with_current_span_mut(|storage| {
        for (key, value) in writes {
            if value.is_object() {
                // Deep-merge with existing object if present
                if let Some(existing) = storage.values().get(key.as_str()).cloned() {
                    if existing.is_object() {
                        let mut merged = existing;
                        deep_merge_json(&mut merged, value);
                        storage.record_value(key, merged);
                        continue;
                    }
                }
            }
            storage.record_value(key, value);
        }
    });
}

/// Convert a set of headers with `Maskable` values into a `serde_json::Value` object.
/// Normal values are preserved as-is; masked values use the standard masking format
/// from `Maskable`'s `Debug` implementation.
pub fn maskable_headers_to_json<'a>(
    headers: impl IntoIterator<Item = &'a (String, hyperswitch_masking::Maskable<String>)>,
) -> serde_json::Value {
    headers
        .into_iter()
        .map(|(k, v)| {
            let val = match v {
                hyperswitch_masking::Maskable::Normal(s) => s.clone(),
                hyperswitch_masking::Maskable::Masked(secret) => format!("{secret:?}"),
            };
            (k.clone(), serde_json::Value::String(val))
        })
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into()
}

/// Record one or more key-value pairs as structured JSON directly into the
/// current tracing span's storage.  Values written this way are emitted as
/// proper nested JSON by the log formatter — no escaping.
///
/// All reserved-key filtering happens **outside** the span lock to avoid
/// deadlocks (the storage layer logs a warning for reserved keys, which would
/// re-enter the subscriber).
#[cfg(feature = "logging")]
pub fn record_json_fields_on_span(fields: Vec<(&'static str, serde_json::Value)>) {
    // Filter reserved keys and log fields outside the span lock to avoid deadlocks
    // (tracing::info!/warn! re-enters the subscriber).
    let fields: Vec<_> = fields
        .into_iter()
        .filter(|(key, value)| {
            if log_utils::Storage::is_reserved(key) {
                tracing::warn!(
                    "Span field `{key}` is reserved by the logging infrastructure, skipping"
                );
                false
            } else {
                tracing::info!(%value, "{key}");
                true
            }
        })
        .collect();

    if fields.is_empty() {
        return;
    }

    log_utils::Storage::with_current_span_mut(|storage| {
        for (key, value) in fields {
            storage.record_value(key, value);
        }
    });
}

/// Recursively merge `source` JSON object into `target`.
/// Keys in `source` overwrite matching keys in `target`.
/// Nested objects are merged recursively.
#[cfg(feature = "log-transformations")]
fn deep_merge_json(target: &mut serde_json::Value, source: serde_json::Value) {
    if let (serde_json::Value::Object(target_map), serde_json::Value::Object(source_map)) =
        (target, source)
    {
        for (key, value) in source_map {
            let entry = target_map.entry(key).or_insert(serde_json::Value::Null);
            if entry.is_object() && value.is_object() {
                deep_merge_json(entry, value);
            } else {
                *entry = value;
            }
        }
    }
}

/// Like `set_nested_value` but takes pre-split path segments instead of a dotted string.
/// Avoids runtime `split('.')` — segments are pre-compiled at startup.
#[cfg(feature = "log-transformations")]
fn set_nested_value_from_segments(
    target: &mut serde_json::Value,
    segments: &[String],
    value: serde_json::Value,
) {
    if segments.is_empty() {
        return;
    }
    if let (1, Some(seg)) = (segments.len(), segments.first()) {
        target[seg.as_str()] = value;
        return;
    }
    // Walk intermediate segments, creating objects as needed
    let mut current = target;
    for (i, segment) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            current[segment.as_str()] = value;
            return;
        }
        if !current[segment.as_str()].is_object() {
            current[segment.as_str()] = serde_json::json!({});
        }
        current = &mut current[segment.as_str()];
    }
}

/// Configuration for events system
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct EventConfig {
    pub enabled: bool,
    pub brokers: Vec<String>,
    /// Outbound connector-call events.
    pub connector_events: KafkaTopicConfig,
    /// Inbound request events.
    pub api_events: KafkaTopicConfig,
    #[serde(default)]
    #[patch(ignore)]
    pub transformation_mode: TransformationMode,
    #[serde(default)]
    pub transformations: HashMap<String, String>, // target_path → source_field
    #[serde(default)]
    pub static_values: HashMap<String, String>, // target_path → static_value
    #[serde(default)]
    pub extractions: HashMap<String, String>, // target_path → extraction_path
}

impl EventConfig {
    /// Topic + partition key for the given event stage.
    pub fn topic_config(&self, stage: &EventStage) -> &KafkaTopicConfig {
        match stage {
            EventStage::ConnectorCall => &self.connector_events,
            EventStage::GrpcRequest => &self.api_events,
        }
    }
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            brokers: vec!["localhost:9092".to_string()],
            connector_events: KafkaTopicConfig::default(),
            api_events: KafkaTopicConfig::default(),
            transformation_mode: TransformationMode::default(),
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

    // Process transformations — collect source fields used for potential removal in replace mode
    let mut used_source_fields: Vec<&str> = Vec::new();
    for (target_path, source_field) in &config.transformations {
        if let Some(value) = result.get(source_field).cloned() {
            used_source_fields.push(source_field.as_str());
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

    // In replace mode, remove original source fields after all transformations complete.
    // This is done after the loop so fan-out works (multiple targets reading the same source).
    if matches!(config.transformation_mode, TransformationMode::Replace) {
        if let Some(obj) = result.as_object_mut() {
            for source_field in &used_source_fields {
                obj.remove(*source_field);
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
    for field in &["request", "masked_response", "error"] {
        if let Some(obj) = result.as_object_mut() {
            if let Some(val) = obj.get(*field) {
                if val.is_object() || val.is_array() {
                    let stringified = val.to_string();
                    obj.insert(field.to_string(), serde_json::Value::String(stringified));
                }
            }
        }
    }

    if config.topic_config(&event.stage).payload_format == EventPayloadFormat::JsonString {
        stringify_event_payloads(&mut result);
    }

    Ok(result)
}

fn stringify_event_payloads(result: &mut serde_json::Value) {
    let Some(obj) = result.as_object_mut() else {
        return;
    };

    for field in ["request_data", "response_data", "error"] {
        let Some(value) = obj.get_mut(field) else {
            continue;
        };

        if !value.is_null() && !value.is_string() {
            *value = serde_json::Value::String(value.to_string());
        }
    }
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
