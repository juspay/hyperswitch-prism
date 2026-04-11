//! RazorpayV2 transformers for converting between domain types and RazorpayV2 API types

use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD, Engine};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{consts, pii::Email, types::MinorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund, RepeatPayment, SetupMandate},
    connector_types::{
        MandateReference, PaymentCreateOrderData, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    payment_address::Address,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, UpiData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::connectors::razorpay::transformers::ForeignTryFrom;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

// ============ Authentication Types ============

#[derive(Debug)]
pub enum RazorpayV2AuthType {
    AuthToken(Secret<String>),
    ApiKeySecret {
        api_key: Secret<String>,
        api_secret: Secret<String>,
    },
}

impl RazorpayV2AuthType {
    pub fn generate_authorization_header(&self) -> String {
        match self {
            Self::AuthToken(token) => format!("Bearer {}", token.peek()),
            Self::ApiKeySecret {
                api_key,
                api_secret,
            } => {
                let credentials = format!("{}:{}", api_key.peek(), api_secret.peek());
                let encoded = STANDARD.encode(credentials);
                format!("Basic {encoded}")
            }
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for RazorpayV2AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::RazorpayV2 {
                api_key,
                api_secret,
                ..
            } => match api_secret {
                None => Ok(Self::AuthToken(api_key.to_owned())),
                Some(secret) => Ok(Self::ApiKeySecret {
                    api_key: api_key.to_owned(),
                    api_secret: secret.to_owned(),
                }),
            },
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}
// ============ Router Data Wrapper ============

pub struct RazorpayV2RouterData<
    T,
    U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: MinorUnit,
    pub order_id: Option<String>,
    pub router_data: T,
    pub billing_address: Option<Address>,
    #[allow(dead_code)]
    phantom: Option<std::marker::PhantomData<U>>,
}

impl<T, U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(MinorUnit, T, Option<String>, Option<Address>)> for RazorpayV2RouterData<T, U>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (amount, item, order_id, billing_address): (MinorUnit, T, Option<String>, Option<Address>),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            order_id,
            router_data: item,
            billing_address,
            phantom: None,
        })
    }
}

// Keep backward compatibility for existing usage
impl<T, U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(MinorUnit, T, Option<String>)> for RazorpayV2RouterData<T, U>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (amount, item, order_id): (MinorUnit, T, Option<String>),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            order_id,
            router_data: item,
            billing_address: None,
            phantom: None,
        })
    }
}

// ============ Create Order Types ============

#[derive(Debug, Serialize)]
pub struct RazorpayV2CreateOrderRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub receipt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_capture: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<RazorpayV2Notes>,
}

pub type RazorpayV2Notes = Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2CreateOrderResponse {
    pub id: String,
    pub entity: String,
    pub amount: MinorUnit,
    pub amount_paid: MinorUnit,
    pub amount_due: MinorUnit,
    pub currency: String,
    pub receipt: String,
    pub status: String,
    pub attempts: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<String>,
    pub created_at: i64,
}

// ============ Payment Authorization Types ============

#[derive(Debug, Serialize)]
pub struct RazorpayV2PaymentsRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub order_id: String,
    pub email: Email,
    pub contact: Secret<String>,
    pub method: String,
    pub description: Option<String>,
    pub notes: Option<RazorpayV2Notes>,
    pub callback_url: String,
    pub upi: Option<RazorpayV2UpiDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpiFlow {
    Collect,
    Intent,
}

