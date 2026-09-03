use base64::Engine;
use common_enums::{AttemptStatus, CountryAlpha2, Currency, RefundStatus};
use common_utils::{
    consts::BASE64_ENGINE_URL_SAFE_NO_PAD, pii::SecretSerdeValue, types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, PSync, RSync, Refund, ServerSessionAuthenticationToken,
    },
    connector_types::{
        EventType, PaymentWebhookReference, RawConnectorStatus, RefundWebhookReference,
        WebhookResourceReference,
    },
    connector_types::{
        PaymentFlowData, PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId, ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    },
    errors,
    errors::IntegrationErrorContext,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::grabpay::{
        oauth_endpoint, GrabpayRouterData as GrabpayFlowData, GRABPAY_CONFIG_SUGGESTED_ACTION,
        GRABPAY_DOC_URL,
    },
    types::ResponseRouterData as ConnectorResponseData,
    utils,
};

const AUTHORIZATION_CODE_GRANT: &str = "authorization_code";
const OAUTH_AUTHORIZE_PATH: &str = "/grabid/v1/oauth2/authorize";
const CODE_CHALLENGE_METHOD: &str = "S256";
const RESPONSE_TYPE_CODE: &str = "code";
const SCOPE_ONE_TIME_CHARGE: &str = "payment.one_time_charge";

fn grabpay_integration_context(additional_context: impl Into<String>) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(GRABPAY_CONFIG_SUGGESTED_ACTION.to_string()),
        doc_url: Some(GRABPAY_DOC_URL.to_string()),
        additional_context: Some(additional_context.into()),
    }
}

#[derive(Debug, Clone)]
pub struct GrabpayAuthType {
    pub partner_id: Secret<String>,
    pub partner_secret: Secret<String>,
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GrabpayAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Grabpay {
                partner_id,
                partner_secret,
                client_id,
                client_secret,
                merchant_id,
                ..
            } => Ok(Self {
                partner_id: partner_id.to_owned(),
                partner_secret: partner_secret.to_owned(),
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: grabpay_integration_context(
                        "GrabPay auth type can only be built from Grabpay connector configuration"
                    )
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrabpayErrorResponse {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub code: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub message: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub error: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub error_description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub reason: Option<String>,
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::String(value) => Some(value),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayAuthenticateRequest {
    #[serde(rename = "partnerGroupTxID")]
    pub partner_group_tx_id: String,
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: String,
    pub currency: Currency,
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "merchantID")]
    pub merchant_id: String,
    #[serde(rename = "shippingDetails", skip_serializing_if = "Option::is_none")]
    pub shipping_details: Option<GrabpayShippingDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<GrabpayItem>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayItem {
    #[serde(rename = "itemName")]
    pub item_name: String,
    pub quantity: u16,
    pub price: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(rename = "itemCategory", skip_serializing_if = "Option::is_none")]
    pub item_category: Option<String>,
    #[serde(rename = "imageURL", skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayShippingDetails {
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(rename = "countryCode", skip_serializing_if = "Option::is_none")]
    pub country_code: Option<CountryAlpha2>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayAuthenticateResponse {
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: Option<String>,
    pub request: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "txStatus")]
    pub tx_status: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrabpayConnectorFeatureData {
    pub state: Option<Secret<String>>,
    pub callback_state: Option<String>,
    pub code: Option<String>,
    /// GrabID authorization code recovered via the one-time-charge status
    /// inquiry (HMAC-authenticated server-to-server), as opposed to `code`
    /// which arrives via the browser redirect. Its presence signals that the
    /// CSRF state check does not apply.
    pub o_auth_code: Option<String>,
    pub nonce: Option<String>,
    pub code_verifier: Option<Secret<String>>,
    pub redirect_uri: Option<String>,
    pub partner_tx_id: Option<String>,
    pub currency: Option<Currency>,
    #[serde(rename = "txID")]
    pub tx_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: String,
    #[serde(skip)]
    pub phantom: std::marker::PhantomData<T>,
}

/// GrabPay's `/charge/complete` response — the post-redirect Authorize step.
/// The initial redirect is now owned by the `Authenticate` flow, so Authorize
/// only ever completes the charge.
pub type GrabpayAuthorizeResponse = GrabpayChargeCompleteResponse;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayChargeCompleteResponse {
    #[serde(rename = "txID")]
    pub tx_id: String,
    pub status: Option<String>,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "txStatus")]
    pub tx_status: GrabpayPaymentStatus,
    pub reason: Option<String>,
    // Only Grab's `/one-time-charge/{id}/status` inquiry returns this; the
    // `/charge/complete` and `/charge/{id}/status` responses do not.
    #[serde(rename = "oAuthCode", skip_serializing_if = "Option::is_none")]
    pub o_auth_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayRefundRequest {
    #[serde(rename = "partnerGroupTxID")]
    pub partner_group_tx_id: String,
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    #[serde(rename = "merchantID")]
    pub merchant_id: String,
    #[serde(rename = "originTxID")]
    pub origin_tx_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayRefundResponse {
    #[serde(rename = "txID")]
    pub tx_id: String,
    pub status: Option<String>,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "txStatus")]
    pub tx_status: GrabpayRefundStatus,
    pub reason: Option<String>,
    pub echo: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayRefundSyncResponse {
    #[serde(rename = "txID")]
    pub tx_id: String,
    pub status: Option<String>,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "txStatus")]
    pub tx_status: GrabpayRefundStatus,
    pub reason: Option<String>,
    pub echo: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, strum::Display, strum::EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum GrabpayPaymentStatus {
    Success,
    Failed,
    Processing,
    Cancelled,
    Authorised,
    AuthorisationDeclined,
    TransactionAlreadyExist,
    #[serde(untagged)]
    #[strum(default)]
    #[strum(to_string = "{0}")]
    Unknown(String),
}

