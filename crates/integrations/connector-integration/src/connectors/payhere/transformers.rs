use std::collections::HashMap;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::request::Method;
use domain_types::{merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    connector_flow::{ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
        PaymentsAuthorizeData,
        PaymentsResponseData,
        PaymentsSyncData,
        RefundFlowData,
        RefundsData,
        RefundsResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use crate::{types::ResponseRouterData, connectors::payhere::PayhereRouterData};

use error_stack::ResultExt;
use common_utils::crypto::GenerateDigest;
use hyperswitch_masking::{Secret, ExposeInterface, PeekInterface};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PayhereAuthType {
    pub app_id: Secret<String>,
    pub app_secret: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_secret: Secret<String>,
    pub access_token: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PayhereAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match item {
            ConnectorSpecificConfig::Payhere { api_key, key1, api_secret, key2, .. } => {
                Ok(Self {
                    app_id: api_key.clone(),
                    app_secret: api_secret.clone(),
                    merchant_id: key1.clone(),
                    merchant_secret: key2.clone(),
                    access_token: Secret::new("".to_string()),
                })
            }
            _ => Err(error_stack::report!(IntegrationError::FailedToObtainIntegrationUrl {
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: None,
                }
            })),
        }
    }
}

// ---- Access Token ----

#[derive(Debug, Serialize)]
pub struct PayhereServerAuthenticationTokenRequest {
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> TryFrom<PayhereRouterData<RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>, T>> for PayhereServerAuthenticationTokenRequest {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(_: PayhereRouterData<RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>, T>) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: "client_credentials".to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereServerAuthenticationTokenResponse {
    pub access_token: Secret<String>,
    pub expires_in: Option<i64>,
    pub token_type: Option<String>,
}

impl TryFrom<ResponseRouterData<PayhereServerAuthenticationTokenResponse, RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>>> for RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData> {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PayhereServerAuthenticationTokenResponse, RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>>) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        router_data.response = Ok(ServerAuthenticationTokenResponseData {
            access_token: item.response.access_token,
            expires_in: item.response.expires_in,
            token_type: item.response.token_type,
        });
        Ok(router_data)
    }
}

// ---- Authorize ----

pub(crate) fn handle_authorize_response<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &RouterDataV2<
        domain_types::connector_flow::Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    _event_builder: Option<&mut common_utils::events::Event>,
    _res: domain_types::router_response_types::Response,
) -> common_utils::errors::CustomResult<
    RouterDataV2<
        domain_types::connector_flow::Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    domain_types::errors::ConnectorError,
