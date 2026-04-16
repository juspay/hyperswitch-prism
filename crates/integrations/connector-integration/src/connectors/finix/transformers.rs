use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{consts, pii::Email, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, PaymentMethodToken, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorAuthType, ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FinixAuthType {
    pub finix_user_name: Secret<String>,
    pub finix_password: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_identity_id: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for FinixAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::MultiAuthKey {
                api_key,
                api_secret,
                key1,
                key2,
            } => Ok(Self {
                finix_user_name: api_key.to_owned(),
                finix_password: api_secret.to_owned(),
                merchant_id: key1.to_owned(),
                merchant_identity_id: key2.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for FinixAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Finix {
                finix_user_name,
                finix_password,
                merchant_identity_id,
                merchant_id,
                ..
            } => Ok(Self {
                finix_user_name: finix_user_name.clone(),
                finix_password: finix_password.clone(),
                merchant_id: merchant_id.clone(),
                merchant_identity_id: merchant_identity_id.clone(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

impl FinixAuthType {
    pub fn generate_basic_auth(&self) -> String {
        let credentials = format!(
            "{}:{}",
            self.finix_user_name.peek(),
            self.finix_password.peek()
        );
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, credentials)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinixErrorResponse {
    pub total: Option<i64>,
    #[serde(rename = "_embedded")]
    pub embedded: Option<FinixErrorEmbedded>,
}

#[derive(Debug, Clone)]
pub enum FinixId {
    Auth(String),
    Transfer(String),
}

impl From<String> for FinixId {
    fn from(id: String) -> Self {
        if id.starts_with("AU") {
            Self::Auth(id)
        } else if id.starts_with("TR") {
            Self::Transfer(id)
        } else {
            // Default to Auth if prefix doesn't match
            tracing::warn!("Unrecognized Finix ID prefix: {}", id);
            Self::Auth(id)
        }
    }
}

impl std::fmt::Display for FinixId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(id) => write!(f, "{}", id),
            Self::Transfer(id) => write!(f, "{}", id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinixErrorEmbedded {
    pub errors: Option<Vec<FinixError>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinixError {
    pub logref: Option<String>,
    pub message: Option<String>,
    pub code: Option<String>,
}

impl FinixErrorResponse {
    /// Extract error code from the wrapped error format
    pub fn get_code(&self) -> String {
        self.embedded
            .as_ref()
            .and_then(|e| e.errors.as_ref())
            .and_then(|errors| errors.first())
            .and_then(|err| err.code.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string())
    }

    /// Extract error message from the wrapped error format
    pub fn get_message(&self) -> String {
        self.embedded
            .as_ref()
            .and_then(|e| e.errors.as_ref())
            .and_then(|errors| errors.first())
            .and_then(|err| err.message.clone())
            .unwrap_or_else(|| "Unknown error".to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct FinixCreateIdentityRequest {
    pub entity: FinixIdentityEntity,
    pub tags: Option<FinixTags>,
    #[serde(rename = "type")]
    pub identity_type: FinixIdentityType,
}

#[derive(Debug, Serialize)]
pub struct FinixIdentityEntity {
    pub phone: Option<Secret<String>>,
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub email: Option<Email>,
    pub personal_address: Option<FinixAddress>,
}

#[derive(Debug, Serialize)]
pub struct FinixAddress {
    pub line1: Option<Secret<String>>,
    pub line2: Option<Secret<String>>,
    pub city: Option<String>,
    pub region: Option<Secret<String>>,
    pub postal_code: Option<Secret<String>>,
    pub country: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum FinixIdentityType {
    PERSONAL,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinixIdentityResponse {
    pub id: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub application: Option<String>,
    pub entity: Option<HashMap<String, serde_json::Value>>,
    pub tags: Option<HashMap<String, String>>,
}

pub type FinixTags = HashMap<String, String>;

// Tag key used to round-trip the merchant's `connector_request_reference_id`
// through Finix so the response transformer can populate `connector_response_reference_id`
// only when Finix actually echoed the value back.
pub const FINIX_REFERENCE_TAG_KEY: &str = "merchant_reference";

fn build_reference_tags(reference: &str) -> FinixTags {
    let mut tags = HashMap::new();
    tags.insert(FINIX_REFERENCE_TAG_KEY.to_string(), reference.to_string());
    tags
}

#[derive(Debug, Serialize)]
pub struct FinixCreatePaymentInstrumentRequest {
    #[serde(rename = "type")]
    pub instrument_type: FinixPaymentInstrumentType,
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_month: Option<Secret<i8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_year: Option<Secret<i32>>,
    pub identity: String,
    pub tags: Option<FinixTags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<FinixAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_identity: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_token: Option<Secret<String>>,
    // Bank account specific fields for ACH
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<FinixAccountType>,
}

// Finix bank-account `account_type` enum.
// See https://finix.com/docs/api/tag/Payment-Instruments/operation/createPaymentInstrument
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixAccountType {
    Checking,
    Savings,
    BusinessChecking,
    BusinessSavings,
}

impl From<&common_enums::BankHolderType> for FinixAccountType {
    fn from(holder_type: &common_enums::BankHolderType) -> Self {
        match holder_type {
            common_enums::BankHolderType::Personal => Self::Checking,
            common_enums::BankHolderType::Business => Self::BusinessChecking,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FinixPaymentInstrumentType {
    #[serde(rename = "PAYMENT_CARD")]
    PaymentCard,
    #[serde(rename = "GOOGLE_PAY")]
    GooglePay,
    #[serde(rename = "APPLE_PAY")]
    ApplePay,
    #[serde(rename = "BANK_ACCOUNT")]
    BankAccount,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinixInstrumentResponse {
    // `id` is treated as the success indicator: a 2xx with no `id` (or `enabled: false`)
    // is surfaced as a payment failure rather than silently mapped to a success.
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub application: Option<String>,
    pub identity: Option<String>,
    #[serde(rename = "type")]
    pub instrument_type: Option<FinixPaymentInstrumentType>,
    pub tags: Option<FinixTags>,
    pub card_type: Option<String>,
    pub card_brand: Option<String>,
    pub fingerprint: Option<String>,
    pub name: Option<Secret<String>>,
    pub currency: Option<Currency>,
    #[serde(default)]
    pub enabled: bool,
    pub disabled_code: Option<String>,
    pub disabled_message: Option<String>,
}

// SetupMandate uses the same request/response structures as PaymentMethodToken
// Type aliases to satisfy macro system requirements (each flow needs distinct type names)
pub type FinixSetupMandateRequest = FinixCreatePaymentInstrumentRequest;
pub type FinixSetupMandateResponse = FinixInstrumentResponse;

// AUTHORIZE FLOW - REQUEST/RESPONSE

#[derive(Debug, Serialize)]
pub struct FinixAuthorizeRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub source: String,
    pub merchant: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinixAuthorizeResponse {
    pub id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub state: FinixPaymentStatus,
    #[serde(rename = "_links")]
    pub links: Option<FinixLinks>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub transfer: Option<String>,
}

// RepeatPayment (MIT) reuses the Authorize request/response shape.
// A recurring charge is a POST /transfers (or /authorizations) with `source` set to a
// previously stored Payment Instrument ID returned by SetupRecurring.
pub type FinixRepeatPaymentRequest = FinixAuthorizeRequest;
pub type FinixRepeatPaymentResponse = FinixAuthorizeResponse;

#[derive(Debug, Serialize, Deserialize)]
pub struct FinixLinks {
    #[serde(rename = "self")]
    pub self_: Option<FinixLink>,
    pub application: Option<FinixLink>,
    pub merchant_identity: Option<FinixLink>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinixLink {
    pub href: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixPaymentStatus {
    Succeeded,
    Failed,
    Pending,
    Canceled,
    Unknown,
}

impl From<&FinixPaymentStatus> for AttemptStatus {
    fn from(status: &FinixPaymentStatus) -> Self {
        match status {
            FinixPaymentStatus::Succeeded => Self::Charged,
            FinixPaymentStatus::Failed => Self::Failure,
            FinixPaymentStatus::Pending => Self::Pending,
            FinixPaymentStatus::Canceled => Self::Voided,
            FinixPaymentStatus::Unknown => Self::Pending,
        }
    }
}

// TRYFROM IMPLEMENTATIONS - AUTHORIZE REQUEST

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FinixAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
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

        // Extract merchant ID from auth_type
        let auth = FinixAuthType::try_from(&router_data.connector_config)?;
        let merchant_id = auth.merchant_id.peek().to_string();

        // For Finix, we need a payment instrument ID (source)
        // First try to get token from payment_method_token, otherwise create instrument inline
        let source = match &router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => t.token.peek().to_string(),
            _ => return Err(IntegrationError::NotSupported {
                message: "Finix authorize only accepts a tokenized payment instrument ID as source. Raw card/wallet/bank data cannot be passed directly.".to_string(),
                connector: "finix",
                context: IntegrationErrorContext {
                    suggested_action: Some("Call CreateConnectorCustomer then PaymentMethodToken to obtain a Finix Payment Instrument ID (PI...) before authorizing.".to_string()),
                    doc_url: Some("https://docs.finix.com/api/authorizations".to_string()),
                    additional_context: Some("The Finix POST /authorizations `source` field only accepts a Payment Instrument ID. See https://docs.finix.com/api/payment-instruments to tokenize first.".to_string()),
                },
            }.into()),
        };

        Ok(Self {
            amount: router_data.request.amount,
            currency: router_data.request.currency,
            source,
            merchant: merchant_id,
            idempotency_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            tags: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FinixAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Handle error vs success responses
        match &response.failure_message {
            Some(_failure_message) => Ok(Self {
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
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                ..item.router_data
            }),
            None => {
                // Determine status based on ID type (following Hyperswitch pattern):
                // - Transfer (TR*): Succeeded -> Charged
                // - Authorization (AU*): Succeeded -> Authorized
                let finix_id = FinixId::from(response.id.clone());
                let status = match (&finix_id, &response.state) {
                    (FinixId::Transfer(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Charged,
                    (FinixId::Transfer(_), FinixPaymentStatus::Pending) => AttemptStatus::Pending,
                    (FinixId::Auth(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Authorized,
                    (FinixId::Auth(_), FinixPaymentStatus::Pending) => {
                        AttemptStatus::AuthenticationPending
                    }
                    (_, FinixPaymentStatus::Failed) => AttemptStatus::Failure,
                    (_, FinixPaymentStatus::Canceled) => AttemptStatus::Voided,
                    (_, FinixPaymentStatus::Unknown) => AttemptStatus::Pending,
                };

                // Determine connector_transaction_id:
                // - Transfer (TR*): Use the response ID directly (it's already a transfer ID)
                // - Authorization (AU*): Use transfer field if present (for refunds), else use ID
                let connector_transaction_id = match &finix_id {
                    FinixId::Transfer(_) => response.id.clone(),
                    FinixId::Auth(_) => response
                        .transfer
                        .clone()
                        .unwrap_or_else(|| response.id.clone()),
                };

                Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(response.id.clone()),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data.clone()
                    },
                    ..item.router_data
                })
            }
        }
    }
}

// Common response struct for payment operations (PSync, Capture, Void, etc.)
#[derive(Debug, Serialize, Deserialize)]
pub struct FinixPaymentsResponse {
    pub id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    #[serde(alias = "status")]
    pub state: FinixPaymentStatus,
    #[serde(rename = "_links")]
    pub links: Option<FinixLinks>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub transfer: Option<String>,
}

// Aliases for backward compatibility during migration
pub type FinixPSyncResponse = FinixPaymentsResponse;

impl TryFrom<ResponseRouterData<FinixPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FinixPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Determine status based on ID type (AU* = Auth/Authorized, TR* = Transfer/Charged)
        // This follows the same pattern as Hyperswitch
        let finix_id = FinixId::from(response.id.clone());
        let status = match (&finix_id, &response.state) {
            (FinixId::Auth(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Authorized,
            (FinixId::Auth(_), FinixPaymentStatus::Pending) => AttemptStatus::AuthenticationPending,
            (FinixId::Transfer(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Charged,
            (FinixId::Transfer(_), FinixPaymentStatus::Pending) => AttemptStatus::Pending,
            (_, FinixPaymentStatus::Failed) => AttemptStatus::Failure,
            (_, FinixPaymentStatus::Canceled) => AttemptStatus::Voided,
            (_, FinixPaymentStatus::Unknown) => AttemptStatus::Pending,
        };

        // For transfers (TR...), use the transfer ID directly
        // For authorizations (AU...), use transfer ID if available for refunds
        let connector_transaction_id = response
            .transfer
            .clone()
            .unwrap_or_else(|| response.id.clone());

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// CAPTURE FLOW - REQUEST/RESPONSE

#[derive(Debug, Serialize)]
pub struct FinixCaptureRequest {
    pub capture_amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_id: Option<String>,
}

// Use common response struct for capture
pub type FinixCaptureResponse = FinixPaymentsResponse;

// TRYFROM IMPLEMENTATIONS - CAPTURE REQUEST

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for FinixCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            capture_amount: MinorUnit::new(item.router_data.request.amount_to_capture),
            idempotency_id: None,
        })
    }
}

// TRYFROM IMPLEMENTATIONS - CAPTURE RESPONSE

impl TryFrom<ResponseRouterData<FinixCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FinixCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = AttemptStatus::from(&response.state);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// VOID FLOW - REQUEST/RESPONSE

#[derive(Debug, Serialize)]
pub struct FinixVoidRequest {
    pub void_me: bool,
}

// Use common response struct for void
pub type FinixVoidResponse = FinixPaymentsResponse;

// REFUND FLOW - REQUEST/RESPONSE

#[derive(Debug, Serialize)]
pub struct FinixRefundRequest {
    pub refund_amount: MinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_id: Option<String>,
}

// Use common response struct for refund
pub type FinixRefundResponse = FinixPaymentsResponse;

// RSync FLOW - REQUEST/RESPONSE

// Use common response struct for rsync
pub type FinixRSyncResponse = FinixPaymentsResponse;

// TRYFROM IMPLEMENTATIONS - RSync RESPONSE

impl TryFrom<ResponseRouterData<FinixRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FinixRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let refund_status = match response.state {
            FinixPaymentStatus::Succeeded => RefundStatus::Success,
            FinixPaymentStatus::Failed => RefundStatus::Failure,
            FinixPaymentStatus::Pending => RefundStatus::Pending,
            FinixPaymentStatus::Canceled => RefundStatus::Failure,
            FinixPaymentStatus::Unknown => RefundStatus::Pending,
        };
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// TRYFROM IMPLEMENTATIONS - VOID REQUEST

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for FinixVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::FinixRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { void_me: true })
    }
}

// TRYFROM IMPLEMENTATIONS - VOID RESPONSE

impl TryFrom<ResponseRouterData<FinixVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FinixVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        // Void-specific status mapping
        let status = match response.state {
            FinixPaymentStatus::Succeeded => AttemptStatus::Voided,
            FinixPaymentStatus::Failed => AttemptStatus::VoidFailed,
            FinixPaymentStatus::Pending => AttemptStatus::Pending,
            FinixPaymentStatus::Canceled => AttemptStatus::Voided,
            FinixPaymentStatus::Unknown => AttemptStatus::Pending,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

// TRYFROM IMPLEMENTATIONS - REFUND REQUEST

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for FinixRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            refund_amount: item.router_data.request.minor_refund_amount,
            idempotency_id: None,
        })
    }
}

// TRYFROM IMPLEMENTATIONS - REFUND RESPONSE

impl TryFrom<ResponseRouterData<FinixRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FinixRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = match response.state {
            FinixPaymentStatus::Succeeded => RefundStatus::Success,
            FinixPaymentStatus::Failed => RefundStatus::Failure,
            FinixPaymentStatus::Pending => RefundStatus::Pending,
            FinixPaymentStatus::Canceled => RefundStatus::Failure,
            FinixPaymentStatus::Unknown => RefundStatus::Pending,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status: status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// TRYFROM IMPLEMENTATIONS - CREATE CONNECTOR CUSTOMER REQUEST

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for FinixCreateIdentityRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let customer_data = &item.router_data.request;

        // Parse name into first and last name if available
        let (first_name, last_name) = customer_data
            .name
            .as_ref()
            .map(|name| {
                let name_str = name.peek().trim();
                match name_str.rsplit_once(' ') {
                    Some((first, last)) => (
                        Some(Secret::new(first.to_string())),
                        Some(Secret::new(last.to_string())),
                    ),
                    None => (Some(Secret::new(name_str.to_string())), None),
                }
            })
            .unwrap_or((None, None));

        Ok(Self {
            entity: FinixIdentityEntity {
                phone: customer_data.phone.clone(),
                first_name,
                last_name,
                email: customer_data.email.clone().map(|e| e.expose()),
                personal_address: None,
            },
            tags: None,
            identity_type: FinixIdentityType::PERSONAL,
        })
    }
}

// TRYFROM IMPLEMENTATIONS - CREATE CONNECTOR CUSTOMER RESPONSE

impl TryFrom<ResponseRouterData<FinixIdentityResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixIdentityResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: response.id,
            }),
            ..item.router_data
        })
    }
}

// TRYFROM IMPLEMENTATIONS - PAYMENT METHOD TOKEN REQUEST

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for FinixCreatePaymentInstrumentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let token_data = &item.router_data.request;

        // Get customer_id from connector metadata stored in resource_common_data
        let customer_id = item
            .router_data
            .resource_common_data
            .connector_customer
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_customer_id",
                context: Default::default(),
            })?;

        match &token_data.payment_method_data {
            PaymentMethodData::Card(card) => Ok(Self {
                instrument_type: FinixPaymentInstrumentType::PaymentCard,
                name: card.card_holder_name.clone(),
                number: Some(Secret::new(card.card_number.peek().to_string())),
                security_code: Some(card.card_cvc.clone()),
                expiration_month: Some(card.get_expiry_month_as_i8()?),
                expiration_year: Some(card.get_expiry_year_as_i32()?),
                identity: customer_id,
                tags: None,
                address: None,
                merchant_identity: None,
                third_party_token: None,
                account_number: None,
                bank_code: None,
                account_type: None,
            }),
            PaymentMethodData::BankDebit(bank_debit_data) => {
                match bank_debit_data {
                    domain_types::payment_method_data::BankDebitData::AchBankDebit {
                        account_number,
                        routing_number,
                        card_holder_name,
                        bank_account_holder_name,
                        bank_holder_type,
                        ..
                    } => {
                        // Determine account holder name: prefer bank_account_holder_name, fall back to card_holder_name
                        let name = bank_account_holder_name
                            .clone()
                            .or_else(|| card_holder_name.clone());

                        let account_type = bank_holder_type.as_ref().map(FinixAccountType::from);

                        Ok(Self {
                            instrument_type: FinixPaymentInstrumentType::BankAccount,
                            name,
                            number: None,
                            security_code: None,
                            expiration_month: None,
                            expiration_year: None,
                            identity: customer_id,
                            tags: None,
                            address: None,
                            merchant_identity: None,
                            third_party_token: None,
                            account_number: Some(account_number.clone()),
                            bank_code: Some(routing_number.clone()),
                            account_type,
                        })
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        "Only ACH Bank Debit is supported".to_string(),
                        Default::default(),
                    )
                    .into()),
                }
            }
            PaymentMethodData::Wallet(wallet_data) => {
                match wallet_data {
                    domain_types::payment_method_data::WalletData::GooglePay(google_pay_data) => {
                        // Get merchant_identity_id from auth
                        let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;
                        let merchant_identity = auth.merchant_identity_id.peek().to_string();

                        // Extract the encrypted token from Google Pay
                        let third_party_token = google_pay_data
                            .tokenization_data
                            .get_encrypted_google_pay_payment_data_mandatory()
                            .change_context(IntegrationError::InvalidWalletToken {
                                wallet_name: "Google Pay".to_string(),
                                context: Default::default(),
                            })?;

                        Ok(Self {
                            instrument_type: FinixPaymentInstrumentType::GooglePay,
                            name: None, // Name is optional for Google Pay tokenization
                            number: None,
                            security_code: None,
                            expiration_month: None,
                            expiration_year: None,
                            identity: customer_id,
                            tags: None,
                            address: None,
                            merchant_identity: Some(Secret::new(merchant_identity)),
                            third_party_token: Some(Secret::new(third_party_token.token.clone())),
                            account_number: None,
                            bank_code: None,
                            account_type: None,
                        })
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        "Only Google Pay wallet tokenization is supported".into(),
                        Default::default(),
                    )
                    .into()),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card, bank debit, and Google Pay tokenization are supported".into(),
                Default::default(),
            )
            .into()),
        }
    }
}

// TRYFROM IMPLEMENTATIONS - PAYMENT METHOD TOKEN RESPONSE

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FinixInstrumentResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixInstrumentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        Ok(Self {
            response: match (response.id.clone(), response.enabled) {
                (Some(id), true) => Ok(PaymentMethodTokenResponse { token: id }),
                _ => Err(disabled_instrument_error(&response, item.http_code)),
            },
            ..item.router_data
        })
    }
}

