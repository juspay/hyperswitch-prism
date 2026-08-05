use base64::Engine;
use common_enums::{AttemptStatus, CountryAlpha2, Currency, RefundStatus};
use common_utils::{consts::BASE64_ENGINE_URL_SAFE_NO_PAD, pii::SecretSerdeValue};
use domain_types::{
    connector_flow::{Authorize, CreateOrder, PSync, RSync, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    errors,
    errors::IntegrationErrorContext,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::grabpay::{oauth_endpoint, GrabpayRouterData as GrabpayFlowData},
    types::ResponseRouterData as ConnectorResponseData,
    utils,
};

const AUTHORIZATION_CODE_GRANT: &str = "authorization_code";
const OAUTH_AUTHORIZE_PATH: &str = "/grabid/v1/oauth2/authorize";
const CODE_CHALLENGE_METHOD: &str = "S256";
const RESPONSE_TYPE_CODE: &str = "code";
const SCOPE_ONE_TIME_CHARGE: &str = "payment.one_time_charge";

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
                    context: Default::default()
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
pub struct GrabpayCreateOrderRequest {
    #[serde(rename = "partnerGroupTxID")]
    pub partner_group_tx_id: String,
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: String,
    pub currency: Currency,
    pub amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "merchantID")]
    pub merchant_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayCreateOrderResponse {
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
    pub state: Option<String>,
    pub callback_state: Option<String>,
    pub code: Option<String>,
    pub nonce: Option<String>,
    pub code_verifier: Option<String>,
    pub redirect_uri: Option<String>,
    pub partner_tx_id: Option<String>,
    pub currency: Option<Currency>,
    pub request_code: Option<String>,
    #[serde(rename = "txID")]
    pub tx_id: Option<String>,
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in_seconds: Option<i64>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GrabpayAuthorizeResponse {
    Redirect(GrabpayRedirectAuthorizeResponse),
    Complete(GrabpayChargeCompleteResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrabpayRedirectAuthorizeResponse {
    pub redirect_url: String,
    pub connector_feature_data: serde_json::Value,
    pub partner_tx_id: String,
}

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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayRefundRequest {
    #[serde(rename = "partnerGroupTxID")]
    pub partner_group_tx_id: String,
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: String,
    pub amount: i64,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrabpayPaymentStatus {
    Success,
    Failed,
    Processing,
    Cancelled,
    Authorised,
    AuthorisationDeclined,
    TransactionAlreadyExist,
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
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrabpayRefundStatus {
    Success,
    Failed,
    Processing,
    TransactionAlreadyExist,
}

impl From<GrabpayRefundStatus> for RefundStatus {
    fn from(status: GrabpayRefundStatus) -> Self {
        match status {
            GrabpayRefundStatus::Success => Self::Success,
            GrabpayRefundStatus::Failed => Self::Failure,
            GrabpayRefundStatus::Processing | GrabpayRefundStatus::TransactionAlreadyExist => {
                Self::Pending
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct GrabpayRefundMetadata {
    echo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrabpayServerAuthenticationTokenRequest {
    pub grant_type: String,
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub code: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrabpayServerAuthenticationTokenResponse {
    pub access_token: Secret<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
}

impl GrabpayAuthorizeResponse {
    pub fn redirect<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
    >(
        router_data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, error_stack::Report<errors::IntegrationError>> {
        let redirect_context = build_redirect_context(router_data)?;

        Ok(Self::Redirect(GrabpayRedirectAuthorizeResponse {
            redirect_url: redirect_context.redirect_url,
            connector_feature_data: redirect_context.connector_feature_data,
            partner_tx_id: redirect_context.partner_tx_id,
        }))
    }
}

pub fn build_grabpay_authorize_url<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    build_redirect_context(router_data).map(|context| context.redirect_url)
}

struct GrabpayRedirectContext {
    redirect_url: String,
    connector_feature_data: serde_json::Value,
    partner_tx_id: String,
}

fn build_redirect_context<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<GrabpayRedirectContext, error_stack::Report<errors::IntegrationError>> {
    let auth = GrabpayAuthType::try_from(&router_data.connector_config).change_context(
        errors::IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        },
    )?;
    let request_code = required_string(
        router_data.resource_common_data.connector_order_id.clone(),
        "connector_order_id",
        "GrabPay Authorize requires the request code returned by CreateOrder",
    )?;
    let partner_tx_id = router_data
        .resource_common_data
        .connector_request_reference_id
        .clone();
    validate_partner_tx_id(&partner_tx_id)?;
    let redirect_uri = router_data
        .request
        .router_return_url
        .clone()
        .or_else(|| router_data.resource_common_data.return_url.clone())
        .ok_or_else(|| {
            error_stack::report!(errors::IntegrationError::MissingRequiredField {
                field_name: "router_return_url",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "GrabPay OAuth authorize URL requires a redirect_uri".to_string(),
                    ),
                    ..Default::default()
                }
            })
        })?;
    let currency = router_data.request.currency;
    let country = get_country_code(router_data, currency);
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
            context: Default::default(),
        },
    )?;
    url.query_pairs_mut()
        .append_pair("acr_values", &acr_values)
        .append_pair("client_id", auth.client_id.peek())
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", CODE_CHALLENGE_METHOD)
        .append_pair("nonce", &nonce)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("request", &request_code)
        .append_pair("response_type", RESPONSE_TYPE_CODE)
        .append_pair("scope", SCOPE_ONE_TIME_CHARGE)
        .append_pair("state", &state);

    let connector_feature_data = serde_json::json!({
        "state": state,
        "nonce": nonce,
        "code_verifier": code_verifier,
        "redirect_uri": redirect_uri,
        "partner_tx_id": partner_tx_id,
        "currency": currency,
        "request_code": request_code,
    });

    Ok(GrabpayRedirectContext {
        redirect_url: url.to_string(),
        connector_feature_data,
        partner_tx_id,
    })
}

fn build_code_challenge(
    code_verifier: &str,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    use common_utils::crypto::GenerateDigest;

    let digest = common_utils::crypto::Sha256
        .generate_digest(code_verifier.as_bytes())
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
    Ok(BASE64_ENGINE_URL_SAFE_NO_PAD.encode(digest))
}

fn random_token(length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), length)
}

fn get_country_code<T: PaymentMethodDataTypes>(
    _router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    currency: Currency,
) -> String {
    country_from_currency(currency).to_string()
}

fn country_from_currency(currency: Currency) -> CountryAlpha2 {
    match currency {
        Currency::SGD => CountryAlpha2::SG,
        Currency::MYR => CountryAlpha2::MY,
        Currency::PHP => CountryAlpha2::PH,
        Currency::IDR => CountryAlpha2::ID,
        Currency::THB => CountryAlpha2::TH,
        _ => CountryAlpha2::SG,
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GrabpayFlowData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for GrabpayServerAuthenticationTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: GrabpayFlowData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
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
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    > for GrabpayServerAuthenticationTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = GrabpayAuthType::try_from(&router_data.connector_config).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let feature_data = parse_connector_feature_data(
            router_data
                .resource_common_data
                .connector_feature_data
                .as_ref(),
        )?;

        let code = feature_data
            .code
            .clone()
            .ok_or_else(missing_oauth_code_error)?;
        validate_callback_state(&feature_data)?;

        Ok(Self {
            grant_type: AUTHORIZATION_CODE_GRANT.to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
            code_verifier: required_feature_field(feature_data.code_verifier, "code_verifier")?,
            redirect_uri: required_feature_field(feature_data.redirect_uri, "redirect_uri")?,
            code,
        })
    }
}

impl TryFrom<ConnectorResponseData<GrabpayServerAuthenticationTokenResponse, Self>>
    for RouterDataV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayServerAuthenticationTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                token_type: item.response.token_type,
                expires_in: item.response.expires_in,
            }),
            ..item.router_data
        })
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
        match item.response {
            GrabpayAuthorizeResponse::Redirect(response) => Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.partner_tx_id),
                    redirection_data: Some(Box::new(RedirectForm::Uri {
                        uri: response.redirect_url,
                    })),
                    connector_metadata: Some(response.connector_feature_data),
                    mandate_reference: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: item.http_code,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::AuthenticationPending,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
            GrabpayAuthorizeResponse::Complete(response) => {
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
                    item.router_data.resource_common_data.access_token.as_ref(),
                    &response,
                );
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
                    }),
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}