#[derive(Debug, Serialize)]
pub struct RazorpayV2UpiDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<UpiFlow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpa: Option<Secret<String>>, // Only for collect flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_time: Option<i32>, // In minutes (5 to 5760)
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub upi_type: Option<String>, // "recurring" for mandates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<i64>, // For recurring payments
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2PaymentsResponse {
    pub id: String,
    pub entity: String,
    pub amount: i64,
    pub currency: String,
    pub status: RazorpayStatus,
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub international: Option<bool>,
    pub method: String,
    pub amount_refunded: Option<i64>,
    pub refund_status: Option<String>,
    pub captured: Option<bool>,
    pub description: Option<String>,
    pub card_id: Option<String>,
    pub bank: Option<String>,
    pub wallet: Option<String>,
    pub vpa: Option<Secret<String>>,
    pub email: Email,
    pub contact: Secret<String>,
    pub notes: Option<Value>,
    pub fee: Option<i64>,
    pub tax: Option<i64>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RazorpayStatus {
    Created,
    Authorized,
    Captured,
    Refunded,
    Failed,
}

fn get_psync_razorpay_payment_status(razorpay_status: RazorpayStatus) -> AttemptStatus {
    match razorpay_status {
        RazorpayStatus::Created => AttemptStatus::Pending,
        RazorpayStatus::Authorized => AttemptStatus::Authorized,
        RazorpayStatus::Captured => AttemptStatus::Charged,
        RazorpayStatus::Refunded => AttemptStatus::AutoRefunded,
        RazorpayStatus::Failed => AttemptStatus::Failure,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2OrderPaymentsCollectionResponse {
    pub entity: String,
    pub count: i32,
    pub items: Vec<RazorpayV2PaymentsResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RazorpayV2SyncResponse {
    PaymentResponse(Box<RazorpayV2PaymentsResponse>),
    OrderPaymentsCollection(RazorpayV2OrderPaymentsCollectionResponse),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RazorpayV2UpiPaymentsResponse {
    SuccessIntent {
        razorpay_payment_id: String,
        link: String,
    },
    SuccessCollect {
        razorpay_payment_id: String,
    },
    Error {
        error: RazorpayV2ErrorResponse,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RazorpayV2ErrorResponse {
    StandardError { error: RazorpayV2ErrorDetails },
    SimpleError { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2ErrorDetails {
    pub code: String,
    pub description: String,
    pub source: Option<String>,
    pub step: Option<String>,
    pub reason: Option<String>,
    pub metadata: Option<Value>,
    pub field: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2UpiResponseDetails {
    pub flow: Option<String>,
    pub vpa: Option<Secret<String>>,
    pub expiry_time: Option<i32>,
}

// ============ Error Types ============
// Error response structure is already defined above in the enum

// ============ Request Transformations ============

impl<U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&RazorpayV2RouterData<&PaymentCreateOrderData, U>> for RazorpayV2CreateOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RazorpayV2RouterData<&PaymentCreateOrderData, U>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.amount,
            currency: item.router_data.currency.to_string(),
            receipt: item
                .order_id
                .as_ref()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_request_reference_id",
                    context: Default::default(),
                })?
                .clone(),
            payment_capture: Some(true),
            notes: item.router_data.metadata.clone().expose_option(),
        })
    }
}

impl<U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RazorpayV2RouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<U>,
                PaymentsResponseData,
            >,
            U,
        >,
    > for RazorpayV2PaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &RazorpayV2RouterData<
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<U>,
                PaymentsResponseData,
            >,
            U,
        >,
    ) -> Result<Self, Self::Error> {
        // Determine UPI flow based on payment method data
        let (upi_flow, vpa) = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Upi(upi_data) => match upi_data {
                UpiData::UpiCollect(collect_data) => {
                    let vpa_string = collect_data
                        .vpa_id
                        .as_ref()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "vpa_id",
                            context: Default::default(),
                        })?
                        .peek()
                        .to_string();
                    (Some(UpiFlow::Collect), Some(vpa_string))
                }
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => (Some(UpiFlow::Intent), None),
                // UpiData::UpiQr(_) => {
                //     return Err(errors::IntegrationError::not_implemented("UPI QR flow not supported by RazorpayV2".to_string()).into());
                // }
            },
            _ => (None, None),
        };

        // Build UPI details if this is a UPI payment
        let upi_details = if upi_flow.is_some() {
            Some(RazorpayV2UpiDetails {
                flow: upi_flow,
                vpa: vpa.map(Secret::new),
                expiry_time: Some(15), // 15 minutes default
                upi_type: None,
                end_date: None,
            })
        } else {
            None
        };

        let order_id = item
            .order_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "order_id",
                context: Default::default(),
            })?;

        Ok(Self {
            amount: item.amount,
            currency: item.router_data.request.currency.to_string(),
            order_id: order_id.to_string(),
            email: item
                .router_data
                .resource_common_data
                .get_billing_email()
                .or_else(|_| {
                    Email::from_str("customer@example.com").map_err(|_| {
                        error_stack::Report::new(IntegrationError::InvalidDataFormat {
                            field_name: "billing.email",
                            context: Default::default(),
                        })
                    })
                })?,
            contact: Secret::new(
                item.router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .map(|phone| phone.expose())
                    .unwrap_or_else(|_| "9999999999".to_string()),
            ),
            method: "upi".to_string(),
            description: Some("Payment via RazorpayV2".to_string()),
            notes: item.router_data.request.metadata.clone().expose_option(),
            callback_url: item
                .router_data
                .request
                .get_router_return_url()
                .map_err(|_| IntegrationError::MissingRequiredField {
                    field_name: "router_return_url",
                    context: Default::default(),
                })?,
            upi: upi_details,
            customer_id: None,
            save: Some(false),
            recurring: None,
        })
    }
}

