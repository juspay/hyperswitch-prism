use crate::{
    connectors::{hipay::HipayRouterData, macros::GetFormData},
    types::ResponseRouterData,
    utils::build_form_from_struct,
};
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{request::MultipartData, types::StringMajorUnit};
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PaymentMethodToken, RSync, Refund, RepeatPayment, Void,
    },
    connector_types::{
        MandateReferenceId, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct HipayAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for HipayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Hipay {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayErrorResponse {
    pub code: String,
    pub message: String,
}

// HiPay Payment Status Enum - Type-safe status codes from HiPay API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HipayPaymentStatus {
    #[serde(rename = "109")]
    AuthenticationFailed,
    #[serde(rename = "110")]
    Blocked,
    #[serde(rename = "111")]
    Denied,
    #[serde(rename = "112")]
    AuthorizedAndPending,
    #[serde(rename = "113")]
    Refused,
    #[serde(rename = "114")]
    Expired,
    #[serde(rename = "115")]
    Cancelled,
    #[serde(rename = "116")]
    Authorized,
    #[serde(rename = "117")]
    CaptureRequested,
    #[serde(rename = "118")]
    Captured,
    #[serde(rename = "119")]
    PartiallyCaptured,
    #[serde(rename = "129")]
    ChargedBack,
    #[serde(rename = "173")]
    CaptureRefused,
    #[serde(rename = "174")]
    AwaitingTerminal,
    #[serde(rename = "175")]
    AuthorizationCancellationRequested,
    #[serde(rename = "177")]
    ChallengeRequested,
    #[serde(rename = "178")]
    SoftDeclined,
    #[serde(rename = "200")]
    PendingPayment,
    #[serde(rename = "101")]
    Created,
    #[serde(rename = "105")]
    UnableToAuthenticate,
    #[serde(rename = "106")]
    CardholderAuthenticated,
    #[serde(rename = "107")]
    AuthenticationAttempted,
    #[serde(rename = "108")]
    CouldNotAuthenticate,
    #[serde(rename = "120")]
    Collected,
    #[serde(rename = "121")]
    PartiallyCollected,
    #[serde(rename = "122")]
    Settled,
    #[serde(rename = "123")]
    PartiallySettled,
    #[serde(rename = "140")]
    AuthenticationRequested,
    #[serde(rename = "141")]
    Authenticated,
    #[serde(rename = "151")]
    AcquirerNotFound,
    #[serde(rename = "161")]
    RiskAccepted,
    #[serde(rename = "163")]
    AuthorizationRefused,
}

impl From<HipayPaymentStatus> for AttemptStatus {
    fn from(status: HipayPaymentStatus) -> Self {
        match status {
            HipayPaymentStatus::AuthenticationFailed => Self::AuthenticationFailed,
            HipayPaymentStatus::Blocked
            | HipayPaymentStatus::Refused
            | HipayPaymentStatus::Expired
            | HipayPaymentStatus::Denied => Self::Failure,
            HipayPaymentStatus::AuthorizedAndPending => Self::Pending,
            HipayPaymentStatus::Cancelled => Self::Voided,
            HipayPaymentStatus::Authorized => Self::Authorized,
            HipayPaymentStatus::CaptureRequested => Self::CaptureInitiated,
            HipayPaymentStatus::Captured => Self::Charged,
            HipayPaymentStatus::PartiallyCaptured => Self::PartialCharged,
            HipayPaymentStatus::CaptureRefused => Self::CaptureFailed,
            HipayPaymentStatus::AwaitingTerminal => Self::Pending,
            HipayPaymentStatus::AuthorizationCancellationRequested => Self::VoidInitiated,
            HipayPaymentStatus::ChallengeRequested => Self::AuthenticationPending,
            HipayPaymentStatus::SoftDeclined => Self::Failure,
            HipayPaymentStatus::PendingPayment => Self::Pending,
            HipayPaymentStatus::ChargedBack => Self::Failure,
            HipayPaymentStatus::Created => Self::Started,
            HipayPaymentStatus::UnableToAuthenticate | HipayPaymentStatus::CouldNotAuthenticate => {
                Self::AuthenticationFailed
            }
            HipayPaymentStatus::CardholderAuthenticated => Self::Pending,
            HipayPaymentStatus::AuthenticationAttempted => Self::AuthenticationPending,
            HipayPaymentStatus::Collected
            | HipayPaymentStatus::PartiallySettled
            | HipayPaymentStatus::PartiallyCollected
            | HipayPaymentStatus::Settled => Self::Charged,
            HipayPaymentStatus::AuthenticationRequested => Self::AuthenticationPending,
            HipayPaymentStatus::Authenticated => Self::AuthenticationSuccessful,
            HipayPaymentStatus::AcquirerNotFound => Self::Failure,
            HipayPaymentStatus::RiskAccepted => Self::Pending,
            HipayPaymentStatus::AuthorizationRefused => Self::Failure,
        }
    }
}

// HiPay Refund Status Enum - Type-safe refund status codes
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HipayRefundStatus {
    #[serde(rename = "124")]
    RefundRequested,
    #[serde(rename = "125")]
    Refunded,
    #[serde(rename = "126")]
    PartiallyRefunded,
    #[serde(rename = "165")]
    RefundRefused,
}

impl From<HipayRefundStatus> for RefundStatus {
    fn from(item: HipayRefundStatus) -> Self {
        match item {
            HipayRefundStatus::RefundRequested => Self::Pending,
            HipayRefundStatus::Refunded | HipayRefundStatus::PartiallyRefunded => Self::Success,
            HipayRefundStatus::RefundRefused => Self::Failure,
        }
    }
}

// Sync Response Types
// Reason struct for PSync response - matches v3 API format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Reason {
    pub reason: Option<String>,
    pub code: Option<u64>,
}

