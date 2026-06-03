use crate::{connectors::przelewy24::Przelewy24RouterData, types::ResponseRouterData};
use common_utils::types::{AmountConvertor, MinorUnit, MinorUnitForConnector};
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::{
    connector_flow::{Authorize, ClientAuthenticationToken},
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, Przelewy24ClientAuthenticationResponse as Przelewy24ClientAuthDomain,
        ResponseId,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// ===== AUTHENTICATION =====

/// Przelewy24 (P24) authentication. Uses HTTP Basic over `posId:apiKey`, and a
/// per-request SHA-384 `sign` computed with the separate `crc` key.
#[derive(Debug, Clone)]
pub struct Przelewy24AuthType {
    /// API key — HTTP Basic password.
    pub api_key: Secret<String>,
    /// Merchant / POS id (numeric) — HTTP Basic username and request body `merchantId`/`posId`.
    pub merchant_id: Secret<String>,
    /// CRC key used to compute the SHA-384 `sign`.
    pub crc: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for Przelewy24AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Przelewy24 {
                api_key,
                merchant_id,
                crc,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_id: merchant_id.to_owned(),
                crc: crc.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

impl Przelewy24AuthType {
    /// Parses the numeric merchant id (= posId).
    fn merchant_id_num(&self) -> Result<u64, error_stack::Report<IntegrationError>> {
        self.merchant_id
            .peek()
            .trim()
            .parse::<u64>()
            .change_context(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })
    }
}

/// Computes the P24 register `sign`:
/// `SHA384('{"sessionId":"..","merchantId":N,"amount":N,"currency":"..","crc":".."}')`.
fn compute_register_sign(
    session_id: &str,
    merchant_id: u64,
    amount: i64,
    currency: &str,
    crc: &str,
) -> String {
    use sha2::{Digest, Sha384};
    let payload = format!(
        "{{\"sessionId\":\"{session_id}\",\"merchantId\":{merchant_id},\"amount\":{amount},\"currency\":\"{currency}\",\"crc\":\"{crc}\"}}"
    );
    let mut hasher = Sha384::new();
    hasher.update(payload.as_bytes());
    hex::encode(hasher.finalize())
}

// ===== ERROR RESPONSE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Przelewy24ErrorResponse {
    #[serde(default)]
    pub code: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<serde_json::Value>,
}

// ===== REGISTER REQUEST (shared by Authorize and ClientAuthenticationToken) =====

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize)]
pub struct Przelewy24RegisterRequest {
    pub merchant_id: u64,
    pub pos_id: u64,
    pub session_id: String,
    pub amount: i64,
    pub currency: String,
    pub description: String,
    pub email: String,
    pub country: String,
    pub language: String,
    pub url_return: String,
    pub url_status: Option<String>,
    pub sign: String,
}

// camelCase is required by the P24 API; derive manually to keep field names explicit.
impl Serialize for Przelewy24RegisterRequestSerde<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let r = self.0;
        let mut st = serializer.serialize_struct("Przelewy24RegisterRequest", 11)?;
        st.serialize_field("merchantId", &r.merchant_id)?;
        st.serialize_field("posId", &r.pos_id)?;
        st.serialize_field("sessionId", &r.session_id)?;
        st.serialize_field("amount", &r.amount)?;
        st.serialize_field("currency", &r.currency)?;
        st.serialize_field("description", &r.description)?;
        st.serialize_field("email", &r.email)?;
        st.serialize_field("country", &r.country)?;
        st.serialize_field("language", &r.language)?;
        st.serialize_field("urlReturn", &r.url_return)?;
        if let Some(url_status) = &r.url_status {
            st.serialize_field("urlStatus", url_status)?;
        }
        st.serialize_field("sign", &r.sign)?;
        st.end()
    }
}

/// Newtype used so the request serializes with P24's camelCase keys without renaming every field.
pub struct Przelewy24RegisterRequestSerde<'a>(&'a Przelewy24RegisterRequest);

// ===== REGISTER RESPONSE (shared) =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Przelewy24RegisterResponseData {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Przelewy24RegisterResponse {
    pub data: Przelewy24RegisterResponseData,
    #[serde(rename = "responseCode", default)]
    pub response_code: Option<i64>,
}

// ===== AUTHORIZE =====

/// Connector-local wire request for the Authorize flow.
#[derive(Debug, Clone)]
pub struct Przelewy24PaymentsRequest(pub Przelewy24RegisterRequest);

impl Serialize for Przelewy24PaymentsRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Przelewy24RegisterRequestSerde(&self.0).serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Przelewy24PaymentsResponse(pub Przelewy24RegisterResponse);