impl From<GrabpayPaymentStatus> for AttemptStatus {
    fn from(status: GrabpayPaymentStatus) -> Self {
        match status {
            GrabpayPaymentStatus::Success => Self::Charged,
            GrabpayPaymentStatus::Failed
            | GrabpayPaymentStatus::Cancelled
            | GrabpayPaymentStatus::AuthorisationDeclined => Self::Failure,
            GrabpayPaymentStatus::Processing | GrabpayPaymentStatus::TransactionAlreadyExist => {
                Self::Pending
            }
            GrabpayPaymentStatus::Authorised => Self::Authorized,
            GrabpayPaymentStatus::Unknown(_) => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, strum::Display, strum::EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum GrabpayRefundStatus {
    Success,
    Failed,
    Cancelled,
    AuthorisationDeclined,
    Processing,
    TransactionAlreadyExist,
    #[serde(untagged)]
    #[strum(default)]
    #[strum(to_string = "{0}")]
    Unknown(String),
}

impl From<GrabpayRefundStatus> for RefundStatus {
    fn from(status: GrabpayRefundStatus) -> Self {
        match status {
            GrabpayRefundStatus::Success => Self::Success,
            GrabpayRefundStatus::Failed
            | GrabpayRefundStatus::Cancelled
            | GrabpayRefundStatus::AuthorisationDeclined => Self::Failure,
            GrabpayRefundStatus::Processing | GrabpayRefundStatus::TransactionAlreadyExist => {
                Self::Pending
            }
            GrabpayRefundStatus::Unknown(_) => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayWebhookBody {
    #[serde(rename = "txType")]
    pub tx_type: Option<String>,
    #[serde(rename = "txCategory")]
    pub tx_category: Option<String>,
    #[serde(rename = "txStatus")]
    pub tx_status: Option<String>,
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: Option<String>,
    #[serde(rename = "txID")]
    pub tx_id: Option<String>,
    #[serde(rename = "origTxID")]
    pub orig_tx_id: Option<String>,
    pub amount: Option<serde_json::Value>,
    pub currency: Option<Currency>,
    pub status: Option<String>,
    pub reason: Option<String>,
    pub payload: Option<GrabpayWebhookPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayWebhookPayload {
    #[serde(rename = "partnerGroupTxID")]
    pub partner_group_tx_id: Option<String>,
    #[serde(rename = "newStatus")]
    pub new_status: Option<String>,
    pub reason: Option<String>,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    pub echo: Option<String>,
}

impl GrabpayWebhookBody {
    pub fn is_refund_event(&self) -> bool {
        let tx_type_is_refund = self
            .tx_type
            .as_deref()
            .map(|tx_type| tx_type.to_ascii_lowercase().contains("refund"))
            .unwrap_or(false);
        let tx_category_is_refund = self
            .tx_category
            .as_deref()
            .map(|tx_category| tx_category.to_ascii_lowercase().contains("refund"))
            .unwrap_or(false);

        tx_type_is_refund || tx_category_is_refund
    }

    pub fn effective_status(&self) -> Option<&str> {
        self.payload
            .as_ref()
            .and_then(|payload| payload.new_status.as_deref())
            .or(self.tx_status.as_deref())
            .or(self.status.as_deref())
    }

    pub fn effective_reason(&self) -> Option<String> {
        self.payload
            .as_ref()
            .and_then(|payload| payload.reason.clone())
            .or_else(|| self.reason.clone())
    }

    pub fn webhook_reference(&self) -> WebhookResourceReference {
        if self.is_refund_event() {
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: self.tx_id.clone(),
                merchant_refund_id: self.partner_tx_id.clone(),
                connector_transaction_id: self.orig_tx_id.clone(),
                merchant_transaction_id: self
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.partner_group_tx_id.clone()),
            })
        } else {
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: self.tx_id.clone(),
                merchant_transaction_id: self.partner_tx_id.clone(),
            })
        }
    }
}

pub fn grabpay_webhook_event_type(
    webhook_body: &GrabpayWebhookBody,
) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
    if webhook_body.is_refund_event() {
        let status = parse_webhook_status::<GrabpayRefundStatus>(webhook_body)?;
        Ok(match status {
            GrabpayRefundStatus::Success => EventType::RefundSuccess,
            GrabpayRefundStatus::Failed
            | GrabpayRefundStatus::Cancelled
            | GrabpayRefundStatus::AuthorisationDeclined => EventType::RefundFailure,
            GrabpayRefundStatus::Processing | GrabpayRefundStatus::TransactionAlreadyExist => {
                EventType::RefundProcessing
            }
            GrabpayRefundStatus::Unknown(_) => EventType::IncomingWebhookEventUnspecified,
        })
    } else {
        let status = parse_webhook_status::<GrabpayPaymentStatus>(webhook_body)?;
        Ok(match status {
            GrabpayPaymentStatus::Success => EventType::PaymentIntentSuccess,
            GrabpayPaymentStatus::Failed
            | GrabpayPaymentStatus::Cancelled
            | GrabpayPaymentStatus::AuthorisationDeclined => EventType::PaymentIntentFailure,
            GrabpayPaymentStatus::Processing | GrabpayPaymentStatus::TransactionAlreadyExist => {
                EventType::PaymentIntentProcessing
            }
            GrabpayPaymentStatus::Authorised => EventType::PaymentIntentAuthorizationSuccess,
            GrabpayPaymentStatus::Unknown(_) => EventType::IncomingWebhookEventUnspecified,
        })
    }
}

pub fn grabpay_webhook_attempt_status(
    webhook_body: &GrabpayWebhookBody,
) -> Result<AttemptStatus, error_stack::Report<errors::WebhookError>> {
    let status = parse_webhook_status::<GrabpayPaymentStatus>(webhook_body)?;
    Ok(AttemptStatus::from(status))
}

pub fn grabpay_webhook_refund_status(
    webhook_body: &GrabpayWebhookBody,
) -> Result<RefundStatus, error_stack::Report<errors::WebhookError>> {
    let status = parse_webhook_status::<GrabpayRefundStatus>(webhook_body)?;
    Ok(RefundStatus::from(status))
}

fn parse_webhook_status<T>(
    webhook_body: &GrabpayWebhookBody,
) -> Result<T, error_stack::Report<errors::WebhookError>>
where
    T: std::str::FromStr,
{
    let status = webhook_body
        .effective_status()
        .ok_or_else(|| error_stack::report!(errors::WebhookError::WebhookBodyDecodingFailed))?;
    status
        .trim()
        .parse::<T>()
        .map_err(|_| error_stack::report!(errors::WebhookError::WebhookBodyDecodingFailed))
}

#[derive(Debug, Clone, Deserialize)]
struct GrabpayRefundMetadata {
    echo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrabpayServerSessionAuthenticationTokenRequest {
    pub grant_type: String,
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub code: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrabpayServerSessionAuthenticationTokenResponse {
    pub access_token: Secret<String>,
    pub id_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
}

struct GrabpayRedirectContext {
    redirect_url: String,
    connector_feature_data: SecretSerdeValue,
}

fn build_code_challenge(
    code_verifier: &str,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    use common_utils::crypto::GenerateDigest;

    let digest = common_utils::crypto::Sha256
        .generate_digest(code_verifier.as_bytes())
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: grabpay_integration_context(
                "GrabPay redirect authorize failed to generate OAuth code challenge",
            ),
        })?;
    Ok(BASE64_ENGINE_URL_SAFE_NO_PAD.encode(digest))
}

fn random_token(length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), length)
}

