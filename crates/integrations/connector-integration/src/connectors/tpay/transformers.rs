use common_enums::AttemptStatus;
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{Authorize, ClientAuthenticationToken},
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, ResponseId, TpayClientAuthenticationResponse,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::tpay::TpayRouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================
// TPay uses OAuth 2.0 client-credentials: `api_key` carries the merchant
// `client_id`, `client_secret` carries the merchant `client_secret`. These are
// exchanged at POST /oauth/auth for a short-lived Bearer access token.
#[derive(Debug, Clone)]
pub struct TpayAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TpayAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Tpay {
                api_key,
                client_secret,
                ..
            } => Ok(Self {
                client_id: api_key.to_owned(),
                client_secret: client_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
// TPay does not document a standalone error JSON schema; domain outcomes are
// surfaced via the `result` field and free-form business error messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpayErrorResponse {
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

// =============================================================================
// CLIENT SDK SESSION TOKEN (ClientAuthenticationToken) — OAuth token acquisition
// =============================================================================
// Google Pay client-SDK init constants (static merchant configuration).
const TPAY_GPAY_GATEWAY: &str = "tpaycom";

fn tpay_allowed_card_networks() -> Vec<String> {
    vec!["MASTERCARD".to_string(), "VISA".to_string()]
}

fn tpay_allowed_card_auth_methods() -> Vec<String> {
    vec!["PAN_ONLY".to_string(), "CRYPTOGRAM_3DS".to_string()]
}

/// Form-urlencoded body for POST /oauth/auth.
#[derive(Debug, Serialize)]
pub struct TpayClientAuthRequest {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TpayClientAuthRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: TpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TpayAuthType::try_from(&wrapper.router_data.connector_config)?;
        Ok(Self {
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

/// Response from POST /oauth/auth.
#[derive(Debug, Deserialize, Serialize)]
pub struct TpayOAuthResponse {
    pub access_token: Secret<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub issued_at: Option<i64>,
    #[serde(default)]
    pub client_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TpayOAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TpayOAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Merchant gatewayMerchantId for the Google Pay SDK is the OAuth
        // client_id returned by TPay (merchant identifier assigned at
        // registration), per the spec's static client-SDK configuration.
        let gateway_merchant_id = response.client_id.clone();

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Tpay(TpayClientAuthenticationResponse {
                access_token: response.access_token,
                token_type: response.token_type,
                expires_in: response.expires_in,
                gateway: TPAY_GPAY_GATEWAY.to_string(),
                gateway_merchant_id,
                allowed_card_networks: tpay_allowed_card_networks(),
                allowed_card_auth_methods: tpay_allowed_card_auth_methods(),
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

// =============================================================================
// AUTHORIZE — POST /transactions (create pending transaction)
// =============================================================================
// Card payment group. The `/pay` step (RSA-encrypted cardPaymentData) and 3DS
// redirect are completed by the payer at `transactionPaymentUrl`; the create
// call returns `status: "pending"` with that URL.
const TPAY_CARD_GROUP_ID: u16 = 103;

#[derive(Debug, Serialize)]
pub struct TpayPayer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TpayPay {
    #[serde(rename = "groupId")]
    pub group_id: u16,
}

#[derive(Debug, Serialize)]
pub struct TpayPaymentsRequest<T: PaymentMethodDataTypes> {
    pub amount: FloatMajorUnit,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<TpayPayer>,
    pub pay: TpayPay,
    #[serde(skip)]
    pub _phantom: std::marker::PhantomData<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TpayRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for TpayPaymentsRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: TpayRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        let amount = wrapper
            .connector
            .amount_converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let description = item
            .resource_common_data
            .description
            .clone()
            .unwrap_or_else(|| item.resource_common_data.connector_request_reference_id.clone());

        let email = item.request.email.as_ref().map(|e| e.clone().expose().expose());
        let name = item.request.customer_name.clone();
        let ip = item
            .request
            .browser_info
            .as_ref()
            .and_then(|b| b.ip_address.as_ref())
            .map(|ip| ip.to_string());

        let payer = if email.is_some() || name.is_some() || ip.is_some() {
            Some(TpayPayer { email, name, ip })
        } else {
            None
        };

        Ok(Self {
            amount,
            description,
            payer,
            pay: TpayPay {
                group_id: TPAY_CARD_GROUP_ID,
            },
            _phantom: std::marker::PhantomData,
        })
    }
}

/// Transaction object returned by POST /transactions (and POST /transactions/{id}/pay).
#[derive(Debug, Deserialize, Serialize)]
pub struct TpayPaymentsResponse {
    #[serde(default)]
    pub result: Option<String>,
    #[serde(rename = "transactionId")]
    pub transaction_id: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "transactionPaymentUrl", default)]
    pub transaction_payment_url: Option<String>,
}

fn map_tpay_status(status: Option<&str>, has_redirect: bool) -> AttemptStatus {
    match status {
        Some("correct") | Some("paid") | Some("success") => AttemptStatus::Charged,
        // `pending` with a payment URL means the payer must complete payment /
        // 3DS at `transactionPaymentUrl`.
        Some("pending") if has_redirect => AttemptStatus::AuthenticationPending,
        Some("pending") => AttemptStatus::Pending,
        Some("declined") | Some("error") | Some("failed") => AttemptStatus::Failure,
        _ if has_redirect => AttemptStatus::AuthenticationPending,
        _ => AttemptStatus::Pending,
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        ResponseRouterData<
            TpayPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            TpayPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let redirection_data = item
            .response
            .transaction_payment_url
            .clone()
            .map(|uri| Box::new(RedirectForm::Uri { uri }));

        let status = map_tpay_status(
            item.response.status.as_deref(),
            redirection_data.is_some(),
        );

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.transaction_id),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.title,
                incremental_authorization_allowed: None,
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
