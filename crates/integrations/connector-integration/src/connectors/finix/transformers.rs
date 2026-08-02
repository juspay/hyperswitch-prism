use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, CountryAlpha2, CountryAlpha3, Currency, RefundStatus};
use common_utils::{
    consts,
    pii::{Email, SecretSerdeValue},
    types::MinorUnit,
};
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
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorAuthType, ConnectorResponseData,
        ConnectorSpecificConfig, ErrorResponse, FlowStatus,
    },
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

pub(crate) fn convert_to_additional_payment_method_connector_response(
    finix_payments_response: &FinixAuthorizeResponse,
) -> Option<AdditionalPaymentMethodConnectorResponse> {
    let address_verification_check = finix_payments_response.address_verification.as_ref();
    let network_details = finix_payments_response.network_details.as_ref();

    if address_verification_check.is_none() && network_details.is_none() {
        return None;
    }

    let mut payment_checks = serde_json::Map::new();
    if let Some(code) = address_verification_check {
        payment_checks.insert("avs_result".to_string(), serde_json::json!(code));
    }

    let card_network = network_details.and_then(|details| details.brand.clone());
    let auth_code = network_details.and_then(|details| details.authorization_code.clone());

    Some(AdditionalPaymentMethodConnectorResponse::Card {
        authentication_data: None,
        payment_checks: (!payment_checks.is_empty())
            .then(|| serde_json::Value::Object(payment_checks)),
        card_network,
        domestic_network: None,
        auth_code,
    })
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
    pub country: Option<CountryAlpha3>,
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
    // HS emits these as explicit `null` (no skip_serializing_if) for card payment instruments;
    // mirror that so the shadow request bodies match. Finix derives card_brand/card_type from the
    // PAN, so they stay None for cards; merchant_identity/third_party_token carry values only for
    // wallet (Google/Apple Pay) tokenization.
    pub card_brand: Option<String>,
    pub card_type: Option<String>,
    pub additional_data: Option<HashMap<String, String>>,
    pub merchant_identity: Option<Secret<String>>,
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

// =============================================================================
// APPLE PAY `third_party_token` PAYLOAD
// =============================================================================
// Finix expects the raw PassKit payment token, re-serialized as a JSON *string*
// in `third_party_token`. Field naming inside the Apple Pay token is camelCase
// (`paymentData`, `paymentMethod`, `publicKeyHash`, `ephemeralPublicKey`,
// `transactionId`, `transactionIdentifier`) while the enclosing Finix Payment
// Instrument body stays snake_case.
// See https://docs.finix.com/guides/online-payments/digital-wallets/apple-pay/apple-pay-on-ios

#[derive(Debug, Serialize, Deserialize)]
pub struct FinixApplePayPaymentToken {
    pub token: FinixApplePayToken,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinixApplePayToken {
    pub payment_data: FinixApplePayEncryptedData,
    pub payment_method: FinixApplePayPaymentMethod,
    pub transaction_identifier: String,
}

/// The `paymentData` object of an Apple Pay token. This is also the shape we
/// deserialize the decoded PassKit token into, so every field is the cryptogram
/// material itself and stays `Secret` from construction.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinixApplePayEncryptedData {
    pub data: Secret<String>,
    pub signature: Secret<String>,
    pub header: FinixApplePayHeader,
    pub version: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinixApplePayHeader {
    pub public_key_hash: Secret<String>,
    pub ephemeral_public_key: Secret<String>,
    pub transaction_id: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinixApplePayPaymentMethod {
    pub display_name: Secret<String>,
    pub network: Secret<String>,
    #[serde(rename = "type")]
    pub method_type: Secret<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinixPaymentThreeDSecure {
    pub cardholder_authentication: Secret<String>,
    pub electronic_commerce_indicator: String,
    pub transaction_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FinixPaymentsRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub source: String,
    pub merchant: String,
    pub tags: Option<serde_json::Value>,
    pub fraud_session_id: Option<String>,
    #[serde(rename = "3d_secure_authentication")]
    pub three_d_secure_authentication: Option<FinixPaymentThreeDSecure>,
    pub idempotency_id: Option<String>,
    pub statement_descriptor: Option<String>,
}

pub type FinixAuthorizeRequest = FinixPaymentsRequest;

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
    pub address_verification: Option<String>,
    pub source: Option<Secret<String>>,
    pub network_details: Option<FinixNetworkDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FinixNetworkDetails {
    pub brand: Option<String>,
    pub authorization_code: Option<String>,
}

// RepeatPayment (MIT) uses the same payment-create request shape as Authorize,
// matching HS's `FinixPaymentsRequest` serialization for both flows.
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

/// Finix resource `state`. This is the single deserialization source of truth for the
/// Finix `state` field: authorizations, transfers, reversals AND every incoming-webhook
/// payload read it through this one enum, so the flows cannot drift apart.
///
/// `RETURNED` is documented on `GET /transfers/{id}` (ACH-style return of an already
/// settled transfer) and is modelled explicitly rather than being folded into `Unknown`;
/// `#[serde(other)]` still keeps any future value decodable instead of failing the whole
/// response/webhook body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixPaymentStatus {
    Succeeded,
    Failed,
    Pending,
    Canceled,
    Returned,
    #[serde(other)]
    Unknown,
}

/// Which Finix resource a `state` was read from. Mirrors HS `FinixFlow`: it decides
/// whether a `SUCCEEDED` is an authorization hold (`AU…`) or a captured charge (`TR…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinixFlow {
    Auth,
    Transfer,
    Capture,
}

impl From<&FinixId> for FinixFlow {
    fn from(id: &FinixId) -> Self {
        match id {
            FinixId::Auth(_) => Self::Auth,
            FinixId::Transfer(_) => Self::Transfer,
        }
    }
}

/// Canonical Finix `state` -> `AttemptStatus` mapping. Authorize, PSync, Capture, Void,
/// RepeatPayment and the incoming-webhook path all route through this one function so a
/// webhook can never report a different status than a sync for the same connector state.
///
/// Deliberate departures from the hyperswitch (Direct) `get_attempt_status`, both applied
/// here once rather than per flow:
/// * `UNKNOWN` is indeterminate, so it stays `Pending` (re-sync decides) instead of
///   becoming a terminal `Failure`/`VoidFailed`.
/// * `CANCELED` means the resource was voided, so it maps to `Voided` rather than
///   `AuthorizationFailed`/`Failure`.
/// * A `PENDING` void stays `Pending` rather than being reported as `Voided` early.
pub fn get_finix_attempt_status(
    state: &FinixPaymentStatus,
    flow: FinixFlow,
    is_void: Option<bool>,
) -> AttemptStatus {
    if is_void == Some(true) {
        return match state {
            FinixPaymentStatus::Succeeded | FinixPaymentStatus::Canceled => AttemptStatus::Voided,
            FinixPaymentStatus::Failed | FinixPaymentStatus::Returned => AttemptStatus::VoidFailed,
            FinixPaymentStatus::Pending | FinixPaymentStatus::Unknown => AttemptStatus::Pending,
        };
    }

    match state {
        FinixPaymentStatus::Canceled => AttemptStatus::Voided,
        FinixPaymentStatus::Unknown => AttemptStatus::Pending,
        FinixPaymentStatus::Pending => match flow {
            FinixFlow::Auth => AttemptStatus::AuthenticationPending,
            FinixFlow::Transfer | FinixFlow::Capture => AttemptStatus::Pending,
        },
        FinixPaymentStatus::Succeeded => match flow {
            FinixFlow::Auth => AttemptStatus::Authorized,
            FinixFlow::Transfer => AttemptStatus::Charged,
            // A SUCCEEDED capture only confirms the authorization update; the funds
            // movement (transfer) settles asynchronously, so the attempt stays Pending
            // until a PSync on the transfer id confirms the charge.
            FinixFlow::Capture => AttemptStatus::Pending,
        },
        FinixPaymentStatus::Failed | FinixPaymentStatus::Returned => match flow {
            FinixFlow::Auth => AttemptStatus::AuthorizationFailed,
            FinixFlow::Transfer | FinixFlow::Capture => AttemptStatus::Failure,
        },
    }
}

impl From<&FinixPaymentStatus> for AttemptStatus {
    fn from(status: &FinixPaymentStatus) -> Self {
        get_finix_attempt_status(status, FinixFlow::Transfer, None)
    }
}

/// Canonical Finix `state` -> `RefundStatus` mapping, shared by Refund, RSync and the
/// REVERSAL-transfer webhook path.
impl From<&FinixPaymentStatus> for RefundStatus {
    fn from(status: &FinixPaymentStatus) -> Self {
        match status {
            FinixPaymentStatus::Succeeded => Self::Success,
            FinixPaymentStatus::Failed
            | FinixPaymentStatus::Canceled
            | FinixPaymentStatus::Returned => Self::Failure,
            FinixPaymentStatus::Pending => Self::Pending,
            // Indeterminate — keep the refund open so RSync retries rather than
            // reporting a terminal failure.
            FinixPaymentStatus::Unknown => Self::Pending,
        }
    }
}

fn get_finix_three_d_secure(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> Result<Option<FinixPaymentThreeDSecure>, error_stack::Report<IntegrationError>> {
    authentication_data
        .map(|auth_data| {
            Ok(FinixPaymentThreeDSecure {
                cardholder_authentication: auth_data.cavv.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "authentication_data.cavv",
                        context: IntegrationErrorContext::default(),
                    },
                )?,
                electronic_commerce_indicator: auth_data.eci.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "authentication_data.eci",
                        context: IntegrationErrorContext::default(),
                    },
                )?,
                transaction_id: auth_data.threeds_server_transaction_id.clone(),
            })
        })
        .transpose()
}

