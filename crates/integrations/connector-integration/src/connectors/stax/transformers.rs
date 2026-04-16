use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts,
    pii::Email,
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, PaymentMethodToken, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
    },
    payment_method_data::{
        BankDebitData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::StaxRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

// Empty request structures for GET requests that don't send request bodies
#[derive(Debug, Serialize, Default)]
pub struct StaxPSyncRequest {}

#[derive(Debug, Serialize, Default)]
pub struct StaxVoidRequest {}

#[derive(Debug, Serialize, Default)]
pub struct StaxRSyncRequest {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for StaxPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: StaxRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for StaxVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: StaxRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>, T>,
    > for StaxRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: StaxRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaxTransactionType {
    Charge,
    PreAuth,
    Refund,
    Void,
}

impl StaxTransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Charge => "charge",
            Self::PreAuth => "pre_auth",
            Self::Refund => "refund",
            Self::Void => "void",
        }
    }
}

// ===== AUTH TYPE =====
#[derive(Debug, Clone)]
pub struct StaxAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for StaxAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Stax { api_key, .. } => Ok(Self {
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

// ===== ERROR RESPONSE =====
/// Stax returns the full transaction structure for failed payments (HTTP 400)
/// NOT a simple error object. The response is identical to StaxPaymentResponse
/// but with success: false and a message field.
///
/// All fields are optional with defaults since error responses may vary.
///
/// IMPORTANT: Stax validation errors can have different formats:
/// 1. Transaction error: {"success": false, "id": "txn_123", "message": "error"}
/// 2. Validation error: {"error": ["The selected id is invalid."]}
/// 3. Field validation: {"card_number": ["Invalid Card Number"]}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaxErrorResponse {
    #[serde(default)]
    pub success: bool,
    // id can be a String (transaction ID) OR Array (validation errors)
    pub id: Option<serde_json::Value>,
    pub message: Option<String>,
    #[serde(rename = "type")]
    pub transaction_type: Option<String>,
    pub is_captured: Option<i8>,
    pub is_voided: Option<bool>,
    pub validation: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub code: Option<String>,
    /// Capture any other fields for field-level validation errors
    #[serde(flatten)]
    pub other: std::collections::HashMap<String, serde_json::Value>,
}

impl StaxErrorResponse {
    /// Extract error message from various Stax error response formats
    pub fn get_error_message(&self) -> String {
        // Helper to extract first array element as string
        let extract_array_msg = |value: &serde_json::Value| -> Option<String> {
            value
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(String::from)
        };

        // Try different error formats in priority order
        self.message
            .clone()
            .or_else(|| self.id.as_ref().and_then(extract_array_msg))
            .or_else(|| {
                self.error
                    .as_ref()
                    .and_then(|v| v.as_str().map(String::from))
            })
            .or_else(|| self.error.as_ref().and_then(extract_array_msg))
            .or_else(|| {
                self.validation
                    .as_ref()
                    .and_then(|v| v.as_str().map(String::from))
            })
            .or_else(|| self.validation.as_ref().and_then(extract_array_msg))
            .or_else(|| {
                // Check field-level validation errors (e.g., {"card_number": ["Invalid Card Number"]})
                self.other.values().find_map(extract_array_msg)
            })
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string())
    }

    /// Extract connector transaction ID if available
    pub fn get_connector_transaction_id(&self) -> Option<String> {
        self.id.as_ref().and_then(|v| v.as_str()).map(String::from)
    }
}

// ===== AUTHORIZE REQUEST =====
/// Request structure for Stax payment authorization
///
/// # Amount Handling
/// Note: Stax API requires amounts in dollars (major units) rather than cents.
/// We convert from MinorUnit (cents) to FloatMajorUnit (dollars) at the API boundary.
/// Example: MinorUnit(1000) -> FloatMajorUnit(10.00) dollars
#[derive(Debug, Serialize)]
pub struct StaxAuthorizeRequest {
    /// Amount in dollars (major units). Converted from MinorUnit at boundary.
    pub total: FloatMajorUnit,
    pub payment_method_id: String,
    pub is_refundable: bool,
    pub pre_auth: bool,
    /// Metadata object - required by Stax API
    pub meta: StaxMeta,
    pub idempotency_id: Option<String>,
}