// ============ Refund Types ============

#[derive(Debug, Serialize)]
pub struct RazorpayV2RefundRequest {
    pub amount: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2RefundResponse {
    pub id: String,
    pub entity: String,
    pub amount: i64,
    pub currency: String,
    pub payment_id: String,
    pub status: String,
    pub speed_requested: Option<String>,
    pub speed_processed: Option<String>,
    pub receipt: Option<String>,
    pub created_at: i64,
}

impl<U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&RazorpayV2RouterData<&RefundsData, U>> for RazorpayV2RefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: &RazorpayV2RouterData<&RefundsData, U>) -> Result<Self, Self::Error> {
        let amount_in_minor_units = item.amount.get_amount_as_i64();
        Ok(Self {
            amount: amount_in_minor_units,
        })
    }
}

// ============ Response Transformations ============

impl
    ForeignTryFrom<(
        RazorpayV2RefundResponse,
        Self,
        u16,
        Vec<u8>, // raw_response
    )> for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (response, data, _status_code, _raw_response): (
            RazorpayV2RefundResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        // Map Razorpay refund status to internal status
        let status = match response.status.as_str() {
            "processed" => RefundStatus::Success,
            "pending" | "created" => RefundStatus::Pending,
            "failed" => RefundStatus::Failure,
            _ => RefundStatus::Pending,
        };

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: response.id,
            refund_status: status,
            status_code: _status_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            resource_common_data: RefundFlowData {
                status,
                ..data.resource_common_data.clone()
            },
            ..data
        })
    }
}

impl
    ForeignTryFrom<(
        RazorpayV2RefundResponse,
        Self,
        u16,
        Vec<u8>, // raw_response
    )> for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (response, data, _status_code, _raw_response): (
            RazorpayV2RefundResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        // Map Razorpay refund status to internal status
        let status = match response.status.as_str() {
            "processed" => RefundStatus::Success,
            "pending" | "created" => RefundStatus::Pending,
            "failed" => RefundStatus::Failure,
            _ => RefundStatus::Pending,
        };

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: response.id,
            refund_status: status,
            status_code: _status_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            resource_common_data: RefundFlowData {
                status,
                ..data.resource_common_data.clone()
            },
            ..data
        })
    }
}