fn build_register_request<T>(
    session_id: String,
    amount: MinorUnit,
    currency: common_enums::Currency,
    email: Option<String>,
    country: Option<common_enums::CountryAlpha2>,
    description: String,
    url_return: String,
    url_status: Option<String>,
    auth: &Przelewy24AuthType,
) -> Result<Przelewy24RegisterRequest, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    let converter = MinorUnitForConnector;
    let amount_minor = converter
        .convert(amount, currency)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("Failed to convert amount to minor unit")?;
    let amount_i64 = amount_minor.get_amount_as_i64();

    let merchant_id = auth.merchant_id_num()?;
    let currency_str = currency.to_string().to_uppercase();
    let country_str = country
        .map(|c| c.to_string().to_uppercase())
        .unwrap_or_else(|| "PL".to_string());
    let email = email.ok_or(IntegrationError::MissingRequiredField {
        field_name: "email",
        context: Default::default(),
    })?;

    let sign = compute_register_sign(
        &session_id,
        merchant_id,
        amount_i64,
        &currency_str,
        auth.crc.peek(),
    );

    Ok(Przelewy24RegisterRequest {
        merchant_id,
        pos_id: merchant_id,
        session_id,
        amount: amount_i64,
        currency: currency_str,
        description,
        email,
        country: country_str,
        language: "en".to_string(),
        url_return,
        url_status,
        sign,
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Przelewy24RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Przelewy24PaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Przelewy24RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = Przelewy24AuthType::try_from(&router_data.connector_config)?;

        let session_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let email = router_data
            .request
            .email
            .as_ref()
            .map(|e| e.peek().to_string());

        let country = router_data
            .resource_common_data
            .address
            .get_payment_method_billing()
            .and_then(|billing| billing.address.as_ref())
            .and_then(|address| address.country);

        let description = router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_else(|| format!("Payment {session_id}"));

        let url_return = router_data
            .request
            .router_return_url
            .clone()
            .unwrap_or_default();

        let url_status = router_data.request.webhook_url.clone();

        let request = build_register_request::<T>(
            session_id,
            router_data.request.amount,
            router_data.request.currency,
            email,
            country,
            description,
            url_return,
            url_status,
            &auth,
        )?;

        Ok(Self(request))
    }
}

/// Builds the `{base_url}/trnRequest/{token}` redirect URL for the customer.
fn build_redirect_form(base_url: &str, token: &str) -> Option<Box<RedirectForm>> {
    let trimmed = base_url.trim_end_matches('/');
    let url_str = format!("{trimmed}/trnRequest/{token}");
    url::Url::parse(&url_str).ok().map(|url| {
        Box::new(RedirectForm::from((
            url,
            common_utils::request::Method::Get,
        )))
    })
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<Przelewy24PaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Przelewy24PaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let token = item.response.0.data.token.clone();

        let base_url = item
            .router_data
            .resource_common_data
            .connectors
            .przelewy24
            .base_url
            .clone();

        let redirection_data = build_redirect_form(&base_url, &token);

        // P24 has no order id at register time; use the sessionId we sent as the reference.
        let session_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(session_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(session_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::AuthenticationPending,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== ClientAuthenticationToken (ClientSDKSessionToken) =====

#[derive(Debug, Clone)]
pub struct Przelewy24ClientAuthRequest(pub Przelewy24RegisterRequest);

impl Serialize for Przelewy24ClientAuthRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Przelewy24RegisterRequestSerde(&self.0).serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Przelewy24ClientAuthResponse(pub Przelewy24RegisterResponse);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Przelewy24RouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for Przelewy24ClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Przelewy24RouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = Przelewy24AuthType::try_from(&router_data.connector_config)?;

        let session_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let email = router_data
            .request
            .email
            .as_ref()
            .map(|e| e.peek().to_string());

        let country = router_data.request.country;

        let description = router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_else(|| format!("Payment {session_id}"));

        let url_return = router_data
            .resource_common_data
            .return_url
            .clone()
            .unwrap_or_default();

        // PaymentFlowData has no webhook_url; status notifications are out of scope for the
        // ClientSDKSessionToken flow, so urlStatus is omitted.
        let url_status = None;

        let request = build_register_request::<T>(
            session_id,
            router_data.request.amount,
            router_data.request.currency,
            email,
            country,
            description,
            url_return,
            url_status,
            &auth,
        )?;

        Ok(Self(request))
    }
}

impl TryFrom<ResponseRouterData<Przelewy24ClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<Przelewy24ClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let token = item.response.0.data.token.clone();

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Przelewy24(Przelewy24ClientAuthDomain {
                token: Secret::new(token),
            }),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