fn get_finix_fraud_session_id(connector_feature_data: Option<&SecretSerdeValue>) -> Option<String> {
    connector_feature_data.and_then(|feature_data| {
        feature_data
            .peek()
            .get("finix_additional_details")
            .and_then(|details| details.get("fraud_session_id"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
    })
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

        let statement_descriptor = item
            .router_data
            .request
            .billing_descriptor
            .clone()
            .and_then(|billing_descriptor| billing_descriptor.statement_descriptor);
        let three_d_secure_authentication =
            get_finix_three_d_secure(router_data.request.authentication_data.as_ref())?;
        let fraud_session_id =
            get_finix_fraud_session_id(router_data.request.connector_feature_data.as_ref());

        Ok(Self {
            amount: router_data.request.amount,
            currency: router_data.request.currency,
            source,
            merchant: merchant_id,
            tags: None,
            fraud_session_id,
            three_d_secure_authentication,
            idempotency_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            statement_descriptor,
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

        let connector_response_data =
            convert_to_additional_payment_method_connector_response(&item.response)
                .map(ConnectorResponseData::with_additional_payment_method_data);

        // Determine status based on ID type via the canonical mapping:
        // - Transfer (TR*): Succeeded -> Charged, failure -> Failure
        // - Authorization (AU*): Succeeded -> Authorized, failure -> AuthorizationFailed
        let finix_id = FinixId::from(response.id.clone());
        let status = get_finix_attempt_status(&response.state, FinixFlow::from(&finix_id), None);

        // Finix reports declines as an HTTP-2xx body with `state: FAILED` (soft decline).
        // Hyperswitch's Direct path maps any `state.is_failure()` (FAILED/CANCELED/UNKNOWN)
        // to `response: Err(ErrorResponse)` carrying the connector failure code/message, so
        // mirror that here for shadow parity (was previously `Ok` with status=Failure).
        let is_failure = domain_types::utils::is_payment_failure(status);

        let flow_response = if is_failure {
            Err(ErrorResponse {
                code: response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: response
                    .failure_message
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
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

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: response.source.clone().map(|id| id.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            })
        };

        // `connector_response` (AVS / network details) is set on BOTH success and failure
        // paths, matching Direct which always populates it from the Finix response body.
        Ok(Self {
            response: flow_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response_data,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
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
    pub messages: Option<Vec<String>>,
    pub transfer: Option<String>,
    pub is_void: Option<bool>,
    // Stored Payment Instrument backing this transaction (PI...). Hyperswitch maps this
    // into `mandate_reference.connector_mandate_id` so the sync response stays in parity.
    pub source: Option<Secret<String>>,
    // AVS / network details echoed by Finix; surfaced into `connector_response` to mirror HS.
    pub address_verification: Option<String>,
    pub network_details: Option<FinixNetworkDetails>,
}

// Build the additional-payment-method `connector_response` from Finix AVS / network
// details, mirroring Hyperswitch's `convert_to_additional_payment_method_connector_response`.
fn build_finix_connector_response(
    response: &FinixPaymentsResponse,
) -> Option<ConnectorResponseData> {
    if response.address_verification.is_none() && response.network_details.is_none() {
        return None;
    }

    let mut payment_checks = serde_json::Map::new();
    if let Some(code) = response.address_verification.as_ref() {
        payment_checks.insert("avs_result".to_string(), serde_json::json!(code));
    }

    let card_network = response
        .network_details
        .as_ref()
        .and_then(|details| details.brand.clone());
    let auth_code = response
        .network_details
        .as_ref()
        .and_then(|details| details.authorization_code.clone());

    let additional = AdditionalPaymentMethodConnectorResponse::Card {
        authentication_data: None,
        payment_checks: (!payment_checks.is_empty())
            .then(|| serde_json::Value::Object(payment_checks)),
        card_network,
        domestic_network: None,
        auth_code,
    };

    Some(ConnectorResponseData::with_additional_payment_method_data(
        additional,
    ))
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
        // through the canonical mapping shared with Authorize, Capture and webhooks.
        let finix_id = FinixId::from(response.id.clone());
        let status = get_finix_attempt_status(
            &response.state,
            FinixFlow::from(&finix_id),
            response.is_void,
        );

        // For transfers (TR...), use the transfer ID directly
        // For authorizations (AU...), use transfer ID if available for refunds
        let connector_transaction_id = response
            .transfer
            .clone()
            .unwrap_or_else(|| response.id.clone());

        // Surface the stored Payment Instrument (`source`) as the mandate reference and
        // propagate AVS / network details via `connector_response`.
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: response.source.as_ref().map(|s| s.clone().expose()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: None,
        }));
        let connector_response = build_finix_connector_response(response);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
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

        // A SUCCEEDED capture only confirms the authorization update; the funds
        // movement (transfer) settles asynchronously, so the attempt stays Pending
        // until a PSync on the transfer id confirms the charge (canonical mapping,
        // `FinixFlow::Capture`).
        let status =
            get_finix_attempt_status(&response.state, FinixFlow::Capture, response.is_void);

        let connector_response = build_finix_connector_response(&response);

        let is_failure = domain_types::utils::is_payment_failure(status);

        let flow_response = if is_failure {
            Err(ErrorResponse {
                code: response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: response
                    .failure_message
                    .clone()
                    .or_else(|| response.messages.clone().map(|msg| msg.join(",")))
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                // The transfer id (TR*) tracks the actual funds movement; fall back
                // to the authorization id when Finix has not created a transfer yet.
                resource_id: ResponseId::ConnectorTransactionId(
                    response
                        .transfer
                        .clone()
                        .unwrap_or_else(|| response.id.clone()),
                ),
                redirection_data: None,
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: response.source.clone().map(|id| id.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            })
        };

        Ok(Self {
            response: flow_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
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
        let refund_status = RefundStatus::from(&response.state);
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
        // Void-specific status mapping — the canonical mapping's `is_void` branch.
        let status = get_finix_attempt_status(&response.state, FinixFlow::Auth, Some(true));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
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
        let status = RefundStatus::from(&response.state);

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

fn get_billing_address_as_finix_address(
    resource_common_data: &PaymentFlowData,
) -> Option<FinixAddress> {
    resource_common_data
        .get_optional_billing()
        .and_then(|address| {
            let billing = address.address.as_ref();
            billing.map(|billing_address| FinixAddress {
                line1: billing_address.line1.clone(),
                line2: billing_address.line2.clone(),
                city: billing_address.city.clone().map(|s| s.expose()),
                region: billing_address.to_state_code_as_optional().ok().flatten(),
                postal_code: billing_address.get_optional_zip(),
                country: billing_address
                    .get_optional_country()
                    .map(CountryAlpha2::from_alpha2_to_alpha3),
            })
        })
}

/// Finix's `POST /payment_instruments` requires a **four-digit** `expiration_year`
/// (tech spec §SR-2) and hyperswitch's Direct gateway sends
/// `get_expiry_year_as_4_digit_i32`. `card_exp_year` may legitimately arrive as `YY`,
/// so it is expanded through the shared `get_expiry_year_4_digit()` helper before being
/// parsed — sending the raw two-digit value would store the mandate with an expiry in
/// year 30 instead of 2030 and make every later MIT charge fail at the network.
///
/// Shared by the Tokenize (`PaymentMethodToken`) and `SetupMandate` card arms so the two
/// paths cannot drift.
fn get_finix_expiration_year<T: PaymentMethodDataTypes>(
    card: &domain_types::payment_method_data::Card<T>,
) -> Result<Secret<i32>, error_stack::Report<IntegrationError>> {
    card.get_expiry_year_4_digit()
        .peek()
        .parse::<i32>()
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "payment_method_data.card.card_exp_year",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Finix `expiration_year` must be a four-digit year; expected card_exp_year \
                     in YY or YYYY form"
                        .to_string(),
                ),
                suggested_action: Some(
                    "Send card_exp_year as two or four decimal digits".to_string(),
                ),
                doc_url: Some("https://docs.finix.com/api/payment-instruments".to_string()),
            },
        })
        .map(Secret::new)
        .attach_printable(
            "failed to build the Finix payment-instrument `expiration_year` from card_exp_year",
        )
}

/// Builds the Finix `POST /payment_instruments` body for a Google Pay device token.
///
/// Shared by the Tokenize (`PaymentMethodToken`) and `SetupMandate` flows — both create the
/// same Google Pay Payment Instrument — mirroring `build_apple_pay_instrument_request` so the
/// two paths cannot drift. `name` and `address` are populated exactly as hyperswitch's
/// `get_token_request` does for wallets: the billing full name (wallets carry no cardholder
/// name of their own) and the billing address.
fn build_google_pay_instrument_request(
    google_pay_data: &domain_types::payment_method_data::GooglePayWalletData,
    identity: String,
    merchant_identity_id: Secret<String>,
    name: Option<Secret<String>>,
    address: Option<FinixAddress>,
    tags: Option<FinixTags>,
) -> Result<FinixCreatePaymentInstrumentRequest, error_stack::Report<IntegrationError>> {
    let encrypted_token = google_pay_data
        .tokenization_data
        .get_encrypted_google_pay_payment_data_mandatory()
        .change_context(IntegrationError::InvalidWalletToken {
            wallet_name: "Google Pay".to_string(),
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Google Pay token must carry the encrypted \
                     `tokenizationData.token` string (signature, protocolVersion, signedMessage \
                     and intermediateSigningKey); decrypted Google Pay data is not accepted by \
                     Finix"
                        .to_string(),
                ),
                suggested_action: Some(
                    "Forward the unmodified Google Pay \
                     `paymentMethodData.tokenizationData.token`"
                        .to_string(),
                ),
                doc_url: Some(
                    "https://docs.finix.com/guides/online-payments/digital-wallets".to_string(),
                ),
            },
        })
        .attach_printable(
            "Finix requires the encrypted Google Pay token as the `third_party_token` of the \
             GOOGLE_PAY payment instrument",
        )?;

    Ok(FinixCreatePaymentInstrumentRequest {
        instrument_type: FinixPaymentInstrumentType::GooglePay,
        name,
        number: None,
        security_code: None,
        expiration_month: None,
        expiration_year: None,
        identity,
        tags,
        address,
        card_brand: None,
        card_type: None,
        additional_data: None,
        merchant_identity: Some(merchant_identity_id),
        third_party_token: Some(Secret::new(encrypted_token.token.clone())),
        account_number: None,
        bank_code: None,
        account_type: None,
    })
}

/// Builds the Finix `POST /payment_instruments` body for an Apple Pay device token.
///
/// Shared by the Tokenize (`PaymentMethodToken`) and `SetupMandate` flows — both create
/// the same Apple Pay Payment Instrument, so the token reshaping lives here once and the
/// two paths cannot drift.
///
/// Finix wants the PassKit token re-serialized as a JSON *string* under
/// `third_party_token`, with `merchant_identity` set to the Identity registered for
/// Apple Pay. See
/// https://docs.finix.com/guides/online-payments/digital-wallets/apple-pay/apple-pay-on-ios
fn build_apple_pay_instrument_request(
    apple_pay_data: &domain_types::payment_method_data::ApplePayWalletData,
    identity: String,
    merchant_identity_id: Secret<String>,
    name: Option<Secret<String>>,
    address: Option<FinixAddress>,
    tags: Option<FinixTags>,
) -> Result<FinixCreatePaymentInstrumentRequest, error_stack::Report<IntegrationError>> {
    // Base64-decodes the PassKit token and keeps it inside a `Secret`; the util's own
    // error already names Apple Pay.
    let decoded_token = apple_pay_data
        .get_applepay_decoded_payment_data()
        .attach_printable(
            "Apple Pay `payment_data` must carry the base64-encoded PassKit payment token; \
             pre-decrypted Apple Pay data is not accepted by Finix",
        )?;

    let apple_pay_payment_data: FinixApplePayEncryptedData = serde_json::from_str(
        decoded_token.peek(),
    )
    .change_context(IntegrationError::InvalidWalletToken {
        wallet_name: "Apple Pay".to_string(),
        context: IntegrationErrorContext {
            additional_context: Some(
                "Apple Pay token must contain data, signature, version and a header with \
                 publicKeyHash, ephemeralPublicKey and transactionId"
                    .to_string(),
            ),
            suggested_action: Some(
                "Forward the unmodified PassKit `paymentData` object, base64-encoded".to_string(),
            ),
            doc_url: Some(
                "https://docs.finix.com/guides/online-payments/digital-wallets/apple-pay/apple-pay-on-ios"
                    .to_string(),
            ),
        },
    })?;

    let finix_token = FinixApplePayPaymentToken {
        token: FinixApplePayToken {
            payment_data: apple_pay_payment_data,
            payment_method: FinixApplePayPaymentMethod {
                display_name: Secret::new(apple_pay_data.payment_method.display_name.clone()),
                network: Secret::new(apple_pay_data.payment_method.network.clone()),
                method_type: Secret::new(apple_pay_data.payment_method.pm_type.clone()),
            },
            transaction_identifier: apple_pay_data.transaction_identifier.clone(),
        },
    };

    let third_party_token = serde_json::to_string(&finix_token).change_context(
        IntegrationError::InvalidWalletToken {
            wallet_name: "Apple Pay".to_string(),
            context: IntegrationErrorContext {
                additional_context: Some(
                    "failed to serialize the Apple Pay token (paymentData, paymentMethod, \
                     transactionIdentifier) into Finix's `third_party_token` string"
                        .to_string(),
                ),
                ..Default::default()
            },
        },
    )?;

    Ok(FinixCreatePaymentInstrumentRequest {
        instrument_type: FinixPaymentInstrumentType::ApplePay,
        name,
        number: None,
        security_code: None,
        expiration_month: None,
        expiration_year: None,
        identity,
        tags,
        address,
        card_brand: None,
        card_type: None,
        additional_data: None,
        merchant_identity: Some(merchant_identity_id),
        third_party_token: Some(Secret::new(third_party_token)),
        account_number: None,
        bank_code: None,
        account_type: None,
    })
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
        let personal_address =
            get_billing_address_as_finix_address(&item.router_data.resource_common_data);
        let customer_data = &item.router_data.request;

        let entity = FinixIdentityEntity {
            phone: customer_data.phone.clone(),
            first_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_first_name(),
            last_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_last_name(),
            email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            personal_address,
        };

        Ok(Self {
            entity,
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
                status_code: item.http_code,
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
                expiration_year: Some(get_finix_expiration_year(card)?),
                identity: customer_id,
                tags: None,
                address: get_billing_address_as_finix_address(
                    &item.router_data.resource_common_data,
                ),
                card_brand: None,
                card_type: None,
                additional_data: None,
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
                            card_brand: None,
                            card_type: None,
                            additional_data: None,
                            merchant_identity: None,
                            third_party_token: None,
                            account_number: Some(account_number.clone()),
                            bank_code: Some(routing_number.clone()),
                            account_type,
                        })
                    }
                    _ => Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "Only ACH Bank Debit is supported".to_string(),
                        connector: "Finix",
                        context: Default::default(),
                    })),
                }
            }
            PaymentMethodData::Wallet(wallet_data) => {
                match wallet_data {
                    domain_types::payment_method_data::WalletData::GooglePay(google_pay_data) => {
                        // Finix requires the Google Pay-registered Identity (merchant_identity)
                        // alongside the buyer identity for GOOGLE_PAY instruments.
                        let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;

                        build_google_pay_instrument_request(
                            google_pay_data,
                            customer_id,
                            auth.merchant_identity_id,
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name(),
                            get_billing_address_as_finix_address(
                                &item.router_data.resource_common_data,
                            ),
                            None,
                        )
                    }
                    domain_types::payment_method_data::WalletData::ApplePay(apple_pay_data) => {
                        // Finix requires the Apple Pay-registered Identity (merchant_identity)
                        // alongside the buyer identity for APPLE_PAY instruments.
                        let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;

                        build_apple_pay_instrument_request(
                            apple_pay_data,
                            customer_id,
                            auth.merchant_identity_id,
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name(),
                            get_billing_address_as_finix_address(
                                &item.router_data.resource_common_data,
                            ),
                            None,
                        )
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        "Only Google Pay and Apple Pay wallet tokenization are supported".into(),
                        Default::default(),
                    )
                    .into()),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card, bank debit, Google Pay, and Apple Pay tokenization are supported"
                    .into(),
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
                _ => Err(instrument_not_usable_error(&response, item.http_code)),
            },
            ..item.router_data
        })
    }
}

