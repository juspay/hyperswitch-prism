use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{pii, types::MinorUnit};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

const LANGUAGE: &str = "en";

// Type alias for PaymeRouterData to avoid repetition
type PaymeRouterData<RD, T> = crate::connectors::payme::PaymeRouterData<RD, T>;

// ===== AUTHENTICATION TYPE =====
#[derive(Debug, Clone)]
pub struct PaymeAuthType {
    pub seller_payme_id: Secret<String>,
    pub payme_client_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for PaymeAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Payme {
                seller_payme_id,
                payme_client_key,
                ..
            } => Ok(Self {
                seller_payme_id: seller_payme_id.to_owned(),
                payme_client_key: payme_client_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct PaymeErrorResponse {
    pub status_code: u16,
    pub status_error_details: String,
    pub status_additional_info: serde_json::Value,
    pub status_error_code: u32,
}

// ===== SALE STATUS ENUM =====
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, strum::Display)]
#[serde(rename_all = "kebab-case")]
pub enum SaleStatus {
    Initial,
    Completed,
    Refunded,
    PartialRefund,
    Authorized,
    Voided,
    PartialVoid,
    Failed,
    Chargeback,
}

impl From<SaleStatus> for AttemptStatus {
    fn from(item: SaleStatus) -> Self {
        match item {
            SaleStatus::Initial => Self::Pending,
            SaleStatus::Completed => Self::Charged,
            SaleStatus::Refunded | SaleStatus::PartialRefund => Self::AutoRefunded,
            SaleStatus::Authorized => Self::Authorized,
            SaleStatus::Voided | SaleStatus::PartialVoid => Self::Voided,
            SaleStatus::Failed => Self::Failure,
            SaleStatus::Chargeback => Self::AutoRefunded,
        }
    }
}

impl TryFrom<SaleStatus> for RefundStatus {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(sale_status: SaleStatus) -> Result<Self, Self::Error> {
        match sale_status {
            SaleStatus::Completed | SaleStatus::Refunded | SaleStatus::PartialRefund => {
                Ok(Self::Success)
            }
            SaleStatus::Initial | SaleStatus::Authorized => Ok(Self::Pending),
            SaleStatus::Failed => Ok(Self::Failure),
            SaleStatus::Voided | SaleStatus::PartialVoid | SaleStatus::Chargeback => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "refund status mapping not defined for this sale status".to_string(),
                    connector: "Payme",
                    context: Default::default(),
                }))?
            }
        }
    }
}

// ===== PAYMENT REQUEST STRUCTURES =====
// Simplified authorize request - uses payme_sale_id from CreateOrder
#[derive(Debug, Serialize)]
pub struct PaymePaymentRequest<T: PaymentMethodDataTypes> {
    pub buyer_name: Secret<String>,
    pub buyer_email: pii::Email,
    pub payme_sale_id: String,
    #[serde(flatten)]
    pub card: PaymeCardDetails<T>,
    pub language: String,
}

#[derive(Debug, Serialize)]
pub struct PaymeCardDetails<T: PaymentMethodDataTypes> {
    pub credit_card_number: RawCardNumber<T>,
    pub credit_card_exp: Secret<String>,
    pub credit_card_cvv: Secret<String>,
}

// ===== PAYMENT RESPONSE STRUCTURES =====
#[derive(Debug, Deserialize, Serialize)]
pub struct PaymePaymentResponse {
    pub status_code: i32,
    pub payme_status: String,
    pub payme_sale_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_sale_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_sale_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_status: Option<SaleStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_token_sale: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_signature: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_transaction_total: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_transaction_card_brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_transaction_auth_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_error_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_error_details: Option<String>,
}

