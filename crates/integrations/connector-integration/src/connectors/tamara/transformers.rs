use std::fmt::Debug;

use crate::connectors::tamara::TamaraRouterData;
use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{types::MinorUnit, Email};
use common_enums::EligibilityStatus;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, PaymentMethodEligibility, RSync, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentMethodEligibilityData,
        PaymentMethodEligibilityResponse, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
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
                    context: errors::IntegrationErrorContext {
                        additional_context: Some("Expected Tamara api_key".to_string()),
                        ..Default::default()
                    }
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
            TamaraPaymentStatus::Declined | TamaraPaymentStatus::Expired => Self::Failure,
            TamaraPaymentStatus::New | TamaraPaymentStatus::Approved => Self::Pending,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TamaraRefundStatus {
    New,
    Declined,
    Expired,
    Approved,
    Authorised,
    PartiallyCaptured,
    FullyCaptured,
    PartiallyRefunded,
    FullyRefunded,
    Canceled,
    Updated,
}

impl From<TamaraRefundStatus> for RefundStatus {
    fn from(status: TamaraRefundStatus) -> Self {
        match status {
            TamaraRefundStatus::FullyRefunded | TamaraRefundStatus::PartiallyRefunded => {
                Self::Success
            }
            TamaraRefundStatus::PartiallyCaptured
            | TamaraRefundStatus::FullyCaptured
            | TamaraRefundStatus::Approved
            | TamaraRefundStatus::Authorised => Self::Pending,
            TamaraRefundStatus::Declined
            | TamaraRefundStatus::Expired
            | TamaraRefundStatus::Canceled
            | TamaraRefundStatus::Updated => Self::Failure,
            TamaraRefundStatus::New => Self::Pending,
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
}

#[derive(Debug, Serialize)]
pub struct TamaraLineItem {
    pub name: String,
    pub quantity: u16,
    pub reference_id: String,
    pub sku: String,
    pub total_amount: TamaraAmount,
}

#[derive(Debug, Serialize)]
pub struct TamaraConsumer {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub phone_number: Secret<String>,
    pub email: Email,
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
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub line1: Secret<String>,
    pub city: Secret<String>,
    pub country_code: Secret<String>,
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
        let amount = router_data.request.amount;
        let currency = router_data.request.currency;
        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let webhook = router_data.request.webhook_url.clone().unwrap_or_default();
        let return_url = router_data
            .request
            .router_return_url
            .clone()
            .unwrap_or_default();
        let shipping_amount = router_data.request.shipping_cost.unwrap_or(MinorUnit(0));
        let tax_amount = router_data.request.order_tax_amount.unwrap_or(MinorUnit(0));
        let country_code = router_data
            .resource_common_data
            .get_shipping_or_billing_country()?
            .to_string();
        let description = router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_default();
        let email = router_data.request.get_email()?;
        let first_name = router_data
            .resource_common_data
            .get_shipping_or_billing_first_name()?;
        let last_name = router_data
            .resource_common_data
            .get_shipping_or_billing_last_name()?;
        let phone_number = router_data
            .resource_common_data
            .get_shipping_or_billing_phone_number_plain()?;
        let shipping_line1 = router_data
            .resource_common_data
            .get_optional_shipping_or_billing_line1_string();
        let shipping_city = router_data
            .resource_common_data
            .get_optional_shipping_or_billing_city_string();
        let shipping_country = router_data
            .resource_common_data
            .get_optional_shipping_or_billing_country_string();

        let items = router_data
            .resource_common_data
            .order_details
            .clone()
            .map(|od| {
                od.iter()
                    .enumerate()
                    .map(|(i, data)| TamaraLineItem {
                        name: data.product_name.clone(),
                        quantity: data.quantity,
                        reference_id: data
                            .product_id
                            .clone()
                            .unwrap_or_else(|| format!("item_{i}")),
                        sku: data
                            .product_id
                            .clone()
                            .unwrap_or_else(|| format!("item_{i}")),
                        total_amount: TamaraAmount {
                            amount: data.amount,
                            currency,
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            total_amount: TamaraAmount { amount, currency },
            shipping_amount: TamaraAmount {
                amount: shipping_amount,
                currency,
            },
            tax_amount: TamaraAmount {
                amount: tax_amount,
                currency,
            },
            order_reference_id: order_ref.clone(),
            country_code,
            description: description.clone(),
            items,
            consumer: TamaraConsumer {
                first_name: first_name.clone(),
                last_name: last_name.clone(),
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
                first_name,
                last_name,
                line1: shipping_line1.into(),
                city: shipping_city.into(),
                country_code: shipping_country.into(),
            },
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
                network_txn_link_id: None,
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
                network_txn_link_id: None,
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
    pub status: TamaraRefundStatus,
    pub order_reference_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status);

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
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Missing connector transaction ID (order_id)".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        Ok(Self {
            order_id,
            total_amount: TamaraAmount {
                amount: item.router_data.request.minor_amount_to_capture,
                currency: item.router_data.request.currency,
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TamaraAmount {
    pub amount: MinorUnit,
    pub currency: Currency,
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
                network_txn_link_id: None,
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
                context: errors::IntegrationErrorContext {
                    additional_context: Some("Missing void amount".to_string()),
                    ..Default::default()
                }
            }
        ))?;
        let currency = router_data.request.currency.ok_or(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: errors::IntegrationErrorContext {
                    additional_context: Some("Missing void currency".to_string()),
                    ..Default::default()
                }
            }
        ))?;
        Ok(Self {
            total_amount: TamaraAmount { amount, currency },
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
                network_txn_link_id: None,
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
    pub merchant_refund_id: String,
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
        let comment = router_data
            .request
            .reason
            .clone()
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "reason",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some("Missing refund reason/comment".to_string()),
                        ..Default::default()
                    }
                }
            ))?;
        Ok(Self {
            total_amount: TamaraAmount {
                amount: item.router_data.request.minor_refund_amount,
                currency: item.router_data.request.currency,
            },
            merchant_refund_id: item.router_data.request.refund_id.clone(),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TamaraWebhookEvent {
    OrderApproved,
    OrderAuthorised,
    OrderCanceled,
    OrderCaptured,
    OrderRefunded,
    OrderUpdated,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookEventType {
    pub order_id: String,
    pub order_reference_id: Option<String>,
    pub order_number: Option<String>,
    pub event_type: TamaraWebhookEvent,
    pub data: Option<serde_json::Value>,
}

impl From<TamaraWebhookEvent> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::PaymentIntentAuthorizationSuccess
            }
            TamaraWebhookEvent::OrderCanceled => Self::PaymentIntentCancelled,
            TamaraWebhookEvent::OrderCaptured => Self::PaymentIntentCaptureSuccess,
            TamaraWebhookEvent::OrderRefunded => Self::RefundSuccess,
            TamaraWebhookEvent::OrderUpdated => Self::PaymentIntentProcessing,
            TamaraWebhookEvent::Unknown => Self::EventNotSupported,
        }
    }
}

impl From<TamaraWebhookEventType> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event: TamaraWebhookEventType) -> Self {
        Self::from(event.event_type)
    }
}