// Maps a 2xx `POST /payment_instruments` body that is not a usable instrument into a
// failure. Tech spec §SR-5 lists these as two distinct outcomes and they must not collapse
// into the same opaque `NO_ERROR_CODE` / `NO_ERROR_MESSAGE` pair:
//   * `enabled: false` -> Finix stored the instrument but it can never be charged; propagate
//     `disabled_code` / `disabled_message` verbatim as the connector's own code/message.
//   * `id` absent      -> malformed response; no Payment Instrument (PI...) reference can be
//     extracted at all, so say exactly that instead of an empty message.
// Shared by the Tokenize and SetupMandate response transformers.
fn instrument_not_usable_error(
    response: &FinixInstrumentResponse,
    status_code: u16,
) -> ErrorResponse {
    let (code, message, reason) = match response.id.as_ref() {
        // `id` present => the instrument exists but Finix flagged it `enabled: false`.
        Some(instrument_id) => {
            let message = response.disabled_message.clone().unwrap_or_else(|| {
                format!(
                    "Finix returned payment instrument {instrument_id} with `enabled: false`; \
                     it cannot be charged"
                )
            });
            (
                response
                    .disabled_code
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message.clone(),
                Some(message),
            )
        }
        // No `id` => nothing to store as a token or mandate reference.
        None => (
            consts::NO_ERROR_CODE.to_string(),
            "Finix accepted the request but returned no payment instrument id".to_string(),
            Some(
                "POST /payment_instruments returned a 2xx body without an `id`, so no Payment \
                 Instrument (PI...) reference could be extracted for the token / mandate"
                    .to_string(),
            ),
        ),
    };

    ErrorResponse {
        code,
        message,
        reason,
        status_code,
        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
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
                expiration_year: Some(get_finix_expiration_year(card)?),
                identity: customer_id,
                tags,
                address: get_billing_address_as_finix_address(
                    &item.router_data.resource_common_data,
                ),
                card_brand: None,
                card_type: None,
                additional_data: None,
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
                            card_brand: None,
                            card_type: None,
                            additional_data: None,
                            merchant_identity: None,
                            third_party_token: None,
                            account_number: Some(account_number.clone()),
                            bank_code: Some(routing_number.clone()),
                            account_type,
                        })
                    }
                    _ => Err(error_stack::report!(IntegrationError::NotSupported {
                        message: "Only ACH Bank Debit is supported for SetupMandate".to_string(),
                        connector: "Finix",
                        context: Default::default(),
                    })),
                }
            }
            PaymentMethodData::Wallet(wallet_data) => {
                match wallet_data {
                    domain_types::payment_method_data::WalletData::GooglePay(google_pay_data) => {
                        // Same Google Pay Payment Instrument as the Tokenize flow — built by the
                        // shared helper so the two paths cannot drift.
                        let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;

                        build_google_pay_instrument_request(
                            google_pay_data,
                            customer_id,
                            auth.merchant_identity_id,
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name(),
                            get_billing_address_as_finix_address(
                                &item.router_data.resource_common_data,
                            ),
                            tags,
                        )
                    }
                    domain_types::payment_method_data::WalletData::ApplePay(apple_pay_data) => {
                        // Same Apple Pay Payment Instrument as the Tokenize flow — built by the
                        // shared helper so the two paths cannot drift.
                        let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;

                        build_apple_pay_instrument_request(
                            apple_pay_data,
                            customer_id,
                            auth.merchant_identity_id,
                            item.router_data
                                .resource_common_data
                                .get_optional_billing_full_name(),
                            get_billing_address_as_finix_address(
                                &item.router_data.resource_common_data,
                            ),
                            tags,
                        )
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        "Only Google Pay and Apple Pay wallets are supported for SetupMandate"
                            .into(),
                        Default::default(),
                    )
                    .into()),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card, bank debit (ACH), Google Pay, and Apple Pay are supported for \
                 SetupMandate"
                    .into(),
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
                    mandate_metadata: None,
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
                        network_txn_link_id: None,
                        connector_response_reference_id: echoed,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                    }),
                    ..item.router_data
                })
            }
            _ => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(instrument_not_usable_error(&response, item.http_code)),
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

        let statement_descriptor = router_data
            .request
            .billing_descriptor
            .clone()
            .and_then(|billing_descriptor| billing_descriptor.statement_descriptor);
        let three_d_secure_authentication =
            get_finix_three_d_secure(router_data.request.authentication_data.as_ref())?;
        let fraud_session_id =
            get_finix_fraud_session_id(router_data.request.connector_feature_data.as_ref());

        Ok(Self {
            amount: router_data.request.minor_amount,
            currency: router_data.request.currency,
            source,
            merchant: merchant_id,
            tags: None,
            fraud_session_id,
            three_d_secure_authentication,
            idempotency_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            statement_descriptor,
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

        // Finix echoes AVS / network details on failed charges too, and hyperswitch's
        // `get_finix_response` attaches them outside its success/failure split, so this
        // must be computed before the failure branch to stay at parity.
        let connector_response_data =
            convert_to_additional_payment_method_connector_response(&item.response)
                .map(ConnectorResponseData::with_additional_payment_method_data);

        // Surface explicit Finix failure responses (failure_message present) directly.
        if let Some(failure_message) = response.failure_message.clone() {
            let code = response
                .failure_code
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    connector_response: connector_response_data,
                    ..item.router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code,
                    message: failure_message,
                    reason: None,
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(response.id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            });
        }

        // Reuse the canonical ID-aware status mapping: TR* → Charged, AU* → Authorized.
        let finix_id = FinixId::from(response.id.clone());
        let status = get_finix_attempt_status(&response.state, FinixFlow::from(&finix_id), None);

        // Mirror the Authorize flow for shadow parity: surface the charged Payment
        // Instrument (`source`) as the connector mandate id and attach the AVS / network
        // details as the connector_response.
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response_data,
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                redirection_data: None,
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: response.source.clone().map(|id| id.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// INCOMING WEBHOOKS
// Ported 1:1 from hyperswitch `crates/hyperswitch_connectors/src/connectors/finix.rs`
// (`impl IncomingWebhook for Finix`) and `finix/transformers/response.rs`
// (`FinixWebhookBody::{get_webhook_object_reference_id, get_webhook_event_type,
// get_dispute_details}`). Hyperswitch (Direct gateway) is the source of truth.
// =============================================================================

use domain_types::connector_types::{
    DisputeWebhookDetailsResponse, DisputeWebhookReference, EventType, PaymentWebhookReference,
    RefundWebhookDetailsResponse, RefundWebhookReference, WebhookDetailsResponse,
    WebhookResourceReference,
};
use domain_types::errors::WebhookError;
use time::PrimitiveDateTime;

/// Webhook transfer type. Mirrors HS `FinixPaymentType` (transformers/request.rs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixPaymentType {
    Debit,
    Credit,
    Reversal,
    Fee,
    Adjustment,
    Dispute,
    Reserve,
    Settlement,
    #[serde(other)]
    Unknown,
}

/// Mirrors HS `FinixThreeDSecure` (transformers/request.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixThreeDSecure {
    pub cardholder_authentication: Secret<String>,
    pub electronic_commerce_indicator: String,
    pub transaction_id: Option<String>,
}