fn grabpay_request_currency(
    currency: Option<Currency>,
) -> Result<Currency, error_stack::Report<errors::IntegrationError>> {
    currency.ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "currency",
            context: grabpay_integration_context(
                "GrabPay Authenticate requires a currency on the payment request"
            )
        })
    })
}

fn grabpay_billing_country(
    flow_data: &PaymentFlowData,
) -> Result<CountryAlpha2, error_stack::Report<errors::IntegrationError>> {
    flow_data.get_optional_billing_country().ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "billing.address.country",
            context: grabpay_integration_context(
                "Provide billing address country in the payment request".to_string(),
            )
        })
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GrabpayFlowData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for GrabpayServerSessionAuthenticationTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: GrabpayFlowData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl
    TryFrom<
        &RouterDataV2<
            ServerSessionAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
    > for GrabpayServerSessionAuthenticationTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            ServerSessionAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth =
            GrabpayAuthType::try_from(&router_data.connector_config)
                .change_context(errors::IntegrationError::FailedToObtainAuthType {
                context: grabpay_integration_context(
                    "GrabPay OAuth session-token request requires GrabPay connector configuration",
                ),
            })?;
        let feature_data = parse_connector_feature_data(
            router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        )?;

        // Prefer the redirect-derived code; fall back to the codes recovered
        // from the one-time-charge status inquiry (HMAC-fallback PSync).
        // Ignore empty strings: `code` may carry an empty-JSON-string default
        // ("") from upstream plumbing, which must not shadow the fallback.
        let code_from_redirect = feature_data.code.as_deref().filter(|code| !code.is_empty());
        let code = code_from_redirect
            .or(feature_data
                .o_auth_code
                .as_deref()
                .filter(|code| !code.is_empty()))
            .map(str::to_string)
            .ok_or_else(missing_oauth_code_error)?;

        // The state check is OAuth CSRF protection for browser-redirect
        // flows. When the code was recovered via the OTC status inquiry
        // (HMAC-authenticated server-to-server, no redirect), skip it.
        if code_from_redirect.is_some() {
            validate_callback_state(&feature_data)?;
        }

        Ok(Self {
            grant_type: AUTHORIZATION_CODE_GRANT.to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
            code_verifier: required_feature_field(feature_data.code_verifier, "code_verifier")?
                .peek()
                .to_string(),
            redirect_uri: required_feature_field(feature_data.redirect_uri, "redirect_uri")?,
            code,
        })
    }
}

