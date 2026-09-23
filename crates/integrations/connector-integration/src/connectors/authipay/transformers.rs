use std::collections::HashMap;

use crate::types::ResponseRouterData;
use base64::{engine::general_purpose, Engine};
use common_enums::AttemptStatus;
use common_utils::{
    crypto::{self, SignMessage},
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PostAuthenticate, PreAuthenticate, RSync, Refund, Void, VoidPC,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::IntegrationErrorContext,
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

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
        common_utils::fp_utils::generate_uuid_v4()
    }

    /// Generate timestamp in milliseconds since Unix epoch
    pub fn generate_timestamp() -> String {
        common_utils::date_time::now_unix_millis().to_string()
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

/// Spec shape-(a) error envelope (`{"errors": [{title, detail, source}, ...]}`)
/// returned by edge/Apigee validation — see spec "Status Mappings — Error →
/// ErrorResponse mapping (recommended)" tail paragraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthipayApigeeErrorsEnvelope {
    pub errors: Vec<AuthipayApigeeErrorEntry>,
}

/// One entry of the shape-(a) envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthipayApigeeErrorEntry {
    pub title: Option<String>,
    pub detail: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
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

impl AuthipayErrorResponse {
    /// Parse an error body tolerating both documented shapes: shape (b) — the
    /// standard `{code, message, details}` object — first (kept behaviour); on
    /// shape-(a)'s `{"errors": [...]}` envelope, map the first entry's
    /// `title`/`detail` into `code`/`message` (spec recommendation) rather
    /// than failing deserialisation.
    pub fn from_bytes(body: &[u8]) -> Option<Self> {
        if let Ok(shaped) = serde_json::from_slice::<Self>(body) {
            return Some(shaped);
        }
        if let Ok(envelope) = serde_json::from_slice::<AuthipayApigeeErrorsEnvelope>(body) {
            if let Some(first) = envelope.errors.into_iter().next() {
                return Some(Self {
                    code: first.title,
                    message: first.detail,
                    details: None,
                    api_trace_id: first.source,
                });
            }
        }
        None
    }
}

// ===== REQUEST TYPE ENUMS =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthipayRequestType {
    PaymentCardSaleTransaction,
    PaymentCardPreAuthTransaction,
    PaymentCardPayerAuthTransaction,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_result: Option<AuthipayAuthenticationResult>,
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
    // Spec §4 PaymentCard wire name (authipay technical_specification.md:339):
    // the JSON key must be `cardholderName`; camelCase of `holder` would emit `holder`,
    // which the gateway rejects with 400 INVALID_INPUT "No field named 'holder' exists
    // for class PaymentCard".
    #[serde(rename = "cardholderName", skip_serializing_if = "Option::is_none")]
    pub holder: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpiryDate {
    pub month: Secret<String>,
    pub year: Secret<String>,
}

// ===== SHARED 3-D SECURE WIRE STRUCTURES =====
// Used by the PreAuthenticate leg (authenticationRequest), the PostAuthenticate leg
// (Secure3DAuthenticationUpdateRequest) and the Authorize 3DS completion arm
// (authenticationResult). Never send authenticationRequest and authenticationResult in the
// same request (spec: Complete Endpoint Inventory — the two are mutually exclusive).

#[derive(Debug, Clone, Serialize)]
pub enum AuthipayAuthenticationRequestType {
    #[serde(rename = "Secure3DAuthenticationRequest")]
    Secure3DAuthenticationRequest,
}

