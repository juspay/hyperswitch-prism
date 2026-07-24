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
    payment_address::Address,
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
/// value on the token request. `auth_server_id` is the account/environment
/// specific OAuth authorization-server id (sandbox vs production differ).
#[derive(Debug, Clone)]
pub struct KountAuthType {
    pub api_key: Secret<String>,
    pub auth_server_id: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for KountAuthType {
    type Error = Error;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Kount {
                api_key,
                auth_server_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
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
                            "Send the Kount connector config (api_key, optional auth_server_id) \
                             for Kount FRM flows"
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_url: Option<String>,
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
}

/// Kount `merchant` object. Only `id` for now (matches the FRM `MerchantDetails`
/// contract); the Kount schema also allows name/storeName/websiteUrl/etc.
#[derive(Debug, Clone, Serialize)]
pub struct KountMerchant {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Whether the account is active. Omitted when unknown (the FRM request
    /// carries no account-status signal).
    #[serde(rename = "accountIsActive", skip_serializing_if = "Option::is_none")]
    pub account_is_active: Option<bool>,
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
    /// Card BIN (first 6 digits) — no full PAN is sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last4: Option<String>,
    /// Stable, salted hash of the payment instrument (never the raw PAN). Lets
    /// Kount link/score the instrument across orders. See [`payment_token_hash`].
    #[serde(rename = "paymentToken", skip_serializing_if = "Option::is_none")]
    pub payment_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountPerson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<KountName>,
    #[serde(rename = "emailAddress", skip_serializing_if = "Option::is_none")]
    pub email_address: Option<String>,
    #[serde(rename = "phoneNumber", skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<KountAddress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KountName {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

impl KountName {
    /// Build a name only when at least one part is present, so we never emit an
    /// empty `{}` name object.
    fn from_parts(first: Option<String>, last: Option<String>) -> Option<Self> {
        (first.is_some() || last.is_some()).then_some(Self { first, last })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KountAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(rename = "countryCode", skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
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
        line1: details.line1.as_ref().map(|line| line.peek().to_string()),
        line2: details.line2.as_ref().map(|line| line.peek().to_string()),
        city: details.city.as_ref().map(|city| city.peek().to_string()),
        region: details.state.as_ref().map(|state| state.peek().to_string()),
        country_code: details.country.map(|country| country.to_string()),
        postal_code: details.zip.as_ref().map(|zip| zip.peek().to_string()),
    });
    let email_address = addr.email.as_ref().map(|email| email.peek().to_string());
    let phone_number = addr
        .phone
        .as_ref()
        .and_then(|phone| phone.number.as_ref().map(|num| num.peek().to_string()));
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

/// Fallback person block from customer info (name / email / phone, no address).
/// `None` when the customer carries no usable fields.
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
        .map(|email| email.peek().to_string());
    let phone_number = customer
        .customer_phone_number
        .as_ref()
        .map(|phone| phone.peek().to_string());
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
            }),
            details.merchant_category_code,
        ),
        None => (None, None),
    }
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
            username: customer
                .customer_email
                .as_ref()
                .map(|email| email.peek().to_string())
                .or_else(|| customer.get_full_name().map(|name| name.peek().to_string())),
            // No account-status signal in the FRM request, so leave it unset.
            account_is_active: None,
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

        // Billing / shipping persons from the payment address.
        let address = req.address.as_ref();
        let billed_person = address
            .and_then(|addr| {
                addr.get_payment_billing()
                    .or_else(|| addr.get_payment_method_billing())
            })
            .and_then(kount_person_from_address)
            .or_else(|| {
                req.customer_info
                    .as_ref()
                    .and_then(kount_person_from_customer)
            });
        // Fulfillment recipient: identity (name / email / phone) from customer
        // info, with the order address attached (customer info has no address).
        // Prefer the shipping address; fall back to billing, since the FRM request
        // carries a single address that maps to billing (shipping is unset).
        let recipient_address = address
            .and_then(|addr| addr.get_shipping().or_else(|| addr.get_payment_billing()))
            .and_then(kount_person_from_address)
            .and_then(|person| person.address);
        let fulfillment = req
            .customer_info
            .as_ref()
            .and_then(kount_person_from_customer)
            .map(|mut recipient| {
                recipient.address = recipient_address;
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
        let payment = req.payment_method.as_ref().and_then(|pm| {
            kount_instrument(pm, req.payment_method_type).map(|instrument| {
                // Cards carry BIN/last4 + a PAN-derived token; the type reflects credit/debit.
                let (bin, last4, payment_token) = match pm {
                    PaymentMethodData::Card(card) => {
                        let pan = card.card_number.peek();
                        let (bin, last4) = card_bin_last4(pan);
                        (bin, last4, salted_token(pan))
                    }
                    // Non-card methods: salted token of the instrument identifier
                    // (payer email, IBAN, account number, …) when one is available.
                    _ => (
                        None,
                        None,
                        instrument.token_source.as_deref().and_then(salted_token),
                    ),
                };
                KountPayment {
                    payment_type: instrument.payment_type,
                    bin,
                    last4,
                    payment_token,
                }
            })
        });

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
        })
    }
}

impl TryFrom<ResponseRouterData<KountOrderResponse, Self>>
    for RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>
{
    type Error = ResponseError;

    fn try_from(item: ResponseRouterData<KountOrderResponse, Self>) -> Result<Self, Self::Error> {
        // Always surface the raw Kount response (independent of the global
        // `return_raw_connector_data` flag), mirroring twoc_twop_paco / ppro.
        // Mask through the PII serializer so the stored/egressed blob never carries
        // cleartext card/PII — a plain `serde_json::to_string` emits `Secret` fields
        // exposed. `None` on serialization failure (degrades the audit trail rather
        // than failing the flow).
        let raw_connector_response = hyperswitch_masking::masked_serialize(&item.response)
            .ok()
            .map(|value| Secret::new(value.to_string()));
        let order = item.response.order.as_ref();
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

// ──────────────────────────────────────────────────────────────────────────
// Notify flows (FrmPaymentOutcome / FrmRefundProcessed) = Update Order
// PATCH /commerce/v2/orders/{orderId}
// ──────────────────────────────────────────────────────────────────────────

/// Response from the Kount Orders API update (`PATCH /commerce/v2/orders/{id}`).
/// Distinct type from [`KountOrderResponse`] so the connector macros generate a
/// unique templating type per flow.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountUpdateOrderResponse {
    #[serde(alias = "orderId")]
    pub order_id: Option<String>,
    pub decision: Option<KountDecision>,
    #[serde(alias = "omniscore", alias = "riskScore")]
    pub score: Option<f64>,
    pub reason: Option<String>,
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
        })
    }
}

impl TryFrom<ResponseRouterData<KountUpdateOrderResponse, Self>>
    for RouterDataV2<
        FrmPaymentOutcome,
        FrmFlowData,
        FrmPaymentOutcomeRequest,
        FrmPaymentOutcomeResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<KountUpdateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(FrmPaymentOutcomeResponse {
                status_code: item.http_code,
            }),
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

/// Response from the refund Update Order PATCH. Distinct type from
/// [`KountUpdateOrderResponse`] so the connector macros generate a unique
/// templating type per flow.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KountRefundUpdateResponse {
    #[serde(alias = "orderId")]
    pub order_id: Option<String>,
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

impl TryFrom<ResponseRouterData<KountRefundUpdateResponse, Self>>
    for RouterDataV2<
        FrmRefundProcessed,
        FrmFlowData,
        FrmRefundProcessedRequest,
        FrmRefundProcessedResponse,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<KountRefundUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(FrmRefundProcessedResponse {
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