/// Webhook payment resource (authorization or transfer). Mirrors HS
/// `FinixPaymentsResponse` (transformers/response.rs) field-for-field so that
/// (de)serialization succeeds/fails on exactly the same payloads.
/// Named `FinixWebhookPaymentsResponse` because prism already has a different
/// `FinixPaymentsResponse` for the sync/authorize API responses.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixWebhookPaymentsResponse {
    pub id: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub application: Option<Secret<String>>,
    pub amount: MinorUnit,
    pub captured_amount: Option<MinorUnit>,
    pub currency: Currency,
    pub is_void: Option<bool>,
    pub source: Option<Secret<String>>,
    pub state: FinixPaymentStatus,
    pub failure_code: Option<String>,
    pub messages: Option<Vec<String>>,
    pub failure_message: Option<String>,
    pub transfer: Option<String>,
    // Optional in the Direct (hyperswitch) model and genuinely omitted by the documented
    // `authorization` webhook sample — a required field here would reject those events.
    #[serde(default)]
    pub tags: Option<FinixTags>,
    #[serde(rename = "type")]
    pub payment_type: Option<FinixPaymentType>,
    pub three_d_secure: Option<FinixThreeDSecure>,
    pub address_verification: Option<String>,
    pub network_details: Option<FinixNetworkDetails>,
}