impl
    ForeignTryFrom<(
        RazorpayV2SyncResponse,
        Self,
        u16,
        Vec<u8>, // raw_response
    )> for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (sync_response, data, _status_code, _raw_response): (
            RazorpayV2SyncResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        // Extract the payment response from either format
        let payment_response = match sync_response {
            RazorpayV2SyncResponse::PaymentResponse(payment) => *payment,
            RazorpayV2SyncResponse::OrderPaymentsCollection(collection) => {
                // Get the first (and typically only) payment from the collection
                collection.items.into_iter().next().ok_or(
                    crate::utils::response_handling_fail_for_connector(_status_code, "razorpayv2"),
                )?
            }
        };

        // Map Razorpay payment status to internal status, preserving original status
        let status = get_psync_razorpay_payment_status(payment_response.status);

        let payments_response_data = match payment_response.status {
            RazorpayStatus::Created
            | RazorpayStatus::Authorized
            | RazorpayStatus::Captured
            | RazorpayStatus::Refunded => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(payment_response.id),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                connector_response_reference_id: payment_response.order_id,
                incremental_authorization_allowed: None,
                status_code: _status_code,
            }),
            RazorpayStatus::Failed => Err(ErrorResponse {
                code: payment_response
                    .error_code
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: payment_response
                    .error_description
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: payment_response.error_reason,
                status_code: _status_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(payment_response.id),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        Ok(Self {
            response: payments_response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..data.resource_common_data.clone()
            },
            ..data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ForeignTryFrom<(
        RazorpayV2UpiPaymentsResponse,
        Self,
        u16,
        Vec<u8>, // raw_response
    )>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (upi_response, data, _status_code, _raw_response): (
            RazorpayV2UpiPaymentsResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        let (transaction_id, redirection_data) = match upi_response {
            RazorpayV2UpiPaymentsResponse::SuccessIntent {
                razorpay_payment_id,
                link,
            } => {
                let redirect_form = RedirectForm::Uri { uri: link };
                (
                    ResponseId::ConnectorTransactionId(razorpay_payment_id),
                    Some(redirect_form),
                )
            }
            RazorpayV2UpiPaymentsResponse::SuccessCollect {
                razorpay_payment_id,
            } => {
                // For UPI Collect, there's no link, so no redirection data
                (
                    ResponseId::ConnectorTransactionId(razorpay_payment_id),
                    None,
                )
            }
            RazorpayV2UpiPaymentsResponse::Error { error: _ } => {
                // Handle error case - this should probably return an error instead
                return Err(crate::utils::response_handling_fail_for_connector(
                    _status_code,
                    "razorpayv2",
                ));
            }
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: transaction_id,
            redirection_data: redirection_data.map(Box::new),
            connector_metadata: None,
            mandate_reference: None,
            network_txn_id: None,
            connector_response_reference_id: data.resource_common_data.connector_order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: _status_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                ..data.resource_common_data
            },
            ..data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ForeignTryFrom<(
        RazorpayV2PaymentsResponse,
        Self,
        u16,
        Vec<u8>, // raw_response
    )>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (response, data, _status_code, __raw_response): (
            RazorpayV2PaymentsResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id),
            redirection_data: None,
            connector_metadata: None,
            mandate_reference: None,
            network_txn_id: None,
            connector_response_reference_id: data.resource_common_data.connector_order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: _status_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::AuthenticationPending,
                ..data.resource_common_data
            },
            ..data
        })
    }
}

// ============ SetupMandate (Zero Dollar Auth / Token Creation) Types ============

/// SetupMandate request reuses the same payment request structure but with
/// save=true and recurring="1" to create a token for future MIT payments.
#[derive(Debug, Serialize)]
pub struct RazorpayV2SetupMandateRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub order_id: String,
    pub email: Email,
    pub contact: Secret<String>,
    pub method: String,
    pub description: Option<String>,
    pub notes: Option<RazorpayV2Notes>,
    pub callback_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    pub save: bool,
    pub recurring: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<RazorpayV2TokenDetails>,
}

#[derive(Debug, Serialize)]
pub struct RazorpayV2TokenDetails {
    pub max_amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
}

