use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    request::Method,
    types::{FloatMajorUnit, StringMajorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone)]
pub struct AirwallexAuthType {
    pub api_key: Secret<String>,
    pub client_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AirwallexAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Airwallex {
            api_key, client_id, ..
        } = auth_type
        {
            Ok(Self {
                api_key: api_key.clone(),
                client_id: client_id.clone(),
            })
        } else {
            Err(error_stack::report!(
                errors::ConnectorError::FailedToObtainAuthType
            ))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirwallexErrorResponse {
    pub code: String,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirwallexAccessTokenResponse {
    pub token: Secret<String>,
    #[serde(with = "common_utils::custom_serde::iso8601")]
    pub expires_at: time::PrimitiveDateTime,
}

// Empty request body for CreateAccessToken - Airwallex requires empty JSON object {}
#[derive(Debug, Serialize)]
pub struct AirwallexAccessTokenRequest {
    // Empty struct that serializes to {} - Airwallex API requirement
}

// New unified request type for macro pattern that includes payment intent creation and confirmation
#[derive(Debug, Serialize)]
pub struct AirwallexPaymentRequest {
    // Request ID for confirm request
    pub request_id: String,
    // Payment method data for confirm step
    pub payment_method: AirwallexPaymentMethod,
    // Options for payment processing
    pub payment_method_options: Option<AirwallexPaymentOptions>,
    pub return_url: Option<String>,
    // Device data for fraud detection
    pub device_data: Option<AirwallexDeviceData>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AirwallexPaymentMethod {
    Card(AirwallexCardData),
    BankRedirect(AirwallexBankRedirectData),
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AirwallexBankRedirectData {
    Ideal(AirwallexIdealData),
    Trustly(AirwallexTrustlyData),
    Blik(AirwallexBlikData),
}

// Removed old AirwallexPaymentMethodData enum - now using individual Option fields for cleaner serialization

#[derive(Debug, Serialize)]
pub struct AirwallexCardData {
    pub card: AirwallexCardDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexCardDetails {
    pub number: Secret<String>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvc: Secret<String>,
    pub name: Option<Secret<String>>,
}

// Note: Wallet, PayLater, and BankRedirect data structures removed
// as they are not implemented yet. Only card payments are supported.

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexPaymentType {
    Card,
    Googlepay,
    Paypal,
    Klarna,
    Atome,
    Trustly,
    Blik,
    Ideal,
    Skrill,
    BankTransfer,
}

// BankRedirect-specific data structures
#[derive(Debug, Serialize)]
pub struct AirwallexIdealData {
    pub ideal: AirwallexIdealDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexIdealDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<common_enums::BankNames>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexTrustlyData {
    pub trustly: AirwallexTrustlyDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexTrustlyDetails {
    pub shopper_name: Secret<String>,
    pub country_code: common_enums::CountryAlpha2,
}

#[derive(Debug, Serialize)]
pub struct AirwallexBlikData {
    pub blik: AirwallexBlikDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexBlikDetails {
    pub shopper_name: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexDeviceData {
    pub accept_header: String,
    pub browser: AirwallexBrowser,
    pub ip_address: Option<Secret<String>>,
    pub language: String,
    pub mobile: Option<AirwallexMobile>,
    pub screen_color_depth: u8,
    pub screen_height: u32,
    pub screen_width: u32,
    pub timezone: String,
}

#[derive(Debug, Serialize)]
pub struct AirwallexBrowser {
    pub java_enabled: bool,
    pub javascript_enabled: bool,
    pub user_agent: String,
}

#[derive(Debug, Serialize)]
pub struct AirwallexMobile {
    pub device_model: Option<String>,
    pub os_type: Option<String>,
    pub os_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPaymentOptions {
    pub card: Option<AirwallexCardOptions>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexCardOptions {
    pub auto_capture: Option<bool>,
}

// Confirm request structure for 2-step flow (only payment method data)
#[derive(Debug, Serialize)]
pub struct AirwallexConfirmRequest {
    pub request_id: String,
    pub payment_method: AirwallexPaymentMethod,
    pub payment_method_options: Option<AirwallexPaymentOptions>,
    pub return_url: Option<String>,
    pub device_data: Option<AirwallexDeviceData>,
}

// Helper function to extract device data from browser info (matching Hyperswitch pattern)
fn get_device_data<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<Option<AirwallexDeviceData>, error_stack::Report<errors::ConnectorError>> {
    let browser_info = match request.get_browser_info() {
        Ok(info) => info,
        Err(_) => return Ok(None), // If browser info is not available, return None instead of erroring
    };

    let browser = AirwallexBrowser {
        java_enabled: browser_info.get_java_enabled().unwrap_or(false),
        javascript_enabled: browser_info.get_java_script_enabled().unwrap_or(true),
        user_agent: browser_info.get_user_agent().unwrap_or_default(),
    };

    let mobile = {
        let device_model = browser_info.device_model.clone();
        let os_type = browser_info.os_type.clone();
        let os_version = browser_info.os_version.clone();

        if device_model.is_some() || os_type.is_some() || os_version.is_some() {
            Some(AirwallexMobile {
                device_model,
                os_type,
                os_version,
            })
        } else {
            None
        }
    };

    Ok(Some(AirwallexDeviceData {
        accept_header: browser_info.get_accept_header().unwrap_or_default(),
        browser,
        ip_address: browser_info
            .get_ip_address()
            .ok()
            .map(|ip| Secret::new(ip.expose().to_string())),
        language: browser_info.get_language().unwrap_or_default(),
        mobile,
        screen_color_depth: browser_info.get_color_depth().unwrap_or(24),
        screen_height: browser_info.get_screen_height().unwrap_or(1080),
        screen_width: browser_info.get_screen_width().unwrap_or(1920),
        timezone: browser_info
            .get_time_zone()
            .map(|tz| tz.to_string())
            .unwrap_or_else(|_| "0".to_string()),
    }))
}

// Implementation for new unified request type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AirwallexPaymentRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // UCS unified flow - always create payment intent with payment method

        let payment_method = match item.router_data.request.payment_method_data.clone() {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                AirwallexPaymentMethod::Card(AirwallexCardData {
                    card: AirwallexCardDetails {
                        number: Secret::new(card_data.card_number.peek().to_string()),
                        expiry_month: card_data.card_exp_month.clone(),
                        expiry_year: card_data.get_expiry_year_4_digit(),
                        cvc: card_data.card_cvc.clone(),
                        name: card_data
                            .card_holder_name
                            .map(|name| Secret::new(name.expose())),
                    },
                    payment_method_type: AirwallexPaymentType::Card,
                })
            }
            domain_types::payment_method_data::PaymentMethodData::BankRedirect(
                bank_redirect_data,
            ) => match bank_redirect_data {
                domain_types::payment_method_data::BankRedirectData::Ideal { bank_name } => {
                    AirwallexPaymentMethod::BankRedirect(AirwallexBankRedirectData::Ideal(
                        AirwallexIdealData {
                            ideal: AirwallexIdealDetails { bank_name },
                            payment_method_type: AirwallexPaymentType::Ideal,
                        },
                    ))
                }
                domain_types::payment_method_data::BankRedirectData::Trustly { .. } => {
                    AirwallexPaymentMethod::BankRedirect(AirwallexBankRedirectData::Trustly(
                        AirwallexTrustlyData {
                            trustly: AirwallexTrustlyDetails {
                                shopper_name: item
                                    .router_data
                                    .resource_common_data
                                    .get_billing_full_name()
                                    .map_err(|_| errors::ConnectorError::MissingRequiredField {
                                        field_name: "billing.first_name",
                                    })?,
                                country_code: item
                                    .router_data
                                    .resource_common_data
                                    .get_billing_country()
                                    .map_err(|_| errors::ConnectorError::MissingRequiredField {
                                        field_name: "country_code",
                                    })?,
                            },
                            payment_method_type: AirwallexPaymentType::Trustly,
                        },
                    ))
                }
                domain_types::payment_method_data::BankRedirectData::Blik { blik_code: _ } => {
                    AirwallexPaymentMethod::BankRedirect(AirwallexBankRedirectData::Blik(
                        AirwallexBlikData {
                            blik: AirwallexBlikDetails {
                                shopper_name: item
                                    .router_data
                                    .resource_common_data
                                    .get_billing_full_name()
                                    .map_err(|_| errors::ConnectorError::MissingRequiredField {
                                        field_name: "billing.first_name",
                                    })?,
                            },
                            payment_method_type: AirwallexPaymentType::Blik,
                        },
                    ))
                }
                _ => {
                    return Err(errors::ConnectorError::NotSupported {
                        message: "Bank Redirect Payment Method".to_string(),
                        connector: "Airwallex",
                    }
                    .into())
                }
            },
            _ => {
                return Err(errors::ConnectorError::NotSupported {
                    message: "Payment Method".to_string(),
                    connector: "Airwallex",
                }
                .into())
            }
        };

        let auto_capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
        );

        let payment_method_options = Some(AirwallexPaymentOptions {
            card: Some(AirwallexCardOptions {
                auto_capture: Some(auto_capture),
            }),
        });

        let device_data = get_device_data(&item.router_data.request)?;

        // Generate unique request_id for Authorize/confirm step
        // Different from CreateOrder to avoid Airwallex duplicate_request error
        let request_id = format!(
            "confirm_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            request_id,
            payment_method,
            payment_method_options,
            return_url: item.router_data.request.get_router_return_url().ok(),
            device_data,
        })
    }
}

// Unified response type for all payment operations (Authorize, PSync, Capture, Void)
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexPaymentsResponse {
    pub id: String,
    pub status: AirwallexPaymentStatus,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // Latest payment attempt information
    pub latest_payment_attempt: Option<AirwallexPaymentAttempt>,
    // Payment method information
    pub payment_method: Option<AirwallexPaymentMethodInfo>,
    // Next action for 3DS or other redirects
    pub next_action: Option<AirwallexNextAction>,
    // Payment intent details
    pub payment_intent_id: Option<String>,
    // Capture information
    pub captured_amount: Option<FloatMajorUnit>,
    // Authorization code from processor
    pub authorization_code: Option<String>,
    // Network transaction ID
    pub network_transaction_id: Option<String>,
    // Processor response
    pub processor_response: Option<AirwallexProcessorResponse>,
    // Risk information
    pub risk_score: Option<String>,
    // Void-specific fields
    pub cancelled_at: Option<String>,
    pub cancellation_reason: Option<String>,
}

// Type alias - reuse the same response structure for PSync
pub type AirwallexSyncResponse = AirwallexPaymentsResponse;

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexPaymentAttempt {
    pub id: Option<String>,
    pub status: Option<String>, // Changed from AirwallexPaymentStatus to String to handle different values
    pub amount: Option<FloatMajorUnit>,
    pub payment_method: Option<AirwallexPaymentMethodInfo>,
    pub authorization_code: Option<String>,
    pub network_transaction_id: Option<String>,
    pub processor_response: Option<AirwallexProcessorResponse>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexPaymentStatus {
    RequiresPaymentMethod,
    RequiresCustomerAction,
    RequiresCapture,
    Authorized,       // Payment authorized (from latest_payment_attempt)
    Paid,             // Payment paid/captured (from latest_payment_attempt)
    CaptureRequested, // Payment captured but settlement in progress
    Processing,
    Succeeded,
    Settled, // Payment fully settled - indicates successful completion
    Cancelled,
    Failed,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexPaymentMethodInfo {
    #[serde(rename = "type")]
    pub method_type: String,
    pub card: Option<AirwallexCardInfo>,
    // Bank redirect fields
    pub blik: Option<Secret<serde_json::Value>>, // For BLIK payment method details
    pub ideal: Option<Secret<serde_json::Value>>, // For iDEAL payment method details
    pub trustly: Option<Secret<serde_json::Value>>, // For Trustly payment method details
    // Additional payment method fields
    pub id: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexCardInfo {
    pub last4: Option<String>,
    pub brand: Option<String>,
    pub exp_month: Option<Secret<String>>,
    pub exp_year: Option<Secret<String>>,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexNextAction {
    #[serde(rename = "type")]
    pub action_type: String,
    pub method: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexProcessorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
    pub decline_code: Option<String>,
    pub network_code: Option<String>,
}

// Helper function to get payment status from Airwallex status (following Hyperswitch pattern)
fn get_payment_status(
    status: &AirwallexPaymentStatus,
    next_action: &Option<AirwallexNextAction>,
) -> AttemptStatus {
    match status {
        AirwallexPaymentStatus::Succeeded => AttemptStatus::Charged,
        AirwallexPaymentStatus::Failed => AttemptStatus::Failure,
        AirwallexPaymentStatus::Processing => AttemptStatus::Pending,
        AirwallexPaymentStatus::RequiresPaymentMethod => AttemptStatus::PaymentMethodAwaited,
        AirwallexPaymentStatus::RequiresCustomerAction => {
            // Check next_action to determine specific pending state based on action_type
            next_action
                .as_ref()
                .map_or(AttemptStatus::AuthenticationPending, |action| match action
                    .action_type
                    .as_str()
                {
                    "device_data_collection" => AttemptStatus::DeviceDataCollectionPending,
                    _ => AttemptStatus::AuthenticationPending,
                })
        }
        AirwallexPaymentStatus::RequiresCapture => AttemptStatus::Authorized,
        AirwallexPaymentStatus::Authorized => AttemptStatus::Authorized,
        AirwallexPaymentStatus::Paid => AttemptStatus::Charged,
        AirwallexPaymentStatus::Cancelled => AttemptStatus::Voided,
        AirwallexPaymentStatus::CaptureRequested => AttemptStatus::Charged,
        AirwallexPaymentStatus::Settled => AttemptStatus::Charged,
    }
}

// New response transformer that addresses PR #240 critical issues
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // Handle redirection for bank redirects and 3DS
        let redirection_data = item.response.next_action.as_ref().and_then(|next_action| {
            if next_action.action_type == "redirect" {
                next_action.url.as_ref().and_then(|url_str| {
                    Url::parse(url_str)
                        .ok()
                        .map(|url| Box::new(RedirectForm::from((url, Method::Get))))
                })
            } else {
                None
            }
        });

        // Extract network transaction ID for network response fields (PR #240 Issue #4)
        let network_txn_id = item
            .response
            .network_transaction_id
            .or(item.response.authorization_code.clone());

        // Following hyperswitch pattern - no connector_metadata
        let connector_metadata = None;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data,
                mandate_reference: None,
                connector_metadata,
                network_txn_id,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
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

impl TryFrom<ResponseRouterData<AirwallexSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Use the same simple status mapping as hyperswitch
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // Extract network transaction ID (check latest_payment_attempt first, then main response)
        let network_txn_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|attempt| attempt.network_transaction_id.clone())
            .or_else(|| item.response.network_transaction_id.clone())
            .or_else(|| {
                item.response
                    .latest_payment_attempt
                    .as_ref()
                    .and_then(|attempt| attempt.authorization_code.clone())
            })
            .or(item.response.authorization_code.clone());

        // Following hyperswitch pattern - no connector_metadata
        let connector_metadata = None;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None, // PSync doesn't handle redirections
                mandate_reference: None,
                connector_metadata,
                network_txn_id,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
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
// ===== CAPTURE FLOW TYPES =====

#[derive(Debug, Serialize)]
pub struct AirwallexCaptureRequest {
    pub amount: StringMajorUnit, // Amount in major units
    pub request_id: String,      // Unique identifier for this capture request
}

// Type alias - reuse the same response structure for Capture
pub type AirwallexCaptureResponse = AirwallexPaymentsResponse;

// Request transformer for Capture flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AirwallexCaptureRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract capture amount from the capture data
        let capture_amount = item.router_data.request.amount_to_capture;

        // Use connector amount converter for proper amount formatting in major units (hyperswitch pattern)
        let amount = item
            .connector
            .amount_converter
            .convert(
                common_utils::MinorUnit::new(capture_amount),
                item.router_data.request.currency,
            )
            .map_err(|e| {
                errors::ConnectorError::RequestEncodingFailedWithReason(format!(
                    "Amount conversion failed: {e}"
                ))
            })?;

        // Generate unique request_id for idempotency using connector_request_reference_id
        let request_id = format!(
            "capture_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self { amount, request_id })
    }
}

// Response transformer for Capture flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Use the same simple status mapping as hyperswitch
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // Address PR #240 Issue #4: Network Specific Fields
        // Extract network transaction ID (prefer latest attempt, then main response)
        let network_txn_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|attempt| attempt.network_transaction_id.clone())
            .or_else(|| item.response.network_transaction_id.clone())
            .or_else(|| {
                item.response
                    .latest_payment_attempt
                    .as_ref()
                    .and_then(|attempt| attempt.authorization_code.clone())
            })
            .or(item.response.authorization_code.clone());

        // Following hyperswitch pattern - no connector_metadata
        let connector_metadata = None;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None, // Capture doesn't involve redirections
                mandate_reference: None,
                connector_metadata,
                network_txn_id,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
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

// ===== REFUND FLOW TYPES =====

#[derive(Debug, Serialize)]
pub struct AirwallexRefundRequest {
    pub payment_attempt_id: String, // From connector_transaction_id
    pub amount: StringMajorUnit,    // Refund amount in major units
    pub reason: Option<String>,     // Refund reason if provided
    pub request_id: String,         // Unique identifier for idempotency
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexRefundResponse {
    pub id: String,                         // Refund ID
    pub request_id: Option<String>,         // Echo back request ID
    pub payment_intent_id: Option<String>,  // Original payment intent ID
    pub payment_attempt_id: Option<String>, // Original payment attempt ID
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,                 // Currency code
    pub reason: Option<String>,                     // Refund reason
    pub status: AirwallexRefundStatus,              // RECEIVED, ACCEPTED, SETTLED, FAILED
    pub created_at: Option<String>,                 // Creation timestamp
    pub updated_at: Option<String>,                 // Update timestamp
    pub acquirer_reference_number: Option<String>,  // Network reference
    pub failure_details: Option<serde_json::Value>, // Error details if failed
    pub metadata: Option<serde_json::Value>,        // Additional metadata
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexRefundStatus {
    Received,
    Accepted,
    Settled,
    Failed,
}

// Request transformer for Refund flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AirwallexRefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract payment attempt ID from connector_transaction_id
        let payment_attempt_id = item.router_data.request.connector_transaction_id.clone();

        // Extract refund amount from RefundsData and convert to major units (hyperswitch pattern)
        let refund_amount = item.router_data.request.refund_amount;
        let amount = item
            .connector
            .amount_converter
            .convert(
                common_utils::MinorUnit::new(refund_amount),
                item.router_data.request.currency,
            )
            .map_err(|e| {
                errors::ConnectorError::RequestEncodingFailedWithReason(format!(
                    "Amount conversion failed: {e}"
                ))
            })?;

        // Generate unique request_id for idempotency using connector_request_reference_id
        let request_id = format!(
            "refund_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            payment_attempt_id,
            amount,
            reason: item.router_data.request.reason.clone(),
            request_id,
        })
    }
}

// Response transformer for Refund flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = RefundStatus::from(item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status: status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC FLOW TYPES =====

// Reuse the same response structure as AirwallexRefundResponse since it's the same endpoint (GET /pa/refunds/{id})
pub type AirwallexRefundSyncResponse = AirwallexRefundResponse;

// Response transformer for RSync flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = RefundStatus::from(item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status: status,
                status_code: item.http_code,
            }),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Simple status mapping following Hyperswitch pattern
// Trust the Airwallex API to return correct status
impl From<AirwallexRefundStatus> for RefundStatus {
    fn from(status: AirwallexRefundStatus) -> Self {
        match status {
            AirwallexRefundStatus::Settled => Self::Success,
            AirwallexRefundStatus::Failed => Self::Failure,
            AirwallexRefundStatus::Received | AirwallexRefundStatus::Accepted => Self::Pending,
        }
    }
}

// ===== VOID FLOW TYPES =====

#[derive(Debug, Serialize)]
pub struct AirwallexVoidRequest {
    pub cancellation_reason: Option<String>, // Reason for cancellation
    pub request_id: String,                  // Unique identifier for idempotency
}

// Type alias - reuse the same response structure for Void
pub type AirwallexVoidResponse = AirwallexPaymentsResponse;

// Request transformer for Void flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AirwallexVoidRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract cancellation reason from PaymentVoidData (if available)
        let cancellation_reason = item
            .router_data
            .request
            .cancellation_reason
            .clone()
            .or_else(|| Some("Voided by merchant".to_string()));

        // Generate unique request_id for idempotency using connector_request_reference_id
        let request_id = format!(
            "void_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            cancellation_reason,
            request_id,
        })
    }
}

// Response transformer for Void flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // Address PR #240 Issue #4: Network Specific Fields
        // Extract network transaction ID (prefer latest attempt, then main response)
        let network_txn_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|attempt| attempt.network_transaction_id.clone())
            .or_else(|| item.response.network_transaction_id.clone())
            .or_else(|| {
                item.response
                    .latest_payment_attempt
                    .as_ref()
                    .and_then(|attempt| attempt.authorization_code.clone())
            })
            .or(item.response.authorization_code.clone());