#[derive(Debug, Clone, Serialize)]
pub enum AuthipayMessageCategory {
    #[serde(rename = "01")]
    PaymentAuthentication,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayAuthenticationRequest {
    pub authentication_type: AuthipayAuthenticationRequestType,
    #[serde(rename = "termURL")]
    pub term_url: String,
    #[serde(rename = "methodNotificationURL")]
    pub method_notification_url: String,
    pub message_category: AuthipayMessageCategory,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayAuthenticationResult {
    pub authentication_type: AuthipayAuthenticationResultType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(rename = "dsTransactionId", skip_serializing_if = "Option::is_none")]
    pub ds_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum AuthipayAuthenticationResultType {
    #[serde(rename = "Secure3DAuthenticationResult")]
    Secure3DAuthenticationResult,
}

/// Build an internal PaymentCard from shared card data (Authorize + PreAuthenticate arms).
fn build_payment_card<T: PaymentMethodDataTypes>(
    card_data: &Card<T>,
    customer_name: Option<String>,
) -> Result<PaymentMethod<T>, error_stack::Report<IntegrationError>> {
    let year_yy = card_data.get_card_expiry_year_2_digit()?;
    Ok(PaymentMethod {
        payment_card: PaymentCard {
            number: card_data.card_number.clone(),
            expiry_date: ExpiryDate {
                month: Secret::new(card_data.card_exp_month.peek().clone()),
                year: year_yy,
            },
            security_code: Some(card_data.card_cvc.clone()),
            holder: customer_name.map(Secret::new),
        },
    })
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
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported".to_string(),
                    Default::default()
                )))
            }
        };

        // 3DS completion arm (G-ThreeDS__Card-05): when the composite 3DS path fed external
        // authentication results into request.authentication_data, embed them via
        // authenticationResult. This is mutually exclusive with authenticationRequest on the
        // wire; the Authorize flow never emits authenticationRequest, so the invariant
        // "never both" holds by construction. Absence of authentication_data keeps the plain
        // no-3DS sale/preauth body byte-for-byte unchanged.
        let authentication_result =
            item.request
                .authentication_data
                .as_ref()
                .map(|authentication_data| AuthipayAuthenticationResult {
                    authentication_type:
                        AuthipayAuthenticationResultType::Secure3DAuthenticationResult,
                    cavv: authentication_data.cavv.clone(),
                    xid: authentication_data.threeds_server_transaction_id.clone(),
                    ds_transaction_id: authentication_data.ds_trans_id.clone(),
                    eci: authentication_data.eci.clone(),
                });

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
                authentication_result,
            })
        } else {
            Ok(Self {
                request_type: AuthipayRequestType::PaymentCardSaleTransaction,
                merchant_transaction_id,
                transaction_amount,
                order,
                payment_method,
                authentication_result,
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
    // 3DS payloads, present only on the payer-auth initiate (WAITING) and validate
    // responses; serde-optional so plain authorize/PSync/capture/void responses parse unchanged.
    pub authentication_response: Option<AuthipayAuthenticationResponse>,
    #[serde(rename = "secure3dResponse")]
    pub secure3d_response: Option<AuthipaySecure3DResponse>,
}

/// Secure3DAuthenticationResponse — returned on the payer-auth initiate call when an ACS
/// challenge round-trip is required (spec: authenticationResponse — WAITING payload).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayAuthenticationResponse {
    #[serde(rename = "type")]
    pub auth_type: Option<String>,
    pub version: Option<String>,
    pub params: Option<AuthipayAuthenticationResponseParams>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayAuthenticationResponseParams {
    #[serde(rename = "acsURL")]
    pub acs_url: Option<String>,
    #[serde(rename = "cReq")]
    pub c_req: Option<String>,
    pub payer_authentication_request: Option<String>,
    pub merchant_data: Option<String>,
    pub session_data: Option<String>,
}

/// secure3dResponse — returned on the payer-auth validate (PATCH) call with the final
/// authentication values (spec: Secure3DAuthenticationUpdateRequest response).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipaySecure3DResponse {
    #[serde(rename = "responseCode3dSecure")]
    pub response_code_3d_secure: Option<String>,
    /// Cardholder Authentication Verification Value (CAVV) — PII-like; masked.
    pub authentication_value: Option<Secret<String>>,
    pub directory_server_transaction_id: Option<String>,
    pub eci: Option<String>,
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
                network_txn_link_id: None,
                connector_response_reference_id: item.response.client_request_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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
                network_txn_link_id: None,
                connector_response_reference_id: item.response.client_request_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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
                network_txn_link_id: None,
                connector_response_reference_id: item.response.client_request_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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
                if matches!(transaction_result, Some(AuthipayPaymentResult::Approved))
                    || matches!(transaction_status, Some(AuthipayPaymentStatus::Approved)) =>
            {
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
            acquirer_reference_number: None,
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
            acquirer_reference_number: None,
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
                network_txn_link_id: None,
                connector_response_reference_id: item.response.client_request_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== VOIDPC REQUEST STRUCTURE =====
// VoidPostCapture (Reverse) — cancels a captured (PostAuth) transaction before settlement
// Uses requestType: VoidTransaction (distinct from Void which uses VoidPreAuthTransactions)
// AUTHIPAY always voids the full original amount; partial void is not supported

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayVoidPCRequest {
    pub request_type: AuthipayRequestType,
}

// ===== VOIDPC REQUEST TRANSFORMATION =====

impl
    TryFrom<
        &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
    > for AuthipayVoidPCRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // VoidTransaction requires no amount — AUTHIPAY always voids the full original amount
        Ok(Self {
            request_type: AuthipayRequestType::VoidTransaction,
        })
    }
}

// ===== VOIDPC RESPONSE TRANSFORMATION =====

fn map_void_pc_status(
    transaction_type: AuthipayTransactionType,
    transaction_status: Option<AuthipayPaymentStatus>,
    transaction_result: Option<AuthipayPaymentResult>,
    transaction_state: Option<AuthipayTransactionState>,
) -> common_enums::PostCaptureVoidStatus {
    if transaction_type != AuthipayTransactionType::Void {
        return common_enums::PostCaptureVoidStatus::Failed;
    }

    if let Some(state) = transaction_state {
        match state {
            AuthipayTransactionState::Voided => {
                return common_enums::PostCaptureVoidStatus::Succeeded;
            }
            AuthipayTransactionState::Declined => {
                return common_enums::PostCaptureVoidStatus::Failed;
            }
            AuthipayTransactionState::Pending | AuthipayTransactionState::Waiting => {
                return common_enums::PostCaptureVoidStatus::Pending;
            }
            _ => {}
        }
    }

    if let Some(result) = transaction_result {
        return match result {
            AuthipayPaymentResult::Approved => common_enums::PostCaptureVoidStatus::Succeeded,
            AuthipayPaymentResult::Waiting | AuthipayPaymentResult::Partial => {
                common_enums::PostCaptureVoidStatus::Pending
            }
            AuthipayPaymentResult::Declined
            | AuthipayPaymentResult::Failed
            | AuthipayPaymentResult::Fraud => common_enums::PostCaptureVoidStatus::Failed,
        };
    }

    if let Some(status) = transaction_status {
        return match status {
            AuthipayPaymentStatus::Approved => common_enums::PostCaptureVoidStatus::Succeeded,
            AuthipayPaymentStatus::Waiting | AuthipayPaymentStatus::Partial => {
                common_enums::PostCaptureVoidStatus::Pending
            }
            AuthipayPaymentStatus::ValidationFailed
            | AuthipayPaymentStatus::ProcessingFailed
            | AuthipayPaymentStatus::Declined => common_enums::PostCaptureVoidStatus::Failed,
        };
    }

    common_enums::PostCaptureVoidStatus::Pending
}

impl TryFrom<ResponseRouterData<AuthipayPaymentsResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let post_capture_void_status = map_void_pc_status(
            item.response.transaction_type.clone(),
            item.response.transaction_status.clone(),
            item.response.transaction_result.clone(),
            item.response.transaction_state.clone(),
        );

        let description = post_capture_void_status
            .is_post_capture_void_failure()
            .then(|| {
                item.response.error_message.clone().or_else(|| {
                    item.response
                        .processor
                        .as_ref()
                        .and_then(|p| p.response_message.clone())
                })
            })
            .flatten();

        Ok(Self {
            response: Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id: Some(item.response.ipg_transaction_id.clone()),
                description,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== PRE-AUTHENTICATE (payer-auth initiate) =====
// POST /payments with requestType=PaymentCardPayerAuthTransaction + authenticationRequest.
// Spec Leg A1: Charges NO; returns WAITING + authenticationResponse.params (ACS redirect
// payload). The transactionAmount mirrors the eventual charge amount but moves no money.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPreAuthenticateRequest<T: PaymentMethodDataTypes> {
    pub request_type: AuthipayRequestType,
    pub merchant_transaction_id: String,
    pub transaction_amount: TransactionAmount,
    pub order: OrderDetails,
    pub payment_method: PaymentMethod<T>,
    pub authentication_request: AuthipayAuthenticationRequest,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    > for AuthipayPreAuthenticateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // G-ThreeDS__Card-03: validate capture intent the gateway cannot express.
        // Note: the payer-auth leg carries no requestType distinction for Manual vs
        // Automatic (the charge intent is decided later on the Authorize leg); the helper
        // is only consulted for its CaptureMethodNotSupported error on
        // ManualMultiple/Scheduled.
        item.request.is_auto_capture()?;

        let payment_method = match &item.request.payment_method_data {
            Some(PaymentMethodData::Card(card_data)) => build_payment_card(card_data, None)?,
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    "Only card payments are supported".to_string(),
                    Default::default()
                )))
            }
        };

        let currency = item
            .request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide amount.currency on the PreAuthenticate request; Authipay's \
                         transactionAmount requires a currency."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            })?;

        let converter = FloatMajorUnitForConnector;
        let amount_major = converter
            .convert(item.request.amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let transaction_amount = TransactionAmount {
            total: amount_major,
            currency,
        };

        // UD-03: order.order_id := merchant_transaction_id := connector_request_reference_id.
        let merchant_transaction_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // UD-01: termURL := router_return_url. methodNotificationURL := webhook_url when
        // available else router_return_url; this request type carries no webhook_url field,
        // so it falls back to router_return_url as well.
        let return_url = item
            .request
            .router_return_url
            .as_ref()
            .map(|url| url.to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "router_return_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide return_url on the PreAuthenticate request so Authipay can \
                         route the ACS challenge result back to it (termURL)."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Authipay Secure3DAuthenticationRequest requires both termURL and \
                         methodNotificationURL; both are derived from return_url."
                            .to_string(),
                    ),
                },
            })?;

        let authentication_request = AuthipayAuthenticationRequest {
            authentication_type: AuthipayAuthenticationRequestType::Secure3DAuthenticationRequest,
            term_url: return_url.clone(),
            method_notification_url: return_url,
            message_category: AuthipayMessageCategory::PaymentAuthentication,
        };

        Ok(Self {
            request_type: AuthipayRequestType::PaymentCardPayerAuthTransaction,
            merchant_transaction_id: merchant_transaction_id.clone(),
            transaction_amount,
            order: OrderDetails {
                order_id: merchant_transaction_id,
            },
            payment_method,
            authentication_request,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AuthipayPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Build the ACS redirect form from authenticationResponse.params when the gateway
        // signals a challenge round-trip (transactionState/Result WAITING). Form field names
        // are verbatim wire names (spec: authenticationResponse params) — no re-casing.
        let redirect_form = item
            .response
            .authentication_response
            .as_ref()
            .and_then(|auth_response| auth_response.params.as_ref())
            .and_then(|params| {
                params.acs_url.as_ref().map(|acs_url| {
                    let mut form_fields = HashMap::new();
                    if let Some(c_req) = &params.c_req {
                        form_fields.insert("cReq".to_string(), c_req.clone());
                    }
                    if let Some(pa_req) = &params.payer_authentication_request {
                        form_fields
                            .insert("payerAuthenticationRequest".to_string(), pa_req.clone());
                    }
                    if let Some(session_data) = &params.session_data {
                        form_fields.insert("sessionData".to_string(), session_data.clone());
                    }
                    if let Some(merchant_data) = &params.merchant_data {
                        form_fields.insert("merchantData".to_string(), merchant_data.clone());
                    }
                    RedirectForm::Form {
                        endpoint: acs_url.clone(),
                        method: common_utils::request::Method::Post,
                        form_fields,
                    }
                })
            });

        let is_waiting = matches!(
            item.response.transaction_state,
            Some(AuthipayTransactionState::Waiting)
        ) || matches!(
            item.response.transaction_result,
            Some(AuthipayPaymentResult::Waiting)
        ) || matches!(
            item.response.transaction_status,
            Some(AuthipayPaymentStatus::Waiting)
        );

        // AuthenticationPending when the gateway is waiting on the ACS round-trip (or we
        // constructed the redirect form); AuthenticationSuccessful when the payer-auth
        // transaction completed with no challenge (frictionless).
        let status = if is_waiting || redirect_form.is_some() {
            AttemptStatus::AuthenticationPending
        } else {
            map_status(
                item.response.transaction_status.clone(),
                item.response.transaction_result.clone(),
                item.response.transaction_state.clone(),
                item.response.transaction_type.clone(),
            )
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(
                    item.response.ipg_transaction_id.clone(),
                )),
                authentication_data: None,
                redirection_data: redirect_form.map(Box::new),
                connector_response_reference_id: Some(item.response.ipg_transaction_id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== POST-AUTHENTICATE (payer-auth validate) =====
// PATCH /payments/{payer-auth-ipgTransactionId} with
// authenticationType=Secure3DAuthenticationUpdateRequest + acsResponse.cRes. Spec Leg A3:
// Charges NO — this leg validates the challenge and never moves money (TDS-01/INV-23: no
// amount and no capture-intent handling anywhere on this leg).

/// The browser's urlencoded ACS POST back to termURL carries the Base64 CRes.
#[derive(Debug, Deserialize)]
pub struct AuthipayAcsResponseForm {
    #[serde(rename = "cRes")]
    pub c_res: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayPostAuthenticateRequest {
    pub authentication_type: AuthipayAuthenticationUpdateType,
    pub acs_response: AuthipayAcsResponse,
}

#[derive(Debug, Clone, Serialize)]
pub enum AuthipayAuthenticationUpdateType {
    #[serde(rename = "Secure3DAuthenticationUpdateRequest")]
    Secure3DAuthenticationUpdateRequest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthipayAcsResponse {
    #[serde(rename = "cRes")]
    pub c_res: String,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    > for AuthipayPostAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // G-ThreeDS__Card-04: refuse when the browser round-trip did not deliver the CRes —
        // fail closed before any HTTP call.
        let params = item
            .request
            .redirect_response
            .as_ref()
            .and_then(|redirect_response| redirect_response.params.as_ref())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "redirect_response.params",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Forward the ACS termURL POST body to the PostAuthenticate request as \
                         redirection_response.params so the Base64 CRes can be validated."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Authipay Secure3DAuthenticationUpdateRequest requires \
                         acsResponse.cRes, which arrives as the urlencoded cRes field the ACS \
                         posted back to termURL."
                            .to_string(),
                    ),
                },
            })?;

        let acs_form = serde_urlencoded::from_str::<AuthipayAcsResponseForm>(params.peek())
            .change_context(IntegrationError::BodySerializationFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure redirection_response.params is the urlencoded ACS termURL \
                             form body containing a cRes field."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            })?;

        Ok(Self {
            authentication_type:
                AuthipayAuthenticationUpdateType::Secure3DAuthenticationUpdateRequest,
            acs_response: AuthipayAcsResponse {
                c_res: acs_form.c_res,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AuthipayPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AuthipayPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // The validate response is a completed TransactionResponse. The gateway has no
        // separate authentication-failure state: a rejected authentication surfaces as the
        // payer-auth transaction's DECLINED status/result/state; WAITING (or any other
        // in-flight state) means the round-trip is not finished yet.
        let is_declined = matches!(
            item.response.transaction_status,
            Some(
                AuthipayPaymentStatus::Declined
                    | AuthipayPaymentStatus::ValidationFailed
                    | AuthipayPaymentStatus::ProcessingFailed
            )
        ) || matches!(
            item.response.transaction_result,
            Some(
                AuthipayPaymentResult::Declined
                    | AuthipayPaymentResult::Failed
                    | AuthipayPaymentResult::Fraud
            )
        ) || matches!(
            item.response.transaction_state,
            Some(AuthipayTransactionState::Declined)
        );

        let is_waiting = matches!(
            item.response.transaction_state,
            Some(AuthipayTransactionState::Waiting | AuthipayTransactionState::Pending)
        ) || matches!(
            item.response.transaction_result,
            Some(AuthipayPaymentResult::Waiting)
        ) || matches!(
            item.response.transaction_status,
            Some(AuthipayPaymentStatus::Waiting)
        );

        let status = if is_declined {
            AttemptStatus::AuthenticationFailed
        } else if is_waiting {
            AttemptStatus::AuthenticationPending
        } else {
            AttemptStatus::AuthenticationSuccessful
        };

        let authentication_data = Some(AuthenticationData {
            trans_status: None,
            eci: item
                .response
                .secure3d_response
                .as_ref()
                .and_then(|secure3d| secure3d.eci.clone()),
            cavv: item
                .response
                .secure3d_response
                .as_ref()
                .and_then(|secure3d| secure3d.authentication_value.clone()),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: item
                .response
                .secure3d_response
                .as_ref()
                .and_then(|secure3d| secure3d.directory_server_transaction_id.clone()),
            message_version: None,
            ds_trans_id: None,
            acs_transaction_id: None,
            // Payer-auth ipgTransactionId; downstream Authorize correlates via this.
            transaction_id: Some(item.response.ipg_transaction_id.clone()),
            network_params: None,
            exemption_indicator: None,
            created_at: None,
            challenge_code: None,
            challenge_cancel: None,
            challenge_code_reason: None,
            message_extension: None,
            authentication_type: None,
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id: Some(item.response.ipg_transaction_id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== TYPE ALIASES FOR MACRO COMPATIBILITY =====
// Each flow needs its own response type for the macro system
// Even though they all use the same underlying AuthipayPaymentsResponse struct
pub type AuthipayAuthorizeResponse = AuthipayPaymentsResponse;
pub type AuthipayPreAuthenticateResponse = AuthipayPaymentsResponse;
pub type AuthipayPostAuthenticateResponse = AuthipayPaymentsResponse;
pub type AuthipaySyncResponse = AuthipayPaymentsResponse;
pub type AuthipayVoidResponse = AuthipayPaymentsResponse;
pub type AuthipayVoidPCResponse = AuthipayPaymentsResponse;
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AuthipayRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthipayVoidPCRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
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
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthipayPreAuthenticateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
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
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AuthipayPostAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: AuthipayRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&item.router_data)
    }
}

// ===== WEBHOOK (OMS server-to-server notification) STRUCTURES =====
// Spec `### IncomingWebhook`: the notification is a form-urlencoded POST
// (application/x-www-form-urlencoded), verified with an HMAC whose algorithm is
// named by the notification itself (`hash_algorithm`) over the pipe-joined
// fields `chargetotal|currency|txndatetime|storename|approval_code`, compared
// against the in-body `notification_hash` in constant time. One shared struct
// is decoded by verify_webhook_source / get_event_type /
// get_webhook_event_reference / process_payment_webhook /
// process_refund_webhook alike (TH-05).

/// `status` field of the OMS notification (spec payload table).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuthipayWebhookStatus {
    Approved,
    Declined,
    Failed,
    Waiting,
    #[serde(other)]
    Unknown,
}

/// `hash_algorithm` field of the OMS notification. The algorithm comes from
/// the notification itself (spec verification step 2), never from config.
/// G-IncomingWebhook-02: an unresolvable value decodes to `Unknown` and
/// verification fails closed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthipayHashAlgorithm {
    /// SHA-256, both documented spellings
    #[serde(rename = "SHA256", alias = "HMACSHA256")]
    Sha256,
    /// SHA-1 legacy
    #[serde(rename = "SHA1", alias = "HMACSHA1")]
    Sha1,
    /// SHA-384 — recognised; common_utils has no HMAC-SHA384, so verification
    /// fails closed (fail-closed, deviation recorded in the run decisions)
    #[serde(rename = "SHA384", alias = "HMACSHA384")]
    Sha384,
    /// SHA-512
    #[serde(rename = "SHA512", alias = "HMACSHA512")]
    Sha512,
    #[serde(other)]
    Unknown,
}

impl AuthipayHashAlgorithm {
    /// Compute the HMAC over `message` keyed by `key` with this algorithm.
    /// Returns `None` when the algorithm is not supportable (Unknown, or
    /// SHA-384 which the shared crypto crate does not expose) — callers treat
    /// `None` as verification failure (fail closed).
    pub fn sign(&self, key: &[u8], message: &[u8]) -> Option<Vec<u8>> {
        match self {
            Self::Sha256 => crypto::HmacSha256.sign_message(key, message).ok(),
            Self::Sha1 => crypto::HmacSha1.sign_message(key, message).ok(),
            Self::Sha512 => crypto::HmacSha512.sign_message(key, message).ok(),
            Self::Sha384 | Self::Unknown => None,
        }
    }

    /// Digest length in bytes for the declared algorithm; 0 when unknown.
    pub fn digest_len(&self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
            Self::Unknown => 0,
        }
    }
}

/// Form-urlencoded OMS transaction notification (spec `### IncomingWebhook`
/// payload table). Field names follow the wire spelling exactly — this struct
/// is decoded with serde_urlencoded, where field names are case-sensitive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthipayWebhookBody {
    /// IPG transaction identifier — the primary idempotency key.
    #[serde(rename = "ipgTransactionId", default)]
    pub ipg_transaction_id: Option<String>,
    /// Merchant order ID echoed back when it was supplied on the request
    /// (the connector sets `order.orderId := merchant_transaction_id`).
    #[serde(default)]
    pub oid: Option<String>,
    /// Merchant transaction ID echo (present on REST-initiated transactions).
    #[serde(rename = "merchantTransactionId", default)]
    pub merchant_transaction_id: Option<String>,
    /// Processed amount as a major-units decimal string (TH-02: it is an
    /// input to the signature check, kept as the exact wire string; no
    /// arithmetic is ever performed on it in this flow).
    #[serde(rename = "chargetotal")]
    pub charge_total: String,
    /// Currency as the wire sends it (ISO numeric on Connect e.g. "978";
    /// alpha on REST-originated notifications e.g. "USD").
    #[serde(rename = "currency")]
    pub currency: String,
    /// Format `yyyy:MM:dd-HH:mm:ss`.
    #[serde(rename = "txndatetime")]
    pub txn_datetime: String,
    /// Store ID — selects the shared secret on the merchant side.
    pub storename: String,
    /// Approval/result code (`Y:...` approved, `N:...` not approved,
    /// `?:...` waiting).
    pub approval_code: String,
    pub status: AuthipayWebhookStatus,
    /// Payment vs refund discriminator (spec: `txntype=RETURN` marks a
    /// return-transaction notification).
    pub txntype: Option<AuthipayTransactionType>,
    /// Hash algorithm that `notification_hash` was computed with.
    pub hash_algorithm: AuthipayHashAlgorithm,
    /// Integrity value (HMAC over the pipe-joined fields, keyed by the store
    /// shared secret). Kept secret — it is a credential-equivalent verifier.
    pub notification_hash: Secret<String>,
    /// Failure reason on declined/failed outcomes.
    #[serde(default)]
    pub fail_reason: Option<String>,
    /// Backend response code.
    #[serde(default)]
    pub processor_response_code: Option<String>,
    /// Transaction identification number.
    #[serde(default)]
    pub tdate: Option<String>,
    /// Reference number (order-level refund reference on `txntype=RETURN`).
    #[serde(default)]
    pub refnumber: Option<String>,
    /// Present on browser redirect responses; tolerated here so one parser
    /// covers both the redirect and the notification shapes.
    #[serde(default)]
    pub response_hash: Option<String>,
}

impl AuthipayWebhookBody {
    /// The documented hash preimage (spec verification step 1; TH-14 carries
    /// this in code): `chargetotal|currency|txndatetime|storename|approval_code`
    /// — pipe-joined, no spaces, in exactly this order, using raw wire strings.
    pub fn hash_preimage(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.charge_total, self.currency, self.txn_datetime, self.storename, self.approval_code
        )
    }

    /// Whether this notification reports a refund (spec: `txntype=RETURN`)
    /// carrying its own refund reference (`refnumber` present).
    pub fn is_refund_notification(&self) -> bool {
        self.txntype == Some(AuthipayTransactionType::Return) && self.refnumber.is_some()
    }

    /// Merchant-side reference best echo: `merchantTransactionId` first, else
    /// the order id (plan TH-16: same id chain as the payment flow).
    pub fn merchant_reference(&self) -> Option<String> {
        self.merchant_transaction_id
            .clone()
            .or_else(|| self.oid.clone())
    }
}

/// Decode-only amount conversion for the OMS notification amount (plan §4
/// `amount`: FloatMajorUnit through `FloatMajorUnitForConnector`; no
/// arithmetic is performed — the wire major-units decimal string is parsed
/// and converted straight to minor units).
pub fn authipay_webhook_minor_amount(
    charge_total: &str,
    currency: common_enums::Currency,
) -> Result<common_utils::types::MinorUnit, error_stack::Report<common_utils::errors::ParsingError>>
{
    let major: f64 = charge_total.trim().parse().map_err(|_| {
        error_stack::report!(common_utils::errors::ParsingError::StructParseFailure(
            "chargetotal is not a decimal number"
        ))
    })?;
    FloatMajorUnitForConnector.convert_back(FloatMajorUnit(major), currency)
}