impl TryFrom<ConnectorResponseData<GrabpayServerSessionAuthenticationTokenResponse, Self>>
    for RouterDataV2<
        ServerSessionAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayServerSessionAuthenticationTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Err(msg) = validate_id_token_nonce(
            &item.response.id_token,
            &item.router_data.resource_common_data.connector_feature_data,
        ) {
            return Err(errors::ConnectorError::ResponseDeserializationFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(msg),
                },
            }
            .into());
        }

        Ok(Self {
            response: Ok(ServerSessionAuthenticationTokenResponseData {
                session_token: item.response.access_token.peek().to_string(),
            }),
            ..item.router_data
        })
    }
}

fn extract_jwt_nonce(id_token: &str) -> Option<String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let payload = id_token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    claims.get("nonce")?.as_str().map(String::from)
}

fn validate_id_token_nonce(
    id_token: &Option<String>,
    connector_feature_data: &Option<SecretSerdeValue>,
) -> Result<(), String> {
    let (Some(id_token), Some(feature_data)) = (id_token.as_ref(), connector_feature_data.as_ref())
    else {
        return Ok(());
    };
    let expected_nonce = parse_connector_feature_data(Some(feature_data))
        .ok()
        .and_then(|fd| fd.nonce);
    let Some(expected) = expected_nonce else {
        return Ok(());
    };
    let actual = extract_jwt_nonce(id_token);
    if actual.as_deref() == Some(expected.as_str()) {
        Ok(())
    } else {
        Err("GrabPay id_token nonce mismatch".to_string())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GrabpayFlowData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GrabpayAuthorizeRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: GrabpayFlowData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for GrabpayAuthorizeRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        router_data: RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let feature_data = parse_connector_feature_data(
            router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        )?;
        let partner_tx_id = feature_data.partner_tx_id.unwrap_or(
            router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        validate_partner_tx_id(&partner_tx_id)?;

        Ok(Self {
            partner_tx_id,
            phantom: std::marker::PhantomData,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ConnectorResponseData<GrabpayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let session_token = item
            .router_data
            .resource_common_data
            .get_session_token()
            .ok();
        let response = item.response;
        let status = AttemptStatus::from(response.tx_status.clone());
        let resource_id = match item.router_data.response.as_ref() {
            Ok(PaymentsResponseData::TransactionResponse { resource_id, .. }) => {
                resource_id.clone()
            }
            _ => ResponseId::ConnectorTransactionId(response.tx_id.clone()),
        };
        let connector_metadata = build_complete_connector_feature_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
            &response,
            session_token.as_deref(),
        );
        let raw_connector_status = grabpay_raw_connector_status(&response);
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                connector_metadata: Some(connector_metadata),
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ConnectorResponseData<GrabpayChargeCompleteResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayChargeCompleteResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let session_token = item
            .router_data
            .resource_common_data
            .get_session_token()
            .ok();
        let response = item.response;
        let status = AttemptStatus::from(response.tx_status.clone());
        let resource_id = match item.router_data.response.as_ref() {
            Ok(PaymentsResponseData::TransactionResponse { resource_id, .. }) => {
                resource_id.clone()
            }
            _ => ResponseId::ConnectorTransactionId(response.tx_id.clone()),
        };
        let connector_metadata = build_complete_connector_feature_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
            &response,
            session_token.as_deref(),
        );
        let raw_connector_status = grabpay_raw_connector_status(&response);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                connector_metadata: Some(connector_metadata),
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(
                    item.router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GrabpayFlowData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for GrabpayRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: GrabpayFlowData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;
        let auth = GrabpayAuthType::try_from(&router_data.connector_config).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: grabpay_integration_context(
                    "GrabPay Refund request requires GrabPay connector configuration",
                ),
            },
        )?;
        let partner_tx_id = router_data.request.refund_id;
        validate_partner_tx_id(&partner_tx_id)?;
        let origin_tx_id = charge_tx_id_from_connector_feature_data(
            router_data.resource_common_data.connector_feature_data.as_ref(),
        )
        .or_else(|_| {
            required_string(
                router_data.request.connector_order_id.clone(),
                "connector_order_id",
                "GrabPay Refund requires the original charge txID as originTxID in connector_order_id",
            )
        })?;
        let echo = router_data
            .request
            .refund_connector_metadata
            .and_then(|metadata| {
                utils::to_connector_meta_from_secret::<GrabpayRefundMetadata>(Some(metadata))
                    .ok()
                    .and_then(|metadata| metadata.echo)
            });

        Ok(Self {
            partner_group_tx_id: router_data.request.connector_transaction_id,
            partner_tx_id,
            amount: router_data.request.minor_refund_amount,
            currency: router_data.request.currency,
            merchant_id: auth.merchant_id.peek().to_string(),
            origin_tx_id,
            description: router_data.request.reason,
            echo,
        })
    }
}