// ===== REQUEST TRANSFORMATION =====
// Note: The actual TryFrom will be from PaymeRouterData (generated by create_all_prerequisites! macro)
// We implement a helper function that both can use
fn create_payment_request_from_router_data<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<PaymePaymentRequest<T>, error_stack::Report<IntegrationError>> {
    // Get payme_sale_id from CreateOrder (stored in connector_order_id)
    let payme_sale_id = router_data
        .resource_common_data
        .connector_order_id
        .as_ref()
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "connector_order_id (payme_sale_id from CreateOrder)",
            context: Default::default(),
        })?
        .clone();

    // Get payment method data - only card is supported for no3ds
    let card = match &router_data.request.payment_method_data {
        PaymentMethodData::Card(card_data) => build_card_details(card_data)?,
        _ => {
            return Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Payment method not yet implemented for Payme".to_string(),
                connector: "Payme",
                context: Default::default(),
            }))
        }
    };

    // Get buyer information
    let buyer_email =
        router_data
            .request
            .email
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "email",
                context: Default::default(),
            })?;

    let buyer_name = router_data
        .request
        .customer_name
        .as_ref()
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "customer.name",
            context: Default::default(),
        })?
        .clone();

    Ok(PaymePaymentRequest {
        buyer_name: Secret::new(buyer_name),
        buyer_email,
        payme_sale_id,
        card,
        language: LANGUAGE.to_string(),
    })
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for PaymePaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        create_payment_request_from_router_data(item)
    }
}

// Implementation for the macro-generated PaymeRouterData wrapper type
// This is what the macro actually needs
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaymeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaymePaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaymeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        create_payment_request_from_router_data(&item.router_data)
    }
}

// Helper function to build card details
fn build_card_details<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<PaymeCardDetails<T>, error_stack::Report<IntegrationError>> {
    // Format expiry as MMYY using utility function (e.g., "0322" for March 2022)
    let credit_card_exp = card.get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?;

    Ok(PaymeCardDetails {
        credit_card_number: card.card_number.clone(),
        credit_card_exp,
        credit_card_cvv: card.card_cvc.clone(),
    })
}

// ===== RESPONSE TRANSFORMATION =====
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaymePaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaymePaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Check for error responses - Hyperswitch pattern
        // Error if status_code is non-zero AND (payme_status is not success OR status_error_code exists)
        if response.status_code != 0
            && (response.payme_status != "success" || response.status_error_code.is_some())
        {
            let error_code = response
                .status_error_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| response.status_code.to_string());
            let error_message = response
                .status_error_details
                .clone()
                .unwrap_or_else(|| "Payment failed".to_string());

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(response.payme_sale_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data.clone()
            })
        } else {
            // Map PayMe sale status to AttemptStatus using SaleStatus enum
            let status = response
                .sale_status
                .clone()
                .map(AttemptStatus::from)
                .unwrap_or(AttemptStatus::Pending);

            // Build success response
            let payments_response_data = PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payme_sale_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response.payme_transaction_id.clone(),
                connector_response_reference_id: response.transaction_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Ok(payments_response_data),
                ..item.router_data.clone()
            })
        }
    }
}

// ===== PSYNC REQUEST STRUCTURES =====
#[derive(Debug, Serialize)]
pub struct PaymeSyncRequest {
    pub seller_payme_id: Secret<String>,
    pub sale_payme_id: String,
}

// ===== PSYNC RESPONSE STRUCTURES =====
#[derive(Debug, Deserialize, Serialize)]
pub struct PaymeSyncResponse {
    pub items_count: i32,
    pub items: Vec<PaymeSyncItem>,
    pub status_code: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymeSyncItem {
    pub seller_payme_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_id: Option<String>,
    pub sale_payme_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_payme_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_status: Option<SaleStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_price: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_price_after_fees: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_installments: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_paid_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_auth_number: Option<String>,
}

// ===== PSYNC REQUEST TRANSFORMATION =====
impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for PaymeSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Get authentication
        let auth = PaymeAuthType::try_from(&item.connector_config)?;

        // Extract connector transaction ID (payme_sale_id)
        let sale_payme_id = item.request.get_connector_transaction_id().change_context(
            IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            },
        )?;

        Ok(Self {
            seller_payme_id: auth.seller_payme_id,
            sale_payme_id,
        })
    }
}

// Implementation for the macro-generated PaymeRouterData wrapper type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaymeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PaymeSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaymeRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        (&item.router_data).try_into()
    }
}