// HiPay v3 PSync Response - flat structure matching v3 transaction API
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HipaySyncResponse {
    Response {
        id: i64,
        status: i32,
        #[serde(default)]
        reason: Reason,
        #[serde(flatten)]
        extra: std::collections::HashMap<String, serde_json::Value>,
    },
    Error {
        message: String,
        code: u32,
    },
}

// HiPay v3 Refund Sync Response - JSON structure matching v3 transaction API
// Same endpoint as PSync but for refund transactions
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayRefundSyncJsonResponse {
    pub id: i64,
    pub status: i32,
}

// Type alias for backward compatibility
pub type HipayRefundSyncResponse = HipayRefundSyncJsonResponse;

// HiPay Operation Enum - Type-safe operation codes for maintenance requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HipayOperation {
    Capture,
    Refund,
    Cancel,
}

impl std::fmt::Display for HipayOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capture => write!(f, "capture"),
            Self::Refund => write!(f, "refund"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Authorization,
    Sale,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HipayPaymentsRequest {
    pub payment_product: String,
    pub orderid: String,
    pub operation: Operation,
    pub description: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardtoken: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decline_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_indicator: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HipayPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Get payment method - determine payment_product based on card network
        // Priority order (matching Hyperswitch):
        // 1. For tokenized cards: Use connector_customer (contains domestic_network from tokenization)
        // 2. For raw cards: Map card_network enum to HiPay payment products
        let payment_product = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Map card network to HiPay payment product
                match card_data.card_network.as_ref() {
                    Some(network) => match network {
                        common_enums::CardNetwork::Visa => "visa",
                        common_enums::CardNetwork::Mastercard => "mastercard",
                        common_enums::CardNetwork::AmericanExpress => "american-express",
                        common_enums::CardNetwork::JCB => "jcb",
                        common_enums::CardNetwork::DinersClub => "diners",
                        common_enums::CardNetwork::Discover => "discover",
                        common_enums::CardNetwork::CartesBancaires => "cb",
                        common_enums::CardNetwork::UnionPay => "unionpay",
                        common_enums::CardNetwork::Interac => "interac",
                        common_enums::CardNetwork::RuPay => "rupay",
                        common_enums::CardNetwork::Maestro => "maestro",
                        _ => "", // Empty string for unsupported card networks
                    },
                    None => "", // Empty string when card network is not provided
                }
                .to_string()
            }
            PaymentMethodData::PaymentMethodToken(_) => {
                // For tokenized cards, use connector_customer field which contains
                // the payment product/domestic_network from tokenization response
                item.router_data
                    .resource_common_data
                    .connector_customer
                    .clone()
                    .unwrap_or_default() // Empty string fallback
            }
            _ => {
                return Err(IntegrationError::not_implemented(
                    "Payment method not supported".to_string(),
                ))
                .change_context(IntegrationError::not_implemented(
                    "Payment method".to_string(),
                ))
            }
        };

        // Convert amount to StringMajorUnit (HiPay expects string with decimals)
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Determine operation based on capture method (matching HS)
        let operation = match item.router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Operation::Authorization,
            _ => Operation::Sale, // Automatic capture or default
        };

        // Extract card token from payment_method_token if present,
        // or from connector_customer as fallback (when token is passed via gRPC)
        let cardtoken = match &item.router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => Some(t.token.peek().to_string()),
            _ => item
                .router_data
                .resource_common_data
                .connector_customer
                .clone(),
        };

        // Build callback URLs matching HS implementation
        // Use /redirect/response/hipay path (not /redirect/complete/hipay)
        let base_url = item.router_data.request.complete_authorize_url.clone();
        let redirect_base = base_url.map(|url| {
            // Replace /redirect/complete/hipay with /redirect/response/hipay
            url.replace("/redirect/complete/hipay", "/redirect/response/hipay")
        });

        let accept_url = redirect_base.clone();
        let decline_url = redirect_base.clone();
        let pending_url = redirect_base.clone();
        let cancel_url = redirect_base.clone();
        let notify_url = redirect_base;

        // Set authentication_indicator to "0" for non-3DS (matching HS)
        // Can be extended later to support 3DS flows
        let authentication_indicator = Some("0".to_string());

        // Use description from resource_common_data (matching HS)
        // HS uses item.router_data.get_description() which returns this field
        // Default to "Short Description" to match HS if not present
        let description = item
            .router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_else(|| "Short Description".to_string());

        Ok(Self {
            payment_product,
            orderid: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            operation,
            description,
            currency: item.router_data.request.currency,
            amount,
            cardtoken,
            accept_url,
            decline_url,
            pending_url,
            cancel_url,
            notify_url,
            authentication_indicator,
        })
    }
}