impl TryFrom<ConnectorResponseData<GrabpayChargeCompleteResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayChargeCompleteResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = AttemptStatus::from(response.tx_status.clone());
        let resource_id = match item.router_data.response.as_ref() {
            Ok(PaymentsResponseData::TransactionResponse { resource_id, .. }) => {
                resource_id.clone()
            }
            _ => ResponseId::ConnectorTransactionId(response.tx_id.clone()),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                connector_metadata: Some(serde_json::json!({
                    "txID": response.tx_id,
                    "status": response.status,
                    "paymentMethod": response.payment_method,
                    "description": response.description,
                    "txStatus": response.tx_status,
                    "reason": response.reason,
                })),
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
            }),
            resource_common_data: PaymentFlowData {
                status,
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
                context: Default::default(),
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
            amount: router_data.request.refund_amount,
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

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.router_data.request.refund_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_response: Some(Secret::new(
                    serde_json::json!({
                        "txID": response.tx_id,
                        "status": response.status,
                        "paymentMethod": response.payment_method,
                        "description": response.description,
                        "txStatus": response.tx_status,
                        "reason": response.reason,
                        "echo": response.echo,
                    })
                    .to_string(),
                )),
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

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.router_data.request.connector_refund_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                raw_connector_response: Some(Secret::new(
                    serde_json::json!({
                        "txID": response.tx_id,
                        "status": response.status,
                        "paymentMethod": response.payment_method,
                        "description": response.description,
                        "txStatus": response.tx_status,
                        "reason": response.reason,
                        "echo": response.echo,
                    })
                    .to_string(),
                )),
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
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for GrabpayCreateOrderRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: GrabpayFlowData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(wrapper.router_data)
    }
}