        // Following hyperswitch pattern - no connector_metadata for void
        let connector_metadata = None;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None, // Void doesn't involve redirections
                mandate_reference: None,
                connector_metadata,
                network_txn_id,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
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

// Removed over-engineered validation - use simple get_payment_status instead
// The Airwallex API is trusted to return correct status (following Hyperswitch pattern)

// Implementation for confirm request type (2-step flow)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AirwallexConfirmRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Confirm flow for 2-step process (not currently used in UCS)

        let payment_method = match item.router_data.request.payment_method_data.clone() {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                AirwallexPaymentMethod::Card(AirwallexCardData {
                    card: AirwallexCardDetails {
                        number: Secret::new(card_data.card_number.peek().to_string()),
                        expiry_month: card_data.card_exp_month.clone(),
                        expiry_year: card_data.get_expiry_year_4_digit(),
                        cvc: card_data.card_cvc.clone(),
                        name: card_data
                            .card_holder_name
                            .map(|name| Secret::new(name.expose())),
                    },
                    payment_method_type: AirwallexPaymentType::Card,
                })
            }
            domain_types::payment_method_data::PaymentMethodData::BankRedirect(
                bank_redirect_data,
            ) => match bank_redirect_data {
                domain_types::payment_method_data::BankRedirectData::Ideal { bank_name } => {
                    AirwallexPaymentMethod::BankRedirect(AirwallexBankRedirectData::Ideal(
                        AirwallexIdealData {
                            ideal: AirwallexIdealDetails { bank_name },
                            payment_method_type: AirwallexPaymentType::Ideal,
                        },
                    ))
                }
                domain_types::payment_method_data::BankRedirectData::Trustly { .. } => {
                    AirwallexPaymentMethod::BankRedirect(AirwallexBankRedirectData::Trustly(
                        AirwallexTrustlyData {
                            trustly: AirwallexTrustlyDetails {
                                shopper_name: item
                                    .router_data
                                    .resource_common_data
                                    .get_billing_full_name()
                                    .map_err(|_| errors::ConnectorError::MissingRequiredField {
                                        field_name: "billing.first_name",
                                    })?,
                                country_code: item
                                    .router_data
                                    .resource_common_data
                                    .get_billing_country()
                                    .map_err(|_| errors::ConnectorError::MissingRequiredField {
                                        field_name: "country_code",
                                    })?,
                            },
                            payment_method_type: AirwallexPaymentType::Trustly,
                        },
                    ))
                }
                domain_types::payment_method_data::BankRedirectData::Blik { blik_code: _ } => {
                    AirwallexPaymentMethod::BankRedirect(AirwallexBankRedirectData::Blik(
                        AirwallexBlikData {
                            blik: AirwallexBlikDetails {
                                shopper_name: item
                                    .router_data
                                    .resource_common_data
                                    .get_billing_full_name()
                                    .map_err(|_| errors::ConnectorError::MissingRequiredField {
                                        field_name: "billing.first_name",
                                    })?,
                            },
                            payment_method_type: AirwallexPaymentType::Blik,
                        },
                    ))
                }
                _ => {
                    return Err(errors::ConnectorError::NotSupported {
                        message: "Bank Redirect Payment Method".to_string(),
                        connector: "Airwallex",
                    }
                    .into())
                }
            },
            _ => {
                return Err(errors::ConnectorError::NotSupported {
                    message: "Payment Method".to_string(),
                    connector: "Airwallex",
                }
                .into())
            }
        };

        let auto_capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
        );

        let payment_method_options = Some(AirwallexPaymentOptions {
            card: Some(AirwallexCardOptions {
                auto_capture: Some(auto_capture),
            }),
        });

        let device_data = get_device_data(&item.router_data.request)?;

        Ok(Self {
            request_id: format!(
                "confirm_{}",
                item.router_data.resource_common_data.payment_id
            ),
            payment_method,
            payment_method_options,
            return_url: item.router_data.request.get_router_return_url().ok(),
            device_data,
        })
    }
}

