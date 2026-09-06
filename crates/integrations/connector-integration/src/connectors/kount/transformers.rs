use common_enums::{AttemptStatus, FrmDecision, PaymentMethodType};
use common_utils::types::StringMinorUnit;
use domain_types::{
    connector_flow::{
        FrmPaymentOutcome, FrmRefundProcessed, PreRiskCheck, ServerAuthenticationToken,
    },
    connector_types::{
        CustomerInfo, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors,
    frm::frm_types::{
        FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse,
        FrmRefundProcessedRequest, FrmRefundProcessedResponse, MerchantDetails,
        PreRiskCheckRequest, PreRiskCheckResponse,
    },
    mandates::MandateAmountData,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::{e123_phone_number, Address},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{connectors::kount::KountRouterData, types::ResponseRouterData};

type Error = error_stack::Report<errors::IntegrationError>;
type ResponseError = error_stack::Report<errors::ConnectorError>;

/// OAuth scope required by the Kount Orders API.
const KOUNT_API_SCOPE: &str = "k1_integration_api";
/// Fallback values for an unparsable Kount error body.
const KOUNT_DEFAULT_ERROR_CODE: &str = "KOUNT_ERROR";
const KOUNT_DEFAULT_ERROR_MESSAGE: &str = "Kount request failed";
/// Kount developer documentation, surfaced in error contexts.
pub const KOUNT_DOC_URL: &str = "https://developer.kount.com/";

// ──────────────────────────────────────────────────────────────────────────
// Auth + error types
// ──────────────────────────────────────────────────────────────────────────

/// Kount auth. `api_key` is the base64 of `CLIENT_ID:CLIENT_SECRET` (Kount's
/// "API Key"); it is used directly as the `Authorization: Basic {api_key}`
/// value on the token request. `client_id` is the Kount-assigned merchant CID
/// rendered into the Device Data Collection script as the Web SDK `clientID`;
/// it is only ever read from the connector config, and is optional here —
/// only the DDC flow requires one, and validates that itself.
/// `auth_server_id` is the account/environment specific OAuth
/// authorization-server id (sandbox vs production differ).
#[derive(Debug, Clone)]
pub struct KountAuthType {
    pub api_key: Secret<String>,
    pub client_id: Option<String>,
    pub auth_server_id: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for KountAuthType {
    type Error = Error;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Kount {
                api_key,
                client_id,
                auth_server_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                client_id: client_id.to_owned(),
                auth_server_id: auth_server_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Kount expects ConnectorSpecificConfig::Kount with a base64 \
                             `CLIENT_ID:CLIENT_SECRET` api_key, but a different connector \
                             config variant was supplied"
                                .to_owned(),
                        ),
                        suggested_action: Some(
                            "Send the Kount connector config (api_key, client_id, optional \
                             auth_server_id) for Kount FRM flows"
                                .to_owned(),
                        ),
                        doc_url: Some(KOUNT_DOC_URL.to_owned()),
                    }
                }
            )),
        }
    }
}

/// Kount error body. Both fields are optional — a malformed/empty body falls back
/// to [`KOUNT_DEFAULT_ERROR_CODE`] / [`KOUNT_DEFAULT_ERROR_MESSAGE`] when the
/// response is mapped (see `Kount::build_error_response`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KountErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
}

impl KountErrorResponse {
    /// Error code, falling back to the default when Kount omits it.
    pub fn code(&self) -> String {
        self.code
            .clone()
            .unwrap_or_else(|| KOUNT_DEFAULT_ERROR_CODE.to_string())
    }

    /// Error message, falling back to the default when Kount omits it.
    pub fn message(&self) -> String {
        self.message
            .clone()
            .unwrap_or_else(|| KOUNT_DEFAULT_ERROR_MESSAGE.to_string())
    }
}

// ──────────────────────────────────────────────────────────────────────────
// ServerAuthenticationToken (OAuth client-credentials)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct KountTokenRequest {
    pub grant_type: String,
    pub scope: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for KountTokenRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: item.router_data.request.grant_type.clone(),
            scope: KOUNT_API_SCOPE.to_string(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountTokenResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
}

impl<F> TryFrom<ResponseRouterData<KountTokenResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(item: ResponseRouterData<KountTokenResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                token_type: Some(item.response.token_type),
                expires_in: Some(item.response.expires_in),
            }),
            ..item.router_data
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Kount Orders API — shared response shape + decision mapping
// ──────────────────────────────────────────────────────────────────────────

/// Kount Orders API decision values.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum KountDecision {
    Approve,
    Review,
    Decline,
    #[serde(other)]
    Unknown,
}

impl From<&KountDecision> for FrmDecision {
    fn from(value: &KountDecision) -> Self {
        match value {
            KountDecision::Approve => Self::Approve,
            KountDecision::Review => Self::Review,
            KountDecision::Decline => Self::Reject,
            // Kount documents only APPROVE/DECLINE/REVIEW; treat anything else as REVIEW.
            KountDecision::Unknown => Self::Review,
        }
    }
}

/// Pairs a parsed Kount response with the verbatim JSON body it was parsed from.
///
/// `raw_connector_response` must carry Kount's response *exactly* as received —
/// re-serialising the typed struct silently drops every field we don't model.
/// Deserialising into a `serde_json::Value` first preserves the full payload,
/// while the typed `parsed_response` still drives decision mapping.
///
#[derive(Debug, Clone)]
pub struct KountResponseWithRaw<T> {
    pub parsed_response: T,
    pub raw_response: serde_json::Value,
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for KountResponseWithRaw<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw_response = serde_json::Value::deserialize(deserializer)?;
        let parsed_response = T::deserialize(&raw_response).map_err(serde::de::Error::custom)?;
        Ok(Self {
            parsed_response,
            raw_response,
        })
    }
}

impl<T: Serialize> Serialize for KountResponseWithRaw<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.parsed_response.serialize(serializer)
    }
}

/// Per-flow response newtypes. Each is a distinct named type so the connector
/// macros generate a unique templating type per flow, while sharing
/// [`KountResponseWithRaw`]'s full-body capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KountPreRiskCheckResponse(pub KountResponseWithRaw<KountOrderResponse>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KountFrmPaymentOutcomeResponse(pub KountResponseWithRaw<KountUpdateOrderResponse>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KountFrmRefundProcessedResponse(pub KountResponseWithRaw<KountRefundUpdateResponse>);

/// Evaluate Order response (`POST /commerce/v2/orders`), shared by PreRiskCheck
/// and the notify flows. PII/card fields are `Secret`-wrapped so they mask in the
/// event log; risk analytics stay in plaintext.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountOrderResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub order: Option<KountOrder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountOrder {
    /// Kount-assigned order id.
    pub order_id: Option<String>,
    pub risk_inquiry: Option<KountRiskInquiry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Device/session identifier — PII, masked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_session_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transactions: Option<Vec<KountRespTransaction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<Vec<KountRespFulfillment>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountRiskInquiry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<KountDecision>,
    // Kept as `f64` (the live Kount response returns a JSON number, e.g. `61.4`);
    // the PreRiskCheck mapping reads this to derive the integer risk score.
    #[serde(alias = "score", skip_serializing_if = "Option::is_none")]
    pub omniscore: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<KountPersona>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<KountRespDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_executed: Option<KountSegmentExecuted>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<KountEmailSignals>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_management: Option<KountPolicyManagement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

/// Response from the Kount Orders API update (`PATCH /commerce/v2/orders/{id}`).
/// The PATCH ack echoes the order envelope only — no risk body (that comes solely
/// from Evaluate Order). Modelling the real ack fields keeps the notify
/// `connector_response_data` log complete and per-field masked. Distinct type from
/// [`KountOrderResponse`] so the connector macros generate a unique templating type
/// per flow.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountUpdateOrderResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Device fingerprint id — masks in the event log.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_session_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_date_time: Option<String>,
}

/// Accept a JSON scalar that Kount may send as either a string or a number
/// (the Orders guide documents several count fields as stringified numbers —
/// e.g. `"uniqueCards": "3"` — while the live sandbox returns JSON numbers).
/// Normalises both into an `Option<String>` so the field always parses.
fn de_stringy<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        S(String),
        N(serde_json::Number),
        B(bool),
    }
    Ok(
        Option::<StringOrNumber>::deserialize(deserializer)?.map(|value| match value {
            StringOrNumber::S(s) => s,
            StringOrNumber::N(n) => n.to_string(),
            StringOrNumber::B(b) => b.to_string(),
        }),
    )
}

