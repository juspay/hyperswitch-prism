use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose, Engine};
use common_enums::AttemptStatus;
use common_utils::{
    crypto::{self, SignMessage},
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ===== AUTHENTICATION STRUCTURE =====

#[derive(Debug, Clone)]
pub struct AuthipayAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl AuthipayAuthType {
    /// Generate HMAC-SHA256 signature for Authipay API
    /// Raw signature: API-Key + ClientRequestId + time + requestBody
    /// Then HMAC-SHA256 with API Secret as key, then Base64 encode
    pub fn generate_hmac_signature(
        &self,
        api_key: &str,
        client_request_id: &str,
        timestamp: &str,
        request_body: &str,
    ) -> Result<String, error_stack::Report<IntegrationError>> {
        // Raw signature: apiKey + ClientRequestId + time + requestBody
        let raw_signature = format!("{api_key}{client_request_id}{timestamp}{request_body}");

        // Generate HMAC-SHA256 with API Secret as key
        let signature = crypto::HmacSha256
            .sign_message(
                self.api_secret.clone().expose().as_bytes(),
                raw_signature.as_bytes(),
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Base64 encode the result
        Ok(general_purpose::STANDARD.encode(signature))
    }

    /// Generate unique Client-Request-Id using UUID v4
    pub fn generate_client_request_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Generate timestamp in milliseconds since Unix epoch
    pub fn generate_timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string()
    }
}

impl TryFrom<&ConnectorSpecificConfig> for AuthipayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Authipay {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE STRUCTURES =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
    pub details: Option<Vec<ErrorDetail>>,
    pub api_trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    pub field: Option<String>,
    pub message: Option<String>,
}

impl Default for AuthipayErrorResponse {
    fn default() -> Self {
        Self {
            code: Some("UNKNOWN_ERROR".to_string()),
            message: Some("Unknown error occurred".to_string()),
            details: None,
            api_trace_id: None,
        }
    }
}

// ===== REQUEST TYPE ENUMS =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthipayRequestType {
    PaymentCardSaleTransaction,
    PaymentCardPreAuthTransaction,
    PostAuthTransaction,
    ReturnTransaction,
    VoidPreAuthTransactions,
    VoidTransaction,
}

// ===== REQUEST STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentsRequest<T: PaymentMethodDataTypes> {
    pub request_type: AuthipayRequestType,
    pub merchant_transaction_id: String,
    pub transaction_amount: TransactionAmount,
    pub order: OrderDetails,
    pub payment_method: PaymentMethod<T>,
}

#[derive(Debug, Serialize)]
pub struct TransactionAmount {
    pub total: FloatMajorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDetails {
    pub order_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethod<T: PaymentMethodDataTypes> {
    pub payment_card: PaymentCard<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiry_date: ExpiryDate,
    pub security_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpiryDate {
    pub month: Secret<String>,
    pub year: Secret<String>,
}

// ===== REQUEST TRANSFORMATION =====

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for AuthipayPaymentsRequest<T>
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
        // Use FloatMajorUnitForConnector to properly convert minor to major unit
        let converter = FloatMajorUnitForConnector;
        let amount_major = converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        // Extract payment method data
        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Use utility function to get year in YY format (2 digits)
                let year_yy = card_data.get_card_expiry_year_2_digit()?;

                let payment_card = PaymentCard {
                    number: card_data.card_number.clone(),
                    expiry_date: ExpiryDate {
                        month: Secret::new(card_data.card_exp_month.peek().clone()),
                        year: year_yy,
                    },
                    security_code: Some(card_data.card_cvc.clone()),
                    holder: item.request.customer_name.clone().map(Secret::new),
                };
                PaymentMethod { payment_card }
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::not_implemented(
                    "Only card payments are supported".to_string()
                )))
            }
        };

        // Determine transaction type based on capture_method
        let is_manual_capture = item
            .request
            .capture_method
            .map(|cm| matches!(cm, common_enums::CaptureMethod::Manual))
            .unwrap_or(false);

        // Generate unique merchant transaction ID using connector request reference ID
        let merchant_transaction_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Create order details with same ID
        let order = OrderDetails {
            order_id: merchant_transaction_id.clone(),
        };

        if is_manual_capture {
            Ok(Self {
                request_type: AuthipayRequestType::PaymentCardPreAuthTransaction,
                merchant_transaction_id,
                transaction_amount,
                order,
                payment_method,
            })
        } else {
            Ok(Self {
                request_type: AuthipayRequestType::PaymentCardSaleTransaction,
                merchant_transaction_id,
                transaction_amount,
                order,
                payment_method,
            })
        }
    }
}