// Response Structures aligned with Hyperswitch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOrder {
    id: String,
}

// Authorize Response - matches HiPay's order API response (camelCase from HiPay API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayPaymentsResponse {
    status: HipayPaymentStatus,
    message: String,
    order: PaymentOrder,
    #[serde(default)]
    #[serde(rename = "forwardUrl")]
    forward_url: String,
    #[serde(rename = "transactionReference")]
    transaction_reference: String,
}

// Generic Maintenance Response for Capture/Void/Refund operations (camelCase from HiPay API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HipayMaintenanceResponse<S> {
    status: S,
    message: String,
    #[serde(rename = "transactionReference")]
    transaction_reference: String,
}

// Type aliases for different flows - operation-specific types
pub type HipayAuthorizeResponse = HipayPaymentsResponse;
pub type HipayCaptureResponse = HipayMaintenanceResponse<HipayPaymentStatus>;
pub type HipayVoidResponse = HipayMaintenanceResponse<HipayPaymentStatus>;
pub type HipayRefundResponse = HipayMaintenanceResponse<HipayRefundStatus>;
pub type HipayPSyncResponse = HipaySyncResponse;
pub type HipayRSyncResponse = HipayRefundSyncResponse;

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<HipayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // Check if status is failure to return error response
        let response = if status == AttemptStatus::Failure {
            Err(domain_types::router_data::ErrorResponse {
                code: "DECLINED".to_string(),
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            // Check if redirection is needed (for 3DS flows)
            let redirection_data = if !item.response.forward_url.is_empty() {
                Some(Box::new(
                    domain_types::router_response_types::RedirectForm::Uri {
                        uri: item.response.forward_url.clone(),
                    },
                ))
            } else {
                None
            };

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Tokenization Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayTokenRequest<T: PaymentMethodDataTypes> {
    pub card_number: domain_types::payment_method_data::RawCardNumber<T>,
    pub card_expiry_month: Secret<String>,
    pub card_expiry_year: Secret<String>,
    pub card_holder: Secret<String>,
    pub cvc: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for HipayTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => Ok(Self {
                card_number: card_data.card_number.clone(),
                card_expiry_month: card_data.card_exp_month.clone(),
                card_expiry_year: card_data.card_exp_year.clone(),
                card_holder: item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .unwrap_or(Secret::new("".to_string())),
                cvc: card_data.card_cvc.clone(),
            }),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    "Payment method not supported for tokenization".to_string(),
                ))
                .change_context(IntegrationError::not_implemented(
                    "Payment method".to_string(),
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HipayTokenResponse {
    pub token: Secret<String>,
    pub request_id: String,
    pub brand: String,
    pub pan: Secret<String>,
    pub card_holder: Secret<String>,
    pub card_expiry_month: Secret<String>,
    pub card_expiry_year: Secret<String>,
    pub issuer: Option<String>,
    pub country: Option<common_enums::CountryAlpha2>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayTokenResponse, Self>) -> Result<Self, Self::Error> {
        use hyperswitch_masking::ExposeInterface;
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.token.expose(),
            }),
            ..item.router_data
        })
    }
}