/// Additional metadata for Stax transactions
///
/// # Tax Field
/// Stax requires the meta object with at least a tax field.
/// For simple transactions without tax, set tax to 0.
#[derive(Debug, Serialize)]
pub struct StaxMeta {
    /// Tax amount in minor units (cents)
    pub tax: MinorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for StaxAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: StaxRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let converter = FloatMajorUnitForConnector;
        let total = converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let payment_method_id = match &item.router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => t.token.peek().to_string(),
            PaymentMethodData::Card(_) | PaymentMethodData::BankDebit(_) => {
                if let Some(mandate_id) = item.router_data.request.connector_mandate_id() {
                    mandate_id
                } else {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "payment_method_token (from PaymentMethodToken flow) or connector_mandate_id (for saved payment methods)",
                        context: Default::default()
                    })?;
                }
            }
            _ => {
                return Err(IntegrationError::not_implemented(
                    "Only card and ACH bank debit payments are supported for Stax".to_string(),
                ))?;
            }
        };

        let is_auto_capture = item.router_data.request.is_auto_capture();

        Ok(Self {
            total,
            payment_method_id,
            is_refundable: true,
            pre_auth: !is_auto_capture,
            meta: StaxMeta {
                tax: MinorUnit::zero(),
            },
            idempotency_id: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}

// ===== AUTHORIZE RESPONSE =====
/// Payment response structure from Stax API
///
/// # Field Strategy
/// Following Hyperswitch's minimal approach - only deserialize the fields we actually need.
/// Serde will automatically ignore any extra fields in the JSON response.
/// This makes deserialization robust against Stax adding new fields.
///
/// # Amount Fields
/// Note: Stax API returns amounts in dollars (f64 major units).
/// The `total` field is in dollars and needs conversion back to MinorUnit
/// when mapping to RouterDataV2.
#[derive(Debug, Deserialize, Serialize)]
pub struct StaxPaymentResponse {
    pub success: bool,
    pub id: String,
    pub is_captured: i8,
    pub is_voided: bool,
    pub child_captures: Vec<ChildCapture>,
    #[serde(rename = "type")]
    pub transaction_type: StaxTransactionType,
    pub pre_auth: bool,
    pub settled_at: Option<String>,
    pub child_transactions: Vec<ChildTransaction>,
    pub message: Option<String>,
}

// Type aliases for each flow to avoid macro conflicts
// The macro generates templating structs based on response types, so each flow needs a unique type
pub type StaxAuthorizeResponse = StaxPaymentResponse;
pub type StaxPSyncResponse = StaxPaymentResponse;
pub type StaxCaptureResponse = StaxPaymentResponse;
pub type StaxVoidResponse = StaxPaymentResponse;
pub type StaxRefundResponse = StaxPaymentResponse;
pub type StaxRSyncResponse = StaxPaymentResponse;

/// Child capture transaction (for pre-auth captures)
#[derive(Debug, Deserialize, Serialize)]
pub struct ChildCapture {
    pub id: String,
}

/// Metadata stored for capture operations
#[derive(Debug, Serialize, Deserialize)]
pub struct StaxMetaData {
    pub capture_id: String,
}

/// Child transaction (for refunds/voids)
#[derive(Debug, Deserialize, Serialize)]
pub struct ChildTransaction {
    pub id: String,
    #[serde(rename = "type")]
    pub transaction_type: StaxTransactionType,
    pub success: bool,
    pub total: Option<FloatMajorUnit>,
    pub reference_id: Option<String>,
    pub created_at: Option<String>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<StaxPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let status = get_payment_status(response, item.http_code)?;

        // Store capture_id in metadata when pre-auth is captured (following HS pattern)
        let connector_metadata = if response.transaction_type == StaxTransactionType::PreAuth
            && response.is_captured != 0
        {
            response.child_captures.first().map(|child_captures| {
                serde_json::json!(StaxMetaData {
                    capture_id: child_captures.id.clone()
                })
            })
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: None,
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
// PSync uses the same StaxPaymentResponse structure as Authorize
impl TryFrom<ResponseRouterData<StaxPaymentResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let status = get_payment_status(response, item.http_code)?;

        // Store capture_id in metadata when pre-auth is captured (following HS pattern)
        let connector_metadata = if response.transaction_type == StaxTransactionType::PreAuth
            && response.is_captured != 0
        {
            response.child_captures.first().map(|child_captures| {
                serde_json::json!(StaxMetaData {
                    capture_id: child_captures.id.clone()
                })
            })
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata,
                network_txn_id: None,
                connector_response_reference_id: None,
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

// ===== CAPTURE REQUEST =====
/// Request structure for capturing a pre-authorized transaction
///
/// # Amount Handling
/// Note: Stax API expects amounts in dollars (major units).
/// We convert from MinorUnit (cents) to FloatMajorUnit (dollars) at the API boundary.
#[derive(Debug, Serialize)]
pub struct StaxCaptureRequest {
    /// Capture amount in dollars (converted from MinorUnit).
    pub total: FloatMajorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for StaxCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: StaxRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let converter = FloatMajorUnitForConnector;
        let total = converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self { total })
    }
}

// ===== CAPTURE RESPONSE TRANSFORMATION =====
// Capture uses the same StaxPaymentResponse structure as Authorize
impl TryFrom<ResponseRouterData<StaxPaymentResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // CRITICAL: Follow reviewer feedback - check transaction type and status
        // After capture, transaction type should be "charge" with pre_auth: false
        let status = get_capture_status(response, item.http_code)?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
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

fn get_payment_status(
    response: &StaxPaymentResponse,
    http_status: u16,
) -> Result<AttemptStatus, error_stack::Report<ConnectorError>> {
    let mut status = if !response.success {
        AttemptStatus::Failure
    } else {
        match response.transaction_type {
            StaxTransactionType::PreAuth => match response.is_captured {
                0 => AttemptStatus::Authorized,
                _ => AttemptStatus::Charged,
            },
            StaxTransactionType::Charge => AttemptStatus::Charged,
            _ => {
                return Err(error_stack::report!(
                    crate::utils::response_handling_fail_for_connector(http_status, "stax")
                )
                .attach_printable("Unsupported transaction type"))
            }
        }
    };

    if response.is_voided {
        status = AttemptStatus::Voided;
    }

    Ok(status)
}

fn get_capture_status(
    response: &StaxPaymentResponse,
    http_status: u16,
) -> Result<AttemptStatus, error_stack::Report<ConnectorError>> {
    get_payment_status(response, http_status)
}

// ===== REFUND REQUEST =====
/// Request structure for refunding a settled transaction
///
/// # Amount Handling
/// Note: Stax API expects amounts in dollars (major units).
/// We convert from MinorUnit (cents) to FloatMajorUnit (dollars) at the API boundary.
#[derive(Debug, Serialize)]
pub struct StaxRefundRequest {
    /// Refund amount in dollars (converted from MinorUnit).
    pub total: FloatMajorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for StaxRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: StaxRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let converter = FloatMajorUnitForConnector;
        let total = converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self { total })
    }
}