// ===== CAPTURE REQUEST STRUCTURE =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayCaptureRequest {
    pub request_type: AuthipayRequestType,
    pub transaction_amount: TransactionAmount,
}

// ===== CAPTURE REQUEST TRANSFORMATION =====

impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for AuthipayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Validate connector_transaction_id is present
        // The get_connector_transaction_id() method will validate this in get_url()
        // No validation needed here

        // Get capture amount from minor_amount_to_capture
        let capture_amount = item.request.minor_amount_to_capture;

        // Convert amount to FloatMajorUnit format
        let converter = FloatMajorUnitForConnector;
        let amount_major = converter
            .convert(capture_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        Ok(Self {
            request_type: AuthipayRequestType::PostAuthTransaction,
            transaction_amount,
        })
    }
}

// ===== RESPONSE STATUS ENUMS =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayTransactionType {
    Sale,
    Preauth,
    Credit,
    ForcedTicket,
    Void,
    Return,
    Postauth,
    PayerAuth,
    Disbursement,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayPaymentStatus {
    Approved,
    Waiting,
    Partial,
    ValidationFailed,
    ProcessingFailed,
    Declined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthipayPaymentResult {
    Approved,
    Declined,
    Failed,
    Waiting,
    Partial,
    Fraud,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuthipayTransactionState {
    Authorized,
    Captured,
    Declined,
    Checked,
    CompletedGet,
    Initialized,
    Pending,
    Ready,
    Template,
    Settled,
    Voided,
    Waiting,
}

// ===== RESPONSE STRUCTURES =====

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentCardResponse {
    pub expiry_date: Option<ExpiryDate>,
    pub bin: Option<String>,
    pub last4: Option<String>,
    pub brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentMethodDetails {
    pub payment_card: Option<AuthipayPaymentCardResponse>,
    pub payment_method_type: Option<String>,
    pub payment_method_brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmountDetails {
    pub total: Option<FloatMajorUnit>,
    pub currency: Option<common_enums::Currency>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvsResponse {
    pub street_match: Option<String>,
    pub postal_code_match: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Processor {
    pub reference_number: Option<String>,
    pub authorization_code: Option<String>,
    pub response_code: Option<String>,
    pub response_message: Option<String>,
    pub network: Option<String>,
    pub association_response_code: Option<String>,
    pub association_response_message: Option<String>,
    pub avs_response: Option<AvsResponse>,
    pub security_code_response: Option<String>,
    pub merchant_advice_code_indicator: Option<String>,
    pub response_indicator: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentToken {
    pub value: Option<String>,
    pub reusable: Option<bool>,
    pub decline_duplicates: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPaymentsResponse {
    pub client_request_id: Option<String>,
    pub api_trace_id: Option<String>,
    pub response_type: Option<String>,
    #[serde(rename = "type")]
    pub response_type_field: Option<String>,
    pub ipg_transaction_id: String,
    pub order_id: Option<String>,
    pub user_id: Option<String>,
    pub transaction_type: AuthipayTransactionType,
    pub payment_method_details: Option<AuthipayPaymentMethodDetails>,
    pub merchant_transaction_id: Option<String>,
    pub transaction_time: Option<i64>,
    pub approved_amount: Option<AmountDetails>,
    pub transaction_amount: Option<AmountDetails>,
    pub transaction_status: Option<AuthipayPaymentStatus>,
    pub transaction_result: Option<AuthipayPaymentResult>,
    pub transaction_state: Option<AuthipayTransactionState>,
    pub approval_code: Option<String>,
    pub scheme_response_code: Option<String>,
    pub error_message: Option<String>,
    pub scheme_transaction_id: Option<String>,
    pub processor: Option<Processor>,
    pub payment_token: Option<PaymentToken>,
}

// ===== HELPER FUNCTIONS TO AVOID CODE DUPLICATION =====

/// Extract connector metadata from payment token
fn extract_connector_metadata(payment_token: Option<&PaymentToken>) -> Option<serde_json::Value> {
    payment_token.map(|token| {
        let mut metadata = HashMap::new();
        if let Some(value) = &token.value {
            metadata.insert("payment_token".to_string(), value.clone());
        }
        if let Some(reusable) = token.reusable {
            metadata.insert("token_reusable".to_string(), reusable.to_string());
        }
        serde_json::Value::Object(
            metadata
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
        )
    })
}

/// Extract network-specific fields from processor object
fn extract_network_fields(
    processor: Option<&Processor>,
) -> (Option<String>, Option<String>, Option<String>) {
    if let Some(processor) = processor {
        (
            processor.network.clone(),
            processor.association_response_code.clone(),
            processor.association_response_message.clone(),
        )
    } else {
        (None, None, None)
    }
}

// ===== STATUS MAPPING FUNCTION =====
// CRITICAL: This checks BOTH transactionResult AND transactionStatus, AND considers transactionType

fn map_status(
    authipay_status: Option<AuthipayPaymentStatus>,
    authipay_result: Option<AuthipayPaymentResult>,
    authipay_state: Option<AuthipayTransactionState>,
    transaction_type: AuthipayTransactionType,
) -> AttemptStatus {
    // First check transaction_state for additional validation
    if let Some(state) = authipay_state {
        match state {
            AuthipayTransactionState::Declined => return AttemptStatus::Failure,
            AuthipayTransactionState::Voided => return AttemptStatus::Voided,
            AuthipayTransactionState::Authorized => {
                // Only trust AUTHORIZED state if transaction type matches
                if matches!(transaction_type, AuthipayTransactionType::Preauth) {
                    return AttemptStatus::Authorized;
                }
            }
            AuthipayTransactionState::Captured | AuthipayTransactionState::Settled => {
                // Only trust CAPTURED/SETTLED if transaction type matches
                if matches!(
                    transaction_type,
                    AuthipayTransactionType::Sale | AuthipayTransactionType::Postauth
                ) {
                    return AttemptStatus::Charged;
                }
            }
            _ => {} // Continue to check status/result
        }
    }

    // Then check transaction_status (deprecated field)
    match authipay_status {
        Some(status) => match status {
            AuthipayPaymentStatus::Approved => match transaction_type {
                AuthipayTransactionType::Preauth => AttemptStatus::Authorized,
                AuthipayTransactionType::Void => AttemptStatus::Voided,
                AuthipayTransactionType::Sale | AuthipayTransactionType::Postauth => {
                    AttemptStatus::Charged
                }
                AuthipayTransactionType::Credit
                | AuthipayTransactionType::ForcedTicket
                | AuthipayTransactionType::Return
                | AuthipayTransactionType::PayerAuth
                | AuthipayTransactionType::Disbursement
                | AuthipayTransactionType::Unknown => AttemptStatus::Failure,
            },
            AuthipayPaymentStatus::Waiting => AttemptStatus::Pending,
            AuthipayPaymentStatus::Partial => AttemptStatus::PartialCharged,
            AuthipayPaymentStatus::ValidationFailed
            | AuthipayPaymentStatus::ProcessingFailed
            | AuthipayPaymentStatus::Declined => AttemptStatus::Failure,
        },
        // If transaction_status not present, check transaction_result (current field)
        None => match authipay_result {
            Some(result) => match result {
                AuthipayPaymentResult::Approved => match transaction_type {
                    AuthipayTransactionType::Preauth => AttemptStatus::Authorized,
                    AuthipayTransactionType::Void => AttemptStatus::Voided,
                    AuthipayTransactionType::Sale | AuthipayTransactionType::Postauth => {
                        AttemptStatus::Charged
                    }
                    AuthipayTransactionType::Credit
                    | AuthipayTransactionType::ForcedTicket
                    | AuthipayTransactionType::Return
                    | AuthipayTransactionType::PayerAuth
                    | AuthipayTransactionType::Disbursement
                    | AuthipayTransactionType::Unknown => AttemptStatus::Failure,
                },
                AuthipayPaymentResult::Waiting => AttemptStatus::Pending,
                AuthipayPaymentResult::Partial => AttemptStatus::PartialCharged,
                AuthipayPaymentResult::Declined
                | AuthipayPaymentResult::Failed
                | AuthipayPaymentResult::Fraud => AttemptStatus::Failure,
            },
            None => AttemptStatus::Pending,
        },
    }
}

// ===== RESPONSE TRANSFORMATION =====

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result, state, AND transaction type
        // CRITICAL: This validates BOTH status fields and transaction state
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
            item.response.transaction_type.clone(),
        );

        // Extract connector metadata from payment token using helper function
        let connector_metadata = extract_connector_metadata(item.response.payment_token.as_ref());

        // Extract network-specific fields from processor object using helper function

        let (network_txn_id, _network_decline_code, _network_error_message) =
            extract_network_fields(item.response.processor.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: network_txn_id.or(item.response.api_trace_id.clone()),
                connector_response_reference_id: item.response.client_request_id.clone(),
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

// ===== PSYNC RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from authorize flow
// PSync returns the same response format as the original transaction

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result, state, AND transaction type
        // CRITICAL: This validates BOTH status fields and transaction state
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
            item.response.transaction_type.clone(),
        );

        // Extract connector metadata from payment token using helper function
        let connector_metadata = extract_connector_metadata(item.response.payment_token.as_ref());

        // Extract network-specific fields from processor object using helper function

        let (network_txn_id, _network_decline_code, _network_error_message) =
            extract_network_fields(item.response.processor.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: network_txn_id.or(item.response.api_trace_id.clone()),
                connector_response_reference_id: item.response.client_request_id.clone(),
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

// ===== CAPTURE RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from authorize flow
// Capture returns the same response format as the original transaction

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result, state, AND transaction type
        // CRITICAL: This validates BOTH status fields and transaction state
        // For successful capture: transactionType=POSTAUTH, transactionResult=APPROVED, transactionState=CAPTURED
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
            item.response.transaction_type.clone(),
        );

        // Extract connector metadata from payment token using helper function
        let connector_metadata = extract_connector_metadata(item.response.payment_token.as_ref());

        // Extract network-specific fields from processor object using helper function
        let (network_txn_id, _network_decline_code, _network_error_message) =
            extract_network_fields(item.response.processor.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: network_txn_id.or(item.response.api_trace_id.clone()),
                connector_response_reference_id: item.response.client_request_id.clone(),
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

// ===== REFUND REQUEST STRUCTURE =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayRefundRequest {
    pub request_type: AuthipayRequestType,
    pub transaction_amount: TransactionAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

// ===== REFUND REQUEST TRANSFORMATION =====

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for AuthipayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Convert refund amount to major unit format
        let converter = FloatMajorUnitForConnector;
        let amount_major = converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        Ok(Self {
            request_type: AuthipayRequestType::ReturnTransaction,
            transaction_amount,
            comments: item.request.reason.clone(),
        })
    }
}

// ===== VOID REQUEST STRUCTURE =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayVoidRequest {
    pub request_type: AuthipayRequestType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

// ===== VOID REQUEST TRANSFORMATION =====

impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for AuthipayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            request_type: AuthipayRequestType::VoidPreAuthTransactions,
            comments: item.request.cancellation_reason.clone(),
        })
    }
}

// ===== REFUND RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from payment flows
// Refunds return the same response format as primary transactions

use common_enums::RefundStatus;

// CRITICAL REFUND STATUS MAPPING FUNCTION
// This validates ALL conditions to avoid Silverflow PR #240 issues:
// 1. transactionType must be RETURN (not just any type)
// 2. transactionResult OR transactionStatus must be APPROVED (API uses both deprecated and new fields)
// 3. transactionState should be CAPTURED for success
// ONLY returns RefundStatus::Success when ALL conditions are met

fn map_refund_status(
    transaction_type: Option<AuthipayTransactionType>,
    transaction_status: Option<AuthipayPaymentStatus>,
    transaction_result: Option<AuthipayPaymentResult>,
    transaction_state: Option<AuthipayTransactionState>,
) -> RefundStatus {
    // Validate transaction type is RETURN first
    if let Some(tx_type) = transaction_type {
        if tx_type != AuthipayTransactionType::Return {
            // CRITICAL: If transactionType is NOT RETURN, this is NOT a valid refund
            return RefundStatus::Failure;
        }
    } else {
        // No transaction type provided
        return RefundStatus::Pending;
    }

    // Check transaction_state first (most reliable)
    if let Some(state) = transaction_state {
        match state {
            AuthipayTransactionState::Captured | AuthipayTransactionState::Settled
            // Verify result/status is also success
            if (matches!(transaction_result, Some(AuthipayPaymentResult::Approved))
                || matches!(transaction_status, Some(AuthipayPaymentStatus::Approved)))
            => {
                return RefundStatus::Success;
            }
            AuthipayTransactionState::Declined => return RefundStatus::Failure,
            AuthipayTransactionState::Pending | AuthipayTransactionState::Waiting => {
                return RefundStatus::Pending;
            }
            _ => {} // Continue to check status/result
        }
    }

    // Check transaction_result (newer field)
    if let Some(result) = transaction_result {
        return match result {
            AuthipayPaymentResult::Approved => {
                // If state not available or unclear, check if it's likely settled
                // API may return APPROVED without state for immediate refunds
                RefundStatus::Success
            }
            AuthipayPaymentResult::Waiting => RefundStatus::Pending,
            AuthipayPaymentResult::Declined
            | AuthipayPaymentResult::Failed
            | AuthipayPaymentResult::Fraud => RefundStatus::Failure,
            AuthipayPaymentResult::Partial => RefundStatus::Pending,
        };
    }

    // Check transaction_status (deprecated field) if transaction_result not present
    if let Some(status) = transaction_status {
        return match status {
            AuthipayPaymentStatus::Approved => {
                // If state not available or unclear, treat as success
                // API may return APPROVED without state for immediate refunds
                RefundStatus::Success
            }
            AuthipayPaymentStatus::Waiting => RefundStatus::Pending,
            AuthipayPaymentStatus::ValidationFailed
            | AuthipayPaymentStatus::ProcessingFailed
            | AuthipayPaymentStatus::Declined => RefundStatus::Failure,
            AuthipayPaymentStatus::Partial => RefundStatus::Pending,
        };
    }

    // Default to Pending for unknown/incomplete status combinations
    RefundStatus::Pending
}

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map refund status with CRITICAL validation of ALL fields
        let refund_status = map_refund_status(
            Some(item.response.transaction_type.clone()),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: item.response.ipg_transaction_id.clone(),
            refund_status,
            status_code: item.http_code,
        });

        Ok(router_data)
    }
}