// Treat a 2xx that omits `id` or returns `enabled: false` as a payment failure.
// Surfaces `disabled_code` / `disabled_message` so the caller can act on it.
fn disabled_instrument_error(
    response: &FinixInstrumentResponse,
    status_code: u16,
) -> ErrorResponse {
    let code = response
        .disabled_code
        .clone()
        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
    let message = response
        .disabled_message
        .clone()
        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
    ErrorResponse {
        code,
        message: message.clone(),
        reason: Some(message),
        status_code,
        attempt_status: Some(AttemptStatus::Failure),
        connector_transaction_id: response.id.clone(),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    }
}

// =============================================================================
// SETUP MANDATE FLOW - REQUEST/RESPONSE TRANSFORMERS
// =============================================================================
// SetupMandate creates a payment instrument in Finix that can be used for future
// recurring payments. The connector_mandate_id returned is the Finix Payment
// Instrument ID (PI...) which can be passed as `source` in future authorizations.
//
// Prerequisites:
// - CreateConnectorCustomer must be called first to create a Finix identity
// - The identity ID is stored in resource_common_data.connector_customer
//
// Flow:
// 1. SetupMandate receives payment method data (card, bank, wallet)
// 2. Transformer creates FinixCreatePaymentInstrumentRequest with identity
// 3. POST /payment_instruments creates the instrument
// 4. Response contains the Payment Instrument ID as connector_mandate_id

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FinixCreatePaymentInstrumentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let mandate_data = &item.router_data.request;

        // Get customer_id (Finix identity) from connector metadata stored in resource_common_data
        // This is set by the CreateConnectorCustomer flow which must run before SetupMandate
        let customer_id = item
            .router_data
            .resource_common_data
            .connector_customer
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_customer_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Call CreateConnectorCustomer flow first to create a Finix identity"
                            .to_string(),
                    ),
                    doc_url: Some("https://docs.finix.com/api/identities".to_string()),
                    additional_context: Some(
                        "Finix requires an identity (connector customer) to create a payment instrument. \
                         The identity ID (ID...) must be provided in the 'identity' field."
                            .to_string(),
                    ),
                },
            })?;

        let tags = Some(build_reference_tags(
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        ));

        match &mandate_data.payment_method_data {
            PaymentMethodData::Card(card) => Ok(Self {
                instrument_type: FinixPaymentInstrumentType::PaymentCard,
                name: card.card_holder_name.clone(),
                number: Some(Secret::new(card.card_number.peek().to_string())),
                security_code: Some(card.card_cvc.clone()),
                expiration_month: Some(card.get_expiry_month_as_i8()?),
                expiration_year: Some(card.get_expiry_year_as_i32()?),
                identity: customer_id,
                tags,
                address: None,
                merchant_identity: None,
                third_party_token: None,
                account_number: None,
                bank_code: None,
                account_type: None,
            }),
            PaymentMethodData::BankDebit(bank_debit_data) => {
                match bank_debit_data {
                    domain_types::payment_method_data::BankDebitData::AchBankDebit {
                        account_number,
                        routing_number,
                        card_holder_name,
                        bank_account_holder_name,
                        bank_holder_type,
                        ..
                    } => {
                        // Determine account holder name: prefer bank_account_holder_name, fall back to card_holder_name
                        let name = bank_account_holder_name
                            .clone()
                            .or_else(|| card_holder_name.clone());

                        let account_type = bank_holder_type.as_ref().map(FinixAccountType::from);

                        Ok(Self {
                            instrument_type: FinixPaymentInstrumentType::BankAccount,
                            name,
                            number: None,
                            security_code: None,
                            expiration_month: None,
                            expiration_year: None,
                            identity: customer_id,
                            tags,
                            address: None,
                            merchant_identity: None,
                            third_party_token: None,
                            account_number: Some(account_number.clone()),
                            bank_code: Some(routing_number.clone()),
                            account_type,
                        })
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        "Only ACH Bank Debit is supported for SetupMandate".to_string(),
                        Default::default(),
                    )
                    .into()),
                }
            }
            PaymentMethodData::Wallet(wallet_data) => {
                match wallet_data {
                    domain_types::payment_method_data::WalletData::GooglePay(google_pay_data) => {
                        // Get merchant_identity_id from auth for wallet tokenization
                        let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;
                        let merchant_identity = auth.merchant_identity_id.peek().to_string();

                        // Extract the encrypted token from Google Pay
                        let third_party_token = google_pay_data
                            .tokenization_data
                            .get_encrypted_google_pay_payment_data_mandatory()
                            .change_context(IntegrationError::InvalidWalletToken {
                                wallet_name: "Google Pay".to_string(),
                                context: Default::default(),
                            })?;

                        Ok(Self {
                            instrument_type: FinixPaymentInstrumentType::GooglePay,
                            name: None,
                            number: None,
                            security_code: None,
                            expiration_month: None,
                            expiration_year: None,
                            identity: customer_id,
                            tags,
                            address: None,
                            merchant_identity: Some(Secret::new(merchant_identity)),
                            third_party_token: Some(Secret::new(third_party_token.token.clone())),
                            account_number: None,
                            bank_code: None,
                            account_type: None,
                        })
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        "Only Google Pay wallet is supported for SetupMandate".into(),
                        Default::default(),
                    )
                    .into()),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card, bank debit (ACH), and Google Pay are supported for SetupMandate".into(),
                Default::default(),
            )
            .into()),
        }
    }
}