// ===== PSYNC RESPONSE TRANSFORMATION =====
impl TryFrom<ResponseRouterData<PaymeSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaymeSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for error responses based on status_code
        if response.status_code != 0 {
            let error_code = response.status_code.to_string();
            let error_message = "Failed to retrieve sale information".to_string();

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            })
        } else {
            // Get the first sale item from the items array
            let sale_item = response.items.first().ok_or(
                crate::utils::response_deserialization_fail(
                    item.http_code,
                "payme: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

            // Map PayMe sale status to AttemptStatus using SaleStatus enum
            let status = sale_item
                .sale_status
                .clone()
                .map(AttemptStatus::from)
                .unwrap_or(AttemptStatus::Pending);

            // Build success response
            let payments_response_data = PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(sale_item.sale_payme_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: sale_item.transaction_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Ok(payments_response_data),
                ..router_data.clone()
            })
        }
    }
}

// ===== CAPTURE REQUEST STRUCTURES =====
#[derive(Debug, Serialize)]
pub struct PaymeCaptureRequest {
    pub seller_payme_id: Secret<String>,
    pub payme_sale_id: String,
    pub sale_price: MinorUnit,
}

// ===== CAPTURE RESPONSE STRUCTURES =====
// Capture response uses the same structure as payment response
pub type PaymeCaptureResponse = PaymePaymentResponse;

// ===== CAPTURE REQUEST TRANSFORMATION =====
impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for PaymeCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Get authentication
        let auth = PaymeAuthType::try_from(&item.connector_config)?;

        // Extract connector transaction ID (payme_sale_id) from ResponseId
        let payme_sale_id = item
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        Ok(Self {
            seller_payme_id: auth.seller_payme_id,
            payme_sale_id,
            sale_price: item.request.minor_amount_to_capture,
        })
    }
}

// Implementation for the macro-generated PaymeRouterData wrapper type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaymeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PaymeCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaymeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        (&item.router_data).try_into()
    }
}

// ===== CAPTURE RESPONSE TRANSFORMATION =====
impl TryFrom<ResponseRouterData<PaymeCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaymeCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Check for error responses - Hyperswitch pattern
        if response.status_code != 0
            && (response.payme_status != "success" || response.status_error_code.is_some())
        {
            let error_code = response
                .status_error_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| response.status_code.to_string());
            let error_message = response
                .status_error_details
                .clone()
                .unwrap_or_else(|| "Capture failed".to_string());

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(response.payme_sale_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data.clone()
            })
        } else {
            // Map PayMe sale status to AttemptStatus using SaleStatus enum
            let status = response
                .sale_status
                .clone()
                .map(AttemptStatus::from)
                .unwrap_or(AttemptStatus::Pending);

            // Build success response
            let payments_response_data = PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payme_sale_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response.payme_transaction_id.clone(),
                connector_response_reference_id: response.transaction_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Ok(payments_response_data),
                ..item.router_data.clone()
            })
        }
    }
}

// ===== REFUND REQUEST STRUCTURES =====
#[derive(Debug, Serialize)]
pub struct PaymeRefundRequest {
    pub seller_payme_id: Secret<String>,
    pub payme_sale_id: String,
    pub sale_refund_amount: MinorUnit,
    pub language: String,
}

// ===== REFUND RESPONSE STRUCTURES =====
// Based on similar connectors and PayMe's response patterns
#[derive(Debug, Deserialize, Serialize)]
pub struct PaymeRefundResponse {
    pub status_code: i32,
    pub payme_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_refund_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payme_sale_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_status: Option<SaleStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_error_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_error_details: Option<String>,
}

// ===== REFUND REQUEST TRANSFORMATION =====
impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for PaymeRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Get authentication
        let auth = PaymeAuthType::try_from(&item.connector_config)?;

        // Extract the original payment transaction ID (payme_sale_id)
        let payme_sale_id = item.request.connector_transaction_id.clone();

        Ok(Self {
            seller_payme_id: auth.seller_payme_id,
            payme_sale_id,
            sale_refund_amount: item.request.minor_refund_amount,
            language: LANGUAGE.to_string(),
        })
    }
}

// Implementation for the macro-generated PaymeRouterData wrapper type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaymeRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for PaymeRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaymeRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        (&item.router_data).try_into()
    }
}

