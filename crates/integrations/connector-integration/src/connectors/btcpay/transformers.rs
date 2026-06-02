use common_enums::AttemptStatus;
use common_utils::{
    request::Method,
    types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{Authorize, ServerSessionAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::btcpay::BtcpayRouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================
#[derive(Debug, Clone)]
pub struct BtcpayAuthType {
    pub api_key: Secret<String>,
    pub store_id: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for BtcpayAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Btcpay {
                api_key, store_id, ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                store_id: store_id.to_owned(),
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
// CONNECTOR METADATA (store_id supplied via per-payment metadata)
// =============================================================================
#[derive(Debug, Clone, Deserialize)]
pub struct BtcpayMetadata {
    pub store_id: String,
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================
// BTCPay returns either a single error object or an array of validation errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BtcpayErrorResponse {
    Single(BtcpayError),
    Many(Vec<BtcpayError>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtcpayError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub path: Option<String>,
}

impl BtcpayErrorResponse {
    pub fn first(&self) -> Option<&BtcpayError> {
        match self {
            Self::Single(e) => Some(e),
            Self::Many(errs) => errs.first(),
        }
    }
}

// =============================================================================
// INVOICE STATUS
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum BtcpayInvoiceStatus {
    New,
    Processing,
    Settled,
    Expired,
    Invalid,
}

impl From<&BtcpayInvoiceStatus> for AttemptStatus {
    fn from(status: &BtcpayInvoiceStatus) -> Self {
        match status {
            // Invoice created, customer must pay on the hosted checkout page.
            BtcpayInvoiceStatus::New => Self::AuthenticationPending,
            // Payment seen on-chain, awaiting confirmations.
            BtcpayInvoiceStatus::Processing => Self::Pending,
            BtcpayInvoiceStatus::Settled => Self::Charged,
            BtcpayInvoiceStatus::Expired | BtcpayInvoiceStatus::Invalid => Self::Failure,
        }
    }
}

// =============================================================================
// SHARED CREATE-INVOICE REQUEST + RESPONSE
// =============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct BtcpayCheckoutOptions {
    #[serde(rename = "redirectURL", skip_serializing_if = "Option::is_none")]
    pub redirect_url: Option<String>,
    #[serde(rename = "redirectAutomatically")]
    pub redirect_automatically: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BtcpayInvoiceMetadata {
    #[serde(rename = "orderId")]
    pub order_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BtcpayPaymentsRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BtcpayInvoiceMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<BtcpayCheckoutOptions>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BtcpayInvoiceResponse {
    pub id: String,
    #[serde(rename = "checkoutLink")]
    pub checkout_link: Option<String>,
    pub status: BtcpayInvoiceStatus,
    pub amount: Option<String>,
    pub currency: Option<String>,
}

// Distinct response type for the session-token flow (BTCPay returns the same
// invoice object, but the macro framework generates a per-response templating
// type, so each flow needs its own response struct).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BtcpaySessionTokenResponse {
    pub id: String,
}

// =============================================================================
// AUTHORIZE — request transformation
// =============================================================================
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BtcpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BtcpayPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        value: BtcpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        let amount = StringMajorUnitForConnector
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        let order_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let checkout = item
            .resource_common_data
            .return_url
            .clone()
            .map(|return_url| BtcpayCheckoutOptions {
                redirect_url: Some(return_url),
                redirect_automatically: true,
            });

        Ok(Self {
            amount,
            currency: item.request.currency.to_string(),
            metadata: Some(BtcpayInvoiceMetadata { order_id }),
            checkout,
        })
    }
}

// =============================================================================
// AUTHORIZE — response transformation
// =============================================================================
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            BtcpayInvoiceResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BtcpayInvoiceResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.status);

        // For an unpaid invoice, redirect the customer to the hosted checkout page.
        let redirection_data = item.response.checkout_link.as_ref().map(|link| {
            Box::new(RedirectForm::Form {
                endpoint: link.clone(),
                method: Method::Get,
                form_fields: Default::default(),
            })
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
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
// CLIENT SDK SESSION TOKEN (ServerSessionAuthenticationToken)
// =============================================================================
// BTCPay has no dedicated session-token endpoint; bootstrap a session by
// creating an invoice and returning its id as the session token.
#[derive(Debug, Clone, Serialize)]
pub struct BtcpaySessionTokenRequest {
    pub amount: StringMajorUnit,
    pub currency: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BtcpayRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for BtcpaySessionTokenRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        value: BtcpayRouterData<
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        let amount = StringMajorUnitForConnector
            .convert(item.request.amount, item.request.currency)
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: item.request.currency.to_string(),
        })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            BtcpaySessionTokenResponse,
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
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            BtcpaySessionTokenResponse,
            RouterDataV2<
                ServerSessionAuthenticationToken,
                PaymentFlowData,
                ServerSessionAuthenticationTokenRequestData,
                ServerSessionAuthenticationTokenResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let session_token = item.response.id;

        Ok(Self {
            response: Ok(ServerSessionAuthenticationTokenResponseData {
                session_token: session_token.clone(),
            }),
            resource_common_data: PaymentFlowData {
                session_token: Some(session_token),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
