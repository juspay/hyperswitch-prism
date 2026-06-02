use common_enums::AttemptStatus;
use domain_types::{
    connector_flow::{Authorize, ClientAuthenticationToken},
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, ResponseId, TKassaClientAuthenticationResponse,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::types::ResponseRouterData;

// =============================================================================
// AUTH
// =============================================================================
// T-Bank acquiring authenticates each request with a `Token` SHA-256 signature
// computed from the root-level request values plus the terminal Password.
// The single `api_key` credential carries both values as "TerminalKey:Password".
#[derive(Debug, Clone)]
pub struct TKassaAuthType {
    pub terminal_key: Secret<String>,
    pub password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TKassaAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::TKassa { api_key, .. } => {
                let raw = api_key.clone().expose();
                let (terminal_key, password) = raw.split_once(':').ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::FailedToObtainAuthType {
                        context: errors::IntegrationErrorContext::default()
                    })
                })?;
                Ok(Self {
                    terminal_key: Secret::new(terminal_key.to_string()),
                    password: Secret::new(password.to_string()),
                })
            }
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

/// Compute the T-Bank request `Token`: SHA-256 (lowercase hex) over the
/// root-level (key, value) pairs plus the terminal Password, sorted by key,
/// concatenating values only. Nested objects/arrays are excluded by the caller.
fn compute_token(mut pairs: Vec<(String, String)>, password: &str) -> String {
    pairs.push(("Password".to_string(), password.to_string()));
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let concatenated: String = pairs.into_iter().map(|(_, v)| v).collect();
    let mut hasher = Sha256::new();
    hasher.update(concatenated.as_bytes());
    hex::encode(hasher.finalize())
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TKassaErrorResponse {
    #[serde(rename = "ErrorCode", default)]
    pub code: String,
    #[serde(rename = "Message", default)]
    pub message: String,
    #[serde(rename = "Details")]
    pub details: Option<String>,
}

// =============================================================================
// SHARED Init REQUEST/RESPONSE (POST /v2/Init)
// =============================================================================
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TKassaInitRequest {
    pub terminal_key: String,
    pub amount: i64,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    pub token: String,
}

impl TKassaInitRequest {
    fn build(
        auth: &TKassaAuthType,
        amount: i64,
        order_id: String,
        description: Option<String>,
        device: Option<String>,
    ) -> Self {
        let terminal_key = auth.terminal_key.clone().expose();
        // Token is computed over root-level string values only.
        let mut pairs: Vec<(String, String)> = vec![
            ("TerminalKey".to_string(), terminal_key.clone()),
            ("Amount".to_string(), amount.to_string()),
            ("OrderId".to_string(), order_id.clone()),
        ];
        if let Some(d) = &description {
            pairs.push(("Description".to_string(), d.clone()));
        }
        if let Some(d) = &device {
            pairs.push(("Device".to_string(), d.clone()));
        }
        let token = compute_token(pairs, &auth.password.clone().expose());
        Self {
            terminal_key,
            amount,
            order_id,
            description,
            device,
            token,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TKassaInitResponse {
    pub success: bool,
    #[serde(default)]
    pub error_code: String,
    pub status: Option<TKassaPaymentStatus>,
    pub payment_id: Option<String>,
    pub order_id: Option<String>,
    pub payment_url: Option<String>,
    pub message: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TKassaPaymentStatus {
    New,
    FormShowed,
    Authorizing,
    #[serde(rename = "3DS_CHECKING")]
    ThreeDsChecking,
    #[serde(rename = "3DS_CHECKED")]
    ThreeDsChecked,
    Authorized,
    Confirming,
    Confirmed,
    Reversing,
    Reversed,
    Refunding,
    PartialRefunded,
    Refunded,
    Rejected,
    AuthFail,
    DeadlineExpired,
    Canceled,
    #[serde(other)]
    Unknown,
}

impl From<&TKassaPaymentStatus> for AttemptStatus {
    fn from(status: &TKassaPaymentStatus) -> Self {
        match status {
            TKassaPaymentStatus::New
            | TKassaPaymentStatus::FormShowed
            | TKassaPaymentStatus::Authorizing
            | TKassaPaymentStatus::ThreeDsChecking
            | TKassaPaymentStatus::ThreeDsChecked
            | TKassaPaymentStatus::Confirming => Self::Pending,
            TKassaPaymentStatus::Authorized => Self::Authorized,
            TKassaPaymentStatus::Confirmed => Self::Charged,
            TKassaPaymentStatus::Reversing
            | TKassaPaymentStatus::Reversed
            | TKassaPaymentStatus::Canceled => Self::Voided,
            TKassaPaymentStatus::Refunding
            | TKassaPaymentStatus::PartialRefunded
            | TKassaPaymentStatus::Refunded => Self::Pending,
            TKassaPaymentStatus::Rejected
            | TKassaPaymentStatus::AuthFail
            | TKassaPaymentStatus::DeadlineExpired => Self::Failure,
            TKassaPaymentStatus::Unknown => Self::Pending,
        }
    }
}

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================
#[derive(Debug, Serialize)]
pub struct TKassaPaymentsRequest(pub TKassaInitRequest);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TKassaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TKassaPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::TKassaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TKassaAuthType::try_from(&router_data.connector_config)?;
        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let req = TKassaInitRequest::build(
            &auth,
            router_data.request.minor_amount.get_amount_as_i64(),
            order_id,
            router_data.request.order_category.clone(),
            None,
        );
        Ok(Self(req))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TKassaPaymentsResponse(pub TKassaInitResponse);

impl<T: PaymentMethodDataTypes>
    TryFrom<
        ResponseRouterData<
            TKassaPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            TKassaPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        if !response.success {
            let message = response
                .message
                .or(response.details)
                .unwrap_or_else(|| "T-kassa Init failed".to_string());
            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    status_code: item.http_code,
                    code: response.error_code,
                    message: message.clone(),
                    reason: Some(message),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.payment_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let status = response
            .status
            .as_ref()
            .map(AttemptStatus::from)
            .unwrap_or(AttemptStatus::Pending);

        let resource_id = response
            .payment_id
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        let redirection_data = response
            .payment_url
            .clone()
            .map(|uri| Box::new(RedirectForm::Uri { uri }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response.order_id.clone(),
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

// =============================================================================
// CLIENT SDK SESSION TOKEN FLOW (ClientAuthenticationToken)
// =============================================================================
// Opens an SDK payment session via the same POST /v2/Init call (Device=SDK)
// and returns PaymentId (+ PaymentURL) as the client SDK session token.
#[derive(Debug, Serialize)]
pub struct TKassaClientAuthRequest(pub TKassaInitRequest);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::TKassaRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TKassaClientAuthRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: super::TKassaRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = TKassaAuthType::try_from(&router_data.connector_config)?;
        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let req = TKassaInitRequest::build(
            &auth,
            router_data.request.amount.get_amount_as_i64(),
            order_id,
            None,
            Some("SDK".to_string()),
        );
        Ok(Self(req))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TKassaClientAuthResponse(pub TKassaInitResponse);

impl
    TryFrom<
        ResponseRouterData<
            TKassaClientAuthResponse,
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
        >,
    >
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            TKassaClientAuthResponse,
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let response = item.response.0;

        if !response.success {
            let message = response
                .message
                .or(response.details)
                .unwrap_or_else(|| "T-kassa session init failed".to_string());
            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    status_code: item.http_code,
                    code: response.error_code,
                    message: message.clone(),
                    reason: Some(message),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            });
        }

        let payment_id = response.payment_id.clone().ok_or_else(|| {
            error_stack::report!(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default()
            })
        })?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::TKassa(
                TKassaClientAuthenticationResponse {
                    payment_id,
                    payment_url: response.payment_url.clone(),
                },
            ),
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
