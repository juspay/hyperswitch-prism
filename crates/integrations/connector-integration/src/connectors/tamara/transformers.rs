use std::fmt::Debug;

use crate::connectors::tamara::TamaraRouterData;
use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, VerifyWebhookSource, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, VerifyWebhookSourceFlowData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{RedirectForm, VerifyWebhookSourceResponseData, VerifyWebhookStatus},
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct TamaraAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TamaraAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Tamara { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraErrorResponse {
    pub message: String,
    pub errors: Option<Vec<TamaraErrorDetail>>,
    pub data: Option<serde_json::Value>,
    pub title: Option<String>,
    pub screen_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraErrorDetail {
    pub error_code: String,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TamaraPaymentStatus {
    New,
    Approved,
    Declined,
    Authorised,
    FullyCaptured,
    PartiallyCaptured,
    Canceled,
    Updated,
    FullyRefunded,
    PartiallyRefunded,
    Expired,
}

impl From<TamaraPaymentStatus> for AttemptStatus {
    fn from(status: TamaraPaymentStatus) -> Self {
        match status {
            TamaraPaymentStatus::Authorised => Self::Authorized,
            TamaraPaymentStatus::FullyCaptured | TamaraPaymentStatus::PartiallyCaptured => {
                Self::Charged
            }
            TamaraPaymentStatus::Canceled | TamaraPaymentStatus::Updated => Self::Voided,
            TamaraPaymentStatus::FullyRefunded | TamaraPaymentStatus::PartiallyRefunded => {
                Self::Charged
            }
            TamaraPaymentStatus::Declined | TamaraPaymentStatus::Expired => Self::Failure,
            TamaraPaymentStatus::New | TamaraPaymentStatus::Approved => Self::Pending,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TamaraRefundStatus {
    FullyRefunded,
    PartiallyRefunded,
}

impl From<TamaraRefundStatus> for RefundStatus {
    fn from(status: TamaraRefundStatus) -> Self {
        match status {
            TamaraRefundStatus::FullyRefunded | TamaraRefundStatus::PartiallyRefunded => {
                Self::Success
            }
        }
    }
}

// ===== AUTHORIZE (hits /checkout, creates order and returns checkout_url) =====

#[derive(Debug, Serialize)]
pub struct TamaraPaymentsRequest {
    pub total_amount: TamaraAmount,
    pub shipping_amount: TamaraAmount,
    pub tax_amount: TamaraAmount,
    pub order_reference_id: String,
    pub country_code: String,
    pub description: String,
    pub items: Vec<TamaraLineItem>,
    pub consumer: TamaraConsumer,
    pub merchant_url: TamaraMerchantUrl,
    pub shipping_address: TamaraAddress,
    pub billing_address: Option<TamaraAddress>,
}

#[derive(Debug, Serialize)]
pub struct TamaraLineItem {
    pub name: String,
    pub quantity: i32,
    pub reference_id: String,
    pub r#type: String,
    pub sku: String,
    pub unit_price: TamaraAmount,
    pub total_amount: TamaraAmount,
}

#[derive(Debug, Serialize)]
pub struct TamaraConsumer {
    pub first_name: String,
    pub last_name: String,
    pub phone_number: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct TamaraMerchantUrl {
    pub success: String,
    pub failure: String,
    pub cancel: String,
    pub notification: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TamaraAddress {
    pub first_name: String,
    pub last_name: String,
    pub line1: String,
    pub city: String,
    pub country_code: String,
    pub phone_number: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TamaraPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
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
        let amount = router_data.request.amount.get_amount_as_i64();
        let currency = router_data.request.currency.to_string();
        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let webhook = router_data
            .request
            .webhook_url
            .clone()
            .unwrap_or_default();
        let return_url = router_data
            .request
            .router_return_url
            .clone()
            .unwrap_or_default();
        let shipping_amount = router_data
            .request
            .shipping_cost
            .map(|s| s.get_amount_as_i64())
            .unwrap_or(0);
        let tax_amount = router_data
            .request
            .order_tax_amount
            .map(|t| t.get_amount_as_i64())
            .unwrap_or(0);
        let country_code = router_data
            .resource_common_data
            .get_optional_shipping_country()
            .or(router_data.resource_common_data.get_optional_billing_country())
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "country_code",
                    context: Default::default()
                }
            ))?
            .to_string();
        let description = router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_default();
        let email = router_data
            .resource_common_data
            .get_optional_shipping_email()
            .or(router_data.resource_common_data.get_optional_billing_email())
            .or(router_data.request.email.clone())
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: Default::default()
                }
            ))?
            .peek()
            .to_string();
        let customer_name = router_data.request.customer_name.clone();
        let first_name = router_data
            .resource_common_data
            .get_optional_shipping_first_name()
            .or(router_data.resource_common_data.get_optional_billing_first_name())
            .map(|n| n.peek().to_string())
            .or_else(|| {
                customer_name
                    .as_ref()
                    .and_then(|name| name.split_whitespace().next())
                    .map(|s| s.to_string())
            })
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "first_name",
                    context: Default::default()
                }
            ))?;
        let last_name = router_data
            .resource_common_data
            .get_optional_shipping_last_name()
            .or(router_data.resource_common_data.get_optional_billing_last_name())
            .map(|n| n.peek().to_string())
            .or_else(|| {
                customer_name
                    .as_ref()
                    .and_then(|name| name.split_whitespace().nth(1))
                    .map(|s| s.to_string())
            })
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "last_name",
                    context: Default::default()
                }
            ))?;
        let phone_number = router_data
            .resource_common_data
            .address
            .get_shipping()
            .and_then(|a| a.clone().phone.and_then(|p| p.number))
            .or_else(|| router_data.resource_common_data.get_optional_billing_phone_number())
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "phone_number",
                    context: Default::default()
                }
            ))?
            .peek()
            .to_string();
        let shipping_line1 = router_data
            .resource_common_data
            .get_optional_shipping_line1()
            .or(router_data.resource_common_data.get_optional_billing_line1())
            .map(|l| l.peek().to_string())
            .unwrap_or_default();
        let shipping_city = router_data
            .resource_common_data
            .get_optional_shipping_city()
            .or(router_data.resource_common_data.get_optional_billing_city())
            .map(|c| c.peek().to_string())
            .unwrap_or_default();
        let shipping_country = router_data
            .resource_common_data
            .get_optional_shipping_country()
            .or(router_data.resource_common_data.get_optional_billing_country())
            .map(|c| c.to_string())
            .unwrap_or_default();
        let shipping_first_name = router_data
            .resource_common_data
            .get_optional_shipping_first_name()
            .or(router_data.resource_common_data.get_optional_billing_first_name())
            .map(|n| n.peek().to_string())
            .or_else(|| Some(first_name.clone()))
            .unwrap_or_default();
        let shipping_last_name = router_data
            .resource_common_data
            .get_optional_shipping_last_name()
            .or(router_data.resource_common_data.get_optional_billing_last_name())
            .map(|n| n.peek().to_string())
            .or_else(|| Some(last_name.clone()))
            .unwrap_or_default();
        let shipping_phone = router_data
            .resource_common_data
            .address
            .get_shipping()
            .and_then(|a| a.clone().phone.and_then(|p| p.number))
            .or_else(|| router_data.resource_common_data.get_optional_billing_phone_number())
            .map(|p| p.peek().to_string())
            .or_else(|| Some(phone_number.clone()))
            .unwrap_or_default();

        Ok(Self {
            total_amount: TamaraAmount {
                amount,
                currency: currency.clone(),
            },
            shipping_amount: TamaraAmount {
                amount: shipping_amount,
                currency: currency.clone(),
            },
            tax_amount: TamaraAmount {
                amount: tax_amount,
                currency: currency.clone(),
            },
            order_reference_id: order_ref.clone(),
            country_code,
            description: description.clone(),
            items: vec![TamaraLineItem {
                name: description.clone(),
                quantity: 1,
                reference_id: order_ref.clone(),
                r#type: "Physical".to_string(),
                sku: order_ref.clone(),
                unit_price: TamaraAmount {
                    amount,
                    currency: currency.clone(),
                },
                total_amount: TamaraAmount {
                    amount,
                    currency: currency.clone(),
                },
            }],
            consumer: TamaraConsumer {
                first_name,
                last_name,
                phone_number,
                email,
            },
            merchant_url: TamaraMerchantUrl {
                success: return_url.clone(),
                failure: return_url.clone(),
                cancel: return_url.clone(),
                notification: if webhook.is_empty() {
                    None
                } else {
                    Some(webhook)
                },
            },
            shipping_address: TamaraAddress {
                first_name: shipping_first_name,
                last_name: shipping_last_name,
                line1: shipping_line1,
                city: shipping_city,
                country_code: shipping_country,
                phone_number: shipping_phone,
            },
            billing_address: None,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraPaymentsResponse {
    pub order_id: String,
    pub checkout_id: Option<String>,
    pub checkout_url: Option<String>,
    pub status: TamaraPaymentStatus,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TamaraPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TamaraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());
        let connector_order_id = item.response.order_id.clone();

        let redirection_data = item
            .response
            .checkout_url
            .as_ref()
            .map(|url| Box::new(RedirectForm::Uri { uri: url.clone() }));

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_order_id: Some(connector_order_id),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.checkout_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== PSYNC (calls GET /orders/{order_id}) =====

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraPSyncResponse {
    pub order_id: String,
    pub status: TamaraPaymentStatus,
    pub order_reference_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_reference_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== RSYNC =====

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraRSyncResponse {
    pub order_id: String,
    pub status: TamaraPaymentStatus,
    pub order_reference_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = match item.response.status {
            TamaraPaymentStatus::FullyRefunded => RefundStatus::Success,
            TamaraPaymentStatus::PartiallyRefunded => RefundStatus::Success,
            _ => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.order_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== CAPTURE =====

#[derive(Debug, Serialize)]
pub struct TamaraCaptureRequest {
    pub order_id: String,
    pub total_amount: TamaraAmount,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TamaraCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let order_id = item
            .router_data
            .request
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;
        Ok(Self {
            order_id,
            total_amount: TamaraAmount {
                amount: item
                    .router_data
                    .request
                    .minor_amount_to_capture
                    .get_amount_as_i64(),
                currency: item.router_data.request.currency.to_string(),
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TamaraAmount {
    pub amount: i64,
    pub currency: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraCaptureResponse {
    pub order_id: String,
    pub capture_id: Option<String>,
    pub status: TamaraPaymentStatus,
}

impl TryFrom<ResponseRouterData<TamaraCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TamaraCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.capture_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== VOID =====

#[derive(Debug, Serialize)]
pub struct TamaraVoidRequest {
    pub total_amount: TamaraAmount,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TamaraVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let amount = router_data.request.amount.ok_or(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: Default::default()
            }
        ))?;
        let currency = router_data.request.currency.ok_or(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: Default::default()
            }
        ))?;
        Ok(Self {
            total_amount: TamaraAmount {
                amount: amount.get_amount_as_i64(),
                currency: currency.to_string(),
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraVoidResponse {
    pub order_id: String,
    pub status: TamaraPaymentStatus,
}

impl TryFrom<ResponseRouterData<TamaraVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== REFUND =====

#[derive(Debug, Serialize)]
pub struct TamaraRefundRequest {
    pub total_amount: TamaraAmount,
    pub comment: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for TamaraRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let comment = router_data.request.reason.clone().ok_or(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "reason",
                context: Default::default()
            }
        ))?;
        Ok(Self {
            total_amount: TamaraAmount {
                amount: item
                    .router_data
                    .request
                    .minor_refund_amount
                    .get_amount_as_i64(),
                currency: item.router_data.request.currency.to_string(),
            },
            comment,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraRefundResponse {
    pub order_id: String,
    pub refund_id: String,
    pub status: TamaraRefundStatus,
}

impl TryFrom<ResponseRouterData<TamaraRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data.clone()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookEventType {
    pub order_id: String,
    pub order_reference_id: Option<String>,
    pub order_number: Option<String>,
    pub event_type: String,
    pub data: Option<serde_json::Value>,
}

impl From<TamaraWebhookEventType> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event: TamaraWebhookEventType) -> Self {
        match event.event_type.as_str() {
            "order_approved" | "order_authorised" => Self::PaymentIntentAuthorizationSuccess,
            "order_canceled" => Self::PaymentIntentCancelled,
            "order_captured" => Self::PaymentIntentCaptureSuccess,
            "order_refunded" => Self::RefundSuccess,
            "order_updated" => Self::PaymentIntentProcessing,
            _ => Self::EventNotSupported,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookResourceObject {
    pub order_id: String,
    pub order_reference_id: Option<String>,
    pub order_number: Option<String>,
    pub event_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraSourceVerificationResponse {
    pub order_id: String,
    pub status: TamaraPaymentStatus,
    pub order_reference_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraSourceVerificationResponse, Self>>
    for RouterDataV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TamaraSourceVerificationResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let verification_status = match item.response.status {
            TamaraPaymentStatus::Declined | TamaraPaymentStatus::Expired => {
                VerifyWebhookStatus::SourceNotVerified
            }
            _ => VerifyWebhookStatus::SourceVerified,
        };

        Ok(Self {
            response: Ok(VerifyWebhookSourceResponseData {
                verify_webhook_status: verification_status,
            }),
            ..item.router_data
        })
    }
}