/// Mirrors HS `FinixDisputeState` (transformers/response.rs). `#[serde(other)]` keeps a
/// future Finix dispute state decodable instead of failing the whole webhook body.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixDisputeState {
    Inquiry,
    Pending,
    Lost,
    Won,
    #[serde(other)]
    Unknown,
}

/// Mirrors HS `FinixDisputes` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixDisputes {
    pub transfer: String,
    pub reason: Option<String>,
    // Finix declares `Dispute.amount` as `nullable: true` (OpenAPI
    // `components.schemas.dispute.properties.amount`), and the Dispute schema marks no
    // field as required. A non-`Option` field here therefore fails `FinixWebhookBody`
    // decoding for any dispute delivered without an amount and — because `FinixEmbedded`
    // is untagged — takes `get_event_type` and `get_webhook_event_reference` down with
    // it, the same way the missing `currency` key did. The absence is reported only where
    // the value is actually needed, in `build_finix_dispute_webhook_response`.
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    pub state: FinixDisputeState,
    // The documented Finix Dispute resource (`GET /disputes/{id}`) carries NO `currency`
    // key, so this must stay optional: a required field would fail `FinixWebhookBody`
    // decoding for every real dispute event and take `get_event_type` /
    // `get_webhook_event_reference` down with it (`FinixEmbedded` is untagged). The
    // absence is only reported where the value is actually needed, in
    // `build_finix_dispute_webhook_response`.
    #[serde(default)]
    pub currency: Option<Currency>,
    pub id: String,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub created_at: Option<PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub updated_at: Option<PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub respond_by: Option<PrimitiveDateTime>,
}

