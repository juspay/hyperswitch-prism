use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{pii::Email, MinorUnit};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

// ===== ENUMS FOR STATUS MAPPING =====

/// Celero API response status - matches Hyperswitch implementation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CeleroResponseStatus {
    #[serde(alias = "success", alias = "Success", alias = "SUCCESS")]
    Success,
    #[serde(alias = "error", alias = "Error", alias = "ERROR")]
    Error,
}

/// Celero transaction type - matches Hyperswitch implementation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Sale,
    Authorize,
}

/// Celero transaction status - matches Hyperswitch implementation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CeleroTransactionStatus {
    Approved,
    Declined,
    Error,
    Pending,
    PendingSettlement,
    Settled,
    Voided,
    Reversed,
}

impl From<CeleroTransactionStatus> for AttemptStatus {
    fn from(item: CeleroTransactionStatus) -> Self {
        match item {
            CeleroTransactionStatus::Approved => Self::Authorized,
            CeleroTransactionStatus::Settled => Self::Charged,
            CeleroTransactionStatus::Declined | CeleroTransactionStatus::Error => Self::Failure,
            CeleroTransactionStatus::Pending | CeleroTransactionStatus::PendingSettlement => {
                Self::Pending
            }
            CeleroTransactionStatus::Voided | CeleroTransactionStatus::Reversed => Self::Voided,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CeleroAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for CeleroAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Celero { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// Main error response structure - matches Hyperswitch API spec format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CeleroErrorResponse {
    pub status: CeleroResponseStatus,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// Error details that can be extracted from various response fields
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CeleroErrorDetails {
    pub error_code: Option<String>,
    pub error_message: String,
    pub processor_response_code: Option<String>,
    pub decline_reason: Option<String>,
}

impl From<CeleroErrorResponse> for CeleroErrorDetails {
    fn from(error_response: CeleroErrorResponse) -> Self {
        Self {
            error_code: Some("API_ERROR".to_string()),
            error_message: error_response.msg,
            processor_response_code: None,
            decline_reason: None,
        }
    }
}

// Function to extract error details from transaction response data
impl CeleroErrorDetails {
    pub fn from_transaction_response(response: &CeleroCardResponse, msg: String) -> Self {
        // Map specific error codes based on common response patterns
        let decline_reason = Self::map_processor_error(&response.processor_response_code, &msg);

        Self {
            error_code: response.processor_response_code.clone(),
            error_message: msg,
            processor_response_code: response.processor_response_code.clone(),
            decline_reason,
        }
    }

    pub fn from_top_level_error(msg: String) -> Self {
        Self {
            error_code: Some("API_ERROR".to_string()),
            error_message: msg,
            processor_response_code: None,
            decline_reason: None,
        }
    }

    /// Map processor response codes and messages to specific error codes
    fn map_processor_error(processor_code: &Option<String>, message: &str) -> Option<String> {
        let message_lower = message.to_lowercase();
        // Check processor response codes if available
        if let Some(code) = processor_code {
            match code.as_str() {
                "05" => Some("TRANSACTION_DECLINED".to_string()),
                "14" => Some("INVALID_CARD_DATA".to_string()),
                "51" => Some("INSUFFICIENT_FUNDS".to_string()),
                "54" => Some("EXPIRED_CARD".to_string()),
                "55" => Some("INCORRECT_CVC".to_string()),
                "61" => Some("Exceeds withdrawal amount limit".to_string()),
                "62" => Some("TRANSACTION_DECLINED".to_string()),
                "65" => Some("Exceeds withdrawal frequency limit".to_string()),
                "78" => Some("INVALID_CARD_DATA".to_string()),
                "91" => Some("PROCESSING_ERROR".to_string()),
                "96" => Some("PROCESSING_ERROR".to_string()),
                _ => Some("Transaction failed".to_string()),
            }
        } else {
            Some(message_lower)
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CeleroPaymentsRequest<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub order_id: String,
    pub payment_method: CeleroPaymentMethod<T>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CeleroPaymentMethod<T: PaymentMethodDataTypes> {
    Card { card: CeleroCard<T> },
    Ach { ach: CeleroAch },
}

#[derive(Debug, Serialize)]
pub struct CeleroCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiration_date: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct CeleroAch {
    pub routing_number: Secret<String>,
    pub account_number: Secret<String>,
    pub sec_code: String,
    pub account_type: String,
}

#[derive(Debug, Serialize)]
pub struct CeleroBillingAddress {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub address_line_1: Option<Secret<String>>,
    pub address_line_2: Option<Secret<String>>,
    pub city: Option<String>,
    pub state: Option<Secret<String>>,
    pub postal_code: Option<Secret<String>>,
    pub country: Option<common_enums::CountryAlpha2>,
    pub phone: Option<Secret<String>>,
    pub email: Option<Email>,
}

// Bridge implementation for macro compatibility (CeleroRouterData is created by the macro in celero.rs)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::CeleroRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for CeleroPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::CeleroRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// Owned implementation for efficiency
impl<T: PaymentMethodDataTypes>
    TryFrom<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for CeleroPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item)
    }
}

// Reference implementation for efficiency
impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for CeleroPaymentsRequest<T>
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
        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => CeleroPaymentMethod::Card {
                card: CeleroCard {
                    number: card_data.card_number.clone(),
                    expiration_date: Secret::new(format!(
                        "{}/{}",
                        card_data.card_exp_month.clone().expose(),
                        card_data.card_exp_year.clone().expose()
                    )),
                    cvc: Some(card_data.card_cvc.clone()),
                },
            },
            PaymentMethodData::BankDebit(_bank_debit_data) => {
                return Err(IntegrationError::not_implemented(
                    "ACH payments not yet implemented".to_string(),
                )
                .into())
            }
            _ => {
                return Err(IntegrationError::not_implemented(
                    "Payment method not supported".to_string(),
                )
                .into())
            }
        };

        let is_auto_capture = item.request.is_auto_capture();

        // Validate reference ID is not empty
        let reference_id = &item.resource_common_data.connector_request_reference_id;
        if reference_id.is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "connector_request_reference_id",
                context: Default::default(),
            }
            .into());
        }

        Ok(Self {
            idempotency_key: Some(format!("{reference_id}_idempotency")),
            transaction_type: if is_auto_capture {
                TransactionType::Sale
            } else {
                TransactionType::Authorize
            },
            amount: item.request.minor_amount,
            currency: item.request.currency,
            order_id: reference_id.clone(),
            payment_method,
        })
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CeleroCardResponse {
    pub status: CeleroTransactionStatus,
    pub auth_code: Option<String>,
    pub processor_response_code: Option<String>,
    pub avs_response_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CeleroPaymentMethodResponse {
    Card(CeleroCardResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde_with::skip_serializing_none]
pub struct CeleroTransactionResponseData {
    pub id: String,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub amount: i64,
    pub currency: String,
    pub response: CeleroPaymentMethodResponse,
    pub billing_address: Option<CeleroBillingAddressResponse>,
    pub shipping_address: Option<CeleroBillingAddressResponse>,
    // Additional fields from the sample response
    pub status: Option<String>,
    pub response_code: Option<i32>,
    pub customer_id: Option<String>,
    pub payment_method_id: Option<String>,
}

// Matches Hyperswitch CeleroPaymentsResponse structure
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CeleroPaymentsResponse {
    pub status: CeleroResponseStatus,
    pub msg: String,
    pub data: Option<CeleroTransactionResponseData>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CeleroBillingAddressResponse {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
    address_line_1: Option<Secret<String>>,
    address_line_2: Option<Secret<String>>,
    city: Option<String>,
    state: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
    country: Option<common_enums::CountryAlpha2>,
    phone: Option<Secret<String>>,
    email: Option<Secret<String>>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<CeleroPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CeleroPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.status {
            CeleroResponseStatus::Success => {
                if let Some(data) = item.response.data {
                    let CeleroPaymentMethodResponse::Card(response) = &data.response;
                    // Check if transaction itself failed despite successful API call
                    match response.status {
                        CeleroTransactionStatus::Declined | CeleroTransactionStatus::Error => {
                            // Transaction failed - create error response with transaction details
                            let error_details = CeleroErrorDetails::from_transaction_response(
                                response,
                                item.response.msg,
                            );

                            Ok(Self {
                                response: Err(ErrorResponse {
                                    code: error_details
                                        .error_code
                                        .unwrap_or_else(|| "TRANSACTION_FAILED".to_string()),
                                    message: error_details.error_message,
                                    reason: error_details.decline_reason,
                                    status_code: item.http_code,
                                    attempt_status: None,
                                    connector_transaction_id: Some(data.id),
                                    network_decline_code: None,
                                    network_advice_code: None,
                                    network_error_message: None,
                                }),
                                resource_common_data: PaymentFlowData {
                                    status: AttemptStatus::Failure,
                                    ..item.router_data.resource_common_data
                                },
                                ..item.router_data
                            })
                        }
                        _ => {
                            let final_status: AttemptStatus = response.status.into();
                            Ok(Self {
                                response: Ok(PaymentsResponseData::TransactionResponse {
                                    resource_id: ResponseId::ConnectorTransactionId(
                                        data.id.clone(),
                                    ),
                                    redirection_data: None,
                                    mandate_reference: None, // Mandates not yet implemented in UCS
                                    connector_metadata: None,
                                    network_txn_id: None,
                                    connector_response_reference_id: response.auth_code.clone(),
                                    incremental_authorization_allowed: None,
                                    status_code: item.http_code,
                                }),
                                resource_common_data: PaymentFlowData {
                                    status: final_status,
                                    ..item.router_data.resource_common_data
                                },
                                ..item.router_data
                            })
                        }
                    }
                } else {
                    // No transaction data in successful response
                    Ok(Self {
                        response: Err(ErrorResponse {
                            code: "MISSING_DATA".to_string(),
                            message: "No transaction data in response".to_string(),
                            reason: Some(item.response.msg),
                            status_code: item.http_code,
                            attempt_status: None,
                            connector_transaction_id: None,
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                        }),
                        resource_common_data: PaymentFlowData {
                            status: AttemptStatus::Failure,
                            ..item.router_data.resource_common_data
                        },
                        ..item.router_data
                    })
                }
            }
            CeleroResponseStatus::Error => {
                // Top-level API error
                let error_details =
                    CeleroErrorDetails::from_top_level_error(item.response.msg.clone());

                // Extract transaction ID from the top-level data if available
                let connector_transaction_id =
                    item.response.data.as_ref().map(|data| data.id.clone());

                Ok(Self {
                    response: Err(ErrorResponse {
                        code: error_details
                            .error_code
                            .unwrap_or_else(|| "API_ERROR".to_string()),
                        message: error_details.error_message,
                        reason: error_details.decline_reason,
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}

// ===== PSYNC STRUCTURES =====

// Empty request structure for GET-based transaction lookup
// Using empty struct {} instead of unit struct to serialize to {} instead of null
#[derive(Debug, Serialize)]
pub struct CeleroSyncRequest {}

// Response structure based on Celero API spec for GET /api/transaction/{id}
#[derive(Debug, Deserialize, Serialize)]
pub struct CeleroSyncResponse {
    pub status: CeleroResponseStatus,
    pub msg: String,
    pub data: Vec<CeleroTransactionData>,
    pub total_count: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CeleroTransactionData {
    pub id: String,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub amount: MinorUnit,
    pub tax_amount: Option<MinorUnit>,
    pub tax_exempt: Option<bool>,
    pub shipping_amount: Option<MinorUnit>,
    pub currency: common_enums::Currency,
    pub description: Option<String>,
    pub order_id: Option<String>,
    pub po_number: Option<String>,
    pub ip_address: Option<String>,
    pub email_receipt: Option<bool>,
    pub payment_method: Option<String>,
    pub response: Option<CeleroTransactionResponseDetails>,
    pub status: CeleroTransactionStatus,
    pub billing_address: Option<CeleroBillingAddressResponse>,
    pub shipping_address: Option<CeleroBillingAddressResponse>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CeleroTransactionResponseDetails {
    pub card: Option<CeleroCardResponse>,
}

// ===== PSYNC TRANSFORMATIONS =====

// Bridge implementation for macro compatibility
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::CeleroRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for CeleroSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::CeleroRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

// Response transformation for PSync
impl TryFrom<ResponseRouterData<CeleroSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<CeleroSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if response status indicates success
        if response.status != CeleroResponseStatus::Success {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: "SYNC_ERROR".to_string(),
                    message: response.msg.clone(),
                    reason: Some(format!(
                        "Sync request failed with status: {:?}",
                        response.status
                    )),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract first transaction data (API returns array but we expect single transaction)
        let transaction_data = response.data.first().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "celero: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Extract card response for detailed checking
        let card_response = transaction_data
            .response
            .as_ref()
            .and_then(|r| r.card.as_ref());

        // Map transaction status to attempt status using the enum
        let status = AttemptStatus::from(transaction_data.status);

        // CRITICAL: Check if transaction failed and return error response with processor details
        // This ensures we capture declined/failed transactions properly in sync
        match transaction_data.status {
            CeleroTransactionStatus::Declined | CeleroTransactionStatus::Error => {
                // Transaction failed - return error response with processor details
                return Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Err(ErrorResponse {
                        code: card_response
                            .and_then(|c| c.processor_response_code.clone())
                            .unwrap_or_else(|| "TRANSACTION_DECLINED".to_string()),
                        message: "Transaction declined".to_string(),
                        reason: None,
                        status_code: item.http_code,
                        connector_transaction_id: Some(transaction_data.id.clone()),
                        network_decline_code: card_response
                            .and_then(|c| c.processor_response_code.clone()),
                        network_advice_code: card_response
                            .and_then(|c| c.avs_response_code.clone()),
                        network_error_message: None,
                        ..Default::default()
                    }),
                    ..router_data.clone()
                });
            }
            _ => {}
        }

        // Build success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_data.id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: card_response.and_then(|c| c.auth_code.clone()),
            connector_response_reference_id: transaction_data.order_id.clone(),
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

// ===== CAPTURE STRUCTURES =====

// Capture request structure based on Celero API spec for POST /api/transaction/{id}/capture
#[derive(Debug, Serialize)]
pub struct CeleroCaptureRequest {
    /// Total amount to capture, in cents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
    /// Tax amount to capture, in cents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_amount: Option<MinorUnit>,
    /// Shipping amount to capture, in cents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_amount: Option<MinorUnit>,
    /// Is the transaction tax exempt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_exempt: Option<bool>,
    /// Alphanumeric order identifier (max 17 characters)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    /// Alphanumeric PO number (max 17 characters)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub po_number: Option<String>,
    /// IPV4 or IPV6 address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<Secret<String>>,
}

// Capture response structure (uses same format as payment response)
#[derive(Debug, Deserialize, Serialize)]
pub struct CeleroCaptureResponse {
    pub status: CeleroResponseStatus,
    pub msg: String,
    pub data: Option<serde_json::Value>, // Celero capture returns null data on success
}

// ===== CAPTURE TRANSFORMATIONS =====

// Owned implementation for macro compatibility
impl TryFrom<RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for CeleroCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item)
    }
}

// Reference implementation for efficiency
impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for CeleroCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: Some(item.request.minor_amount_to_capture),
            tax_amount: None,      // Not available in PaymentsCaptureData
            shipping_amount: None, // Not available in PaymentsCaptureData
            tax_exempt: None,      // Not available in PaymentsCaptureData
            order_id: None,        // Not available in PaymentsCaptureData
            po_number: None,       // Not available in PaymentsCaptureData
            ip_address: None,      // Not available in PaymentsCaptureData
        })
    }
}

// Response transformation for Capture
impl TryFrom<ResponseRouterData<CeleroCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CeleroCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if response status indicates success
        let status = match response.status {
            CeleroResponseStatus::Success => AttemptStatus::Charged,
            CeleroResponseStatus::Error => AttemptStatus::Failure,
        };

        // Extract connector transaction ID from the request (should be available)
        let connector_transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::NoResponseId => "unknown".to_string(),
            _ => "unknown".to_string(),
        };

        if response.status != CeleroResponseStatus::Success {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: "CAPTURE_ERROR".to_string(),
                    message: response.msg.clone(),
                    reason: Some(format!(
                        "Capture request failed with status: {:?}",
                        response.status
                    )),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(connector_transaction_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Build success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
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

// ===== REFUND STRUCTURES =====

// Refund request structure based on Celero API spec for POST /api/transaction/{id}/refund
#[derive(Debug, Serialize)]
pub struct CeleroRefundRequest {
    /// Total amount to refund, in cents (optional - defaults to full amount)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
    /// Surcharge amount, in cents (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surcharge: Option<MinorUnit>,
}

// Refund response structure - simplified based on Celero API pattern
#[derive(Debug, Deserialize, Serialize)]
pub struct CeleroRefundResponse {
    pub status: CeleroResponseStatus,
    pub msg: String,
    pub data: Option<serde_json::Value>, // Celero refund returns null data on success
}

// ===== REFUND TRANSFORMATIONS =====

// Owned implementation for macro compatibility
impl TryFrom<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for CeleroRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item)
    }
}

// Reference implementation for efficiency
impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for CeleroRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: Some(item.request.minor_refund_amount),
            surcharge: None, // Not available in RefundsData - could be added if needed
        })
    }
}