/// Like [`de_stringy`] but yields a masked `Secret` — for PII scalars (e.g.
/// geo-coordinates) that Kount may send as a string or a number.
fn de_stringy_secret<'de, D>(deserializer: D) -> Result<Option<Secret<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(de_stringy(deserializer)?.map(Secret::new))
}

/// Persona velocity/aggregate counts. Kount documents these as stringified
/// numbers but the live API returns JSON numbers, so they parse leniently via
/// [`de_stringy`]. Not PII — kept in plaintext for risk analytics.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountPersona {
    #[serde(
        default,
        deserialize_with = "de_stringy",
        skip_serializing_if = "Option::is_none"
    )]
    pub unique_cards: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_stringy",
        skip_serializing_if = "Option::is_none"
    )]
    pub unique_devices: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_stringy",
        skip_serializing_if = "Option::is_none"
    )]
    pub unique_emails: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub riskiest_country: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_stringy",
        skip_serializing_if = "Option::is_none"
    )]
    pub total_bank_approved_orders: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_stringy",
        skip_serializing_if = "Option::is_none"
    )]
    pub total_bank_declined_orders: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_stringy",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_velocity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub riskiest_region: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountRespDevice {
    /// Device fingerprint id — PII, masked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_attributes: Option<KountDeviceAttributes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<KountDeviceLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tor: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountDeviceAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_stringy",
        skip_serializing_if = "Option::is_none"
    )]
    pub timezone_offset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobile_sdk_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookies_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<Vec<KountDeviceIp>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

/// Device IP details — all addresses are PII, masked.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountDeviceIp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pierced_address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pierced_organization: Option<String>,
}

/// Geolocation of the device. Everything that pinpoints the end user — precise
/// coordinates, postal code, area code, city and region — is PII and masked;
/// only country-level fields stay in plaintext.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountDeviceLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_stringy_secret",
        skip_serializing_if = "Option::is_none"
    )]
    pub latitude: Option<Secret<String>>,
    #[serde(
        default,
        deserialize_with = "de_stringy_secret",
        skip_serializing_if = "Option::is_none"
    )]
    pub longitude: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale_country_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountSegmentExecuted {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<KountSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policies_executed: Option<Vec<KountPolicyExecuted>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountSegment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountPolicyExecuted {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<KountPolicyOutcome>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountPolicyOutcome {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub outcome_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Email reputation signals (metadata about the email, not the address itself).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountEmailSignals {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_verified_domain: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_recent: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountPolicyManagement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<KountDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_executed: Option<KountNameVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_executed: Option<KountSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policies_executed: Option<Vec<KountPolicyExecuted>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_weights: Option<Vec<KountTagWeight>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set: Option<KountNameVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<KountSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<KountAction>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountNameVersion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountTagWeight {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountAction {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub action_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_to_list_values: Option<Vec<KountAddToListValue>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountAddToListValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_types: Option<Vec<serde_json::Value>>,
}

/// A transaction on the Orders response. `payment` carries card data — masked.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountRespTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment: Option<Vec<KountRespPayment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processor_merchant_id: Option<String>,
}

/// Payment instrument on the Orders response. Every card-identifying field is
/// PII/card data — masked.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountRespPayment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_brand: Option<Secret<String>>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub payment_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_token: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last4: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuing_organization: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_month: Option<Secret<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_year: Option<Secret<i64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountRespFulfillment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_fulfillment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<KountRespShipping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Digital-delivery access URL — may carry a download token, so masked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_url: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digital_downloaded: Option<bool>,
    /// Download device IP — PII, masked. (Kount's field name carries a typo,
    /// `downnloadDeviceIp`, preserved here so it deserializes.)
    #[serde(rename = "downnloadDeviceIp", skip_serializing_if = "Option::is_none")]
    pub download_device_ip: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountRespShipping {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipped_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_date_time: Option<String>,
}

/// Truncate a merchant-supplied id into a valid Kount `sessionId` (≤32 chars,
/// alphanumeric / `-` / `_`). Fallback for [`hash_session_id`].
pub fn to_session_id(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect()
}

/// Kount `sessionId` / `deviceSessionId` derived from the merchant transaction
/// id: the first 32 hex chars of its SHA-256 digest. Hashing keeps the raw
/// merchant id out of the client-visible DDC HTML and yields a valid (≤32-char,
/// alphanumeric) session id regardless of the source format. The DDC HTML
/// (PreAuthenticate) and the Evaluate Order both hash the *same* merchant
/// transaction id, so the collected device data correlates.
pub fn hash_session_id(raw: &str) -> String {
    use common_utils::crypto::{GenerateDigest, Sha256};
    Sha256
        .generate_digest(raw.as_bytes())
        .map(|digest| hex::encode(digest).chars().take(32).collect())
        .unwrap_or_else(|_| to_session_id(raw))
}

/// Round a Kount omniscore (a 0–99 float) to the integer FRM risk score.
/// `f64 -> i32` has no safe `TryFrom`, and the value is bounded, so the cast is
/// scoped here behind an explicit allow.
#[allow(clippy::as_conversions)]
fn omniscore_to_risk_score(omniscore: f64) -> i32 {
    omniscore.round() as i32
}

// ──────────────────────────────────────────────────────────────────────────
// PreRiskCheck = Evaluate Order (POST /commerce/v2/orders)
// ──────────────────────────────────────────────────────────────────────────

/// Sales channel reported on the Evaluate Order.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountChannel {
    Web,
}

/// Kount account type, derived from whether the customer is registered.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountAccountType {
    Registered,
    Guest,
}

/// Kount fulfillment method for an order line.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountFulfillmentType {
    /// Physically shipped to the recipient.
    Shipped,
    /// Digitally delivered (no shipment).
    Digital,
}

/// Kount payment instrument type (`transactions[].payment.type`). The full
/// enumeration from the Kount Orders API is modelled here; only the variants
/// with a UCS payment-method equivalent are ever produced (see
/// [`kount_payment_type`]).
#[derive(Debug, Clone, Copy, Serialize)]
pub enum KountPaymentType {
    #[serde(rename = "APAY")]
    ApplePay,
    #[serde(rename = "CARD")]
    Card,
    #[serde(rename = "CREDIT_CARD")]
    CreditCard,
    #[serde(rename = "DEBIT_CARD")]
    DebitCard,
    #[serde(rename = "PYPL")]
    Paypal,
    #[serde(rename = "CHEK")]
    Check,
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "TOKEN")]
    Token,
    #[serde(rename = "GDMP")]
    GreenDotMoneyPak,
    #[serde(rename = "GOOG")]
    GooglePay,
    #[serde(rename = "BLML")]
    BillMeLater,
    #[serde(rename = "GIFT")]
    GiftCard,
    #[serde(rename = "BPAY")]
    Bpay,
    #[serde(rename = "NETELLER")]
    Neteller,
    #[serde(rename = "GIROPAY")]
    Giropay,
    #[serde(rename = "ELV")]
    Elv,
    #[serde(rename = "MERCADE_PAGO")]
    MercadoPago,
    #[serde(rename = "SEPA")]
    Sepa,
    #[serde(rename = "INTERAC")]
    Interac,
    #[serde(rename = "CARTE_BLEUE")]
    CarteBleue,
    #[serde(rename = "POLI")]
    Poli,
    #[serde(rename = "SKRILL")]
    Skrill,
    #[serde(rename = "SOFORT")]
    Sofort,
    #[serde(rename = "AMZN")]
    AmazonPay,
    #[serde(rename = "SAMPAY")]
    SamsungPay,
    #[serde(rename = "ALIPAY")]
    AliPay,
    #[serde(rename = "WCPAY")]
    WeChatPay,
    #[serde(rename = "CRYPTO")]
    Crypto,
    #[serde(rename = "KLARNA")]
    Klarna,
    #[serde(rename = "AFTRPAY")]
    Afterpay,
    #[serde(rename = "AFFIRM")]
    Affirm,
    #[serde(rename = "SPLIT")]
    Splitit,
    #[serde(rename = "FBPAY")]
    FacebookPay,
    #[serde(rename = "CASH")]
    Cash,
}