// ===== CREATE ORDER FLOW TYPES =====

// Referrer data to identify UCS implementation to Airwallex
#[derive(Debug, Serialize)]
pub struct AirwallexReferrerData {
    #[serde(rename = "type")]
    pub r_type: String,
    pub version: String,
}

// Order data for payment intents (required for pay-later methods)
#[derive(Debug, Serialize)]
pub struct AirwallexOrderData {
    pub products: Vec<AirwallexProductData>,
    pub shipping: Option<AirwallexShippingData>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexProductData {
    pub name: String,
    pub quantity: u16,
    pub unit_price: StringMajorUnit, // Using StringMajorUnit for amount consistency
}

#[derive(Debug, Serialize)]
pub struct AirwallexShippingData {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone_number: Option<Secret<String>>,
    pub shipping_method: Option<String>,
    pub address: Option<AirwallexAddressData>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexAddressData {
    pub country_code: String,
    pub state: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub street: Option<Secret<String>>,
    pub postcode: Option<Secret<String>>,
}

// CreateOrder request structure (Step 1 - Intent creation without payment method)
#[derive(Debug, Serialize)]
pub struct AirwallexIntentRequest {
    pub request_id: String,
    pub amount: StringMajorUnit,
    pub currency: Currency,
    pub merchant_order_id: String,
    // UCS identification for Airwallex whitelisting
    pub referrer_data: AirwallexReferrerData,
    // Optional order data for pay-later methods
    pub order: Option<AirwallexOrderData>,
}

// CreateOrder response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexIntentResponse {
    pub id: String,
    pub request_id: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub merchant_order_id: Option<String>,
    pub status: AirwallexPaymentStatus,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // Client secret for frontend integration
    pub client_secret: Option<String>,
    // Available payment method types
    pub available_payment_method_types: Option<Vec<String>>,
}

// Request transformer for CreateOrder flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateOrder,
                PaymentFlowData,
                domain_types::connector_types::PaymentCreateOrderData,
                domain_types::connector_types::PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for AirwallexIntentRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateOrder,
                PaymentFlowData,
                domain_types::connector_types::PaymentCreateOrderData,
                domain_types::connector_types::PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Create referrer data for Airwallex identification
        let referrer_data = AirwallexReferrerData {
            r_type: "hyperswitch".to_string(),
            version: "1.0.0".to_string(),
        };