// Response transformation for Refund
impl TryFrom<ResponseRouterData<CeleroRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<CeleroRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map response status to refund status
        let refund_status = match response.status {
            CeleroResponseStatus::Success => RefundStatus::Success,
            CeleroResponseStatus::Error => RefundStatus::Failure,
        };

        // Extract connector transaction ID from request
        let connector_refund_id =
            format!("refund_{}", router_data.request.connector_transaction_id);

        if response.status != CeleroResponseStatus::Success {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: "REFUND_ERROR".to_string(),
                    message: response.msg.clone(),
                    reason: Some(format!(
                        "Refund request failed with status: {:?}",
                        response.status
                    )),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(
                        router_data.request.connector_transaction_id.clone(),
                    ),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Build success response
        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// ===== REFUND SYNC STRUCTURES =====

// Empty request structure for GET-based refund status lookup
// Using empty struct {} instead of unit struct to serialize to {} instead of null
#[derive(Debug, Serialize)]
pub struct CeleroRefundSyncRequest {}

// Refund sync uses the same response structure as refund execute
// Both just check top-level API status, not transaction details
pub type CeleroRefundSyncResponse = CeleroRefundResponse;

// ===== REFUND SYNC TRANSFORMATIONS =====

// Bridge implementation for macro compatibility
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::CeleroRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for CeleroRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::CeleroRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

// Response transformation for RSync - matches hyperswitch implementation
impl TryFrom<ResponseRouterData<CeleroRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<CeleroRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        match response.status {
            CeleroResponseStatus::Success => {
                let connector_refund_id = if router_data.request.connector_refund_id.is_empty() {
                    router_data.request.connector_transaction_id.clone()
                } else {
                    router_data.request.connector_refund_id.clone()
                };

                Ok(Self {
                    response: Ok(RefundsResponseData {
                        connector_refund_id,
                        refund_status: RefundStatus::Success,
                        status_code: item.http_code,
                    }),
                    ..router_data.clone()
                })
            }
            CeleroResponseStatus::Error => Ok(Self {
                response: Err(ErrorResponse {
                    code: "REFUND_SYNC_FAILED".to_string(),
                    message: response.msg.clone(),
                    reason: Some(response.msg.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(
                        router_data.request.connector_transaction_id.clone(),
                    ),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            }),
        }
    }
}

// ===== VOID STRUCTURES =====

// Empty request structure for POST-based void operation (no body required)
// Using empty struct {} instead of unit struct to serialize to {} instead of null
#[derive(Debug, Serialize)]
pub struct CeleroVoidRequest {}

// Void response structure based on Celero API spec for POST /api/transaction/{id}/void
#[derive(Debug, Deserialize, Serialize)]
pub struct CeleroVoidResponse {
    pub status: CeleroResponseStatus,
    pub msg: String,
    pub data: Option<serde_json::Value>, // Celero void returns null data on success
}

// ===== VOID TRANSFORMATIONS =====

// Owned implementation for macro compatibility
impl TryFrom<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for CeleroVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item)
    }
}

// Reference implementation for void operation
impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for CeleroVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Empty request for void operation - transaction ID is passed in URL
        Ok(Self {})
    }
}

// Response transformation for Void
impl TryFrom<ResponseRouterData<CeleroVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<CeleroVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if response status indicates success
        let status = match response.status {
            CeleroResponseStatus::Success => AttemptStatus::Voided,
            CeleroResponseStatus::Error => AttemptStatus::VoidFailed,
        };

        // Extract connector transaction ID from the request (should be available)
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        if response.status != CeleroResponseStatus::Success {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: "VOID_ERROR".to_string(),
                    message: response.msg.clone(),
                    reason: Some(format!(
                        "Void request failed with status: {:?}",
                        response.status
                    )),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::VoidFailed),
                    connector_transaction_id: Some(connector_transaction_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Build success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
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

// Additional bridge implementations for macro compatibility

// Capture bridge
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::CeleroRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for CeleroCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::CeleroRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// Void bridge
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::CeleroRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for CeleroVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::CeleroRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// Refund bridge
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::CeleroRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for CeleroRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::CeleroRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}