/// Evaluate Order request (`POST /commerce/v2/orders`). Modelled on the Kount
/// Orders schema and populated with as much merchant context as the FRM request
/// carries — account, line items, fulfillment, and a transaction block with the
/// payment instrument and billed person — so Kount has enough signal to return a
/// meaningful `riskInquiry` decision rather than a bare order.
#[derive(Debug, Clone, Serialize)]
pub struct KountEvaluateOrderRequest {
    /// Merchant's own order reference (Kount returns its own `order.orderId`).
    #[serde(rename = "merchantOrderId")]
    pub order_id: String,
    /// Links the DDC-collected device data — must equal the DDC SDK sessionID.
    #[serde(rename = "deviceSessionId")]
    pub session_id: String,
    /// Sales channel; web checkout by default.
    pub channel: KountChannel,
    /// Order creation timestamp (RFC 3339).
    #[serde(rename = "creationDateTime")]
    pub creation_date_time: String,
    /// End-user IP address (from browser info), when available.
    #[serde(rename = "userIp", skip_serializing_if = "Option::is_none")]
    pub user_ip: Option<String>,
    /// Customer/account context, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<KountAccount>,
    /// Purchased line items.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<KountItem>,
    /// Fulfillment / shipping context, when a shipping address is present.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fulfillment: Vec<KountFulfillment>,
    /// Payment transaction(s) with the billed person and amount.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transactions: Vec<KountTransaction>,
    /// Device/browser details derived from the browser info, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<KountDevice>,
    /// Merchant Category Code (ISO 18245), when provided.
    #[serde(
        rename = "merchantCategoryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_category_code: Option<u32>,
    /// Merchant details (id), when provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant: Option<KountMerchant>,
    /// Arbitrary merchant-supplied custom fields, sourced from the Pre Risk
    /// Check request's `metadata` blob. See [`kount_custom_fields`] for
    /// Kount's constraints (key ≤32 chars, string values ≤256 chars, no
    /// nested objects/arrays).
    #[serde(rename = "customFields", skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Kount `merchant` object. `id` comes from the FRM `MerchantDetails` contract;
/// `name` / `contactPhoneNumber` come from the Pre Risk Check's
/// `connector_feature_data` and so are only ever set on the Evaluate Order path
/// — Update Order and refund leave them unset, keeping those bodies unchanged.
/// The Kount schema also allows storeName/websiteUrl/contactEmail.
#[derive(Debug, Clone, Default, Serialize)]
pub struct KountMerchant {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Merchant display name. Masked in logs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    /// Merchant contact phone number. Masked in logs.
    #[serde(rename = "contactPhoneNumber", skip_serializing_if = "Option::is_none")]
    pub contact_phone_number: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountDevice {
    #[serde(rename = "ipAddress", skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountAccount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub account_type: KountAccountType,
    /// Whether the account is active. Omitted when unknown (the FRM request
    /// carries no account-status signal).
    #[serde(rename = "accountIsActive", skip_serializing_if = "Option::is_none")]
    pub account_is_active: Option<bool>,
    /// When the customer's account was created (RFC 3339, UTC), from the Pre
    /// Risk Check's `connector_feature_data.customerDataCreated`. Distinct from
    /// the order-level `creationDateTime` on [`KountEvaluateOrderRequest`].
    #[serde(rename = "creationDateTime", skip_serializing_if = "Option::is_none")]
    pub creation_date_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountItem {
    /// Per-unit price in the smallest currency unit (string per Kount schema).
    pub price: StringMinorUnit,
    pub name: String,
    pub quantity: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(rename = "isDigital", skip_serializing_if = "Option::is_none")]
    pub is_digital: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    /// Subscription / recurring billing context for this line item, when the
    /// order carries mandate info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring: Option<KountRecurring>,
    #[serde(rename = "subCategory", skip_serializing_if = "Option::is_none")]
    pub sub_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Kount Orders `recurring` (RecurringDetails) block carried on a line item.
/// Populated from the FRM request's `mandate_details`. Amounts are serialized as
/// strings to match Kount's `string<uint64>` schema.
#[derive(Debug, Clone, Serialize)]
pub struct KountRecurring {
    #[serde(rename = "startDate", skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate", skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(
        rename = "initialBillingAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub initial_billing_amount: Option<String>,
    #[serde(
        rename = "periodBillingAmount",
        skip_serializing_if = "Option::is_none"
    )]
    pub period_billing_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    #[serde(
        rename = "externalSubscriptionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_subscription_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(rename = "nextBillingDate", skip_serializing_if = "Option::is_none")]
    pub next_billing_date: Option<String>,
    #[serde(rename = "billingCycle", skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Format a domain date as an RFC 3339 (UTC) string for Kount's recurring block.
fn format_kount_date(date: time::PrimitiveDateTime) -> Result<String, Error> {
    date.assume_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| {
            error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                field_name: "mandate_details.date",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to format a recurring date as RFC 3339".to_owned(),
                    ),
                    ..Default::default()
                },
            })
        })
}

/// Epoch values whose magnitude reaches this are milliseconds; below it, they
/// are seconds. 1e11 seconds is the year 5138 and 1e11 milliseconds is 1973, so
/// every realistic millisecond timestamp sits above the threshold and every
/// realistic second one below it.
const KOUNT_EPOCH_MILLIS_THRESHOLD: u64 = 100_000_000_000;

/// Normalise a caller-supplied timestamp into the RFC 3339 (UTC) string Kount's
/// `account.creationDateTime` expects. Accepted, in order:
///
/// 1. RFC 3339 with an explicit offset (`2019-08-24T19:45:22+05:30` becomes
///    `2019-08-24T14:15:22Z`).
/// 2. `YYYY-MM-DD HH:MM:SS` or `YYYY-MM-DDTHH:MM:SS` with no offset — assumed
///    UTC, the same convention [`format_kount_date`] uses.
/// 3. Epoch seconds or milliseconds. The caller may send these as a JSON number
///    or a numeric string; [`de_stringy`] delivers both here as a string.
///
/// Returns `None` (with a warning) when nothing parses, so a bad value drops the
/// field instead of failing the Evaluate Order call — the same warn-and-omit
/// convention the notify path uses for its feature data.
pub(super) fn normalize_kount_timestamp(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    // `format_description!` results are bound to locals rather than named in a
    // type: `FormatItem` is deprecated in this version of `time`, and naming it
    // would trip the crate's deny-warnings build.
    let parsed = OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .ok()
        .or_else(|| {
            let space_separated =
                time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
            let t_separated =
                time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
            time::PrimitiveDateTime::parse(value, &space_separated)
                .or_else(|_| time::PrimitiveDateTime::parse(value, &t_separated))
                .ok()
                .map(time::PrimitiveDateTime::assume_utc)
        })
        .or_else(|| {
            // Tolerate a fractional epoch ("1566656122.25") by truncating.
            let whole = value.split_once('.').map_or(value, |(whole, _)| whole);
            let epoch: i64 = whole.parse().ok()?;
            if epoch.unsigned_abs() >= KOUNT_EPOCH_MILLIS_THRESHOLD {
                OffsetDateTime::from_unix_timestamp_nanos(i128::from(epoch) * 1_000_000).ok()
            } else {
                OffsetDateTime::from_unix_timestamp(epoch).ok()
            }
        });

    match parsed {
        Some(date_time) => date_time
            .to_offset(time::UtcOffset::UTC)
            .format(&time::format_description::well_known::Rfc3339)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Kount failed to format customerDataCreated as RFC 3339; \
                     account.creationDateTime will not be sent"
                );
            })
            .ok(),
        None => {
            tracing::warn!(
                "Kount customerDataCreated is neither RFC 3339, `YYYY-MM-DD HH:MM:SS`, nor an \
                 epoch timestamp; account.creationDateTime will not be sent"
            );
            None
        }
    }
}

/// Maps the domain mandate status to Kount's `recurring.status` string.
fn kount_recurring_status(status: &common_enums::MandateStatus) -> String {
    match status {
        common_enums::MandateStatus::Active => "ACTIVE",
        common_enums::MandateStatus::Inactive => "INACTIVE",
        common_enums::MandateStatus::Pending => "PENDING",
        common_enums::MandateStatus::Revoked => "REVOKED",
    }
    .to_string()
}

impl TryFrom<&MandateAmountData> for KountRecurring {
    type Error = Error;