        // Convert amount using the same converter as other flows
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.amount,
                item.router_data.request.currency,
            )
            .map_err(|e| {
                errors::ConnectorError::RequestEncodingFailedWithReason(format!(
                    "Amount conversion failed: {e}"
                ))
            })?;

        // For now, no order data - can be enhanced later when order details are needed
        let order = None;

        // Generate unique request_id for CreateOrder step
        let request_id = format!(
            "create_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            request_id,
            amount,
            currency: item.router_data.request.currency,
            merchant_order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            referrer_data,
            order,
        })
    }
}

// Response transformer for CreateOrder flow
impl TryFrom<ResponseRouterData<AirwallexIntentResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::CreateOrder,
        PaymentFlowData,
        domain_types::connector_types::PaymentCreateOrderData,
        domain_types::connector_types::PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexIntentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;

        // Map intent status to order status
        let status = match item.response.status {
            AirwallexPaymentStatus::RequiresPaymentMethod => AttemptStatus::PaymentMethodAwaited,
            AirwallexPaymentStatus::RequiresCustomerAction => AttemptStatus::AuthenticationPending,
            AirwallexPaymentStatus::Processing => AttemptStatus::Pending,
            AirwallexPaymentStatus::Succeeded => AttemptStatus::Charged,
            AirwallexPaymentStatus::Settled => AttemptStatus::Charged,
            AirwallexPaymentStatus::Failed => AttemptStatus::Failure,
            AirwallexPaymentStatus::Cancelled => AttemptStatus::Voided,
            AirwallexPaymentStatus::RequiresCapture => AttemptStatus::Authorized,
            AirwallexPaymentStatus::Authorized => AttemptStatus::Authorized,
            AirwallexPaymentStatus::Paid => AttemptStatus::Charged,
            AirwallexPaymentStatus::CaptureRequested => AttemptStatus::Charged,
        };

        router_data.response = Ok(domain_types::connector_types::PaymentCreateOrderResponse {
            order_id: item.response.id.clone(),
            session_token: None,
        });

        // Update the flow data with the new status and store payment intent ID as reference_id (like Razorpay V2)
        router_data.resource_common_data = PaymentFlowData {
            status,
            reference_id: Some(item.response.id), // Store payment intent ID for subsequent Authorize call
            ..router_data.resource_common_data
        };

        Ok(router_data)
    }
}

// Access Token Request Transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateAccessToken,
                PaymentFlowData,
                domain_types::connector_types::AccessTokenRequestData,
                domain_types::connector_types::AccessTokenResponseData,
            >,
            T,
        >,
    > for AirwallexAccessTokenRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        _item: super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateAccessToken,
                PaymentFlowData,
                domain_types::connector_types::AccessTokenRequestData,
                domain_types::connector_types::AccessTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Airwallex CreateAccessToken requires empty JSON body {}
        // The authentication headers (x-api-key, x-client-id) are set separately
        Ok(Self {
            // Empty struct serializes to {}
        })
    }
}

// Access Token Response Transformer
impl TryFrom<ResponseRouterData<AirwallexAccessTokenResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::CreateAccessToken,
        PaymentFlowData,
        domain_types::connector_types::AccessTokenRequestData,
        domain_types::connector_types::AccessTokenResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;

        let expires = (item.response.expires_at - common_utils::date_time::now()).whole_seconds();

        router_data.response = Ok(domain_types::connector_types::AccessTokenResponseData {
            access_token: item.response.token,
            token_type: Some("Bearer".to_string()),
            expires_in: Some(expires),
        });

        Ok(router_data)
    }
}