// ===== REFUND RESPONSE =====
// CRITICAL: Stax returns the PARENT transaction with refund as a child in child_transactions array
// We reuse StaxPaymentResponse since the structure is identical
// The key is to extract and validate the refund child transaction

impl TryFrom<ResponseRouterData<StaxPaymentResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Convert refund amount to FloatMajorUnit for filtering (like HS does)
        let converter = FloatMajorUnitForConnector;
        let refund_amount = converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "stax",
            ))?;

        // MUST find and validate child transaction with type="refund"
        // Following HS pattern: filter by amount and find most recent by created_at
        let refund_status = get_refund_status(response, refund_amount, item.http_code)?;

        // Extract refund ID from the child transaction
        let connector_refund_id = extract_refund_id(response, refund_amount, item.http_code)?;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== RSYNC RESPONSE =====
// CRITICAL: RSync queries the refund transaction directly using /transaction/{refund_id}
// Stax returns the refund transaction at the TOP LEVEL (not as a child transaction)
// This is different from Refund Execute which returns parent with child_transactions array
impl TryFrom<ResponseRouterData<StaxPaymentResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Use top-level fields since Stax returns the refund transaction directly
        // (not in child_transactions array like Refund Execute)
        let refund_status = if response.success {
            RefundStatus::Success
        } else {
            RefundStatus::Failure
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(), // Top-level ID is the refund ID
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

fn get_refund_status(
    response: &StaxPaymentResponse,
    refund_amount: FloatMajorUnit,
    http_status: u16,
) -> Result<RefundStatus, error_stack::Report<ConnectorError>> {
    // Following HS pattern: filter by amount, then find most recent by created_at
    let filtered_refunds: Vec<&ChildTransaction> = response
        .child_transactions
        .iter()
        .filter(|child| {
            child.transaction_type == StaxTransactionType::Refund
                && (child.total == Some(refund_amount))
        })
        .collect();

    let mut refund_child = filtered_refunds.first().ok_or_else(|| {
        error_stack::report!(crate::utils::response_handling_fail_for_connector(
            http_status,
            "stax"
        ))
        .attach_printable("No refund child transaction found with matching amount")
    })?;

    // Find most recent refund by comparing created_at timestamps
    for child in filtered_refunds.iter() {
        if let (Some(child_created_at), Some(current_created_at)) =
            (&child.created_at, &refund_child.created_at)
        {
            if child_created_at > current_created_at {
                refund_child = child;
            }
        }
    }

    if refund_child.success {
        Ok(RefundStatus::Success)
    } else {
        Ok(RefundStatus::Failure)
    }
}

fn extract_refund_id(
    response: &StaxPaymentResponse,
    refund_amount: FloatMajorUnit,
    http_status: u16,
) -> Result<String, error_stack::Report<ConnectorError>> {
    // Following HS pattern: filter by amount, then find most recent by created_at
    let filtered_refunds: Vec<&ChildTransaction> = response
        .child_transactions
        .iter()
        .filter(|child| {
            child.transaction_type == StaxTransactionType::Refund
                && (child.total == Some(refund_amount))
        })
        .collect();

    let mut refund_child = filtered_refunds.first().ok_or_else(|| {
        error_stack::report!(crate::utils::response_handling_fail_for_connector(
            http_status,
            "stax"
        ))
        .attach_printable("No refund child transaction found with matching amount")
    })?;

    // Find most recent refund by comparing created_at timestamps
    for child in filtered_refunds.iter() {
        if let (Some(child_created_at), Some(current_created_at)) =
            (&child.created_at, &refund_child.created_at)
        {
            if child_created_at > current_created_at {
                refund_child = child;
            }
        }
    }

    Ok(refund_child.id.clone())
}

impl TryFrom<ResponseRouterData<StaxPaymentResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        let status = if response.is_voided {
            AttemptStatus::Voided
        } else {
            AttemptStatus::VoidFailed
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
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

// ===== CREATE CONNECTOR CUSTOMER =====
/// Request to create a Stax customer account
#[derive(Debug, Serialize)]
pub struct StaxCustomerRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstname: Option<Secret<String>>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateConnectorCustomer,
                PaymentFlowData,
                domain_types::connector_types::ConnectorCustomerData,
                domain_types::connector_types::ConnectorCustomerResponse,
            >,
            T,
        >,
    > for StaxCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: StaxRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateConnectorCustomer,
                PaymentFlowData,
                domain_types::connector_types::ConnectorCustomerData,
                domain_types::connector_types::ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if item.router_data.request.email.is_none() {
            Err(IntegrationError::MissingRequiredField {
                field_name: "email",
                context: Default::default(),
            })?
        } else if item.router_data.request.name.is_none() {
            Err(IntegrationError::MissingRequiredField {
                field_name: "name",
                context: Default::default(),
            })?
        } else {
            Ok(Self {
                email: item
                    .router_data
                    .request
                    .email
                    .as_ref()
                    .map(|e| e.peek().clone()),
                firstname: item.router_data.request.name.clone(),
            })
        }
    }
}