/// Mirrors HS `SingleEventType` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SingleEventType<T>(Vec<T>);

impl<T: Clone> SingleEventType<T> {
    pub fn get_first_event(&self) -> Result<T, error_stack::Report<WebhookError>> {
        self.0
            .first()
            .cloned()
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookBodyDecodingFailed))
    }
}

/// Mirrors HS `FinixEvidence` (transformers/response.rs). `evidence.created` is a
/// default-on Finix subscription, so the variant exists purely so such a delivery
/// degrades to "event not supported" instead of `WebhookBodyDecodingFailed`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixEvidence {
    pub dispute: String,
    pub id: String,
    pub state: String,
}

/// Mirrors HS `FinixEmbedded` (transformers/response.rs). Untagged: variant order
/// (Authorizations, Transfers, Disputes, Evidences) matches HS so ambiguous payloads
/// resolve identically.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FinixEmbedded {
    Authorizations {
        authorizations: SingleEventType<FinixWebhookPaymentsResponse>,
    },
    Transfers {
        transfers: SingleEventType<FinixWebhookPaymentsResponse>,
    },
    Disputes {
        disputes: SingleEventType<FinixDisputes>,
    },
    Evidences {
        evidences: SingleEventType<FinixEvidence>,
    },
}

/// Mirrors HS `FinixWebhookBody` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixWebhookBody {
    #[serde(rename = "type")]
    pub webhook_type: String,
    pub entity: String,
    #[serde(rename = "_embedded")]
    pub webhook_embedded: FinixEmbedded,
}

/// Parsed `Finix-Signature` header. Mirrors HS `FinixWebhookSignature`
/// (`sig` kept as the hex string; hex-decoded at the verification site).
#[derive(Debug)]
pub struct FinixWebhookSignature {
    pub timestamp: String,
    pub sig: String,
}

/// HS treats an empty body or a body that trims to `{}` as Finix's
/// endpoint-verification probe (finix.rs `get_webhook_event_type`). HS's
/// `SetupWebhook` arm is unreachable there (`is_test_webhook` only matches an
/// exact `"{}"` body, which this check already catches), so it is not ported.
pub(super) fn is_endpoint_verification_body(body: &[u8]) -> bool {
    let trimmed_body = std::str::from_utf8(body)
        .map(|string| string.trim())
        .unwrap_or("");
    body.is_empty() || trimmed_body == "{}"
}

pub(super) fn parse_finix_webhook_body(
    body: &[u8],
) -> Result<FinixWebhookBody, error_stack::Report<WebhookError>> {
    serde_json::from_slice::<FinixWebhookBody>(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("failed to deserialize the Finix webhook body (FinixWebhookBody)")
}

/// Parses the comma-separated `key=value` pairs of the `Finix-Signature` header.
/// Ports HS `decode_finix_signature` (finix.rs); header lookup is
/// case-insensitive because the gRPC surface transports headers as a plain map.
pub(super) fn decode_finix_signature(
    headers: &HashMap<String, String>,
) -> Result<FinixWebhookSignature, error_stack::Report<WebhookError>> {
    let security_header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("finix-signature"))
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;

    let map: HashMap<&str, &str> = security_header
        .split(',')
        .map(|kv| {
            let mut parts = kv.trim().splitn(2, '=');
            let key = parts
                .next()
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;
            let value = parts
                .next()
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;
            Ok((key, value))
        })
        .collect::<Result<_, error_stack::Report<WebhookError>>>()?;

    let timestamp = map
        .get("timestamp")
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?
        .to_string();

    let sig = map
        .get("sig")
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?
        .to_string();

    Ok(FinixWebhookSignature { timestamp, sig })
}

