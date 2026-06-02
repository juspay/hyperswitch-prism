use common_enums::AttemptStatus;
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{Authorize, ServerSessionAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorAuthType, ConnectorSpecificConfig},
    router_data_v2::RouterDataV2,
    utils,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::mercadopago::MercadopagoRouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================
// Mercado Pago uses the backend `access_token` as a Bearer credential
// (`api_key`) and exposes the `public_key` to the client SDK via the
// ServerSessionAuthenticationToken flow.
#[derive(Debug, Clone)]
pub struct MercadopagoAuthType {
    pub api_key: Secret<String>,
    pub public_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for MercadopagoAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Mercadopago {
                api_key,
                public_key,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                public_key: public_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

impl TryFrom<&ConnectorAuthType> for MercadopagoAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self {
                api_key: api_key.to_owned(),
                public_key: Some(key1.to_owned()),
            }),
            ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
                public_key: None,
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// =============================================================================
// ERROR
// =============================================================================
// Mercado Pago error body, e.g.
// { "message": "Invalid token", "error": "bad_request", "status": 400,
//   "cause": [ { "code": 2034, "description": "Invalid token" } ] }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MercadopagoErrorResponse {
    #[serde(default)]
    pub message: String,
    pub error: Option<String>,
    pub status: Option<i64>,
    #[serde(default)]
    pub cause: Vec<MercadopagoErrorCause>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MercadopagoErrorCause {
    pub code: Option<serde_json::Value>,
    pub description: Option<String>,
}

// =============================================================================
// SERVER SESSION AUTHENTICATION TOKEN (ClientSDK session token)
// =============================================================================
// Mercado Pago has no backend "create session" REST endpoint for card checkout.
// The merchant `public_key` is what the MercadoPago.js client SDK needs to
// initialize (`new MercadoPago(public_key)`) and tokenize the card. The
// connector call hits the authenticated `GET /v1/payment_methods` endpoint to
// validate the access_token, and the `session_token` returned to the client is
// the merchant `public_key` (taken from the connector auth, not the response
// body).
//
// No request body is sent (GET).
#[derive(Debug, Serialize)]
pub struct MercadopagoSessionTokenRequest {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MercadopagoRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for MercadopagoSessionTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: MercadopagoRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

// `GET /v1/payment_methods` returns a JSON array; we don't depend on its shape,
// the `session_token` is derived from the merchant `public_key` in auth.
#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct MercadopagoSessionTokenResponse {
    pub raw: serde_json::Value,
}

impl
    TryFrom<
        ResponseRouterData<
            MercadopagoSessionTokenResponse,
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
        >,
    >
    for RouterDataV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            MercadopagoSessionTokenResponse,
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let auth =
            MercadopagoAuthType::try_from(&item.router_data.connector_config).map_err(|_| {
                error_stack::report!(ConnectorError::ResponseHandlingFailed {
                    context: Default::default(),
                })
            })?;
        let session_token = auth
            .public_key
            .ok_or_else(|| {
                error_stack::report!(ConnectorError::ResponseHandlingFailed {
                    context: Default::default(),
                })
            })?
            .expose();
        Ok(Self {
            response: Ok(ServerSessionAuthenticationTokenResponseData { session_token }),
            ..item.router_data
        })
    }
}

// =============================================================================
// AUTHORIZE (Create Payment) -- POST /v1/payments
// =============================================================================
#[derive(Debug, Serialize)]
pub struct MercadopagoPaymentsRequest {
    pub transaction_amount: FloatMajorUnit,
    pub token: Secret<String>,
    pub installments: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub capture: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_reference: Option<String>,
    pub payer: MercadopagoPayer,
}

#[derive(Debug, Serialize)]
pub struct MercadopagoPayer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

fn map_card_network_to_payment_method_id(
    network: Option<&common_enums::CardNetwork>,
) -> Option<String> {
    network.map(|n| match n {
        common_enums::CardNetwork::Visa => "visa".to_string(),
        common_enums::CardNetwork::Mastercard => "master".to_string(),
        common_enums::CardNetwork::AmericanExpress => "amex".to_string(),
        other => other.to_string().to_lowercase(),
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MercadopagoRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MercadopagoPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: MercadopagoRouterData<
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
        let request = &router_data.request;

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            request.minor_amount,
            request.currency,
        )?;

        // Mercado Pago's /v1/payments requires a single-use card `token` that the
        // MercadoPago.js client SDK produces. It is delivered to UCS either as an
        // already-tokenized payment method, or alongside raw card data via the
        // session token created in the ClientSDK session flow.
        let (token, payment_method_id) = match &request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(pm_token) => (pm_token.token.clone(), None),
            PaymentMethodData::Card(card) => {
                let token = request
                    .session_token
                    .clone()
                    .map(Secret::new)
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "token (Mercado Pago card token from client SDK)",
                            context: Default::default(),
                        })
                    })?;
                (
                    token,
                    map_card_network_to_payment_method_id(card.card_network.as_ref()),
                )
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Payment method not supported by Mercadopago".to_string(),
                    Default::default(),
                )))
            }
        };

        let capture = !matches!(
            request.capture_method,
            Some(common_enums::CaptureMethod::Manual)
        );

        Ok(Self {
            transaction_amount: amount,
            token,
            installments: 1,
            payment_method_id,
            description: router_data.resource_common_data.description.clone(),
            capture,
            external_reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            payer: MercadopagoPayer {
                email: request.email.as_ref().map(|e| e.peek().to_string()),
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MercadopagoPaymentStatus {
    Approved,
    Authorized,
    InProcess,
    InMediation,
    Pending,
    Rejected,
    Cancelled,
    Refunded,
    ChargedBack,
    #[serde(other)]
    Unknown,
}

fn map_mercadopago_status(
    status: &MercadopagoPaymentStatus,
    is_manual_capture: bool,
) -> AttemptStatus {
    match status {
        MercadopagoPaymentStatus::Approved => {
            if is_manual_capture {
                AttemptStatus::Authorized
            } else {
                AttemptStatus::Charged
            }
        }
        MercadopagoPaymentStatus::Authorized => AttemptStatus::Authorized,
        MercadopagoPaymentStatus::InProcess
        | MercadopagoPaymentStatus::InMediation
        | MercadopagoPaymentStatus::Pending => AttemptStatus::Pending,
        MercadopagoPaymentStatus::Rejected | MercadopagoPaymentStatus::Cancelled => {
            AttemptStatus::Failure
        }
        MercadopagoPaymentStatus::Refunded | MercadopagoPaymentStatus::ChargedBack => {
            AttemptStatus::Charged
        }
        MercadopagoPaymentStatus::Unknown => AttemptStatus::Pending,
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MercadopagoPaymentsResponse {
    pub id: i64,
    pub status: MercadopagoPaymentStatus,
    pub status_detail: Option<String>,
    #[serde(default)]
    pub external_reference: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            MercadopagoPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            MercadopagoPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let is_manual_capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Manual)
        );
        let status = map_mercadopago_status(&item.response.status, is_manual_capture);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.to_string()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.external_reference.clone(),
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