impl TryFrom<ConnectorResponseData<GrabpayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let refund_status = RefundStatus::from(response.tx_status.clone());
        let raw_connector_status = grabpay_refund_raw_connector_status(
            &response.tx_status,
            response.description.clone(),
            response.reason.clone(),
        );

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.router_data.request.refund_id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ConnectorResponseData<GrabpayRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let refund_status = RefundStatus::from(response.tx_status.clone());
        let raw_connector_status = grabpay_refund_raw_connector_status(
            &response.tx_status,
            response.description.clone(),
            response.reason.clone(),
        );

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.router_data.request.connector_refund_id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GrabpayFlowData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GrabpayAuthenticateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: GrabpayFlowData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    > for GrabpayAuthenticateRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        router_data: RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        match &router_data.request.payment_method_data {
            Some(PaymentMethodData::Wallet(WalletData::GrabpayRedirect {})) => {}
            other => {
                return Err(error_stack::report!(errors::IntegrationError::NotImplemented(
                    format!("GrabPay only supports Wallet(GrabpayRedirect); received {other:?}"),
                    grabpay_integration_context(
                        "GrabPay Authenticate only accepts PaymentMethodData::Wallet(WalletData::GrabpayRedirect {{}})",
                    ),
                )));
            }
        }

        let auth = GrabpayAuthType::try_from(&router_data.connector_config).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: grabpay_integration_context(
                    "GrabPay Authenticate request requires GrabPay connector configuration",
                ),
            },
        )?;
        let shipping_details = build_shipping_details(&router_data.resource_common_data);
        let items = build_items(&router_data.resource_common_data.order_details);
        let partner_tx_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        validate_partner_tx_id(&partner_tx_id)?;
        let currency = grabpay_request_currency(router_data.request.currency)?;
        grabpay_billing_country(&router_data.resource_common_data)?;

        Ok(Self {
            partner_group_tx_id: partner_tx_id.clone(),
            partner_tx_id,
            currency,
            amount: router_data.request.amount,
            description: router_data.resource_common_data.description,
            merchant_id: auth.merchant_id.peek().to_string(),
            shipping_details,
            items,
        })
    }
}