> {
    use hyperswitch_masking::ExposeInterface;
    use hyperswitch_masking::Maskable;
    use error_stack::ResultExt;

    let mut router_data = data.clone();
    let item = data.clone();

    // Check payment method
    match item.request.payment_method_data.clone() {
        domain_types::payment_method_data::PaymentMethodData::Wallet(
            domain_types::payment_method_data::WalletData::PayhereRedirect {},
        ) => {}
        _ => {
            return Err(error_stack::report!(domain_types::errors::ConnectorError::ResponseHandlingFailed {
                context: domain_types::errors::ResponseTransformationErrorContext { http_status_code: None, additional_context: None }
            }))
        }
    }

    let auth = PayhereAuthType::try_from(&item.connector_config)
        .change_context(domain_types::errors::ConnectorError::ResponseHandlingFailed { context: domain_types::errors::ResponseTransformationErrorContext { http_status_code: None, additional_context: None } })?;
        
    let amount = common_utils::types::AmountConvertor::convert(&common_utils::types::StringMajorUnitForConnector, item.request.minor_amount, item.request.currency)
        .change_context(domain_types::errors::ConnectorError::ResponseHandlingFailed { context: domain_types::errors::ResponseTransformationErrorContext { http_status_code: None, additional_context: None } })?
        .get_amount_as_string();
    
    let merchant_secret = auth.merchant_secret.expose();
    
    let merchant_secret = auth.merchant_secret.expose();
    let merchant_secret = auth.merchant_secret.expose();
    let hash_secret = common_utils::crypto::Md5.generate_digest(merchant_secret.as_bytes())
        .change_context(domain_types::errors::ConnectorError::ResponseHandlingFailed { context: domain_types::errors::ResponseTransformationErrorContext { http_status_code: None, additional_context: None } })?;
    let hash_secret_upper = hex::encode(hash_secret).to_uppercase();
    
    let message = format!("{}{}{}{}{}", auth.merchant_id.clone().expose(), item.resource_common_data.connector_request_reference_id, amount, item.request.currency.to_string(), hash_secret_upper);
    let final_hash = common_utils::crypto::Md5.generate_digest(message.as_bytes())
        .change_context(domain_types::errors::ConnectorError::ResponseHandlingFailed { context: domain_types::errors::ResponseTransformationErrorContext { http_status_code: None, additional_context: None } })?;
    let hash = hex::encode(final_hash).to_uppercase();

    let billing_addr = item
        .resource_common_data
        .address
        .get_payment_method_billing()
        .or_else(|| item.resource_common_data.address.get_payment_billing());
    let details = billing_addr.and_then(|a| a.address.as_ref());
    let first_name = details
        .and_then(|d| d.first_name.clone())
        .unwrap_or_else(|| "John".to_string()).into();
    let last_name = details
        .and_then(|d| d.last_name.clone())
        .unwrap_or_else(|| "Doe".to_string()).into();
    let email = item
        .request
        .email
        .as_ref()
        .map(|e| e.peek().to_string()).into()
        .or_else(|| {
            billing_addr
                .and_then(|a| a.email.as_ref())
                .map(|e| e.peek().to_string()).into()
        })
        .unwrap_or_else(|| "john.doe@example.com".to_string()).into();
    let phone = billing_addr
        .and_then(|a| a.phone.as_ref())
        .and_then(|p| p.number.clone())
        .unwrap_or_else(|| "0000000000".to_string()).into();
    let address = details
        .and_then(|d| d.line1.clone())
        .unwrap_or_else(|| "address".to_string()).into();
    let city = details
        .and_then(|d| d.city.clone())
        .unwrap_or_else(|| "city".to_string()).into();
    let country = details
        .and_then(|d| d.country)
        .map(|c| c.to_string())
        .unwrap_or_else(|| "LK".to_string());

    let merchant_id = auth.merchant_id.expose();
    let return_url = item.request.router_return_url.clone().unwrap_or_default();
    let cancel_url = item.request.router_return_url.clone().unwrap_or_default();
    let notify_url = item.request.webhook_url.clone().unwrap_or_default();
    let order_id = item.resource_common_data.connector_request_reference_id.clone();
    let items = "Order".to_string();
    let currency = item.request.currency.to_string();

    let mut form_fields = std::collections::HashMap::new();
    form_fields.insert("merchant_id".to_string(), merchant_id);
    form_fields.insert("return_url".to_string(), return_url);
    form_fields.insert("cancel_url".to_string(), cancel_url);
    form_fields.insert("notify_url".to_string(), notify_url);
    form_fields.insert("first_name".to_string(), first_name.expose());
    form_fields.insert("last_name".to_string(), last_name.expose());
    form_fields.insert("email".to_string(), email.expose());
    form_fields.insert("phone".to_string(), phone.expose());
    form_fields.insert("address".to_string(), address.expose());
    form_fields.insert("city".to_string(), city.expose());
    form_fields.insert("country".to_string(), country);
    form_fields.insert("order_id".to_string(), order_id);
    form_fields.insert("items".to_string(), items);
    form_fields.insert("currency".to_string(), currency);
    form_fields.insert("amount".to_string(), amount);
    form_fields.insert("hash".to_string(), hash);

    let base = if router_data
        .resource_common_data
        .connectors
        .payhere
        .base_url
        .contains("sandbox")
    {
        "https://sandbox.payhere.lk"
    } else {
        "https://www.payhere.lk"
    };
    let endpoint = format!("{}/pay/checkout", base);

    let redirection_data = Some(Box::new(domain_types::router_response_types::RedirectForm::Form {
        endpoint,
        method: common_utils::request::Method::Post,
        form_fields,
    }));

    router_data.response = Ok(PaymentsResponseData::TransactionResponse {
        resource_id: domain_types::connector_types::ResponseId::NoResponseId,
        redirection_data,
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        network_txn_link_id: None,
        payment_account_reference: None,
        splits: None,
        incremental_authorization_allowed: None,
        status_code: 200, // HTTP OK since it's local
    });
    router_data.resource_common_data.status = common_enums::AttemptStatus::AuthenticationPending;
    Ok(router_data)
}
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PayhereSyncResponse {
    pub status: i32,
    pub msg: String,
    pub data: Option<Vec<PayherePaymentData>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayherePaymentData {
    pub payment_id: i64,
    pub order_id: String,
    pub status: String,
    pub currency: String,
    pub amount: f64,
}

impl TryFrom<ResponseRouterData<PayhereSyncResponse, RouterDataV2<domain_types::connector_flow::PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>> for RouterDataV2<domain_types::connector_flow::PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData> {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PayhereSyncResponse, RouterDataV2<domain_types::connector_flow::PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        
        let payment_data = item.response.data.and_then(|mut d| d.pop());
        
        let status = if let Some(ref data) = payment_data {
            match data.status.as_str() {
                "RECEIVED" | "REFUND REQUESTED" | "REFUND PROCESSING" | "REFUNDED" | "CHARGEBACKED" => AttemptStatus::Charged,
                _ => AttemptStatus::Pending,
            }
        } else {
            AttemptStatus::Pending
        };

        router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: domain_types::connector_types::ResponseId::ConnectorTransactionId(
                payment_data.map(|d| d.payment_id.to_string()).unwrap_or_default()
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            network_txn_link_id: None,
            payment_account_reference: None,
            splits: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        });
        router_data.resource_common_data.status = status;
        
        Ok(router_data)
    }
}