impl From<TamaraWebhookEvent> for AttemptStatus {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::Authorized
            }
            TamaraWebhookEvent::OrderCaptured | TamaraWebhookEvent::OrderRefunded => Self::Charged,
            TamaraWebhookEvent::OrderCanceled => Self::Voided,
            TamaraWebhookEvent::OrderUpdated | TamaraWebhookEvent::Unknown => Self::Pending,
        }
    }
}

impl From<TamaraWebhookEvent> for RefundStatus {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderRefunded => Self::Success,
            _ => Self::Pending,
        }
    }
}

impl From<TamaraWebhookEvent> for EventType {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::PaymentIntentAuthorizationSuccess
            }
            TamaraWebhookEvent::OrderCanceled => Self::PaymentIntentCancelled,
            TamaraWebhookEvent::OrderCaptured => Self::PaymentIntentCaptureSuccess,
            TamaraWebhookEvent::OrderRefunded => Self::RefundSuccess,
            TamaraWebhookEvent::OrderUpdated => Self::PaymentIntentProcessing,
            TamaraWebhookEvent::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

// ===== ELIGIBILITY =====

#[derive(Debug, Serialize)]
pub struct TamaraEligibilityRequest {
    pub order: TamaraEligibilityOrder,
    pub customer: TamaraEligibilityCustomer,
}

#[derive(Debug, Serialize)]
pub struct TamaraEligibilityOrder {
    pub total_amount: TamaraAmount,
    pub country_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TamaraEligibilityCustomer {
    pub phone_number: Secret<String>,
    pub email: Email,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<
                PaymentMethodEligibility,
                PaymentFlowData,
                PaymentMethodEligibilityData,
                PaymentMethodEligibilityResponse,
            >,
            T,
        >,
    > for TamaraEligibilityRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<
                PaymentMethodEligibility,
                PaymentFlowData,
                PaymentMethodEligibilityData,
                PaymentMethodEligibilityResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data.request;
        let customer = data.customer.as_ref().ok_or(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "customer",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Customer details are required for Tamara eligibility check".to_string()
                    ),
                    ..Default::default()
                },
            }
        ))?;
        let email = customer.get_email()?;
        let phone_number = customer.get_phone_number()?;
        // Prefer the explicit country on the request, otherwise derive it from
        // the billing/shipping address carried on the flow data.
        let country_code = data
            .country_code
            .map(|country| country.to_string())
            .or_else(|| {
                let from_address = item
                    .router_data
                    .resource_common_data
                    .get_optional_shipping_or_billing_country_string();
                (!from_address.is_empty()).then_some(from_address)
            });
        Ok(Self {
            order: TamaraEligibilityOrder {
                total_amount: TamaraAmount {
                    amount: data.amount,
                    currency: data.currency,
                },
                country_code,
            },
            customer: TamaraEligibilityCustomer {
                phone_number,
                email,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraEligibilityResponse {
    #[serde(alias = "is_eligible")]
    pub eligible: Option<bool>,
    #[serde(alias = "description")]
    pub message: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraEligibilityResponse, Self>>
    for RouterDataV2<
        PaymentMethodEligibility,
        PaymentFlowData,
        PaymentMethodEligibilityData,
        PaymentMethodEligibilityResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TamaraEligibilityResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let eligibility = if item.response.eligible.unwrap_or(false) {
            EligibilityStatus::Eligible
        } else {
            EligibilityStatus::Ineligible
        };
        Ok(Self {
            response: Ok(PaymentMethodEligibilityResponse {
                eligibility,
                status_code: u32::from(item.http_code),
            }),
            ..item.router_data.clone()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookResourceObject {
    pub order_id: String,
    pub order_reference_id: Option<String>,
    pub order_number: Option<String>,
    pub event_type: TamaraWebhookEvent,
}