fn build_items(
    order_details: &Option<Vec<domain_types::payment_address::OrderDetailsWithAmount>>,
) -> Option<Vec<GrabpayItem>> {
    let items = order_details
        .as_ref()?
        .iter()
        .filter(|detail| !detail.product_name.is_empty() && detail.quantity > 0)
        .map(|detail| GrabpayItem {
            item_name: detail.product_name.clone(),
            quantity: detail.quantity,
            price: detail.amount,
            category: detail.category.clone(),
            item_category: detail.sub_category.clone(),
            image_url: detail.product_img_link.clone(),
        })
        .collect::<Vec<_>>();

    (!items.is_empty()).then_some(items)
}

fn build_shipping_details(flow_data: &PaymentFlowData) -> Option<GrabpayShippingDetails> {
    flow_data.get_optional_shipping().and_then(|_| {
        let shipping_details = GrabpayShippingDetails {
            first_name: flow_data.get_optional_shipping_first_name(),
            last_name: flow_data.get_optional_shipping_last_name(),
            address: build_shipping_address(flow_data),
            city: flow_data.get_optional_shipping_city(),
            postal_code: flow_data.get_optional_shipping_zip(),
            phone: flow_data.get_optional_shipping_phone_number(),
            email: flow_data.get_optional_shipping_email(),
            country_code: flow_data.get_optional_shipping_country(),
        };

        shipping_details.has_any_field().then_some(shipping_details)
    })
}

impl GrabpayShippingDetails {
    fn has_any_field(&self) -> bool {
        self.first_name.is_some()
            || self.last_name.is_some()
            || self.address.is_some()
            || self.city.is_some()
            || self.postal_code.is_some()
            || self.phone.is_some()
            || self.email.is_some()
            || self.country_code.is_some()
    }
}