// ===== REFUND RESPONSE TRANSFORMATION =====
impl TryFrom<ResponseRouterData<PaymeRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaymeRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Check for error responses based on status_code or error fields
        if response.status_code != 0 || response.payme_status != "success" {
            let error_code = response
                .status_error_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| response.status_code.to_string());
            let error_message = response
                .status_error_details
                .clone()
                .unwrap_or_else(|| "Refund failed".to_string());

            Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: response.payme_sale_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data.clone()
            })
        } else {
            // Map PayMe refund status to RefundStatus using SaleStatus enum
            let refund_status = response
                .refund_status
                .clone()
                .and_then(|s| RefundStatus::try_from(s).ok())
                .unwrap_or(RefundStatus::Pending);

            // Extract refund ID - use payme_refund_id if available, otherwise payme_sale_id
            let connector_refund_id = response
                .payme_refund_id
                .clone()
                .or_else(|| response.payme_sale_id.clone())
                .unwrap_or_else(|| "unknown".to_string());

            // Build success response
            let refunds_response_data = RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            };

            Ok(Self {
                response: Ok(refunds_response_data),
                ..item.router_data.clone()
            })
        }
    }
}

// ===== RSYNC REQUEST STRUCTURES =====
// RSync uses /get-transactions endpoint (different from PSync!)
// We need to pass the payme_transaction_id from the refund response
#[derive(Debug, Serialize)]
pub struct PaymeRSyncRequest {
    pub seller_payme_id: Secret<String>,
    pub payme_transaction_id: String,
}

// ===== RSYNC RESPONSE STRUCTURES =====
// RSync response structure for transaction query
#[derive(Debug, Deserialize, Serialize)]
pub struct PaymeRSyncResponse {
    pub items: Vec<PaymeTransactionItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymeTransactionItem {
    pub sale_status: Option<SaleStatus>,
    pub payme_transaction_id: String,
}

// ===== RSYNC REQUEST TRANSFORMATION =====
impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for PaymeRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Get authentication
        let auth = PaymeAuthType::try_from(&item.connector_config)?;

        // Extract payme_transaction_id (this is the connector_refund_id from refund response)
        let payme_transaction_id = item.request.connector_refund_id.clone();

        Ok(Self {
            seller_payme_id: auth.seller_payme_id,
            payme_transaction_id,
        })
    }
}

// Implementation for the macro-generated PaymeRouterData wrapper type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaymeRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for PaymeRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaymeRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        (&item.router_data).try_into()
    }
}

// ===== RSYNC RESPONSE TRANSFORMATION =====
impl TryFrom<ResponseRouterData<PaymeRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaymeRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Get the first transaction item from the items array
        let transaction_item = response.items.first().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "payme: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Map PayMe sale status to RefundStatus using SaleStatus enum
        let refund_status = transaction_item
            .sale_status
            .clone()
            .and_then(|s| RefundStatus::try_from(s).ok())
            .unwrap_or(RefundStatus::Pending);

        // Build success response
        let refunds_response_data = RefundsResponseData {
            connector_refund_id: transaction_item.payme_transaction_id.clone(),
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// ===== VOID REQUEST STRUCTURES =====
// Void endpoint: /void-sale (POST) - cancels authorized payment before capture
#[derive(Debug, Serialize)]
pub struct PaymeVoidRequest {
    pub seller_payme_id: Secret<String>,
    pub payme_sale_id: String,
    pub sale_currency: Currency,
    pub language: String,
}

// ===== VOID RESPONSE STRUCTURES =====
// Void response uses similar structure to payment response
pub type PaymeVoidResponse = PaymePaymentResponse;

// ===== VOID REQUEST TRANSFORMATION =====
impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for PaymeVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Get authentication
        let auth = PaymeAuthType::try_from(&item.connector_config)?;

        // Extract connector transaction ID (payme_sale_id) to void
        let payme_sale_id = item.request.connector_transaction_id.clone();

        // Get currency
        let sale_currency =
            item.request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;

        Ok(Self {
            seller_payme_id: auth.seller_payme_id,
            payme_sale_id,
            sale_currency,
            language: LANGUAGE.to_string(),
        })
    }
}