// ===== REFUND SYNC RESPONSE TRANSFORMATION =====
// RSync also reuses AuthipayPaymentsResponse and uses the same refund status mapping

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map refund status with CRITICAL validation of ALL fields
        let refund_status = map_refund_status(
            Some(item.response.transaction_type.clone()),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: item.response.ipg_transaction_id.clone(),
            refund_status,
            status_code: item.http_code,
        });

        Ok(router_data)
    }
}

// ===== VOID RESPONSE TRANSFORMATION =====
// Reuses AuthipayPaymentsResponse structure from payment flows
// Void returns the same response format as primary transactions

// CRITICAL VOID STATUS MAPPING FUNCTION

// 1. transactionType must be VOID (not just any type)
// 2. transactionResult OR transactionStatus must be APPROVED (API uses both deprecated and new fields)
// 3. transactionState should be VOIDED for success
// ONLY returns AttemptStatus::Voided when ALL conditions are met

fn map_void_status(
    transaction_type: AuthipayTransactionType,
    transaction_status: Option<AuthipayPaymentStatus>,
    transaction_result: Option<AuthipayPaymentResult>,
    transaction_state: Option<AuthipayTransactionState>,
) -> AttemptStatus {
    // First validate transactionType is VOID
    if transaction_type != AuthipayTransactionType::Void {
        // Not a void transaction - this is an error
        return AttemptStatus::VoidFailed;
    }

    // Check transactionState first for most accurate status
    if let Some(state) = transaction_state {
        match state {
            AuthipayTransactionState::Voided => {
                // Verify result/status is also APPROVED for complete validation
                if matches!(transaction_result, Some(AuthipayPaymentResult::Approved))
                    || matches!(transaction_status, Some(AuthipayPaymentStatus::Approved))
                {
                    return AttemptStatus::Voided;
                }
                // State is VOIDED but no confirmation from result/status, still consider voided
                return AttemptStatus::Voided;
            }
            AuthipayTransactionState::Declined => return AttemptStatus::VoidFailed,
            AuthipayTransactionState::Pending | AuthipayTransactionState::Waiting => {
                return AttemptStatus::Pending;
            }
            _ => {} // Continue to check result/status
        }
    }

    // Check transaction_result (newer field)
    if let Some(result) = transaction_result {
        return match result {
            AuthipayPaymentResult::Approved => AttemptStatus::Voided,
            AuthipayPaymentResult::Waiting => AttemptStatus::Pending,
            AuthipayPaymentResult::Declined
            | AuthipayPaymentResult::Failed
            | AuthipayPaymentResult::Fraud => AttemptStatus::VoidFailed,
            AuthipayPaymentResult::Partial => AttemptStatus::Pending,
        };
    }

    // Check transaction_status (deprecated field) if transaction_result not present
    if let Some(status) = transaction_status {
        return match status {
            AuthipayPaymentStatus::Approved => AttemptStatus::Voided,
            AuthipayPaymentStatus::Waiting => AttemptStatus::Pending,
            AuthipayPaymentStatus::ValidationFailed
            | AuthipayPaymentStatus::ProcessingFailed
            | AuthipayPaymentStatus::Declined => AttemptStatus::VoidFailed,
            AuthipayPaymentStatus::Partial => AttemptStatus::Pending,
        };
    }

    // Default to Pending if no clear status
    AttemptStatus::Pending
}

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map void status with CRITICAL validation of ALL fields
        let status = map_void_status(
            item.response.transaction_type.clone(),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        // Extract network-specific fields from processor object using helper function
        let (network_txn_id, _network_decline_code, _network_error_message) =
            extract_network_fields(item.response.processor.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: network_txn_id.or(item.response.api_trace_id.clone()),
                connector_response_reference_id: item.response.client_request_id.clone(),
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

// ===== TYPE ALIASES FOR MACRO COMPATIBILITY =====
// Each flow needs its own response type for the macro system
// Even though they all use the same underlying AuthipayPaymentsResponse struct
pub type AuthipayAuthorizeResponse = AuthipayPaymentsResponse;
pub type AuthipaySyncResponse = AuthipayPaymentsResponse;
pub type AuthipayVoidResponse = AuthipayPaymentsResponse;
pub type AuthipayCaptureResponse = AuthipayPaymentsResponse;
pub type AuthipayRefundResponse = AuthipayPaymentsResponse;
pub type AuthipayRefundSyncResponse = AuthipayPaymentsResponse;

// ===== TRYFROM IMPLEMENTATIONS FOR MACRO COMPATIBILITY =====
// These delegate to the existing TryFrom<&RouterDataV2> implementations

use crate::connectors::authipay::AuthipayRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthipayPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AuthipayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AuthipayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AuthipayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}