fn build_shipping_address(flow_data: &PaymentFlowData) -> Option<Secret<String>> {
    let line1 = flow_data
        .get_optional_shipping_line1()
        .map(|line1| line1.expose());
    let line2 = flow_data
        .get_optional_shipping_line2()
        .map(|line2| line2.expose());

    match (line1, line2) {
        (Some(line1), Some(line2)) if !line2.is_empty() => {
            Some(Secret::new(format!("{line1}, {line2}")))
        }
        (Some(line1), _) => Some(Secret::new(line1)),
        (None, Some(line2)) => Some(Secret::new(line2)),
        (None, None) => None,
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ConnectorResponseData<GrabpayAuthenticateResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let request_code = response.request.ok_or_else(|| {
            error_stack::report!(errors::ConnectorError::ResponseHandlingFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(format!(
                        "GrabPay Authenticate did not return request code; partner_tx_id={:?}, status={:?}, tx_status={:?}, reason={:?}, message={:?}, code={:?}",
                        response.partner_tx_id,
                        response.status,
                        response.tx_status,
                        response.reason,
                        response.message,
                        response.code,
                    )),
                },
            })
        })?;
        let partner_tx_id = response.partner_tx_id.unwrap_or_else(|| {
            item.router_data
                .resource_common_data
                .connector_request_reference_id
                .clone()
        });
        validate_partner_tx_id(&partner_tx_id).change_context(
            errors::ConnectorError::ResponseHandlingFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "GrabPay Authenticate returned an invalid partnerTxID".to_string(),
                    ),
                },
            },
        )?;
        let redirect_context =
            build_authenticate_redirect_context(&item.router_data, &request_code, &partner_tx_id)
                .change_context(errors::ConnectorError::ResponseHandlingFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "GrabPay Authenticate failed to build OAuth redirect URL".to_string(),
                    ),
                },
            })?;

        Ok(Self {
            response: Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(partner_tx_id.clone())),
                redirection_data: Some(Box::new(RedirectForm::Uri {
                    uri: redirect_context.redirect_url,
                })),
                authentication_data: None,
                connector_feature_data: Some(redirect_context.connector_feature_data.expose()),
                connector_response_reference_id: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                reference_id: Some(partner_tx_id.clone()),
                connector_order_id: Some(partner_tx_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

fn build_authenticate_redirect_context<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >,
    request_code: &str,
    partner_tx_id: &str,
) -> Result<GrabpayRedirectContext, error_stack::Report<errors::IntegrationError>> {
    let auth = GrabpayAuthType::try_from(&router_data.connector_config).change_context(
        errors::IntegrationError::FailedToObtainAuthType {
            context: grabpay_integration_context(
                "GrabPay Authenticate redirect requires GrabPay connector configuration",
            ),
        },
    )?;
    let redirect_uri = router_data
        .resource_common_data
        .return_url
        .clone()
        .or_else(|| {
            router_data
                .request
                .router_return_url
                .as_ref()
                .map(ToString::to_string)
        })
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: grabpay_integration_context(
                    "GrabPay OAuth authorize URL requires a redirect_uri".to_string(),
                )
            })
        })?;
    let currency = grabpay_request_currency(router_data.request.currency)?;
    let country = grabpay_billing_country(&router_data.resource_common_data)?.to_string();
    let state = random_token(32);
    let nonce = random_token(32);
    let code_verifier = random_token(64);
    let code_challenge = build_code_challenge(&code_verifier)?;
    let acr_values = format!("consent_ctx:countryCode={country},currency={currency}");

    let authorize_endpoint = oauth_endpoint(
        &router_data.resource_common_data.connectors.grabpay.base_url,
        OAUTH_AUTHORIZE_PATH,
    );
    let mut url = url::Url::parse(&authorize_endpoint).change_context(
        errors::IntegrationError::RequestEncodingFailed {
            context: grabpay_integration_context(
                "GrabPay Authenticate redirect failed to parse OAuth authorize endpoint",
            ),
        },
    )?;
    url.query_pairs_mut()
        .append_pair("acr_values", &acr_values)
        .append_pair("client_id", auth.client_id.peek())
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", CODE_CHALLENGE_METHOD)
        .append_pair("nonce", &nonce)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("request", request_code)
        .append_pair("response_type", RESPONSE_TYPE_CODE)
        .append_pair("scope", SCOPE_ONE_TIME_CHARGE)
        .append_pair("state", &state);

    let connector_feature_data = SecretSerdeValue::new(serde_json::json!({
        "state": state,
        "nonce": nonce,
        "code_verifier": code_verifier,
        "redirect_uri": redirect_uri,
        "partner_tx_id": partner_tx_id,
        "currency": currency,
    }));

    Ok(GrabpayRedirectContext {
        redirect_url: url.to_string(),
        connector_feature_data,
    })
}

pub(crate) fn validate_partner_tx_id(
    partner_tx_id: &str,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    let is_valid = !partner_tx_id.is_empty()
        && partner_tx_id.len() <= 32
        && partner_tx_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));

    if is_valid {
        Ok(())
    } else {
        Err(error_stack::report!(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "connector_request_reference_id",
                context: grabpay_integration_context(
                    "GrabPay partnerTxID must be non-empty and at most 32 ASCII alphanumeric, hyphen, or underscore characters"
                )
            }
        ))
    }
}

fn parse_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<GrabpayConnectorFeatureData, error_stack::Report<errors::IntegrationError>> {
    match connector_feature_data {
        Some(data) => utils::to_connector_meta_from_secret(Some(data.clone())),
        None => Ok(GrabpayConnectorFeatureData {
            state: None,
            callback_state: None,
            code: None,
            o_auth_code: None,
            nonce: None,
            code_verifier: None,
            redirect_uri: None,
            partner_tx_id: None,
            currency: None,
            tx_id: None,
        }),
    }
}

pub(crate) fn currency_from_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<Currency, error_stack::Report<errors::IntegrationError>> {
    let feature_data = parse_connector_feature_data(connector_feature_data)?;
    feature_data.currency.ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "connector_feature_data.currency",
            context: grabpay_integration_context("GrabPay RSync requires either refund_amount.currency or connector_feature_data.currency"
                        .to_string(),),
        })
    })
}

fn charge_tx_id_from_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    let feature_data = parse_connector_feature_data(connector_feature_data)?;
    required_feature_field(feature_data.tx_id, "txID")
}

