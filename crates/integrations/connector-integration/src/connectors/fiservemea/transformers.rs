use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose, Engine};
use common_enums::AttemptStatus;
use common_utils::{
    crypto::{self, SignMessage},
    types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct FiservemeaAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl FiservemeaAuthType {
    pub fn generate_hmac_signature(
        &self,
        api_key: &str,
        client_request_id: &str,
        timestamp: &str,
        request_body: &str,
    ) -> Result<String, error_stack::Report<errors::ConnectorError>> {
        // Raw signature: apiKey + ClientRequestId + time + requestBody
        let raw_signature = format!("{api_key}{client_request_id}{timestamp}{request_body}");

        // Generate HMAC-SHA256 with API Secret as key
        let signature = crypto::HmacSha256
            .sign_message(
                self.api_secret.clone().expose().as_bytes(),
                raw_signature.as_bytes(),
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        // Base64 encode the result
        Ok(general_purpose::STANDARD.encode(signature))
    }

    pub fn generate_client_request_id() -> String {
        Uuid::new_v4().to_string()
    }

    pub fn generate_timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string()
    }
}

impl TryFrom<&ConnectorSpecificConfig> for FiservemeaAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Fiservemea {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::ConnectorError::FailedToObtainAuthType
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservemeaErrorResponse {
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

impl Default for FiservemeaErrorResponse {
    fn default() -> Self {
        Self {
            code: Some("UNKNOWN_ERROR".to_string()),
            message: Some("Unknown error occurred".to_string()),
            details: None,
            api_trace_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FiservemeaRequestType {
    PaymentCardSaleTransaction,
    PaymentCardPreAuthTransaction,
    PostAuthTransaction,
    VoidPreAuthTransactions,
    ReturnTransaction,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservemeaPaymentsRequest<T: PaymentMethodDataTypes> {
    pub request_type: FiservemeaRequestType,
    pub merchant_transaction_id: String,
    pub transaction_amount: TransactionAmount,
    pub order: OrderDetails,
    pub payment_method: PaymentMethod<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentCardSaleTransaction<T: PaymentMethodDataTypes> {
    pub request_type: FiservemeaRequestType,
    pub transaction_amount: TransactionAmount,
    pub payment_method: PaymentMethod<T>,
    pub transaction_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentCardPreAuthTransaction<T: PaymentMethodDataTypes> {
    pub request_type: FiservemeaRequestType,
    pub transaction_amount: TransactionAmount,
    pub payment_method: PaymentMethod<T>,
    pub transaction_type: String,
}

#[derive(Debug, Serialize)]
pub struct TransactionAmount {
    pub total: StringMajorUnit,
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

// Capture Request Structure - PostAuthTransaction for Secondary Transaction endpoint
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostAuthTransaction {
    pub request_type: FiservemeaRequestType,
    pub transaction_amount: TransactionAmount,
}

// Refund Request Structure - ReturnTransaction for Secondary Transaction endpoint
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnTransaction {
    pub request_type: FiservemeaRequestType,
    pub transaction_amount: TransactionAmount,
}

// Void Request Structure - VoidTransaction for Secondary Transaction endpoint
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoidTransaction {
    pub request_type: FiservemeaRequestType,
}

// Type aliases for flow-specific responses (to avoid macro templating conflicts)
pub type FiservemeaAuthorizeResponse = FiservemeaPaymentsResponse;
pub type FiservemeaSyncResponse = FiservemeaPaymentsResponse;
pub type FiservemeaCaptureResponse = FiservemeaPaymentsResponse;
pub type FiservemeaVoidResponse = FiservemeaPaymentsResponse;
pub type FiservemeaRefundResponse = FiservemeaPaymentsResponse;
pub type FiservemeaRefundSyncResponse = FiservemeaPaymentsResponse;

// The macro creates a FiservemeaRouterData type. We need to provide the use statement.
use super::FiservemeaRouterData;

// Implementations for FiservemeaRouterData - needed for the macro framework
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservemeaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservemeaPaymentsRequest<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: FiservemeaRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Delegate to the existing TryFrom implementation
        Self::try_from(&item.router_data)
    }
}

// Note: Response conversions use the existing TryFrom implementations
// for FiservemeaPaymentsResponse since all response aliases point to it

// TryFrom for Capture
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservemeaRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PostAuthTransaction
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: FiservemeaRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// TryFrom for Void
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservemeaRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for VoidTransaction
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: FiservemeaRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// TryFrom for Refund
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservemeaRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for ReturnTransaction
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: FiservemeaRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for FiservemeaPaymentsRequest<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // Use StringMajorUnitForConnector to properly convert minor to major unit
        let converter = StringMajorUnitForConnector;
        let amount_major = converter
            .convert(item.request.minor_amount, item.request.currency)
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        // Extract payment method data
        let payment_method = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Convert year to YY format (last 2 digits)
                let year_str = card_data.card_exp_year.peek();
                let year_yy = if year_str.len() == 4 {
                    // YYYY format - take last 2 digits
                    Secret::new(year_str[2..].to_string())
                } else {
                    // Already YY format
                    card_data.card_exp_year.clone()
                };

                let payment_card = PaymentCard {
                    number: card_data.card_number.clone(),
                    expiry_date: ExpiryDate {
                        month: Secret::new(card_data.card_exp_month.peek().clone()),
                        year: Secret::new(year_yy.peek().clone()),
                    },
                    security_code: Some(card_data.card_cvc.clone()),
                    holder: item.request.customer_name.clone().map(Secret::new),
                };
                PaymentMethod { payment_card }
            }
            _ => {
                return Err(error_stack::report!(
                    errors::ConnectorError::NotImplemented(
                        "Only card payments are supported".to_string()
                    )
                ))
            }
        };

        // Determine transaction type based on capture_method
        let is_manual_capture = item
            .request
            .capture_method
            .map(|cm| matches!(cm, common_enums::CaptureMethod::Manual))
            .unwrap_or(false);

        // Generate unique merchant transaction ID using connector request reference ID
        // This provides a meaningful, unique identifier for each transaction
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
                request_type: FiservemeaRequestType::PaymentCardPreAuthTransaction,
                merchant_transaction_id,
                transaction_amount,
                order,
                payment_method,
            })
        } else {
            Ok(Self {
                request_type: FiservemeaRequestType::PaymentCardSaleTransaction,
                merchant_transaction_id,
                transaction_amount,
                order,
                payment_method,
            })
        }
    }
}

impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for PostAuthTransaction
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Use StringMajorUnitForConnector to properly convert minor to major unit
        let converter = StringMajorUnitForConnector;
        let amount_major = converter
            .convert(item.request.minor_amount_to_capture, item.request.currency)
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        Ok(Self {
            request_type: FiservemeaRequestType::PostAuthTransaction,
            transaction_amount,
        })
    }
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for ReturnTransaction
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Use StringMajorUnitForConnector to properly convert minor to major unit
        let converter = StringMajorUnitForConnector;
        let amount_major = converter
            .convert(item.request.minor_refund_amount, item.request.currency)
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency: item.request.currency,
        };

        Ok(Self {
            request_type: FiservemeaRequestType::ReturnTransaction,
            transaction_amount,
        })
    }
}

impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for VoidTransaction
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        _item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // For void transactions, we only need to specify the transaction type
        // The transaction ID is passed in the URL path parameter
        Ok(Self {
            request_type: FiservemeaRequestType::VoidPreAuthTransactions,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservemeaTransactionType {
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
pub enum FiservemeaPaymentStatus {
    Approved,
    Waiting,
    Partial,
    ValidationFailed,
    ProcessingFailed,
    Declined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservemeaPaymentResult {
    Approved,
    Declined,
    Failed,
    Waiting,
    Partial,
    Fraud,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum FiservemeaTransactionOrigin {
    Ecom,
    Moto,
    Mail,
    Phone,
    Retail,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservemeaPaymentCardResponse {
    pub expiry_date: Option<ExpiryDate>,
    pub bin: Option<String>,
    pub last4: Option<String>,
    pub brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservemeaPaymentMethodDetails {
    pub payment_card: Option<FiservemeaPaymentCardResponse>,
    pub payment_method_type: Option<String>,
    pub payment_method_brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Components {
    pub subtotal: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmountDetails {
    pub total: Option<f64>,
    pub currency: Option<common_enums::Currency>,
    pub components: Option<Components>,
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
    pub avs_response: Option<AvsResponse>,
    pub security_code_response: Option<String>,
}

fn map_status(
    fiservemea_status: Option<FiservemeaPaymentStatus>,
    fiservemea_result: Option<FiservemeaPaymentResult>,
    transaction_type: FiservemeaTransactionType,
) -> AttemptStatus {
    match fiservemea_status {
        Some(status) => match status {
            FiservemeaPaymentStatus::Approved => match transaction_type {
                FiservemeaTransactionType::Preauth => AttemptStatus::Authorized,
                FiservemeaTransactionType::Void => AttemptStatus::Voided,
                FiservemeaTransactionType::Sale | FiservemeaTransactionType::Postauth => {
                    AttemptStatus::Charged
                }
                FiservemeaTransactionType::Credit
                | FiservemeaTransactionType::ForcedTicket
                | FiservemeaTransactionType::Return
                | FiservemeaTransactionType::PayerAuth
                | FiservemeaTransactionType::Disbursement
                | FiservemeaTransactionType::Unknown => AttemptStatus::Failure,
            },
            FiservemeaPaymentStatus::Waiting => AttemptStatus::Pending,
            FiservemeaPaymentStatus::Partial => AttemptStatus::PartialCharged,
            FiservemeaPaymentStatus::ValidationFailed
            | FiservemeaPaymentStatus::ProcessingFailed
            | FiservemeaPaymentStatus::Declined => AttemptStatus::Failure,
        },
        None => match fiservemea_result {
            Some(result) => match result {
                FiservemeaPaymentResult::Approved => match transaction_type {
                    FiservemeaTransactionType::Preauth => AttemptStatus::Authorized,
                    FiservemeaTransactionType::Void => AttemptStatus::Voided,
                    FiservemeaTransactionType::Sale | FiservemeaTransactionType::Postauth => {
                        AttemptStatus::Charged
                    }
                    FiservemeaTransactionType::Credit
                    | FiservemeaTransactionType::ForcedTicket
                    | FiservemeaTransactionType::Return
                    | FiservemeaTransactionType::PayerAuth
                    | FiservemeaTransactionType::Disbursement
                    | FiservemeaTransactionType::Unknown => AttemptStatus::Failure,
                },
                FiservemeaPaymentResult::Waiting => AttemptStatus::Pending,
                FiservemeaPaymentResult::Partial => AttemptStatus::PartialCharged,
                FiservemeaPaymentResult::Declined
                | FiservemeaPaymentResult::Failed
                | FiservemeaPaymentResult::Fraud => AttemptStatus::Failure,
            },
            None => AttemptStatus::Pending,
        },
    }
}

fn map_refund_status(
    fiservemea_status: Option<FiservemeaPaymentStatus>,
    fiservemea_result: Option<FiservemeaPaymentResult>,
) -> common_enums::RefundStatus {
    match fiservemea_status {
        Some(status) => match status {
            FiservemeaPaymentStatus::Approved => common_enums::RefundStatus::Success,
            FiservemeaPaymentStatus::Partial | FiservemeaPaymentStatus::Waiting => {
                common_enums::RefundStatus::Pending
            }
            FiservemeaPaymentStatus::ValidationFailed
            | FiservemeaPaymentStatus::ProcessingFailed
            | FiservemeaPaymentStatus::Declined => common_enums::RefundStatus::Failure,
        },
        None => match fiservemea_result {
            Some(result) => match result {
                FiservemeaPaymentResult::Approved => common_enums::RefundStatus::Success,
                FiservemeaPaymentResult::Partial | FiservemeaPaymentResult::Waiting => {
                    common_enums::RefundStatus::Pending
                }
                FiservemeaPaymentResult::Declined
                | FiservemeaPaymentResult::Failed
                | FiservemeaPaymentResult::Fraud => common_enums::RefundStatus::Failure,
            },
            None => common_enums::RefundStatus::Pending,
        },
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservemeaPaymentsResponse {
    pub client_request_id: Option<String>,
    pub api_trace_id: Option<String>,
    pub response_type: Option<String>,
    #[serde(rename = "type")]
    pub response_type_field: Option<String>,
    pub ipg_transaction_id: String,
    pub order_id: Option<String>,
    pub user_id: Option<String>,
    pub transaction_type: FiservemeaTransactionType,
    pub transaction_origin: Option<FiservemeaTransactionOrigin>,
    pub payment_method_details: Option<FiservemeaPaymentMethodDetails>,
    pub country: Option<Secret<String>>,
    pub terminal_id: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_transaction_id: Option<String>,
    pub transaction_time: Option<i64>,
    pub approved_amount: Option<AmountDetails>,
    pub transaction_amount: Option<AmountDetails>,
    pub transaction_status: Option<FiservemeaPaymentStatus>,
    pub transaction_result: Option<FiservemeaPaymentResult>,
    pub approval_code: Option<String>,
    pub error_message: Option<String>,
    pub transaction_state: Option<String>,
    pub scheme_transaction_id: Option<String>,
    pub processor: Option<Processor>,
    pub payment_token: Option<PaymentToken>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentToken {
    pub value: Option<String>,
    pub reusable: Option<bool>,
    pub decline_duplicates: Option<bool>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiservemeaPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservemeaPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result and transaction type
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_type.clone(),
        );

        // Prepare connector metadata if available
        let connector_metadata = item.response.payment_token.as_ref().map(|token| {
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
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: item.response.api_trace_id.clone(),
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

impl TryFrom<ResponseRouterData<FiservemeaPaymentsResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservemeaPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result and transaction type
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_type.clone(),
        );

        // Prepare connector metadata if available
        let connector_metadata = item.response.payment_token.as_ref().map(|token| {
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
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: item.response.api_trace_id.clone(),
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

impl TryFrom<ResponseRouterData<FiservemeaPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservemeaPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result and transaction type
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_type.clone(),
        );

        // Prepare connector metadata if available
        let connector_metadata = item.response.payment_token.as_ref().map(|token| {
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
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: item.response.api_trace_id.clone(),
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

impl TryFrom<ResponseRouterData<FiservemeaPaymentsResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservemeaPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result fields
        let refund_status = map_refund_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
        );

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.ipg_transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<FiservemeaPaymentsResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservemeaPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result fields
        let refund_status = map_refund_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
        );

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.ipg_transaction_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<FiservemeaPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservemeaPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map transaction status using status/result and transaction type
        // The map_status function properly handles void status based on transaction_type
        let status = map_status(
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_type.clone(),
        );

        // Prepare connector metadata if available
        let connector_metadata = item.response.payment_token.as_ref().map(|token| {
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
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: item.response.api_trace_id.clone(),
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