impl<U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RazorpayV2RouterData<
            &RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<U>,
                PaymentsResponseData,
            >,
            U,
        >,
    > for RazorpayV2SetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &RazorpayV2RouterData<
            &RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<U>,
                PaymentsResponseData,
            >,
            U,
        >,
    ) -> Result<Self, Self::Error> {
        let order_id = item
            .order_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "order_id",
                context: Default::default(),
            })?;

        Ok(Self {
            amount: item.amount,
            currency: item.router_data.request.currency.to_string(),
            order_id: order_id.to_string(),
            email: item
                .router_data
                .resource_common_data
                .get_billing_email()
                .or_else(|_| {
                    Email::from_str("customer@example.com").map_err(|_| {
                        error_stack::Report::new(IntegrationError::InvalidDataFormat {
                            field_name: "billing.email",
                            context: Default::default(),
                        })
                    })
                })?,
            contact: Secret::new(
                item.router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .map(|phone| phone.expose())
                    .unwrap_or_else(|_| "9999999999".to_string()),
            ),
            method: "card".to_string(),
            description: Some("Setup mandate for recurring payments".to_string()),
            notes: item.router_data.request.metadata.clone().expose_option(),
            callback_url: item
                .router_data
                .request
                .router_return_url
                .clone()
                .unwrap_or_default(),
            customer_id: item
                .router_data
                .request
                .customer_id
                .as_ref()
                .map(|id| id.get_string_repr().to_string()),
            save: true,
            recurring: "1".to_string(),
            token: Some(RazorpayV2TokenDetails {
                max_amount: Some(1500000),
                expire_at: None,
                frequency: Some("as_presented".to_string()),
            }),
        })
    }
}

/// Response for SetupMandate - extracts token info for mandate_reference
#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2SetupMandateResponse {
    pub id: String,
    pub entity: Option<String>,
    pub amount: Option<i64>,
    pub currency: Option<String>,
    pub status: RazorpayStatus,
    pub order_id: Option<String>,
    pub method: Option<String>,
    pub email: Option<Email>,
    pub contact: Option<Secret<String>>,
    pub token_id: Option<String>,
    pub customer_id: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_reason: Option<String>,
    pub notes: Option<Value>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ForeignTryFrom<(RazorpayV2SetupMandateResponse, Self, u16, Vec<u8>)>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (response, data, _status_code, _raw_response): (
            RazorpayV2SetupMandateResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        let status = match response.status {
            RazorpayStatus::Created | RazorpayStatus::Authorized | RazorpayStatus::Captured => {
                AttemptStatus::Charged
            }
            RazorpayStatus::Failed => AttemptStatus::Failure,
            RazorpayStatus::Refunded => AttemptStatus::AutoRefunded,
        };

        // Build mandate_reference using token_id as the connector_mandate_id
        // This token_id will be used for subsequent RepeatPayment (MIT) calls
        let mandate_reference = response.token_id.as_ref().map(|token_id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(token_id.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        });

        let payments_response_data = match response.status {
            RazorpayStatus::Created
            | RazorpayStatus::Authorized
            | RazorpayStatus::Captured
            | RazorpayStatus::Refunded => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference,
                network_txn_id: None,
                connector_response_reference_id: response.order_id,
                incremental_authorization_allowed: None,
                status_code: _status_code,
            }),
            RazorpayStatus::Failed => Err(ErrorResponse {
                code: response
                    .error_code
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: response
                    .error_description
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: response.error_reason,
                status_code: _status_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(response.id),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        Ok(Self {
            response: payments_response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..data.resource_common_data.clone()
            },
            ..data
        })
    }
}

// ============ RepeatPayment (MIT - Subsequent Recurring Payment) Types ============

/// Request for creating a recurring payment using a stored token (MIT)
#[derive(Debug, Serialize)]
pub struct RazorpayV2RepeatPaymentRequest {
    pub email: Email,
    pub contact: Secret<String>,
    pub amount: MinorUnit,
    pub currency: String,
    pub order_id: String,
    pub customer_id: String,
    pub token: String,
    pub recurring: String,
    pub description: Option<String>,
    pub notes: Option<RazorpayV2Notes>,
}