/// Fills the opt-in `raw_connector_status` channel with Grab's granular status
/// (`txStatus`, `reason`, `description`) alongside the unified attempt status,
/// for callers that consume the typed connector status codes.
fn grabpay_raw_connector_status(response: &GrabpayChargeCompleteResponse) -> RawConnectorStatus {
    RawConnectorStatus {
        code: Some(response.tx_status.to_string()),
        message: response.description.clone(),
        reason: response.reason.clone(),
    }
}

/// Same as `grabpay_raw_connector_status` but for the refund and refund-sync
/// responses, which share the same `txStatus`/`reason`/`description` shape.
fn grabpay_refund_raw_connector_status(
    tx_status: &GrabpayRefundStatus,
    description: Option<String>,
    reason: Option<String>,
) -> RawConnectorStatus {
    RawConnectorStatus {
        code: Some(tx_status.to_string()),
        message: description,
        reason,
    }
}

fn build_complete_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
    response: &GrabpayChargeCompleteResponse,
    session_token: Option<&str>,
) -> serde_json::Value {
    let mut connector_metadata = connector_feature_data
        .and_then(|data| {
            utils::to_connector_meta_from_secret::<serde_json::Value>(Some(data.clone())).ok()
        })
        .unwrap_or_else(|| serde_json::json!({}));

    if !connector_metadata.is_object() {
        connector_metadata = serde_json::json!({});
    }

    let Some(metadata) = connector_metadata.as_object_mut() else {
        return connector_metadata;
    };

    metadata.insert("txID".to_string(), serde_json::json!(response.tx_id));
    metadata.insert("status".to_string(), serde_json::json!(response.status));
    metadata.insert(
        "paymentMethod".to_string(),
        serde_json::json!(response.payment_method),
    );
    metadata.insert(
        "description".to_string(),
        serde_json::json!(response.description),
    );
    metadata.insert("reason".to_string(), serde_json::json!(response.reason));
    if let Some(session_token) = session_token {
        metadata.insert(
            "session_token".to_string(),
            serde_json::json!(session_token),
        );
    }

    // Grab's `/one-time-charge/{id}/status` inquiry returns the GrabID
    // authorization code after user consent — crucial in the HMAC-fallback
    // PSync path where the redirect callback was missed. Persist it under
    // `o_auth_code` (not `code`) so the token-exchange transformer can tell
    // it apart from a redirect-derived code and skip the CSRF state check.
    // Grab returns `"oAuthCode": ""` when consent is pending; skip empties.
    if let Some(code) = response
        .o_auth_code
        .as_deref()
        .filter(|code| !code.is_empty())
    {
        metadata.insert("o_auth_code".to_string(), serde_json::json!(code));
    }

    // Strip OAuth-only fields that are dead once the token exchange has
    // completed — detected by a session token being passed in or already
    // persisted in metadata. Before authorization (HMAC-fallback PSync)
    // the exchange is still pending, so the fields — and the recovered
    // `o_auth_code` above — are kept redeemable. Order matters: this strip
    // runs after the `o_auth_code` insert so a pre-existing session token
    // always wins.
    if session_token.is_some() || metadata.get("session_token").is_some() {
        metadata.remove("code_verifier");
        metadata.remove("nonce");
        metadata.remove("state");
        metadata.remove("code");
        metadata.remove("o_auth_code");
        metadata.remove("callback_state");
        metadata.remove("redirect_uri");
    }

    connector_metadata
}

fn validate_callback_state(
    feature_data: &GrabpayConnectorFeatureData,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    let expected_state = required_feature_field(feature_data.state.clone(), "state")?;
    let callback_state =
        required_feature_field(feature_data.callback_state.clone(), "callback_state")?;

    if expected_state.peek().as_str() == callback_state.as_str() {
        Ok(())
    } else {
        Err(error_stack::report!(errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.callback_state",
            context: grabpay_integration_context("GrabPay OAuth callback state does not match the state generated during initial authorization"
                        .to_string(),)
        }))
    }
}

fn required_feature_field<T>(
    value: Option<T>,
    field_name: &'static str,
) -> Result<T, error_stack::Report<errors::IntegrationError>> {
    value.ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name,
            context: grabpay_integration_context(format!(
                "GrabPay OAuth token exchange requires connector_feature_data.{field_name}"
            ))
        })
    })
}

fn required_string(
    value: Option<String>,
    field_name: &'static str,
    message: &'static str,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    value.ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name,
            context: grabpay_integration_context(message.to_string())
        })
    })
}

fn missing_oauth_code_error() -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data.code",
        context: grabpay_integration_context(
            "GrabPay OAuth code is not available yet; skipping token exchange request".to_string(),
        )
    })
}