impl
    TryFrom<
        RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    > for GrabpayCreateOrderRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        router_data: RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = GrabpayAuthType::try_from(&router_data.connector_config).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let partner_tx_id = router_data
            .resource_common_data
            .connector_request_reference_id;
        validate_partner_tx_id(&partner_tx_id)?;

        Ok(Self {
            partner_group_tx_id: partner_tx_id.clone(),
            partner_tx_id,
            currency: router_data.request.currency,
            amount: router_data.request.amount.get_amount_as_i64(),
            description: router_data.resource_common_data.description,
            merchant_id: auth.merchant_id.peek().to_string(),
        })
    }
}

impl TryFrom<ConnectorResponseData<GrabpayCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ConnectorResponseData<GrabpayCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let request = response.request.ok_or_else(|| {
            error_stack::report!(errors::ConnectorError::ResponseHandlingFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(format!(
                        "GrabPay CreateOrder did not return request code; partner_tx_id={:?}, status={:?}, tx_status={:?}, reason={:?}, message={:?}, code={:?}",
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

        Ok(Self {
            response: Ok(PaymentCreateOrderResponse {
                connector_order_id: request.clone(),
                session_data: None,
            }),
            resource_common_data: PaymentFlowData {
                reference_id: Some(request),
                connector_order_id: Some(partner_tx_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

pub(crate) fn validate_partner_tx_id(
    partner_tx_id: &str,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    let is_valid = partner_tx_id.len() <= 32
        && partner_tx_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));

    if is_valid {
        Ok(())
    } else {
        Err(error_stack::report!(
            errors::IntegrationError::InvalidDataFormat {
                field_name: "connector_request_reference_id",
                context: Default::default()
            }
        ))
    }
}

pub fn is_missing_oauth_code_error(error: &error_stack::Report<errors::IntegrationError>) -> bool {
    matches!(
        error.current_context(),
        errors::IntegrationError::MissingRequiredField {
            field_name: "connector_feature_data.code",
            ..
        }
    )
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
            nonce: None,
            code_verifier: None,
            redirect_uri: None,
            partner_tx_id: None,
            currency: None,
            request_code: None,
            tx_id: None,
            access_token: None,
            token_type: None,
            expires_in_seconds: None,
        }),
    }
}

pub(crate) fn access_token_from_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    let feature_data = parse_connector_feature_data(connector_feature_data)?;
    required_feature_field(feature_data.access_token, "access_token")
}

pub(crate) fn currency_from_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<Currency, error_stack::Report<errors::IntegrationError>> {
    let feature_data = parse_connector_feature_data(connector_feature_data)?;
    feature_data.currency.ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name: "connector_feature_data.currency",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "GrabPay RSync requires either refund_amount.currency or connector_feature_data.currency"
                        .to_string(),
                ),
                ..Default::default()
            },
        })
    })
}

fn charge_tx_id_from_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    let feature_data = parse_connector_feature_data(connector_feature_data)?;
    required_feature_field(feature_data.tx_id, "txID")
}

fn build_complete_connector_feature_data(
    connector_feature_data: Option<&SecretSerdeValue>,
    access_token_data: Option<&ServerAuthenticationTokenResponseData>,
    response: &GrabpayChargeCompleteResponse,
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

    if let Some(access_token_data) = access_token_data {
        metadata.insert(
            "access_token".to_string(),
            serde_json::json!(access_token_data.access_token.peek()),
        );
        metadata.insert(
            "token_type".to_string(),
            serde_json::json!(access_token_data.token_type),
        );
        metadata.insert(
            "expires_in_seconds".to_string(),
            serde_json::json!(access_token_data.expires_in),
        );
    }

    connector_metadata
}

fn validate_callback_state(
    feature_data: &GrabpayConnectorFeatureData,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    let expected_state = required_feature_field(feature_data.state.clone(), "state")?;
    let callback_state =
        required_feature_field(feature_data.callback_state.clone(), "callback_state")?;

    if expected_state == callback_state {
        Ok(())
    } else {
        Err(error_stack::report!(errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.callback_state",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "GrabPay OAuth callback state does not match the state generated during initial authorization"
                        .to_string(),
                ),
                ..Default::default()
            }
        }))
    }
}

fn required_feature_field(
    value: Option<String>,
    field_name: &'static str,
) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    value.ok_or_else(|| {
        error_stack::report!(errors::IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "GrabPay OAuth token exchange requires connector_feature_data.{field_name}"
                )),
                ..Default::default()
            }
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
            context: IntegrationErrorContext {
                additional_context: Some(message.to_string()),
                ..Default::default()
            }
        })
    })
}

fn missing_oauth_code_error() -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data.code",
        context: IntegrationErrorContext {
            additional_context: Some(
                "GrabPay OAuth code is not available yet; skipping token exchange request"
                    .to_string(),
            ),
            ..Default::default()
        }
    })
}