/// Ports HS `FinixWebhookBody::get_webhook_event_type` arm-for-arm.
/// HS maps a PENDING reversal to `IncomingWebhookEvent::EventNotSupported`;
/// the equivalent here is `EventType::IncomingWebhookEventUnspecified`
/// (proto `WEBHOOK_EVENT_TYPE_UNSPECIFIED` = 0, which the HS UCS client maps
/// back to `EventNotSupported` in `from_ucs_event_type`).
pub(super) fn get_finix_webhook_event_type(
    body: &FinixWebhookBody,
) -> Result<EventType, error_stack::Report<WebhookError>> {
    match &body.webhook_embedded {
        FinixEmbedded::Authorizations { authorizations } => {
            let authorization = authorizations.get_first_event()?;

            if authorization.is_void == Some(true) {
                match authorization.state {
                    FinixPaymentStatus::Failed
                    | FinixPaymentStatus::Canceled
                    | FinixPaymentStatus::Returned
                    | FinixPaymentStatus::Unknown => Ok(EventType::PaymentIntentCancelFailure),
                    FinixPaymentStatus::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixPaymentStatus::Succeeded => Ok(EventType::PaymentIntentCancelled),
                }
            } else {
                match authorization.state {
                    FinixPaymentStatus::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixPaymentStatus::Succeeded => {
                        Ok(EventType::PaymentIntentAuthorizationSuccess)
                    }
                    FinixPaymentStatus::Failed
                    | FinixPaymentStatus::Canceled
                    | FinixPaymentStatus::Returned => {
                        Ok(EventType::PaymentIntentAuthorizationFailure)
                    }
                    // Indeterminate: emit no event rather than a terminal failure.
                    FinixPaymentStatus::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
                }
            }
        }
        FinixEmbedded::Transfers { transfers } => {
            let transfer = transfers.get_first_event()?;

            match transfer.payment_type {
                Some(FinixPaymentType::Reversal) => match transfer.state {
                    FinixPaymentStatus::Succeeded => Ok(EventType::RefundSuccess),
                    // HS: `EventNotSupported` (see doc comment above).
                    FinixPaymentStatus::Pending => Ok(EventType::IncomingWebhookEventUnspecified),
                    FinixPaymentStatus::Failed
                    | FinixPaymentStatus::Canceled
                    | FinixPaymentStatus::Returned => Ok(EventType::RefundFailure),
                    FinixPaymentStatus::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
                },
                Some(FinixPaymentType::Debit) => match transfer.state {
                    FinixPaymentStatus::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixPaymentStatus::Succeeded => Ok(EventType::PaymentIntentSuccess),
                    FinixPaymentStatus::Failed
                    | FinixPaymentStatus::Canceled
                    | FinixPaymentStatus::Returned => Ok(EventType::PaymentIntentFailure),
                    // HS: `EventNotSupported` for an indeterminate DEBIT transfer.
                    FinixPaymentStatus::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
                },
                // CREDIT / FEE / ADJUSTMENT / DISPUTE / RESERVE / SETTLEMENT / UNKNOWN and
                // an absent `type` are platform-ledger movements, not the merchant's
                // payment. HS returns `EventNotSupported` for all of them; classifying
                // them as payment events would apply a settlement or a dispute debit to
                // the payment attempt.
                Some(
                    FinixPaymentType::Credit
                    | FinixPaymentType::Fee
                    | FinixPaymentType::Adjustment
                    | FinixPaymentType::Dispute
                    | FinixPaymentType::Reserve
                    | FinixPaymentType::Settlement
                    | FinixPaymentType::Unknown,
                )
                | None => Ok(EventType::IncomingWebhookEventUnspecified),
            }
        }
        FinixEmbedded::Disputes { disputes } => {
            let dispute = disputes.get_first_event()?;

            match dispute.state {
                FinixDisputeState::Pending => Ok(EventType::DisputeOpened),
                FinixDisputeState::Inquiry => Ok(EventType::DisputeChallenged),
                FinixDisputeState::Lost => Ok(EventType::DisputeLost),
                FinixDisputeState::Won => Ok(EventType::DisputeWon),
                FinixDisputeState::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
            }
        }
        // HS `FinixEmbedded::Evidences` -> `EventNotSupported`.
        FinixEmbedded::Evidences { .. } => Ok(EventType::IncomingWebhookEventUnspecified),
    }
}

/// Ports HS `FinixWebhookBody::get_webhook_object_reference_id`:
/// - Authorizations -> `PaymentId(ConnectorTransactionId(authorization.id))`
/// - Transfers (REVERSAL) -> `RefundId(ConnectorRefundId(transfer.id))`
/// - Transfers (DEBIT) -> `PaymentId(ConnectorTransactionId(transfer.id))`
/// - Transfers (any other `type`, or an absent one) -> `WebhookEventTypeNotFound`: these
///   are platform-ledger movements (fee, settlement, reserve, dispute debit, …) whose id
///   is not a payment or a refund the caller can resolve.
/// - Disputes -> HS emits `PaymentId(ConnectorTransactionId(dispute.transfer))`; the
///   HS reference normaliser maps a prism Dispute reference via
///   `connector_dispute_id.or(connector_transaction_id)` into exactly that, so
///   `connector_dispute_id` MUST stay `None` for byte-parity with the Direct gateway.
pub(super) fn get_finix_webhook_reference(
    body: &FinixWebhookBody,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    match &body.webhook_embedded {
        FinixEmbedded::Authorizations { authorizations } => {
            let authorization = authorizations.get_first_event()?;

            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(authorization.id),
                    merchant_transaction_id: None,
                },
            )))
        }
        FinixEmbedded::Transfers { transfers } => {
            let transfer = transfers.get_first_event()?;

            match transfer.payment_type {
                Some(FinixPaymentType::Reversal) => Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(transfer.id),
                        merchant_refund_id: None,
                        connector_transaction_id: None,
                    },
                ))),
                Some(FinixPaymentType::Debit) => Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(transfer.id),
                        merchant_transaction_id: None,
                    },
                ))),
                // Platform-ledger transfers (fee, settlement, reserve, dispute debit,
                // credit, adjustment) — HS: Err(WebhookEventTypeNotFound).
                Some(
                    FinixPaymentType::Credit
                    | FinixPaymentType::Fee
                    | FinixPaymentType::Adjustment
                    | FinixPaymentType::Dispute
                    | FinixPaymentType::Reserve
                    | FinixPaymentType::Settlement
                    | FinixPaymentType::Unknown,
                )
                | None => {
                    Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound)).attach_printable(
                        "Finix transfer webhook is not a DEBIT or a REVERSAL, so its id is neither a payment nor a refund reference",
                    )
                }
            }
        }
        FinixEmbedded::Disputes { disputes } => {
            let dispute = disputes.get_first_event()?;

            Ok(Some(WebhookResourceReference::Dispute(
                DisputeWebhookReference {
                    connector_dispute_id: None,
                    connector_transaction_id: Some(dispute.transfer),
                },
            )))
        }
        // HS `FinixEmbedded::Evidences` -> Err(WebhookEventTypeNotFound).
        FinixEmbedded::Evidences { .. } => {
            Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                .attach_printable("Finix `evidence` webhooks carry no payment or refund reference")
        }
    }
}

