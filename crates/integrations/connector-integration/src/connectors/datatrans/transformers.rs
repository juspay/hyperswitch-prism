use crate::types::ResponseRouterData;
use base64::{engine::general_purpose::STANDARD, Engine};
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::MinorUnit;
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::{
    connector_flow::{Authorize, Capture, ClientAuthenticationToken, PSync, RSync, Refund, Void},
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse,
        DatatransClientAuthenticationResponse as DatatransClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// Error message constants
const DEFAULT_ERROR_CODE: &str = "UNKNOWN_ERROR";
const DEFAULT_ERROR_MESSAGE: &str = "Unknown error occurred";
const UNSUPPORTED_PAYMENT_METHOD_ERROR: &str = "Only card payments are supported for Datatrans";

#[derive(Debug, Clone)]
pub struct DatatransAuthType {
    pub merchant_id: Secret<String>,
    pub password: Secret<String>,
}

impl DatatransAuthType {
    pub fn generate_basic_auth(&self) -> String {
        let credentials = format!("{}:{}", self.merchant_id.peek(), self.password.peek());
        let encoded = STANDARD.encode(credentials);
        format!("Basic {encoded}")
    }
}

impl TryFrom<&ConnectorSpecificConfig> for DatatransAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Datatrans {
                merchant_id,
                password,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                password: password.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// Error response structure - Datatrans API uses nested format
// Format: {"error": {"code": "...", "message": "..."}}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatatransErrorResponse {
    pub error: DatatransErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatatransErrorDetail {
    pub code: String,
    pub message: String,
}

impl DatatransErrorResponse {
    pub fn code(&self) -> String {
        self.error.code.clone()
    }

    pub fn message(&self) -> String {
        self.error.message.clone()
    }
}

impl Default for DatatransErrorResponse {
    fn default() -> Self {
        Self {
            error: DatatransErrorDetail {
                code: DEFAULT_ERROR_CODE.to_string(),
                message: DEFAULT_ERROR_MESSAGE.to_string(),
            },
        }
    }
}

// Card details for Datatrans API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_month: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_year: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<RawCardNumber<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub card_type: Option<String>,
}

// Authorize request structure based on tech spec
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub currency: Currency,
    pub refno: String,
    pub amount: MinorUnit,
    pub card: DatatransCard<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_settle: Option<bool>,
    // Don't skip serializing - we want "option": null to appear in JSON
    pub option: Option<DatatransPaymentOptions>,
}

// Payment options for Datatrans API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransPaymentOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_alias: Option<bool>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DatatransPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
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
        // Extract card data or token
        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Direct card flow - use raw card details
                DatatransCard {
                    alias: None,
                    number: Some(card_data.card_number.clone()),
                    expiry_month: Some(card_data.card_exp_month.clone()),
                    expiry_year: Some(card_data.get_card_expiry_year_2_digit()?),
                    cvv: Some(card_data.card_cvc.clone()),
                    card_type: Some("PLAIN".to_string()),
                }
            }
            // TODO: CardToken flow for Datatrans Secure Fields SDK.
            // When the client SDK collects card data via Secure Fields, the transactionId
            // from secureFieldsInit is used as an alias. The authorize-split endpoint
            // (POST /v1/transactions/{transactionId}/authorize) should be called instead
            // of the regular authorize endpoint. The PaymentMethodToken carries the
            // transactionId from the client authentication token response.
            PaymentMethodData::PaymentMethodToken(token_data) => {
                let token = token_data.token.clone();

                DatatransCard {
                    alias: Some(token),
                    number: None,
                    expiry_month: None,
                    expiry_year: None,
                    cvv: None,
                    card_type: None,
                }
            }
            _ => Err(IntegrationError::not_implemented(
                UNSUPPORTED_PAYMENT_METHOD_ERROR.to_string(),
            ))?,
        };

        // Determine auto_settle based on capture method
        let auto_settle = match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Automatic) => Some(true),
            Some(common_enums::CaptureMethod::Manual) => Some(false),
            _ => None, // Let connector decide default behavior
        };

        Ok(Self {
            currency: router_data.request.currency,
            refno: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: router_data.request.minor_amount,
            card,
            auto_settle,
            option: None, // Set to null to match Hyperswitch
        })
    }
}