// Helper function to map v3 API integer status codes to AttemptStatus
// Matches Hyperswitch's get_sync_status function
fn get_sync_status(state: i32) -> AttemptStatus {
    match state {
        9 => AttemptStatus::AuthenticationFailed,
        10 => AttemptStatus::Failure,
        11 => AttemptStatus::Failure,
        12 => AttemptStatus::Pending,
        13 => AttemptStatus::Failure,
        14 => AttemptStatus::Failure,
        15 => AttemptStatus::Voided,
        16 => AttemptStatus::Authorized,
        17 => AttemptStatus::CaptureInitiated,
        18 => AttemptStatus::Charged,
        19 => AttemptStatus::PartialCharged,
        29 => AttemptStatus::Failure,
        73 => AttemptStatus::CaptureFailed,
        74 => AttemptStatus::Pending,
        75 => AttemptStatus::VoidInitiated,
        77 => AttemptStatus::AuthenticationPending,
        78 => AttemptStatus::Failure,
        200 => AttemptStatus::Pending,
        1 => AttemptStatus::Started,
        5 => AttemptStatus::AuthenticationFailed,
        6 => AttemptStatus::Pending,
        7 => AttemptStatus::AuthenticationPending,
        8 => AttemptStatus::AuthenticationFailed,
        20 => AttemptStatus::Charged,
        21 => AttemptStatus::Charged,
        22 => AttemptStatus::Charged,
        23 => AttemptStatus::Charged,
        40 => AttemptStatus::AuthenticationPending,
        41 => AttemptStatus::AuthenticationSuccessful,
        51 => AttemptStatus::Failure,
        61 => AttemptStatus::Pending,
        63 => AttemptStatus::Failure,
        _ => AttemptStatus::Failure,
    }
}

// Payment Sync Response Implementation
// Uses HipaySyncResponse enum with v3 API flat structure
impl TryFrom<ResponseRouterData<HipayPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayPSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Handle sync response - could be Response or Error variant
        match item.response {
            HipaySyncResponse::Response { id, status, .. } => {
                // Convert i32 status code to AttemptStatus using mapping function
                let attempt_status = get_sync_status(status);

                Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(id.to_string()),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            HipaySyncResponse::Error { message, code } => Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: code.to_string(),
                    message: message.clone(),
                    reason: Some(message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: item
                        .router_data
                        .request
                        .connector_transaction_id
                        .get_connector_transaction_id()
                        .ok(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

// Capture Request Structure
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayCaptureRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for HipayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Convert amount to StringMajorUnit (HiPay expects decimal format)
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount_to_capture,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            operation: HipayOperation::Capture,
            currency: Some(item.router_data.request.currency),
            amount: Some(amount),
        })
    }
}