// ---- Refund ----

#[derive(Debug, Serialize)]
pub struct PayhereRefundRequest {
    pub payment_id: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> TryFrom<PayhereRouterData<RouterDataV2<domain_types::connector_flow::Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>> for PayhereRefundRequest {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: PayhereRouterData<RouterDataV2<domain_types::connector_flow::Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>) -> Result<Self, Self::Error> {
        let amount = common_utils::types::AmountConvertor::convert(&common_utils::types::StringMajorUnitForConnector, item.router_data.request.minor_refund_amount, item.router_data.request.currency)
            .change_context(domain_types::errors::IntegrationError::RequestEncodingFailed { context: domain_types::errors::IntegrationErrorContext { suggested_action: None, doc_url: None, additional_context: None, } })?
            .get_amount_as_string();
        let router_data = item.router_data;
        Ok(Self {
            payment_id: router_data.request.connector_transaction_id.clone(),
            description: router_data.request.reason.clone().unwrap_or("Refund".to_string()),
            amount: Some(amount),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereRefundResponse {
    pub status: i32,
    pub msg: String,
    pub data: Option<i64>,
}

impl TryFrom<ResponseRouterData<PayhereRefundResponse, RouterDataV2<domain_types::connector_flow::Refund, RefundFlowData, RefundsData, RefundsResponseData>>> for RouterDataV2<domain_types::connector_flow::Refund, RefundFlowData, RefundsData, RefundsResponseData> {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<PayhereRefundResponse, RouterDataV2<domain_types::connector_flow::Refund, RefundFlowData, RefundsData, RefundsResponseData>>) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        
        let status = match item.response.status {
            1 => RefundStatus::Success,
            -1 => RefundStatus::Failure,
            _ => RefundStatus::Pending,
        };

        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: item.response.data.map(|d| d.to_string()).unwrap_or_default(),
            refund_status: status,
            acquirer_reference_number: None,
            status_code: item.http_code,
        });
        
        Ok(router_data)
    }
}

// ---- Webhook ----

#[derive(Debug, Deserialize)]
pub struct PayhereWebhookPayload {
    pub merchant_id: String,
    pub order_id: String,
    pub payment_id: Option<i64>,
    pub payhere_amount: String,
    pub payhere_currency: String,
    pub status_code: i32,
    pub md5sig: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayhereErrorResponse {
    pub status: i32,
    pub msg: String,
}

pub fn get_status_from_code(code: i32) -> AttemptStatus {
    match code {
        2 => AttemptStatus::Charged,
        0 => AttemptStatus::Pending,
        -1 | -2 => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    }
}