    fn try_from(mandate: &MandateAmountData) -> Result<Self, Self::Error> {
        // Kount's RecurringDetails amounts are smallest-currency-unit strings
        // (`string <uint64>`) — i.e. raw minor units, per the recurring schema and
        // independent of the connector's main amount converter. Omit a 0 value so
        // we never emit a bogus "0" to Kount.
        let minor_unit_amount = |money: &common_utils::types::Money| {
            let amount = money.amount.get_amount_as_i64();
            (amount != 0).then(|| amount.to_string())
        };
        // Shared MandateAmountData carries dates as PrimitiveDateTime; Kount's
        // recurring block wants RFC 3339 (UTC) strings.
        Ok(Self {
            start_date: mandate.start_date.map(format_kount_date).transpose()?,
            end_date: mandate.end_date.map(format_kount_date).transpose()?,
            initial_billing_amount: mandate
                .initial_billing_amount
                .as_ref()
                .and_then(&minor_unit_amount),
            period_billing_amount: minor_unit_amount(&mandate.amount),
            period: mandate.frequency.clone(),
            external_subscription_id: mandate.external_subscription_id.clone(),
            status: mandate.status.as_ref().map(kount_recurring_status),
            next_billing_date: mandate
                .next_billing_date
                .map(format_kount_date)
                .transpose()?,
            billing_cycle: mandate.billing_cycle,
            description: mandate.description.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KountFulfillment {
    #[serde(rename = "type")]
    pub fulfillment_type: KountFulfillmentType,
    #[serde(rename = "recipientPerson", skip_serializing_if = "Option::is_none")]
    pub recipient_person: Option<KountPerson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountTransaction {
    /// Subtotal in the smallest currency unit (string per Kount schema).
    pub subtotal: StringMinorUnit,
    /// Order total in the smallest currency unit (string per Kount schema).
    #[serde(rename = "orderTotal")]
    pub order_total: StringMinorUnit,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment: Option<KountPayment>,
    #[serde(rename = "billedPerson", skip_serializing_if = "Option::is_none")]
    pub billed_person: Option<KountPerson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountPayment {
    #[serde(rename = "type")]
    pub payment_type: KountPaymentType,
    /// Card BIN (first 6 digits) — no full PAN is sent. Card data, masked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last4: Option<Secret<String>>,
    /// Stable, salted hash of the payment instrument (never the raw PAN). Lets
    /// Kount link/score the instrument across orders. See [`payment_token_hash`].
    #[serde(rename = "paymentToken", skip_serializing_if = "Option::is_none")]
    pub payment_token: Option<Secret<String>>,
    /// Card expiry month as an integer (Kount's schema types it int32, not `MM`).
    /// Card data, masked.
    #[serde(rename = "expirationMonth", skip_serializing_if = "Option::is_none")]
    pub expiration_month: Option<Secret<i32>>,
    /// Card expiry year as a four-digit integer. Card data, masked.
    #[serde(rename = "expirationYear", skip_serializing_if = "Option::is_none")]
    pub expiration_year: Option<Secret<i32>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountPerson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<KountName>,
    #[serde(rename = "emailAddress", skip_serializing_if = "Option::is_none")]
    pub email_address: Option<Secret<String>>,
    #[serde(rename = "phoneNumber", skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<KountAddress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountName {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Secret<String>>,
}

impl KountName {
    /// Build a name only when at least one part is present, so we never emit an
    /// empty `{}` name object.
    fn from_parts(first: Option<String>, last: Option<String>) -> Option<Self> {
        (first.is_some() || last.is_some()).then_some(Self {
            first: first.map(Secret::new),
            last: last.map(Secret::new),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KountAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Secret<String>>,
    #[serde(rename = "countryCode", skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
}

/// Build a Kount person block (name / email / phone / address) from a domain
/// address. `None` when the address carries no usable person fields.
fn kount_person_from_address(addr: &Address) -> Option<KountPerson> {
    let details = addr.address.as_ref();
    let name = details.and_then(|details| {
        let first = details
            .first_name
            .as_ref()
            .map(|name| name.peek().to_string());
        let last = details
            .last_name
            .as_ref()
            .map(|name| name.peek().to_string());
        KountName::from_parts(first, last)
    });
    let kount_address = details.map(|details| KountAddress {
        line1: details
            .line1
            .as_ref()
            .map(|line| Secret::new(line.peek().to_string())),
        line2: details
            .line2
            .as_ref()
            .map(|line| Secret::new(line.peek().to_string())),
        city: details
            .city
            .as_ref()
            .map(|city| Secret::new(city.peek().to_string())),
        region: details
            .state
            .as_ref()
            .map(|state| Secret::new(state.peek().to_string())),
        country_code: details.country.map(|country| country.to_string()),
        postal_code: details
            .zip
            .as_ref()
            .map(|zip| Secret::new(zip.peek().to_string())),
    });
    let email_address = addr
        .email
        .as_ref()
        .map(|email| Secret::new(email.peek().to_string()));
    let phone_number = addr.phone.as_ref().and_then(|phone| {
        e123_phone_number(phone.country_code.as_deref(), phone.number.as_ref()?.peek())
    });
    if name.is_none()
        && kount_address.is_none()
        && email_address.is_none()
        && phone_number.is_none()
    {
        return None;
    }
    Some(KountPerson {
        name,
        email_address,
        phone_number,
        address: kount_address,
    })
}

/// Person block from customer info (name / email / phone, no address). Serves
/// the billed person two ways: wholesale when there is no billing address at
/// all, and to supply an email or phone the billing address left out. `None`
/// when the customer carries no usable fields.
fn kount_person_from_customer(customer: &CustomerInfo) -> Option<KountPerson> {
    let first = customer
        .first_name
        .as_ref()
        .map(|name| name.peek().to_string());
    let last = customer
        .last_name
        .as_ref()
        .map(|name| name.peek().to_string());
    let email_address = customer
        .customer_email
        .as_ref()
        .map(|email| Secret::new(email.peek().to_string()));
    let phone_number = customer.customer_phone_number.as_ref().and_then(|phone| {
        e123_phone_number(
            customer.customer_phone_country_code.as_deref(),
            phone.peek(),
        )
    });
    if first.is_none() && last.is_none() && email_address.is_none() && phone_number.is_none() {
        return None;
    }
    Some(KountPerson {
        name: KountName::from_parts(first, last),
        email_address,
        phone_number,
        address: None,
    })
}

/// Stable payment-instrument token for Kount: `hex(HMAC-SHA256(key = api_key,
/// msg = PAN))`. The Kount `api_key` secret is reused as the salt so the token
/// is consistent for a given card under a merchant's credentials while never
/// emitting the raw PAN. Returns `None` if signing fails.
fn payment_token_hash(api_key: &Secret<String>, pan: &str) -> Option<String> {
    use common_utils::crypto::{HmacSha256, SignMessage};
    HmacSha256
        .sign_message(api_key.peek().as_bytes(), pan.as_bytes())
        .ok()
        .map(hex::encode)
}

/// Card BIN (first 6) + last4 from a raw PAN, ignoring formatting. Never emits
/// the full PAN.
fn card_bin_last4(pan: &str) -> (Option<String>, Option<String>) {
    let digits: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
    let bin = (digits.len() >= 6).then(|| digits[..6].to_string());
    let last4 = (digits.len() >= 4).then(|| digits[digits.len() - 4..].to_string());
    (bin, last4)
}

/// Kount `merchant` object + top-level `merchantCategoryCode` from the FRM
/// merchant details. Shared by Evaluate Order and Update Order.
fn kount_merchant(details: Option<&MerchantDetails>) -> (Option<KountMerchant>, Option<u32>) {
    match details {
        Some(details) => (
            details.merchant_id.as_ref().map(|id| KountMerchant {
                id: Some(id.clone()),
                ..Default::default()
            }),
            details.merchant_category_code,
        ),
        None => (None, None),
    }
}

/// Overlay the Pre Risk Check feature-data merchant fields onto the object
/// [`kount_merchant`] built from `MerchantDetails`.
///
/// Kount's `merchant` block is emitted whenever *any* of id / name / contact
/// number is present, so a merchant carrying only a name still gets a block,
/// while a request carrying none of the three omits it entirely. Fields other
/// than the two set here are carried over from `base`, so this stays correct if
/// [`kount_merchant`] later learns to set more of them.
pub(super) fn merge_pre_risk_merchant(
    base: Option<KountMerchant>,
    name: Option<Secret<String>>,
    contact_phone_number: Option<Secret<String>>,
) -> Option<KountMerchant> {
    if name.is_none() && contact_phone_number.is_none() {
        // Nothing to overlay — identical to the notify and refund paths, which
        // never carry these fields.
        return base;
    }
    Some(KountMerchant {
        name,
        contact_phone_number,
        ..base.unwrap_or_default()
    })
}

/// Card expiry as Kount's `expirationMonth` / `expirationYear` integers. The
/// year is expanded to four digits first. Either part that isn't numeric — a
/// vault template token, say — yields `None` rather than failing the order.
fn card_expiry<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<(Option<i32>, Option<i32>), errors::IntegrationError> {
    let month = card
        .get_card_expiry_month_2_digit()?
        .peek()
        .parse::<i32>()
        .ok();
    let year = card
        .get_expiry_year_4_digit()
        .peek()
        .trim()
        .parse::<i32>()
        .ok();
    Ok((month, year))
}

/// Kount payment type for a card, from its (optional) `card_type`. Falls back to
/// the generic `CARD` when credit/debit is unknown.
fn card_payment_type<T: PaymentMethodDataTypes>(card: &Card<T>) -> KountPaymentType {
    match card
        .card_type
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("credit") => KountPaymentType::CreditCard,
        Some("debit") => KountPaymentType::DebitCard,
        _ => KountPaymentType::Card,
    }
}

/// A non-card payment instrument mapped for Kount: the payment `type` plus an
/// optional raw instrument identifier (payer email, IBAN, account number, …)
/// that the caller salted-hashes into `paymentToken`. `token_source` is `None`
/// when the method has no stable identifier worth sending (e.g. device-bound
/// wallet tokens that rotate per transaction and can't link across orders).
struct KountInstrument {
    payment_type: KountPaymentType,
    token_source: Option<String>,
}

impl KountInstrument {
    fn typed(payment_type: KountPaymentType) -> Self {
        Self {
            payment_type,
            token_source: None,
        }
    }

    fn with_token(payment_type: KountPaymentType, token_source: Option<String>) -> Self {
        Self {
            payment_type,
            token_source,
        }
    }
}

/// Maps a `PaymentMethodType` to the corresponding Kount payment type for
/// processor-token instruments (e.g. saved ApplePay → APAY). Returns `None`
/// when there is no direct Kount equivalent, causing the caller to fall back to
/// `TOKEN`.
fn pmt_to_kount_payment_type(pmt: PaymentMethodType) -> Option<KountPaymentType> {
    use KountPaymentType as K;
    match pmt {
        PaymentMethodType::ApplePay => Some(K::ApplePay),
        PaymentMethodType::GooglePay => Some(K::GooglePay),
        PaymentMethodType::Paypal => Some(K::Paypal),
        PaymentMethodType::AmazonPay => Some(K::AmazonPay),
        PaymentMethodType::SamsungPay => Some(K::SamsungPay),
        PaymentMethodType::AliPay | PaymentMethodType::AliPayHk => Some(K::AliPay),
        PaymentMethodType::WeChatPay => Some(K::WeChatPay),
        PaymentMethodType::Giropay => Some(K::Giropay),
        PaymentMethodType::Sofort => Some(K::Sofort),
        PaymentMethodType::Interac => Some(K::Interac),
        PaymentMethodType::Sepa | PaymentMethodType::SepaBankTransfer => Some(K::Sepa),
        PaymentMethodType::Klarna => Some(K::Klarna),
        PaymentMethodType::Affirm => Some(K::Affirm),
        PaymentMethodType::AfterpayClearpay => Some(K::Afterpay),
        _ => None,
    }
}

/// Maps a UCS payment method to a Kount payment instrument (type + optional
/// token source). Returns `None` for methods with no Kount equivalent, in which
/// case the `payment` block is omitted from the Evaluate Order.
fn kount_instrument<T: PaymentMethodDataTypes>(
    pm: &PaymentMethodData<T>,
    pmt: Option<PaymentMethodType>,
) -> Option<KountInstrument> {
    use KountPaymentType as K;
    Some(match pm {
        // Cards carry BIN/last4 + PAN-derived token; type reflects credit/debit.
        PaymentMethodData::Card(card) => KountInstrument::typed(card_payment_type(card)),
        PaymentMethodData::CardRedirect(_) => KountInstrument::typed(K::Card),
        // Processor / network tokens: send the token itself as the (salted-hashed)
        // paymentToken, matching Kount's post-auth `paymentType=TOKEN` guidance.
        PaymentMethodData::NetworkToken(token_data) => {
            KountInstrument::with_token(K::Token, Some(token_data.token_number.peek().to_string()))
        }
        PaymentMethodData::PaymentMethodToken(token_data) => {
            KountInstrument::with_token(K::Token, Some(token_data.token.peek().to_string()))
        }
        PaymentMethodData::Crypto(_) => KountInstrument::typed(K::Crypto),
        PaymentMethodData::GiftCard(_) => KountInstrument::typed(K::GiftCard),
        // For all other payment methods (wallets, pay-later, bank redirect/debit, …)
        // derive the Kount type from PMT; return None when there is no mapping.
        _ => {
            return pmt
                .and_then(pmt_to_kount_payment_type)
                .map(KountInstrument::typed)
        }
    })
}

/// Connector-specific feature data accepted on the FRM Pre Risk Check request.
/// Carries the customer's account-creation timestamp plus merchant identity, so
/// Evaluate Order can populate `account.creationDateTime`, `merchant.name`, and
/// `merchant.contactPhoneNumber`.
///
/// Keys are camelCase to match the caller's blob, which is a *shared* object
/// also carrying unrelated keys (`enableDdc`, `apiVersion`, …) that serde
/// ignores. Every field is optional and parsed leniently through [`de_stringy`]
/// / [`de_stringy_secret`], so one caller typo cannot take the other two fields
/// down with it, and a blob that simply omits all three — or sends them as
/// explicit `null`, which is what the caller emits for an absent value — yields
/// an all-`None` value with no warning.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KountPreRiskCheckFeatureData {
    /// Customer account creation timestamp, normalised by
    /// [`normalize_kount_timestamp`]. `customerDataCreated` is accepted as an
    /// alias — both spellings have been in use by the caller.
    #[serde(
        default,
        alias = "customerDataCreated",
        deserialize_with = "de_stringy"
    )]
    pub(super) customer_creating_time: Option<String>,
    /// Merchant display name — Kount `merchant.name`.
    #[serde(default, deserialize_with = "de_stringy_secret")]
    pub(super) merchant_name: Option<Secret<String>>,
    /// Merchant contact phone number — Kount `merchant.contactPhoneNumber`.
    /// `merchantContactNo` is accepted as an alias, as above.
    #[serde(
        default,
        alias = "merchantContactNo",
        deserialize_with = "de_stringy_secret"
    )]
    pub(super) merchant_contact_number: Option<Secret<String>>,
}

/// Parse the Pre Risk Check `connector_feature_data` blob.
///
/// Unlike the notify path ([`kount_update_transactions`]) an all-`None` result
/// is not suspicious here: the caller sends a shared feature-data object and
/// these three keys are optional additions to it, so their absence is silent.
/// Only malformed JSON warns, and then the whole block is dropped rather than
/// failing the Evaluate Order call.
pub(super) fn kount_pre_risk_feature_data(
    connector_feature_data: Option<&Secret<String>>,
) -> KountPreRiskCheckFeatureData {
    connector_feature_data
        .map(|data| data.peek().trim().to_owned())
        .filter(|data| !data.is_empty())
        .and_then(|data| {
            serde_json::from_str::<KountPreRiskCheckFeatureData>(&data)
                .inspect_err(|err| {
                    tracing::warn!(
                        error = %err,
                        "Kount pre risk check connector_feature_data is not valid JSON; \
                         account.creationDateTime, merchant.name and merchant.contactPhoneNumber \
                         will not be sent"
                    );
                })
                .ok()
        })
        .unwrap_or_default()
}

/// Pull Kount's `customFields` out of the Pre Risk Check `metadata` blob.
///
/// `metadata` is a *shared* object (it already carries `merchantOrderId` /
/// `userIp` from the caller, and may carry more keys later) — only its
/// `customFields` key is used here; every sibling key is ignored. That key's
/// value must itself be a JSON object of caller-supplied key/value pairs.
/// Kount constrains it: keys ≤32 chars; values are string (≤256 chars),
/// number, or boolean — no nested objects/arrays. Entries that violate this
/// are dropped individually (with a warning) rather than losing every custom
/// field over one bad entry. An explicit JSON `null` value is dropped
/// silently — the caller's convention elsewhere is to emit `null` for an
/// absent value, not to omit the key.
pub(super) fn kount_custom_fields(
    metadata: Option<&Secret<String>>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let raw = metadata
        .map(|m| m.peek().trim().to_owned())
        .filter(|s| !s.is_empty())?;

    let parsed = serde_json::from_str::<serde_json::Value>(&raw)
        .inspect_err(|err| {
            tracing::warn!(
                error = %err,
                "Kount metadata is not valid JSON; customFields will not be sent"
            );
        })
        .ok()?;

    let metadata_obj = match parsed {
        serde_json::Value::Object(map) => map,
        _ => {
            tracing::warn!("Kount metadata is not a JSON object; customFields will not be sent");
            return None;
        }
    };

    // Absent is normal/silent — most callers won't send this key yet.
    let custom_fields = metadata_obj.get("customFields")?;

    let obj = match custom_fields {
        serde_json::Value::Object(map) => map.clone(),
        _ => {
            tracing::warn!(
                "Kount metadata.customFields is not a JSON object; customFields will not be sent"
            );
            return None;
        }
    };

    let filtered: serde_json::Map<String, serde_json::Value> = obj
        .into_iter()
        .filter(|(key, value)| match value {
            serde_json::Value::Null => false,
            serde_json::Value::String(s) => {
                let ok = key.len() <= 32 && s.len() <= 256;
                if !ok {
                    tracing::warn!(
                        key = %key,
                        "Kount custom field dropped: exceeds Kount's length limits"
                    );
                }
                ok
            }
            serde_json::Value::Number(_) | serde_json::Value::Bool(_) => {
                let ok = key.len() <= 32;
                if !ok {
                    tracing::warn!(
                        key = %key,
                        "Kount custom field dropped: key exceeds Kount's 32-char limit"
                    );
                }
                ok
            }
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                tracing::warn!(
                    key = %key,
                    "Kount custom field dropped: array/object values are not allowed"
                );
                false
            }
        })
        .collect();

    (!filtered.is_empty()).then_some(filtered)
}

/// Trim a caller-supplied secret and drop it when empty, so Kount never
/// receives `"name": ""` or `"contactPhoneNumber": ""`.
pub(super) fn non_empty_secret(value: Option<Secret<String>>) -> Option<Secret<String>> {
    value.and_then(|value| {
        let trimmed = value.peek().trim();
        (!trimmed.is_empty()).then(|| Secret::new(trimmed.to_owned()))
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    > for KountEvaluateOrderRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
            RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let order_id = req.merchant_transaction_id.clone().ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "merchant_transaction_id",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Kount Evaluate Order needs merchant_transaction_id; it is the merchant \
                         order reference and the basis for the deviceSessionId"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "Set merchant_transaction_id on the FRM Pre Risk Check request".to_owned(),
                    ),
                    doc_url: Some(KOUNT_DOC_URL.to_owned()),
                },
            })
        })?;

        // Connector-specific feature data (customer account-creation date +
        // merchant name / contact number). Parsed once, up front, so both the
        // account block below and the merchant block further down can use it.
        let feature_data = kount_pre_risk_feature_data(req.connector_feature_data.as_ref());
        let custom_fields = kount_custom_fields(req.metadata.as_ref());
        let account_created_at = feature_data
            .customer_creating_time
            .as_deref()
            .and_then(normalize_kount_timestamp);

        // Device / IP from browser info.
        let browser = req.browser_info.as_ref();
        let user_ip = browser.and_then(|info| info.ip_address.map(|ip| ip.to_string()));
        let device = browser.and_then(|info| {
            let ip_address = info.ip_address.map(|ip| ip.to_string());
            let user_agent = info.user_agent.clone();
            (ip_address.is_some() || user_agent.is_some()).then_some(KountDevice {
                ip_address,
                user_agent,
            })
        });

        // Account context from customer info.
        let account = req.customer_info.as_ref().map(|customer| KountAccount {
            account_type: if customer.customer_id.is_some() {
                KountAccountType::Registered
            } else {
                KountAccountType::Guest
            },
            id: customer
                .customer_id
                .as_ref()
                .map(|id| id.get_string_repr().to_string()),
            // No account-status signal in the FRM request, so leave it unset.
            account_is_active: None,
            creation_date_time: account_created_at,
        });

        let currency = req.amount.currency;

        // Subscription / recurring context, applied to every line item when the
        // order carries mandate info.
        let recurring = req
            .mandate_details
            .as_ref()
            .map(KountRecurring::try_from)
            .transpose()?;

        // Line items from order details.
        let items = match req.order_details.as_ref() {
            Some(details) => details
                .iter()
                .map(|detail| {
                    Ok::<_, Self::Error>(KountItem {
                        price: super::KountAmountConvertor::convert(detail.amount, currency)?,
                        name: detail.product_name.clone(),
                        quantity: detail.quantity,
                        description: detail.description.clone(),
                        category: detail.category.clone(),
                        is_digital: detail.requires_shipping.map(|ships| !ships),
                        sku: detail.sku.clone(),
                        recurring: recurring.clone(),
                        sub_category: detail.sub_category.clone(),
                        url: detail.product_link.clone(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        // Fulfillment method (the order's intended fulfillment at evaluation time,
        // not a post-ship status): digital only when every line item is flagged as
        // not requiring shipping, otherwise physical shipment.
        let fulfillment_type = req
            .order_details
            .as_ref()
            .filter(|details| {
                !details.is_empty()
                    && details
                        .iter()
                        .all(|detail| detail.requires_shipping == Some(false))
            })
            .map_or(KountFulfillmentType::Shipped, |_| {
                KountFulfillmentType::Digital
            });

        // Billing / shipping persons from the payment address. The FRM request
        // carries a real PaymentAddress with independently-set billing and
        // shipping addresses (see FrmServicePreRiskCheckRequest.address).
        let address = req.address.as_ref();
        let customer_person = req
            .customer_info
            .as_ref()
            .and_then(kount_person_from_customer);
        // The billing address wins per field; customer info fills only the gaps,
        // so an address carrying just a name still sends Kount the customer's
        // email and phone. Name and postal address stay as the address supplied
        // them. No usable billing address falls back to the customer wholesale.
        let billed_person = address
            .and_then(|addr| addr.get_payment_billing())
            .and_then(kount_person_from_address)
            .map(|person| KountPerson {
                email_address: person.email_address.clone().or_else(|| {
                    customer_person
                        .as_ref()
                        .and_then(|customer| customer.email_address.clone())
                }),
                phone_number: person.phone_number.clone().or_else(|| {
                    customer_person
                        .as_ref()
                        .and_then(|customer| customer.phone_number.clone())
                }),
                ..person
            })
            .or(customer_person);
        // Fulfillment recipient: built entirely from the shipping address (name,
        // email, phone, and postal address) — no fallback to customer_info or to
        // billing. If no shipping address was supplied, no fulfillment recipient
        // is sent at all.
        let fulfillment = address
            .and_then(|addr| addr.get_shipping())
            .and_then(kount_person_from_address)
            .map(|recipient| {
                vec![KountFulfillment {
                    fulfillment_type,
                    recipient_person: Some(recipient),
                }]
            })
            .unwrap_or_default();

        // Payment instrument from the payment method. The Kount api_key is the
        // salt for `paymentToken` (a salted hash of the instrument identifier), so
        // a missing/invalid Kount config is surfaced rather than silently dropped.
        let api_key = KountAuthType::try_from(&item.router_data.connector_config)?.api_key;
        let salted_token = |source: &str| payment_token_hash(&api_key, source);
        let payment = req
            .payment_method
            .as_ref()
            .and_then(|pm| {
                kount_instrument(pm, req.payment_method_type).map(
                    |instrument| -> Result<KountPayment, errors::IntegrationError> {
                        // Cards carry BIN/last4 + expiry + a PAN-derived token; the type
                        // reflects credit/debit.
                        let (bin, last4, payment_token, exp_month, exp_year) = match pm {
                            PaymentMethodData::Card(card) => {
                                let pan = card.card_number.peek();
                                let (bin, last4) = card_bin_last4(pan);
                                let (exp_month, exp_year) = card_expiry(card)?;
                                (bin, last4, salted_token(pan), exp_month, exp_year)
                            }
                            // Non-card methods: salted token of the instrument identifier
                            // (payer email, IBAN, account number, …) when one is available.
                            // No expiry — Kount only defines it for cards.
                            _ => (
                                None,
                                None,
                                instrument.token_source.as_deref().and_then(salted_token),
                                None,
                                None,
                            ),
                        };
                        Ok(KountPayment {
                            payment_type: instrument.payment_type,
                            bin: bin.map(Secret::new),
                            last4: last4.map(Secret::new),
                            payment_token: payment_token.map(Secret::new),
                            expiration_month: exp_month.map(Secret::new),
                            expiration_year: exp_year.map(Secret::new),
                        })
                    },
                )
            })
            .transpose()?;

        let amount = super::KountAmountConvertor::convert(req.amount.amount, currency)?;
        let transactions = vec![KountTransaction {
            subtotal: amount.clone(),
            order_total: amount,
            currency: currency.to_string(),
            payment,
            billed_person,
        }];

        let creation_date_time = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();

        let (merchant, merchant_category_code) = kount_merchant(req.merchant_details.as_ref());
        let merchant = merge_pre_risk_merchant(
            merchant,
            non_empty_secret(feature_data.merchant_name),
            non_empty_secret(feature_data.merchant_contact_number),
        );

        Ok(Self {
            order_id: order_id.clone(),
            session_id: hash_session_id(&order_id),
            channel: KountChannel::Web,
            creation_date_time,
            user_ip,
            account,
            items,
            fulfillment,
            transactions,
            device,
            merchant_category_code,
            merchant,
            custom_fields,
        })
    }
}

impl TryFrom<ResponseRouterData<KountPreRiskCheckResponse, Self>>
    for RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<KountPreRiskCheckResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Always surface the *verbatim* Kount body (independent of the global
        // `return_raw_connector_data` flag). Serialising the captured raw JSON —
        // not the typed struct — keeps every field Kount sent, including any we
        // don't model. Wrapped whole in `Secret` so it masks in the event log.
        // `None` on serialization failure (degrades the audit trail rather than
        // failing the flow).
        let raw_connector_response = serde_json::to_string(&item.response.0.raw_response)
            .ok()
            .map(Secret::new);
        let parsed = &item.response.0.parsed_response;
        let order = parsed.order.as_ref();
        let risk = order.and_then(|order| order.risk_inquiry.as_ref());
        Ok(Self {
            response: Ok(PreRiskCheckResponse {
                frm_decision: risk
                    .and_then(|inquiry| inquiry.decision.as_ref())
                    .map(FrmDecision::from),
                risk_score: risk
                    .and_then(|inquiry| inquiry.omniscore)
                    .map(omniscore_to_risk_score),
                reason: risk.and_then(|inquiry| inquiry.reason.clone()),
                frm_transaction_id: order.and_then(|order| order.order_id.clone()),
                status_code: item.http_code,
            }),
            resource_common_data: FrmFlowData {
                raw_connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// Kount Update Order disposition tokens. Serialized in uppercase to match the
/// Kount Orders schema.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum KountDisposition {
    Approve,
    Decline,
    Review,
}

impl KountDisposition {
    /// Map the FRM decision to a Kount disposition. `Error` has no Kount
    /// disposition, so it is omitted rather than sent as a guessed value.
    fn from_decision(decision: FrmDecision) -> Option<Self> {
        match decision {
            FrmDecision::Approve => Some(Self::Approve),
            FrmDecision::Reject => Some(Self::Decline),
            FrmDecision::Review => Some(Self::Review),
            FrmDecision::Error => None,
        }
    }
}

/// Kount Update Order payment-status tokens. Serialized in uppercase to match
/// the Kount Orders schema.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum KountPaymentStatus {
    Charged,
    Authorized,
    Voided,
    Refunded,
    Declined,
}

impl KountPaymentStatus {
    /// Map the internal attempt status to a Kount payment status. Unmapped
    /// statuses are omitted rather than sent as an unrecognized value.
    fn from_attempt_status(status: AttemptStatus) -> Option<Self> {
        match status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable => Some(Self::Charged),
            AttemptStatus::Authorized | AttemptStatus::PartiallyAuthorized => {
                Some(Self::Authorized)
            }
            AttemptStatus::Voided | AttemptStatus::VoidedPostCapture => Some(Self::Voided),
            AttemptStatus::AutoRefunded => Some(Self::Refunded),
            AttemptStatus::Failure
            | AttemptStatus::AuthorizationFailed
            | AttemptStatus::CaptureFailed
            | AttemptStatus::RouterDeclined => Some(Self::Declined),
            _ => None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// FrmPaymentOutcome (Notify: payment succeeded) = Update Order
// PATCH /commerce/v2/orders/{orderId}
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct KountUpdateOrderRequest {
    /// Connector (gateway) transaction id, attached after authorization.
    #[serde(
        rename = "merchantTransactionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_transaction_id: Option<String>,
    /// Final payment status.
    #[serde(rename = "paymentStatus", skip_serializing_if = "Option::is_none")]
    pub payment_status: Option<KountPaymentStatus>,
    /// Order total in the smallest currency unit (string per Kount schema).
    #[serde(rename = "orderTotal")]
    pub order_total: StringMinorUnit,
    /// ISO 4217 currency code.
    pub currency: String,
    /// FRM decision being notified (APPROVE / DECLINE / REVIEW).
    #[serde(rename = "frmDisposition", skip_serializing_if = "Option::is_none")]
    pub frm_disposition: Option<KountDisposition>,
    /// Merchant Category Code (ISO 18245), when provided.
    #[serde(
        rename = "merchantCategoryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_category_code: Option<u32>,
    /// Merchant details (id), when provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant: Option<KountMerchant>,
    /// Per-transaction authorization/verification results (AVS/CVV), when the
    /// notify request carries connector-specific feature data for them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transactions: Vec<KountUpdateTransaction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountUpdateTransaction {
    #[serde(
        rename = "authorizationStatus",
        skip_serializing_if = "Option::is_none"
    )]
    pub authorization_status: Option<KountAuthorizationStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountAuthorizationStatus {
    #[serde(rename = "authResult", skip_serializing_if = "Option::is_none")]
    pub auth_result: Option<KountAuthResult>,
    #[serde(
        rename = "verificationResponse",
        skip_serializing_if = "Option::is_none"
    )]
    pub verification_response: Option<KountVerificationResponse>,
    /// The payment processor-provided authorization code.
    #[serde(rename = "processorAuthCode", skip_serializing_if = "Option::is_none")]
    pub processor_auth_code: Option<String>,
    /// The unique ID the payment processor or gateway uses for the order.
    ///
    /// Kount documents this on the Evaluate Order (`POST`) authorization status
    /// but omits it from `OrderPatchBody.AuthorizationStatus`. We send it on the
    /// Update Order call regardless: the PATCH schema reads as a trimmed
    /// projection of the same object, so Kount is expected to accept the
    /// superset. If Kount ever validates the patch body strictly, this is the
    /// field to drop first — it would fail the whole notify call, not just
    /// itself.
    #[serde(
        rename = "processorTransactionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub processor_transaction_id: Option<String>,
    /// Gateway that processed the authorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<KountGateway>,
}

/// Kount `transactions[].authorizationStatus.gateway`. Only `id` is populated
/// from the notify feature data; the schema also allows `response`.
#[derive(Debug, Clone, Serialize)]
pub struct KountGateway {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Kount authorization outcome (`transactions[].authorizationStatus.authResult`).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountAuthResult {
    Approved,
    Declined,
    Error,
    Unknown,
}

impl KountAuthResult {
    /// Map the internal attempt status to Kount's authResult. Reuses the same
    /// success/decline categorization as `KountPaymentStatus::from_attempt_status`
    /// (Charged/Authorized/Voided/Refunded-family => APPROVED, explicit
    /// failure/decline-family => DECLINED); anything else maps to `Unknown`
    /// rather than a guessed value.
    fn from_attempt_status(status: AttemptStatus) -> Self {
        match status {
            AttemptStatus::Charged
            | AttemptStatus::PartialCharged
            | AttemptStatus::PartialChargedAndChargeable
            | AttemptStatus::Authorized
            | AttemptStatus::PartiallyAuthorized
            | AttemptStatus::Voided
            | AttemptStatus::VoidedPostCapture
            | AttemptStatus::AutoRefunded => Self::Approved,
            AttemptStatus::Failure
            | AttemptStatus::AuthorizationFailed
            | AttemptStatus::CaptureFailed
            | AttemptStatus::RouterDeclined
            | AttemptStatus::AuthenticationFailed
            | AttemptStatus::VoidFailed => Self::Declined,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KountVerificationResponse {
    #[serde(rename = "cvvStatus", skip_serializing_if = "Option::is_none")]
    pub cvv_status: Option<KountCvvStatus>,
    /// Kount's single-letter AVS response code (e.g. "Y", "N"), passed through as-is.
    #[serde(rename = "avsStatus", skip_serializing_if = "Option::is_none")]
    pub avs_status: Option<String>,
}

/// Kount CVV verification result (`transactions[].authorizationStatus.verificationResponse.cvvStatus`).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KountCvvStatus {
    Match,
    NoMatch,
    Unknown,
}

impl KountCvvStatus {
    /// Lenient parse of a caller-supplied CVV result string. Accepts Kount's own
    /// tokens ("MATCH"/"NO_MATCH") case-insensitively; anything else maps
    /// to `Unknown` rather than failing the notify call.
    fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "MATCH" => Self::Match,
            "NO_MATCH" => Self::NoMatch,
            _ => Self::Unknown,
        }
    }
}

/// Connector-specific feature data accepted on the FRM notify request. Carries
/// the payment connector's authorization results so Kount's Update Order call
/// can relay them under `transactions[].authorizationStatus`: AVS/CVV as
/// `verificationResponse`, plus `processorAuthCode` and `gateway.id`.
///
/// Keys are snake_case here — unlike the camelCase Pre Risk Check blob
/// ([`KountPreRiskCheckFeatureData`]) — because the caller builds the two
/// objects separately. Do not "harmonise" the casing.
///
/// `processor_transaction_id` is relayed even though Kount omits
/// `processorTransactionId` from `OrderPatchBody.AuthorizationStatus` — see the
/// field's doc comment on [`KountAuthorizationStatus`].
///
/// Every field parses leniently through [`de_stringy`] so that one value
/// arriving as a JSON number (a numeric `gateway_id`, say) cannot fail the whole
/// struct and silently stop AVS/CVV from being relayed.
#[derive(Debug, Clone, Default, Deserialize)]
struct KountNotifyFeatureData {
    #[serde(default, deserialize_with = "de_stringy")]
    avs_result: Option<String>,
    #[serde(default, deserialize_with = "de_stringy")]
    cvv_result: Option<String>,
    #[serde(default, deserialize_with = "de_stringy")]
    processor_auth_code: Option<String>,
    #[serde(default, deserialize_with = "de_stringy")]
    processor_transaction_id: Option<String>,
    #[serde(default, deserialize_with = "de_stringy")]
    gateway_id: Option<String>,
}

/// Build the `transactions[].authorizationStatus` block from the payment status
/// (`authResult`) and the notify request's `connector_feature_data` — AVS/CVV as
/// `verificationResponse`, plus `processorAuthCode` and `gateway.id`. Empty when
/// none of those is present, matching the request's `skip_serializing_if`
/// convention.
fn kount_update_transactions(
    payment_status: Option<AttemptStatus>,
    connector_feature_data: Option<&Secret<String>>,
) -> Vec<KountUpdateTransaction> {
    let auth_result = payment_status.map(KountAuthResult::from_attempt_status);

    // Malformed `connector_feature_data` (bad JSON, or keys that don't match the
    // ones modelled above) would otherwise silently deserialize to an all-`None`
    // `KountNotifyFeatureData` — indistinguishable from a caller who simply sent
    // no authorization data. Surface the parse failure so a caller keying
    // mistake is visible instead of the block just vanishing from the call.
    let feature_data = connector_feature_data
        .and_then(|data| {
            serde_json::from_str::<KountNotifyFeatureData>(data.peek())
                .inspect_err(|err| {
                    tracing::warn!(
                        error = %err,
                        "Kount notify connector_feature_data failed to parse; AVS/CVV, \
                         processorAuthCode and gateway.id will not be relayed to Kount"
                    );
                })
                .ok()
        })
        .unwrap_or_default();

    let verification_response = (feature_data.avs_result.is_some()
        || feature_data.cvv_result.is_some())
    .then(|| KountVerificationResponse {
        cvv_status: feature_data
            .cvv_result
            .as_deref()
            .map(KountCvvStatus::from_str),
        avs_status: feature_data.avs_result,
    });
    let gateway = feature_data
        .gateway_id
        .map(|id| KountGateway { id: Some(id) });
    let processor_auth_code = feature_data.processor_auth_code;
    let processor_transaction_id = feature_data.processor_transaction_id;

    if auth_result.is_none()
        && verification_response.is_none()
        && processor_auth_code.is_none()
        && processor_transaction_id.is_none()
        && gateway.is_none()
    {
        return Vec::new();
    }
    vec![KountUpdateTransaction {
        authorization_status: Some(KountAuthorizationStatus {
            auth_result,
            verification_response,
            processor_auth_code,
            processor_transaction_id,
            gateway,
        }),
    }]
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<
                FrmPaymentOutcome,
                FrmFlowData,
                FrmPaymentOutcomeRequest,
                FrmPaymentOutcomeResponse,
            >,
            T,
        >,
    > for KountUpdateOrderRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
            RouterDataV2<
                FrmPaymentOutcome,
                FrmFlowData,
                FrmPaymentOutcomeRequest,
                FrmPaymentOutcomeResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let (merchant, merchant_category_code) = kount_merchant(req.merchant_details.as_ref());
        Ok(Self {
            merchant_transaction_id: req
                .merchant_transaction_id
                .clone()
                .or_else(|| req.connector_transaction_id.clone()),
            payment_status: req
                .payment_status
                .and_then(KountPaymentStatus::from_attempt_status),
            order_total: super::KountAmountConvertor::convert(
                req.amount.amount,
                req.amount.currency,
            )?,
            currency: req.amount.currency.to_string(),
            frm_disposition: req.frm_decision.and_then(KountDisposition::from_decision),
            merchant_category_code,
            merchant,
            transactions: kount_update_transactions(
                req.payment_status,
                req.connector_feature_data.as_ref(),
            ),
        })
    }
}

impl TryFrom<ResponseRouterData<KountFrmPaymentOutcomeResponse, Self>>
    for RouterDataV2<
        FrmPaymentOutcome,
        FrmFlowData,
        FrmPaymentOutcomeRequest,
        FrmPaymentOutcomeResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<KountFrmPaymentOutcomeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Surface the verbatim Kount notify body (see PreRiskCheck mapping).
        let raw_connector_response = serde_json::to_string(&item.response.0.raw_response)
            .ok()
            .map(Secret::new);
        Ok(Self {
            response: Ok(FrmPaymentOutcomeResponse {
                status_code: item.http_code,
            }),
            resource_common_data: FrmFlowData {
                raw_connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────
// FrmRefundProcessed (Notify: refund) = Update Order
// PATCH /commerce/v2/orders/{orderId}
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct KountRefundUpdateRequest {
    /// Connector (gateway) transaction id the refund belongs to.
    #[serde(
        rename = "merchantTransactionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_transaction_id: Option<String>,
    /// Connector refund id.
    #[serde(rename = "refundId", skip_serializing_if = "Option::is_none")]
    pub refund_id: Option<String>,
    /// Reason supplied for the refund.
    #[serde(rename = "refundReason", skip_serializing_if = "Option::is_none")]
    pub refund_reason: Option<String>,
    /// Refund amount in the smallest currency unit (string per Kount schema).
    #[serde(rename = "refundAmount")]
    pub refund_amount: StringMinorUnit,
    /// ISO 4217 currency code.
    pub currency: String,
    /// FRM decision being notified (APPROVE / DECLINE / REVIEW).
    #[serde(rename = "frmDisposition", skip_serializing_if = "Option::is_none")]
    pub frm_disposition: Option<KountDisposition>,
    /// Merchant Category Code (ISO 18245), when provided.
    #[serde(
        rename = "merchantCategoryCode",
        skip_serializing_if = "Option::is_none"
    )]
    pub merchant_category_code: Option<u32>,
    /// Merchant details (id), when provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant: Option<KountMerchant>,
}

/// Response from the refund Update Order PATCH — same order-envelope ack as
/// [`KountUpdateOrderResponse`]. Distinct type so the connector macros generate a
/// unique templating type per flow.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KountRefundUpdateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Device fingerprint id — masks in the event log.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_session_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_date_time: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        KountRouterData<
            RouterDataV2<
                FrmRefundProcessed,
                FrmFlowData,
                FrmRefundProcessedRequest,
                FrmRefundProcessedResponse,
            >,
            T,
        >,
    > for KountRefundUpdateRequest
{
    type Error = Error;

    fn try_from(
        item: KountRouterData<
            RouterDataV2<
                FrmRefundProcessed,
                FrmFlowData,
                FrmRefundProcessedRequest,
                FrmRefundProcessedResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let (merchant, merchant_category_code) = kount_merchant(req.merchant_details.as_ref());
        Ok(Self {
            merchant_transaction_id: req.connector_transaction_id.clone(),
            refund_id: req
                .connector_refund_id
                .clone()
                .or_else(|| req.merchant_refund_id.clone()),
            refund_reason: req.refund_reason.clone(),
            refund_amount: super::KountAmountConvertor::convert(
                req.amount.amount,
                req.amount.currency,
            )?,
            currency: req.amount.currency.to_string(),
            frm_disposition: req.frm_decision.and_then(KountDisposition::from_decision),
            merchant_category_code,
            merchant,
        })
    }
}

impl TryFrom<ResponseRouterData<KountFrmRefundProcessedResponse, Self>>
    for RouterDataV2<
        FrmRefundProcessed,
        FrmFlowData,
        FrmRefundProcessedRequest,
        FrmRefundProcessedResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<KountFrmRefundProcessedResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Surface the verbatim Kount notify body (see PreRiskCheck mapping).
        let raw_connector_response = serde_json::to_string(&item.response.0.raw_response)
            .ok()
            .map(Secret::new);
        Ok(Self {
            response: Ok(FrmRefundProcessedResponse {
                status_code: item.http_code,
            }),
            resource_common_data: FrmFlowData {
                raw_connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