// Capture Response Implementation
// Uses HipayMaintenanceResponse<HipayPaymentStatus> with direct enum conversion
impl TryFrom<ResponseRouterData<HipayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayCaptureResponse, Self>) -> Result<Self, Self::Error> {
        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // Check if status indicates failure
        let response = if status == AttemptStatus::Failure || status == AttemptStatus::CaptureFailed
        {
            Err(domain_types::router_data::ErrorResponse {
                code: "CAPTURE_FAILED".to_string(),
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Refund Request Structure
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayRefundRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for HipayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Convert minor unit amount to StringMajorUnit (HiPay expects decimal format)
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            operation: HipayOperation::Refund,
            currency: Some(item.router_data.request.currency),
            amount: Some(amount),
        })
    }
}

// Refund Response Implementation
// Uses HipayMaintenanceResponse<HipayRefundStatus> with From trait conversion
impl TryFrom<ResponseRouterData<HipayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayRefundResponse, Self>) -> Result<Self, Self::Error> {
        // Convert HipayRefundStatus enum directly to RefundStatus using From trait
        let refund_status = RefundStatus::from(item.response.status.clone());

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_reference.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Refund Sync Response Implementation
// Uses HipayRefundSyncResponse JSON structure from v3 API
impl TryFrom<ResponseRouterData<HipayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayRSyncResponse, Self>) -> Result<Self, Self::Error> {
        // Map numeric status codes to RefundStatus (matching Hyperswitch)
        // Status codes from HiPay API documentation:
        // 24 = Refund Requested (Pending)
        // 25 = Refunded (Success)
        // 26 = Partially Refunded (Success)
        // 65 = Refund Refused (Failure)
        let refund_status = match item.response.status {
            25 | 26 => RefundStatus::Success,
            65 => RefundStatus::Failure,
            24 => RefundStatus::Pending,
            _ => RefundStatus::Pending, // Default to Pending for unknown statuses
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.to_string(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Void Request Structure
#[derive(Debug, Serialize, Deserialize)]
pub struct HipayVoidRequest {
    pub operation: HipayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<common_enums::Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<StringMajorUnit>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for HipayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            operation: HipayOperation::Cancel,
            currency: item.router_data.request.currency,
            amount: None, // None for void requests
        })
    }
}

// Void Response Implementation
// Uses HipayMaintenanceResponse<HipayPaymentStatus> with direct enum conversion
impl TryFrom<ResponseRouterData<HipayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<HipayVoidResponse, Self>) -> Result<Self, Self::Error> {
        // Convert HipayPaymentStatus enum directly to AttemptStatus using From trait
        let status = AttemptStatus::from(item.response.status.clone());