// SetupMandate Response Transformer
// Treats the response as success only when Finix returns an `id` and `enabled: true`.
// A `disabled_code` / `disabled_message` returned alongside `enabled: false` is surfaced
// as a payment failure with the disabled details propagated into the ErrorResponse.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FinixInstrumentResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixInstrumentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let sent_reference = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        match (response.id.clone(), response.enabled) {
            (Some(id), true) => {
                // Only echo `connector_response_reference_id` when Finix returned the
                // merchant_reference tag we sent on the request — otherwise we cannot
                // assert any link between our reference and the connector resource.
                let echoed = response
                    .tags
                    .as_ref()
                    .and_then(|t| t.get(FINIX_REFERENCE_TAG_KEY))
                    .filter(|v| **v == sent_reference)
                    .map(|_| id.clone());

                let mandate_reference = Some(Box::new(MandateReference {
                    connector_mandate_id: Some(id.clone()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                }));

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Charged,
                        ..item.router_data.resource_common_data.clone()
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(id),
                        redirection_data: None,
                        mandate_reference,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: echoed,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            _ => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(disabled_instrument_error(&response, item.http_code)),
                ..item.router_data
            }),
        }
    }
}

// =============================================================================
// REPEAT PAYMENT FLOW - REQUEST/RESPONSE TRANSFORMERS
// =============================================================================
// RepeatPayment is the MIT (merchant-initiated) charge against a previously stored
// Payment Instrument (PI...) returned by SetupRecurring. Finix accepts the stored
// PI directly as the `source` field of POST /transfers (auto-capture) or
// POST /authorizations (manual capture). No MIT-specific fields are required —
// possession of the PI plus a customer_acceptance recorded at SetupRecurring time
// is the merchant-initiated authorization.

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FinixRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let auth = FinixAuthType::try_from(&router_data.connector_config)?;
        let merchant_id = auth.merchant_id.peek().to_string();

        // Extract the stored Payment Instrument ID (PI...) returned by SetupRecurring.
        // Network-mandate variants are rejected — Finix MIT requires its own stored PI.
        let source = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => connector_mandate_ids
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Call SetupRecurring first to obtain a Finix Payment Instrument ID."
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.finix.com/api/payment-instruments".to_string()),
                        additional_context: Some(
                            "Finix RepeatPayment requires the Payment Instrument ID (PI...) \
                             returned by SetupRecurring as the `source` of the new transfer."
                                .to_string(),
                        ),
                    },
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "Finix RepeatPayment only supports connector-mandate references; \
                         network mandate id / network token NTI flows are not supported."
                        .to_string(),
                    connector: "finix",
                    context: Default::default(),
                }
                .into());
            }
        };

        Ok(Self {
            amount: router_data.request.minor_amount,
            currency: router_data.request.currency,
            source,
            merchant: merchant_id,
            idempotency_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            tags: Some(serde_json::json!({
                FINIX_REFERENCE_TAG_KEY: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            })),
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FinixRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Surface explicit Finix failure responses (failure_message present) directly.
        if let Some(failure_message) = response.failure_message.clone() {
            let code = response
                .failure_code
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code,
                    message: failure_message.clone(),
                    reason: Some(failure_message),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: Some(response.id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            });
        }

        // Reuse Authorize's ID-aware status mapping: TR* → Charged, AU* → Authorized.
        let finix_id = FinixId::from(response.id.clone());
        let status = match (&finix_id, &response.state) {
            (FinixId::Transfer(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Charged,
            (FinixId::Transfer(_), FinixPaymentStatus::Pending) => AttemptStatus::Pending,
            (FinixId::Auth(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Authorized,
            (FinixId::Auth(_), FinixPaymentStatus::Pending) => AttemptStatus::AuthenticationPending,
            (_, FinixPaymentStatus::Failed) => AttemptStatus::Failure,
            (_, FinixPaymentStatus::Canceled) => AttemptStatus::Voided,
            (_, FinixPaymentStatus::Unknown) => AttemptStatus::Pending,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