// Response card structure from tech spec
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCardResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked: Option<String>,
}

// Authorize response structure based on tech spec
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransPaymentsResponse {
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<DatatransCardResponse>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<DatatransPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Datatrans authorize endpoint returns 200 on success
        // The presence of transactionId indicates success
        // Status mapping:
        // - If we get a 200 response with transactionId, the payment is authorized
        // - Based on autoSettle parameter, it's either Charged or Authorized
        let is_auto_settle =
            item.router_data.request.capture_method == Some(common_enums::CaptureMethod::Automatic);

        let status = if is_auto_settle {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Authorized
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.acquirer_authorization_code.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PSYNC FLOW STRUCTURES =====

// PSync Request - Empty for GET-based endpoint
#[derive(Debug, Serialize)]
pub struct DatatransSyncRequest;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for DatatransSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::DatatransRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request body for GET-based sync endpoint
        Ok(Self)
    }
}

// Payment Status Enumeration from Datatrans API
#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DatatransPaymentStatus {
    Initialized,
    Authenticated,
    Authorized,
    Settled,
    Transmitted,
    Canceled,
    Failed,
}

// Status mapping for sync responses
impl From<DatatransPaymentStatus> for AttemptStatus {
    fn from(status: DatatransPaymentStatus) -> Self {
        match status {
            // Success statuses - payment is completed/settled
            DatatransPaymentStatus::Settled | DatatransPaymentStatus::Transmitted => Self::Charged,

            // Authorization status - payment is authorized but not captured
            DatatransPaymentStatus::Authorized => Self::Authorized,

            // Failure status
            DatatransPaymentStatus::Failed | DatatransPaymentStatus::Canceled => Self::Failure,

            // Pending statuses - payment is in progress
            DatatransPaymentStatus::Initialized | DatatransPaymentStatus::Authenticated => {
                Self::Pending
            }
        }
    }
}

// History entry structure from tech spec
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransHistoryEntry {
    pub action: String,
    pub amount: Option<MinorUnit>,
    pub success: bool,
    pub date: String,
}

// PSync Response structure based on tech spec GET /v1/transactions/{transactionId}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransSyncResponse {
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub status: DatatransPaymentStatus,
    pub currency: Currency,
    pub refno: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<DatatransTransactionDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<DatatransCardResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<DatatransHistoryEntry>>,
}

// Transaction detail structure
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransTransactionDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorize: Option<DatatransActionDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle: Option<DatatransActionDetail>,
}

// Action detail structure
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransActionDetail {
    pub amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Map Datatrans status to UCS status
        let status = AttemptStatus::from(response.status.clone());

        // Extract acquirer authorization code from detail or history
        let connector_response_reference_id = response.detail.as_ref().and_then(|d| {
            d.authorize
                .as_ref()
                .and_then(|a| a.acquirer_authorization_code.clone())
                .or_else(|| {
                    d.settle
                        .as_ref()
                        .and_then(|s| s.acquirer_authorization_code.clone())
                })
        });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id,
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

// ===== CAPTURE FLOW STRUCTURES =====

// Capture Request structure based on tech spec POST /v1/transactions/{transactionId}/settle
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCaptureRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub refno: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for DatatransCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        // Get the amount to capture from minor_amount_to_capture
        let amount = router_data.request.minor_amount_to_capture;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            refno: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            refno2: None,
        })
    }
}

// Capture Response
// Note: API spec says 204 No Content, but Datatrans actually returns 200 with a JSON body
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransCaptureResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Datatrans returns 200 with JSON body for successful capture
        // Use transaction_id from response if available, otherwise fall back to request
        let transaction_id = item.response.transaction_id.clone().unwrap_or_else(|| {
            item.router_data
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .unwrap_or_default()
        });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.acquirer_authorization_code.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Charged, // Successful capture means payment is charged
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..item.router_data.clone()
        })
    }
}

// ===== REFUND FLOW STRUCTURES =====