        // Check if status indicates void failure
        let response = if status == AttemptStatus::Failure || status == AttemptStatus::VoidFailed {
            Err(domain_types::router_data::ErrorResponse {
                code: "VOID_FAILED".to_string(),
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ========================================================================================
// REPEAT PAYMENT (MIT) REQUEST/RESPONSE TYPES
// ========================================================================================
// HiPay MIT uses the same /v1/order endpoint as Authorize, but with MIT-specific parameters:
// - eci=9 (Recurring E-commerce)
// - recurring_payment=1
// - authentication_indicator=0 (bypass 3DS for merchant-initiated)

#[derive(Debug, Serialize, Deserialize)]
pub struct HipayRepeatPaymentRequest {
    pub payment_product: String,
    pub orderid: String,
    pub operation: Operation,
    pub description: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardtoken: Option<String>,
    /// ECI=9 for MIT (Recurring E-commerce)
    pub eci: String,
    /// Always "1" for recurring/MIT
    pub recurring_payment: String,
    /// Always "0" for MIT (bypass 3DS)
    pub authentication_indicator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HipayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HipayRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: HipayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract card token from mandate reference (connector_mandate_id)
        let cardtoken = match &item.router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                connector_mandate_ref
                    .get_connector_mandate_id()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                            context: Default::default()
                        })
                    })?
            }
            MandateReferenceId::NetworkMandateId(network_mandate_id) => {
                network_mandate_id.clone()
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(error_stack::report!(
                    IntegrationError::NotImplemented(
                        "Network token with NTI not supported for HiPay repeat payments"
                            .to_string(),
                        Default::default(),
                    )
                ));
            }
        };

        // Determine payment_product from payment_method_data
        let payment_product = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                match card_data.card_network.as_ref() {
                    Some(network) => match network {
                        common_enums::CardNetwork::Visa => "visa",
                        common_enums::CardNetwork::Mastercard => "mastercard",
                        common_enums::CardNetwork::AmericanExpress => "american-express",
                        common_enums::CardNetwork::JCB => "jcb",
                        common_enums::CardNetwork::DinersClub => "diners",
                        common_enums::CardNetwork::Discover => "discover",
                        common_enums::CardNetwork::CartesBancaires => "cb",
                        common_enums::CardNetwork::UnionPay => "unionpay",
                        common_enums::CardNetwork::Interac => "interac",
                        common_enums::CardNetwork::RuPay => "rupay",
                        common_enums::CardNetwork::Maestro => "maestro",
                        _ => "visa", // Default to visa for unsupported networks
                    },
                    None => "visa", // Default to visa when network unknown
                }
                .to_string()
            }
            PaymentMethodData::CardToken(_) => {
                // For tokenized cards, use connector_customer field
                item.router_data
                    .resource_common_data
                    .connector_customer
                    .clone()
                    .unwrap_or_else(|| "visa".to_string())
            }
            PaymentMethodData::MandatePayment => {
                // For mandate payments, default to visa (HiPay requires payment_product)
                "visa".to_string()
            }
            _ => {
                return Err(IntegrationError::not_implemented(
                    "Payment method not supported for repeat payment".to_string(),
                ))
                .change_context(IntegrationError::not_implemented(
                    "Payment method".to_string(),
                ))
            }
        };

        // Convert amount to StringMajorUnit
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Determine operation based on capture method
        let operation = match item.router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => Operation::Authorization,
            _ => Operation::Sale,
        };

        let description = item
            .router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_else(|| "Recurring payment".to_string());

        let notify_url = item.router_data.request.webhook_url.clone();

        Ok(Self {
            payment_product,
            orderid: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            operation,
            description,
            currency: item.router_data.request.currency,
            amount,
            cardtoken: Some(cardtoken),
            eci: "9".to_string(),
            recurring_payment: "1".to_string(),
            authentication_indicator: "0".to_string(),
            notify_url,
        })
    }
}

// RepeatPayment response reuses the same Authorize response format
pub type HipayRepeatPaymentResponse = HipayPaymentsResponse;

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<HipayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<HipayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());

        let response = if status == AttemptStatus::Failure {
            Err(domain_types::router_data::ErrorResponse {
                code: "DECLINED".to_string(),
                message: item.response.message.clone(),
                reason: Some(item.response.message.clone()),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(item.response.transaction_reference.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            let redirection_data = if !item.response.forward_url.is_empty() {
                Some(Box::new(
                    domain_types::router_response_types::RedirectForm::Uri {
                        uri: item.response.forward_url.clone(),
                    },
                ))
            } else {
                None
            };

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_reference.clone(),
                ),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// GetFormData implementation for HipayRepeatPaymentRequest
impl GetFormData for HipayRepeatPaymentRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// ========================================================================================
// GetFormData TRAIT IMPLEMENTATIONS
// ========================================================================================
// These implementations enable multipart/form-data request format for HiPay API

// GetFormData implementation for HipayTokenRequest
impl<T: PaymentMethodDataTypes + Serialize> GetFormData for HipayTokenRequest<T> {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayPaymentsRequest
impl GetFormData for HipayPaymentsRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayCaptureRequest
impl GetFormData for HipayCaptureRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayVoidRequest
impl GetFormData for HipayVoidRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

// GetFormData implementation for HipayRefundRequest
impl GetFormData for HipayRefundRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}