/// Dispute status per the HS webhook event mapping (PENDING -> DisputeOpened,
/// INQUIRY -> DisputeChallenged, LOST -> DisputeLost, WON -> DisputeWon).
fn get_finix_webhook_dispute_status(
    state: &FinixDisputeState,
) -> Result<common_enums::DisputeStatus, error_stack::Report<WebhookError>> {
    match state {
        FinixDisputeState::Pending => Ok(common_enums::DisputeStatus::DisputeOpened),
        FinixDisputeState::Inquiry => Ok(common_enums::DisputeStatus::DisputeChallenged),
        FinixDisputeState::Lost => Ok(common_enums::DisputeStatus::DisputeLost),
        FinixDisputeState::Won => Ok(common_enums::DisputeStatus::DisputeWon),
        // An unmapped dispute state has no safe DisputeStatus; report it instead of
        // guessing a terminal outcome.
        FinixDisputeState::Unknown => {
            Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                .attach_printable("unrecognised Finix dispute state on the webhook payload")
        }
    }
}

/// Builds the payment webhook content for authorizations and DEBIT transfers. Status
/// mapping goes through the canonical `get_finix_attempt_status` shared with Authorize,
/// PSync, Capture and Void, with `FinixFlow::Auth` / `FinixFlow::Transfer` respectively.
pub(super) fn build_finix_payment_webhook_response(
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let (resource, flow) = match &body.webhook_embedded {
        FinixEmbedded::Authorizations { authorizations } => {
            (authorizations.get_first_event()?, FinixFlow::Auth)
        }
        FinixEmbedded::Transfers { transfers } => {
            let transfer = transfers.get_first_event()?;
            // Only a DEBIT transfer is the merchant's payment. The shared webhook
            // dispatcher falls back to the payment builder for any event type it does
            // not recognise, so a REVERSAL, a settlement, a fee, a reserve or a
            // dispute debit would otherwise be reported as a charge on the payment
            // attempt. Refuse them here instead of mis-reporting them.
            if transfer.payment_type != Some(FinixPaymentType::Debit) {
                return Err(error_stack::report!(
                    WebhookError::WebhookResourceObjectNotFound
                ))
                .attach_printable(
                    "expected a payment webhook (authorization or DEBIT transfer), but the transfer is not a DEBIT",
                );
            }
            (transfer, FinixFlow::Transfer)
        }
        FinixEmbedded::Disputes { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a payment webhook, but found a dispute webhook");
        }
        FinixEmbedded::Evidences { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a payment webhook, but found an evidence webhook");
        }
    };

    let status = get_finix_attempt_status(&resource.state, flow, resource.is_void);

    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(resource.id.clone())),
        status,
        connector_response_reference_id: Some(resource.id),
        mandate_reference: None,
        error_code: resource.failure_code,
        error_message: resource.failure_message.clone(),
        error_reason: resource.failure_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: resource
            .captured_amount
            .map(|amount| amount.get_amount_as_i64()),
        minor_amount_captured: resource.captured_amount,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}

/// Builds the refund webhook content for REVERSAL transfers. Status mapping goes through
/// the canonical `impl From<&FinixPaymentStatus> for RefundStatus` shared with Refund and
/// RSync.
pub(super) fn build_finix_refund_webhook_response(
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let transfer = match &body.webhook_embedded {
        FinixEmbedded::Transfers { transfers } => {
            let transfer = transfers.get_first_event()?;
            // A Finix refund is a REVERSAL transfer; nothing else may be reported as a
            // refund outcome (mirrors the DEBIT guard in the payment builder).
            if transfer.payment_type != Some(FinixPaymentType::Reversal) {
                return Err(error_stack::report!(
                    WebhookError::WebhookResourceObjectNotFound
                ))
                .attach_printable(
                    "expected a refund (reversal transfer) webhook, but the transfer is not a REVERSAL",
                );
            }
            transfer
        }
        FinixEmbedded::Authorizations { .. }
        | FinixEmbedded::Disputes { .. }
        | FinixEmbedded::Evidences { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a refund (reversal transfer) webhook");
        }
    };

    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(transfer.id),
        // Finix REVERSAL webhooks only carry connector-side ids — no merchant
        // reference for the original payment.
        merchant_transaction_id: None,
        status: RefundStatus::from(&transfer.state),
        connector_response_reference_id: None,
        error_code: transfer.failure_code,
        error_message: transfer.failure_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

/// Builds the dispute webhook content. Ports HS
/// `FinixWebhookBody::get_dispute_details`: amount converted with the HS
/// webhook amount converter (`StringMinorUnitForConnector`), stage is always
/// `Dispute`, reason surfaced as the dispute message.
pub(super) fn build_finix_dispute_webhook_response(
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let dispute = match &body.webhook_embedded {
        FinixEmbedded::Disputes { disputes } => disputes.get_first_event()?,
        FinixEmbedded::Authorizations { .. }
        | FinixEmbedded::Transfers { .. }
        | FinixEmbedded::Evidences { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a dispute webhook, but found other webhooks");
        }
    };

    // Finix does not put a `currency` on the Dispute resource; hyperswitch takes it from
    // the original payment context and errors when it is absent. UCS's dispute webhook
    // entry point carries no such context, so the absence is reported here rather than
    // being fabricated.
    let currency = dispute
        .currency
        .ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "currency" })
        })
        .attach_printable(
            "Finix dispute webhooks do not carry a currency; it cannot be resolved from UCS state",
        )?;

    // `Dispute.amount` is nullable in Finix's schema; report the absence here (where the
    // value is needed) rather than failing the whole body decode, and never substitute a
    // fabricated zero.
    let amount = dispute
        .amount
        .ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "amount" })
        })
        .attach_printable("Finix dispute webhook carried no `amount`")?;

    Ok(DisputeWebhookDetailsResponse {
        amount: domain_types::utils::convert_amount_for_webhook(
            &common_utils::types::StringMinorUnitForConnector,
            amount,
            currency,
        )?,
        currency,
        dispute_id: dispute.id,
        status: get_finix_webhook_dispute_status(&dispute.state)?,
        stage: common_enums::DisputeStage::Dispute,
        connector_response_reference_id: None,
        dispute_message: dispute.reason,
        connector_reason_code: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