impl<U: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RazorpayV2RouterData<
            &RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<U>,
                PaymentsResponseData,
            >,
            U,
        >,
    > for RazorpayV2RepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &RazorpayV2RouterData<
            &RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<U>,
                PaymentsResponseData,
            >,
            U,
        >,
    ) -> Result<Self, Self::Error> {
        let order_id = item
            .order_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "order_id",
                context: Default::default(),
            })?;

        // Get the connector_mandate_id (token) from the mandate reference
        let connector_mandate_id = item.router_data.request.connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            },
        )?;

        // Get customer_id from the resource_common_data
        let customer_id = item
            .router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
            .unwrap_or_default();

        Ok(Self {
            email: item.router_data.request.get_email().or_else(|_| {
                Email::from_str("customer@example.com").map_err(|_| {
                    error_stack::Report::new(IntegrationError::InvalidDataFormat {
                        field_name: "email",
                        context: Default::default(),
                    })
                })
            })?,
            contact: Secret::new(
                item.router_data
                    .resource_common_data
                    .get_billing_phone_number()
                    .map(|phone| phone.expose())
                    .unwrap_or_else(|_| "9999999999".to_string()),
            ),
            amount: item.amount,
            currency: item.router_data.request.currency.to_string(),
            order_id: order_id.to_string(),
            customer_id,
            token: connector_mandate_id,
            recurring: "1".to_string(),
            description: Some("Recurring payment via RazorpayV2".to_string()),
            notes: item.router_data.request.metadata.clone().expose_option(),
        })
    }
}

/// Response for RepeatPayment
#[derive(Debug, Serialize, Deserialize)]
pub struct RazorpayV2RepeatPaymentResponse {
    pub id: Option<String>,
    pub razorpay_payment_id: Option<String>,
    pub entity: Option<String>,
    pub amount: Option<i64>,
    pub currency: Option<String>,
    pub status: Option<RazorpayStatus>,
    pub order_id: Option<String>,
    pub method: Option<String>,
    pub customer_id: Option<String>,
    pub token_id: Option<String>,
    pub recurring: Option<bool>,
    pub email: Option<Email>,
    pub contact: Option<Secret<String>>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_reason: Option<String>,
    pub notes: Option<Value>,
    pub fee: Option<i64>,
    pub tax: Option<i64>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ForeignTryFrom<(RazorpayV2RepeatPaymentResponse, Self, u16, Vec<u8>)>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = ConnectorError;

    fn foreign_try_from(
        (response, data, _status_code, _raw_response): (
            RazorpayV2RepeatPaymentResponse,
            Self,
            u16,
            Vec<u8>,
        ),
    ) -> Result<Self, Self::Error> {
        // Extract payment ID from either field
        let payment_id = response
            .id
            .or(response.razorpay_payment_id)
            .unwrap_or_default();

        let status = match response.status {
            Some(RazorpayStatus::Created) => AttemptStatus::Pending,
            Some(RazorpayStatus::Authorized) => AttemptStatus::Authorized,
            Some(RazorpayStatus::Captured) => AttemptStatus::Charged,
            Some(RazorpayStatus::Refunded) => AttemptStatus::AutoRefunded,
            Some(RazorpayStatus::Failed) => AttemptStatus::Failure,
            None => {
                // If no status but we have a payment_id, assume success
                if !payment_id.is_empty() {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Failure
                }
            }
        };

        // Build mandate_reference preserving the token for future use
        let mandate_reference = response.token_id.as_ref().map(|token_id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(token_id.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
            })
        });

        let payments_response_data = if status == AttemptStatus::Failure {
            Err(ErrorResponse {
                code: response
                    .error_code
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: response
                    .error_description
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: response.error_reason,
                status_code: _status_code,
                attempt_status: Some(status),
                connector_transaction_id: if payment_id.is_empty() {
                    None
                } else {
                    Some(payment_id)
                },
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(payment_id),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference,
                network_txn_id: None,
                connector_response_reference_id: response.order_id,
                incremental_authorization_allowed: None,
                status_code: _status_code,
            })
        };

        Ok(Self {
            response: payments_response_data,
            resource_common_data: PaymentFlowData {
                status,
                ..data.resource_common_data.clone()
            },
            ..data
        })
    }
}