// Implementation for the macro-generated PaymeRouterData wrapper type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaymeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PaymeVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaymeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        (&item.router_data).try_into()
    }
}

// ===== VOID RESPONSE TRANSFORMATION =====
impl TryFrom<ResponseRouterData<PaymeVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaymeVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Check for error responses based on status_code or error fields
        if response.status_code != 0 || response.payme_status != "success" {
            let error_code = response
                .status_error_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| response.status_code.to_string());
            let error_message = response
                .status_error_details
                .clone()
                .unwrap_or_else(|| "Void failed".to_string());

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::VoidFailed),
                    connector_transaction_id: Some(response.payme_sale_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data.clone()
            })
        } else {
            // Map PayMe sale status to AttemptStatus for void
            // Successful void should return "voided" status
            let status = match response.payme_sale_status.as_deref() {
                Some("voided") => AttemptStatus::Voided,
                Some("pending") => AttemptStatus::Pending,
                Some("failed") => AttemptStatus::VoidFailed,
                _ => AttemptStatus::Voided, // Default to Voided for success response
            };

            // Build success response
            let payments_response_data = PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.payme_sale_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response.payme_transaction_id.clone(),
                connector_response_reference_id: response.transaction_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Ok(payments_response_data),
                ..item.router_data.clone()
            })
        }
    }
}

// ===== CREATE ORDER (GENERATE SALE) FLOW =====
// Preprocessing flow for non-3DS payments

#[derive(Debug, Serialize)]
pub struct PaymeGenerateSaleRequest {
    pub seller_payme_id: Secret<String>,
    pub sale_type: String,
    pub sale_price: MinorUnit,
    pub currency: Currency,
    pub sale_payment_method: String,
    pub product_name: Option<String>,
    pub transaction_id: String,
    pub sale_callback_url: Option<String>,
    pub sale_return_url: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymeGenerateSaleResponse {
    pub status_code: i32,
    pub sale_url: Option<String>,
    pub payme_sale_id: String,
    pub payme_sale_code: Option<i64>,
    pub price: Option<MinorUnit>,
    pub transaction_id: Option<String>,
    pub currency: Option<Currency>,
}

// Request Transformer
impl
    TryFrom<
        &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    > for PaymeGenerateSaleRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaymeAuthType::try_from(&item.connector_config)?;

        // For CreateOrder, we default to "authorize" (manual capture)
        // The actual capture behavior is determined later in the payment flow
        let sale_type = "authorize";

        Ok(Self {
            seller_payme_id: auth.seller_payme_id,
            sale_type: sale_type.to_string(),
            sale_price: item.request.amount,
            currency: item.request.currency,
            sale_payment_method: "credit-card".to_string(), // Only card for no3ds
            product_name: item
                .request
                .metadata
                .as_ref()
                .and_then(|meta| meta.peek().get("product_name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()), // Extract from metadata
            transaction_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            sale_callback_url: item.request.webhook_url.clone(),
            sale_return_url: item.resource_common_data.return_url.clone(),
            language: Some(LANGUAGE.to_string()),
        })
    }
}

// Macro wrapper transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaymeRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for PaymeGenerateSaleRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaymeRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// Response Transformer
impl TryFrom<ResponseRouterData<PaymeGenerateSaleResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaymeGenerateSaleResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Check for error responses
        if response.status_code != 0 {
            let error_message = format!(
                "Order creation failed with status code: {}",
                response.status_code
            );

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: response.status_code.to_string(),
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(response.payme_sale_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data.clone()
            })
        } else {
            // Success response
            let order_response = PaymentCreateOrderResponse {
                connector_order_id: response.payme_sale_id.clone(),
                session_data: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Pending,
                    reference_id: Some(response.payme_sale_id.clone()), // Store payme_sale_id for subsequent Authorize call
                    connector_order_id: Some(response.payme_sale_id), // Store payme_sale_id for subsequent Authorize call
                    ..item.router_data.resource_common_data.clone()
                },
                response: Ok(order_response),
                ..item.router_data.clone()
            })
        }
    }
}