/// Response from Stax customer creation
#[derive(Debug, Deserialize, Serialize)]
pub struct StaxCustomerResponse {
    pub id: Secret<String>, // Stax customer ID
}

impl TryFrom<ResponseRouterData<StaxCustomerResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        domain_types::connector_types::ConnectorCustomerData,
        domain_types::connector_types::ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxCustomerResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(domain_types::connector_types::ConnectorCustomerResponse {
                connector_customer_id: item.response.id.expose(),
            }),
            ..item.router_data
        })
    }
}

// ===== PAYMENT METHOD TOKENIZATION =====
/// Card tokenization request data
///
/// # Security
/// All sensitive fields are masked with Secret<> or RawCardNumber (auto-masked)
#[derive(Debug, Serialize)]
pub struct StaxCardTokenizeData<T: PaymentMethodDataTypes> {
    pub person_name: Secret<String>,
    pub card_number: RawCardNumber<T>, // Generic card number type (auto-masked)
    pub card_exp: Secret<String>,      // MMYY format (e.g., "1225")
    pub card_cvv: Secret<String>,
    pub customer_id: Secret<String>, // From CreateConnectorCustomer
}

/// Bank tokenization request data for ACH (BankDebit)
///
/// # Security
/// All sensitive fields are masked with Secret<> as appropriate
#[derive(Debug, Serialize)]
pub struct StaxBankTokenizeData {
    pub person_name: Secret<String>,
    pub bank_account: Secret<String>,
    pub bank_routing: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<common_enums::BankNames>,
    pub bank_type: common_enums::BankType,
    pub bank_holder_type: common_enums::BankHolderType,
    pub customer_id: Secret<String>,
}