// Refund Request structure based on tech spec POST /v1/transactions/{transactionId}/credit
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransRefundRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub refno: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for DatatransRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        // Get the refund amount from RefundsData
        let amount = router_data.request.minor_refund_amount;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            refno: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            refno2: None,
        })
    }
}

// Refund Response structure based on tech spec
// The credit endpoint returns 200 with transaction details on success
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransRefundResponse {
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Datatrans credit endpoint returns 200 on success with transaction details
        // The refund is successful when we get a 200 response with transactionId
        let refunds_response_data = RefundsResponseData {
            connector_refund_id: item.response.transaction_id.clone(),
            refund_status: RefundStatus::Success, // 200 response indicates successful refund
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC (RSync) FLOW STRUCTURES =====

// RSync Request - Empty for GET-based endpoint
#[derive(Debug, Serialize)]
pub struct DatatransRefundSyncRequest;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for DatatransRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::DatatransRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request body for GET-based sync endpoint
        Ok(Self)
    }
}

// Refund Status Enumeration from Datatrans API
#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DatatransRefundStatus {
    Initialized,
    Settled,
    Transmitted,
    Failed,
}

// Status mapping for refund sync responses
impl From<DatatransRefundStatus> for RefundStatus {
    fn from(status: DatatransRefundStatus) -> Self {
        match status {
            // Success statuses - refund is completed
            DatatransRefundStatus::Settled | DatatransRefundStatus::Transmitted => Self::Success,

            // Failure status
            DatatransRefundStatus::Failed => Self::Failure,

            // Pending status - refund is in progress
            DatatransRefundStatus::Initialized => Self::Pending,
        }
    }
}

// RSync Response structure - uses same structure as payment sync but for refund transaction
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransRefundSyncResponse {
    pub transaction_id: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub status: DatatransRefundStatus,
    pub currency: Currency,
    pub refno: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refno2: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Map Datatrans refund status to UCS RefundStatus
        let refund_status = RefundStatus::from(response.status.clone());

        let refunds_response_data = RefundsResponseData {
            connector_refund_id: response.transaction_id.clone(),
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====

// Void Request structure based on tech spec POST /v1/transactions/{transactionId}/cancel
// The tech spec shows "object (CancelRequest)" as request body which appears to be empty/optional
// Using an empty struct to serialize as {} instead of null
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransVoidRequest {
    // Empty struct - will serialize as {} instead of null
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for DatatransVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::DatatransRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Empty request body for cancel endpoint based on tech spec
        // The CancelRequest object appears to be empty - serializes as {}
        Ok(Self {})
    }
}

// Void Response
// Note: API spec says 204 No Content, but Datatrans actually returns 200 with a JSON body
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransVoidResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquirer_authorization_code: Option<String>,
}

impl TryFrom<ResponseRouterData<DatatransVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DatatransVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Datatrans returns 200 with JSON body for successful void
        // Use transaction_id from response if available, otherwise fall back to request
        let transaction_id = item
            .response
            .transaction_id
            .clone()
            .unwrap_or_else(|| item.router_data.request.connector_transaction_id.clone());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.acquirer_authorization_code.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Voided, // Successful void/cancel means payment is voided
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..item.router_data.clone()
        })
    }
}

// ===== CLIENT AUTHENTICATION TOKEN FLOW STRUCTURES =====

/// Request to initialize a Datatrans Secure Fields transaction.
/// Returns a transactionId that serves as a client authentication token.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransClientAuthRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub return_url: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::DatatransRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DatatransClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: super::DatatransRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        Ok(Self {
            amount: router_data.request.amount,
            currency: router_data.request.currency,
            return_url: router_data
                .resource_common_data
                .return_url
                .clone()
                .unwrap_or_else(|| "https://example.com/return".to_string()),
        })
    }
}

/// Datatrans Secure Fields init response — contains the transactionId
/// used as a client authentication token (valid for 30 minutes).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatatransClientAuthResponse {
    pub transaction_id: String,
}

impl TryFrom<ResponseRouterData<DatatransClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DatatransClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Datatrans(
                DatatransClientAuthenticationResponseDomain {
                    transaction_id: Secret::new(response.transaction_id),
                },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
