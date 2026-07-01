use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::{Authorize, CreateConnectorCustomer, CreateOrder, GetConnectorCustomer, PSync, RSync, Refund},
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, EventType, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RefundWebhookDetailsResponse, ResponseId, WebhookDetailsResponse,
    },
    errors,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::connectors::glomopay::GlomopayAmountConvertor;

// ============================================================================
// Auth
// ============================================================================

#[derive(Debug, Clone)]
pub struct GlomopayAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GlomopayAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Glomopay { api_key, .. } => {
                Ok(Self { api_key: api_key.clone() })
            }
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext::default()
                }
            )),
        }
    }
}

// ============================================================================
// Error response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlomopayErrorResponse {
    pub error: Option<String>,
    pub message: Option<String>,
}

// ============================================================================
// Status enums
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayPaymentStatus {
    InProgress,
    ActionRequired,
    Pending,
    Success,
    Failed,
}

impl From<GlomopayPaymentStatus> for AttemptStatus {
    fn from(status: GlomopayPaymentStatus) -> Self {
        match status {
            GlomopayPaymentStatus::InProgress => Self::AuthenticationPending,
            GlomopayPaymentStatus::ActionRequired | GlomopayPaymentStatus::Pending => {
                Self::Pending
            }
            GlomopayPaymentStatus::Success => Self::Charged,
            GlomopayPaymentStatus::Failed => Self::Failure,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayRefundStatus {
    Success,
    Failed,
    UnderReview,
    Pending,
    ActionRequired,
}

impl From<GlomopayRefundStatus> for RefundStatus {
    fn from(status: GlomopayRefundStatus) -> Self {
        match status {
            GlomopayRefundStatus::Success => Self::Success,
            GlomopayRefundStatus::Failed => Self::Failure,
            GlomopayRefundStatus::UnderReview => Self::ManualReview,
            GlomopayRefundStatus::Pending | GlomopayRefundStatus::ActionRequired => Self::Pending,
        }
    }
}

// ============================================================================
// Webhooks
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayWebhookEventType {
    Success,
    InProgress,
    ActionRequired,
    Failed,
    FundsAvailable,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookFeeDetail {
    pub currency: Option<Currency>,
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookFees {
    pub txn_fee: Option<GlomopayWebhookFeeDetail>,
    pub fx_fee: Option<GlomopayWebhookFeeDetail>,
    pub referral_fee: Option<GlomopayWebhookFeeDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookPaymentData {
    pub id: String,
    pub payin_id: Option<String>,
    pub requested_amount: Option<MinorUnit>,
    pub requested_currency: Option<Currency>,
    pub payment_amount: Option<MinorUnit>,
    pub payment_currency: Option<Currency>,
    pub fees: Option<GlomopayWebhookFees>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_message: Option<String>,
    pub funds_available: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookPayload {
    pub entity_type: String,
    pub event_type: GlomopayWebhookEventType,
    pub data: GlomopayWebhookPaymentData,
}

impl GlomopayWebhookPayload {
    pub fn get_event_type(&self) -> EventType {
        match self.event_type {
            GlomopayWebhookEventType::Success | GlomopayWebhookEventType::FundsAvailable => {
                EventType::PaymentIntentSuccess
            }
            GlomopayWebhookEventType::Failed => EventType::PaymentIntentFailure,
            GlomopayWebhookEventType::InProgress => EventType::PaymentIntentProcessing,
            GlomopayWebhookEventType::ActionRequired => EventType::PaymentActionRequired,
        }
    }

    pub fn get_attempt_status(&self) -> AttemptStatus {
        match self.event_type {
            GlomopayWebhookEventType::Success | GlomopayWebhookEventType::FundsAvailable => {
                AttemptStatus::Charged
            }
            GlomopayWebhookEventType::Failed => AttemptStatus::Failure,
            GlomopayWebhookEventType::InProgress => AttemptStatus::AuthenticationPending,
            GlomopayWebhookEventType::ActionRequired => AttemptStatus::Pending,
        }
    }

    pub fn into_webhook_details_response(self, http_code: u16) -> WebhookDetailsResponse {
        let status = self.get_attempt_status();
        let (error_code, error_message) = match self.event_type {
            GlomopayWebhookEventType::Failed => (
                self.data.error_code.clone(),
                self.data
                    .error_message
                    .clone()
                    .or_else(|| self.data.error_description.clone()),
            ),
            _ => (None, None),
        };
        WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(self.data.id.clone())),
            status,
            connector_response_reference_id: Some(self.data.id),
            mandate_reference: None,
            error_code,
            error_message,
            error_reason: None,
            raw_connector_response: None,
            status_code: http_code,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        }
    }
}

// ============================================================================
// Refund Webhooks
// ============================================================================

/// Minimal probe struct — reads only entity_type to route to the correct handler.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayWebhookEntityProbe {
    pub entity_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlomopayRefundWebhookEventType {
    Success,
    ActionRequired,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookFeeDetail {
    pub currency: Option<Currency>,
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookFees {
    pub txn_fees: Option<GlomopayRefundWebhookFeeDetail>,
    pub fx_fees: Option<GlomopayRefundWebhookFeeDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookData {
    pub id: String,
    pub payment_id: Option<String>,
    pub amount: Option<MinorUnit>,
    pub currency: Option<Currency>,
    pub fees: Option<GlomopayRefundWebhookFees>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundWebhookPayload {
    pub entity_type: String,
    pub event_type: GlomopayRefundWebhookEventType,
    pub data: GlomopayRefundWebhookData,
}

impl GlomopayRefundWebhookPayload {
    pub fn get_event_type(&self) -> EventType {
        match self.event_type {
            GlomopayRefundWebhookEventType::Success => EventType::RefundSuccess,
            GlomopayRefundWebhookEventType::Failed
            | GlomopayRefundWebhookEventType::ActionRequired => EventType::RefundFailure,
        }
    }

    pub fn get_refund_status(&self) -> common_enums::RefundStatus {
        match self.event_type {
            GlomopayRefundWebhookEventType::Success => common_enums::RefundStatus::Success,
            GlomopayRefundWebhookEventType::Failed
            | GlomopayRefundWebhookEventType::ActionRequired => {
                common_enums::RefundStatus::Failure
            }
        }
    }

    pub fn into_refund_webhook_details_response(self, http_code: u16) -> RefundWebhookDetailsResponse {
        RefundWebhookDetailsResponse {
            connector_refund_id: Some(self.data.id.clone()),
            status: self.get_refund_status(),
            connector_response_reference_id: Some(self.data.id),
            error_code: None,
            error_message: None,
            raw_connector_response: None,
            status_code: http_code,
            response_headers: None,
        }
    }
}

// ============================================================================
// GetConnectorCustomer (search by email — GET /customer?email_address=...)
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayCustomerItem {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayGetCustomerResponse {
    pub data: Vec<GlomopayCustomerItem>,
}

impl TryFrom<ResponseRouterData<GlomopayGetCustomerResponse, Self>>
    for RouterDataV2<
        GetConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayGetCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        // If customer found in search results, return existing ID
        // If empty, return an error indicating the customer does not exist yet
        let router_response = match response.data.into_iter().next() {
            Some(customer) => Ok(ConnectorCustomerResponse {
                connector_customer_id: customer.id,
                status_code: item.http_code,
            }),
            None => Err(domain_types::router_data::ErrorResponse {
                status_code: item.http_code,
                code: "CUSTOMER_NOT_FOUND".to_string(),
                message: "No existing customer found for the provided email".to_string(),
                reason: None,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };
        Ok(Self {
            response: router_response,
            ..item.router_data
        })
    }
}

// ============================================================================
// CreateConnectorCustomer
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayCreateCustomerRequest {
    pub name: String,
    pub customer_type: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub pincode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayCreateCustomerResponse {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for GlomopayCreateCustomerRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;
        let req = &router_data.request;
        let flow_data = &router_data.resource_common_data;

        let name = req
            .name
            .as_ref()
            .map(|n| n.peek().clone())
            .unwrap_or_else(|| "Customer".to_string());

        let email = req
            .email
            .as_ref()
            .map(|e| e.peek().peek().to_string())
            .unwrap_or_default();

        let phone = req
            .phone
            .as_ref()
            .map(|p| p.peek().clone())
            .unwrap_or_default();

        let address = flow_data
            .get_optional_billing_line1()
            .map(|l| l.expose())
            .unwrap_or_default();

        let city = flow_data
            .get_optional_billing_city()
            .map(|c| c.expose())
            .unwrap_or_default();

        let state = flow_data
            .get_optional_billing_state()
            .map(|s| s.expose())
            .unwrap_or_default();

        let country = flow_data
            .get_optional_billing_country()
            .map(|c| c.to_string())
            .unwrap_or_default();

        let zip = flow_data
            .get_optional_billing_zip()
            .map(|z| z.expose())
            .unwrap_or_default();

        Ok(Self {
            name,
            customer_type: "individual".to_string(),
            email,
            phone,
            address,
            city,
            state,
            country,
            pincode: zip,
        })
    }
}

impl TryFrom<ResponseRouterData<GlomopayCreateCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayCreateCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: response.id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ============================================================================
// CreateOrder
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayCreateOrderRequest {
    pub customer_id: String,
    pub currency: Currency,
    pub amount: MinorUnit,
    pub product: GlomopayProduct,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayProduct {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayCreateOrderResponse {
    pub id: String,
    pub status: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for GlomopayCreateOrderRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let amount = GlomopayAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let customer_id = router_data
            .resource_common_data
            .connector_customer
            .clone()
            .unwrap_or_else(|| router_data.resource_common_data.payment_id.clone());

        Ok(Self {
            customer_id,
            currency: router_data.request.currency,
            amount,
            product: GlomopayProduct {
                name: "Payment".to_string(),
                description: "Payment via Glomopay".to_string(),
            },
            request_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

impl TryFrom<ResponseRouterData<GlomopayCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let order_id = response.id.clone();

        Ok(Self {
            response: Ok(PaymentCreateOrderResponse {
                connector_order_id: order_id.clone(),
                session_data: None,
            }),
            resource_common_data: PaymentFlowData {
                connector_order_id: Some(order_id.clone()),
                reference_id: Some(order_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// Authorize
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayAuthorizeRequest {
    pub order_id: String,
    pub method: String,
    pub sequence: String,
    pub card: Option<GlomopayCard>,
    pub callback_url: Option<String>,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayCard {
    pub holder_name: Option<Secret<String>>,
    pub number: Secret<String>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvv: Secret<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayAuthorizeResponse {
    pub payment_id: String,
    pub status: GlomopayPaymentStatus,
    pub next_steps: Option<Vec<GlomopayNextStep>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayNextStep {
    pub action: String,
    pub payload: Option<GlomopayNextStepPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayNextStepPayload {
    pub url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GlomopayAuthorizeRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let order_id = router_data
            .resource_common_data
            .connector_order_id
            .clone()
            .unwrap_or_else(|| {
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        let (method, card) = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let expiry_year_2digit = get_card_expiry_year_2_digit(card)?;
                (
                    "card".to_string(),
                    Some(GlomopayCard {
                        holder_name: card.card_holder_name.clone(),
                        number: Secret::new(card.card_number.peek().to_string()),
                        expiry_month: card.card_exp_month.clone(),
                        expiry_year: expiry_year_2digit,
                        cvv: card.card_cvc.clone(),
                    }),
                )
            }
            other => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        format!(
                            "Glomopay Authorize does not support payment method variant: {other:?}"
                        ),
                        Default::default(),
                    )
                ));
            }
        };

        Ok(Self {
            order_id,
            method,
            sequence: "initial".to_string(),
            card,
            callback_url: router_data.resource_common_data.return_url.clone(),
            request_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

fn get_card_expiry_year_2_digit<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static>(
    card: &Card<T>,
) -> Result<Secret<String>, error_stack::Report<errors::IntegrationError>> {
    let year = card.card_exp_year.peek().to_string();
    // Normalize: if 4 digits take last 2, if already 2 digits keep as-is
    let year_2digit = if year.len() == 4 {
        year[2..].to_string()
    } else {
        year
    };
    Ok(Secret::new(year_2digit))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GlomopayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = AttemptStatus::from(response.status);

        let redirect_url = response
            .next_steps
            .as_ref()
            .and_then(|steps| steps.iter().find(|s| s.action == "redirect"))
            .and_then(|step| step.payload.as_ref())
            .and_then(|p| p.url.clone());

        let redirection_data = redirect_url.map(|url| {
            Box::new(RedirectForm::Form {
                endpoint: url,
                method: common_utils::request::Method::Get,
                form_fields: HashMap::new(),
            })
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.payment_id),
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

// ============================================================================
// PSync
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayPaymentSyncItem {
    pub id: String,
    pub status: GlomopayPaymentStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayPageMeta {
    pub total: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayPaymentSyncResponse {
    pub data: Vec<GlomopayPaymentSyncItem>,
    pub page_meta: Option<GlomopayPageMeta>,
}

impl TryFrom<ResponseRouterData<GlomopayPaymentSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayPaymentSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let payment = response.data.into_iter().next().ok_or_else(|| {
            error_stack::report!(crate::utils::response_deserialization_fail(
                item.http_code,
                "glomopay: PSync response contained no payment entries",
            ))
        })?;

        let status = AttemptStatus::from(payment.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(payment.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(payment.id),
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

// ============================================================================
// Refund
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct GlomopayRefundRequest {
    pub payment_id: String,
    pub reason: String,
    pub amount: MinorUnit,
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundResponse {
    pub id: String,
    pub status: GlomopayRefundStatus,
    pub amount: Option<MinorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for GlomopayRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        wrapper: crate::connectors::glomopay::GlomopayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = wrapper.router_data;

        let amount = GlomopayAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            payment_id: router_data.request.connector_transaction_id.clone(),
            reason: "Requested By Customer".to_string(),
            amount,
            request_id: router_data.request.refund_id.clone(),
        })
    }
}

impl TryFrom<ResponseRouterData<GlomopayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let refund_status = RefundStatus::from(response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ============================================================================
// RSync
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundSyncItem {
    pub id: String,
    pub status: GlomopayRefundStatus,
    pub amount: Option<MinorUnit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlomopayRefundSyncResponse {
    pub data: Vec<GlomopayRefundSyncItem>,
    pub page_meta: Option<GlomopayPageMeta>,
}

impl TryFrom<ResponseRouterData<GlomopayRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GlomopayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;
        let connector_refund_id = router_data.request.connector_refund_id.clone();

        let refund_entry = response
            .data
            .iter()
            .find(|r| r.id == connector_refund_id)
            .or_else(|| response.data.first());

        let (refund_status, resolved_refund_id) = match refund_entry {
            Some(entry) => (RefundStatus::from(entry.status), entry.id.clone()),
            None => (RefundStatus::Pending, connector_refund_id),
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: resolved_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}