/// Tagged enum for different payment method types
///
/// Stax API uses a `method` field to distinguish between card and bank tokenization
#[derive(Debug, Serialize)]
#[serde(tag = "method")]
#[serde(rename_all = "lowercase")]
pub enum StaxTokenRequest<T: PaymentMethodDataTypes> {
    Card(StaxCardTokenizeData<T>),
    Bank(StaxBankTokenizeData),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        StaxRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for StaxTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: StaxRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Get customer_id from connector metadata or error
        let customer_id = item
            .router_data
            .resource_common_data
            .connector_customer
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_customer_id",
                context: Default::default(),
            })?;

        // Extract card data from payment_method_data
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Format expiry as MMYY (Stax requirement)
                // Use card data helper method to format as MMYY (no delimiter)
                let card_exp =
                    card_data.get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?;

                // Get cardholder name - Stax requires full name (first + last)
                // Try to construct from billing address first, then fallback to card_holder_name
                let person_name = if let Some(billing) = item
                    .router_data
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                {
                    // Try to get first and last name from billing address
                    let first_name = billing.get_optional_first_name();
                    let last_name = billing.get_optional_last_name();

                    match (first_name, last_name) {
                        (Some(first), Some(last)) => {
                            // Both available - concatenate
                            Secret::new(format!("{} {}", first.peek(), last.peek()))
                        }
                        (Some(first), None) => {
                            // Only first name - use card_holder_name if available, otherwise just first
                            card_data
                                .card_holder_name
                                .clone()
                                .unwrap_or_else(|| first.clone())
                        }
                        (None, Some(last)) => {
                            // Only last name (unusual) - use it
                            last.clone()
                        }
                        (None, None) => {
                            // No names in billing - try card_holder_name
                            card_data.card_holder_name.clone().ok_or(IntegrationError::MissingRequiredField {
                                field_name: "card_holder_name or billing.first_name+last_name (required by Stax for payment method tokenization)",
                context: Default::default()
                            })?
                        }
                    }
                } else {
                    // No billing address - use card_holder_name
                    card_data.card_holder_name.clone().ok_or(IntegrationError::MissingRequiredField {
                        field_name: "card_holder_name or billing.first_name+last_name (required by Stax for payment method tokenization)",
                context: Default::default()
                    })?
                };

                Ok(Self::Card(StaxCardTokenizeData {
                    person_name,
                    card_number: card_data.card_number.clone(),
                    card_exp,
                    card_cvv: card_data.card_cvc.clone(),
                    customer_id: Secret::new(customer_id),
                }))
            }
            PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                card_holder_name,
                bank_account_holder_name,
                bank_name,
                bank_type,
                bank_holder_type,
            }) => {
                // Get account holder name
                let person_name = item
                    .router_data
                    .resource_common_data
                    .address
                    .get_payment_method_billing()
                    .and_then(|billing| {
                        let first = billing.get_optional_first_name()?;
                        let last = billing.get_optional_last_name();
                        match last {
                            Some(last) => Some(Secret::new(format!("{} {}", first.peek(), last.peek()))),
                            None => Some(first)
}
                    })
                    .or_else(|| bank_account_holder_name.clone())
                    .or_else(|| card_holder_name.clone())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing.first_name or bank_account_holder_name (required by Stax for bank tokenization)",
                context: Default::default()
                    })?;

                // bank_name is already None if Unspecified was sent in gRPC request
                let bank_type = bank_type.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "bank_type",
                    context: Default::default(),
                })?;
                let bank_holder_type =
                    bank_holder_type.ok_or(IntegrationError::MissingRequiredField {
                        field_name: "bank_holder_type",
                        context: Default::default(),
                    })?;

                Ok(Self::Bank(StaxBankTokenizeData {
                    person_name,
                    bank_account: account_number.clone(),
                    bank_routing: routing_number.clone(),
                    bank_name: *bank_name,
                    bank_type,
                    bank_holder_type,
                    customer_id: Secret::new(customer_id),
                }))
            }
            PaymentMethodData::BankDebit(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    "Only card and ACH bank debit tokenization are supported for Stax".to_string(),
                ))?
            }
        }
    }
}

/// Response from Stax payment method tokenization
#[derive(Debug, Deserialize, Serialize)]
pub struct StaxTokenResponse {
    pub id: Secret<String>, // payment_method_id to use in Authorize
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<StaxTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<StaxTokenResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.id.expose(),
            }),
            ..item.router_data
        })
    }
}
