use crate::{connectors::macros::GetFormData, types::ResponseRouterData};
use common_enums::{
    AttemptStatus, AuthenticationType, CountryAlpha2, CountryAlpha3, Currency, RefundStatus,
};
use common_utils::{
    consts,
    pii::{Email, SecretSerdeValue},
    request::MultipartData,
    types::{MinorUnit, SemanticVersion},
    Method,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateConnectorCustomer, DefendDispute, PSync,
        PaymentMethodToken, PostAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        SubmitEvidence, Void,
    },
    connector_types::{
        AcceptDisputeData, ConnectorCustomerData, ConnectorCustomerResponse, DisputeDefendData,
        DisputeFlowData, DisputeResponseData, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorAuthType, ConnectorResponseData,
        ConnectorSpecificConfig, ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    router_response_types::RedirectForm,
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
        processor_card_network: None,
        card_subtype: None,
        card_segment_type: None,
        funding_source: None,
        card_type: None,
        issuer_name: None,
        issuer_country: None,
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
    /// Decline reason; Finix deprecates `code` for Authorizations/Transfers in its favour.
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    /// Id of the Transfer the failed request created, when one was created.
    pub transfer: Option<String>,
    /// Id of the Authorization the failed request created, when one was created.
    pub authorization: Option<String>,
}

impl FinixErrorResponse {
    fn first_error(&self) -> Option<&FinixError> {
        self.embedded
            .as_ref()
            .and_then(|e| e.errors.as_ref())
            .and_then(|errors| errors.first())
    }

    /// `failure_code` (decline reason), else the envelope `code`.
    pub fn get_code(&self) -> String {
        self.first_error()
            .and_then(|err| err.failure_code.clone().or_else(|| err.code.clone()))
            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string())
    }

    pub fn get_message(&self) -> String {
        self.first_error()
            .and_then(|err| err.message.clone())
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string())
    }

    pub fn get_failure_message(&self) -> Option<String> {
        self.first_error()
            .and_then(|err| err.failure_message.clone())
    }

    /// The Transfer / Authorization the failed request created, if any.
    pub fn get_resource_id(&self) -> Option<String> {
        self.first_error()
            .and_then(|err| err.transfer.clone().or_else(|| err.authorization.clone()))
    }

    /// Attempt status for a charging flow's non-2xx (e.g. 402 DECLINED): a decline that
    /// created a Transfer fails it, one that created an Authorization fails that
    /// authorization; an envelope naming neither is not terminal.
    pub fn get_charge_attempt_status(&self) -> Option<AttemptStatus> {
        self.first_error().and_then(|err| {
            if err.transfer.is_some() {
                Some(AttemptStatus::Failure)
            } else if err.authorization.is_some() {
                Some(AttemptStatus::AuthorizationFailed)
            } else {
                None
            }
        })
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
    // Wallet passthrough (merchant-decrypted Apple Pay / Google Pay) only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub electronic_commerce_indicator_code: Option<String>,
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

// Apple Pay `third_party_token`: Finix wants the stringified PKPaymentToken
// `{"token":{"paymentData":{..},"paymentMethod":{..},"transactionIdentifier":".."}}`.
// `paymentData` is Apple's encrypted blob ({data, signature, header, version}) passed
// through unchanged after base64-decoding.
#[derive(Debug, Serialize)]
pub struct FinixApplePayThirdPartyToken {
    pub token: FinixApplePayToken,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinixApplePayToken {
    pub payment_data: SecretSerdeValue,
    pub payment_method: FinixApplePayPaymentMethod,
    pub transaction_identifier: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinixApplePayPaymentMethod {
    pub display_name: Secret<String>,
    pub network: Secret<String>,
    #[serde(rename = "type")]
    pub method_type: Secret<String>,
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
    pub electronic_commerce_indicator: FinixElectronicCommerceIndicator,
    pub transaction_id: Option<String>,
}

/// Finix `electronic_commerce_indicator`: the outcome class, not the raw two-digit ECI.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixElectronicCommerceIndicator {
    Authenticated,
    Attempted,
    NonAuthenticated,
    #[serde(other)]
    Unknown,
}

impl FinixElectronicCommerceIndicator {
    /// Raw 3DS ECI to Finix's class (Finix docs, 3DS pass-through): Visa 05 /
    /// Mastercard 02 are fully authenticated, Visa 06 / Mastercard 01 are attempts,
    /// anything else carries no liability shift.
    fn from_raw_eci(eci: &str) -> Self {
        match eci {
            "05" | "02" => Self::Authenticated,
            "06" | "01" => Self::Attempted,
            _ => Self::NonAuthenticated,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FinixPaymentsRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub source: String,
    pub merchant: Secret<String>,
    pub tags: Option<serde_json::Value>,
    pub fraud_session_id: Option<String>,
    #[serde(rename = "3d_secure_authentication")]
    pub three_d_secure_authentication: Option<FinixPaymentThreeDSecure>,
    /// Finix-hosted 3DS: the SUCCEEDED session from Authenticate/PostAuthenticate.
    #[serde(rename = "3d_secure_session", skip_serializing_if = "Option::is_none")]
    pub three_d_secure_session: Option<String>,
    pub idempotency_id: Option<String>,
    pub statement_descriptor: Option<String>,
    /// MIT only (RepeatPayment); Authorize sends none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_on_file: Option<FinixCredentialOnFile>,
}

pub type FinixAuthorizeRequest = FinixPaymentsRequest;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixCredentialOnFileType {
    Recurring,
    Unscheduled,
}

impl From<Option<common_enums::MitCategory>> for FinixCredentialOnFileType {
    fn from(mit_category: Option<common_enums::MitCategory>) -> Self {
        match mit_category {
            Some(
                common_enums::MitCategory::Recurring
                | common_enums::MitCategory::Subscription
                | common_enums::MitCategory::Installment,
            ) => Self::Recurring,
            Some(
                common_enums::MitCategory::Unscheduled | common_enums::MitCategory::Resubmission,
            )
            | None => Self::Unscheduled,
        }
    }
}

/// `credential_on_file` of POST /authorizations and POST /transfers. Finix requires
/// `initial_transfer_id` on both (sandbox rejects `type` alone on /transfers).
#[derive(Debug, Serialize)]
pub struct FinixCredentialOnFile {
    #[serde(rename = "type")]
    pub credential_type: FinixCredentialOnFileType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_transfer_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinixCredentialOnFileEcho {
    pub initial_transfer_id: Option<String>,
}

/// The id a later MIT may cite. Finix answers 500 on an MIT citing a PENDING transfer and accepts
/// it once SUCCEEDED, so an authorization or an unsettled transfer gives nothing yet.
fn finix_initial_transfer_id(id: &str, state: &FinixPaymentStatus) -> Option<String> {
    (id.starts_with("TR") && matches!(state, FinixPaymentStatus::Succeeded)).then(|| id.to_string())
}

/// Key under which the CIT transfer id rides in `mandate_reference.mandate_metadata`.
const FINIX_INITIAL_TRANSFER_ID: &str = "initial_transfer_id";

fn get_finix_mandate_metadata(mandate_reference: &MandateReferenceId) -> Option<SecretSerdeValue> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(ids) => ids.get_mandate_metadata(),
        MandateReferenceId::NetworkMandateId(_) | MandateReferenceId::NetworkTokenWithNTI(_) => {
            None
        }
    }
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
    pub address_verification: Option<String>,
    pub source: Option<Secret<String>>,
    pub network_details: Option<FinixNetworkDetails>,
}

/// Finix returns no network (scheme) transaction id in any Authorization or Transfer schema:
/// `network_details` carries only `brand`, `authorization_code` (and, on Mastercard transfers,
/// `acquirer_reference_number`). Every flow therefore leaves `network_txn_id` /
/// `network_txn_link_id` as `None`; `authorization_code` and `trace_id` are not substitutes.
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixPaymentStatus {
    Succeeded,
    Failed,
    Pending,
    Canceled,
    Unknown,
    /// ACH return: "returned and was unable to be processed" (bank_return_details).
    Returned,
    /// A state Finix does not document. Non-terminal everywhere: see the arms below.
    #[serde(other)]
    Unrecognized,
}

/// Authorization with `is_void: true`. Finix: a voided Authorization "can no longer be
/// captured", and void_state stays PENDING while state is SUCCEEDED, so PENDING is voided too.
fn get_finix_void_attempt_status(state: &FinixPaymentStatus) -> AttemptStatus {
    match state {
        FinixPaymentStatus::Succeeded | FinixPaymentStatus::Pending => AttemptStatus::Voided,
        FinixPaymentStatus::Failed
        | FinixPaymentStatus::Canceled
        | FinixPaymentStatus::Unknown
        | FinixPaymentStatus::Returned => AttemptStatus::VoidFailed,
        FinixPaymentStatus::Unrecognized => AttemptStatus::Pending,
    }
}

fn get_finix_attempt_status(id: &FinixId, state: &FinixPaymentStatus) -> AttemptStatus {
    match (id, state) {
        (FinixId::Transfer(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Charged,
        // An unexplained state must not read as failed: a merchant retry could charge twice.
        (FinixId::Transfer(_), FinixPaymentStatus::Pending | FinixPaymentStatus::Unrecognized) => {
            AttemptStatus::Pending
        }
        (
            FinixId::Transfer(_),
            FinixPaymentStatus::Failed
            | FinixPaymentStatus::Canceled
            | FinixPaymentStatus::Unknown
            | FinixPaymentStatus::Returned,
        ) => AttemptStatus::Failure,
        (FinixId::Auth(_), FinixPaymentStatus::Succeeded) => AttemptStatus::Authorized,
        (FinixId::Auth(_), FinixPaymentStatus::Pending | FinixPaymentStatus::Unrecognized) => {
            AttemptStatus::AuthenticationPending
        }
        (
            FinixId::Auth(_),
            FinixPaymentStatus::Failed
            | FinixPaymentStatus::Canceled
            | FinixPaymentStatus::Unknown
            | FinixPaymentStatus::Returned,
        ) => AttemptStatus::AuthorizationFailed,
    }
}

const FINIX_3DS_DOC_URL: &str =
    "https://docs.finix.com/guides/online-payments/fraud-and-risk/3d-secure/finix-api";

/// Finix resource ids are type-prefixed; a 3D Secure Session id reads `3d_secure_session_...`.
const FINIX_3DS_SESSION_ID_PREFIX: &str = "3d_secure_session_";

fn external_3ds_missing_field(field_name: &'static str) -> IntegrationError {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Send both authentication_data.cavv and authentication_data.eci from the external 3DS provider, or run Finix-hosted 3DS (Authenticate + PostAuthenticate).".to_string(),
            ),
            doc_url: Some(FINIX_3DS_DOC_URL.to_string()),
            additional_context: Some(
                "Finix 3d_secure_authentication requires cardholder_authentication (CAVV) and electronic_commerce_indicator.".to_string(),
            ),
        },
    }
}

/// External 3DS pass-through (`3d_secure_authentication`).
fn get_finix_three_d_secure(
    authentication_data: Option<&AuthenticationData>,
) -> Result<Option<FinixPaymentThreeDSecure>, error_stack::Report<IntegrationError>> {
    authentication_data
        .map(|auth_data| {
            let eci = auth_data
                .eci
                .as_deref()
                .ok_or_else(|| external_3ds_missing_field("authentication_data.eci"))?;
            Ok(FinixPaymentThreeDSecure {
                cardholder_authentication: auth_data
                    .cavv
                    .clone()
                    .ok_or_else(|| external_3ds_missing_field("authentication_data.cavv"))?,
                electronic_commerce_indicator: FinixElectronicCommerceIndicator::from_raw_eci(eci),
                // Finix docs: "The ID the issuer assigns to the 3D Secure transaction" (ACS id).
                transaction_id: auth_data.acs_transaction_id.clone(),
            })
        })
        .transpose()
}

/// The Finix 3D Secure Session id carried in `authentication_data.transaction_id`
/// (proto `AuthenticationData.connector_transaction_id`) by Authenticate/PostAuthenticate.
fn get_finix_three_d_secure_session(
    authentication_data: Option<&AuthenticationData>,
) -> Option<String> {
    authentication_data
        .and_then(|auth_data| auth_data.transaction_id.as_ref())
        .filter(|id| id.starts_with(FINIX_3DS_SESSION_ID_PREFIX))
        .cloned()
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
        let merchant_id = auth.merchant_id.clone();

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
        // A Finix 3D Secure Session id goes in `3d_secure_session` alone; any other
        // authentication_data is an external 3DS result passed through as-is.
        let authentication_data = router_data.request.authentication_data.as_ref();
        let three_d_secure_session = get_finix_three_d_secure_session(authentication_data);
        // G-ThreeDS-07: a session id says the process ran, not that the buyer authenticated.
        // Finix: NON_AUTHENTICATED "Liability stays with the seller"; PENDING has no outcome yet.
        if three_d_secure_session.is_some()
            && !matches!(
                authentication_data.and_then(|auth_data| auth_data.trans_status.clone()),
                Some(
                    common_enums::TransactionStatus::Success
                        | common_enums::TransactionStatus::NotVerified
                )
            )
        {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "authentication_data.trans_status",
                context: three_ds_context(
                    "Charge only after PostAuthenticate reports AUTHENTICATION_SUCCESSFUL, and pass its authentication_data unchanged.",
                    "A Finix 3D Secure Session is chargeable only when it authenticated (AUTHENTICATED or ATTEMPTED).",
                ),
            }
            .into());
        }
        let three_d_secure_authentication = match three_d_secure_session {
            Some(_) => None,
            None => get_finix_three_d_secure(authentication_data)?,
        };
        // G-ThreeDS-05: Finix has no in-Authorize 3DS; charging a ThreeDs payment with no
        // authentication result would silently skip authentication.
        if router_data.resource_common_data.auth_type == AuthenticationType::ThreeDs
            && three_d_secure_session.is_none()
            && three_d_secure_authentication.is_none()
        {
            return Err(IntegrationError::NotSupported {
                message: "Finix has no in-Authorize 3DS: a ThreeDs authorize needs a 3D Secure Session or external authentication_data.".to_string(),
                connector: "finix",
                context: IntegrationErrorContext {
                    suggested_action: Some("Run PaymentMethodAuthenticationService Authenticate + PostAuthenticate (or CompositePaymentService/Authorize) and pass the session id as authentication_data.connector_transaction_id, or send external 3DS authentication_data (cavv + eci).".to_string()),
                    doc_url: Some(FINIX_3DS_DOC_URL.to_string()),
                    additional_context: Some("Finix charges with 3D Secure via `3d_secure_session` on POST /transfers or /authorizations.".to_string()),
                },
            }
            .into());
        }
        let fraud_session_id =
            get_finix_fraud_session_id(router_data.request.connector_feature_data.as_ref());

        Ok(Self {
            amount: router_data.request.amount.amount,
            currency: router_data.request.currency,
            source,
            merchant: merchant_id,
            tags: None,
            fraud_session_id,
            three_d_secure_authentication,
            three_d_secure_session,
            idempotency_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            statement_descriptor,
            credential_on_file: None,
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

        // Determine status based on ID type, mirroring Hyperswitch's `get_attempt_status`:
        // - Transfer (TR*): Succeeded -> Charged, failure -> Failure
        // - Authorization (AU*): Succeeded -> Authorized, failure -> AuthorizationFailed
        let finix_id = FinixId::from(response.id.clone());
        let status = get_finix_attempt_status(&finix_id, &response.state);

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
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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

            // The CIT transfer id is what a later MIT sends as
            // `credential_on_file.initial_transfer_id`.
            let initial_transfer_id = finix_initial_transfer_id(&response.id, &response.state);

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
                redirection_data: None,
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: response.source.clone().map(|id| id.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: initial_transfer_id.map(|id| {
                        Secret::new(serde_json::json!({ FINIX_INITIAL_TRANSFER_ID: id }))
                    }),
                })),
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
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

// THREE DS - 3D SECURE SESSIONS (Authenticate / PostAuthenticate)

/// `POST /3d_secure_sessions` (redirect flow, payment-instrument based).
#[derive(Debug, Serialize)]
pub struct FinixThreeDSecureSessionRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub merchant_id: Secret<String>,
    pub payment_instrument_id: Secret<String>,
    pub redirect_url: String,
}

fn three_ds_context(suggested_action: &str, additional_context: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_string()),
        doc_url: Some(FINIX_3DS_DOC_URL.to_string()),
        additional_context: Some(additional_context.to_string()),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FinixThreeDSecureSessionRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // G-ThreeDS-01: a session is created for a Finix Payment Instrument; there is
        // no raw-card session create on the server-side API.
        let payment_instrument_id = match &request.payment_method_data {
            Some(PaymentMethodData::PaymentMethodToken(t)) => t.token.clone(),
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Finix 3D Secure Sessions authenticate a tokenized Payment Instrument only; raw card/wallet data is not accepted.".to_string(),
                    connector: "finix",
                    context: three_ds_context(
                        "Tokenize the card first via PaymentMethodService/Tokenize and send the Finix Payment Instrument id (PI...) as payment_method.token.",
                        "POST /3d_secure_sessions requires payment_instrument_id.",
                    ),
                }
                .into())
            }
        };

        // G-ThreeDS-02: 3DS sessions accept USD and CAD only.
        let currency = match request.currency {
            Some(currency @ (Currency::USD | Currency::CAD)) => currency,
            Some(other) => {
                return Err(IntegrationError::NotSupported {
                    message: format!("Finix 3D Secure Sessions do not support currency {other}"),
                    connector: "finix",
                    context: three_ds_context(
                        "Use USD or CAD for Finix 3D Secure.",
                        "POST /3d_secure_sessions currency must be USD or CAD.",
                    ),
                }
                .into())
            }
            None => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: three_ds_context(
                        "Send the payment currency (USD or CAD).",
                        "POST /3d_secure_sessions requires currency.",
                    ),
                }
                .into())
            }
        };

        // G-ThreeDS-03: the buyer returns to redirect_url after the hosted page.
        let redirect_url = request
            .continue_redirection_url
            .as_ref()
            .or(request.router_return_url.as_ref())
            .map(|url| url.to_string())
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: three_ds_context(
                    "Send continue_redirection_url (or return_url): the HTTPS URL the buyer returns to after authentication.",
                    "POST /3d_secure_sessions requires redirect_url for the redirect flow.",
                ),
            })?;

        let auth = FinixAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            amount: request.amount.amount,
            currency,
            merchant_id: auth.merchant_id.clone(),
            payment_instrument_id,
            redirect_url,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixThreeDSecureSessionState {
    Pending,
    Succeeded,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinixThreeDSecureAuthentication {
    pub cardholder_authentication: Option<Secret<String>>,
    pub electronic_commerce_indicator: Option<FinixElectronicCommerceIndicator>,
    pub transaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinixThreeDSecureSessionResponse {
    pub id: String,
    pub state: FinixThreeDSecureSessionState,
    #[serde(rename = "3d_secure_authentication_url")]
    pub three_d_secure_authentication_url: Option<String>,
    #[serde(rename = "3d_secure_authentication")]
    pub three_d_secure_authentication: Option<FinixThreeDSecureAuthentication>,
    #[serde(rename = "3d_secure_version")]
    pub three_d_secure_version: Option<String>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    /// Null when the session was created from a token_id; UCS always creates it from a PI.
    pub payment_instrument_id: Option<Secret<String>>,
}

pub type FinixAuthenticateResponse = FinixThreeDSecureSessionResponse;
pub type FinixPostAuthenticateResponse = FinixThreeDSecureSessionResponse;

impl FinixThreeDSecureSessionResponse {
    fn eci(&self) -> Option<FinixElectronicCommerceIndicator> {
        self.three_d_secure_authentication
            .as_ref()
            .and_then(|auth| auth.electronic_commerce_indicator)
    }

    /// Finix: "A state of SUCCEEDED means the process finished. It does not mean the
    /// buyer was authenticated" -- the outcome is the electronic_commerce_indicator.
    fn attempt_status(&self) -> AttemptStatus {
        match self.state {
            FinixThreeDSecureSessionState::Failed => AttemptStatus::AuthenticationFailed,
            FinixThreeDSecureSessionState::Pending | FinixThreeDSecureSessionState::Unknown => {
                AttemptStatus::AuthenticationPending
            }
            FinixThreeDSecureSessionState::Succeeded => match self.eci() {
                Some(
                    FinixElectronicCommerceIndicator::Authenticated
                    | FinixElectronicCommerceIndicator::Attempted,
                ) => AttemptStatus::AuthenticationSuccessful,
                Some(FinixElectronicCommerceIndicator::NonAuthenticated) => {
                    AttemptStatus::AuthenticationFailed
                }
                // No documented outcome: stay non-terminal.
                Some(FinixElectronicCommerceIndicator::Unknown) | None => {
                    AttemptStatus::AuthenticationPending
                }
            },
        }
    }

    fn trans_status(&self) -> Option<common_enums::TransactionStatus> {
        match self.eci()? {
            FinixElectronicCommerceIndicator::Authenticated => {
                Some(common_enums::TransactionStatus::Success)
            }
            FinixElectronicCommerceIndicator::Attempted => {
                Some(common_enums::TransactionStatus::NotVerified)
            }
            FinixElectronicCommerceIndicator::NonAuthenticated => {
                Some(common_enums::TransactionStatus::Failure)
            }
            FinixElectronicCommerceIndicator::Unknown => None,
        }
    }

    /// Authentication data carrying the session id in `transaction_id` (proto
    /// `AuthenticationData.connector_transaction_id`), which Authorize sends as
    /// `3d_secure_session`.
    fn authentication_data(&self) -> AuthenticationData {
        let auth = self.three_d_secure_authentication.as_ref();
        AuthenticationData {
            trans_status: self.trans_status(),
            eci: None,
            cavv: auth.and_then(|a| a.cardholder_authentication.clone()),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: None,
            // Meta only: an unparsable version is dropped rather than failing the leg.
            message_version: self
                .three_d_secure_version
                .as_deref()
                .and_then(|v| v.parse::<SemanticVersion>().ok()),
            ds_trans_id: None,
            acs_transaction_id: auth.and_then(|a| a.transaction_id.clone()),
            transaction_id: Some(self.id.clone()),
            network_params: None,
            exemption_indicator: None,
            created_at: None,
            challenge_code: None,
            challenge_cancel: None,
            challenge_code_reason: None,
            message_extension: None,
            authentication_type: None,
        }
    }

    /// FAILED is the only failure state (2xx with `state: FAILED`, `failure_code` set).
    fn failed_error_response(&self, status_code: u16) -> Option<ErrorResponse> {
        (self.state == FinixThreeDSecureSessionState::Failed).then(|| ErrorResponse {
            code: self
                .failure_code
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            message: self
                .failure_message
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
            reason: self.failure_message.clone(),
            status_code,
            attempt_status: Some(FlowStatus::Payment(AttemptStatus::AuthenticationFailed)),
            connector_transaction_id: Some(self.id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FinixAuthenticateResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let status = response.attempt_status();

        let flow_response = match response.failed_error_response(item.http_code) {
            Some(error) => Err(error),
            None => {
                let redirection_data = response
                    .three_d_secure_authentication_url
                    .as_deref()
                    .map(|url| {
                        url::Url::parse(url)
                            .map(|url| Box::new(RedirectForm::from((url, Method::Get))))
                            .change_context(crate::utils::response_deserialization_fail(
                                item.http_code,
                                "finix: 3d_secure_authentication_url is not a valid URL",
                            ))
                    })
                    .transpose()?;
                Ok(PaymentsResponseData::AuthenticateResponse {
                    resource_id: Some(ResponseId::ConnectorTransactionId(response.id.clone())),
                    redirection_data,
                    authentication_data: Some(response.authentication_data()),
                    connector_feature_data: None,
                    connector_response_reference_id: Some(response.id.clone()),
                    status_code: item.http_code,
                    mandate_reference: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                })
            }
        };

        Ok(Self {
            response: flow_response,
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FinixPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let status = response.attempt_status();
        // HS CompleteAuthorize has no other source for the charge: card data does not survive
        // the redirect. The key is a cross-repo contract with HS
        // REDIRECT_COMPLETION_PAYMENT_METHOD_TOKEN_KEYS. An existing object keeps its keys
        // (finix_additional_details); a non-object is replaced so the token is never dropped.
        let existing_feature_data = item
            .router_data
            .resource_common_data
            .connector_feature_data
            .as_ref();
        let connector_feature_data = match response.payment_instrument_id.as_ref() {
            Some(instrument) => {
                let mut data = existing_feature_data
                    .and_then(|existing| existing.peek().as_object().cloned())
                    .unwrap_or_default();
                data.insert(
                    "payment_method_token".to_string(),
                    serde_json::Value::String(instrument.peek().clone()),
                );
                Some(Secret::new(serde_json::Value::Object(data)))
            }
            None => existing_feature_data.cloned(),
        };

        let flow_response = match response.failed_error_response(item.http_code) {
            Some(error) => Err(error),
            None => Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data: Some(response.authentication_data()),
                connector_response_reference_id: Some(response.id.clone()),
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response: flow_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data,
                ..item.router_data.resource_common_data.clone()
            },
            ..item.router_data
        })
    }
}

/// G-ThreeDS-04: the session to retrieve -- the Authenticate leg's id
/// (`connector_order_reference_id`), else the `session` query parameter Finix appends
/// to redirect_url. The value lands in a URL path, so a browser-supplied id must look
/// like a Finix session id.
pub fn get_three_d_secure_session_id<T: PaymentMethodDataTypes>(
    request: &PaymentsPostAuthenticateData<T>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let session_id = request
        .connector_order_reference_id
        .clone()
        .or_else(|| {
            let params = request.redirect_response.as_ref()?.params.as_ref()?;
            url::form_urlencoded::parse(params.peek().trim_start_matches('?').as_bytes())
                .find(|(key, _)| key == "session")
                .map(|(_, value)| value.into_owned())
        })
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "connector_order_reference_id",
            context: three_ds_context(
                "Send the Authenticate response's connector_transaction_id as connector_order_reference_id, or the redirect query string (contains session=...) as redirection_response.params.",
                "GET /3d_secure_sessions/{id} needs the 3D Secure Session id.",
            ),
        })?;

    let well_formed = session_id
        .strip_prefix(FINIX_3DS_SESSION_ID_PREFIX)
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric()));
    if !well_formed {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "connector_order_reference_id",
            context: three_ds_context(
                "Pass the 3d_secure_session_... id returned by Authenticate.",
                "Finix 3D Secure Session ids have the form 3d_secure_session_<alphanumeric>.",
            ),
        }
        .into());
    }
    Ok(session_id)
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
    /// Echoed on an MIT; its root stays the series root across a PSync of that MIT.
    pub credential_on_file: Option<FinixCredentialOnFileEcho>,
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
        processor_card_network: None,
        card_subtype: None,
        card_segment_type: None,
        funding_source: None,
        card_type: None,
        issuer_name: None,
        issuer_country: None,
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
        // This follows the same pattern as Hyperswitch
        let finix_id = FinixId::from(response.id.clone());
        let status = if response.is_void == Some(true) {
            get_finix_void_attempt_status(&response.state)
        } else {
            get_finix_attempt_status(&finix_id, &response.state)
        };

        // For transfers (TR...), use the transfer ID directly
        // For authorizations (AU...), use transfer ID if available for refunds
        let connector_transaction_id = response
            .transfer
            .clone()
            .unwrap_or_else(|| response.id.clone());

        // Surface the stored Payment Instrument (`source`) as the mandate reference and
        // propagate AVS / network details via `connector_response`.
        // The manual-capture path: the transfer is first citable when a sync sees it SUCCEEDED.
        let initial_transfer_id = response
            .credential_on_file
            .as_ref()
            .and_then(|credential| credential.initial_transfer_id.clone())
            .or_else(|| finix_initial_transfer_id(&response.id, &response.state));
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: response.source.as_ref().map(|s| s.clone().expose()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: initial_transfer_id
                .map(|id| Secret::new(serde_json::json!({ FINIX_INITIAL_TRANSFER_ID: id }))),
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
                payment_account_reference: None,
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
            capture_amount: item.router_data.request.amount_to_capture.amount,
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
        // until a PSync on the transfer id confirms the charge.
        let status = if response.is_void == Some(true) {
            get_finix_void_attempt_status(&response.state)
        } else {
            match response.state {
                FinixPaymentStatus::Pending
                | FinixPaymentStatus::Succeeded
                | FinixPaymentStatus::Unrecognized => AttemptStatus::Pending,
                FinixPaymentStatus::Failed
                | FinixPaymentStatus::Canceled
                | FinixPaymentStatus::Unknown
                | FinixPaymentStatus::Returned => AttemptStatus::Failure,
            }
        };

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
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
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
                payment_account_reference: None,
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
        let refund_status = match response.state {
            FinixPaymentStatus::Succeeded => RefundStatus::Success,
            FinixPaymentStatus::Failed => RefundStatus::Failure,
            FinixPaymentStatus::Pending | FinixPaymentStatus::Unrecognized => RefundStatus::Pending,
            FinixPaymentStatus::Canceled
            | FinixPaymentStatus::Unknown
            | FinixPaymentStatus::Returned => RefundStatus::Failure,
        };
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
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
        let status = if response.is_void == Some(true) {
            get_finix_void_attempt_status(&response.state)
        } else {
            match response.state {
                FinixPaymentStatus::Succeeded => AttemptStatus::Voided,
                FinixPaymentStatus::Pending | FinixPaymentStatus::Unrecognized => {
                    AttemptStatus::Pending
                }
                FinixPaymentStatus::Failed
                | FinixPaymentStatus::Canceled
                | FinixPaymentStatus::Unknown
                | FinixPaymentStatus::Returned => AttemptStatus::VoidFailed,
            }
        };

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
                payment_account_reference: None,
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
            refund_amount: item.router_data.request.refund_amount.amount,
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
            FinixPaymentStatus::Pending | FinixPaymentStatus::Unrecognized => RefundStatus::Pending,
            FinixPaymentStatus::Canceled
            | FinixPaymentStatus::Unknown
            | FinixPaymentStatus::Returned => RefundStatus::Failure,
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
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
                expiration_year: Some(Secret::new(i32::from(
                    card.get_expiry_year_4_digit_as_u16()?,
                ))),
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
                cryptogram: None,
                electronic_commerce_indicator_code: None,
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
                            cryptogram: None,
                            electronic_commerce_indicator_code: None,
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
                let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;
                build_finix_wallet_instrument_request(
                    wallet_data,
                    customer_id,
                    auth.merchant_identity_id,
                    None,
                )
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card, ACH bank debit, Google Pay and Apple Pay tokenization are supported"
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
        let http_code = item.http_code;
        let response = item.response;
        let mut router_data = item.router_data;
        // Finix `/payment_instruments` returns HTTP 201. Preserve the real status code so
        // the proto response reports it (was hardcoded 200 → connector_http_status_code diff).
        router_data.resource_common_data.connector_http_status_code = Some(http_code);
        Ok(Self {
            response: match (response.id.clone(), response.enabled) {
                (Some(id), true) => Ok(PaymentMethodTokenResponse {
                    token: id,
                    connector_payment_method_id: None,
                    status_code: http_code,
                }),
                _ => Err(disabled_instrument_error(&response, http_code)),
            },
            ..router_data
        })
    }
}

const FINIX_PAYMENT_INSTRUMENTS_DOC: &str = "https://docs.finix.com/api/payment-instruments";

fn finix_wallet_error_context(
    suggested_action: &str,
    additional_context: &str,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_string()),
        doc_url: Some(FINIX_PAYMENT_INSTRUMENTS_DOC.to_string()),
        additional_context: Some(additional_context.to_string()),
    }
}

// G-Authorize__Wallet-04: Finix passthrough takes `expiration_month` as an integer 1-12
// and `expiration_year` as a 4-digit integer.
fn get_finix_wallet_expiry(
    month: &Secret<String>,
    four_digit_year: &Secret<String>,
) -> Result<(Secret<i8>, Secret<i32>), error_stack::Report<IntegrationError>> {
    let invalid = || {
        IntegrationError::InvalidDataFormat {
        field_name: "expiration",
        context: finix_wallet_error_context(
            "Send the decrypted wallet expiry as month 1-12 and a 2- or 4-digit year.",
            "Finix wallet passthrough requires integer expiration_month (1-12) and 4-digit expiration_year.",
        ),
    }
    };
    let month = month
        .peek()
        .parse::<i8>()
        .ok()
        .filter(|m| (1..=12).contains(m))
        .ok_or_else(invalid)?;
    let year = four_digit_year
        .peek()
        .parse::<i32>()
        .ok()
        .filter(|y| (1000..=9999).contains(y))
        .ok_or_else(invalid)?;
    Ok((Secret::new(month), Secret::new(year)))
}

// One Payment Instrument builder for Apple Pay / Google Pay, shared by
// PaymentMethodToken and SetupMandate. Encrypted wallet data goes in
// `third_party_token` (Finix decrypts); decrypted data goes as passthrough
// (`number` + `cryptogram` + expiry + ECI). Finix rejects both in one request (422).
fn build_finix_wallet_instrument_request(
    wallet_data: &domain_types::payment_method_data::WalletData,
    identity: String,
    merchant_identity: Secret<String>,
    tags: Option<FinixTags>,
) -> Result<FinixCreatePaymentInstrumentRequest, error_stack::Report<IntegrationError>> {
    use domain_types::payment_method_data::{
        ApplePayPaymentData, GpayTokenizationData, WalletData,
    };

    struct WalletFields {
        instrument_type: FinixPaymentInstrumentType,
        third_party_token: Option<Secret<String>>,
        number: Option<Secret<String>>,
        cryptogram: Option<Secret<String>>,
        eci: Option<String>,
        expiry: Option<(Secret<i8>, Secret<i32>)>,
    }

    let fields = match wallet_data {
        WalletData::GooglePay(google_pay) => match &google_pay.tokenization_data {
            GpayTokenizationData::Encrypted(encrypted) => {
                // G-Authorize__Wallet-01
                if encrypted.token.trim().is_empty() {
                    return Err(IntegrationError::InvalidWalletToken {
                        wallet_name: "Google Pay".to_string(),
                        context: finix_wallet_error_context(
                            "Send paymentData.paymentMethodData.tokenizationData.token from Google Pay.",
                            "Finix GOOGLE_PAY Payment Instruments require a non-empty third_party_token.",
                        ),
                    }
                    .into());
                }
                WalletFields {
                    instrument_type: FinixPaymentInstrumentType::GooglePay,
                    third_party_token: Some(Secret::new(encrypted.token.clone())),
                    number: None,
                    cryptogram: None,
                    eci: None,
                    expiry: None,
                }
            }
            GpayTokenizationData::Decrypted(decrypted) => {
                let year =
                    decrypted
                        .get_four_digit_expiry_year()
                        .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "expiration",
                        context: finix_wallet_error_context(
                            "Send the decrypted Google Pay card_exp_year as a 2- or 4-digit year.",
                            "Finix wallet passthrough requires a 4-digit expiration_year.",
                        ),
                    })?;
                WalletFields {
                    instrument_type: FinixPaymentInstrumentType::GooglePay,
                    third_party_token: None,
                    number: Some(Secret::new(
                        decrypted
                            .application_primary_account_number
                            .peek()
                            .to_string(),
                    )),
                    cryptogram: decrypted.cryptogram.clone(),
                    eci: decrypted.eci_indicator.clone(),
                    expiry: Some(get_finix_wallet_expiry(&decrypted.card_exp_month, &year)?),
                }
            }
        },
        WalletData::ApplePay(apple_pay) => match &apple_pay.payment_data {
            ApplePayPaymentData::Encrypted(encrypted) => {
                // G-Authorize__Wallet-02: payment_data must be base64 of Apple's paymentData JSON.
                let invalid = || {
                    IntegrationError::InvalidDataFormat {
                    field_name: "apple_pay.payment_data",
                    context: finix_wallet_error_context(
                        "Send the PKPaymentToken paymentData object base64-encoded.",
                        "Finix APPLE_PAY third_party_token is built from the decoded Apple Pay paymentData JSON.",
                    ),
                }
                };
                let decoded =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encrypted)
                        .change_context(invalid())?;
                let payment_data: serde_json::Value =
                    serde_json::from_slice(&decoded).change_context(invalid())?;
                let token = FinixApplePayThirdPartyToken {
                    token: FinixApplePayToken {
                        payment_data: Secret::new(payment_data),
                        payment_method: FinixApplePayPaymentMethod {
                            display_name: Secret::new(
                                apple_pay.payment_method.display_name.clone(),
                            ),
                            network: Secret::new(apple_pay.payment_method.network.clone()),
                            method_type: Secret::new(apple_pay.payment_method.pm_type.clone()),
                        },
                        transaction_identifier: Secret::new(
                            apple_pay.transaction_identifier.clone(),
                        ),
                    },
                };
                let third_party_token = serde_json::to_string(&token).change_context(invalid())?;
                WalletFields {
                    instrument_type: FinixPaymentInstrumentType::ApplePay,
                    third_party_token: Some(Secret::new(third_party_token)),
                    number: None,
                    cryptogram: None,
                    eci: None,
                    expiry: None,
                }
            }
            ApplePayPaymentData::Decrypted(decrypted) => WalletFields {
                instrument_type: FinixPaymentInstrumentType::ApplePay,
                third_party_token: None,
                number: Some(Secret::new(
                    decrypted
                        .application_primary_account_number
                        .peek()
                        .to_string(),
                )),
                cryptogram: Some(decrypted.payment_data.online_payment_cryptogram.clone()),
                eci: decrypted.payment_data.eci_indicator.clone(),
                expiry: Some(get_finix_wallet_expiry(
                    &decrypted.get_expiry_month(),
                    &decrypted.get_four_digit_expiry_year(),
                )?),
            },
        },
        // G-Authorize__Wallet-03
        _ => return Err(IntegrationError::NotImplemented(
            "Finix supports only Apple Pay and Google Pay wallets".into(),
            finix_wallet_error_context(
                "Use Apple Pay or Google Pay, or a card / ACH bank debit, with Finix.",
                "Finix documents Payment Instruments only for APPLE_PAY and GOOGLE_PAY wallets.",
            ),
        )
        .into()),
    };

    let (expiration_month, expiration_year) = fields.expiry.unzip();
    Ok(FinixCreatePaymentInstrumentRequest {
        instrument_type: fields.instrument_type,
        name: None,
        number: fields.number,
        security_code: None,
        expiration_month,
        expiration_year,
        identity,
        tags,
        address: None,
        card_brand: None,
        card_type: None,
        additional_data: None,
        merchant_identity: Some(merchant_identity),
        third_party_token: fields.third_party_token,
        cryptogram: fields.cryptogram,
        electronic_commerce_indicator_code: fields.eci,
        account_number: None,
        bank_code: None,
        account_type: None,
    })
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
        attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
        connector_transaction_id: response.id.clone(),
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
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
                expiration_year: Some(Secret::new(i32::from(
                    card.get_expiry_year_4_digit_as_u16()?,
                ))),
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
                cryptogram: None,
                electronic_commerce_indicator_code: None,
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
                            cryptogram: None,
                            electronic_commerce_indicator_code: None,
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
                let auth = FinixAuthType::try_from(&item.router_data.connector_config)?;
                build_finix_wallet_instrument_request(
                    wallet_data,
                    customer_id,
                    auth.merchant_identity_id,
                    tags,
                )
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only card, ACH bank debit, Google Pay and Apple Pay are supported for SetupMandate".into(),
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
                        payment_account_reference: None,
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
// POST /authorizations (manual capture), flagged as an MIT via `credential_on_file`.

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
        let merchant_id = auth.merchant_id.clone();

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
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the Finix Payment Instrument ID (PI...) from SetupRecurring as \
                             a connector mandate id."
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.finix.com/api/authorizations".to_string()),
                        additional_context: Some(
                            "Finix returns and accepts no network transaction id; an MIT is \
                             charged against its own stored Payment Instrument."
                                .to_string(),
                        ),
                    },
                }
                .into());
            }
        };

        // MIT credential_on_file: Finix rejects `{type}` alone on POST /transfers (422 "The Transfer ID
        // is invalid.") and accepts `{type, initial_transfer_id}` on both endpoints, so send it whenever
        // the CIT transfer id is known and omit the object otherwise.
        let credential_type =
            FinixCredentialOnFileType::from(router_data.request.mit_category.clone());
        let credential_on_file = get_finix_mandate_metadata(&router_data.request.mandate_reference)
            .and_then(|metadata| {
                metadata
                    .peek()
                    .get(FINIX_INITIAL_TRANSFER_ID)
                    .and_then(|id| id.as_str())
                    .map(str::to_string)
            })
            .map(|initial_transfer_id| FinixCredentialOnFile {
                credential_type,
                initial_transfer_id: Some(initial_transfer_id),
            });

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
            amount: router_data.request.amount.amount,
            currency: router_data.request.currency,
            source,
            merchant: merchant_id,
            tags: None,
            fraud_session_id,
            three_d_secure_authentication,
            three_d_secure_session: None,
            idempotency_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            statement_descriptor,
            credential_on_file,
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
        // Reuse Authorize's ID-aware status mapping: TR* → Charged, AU* → Authorized.
        let finix_id = FinixId::from(response.id.clone());
        let status = get_finix_attempt_status(&finix_id, &response.state);

        // Mirror HS `get_finix_response` (which every finix MIT/repeat charge is routed
        // through on the HS side, via the Authorize flow): any FAILED/CANCELED/UNKNOWN
        // state is a failure, not only when Finix happens to echo a failure_message.
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
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            // Same reference id as Authorize: a transfer is its own id; an authorization is
            // referenced by its transfer once captured, else by itself.
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
                    // Echo the CIT transfer id so the next MIT still has it.
                    mandate_metadata: get_finix_mandate_metadata(
                        &item.router_data.request.mandate_reference,
                    ),
                })),
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response_data,
                ..item.router_data.resource_common_data.clone()
            },
            response: flow_response,
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

/// Webhook transaction state. Mirrors HS `FinixState` (transformers/request.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixState {
    Pending,
    Succeeded,
    Failed,
    Canceled,
    #[serde(other)]
    Unknown,
}

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
    pub state: FinixState,
    pub failure_code: Option<String>,
    pub messages: Option<Vec<String>>,
    pub failure_message: Option<String>,
    pub transfer: Option<String>,
    pub tags: FinixTags,
    #[serde(rename = "type")]
    pub payment_type: Option<FinixPaymentType>,
    pub three_d_secure: Option<FinixThreeDSecure>,
    pub address_verification: Option<String>,
    pub network_details: Option<FinixNetworkDetails>,
}

/// Mirrors HS `FinixDisputeState` (transformers/response.rs).
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

/// Dispute `response_state` (merchant-side response progress).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixDisputeResponseState {
    NeedsResponse,
    Responded,
    Accepted,
    NoResponseAllowed,
    #[serde(other)]
    Unknown,
}

/// The Dispute resource returned by `/disputes/{id}/accept` and `/disputes/{id}/submit`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixDisputeResponse {
    pub id: String,
    pub state: FinixDisputeState,
    pub response_state: Option<FinixDisputeResponseState>,
    pub transfer: Option<String>,
    pub amount: Option<MinorUnit>,
    pub evidence_submitted: Option<String>,
}

/// `response_state` first (it moves on accept / submit before `state` does), then `state`.
pub fn get_finix_dispute_status(response: &FinixDisputeResponse) -> common_enums::DisputeStatus {
    match response.response_state {
        Some(FinixDisputeResponseState::Accepted) => common_enums::DisputeStatus::DisputeAccepted,
        Some(FinixDisputeResponseState::Responded) => {
            common_enums::DisputeStatus::DisputeChallenged
        }
        Some(
            FinixDisputeResponseState::NeedsResponse
            | FinixDisputeResponseState::NoResponseAllowed
            | FinixDisputeResponseState::Unknown,
        )
        | None => match response.state {
            FinixDisputeState::Pending | FinixDisputeState::Unknown => {
                common_enums::DisputeStatus::DisputeOpened
            }
            FinixDisputeState::Inquiry => common_enums::DisputeStatus::DisputeChallenged,
            FinixDisputeState::Won => common_enums::DisputeStatus::DisputeWon,
            FinixDisputeState::Lost => common_enums::DisputeStatus::DisputeLost,
        },
    }
}

/// G-Accept-01 (and every `/disputes/{id}/..` path): the Finix dispute id is required.
pub fn get_connector_dispute_id(connector_dispute_id: &str) -> Result<&str, IntegrationError> {
    if connector_dispute_id.trim().is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "connector_dispute_id",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Pass the Finix dispute id (DI...) from the dispute webhook or GET /disputes as dispute_id.".to_string(),
                ),
                doc_url: Some("https://docs.finix.com/api/disputes".to_string()),
                additional_context: Some(
                    "Finix dispute actions are addressed as /disputes/{dispute_id}/...".to_string(),
                ),
            },
        });
    }
    Ok(connector_dispute_id)
}

// ACCEPT DISPUTE - REQUEST/RESPONSE

#[derive(Debug, Serialize)]
pub struct FinixAcceptDisputeRequest {
    /// Required by Finix; no UCS input carries it, so it is derived (plan UD-01).
    pub note: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    > for FinixAcceptDisputeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let connector_dispute_id =
            get_connector_dispute_id(&item.router_data.request.connector_dispute_id)?;
        Ok(Self {
            note: format!("Accepting dispute {connector_dispute_id}"),
        })
    }
}

pub type FinixAcceptDisputeResponse = FinixDisputeResponse;

impl TryFrom<ResponseRouterData<FinixAcceptDisputeResponse, Self>>
    for RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixAcceptDisputeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let dispute_status = get_finix_dispute_status(&response);
        // Wire spelling (e.g. "ACCEPTED"), not the Rust variant name.
        let connector_dispute_status = response
            .response_state
            .and_then(|state| serde_json::to_value(state).ok())
            .and_then(|value| value.as_str().map(str::to_string));
        Ok(Self {
            response: Ok(DisputeResponseData {
                connector_dispute_id: response.id,
                dispute_status,
                connector_dispute_status,
                submitted_evidence_ids: vec![],
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// DEFEND DISPUTE - REQUEST/RESPONSE

#[derive(Debug, Serialize)]
pub struct FinixDefendDisputeRequest {
    /// Required by Finix: explains the uploaded evidence and the rebuttal.
    pub note: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
            T,
        >,
    > for FinixDefendDisputeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        get_connector_dispute_id(&request.connector_dispute_id)?;
        // G-DefendDispute-01: `note` is required and has no safe default text.
        if request.defense_reason_code.trim().is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "defense_reason_code",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the rebuttal narrative as defense_reason_code; Finix sends it as the submission note.".to_string(),
                    ),
                    doc_url: Some(FINIX_DISPUTE_EVIDENCE_DOC_URL.to_string()),
                    additional_context: Some(
                        "POST /disputes/{dispute_id}/submit requires a non-empty note.".to_string(),
                    ),
                },
            }
            .into());
        }
        Ok(Self {
            note: request.defense_reason_code.clone(),
        })
    }
}

/// Live sandbox: `/submit` answers 202 with the Dispute resource (response_state RESPONDED).
pub type FinixDefendDisputeResponse = FinixDisputeResponse;

impl TryFrom<ResponseRouterData<FinixDefendDisputeResponse, Self>>
    for RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixDefendDisputeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let dispute_status = get_finix_dispute_status(&response);
        // Wire spelling (e.g. "RESPONDED"), not the Rust variant name.
        let connector_dispute_status = response
            .response_state
            .and_then(|state| serde_json::to_value(state).ok())
            .and_then(|value| value.as_str().map(str::to_string));
        Ok(Self {
            response: Ok(DisputeResponseData {
                connector_dispute_id: response.id,
                dispute_status,
                connector_dispute_status,
                submitted_evidence_ids: vec![],
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// SUBMIT EVIDENCE - REQUEST/RESPONSE

const FINIX_DISPUTE_EVIDENCE_DOC_URL: &str = "https://docs.finix.com/api/disputes";

/// One evidence file for `POST /disputes/{id}/evidence` (multipart field `file`).
#[derive(Debug, Serialize)]
pub struct FinixSubmitEvidenceRequest {
    pub file_name: String,
    pub mime_type: String,
    /// Document bytes: sent as the multipart part, kept out of the logged request.
    #[serde(skip)]
    pub file: Vec<u8>,
}

impl GetFormData for FinixSubmitEvidenceRequest {
    fn get_form_data(&self) -> MultipartData {
        let mut form = MultipartData::new();
        form.add_file(
            "file",
            self.file_name.clone(),
            self.file.clone(),
            self.mime_type.clone(),
        );
        form
    }
}

/// Finix accepts JPEG, PDF and PNG evidence only; returns the file extension.
fn finix_evidence_extension(mime_type: &str) -> Option<&'static str> {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "application/pdf" => Some("pdf"),
        _ => None,
    }
}

fn evidence_context(suggested_action: &str) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_string()),
        doc_url: Some(FINIX_DISPUTE_EVIDENCE_DOC_URL.to_string()),
        additional_context: Some(
            "Finix uploads exactly one JPEG/PDF/PNG file (max 1 MB) per POST /disputes/{dispute_id}/evidence.".to_string(),
        ),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::FinixRouterData<
            RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
            T,
        >,
    > for FinixSubmitEvidenceRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::FinixRouterData<
            RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        get_connector_dispute_id(&request.connector_dispute_id)?;
        let documents = [
            (
                "cancellation_policy",
                &request.cancellation_policy,
                &request.cancellation_policy_file_type,
            ),
            (
                "customer_communication",
                &request.customer_communication,
                &request.customer_communication_file_type,
            ),
            (
                "customer_signature",
                &request.customer_signature,
                &request.customer_signature_file_type,
            ),
            ("receipt", &request.receipt, &request.receipt_file_type),
            (
                "refund_policy",
                &request.refund_policy,
                &request.refund_policy_file_type,
            ),
            (
                "service_documentation",
                &request.service_documentation,
                &request.service_documentation_file_type,
            ),
            (
                "shipping_documentation",
                &request.shipping_documentation,
                &request.shipping_documentation_file_type,
            ),
            (
                "invoice_showing_distinct_transactions",
                &request.invoice_showing_distinct_transactions,
                &request.invoice_showing_distinct_transactions_file_type,
            ),
            (
                "recurring_transaction_agreement",
                &request.recurring_transaction_agreement,
                &request.recurring_transaction_agreement_file_type,
            ),
            (
                "uncategorized_file",
                &request.uncategorized_file,
                &request.uncategorized_file_type,
            ),
        ];
        let mut present = documents
            .into_iter()
            .filter_map(|(name, bytes, mime_type)| {
                bytes
                    .as_ref()
                    .filter(|bytes| !bytes.is_empty())
                    .map(|bytes| (name, bytes, mime_type))
            });

        // G-SubmitEvidence-01: Finix needs one file per request.
        let (name, bytes, mime_type) =
            present
                .next()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "evidence_documents.file_content",
                    context: evidence_context(
                        "Send one evidence document with file_content and file_mime_type.",
                    ),
                })?;
        // G-SubmitEvidence-02: never silently drop a document.
        if present.next().is_some() {
            return Err(IntegrationError::NotSupported {
                message: "Finix accepts one evidence file per SubmitEvidence request".to_string(),
                connector: "finix",
                context: evidence_context("Call SubmitEvidence once per document."),
            }
            .into());
        }
        // G-SubmitEvidence-03: JPEG/PDF/PNG only.
        let mime_type = mime_type
            .as_deref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "evidence_documents.file_mime_type",
                context: evidence_context(
                    "Set file_mime_type to image/jpeg, image/png or application/pdf.",
                ),
            })?;
        let extension =
            finix_evidence_extension(mime_type).ok_or(IntegrationError::NotSupported {
                message: format!("Finix does not accept evidence of type {mime_type}"),
                connector: "finix",
                context: evidence_context("Convert the document to JPEG, PNG or PDF."),
            })?;
        Ok(Self {
            file_name: format!("{name}.{extension}"),
            mime_type: mime_type.to_string(),
            file: bytes.clone(),
        })
    }
}

/// Evidence `state`: PENDING until Finix validates the file (~15 s), then SUCCEEDED.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixEvidenceState {
    Pending,
    Succeeded,
    Canceled,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixSubmitEvidenceResponse {
    pub id: String,
    pub dispute: String,
    pub state: FinixEvidenceState,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}

impl TryFrom<ResponseRouterData<FinixSubmitEvidenceResponse, Self>>
    for RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FinixSubmitEvidenceResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let response_data = match response.state {
            FinixEvidenceState::Failed | FinixEvidenceState::Canceled => Err(ErrorResponse {
                code: response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: response
                    .failure_message
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: response.failure_message.clone(),
                status_code: item.http_code,
                attempt_status: None,
                connector_transaction_id: Some(response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            }),
            // Uploading evidence does not move the dispute; it stays open until submitted.
            FinixEvidenceState::Pending
            | FinixEvidenceState::Succeeded
            | FinixEvidenceState::Unknown => Ok(DisputeResponseData {
                connector_dispute_id: response.dispute.clone(),
                dispute_status: common_enums::DisputeStatus::DisputeOpened,
                // Wire spelling (e.g. "PENDING"), not the Rust variant name.
                connector_dispute_status: serde_json::to_value(response.state)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string)),
                submitted_evidence_ids: vec![response.id.clone()],
                status_code: item.http_code,
            }),
        };
        Ok(Self {
            response: response_data,
            ..item.router_data
        })
    }
}

/// Mirrors HS `FinixDisputes` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixDisputes {
    pub transfer: String,
    pub reason: Option<String>,
    pub amount: MinorUnit,
    pub state: FinixDisputeState,
    pub currency: Currency,
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

/// Mirrors HS `FinixEmbedded` (transformers/response.rs). Untagged: variant order
/// (Authorizations, Transfers, Disputes) matches HS so ambiguous payloads resolve
/// identically.
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
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::PaymentIntentCancelFailure)
                    }
                    FinixState::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixState::Succeeded => Ok(EventType::PaymentIntentCancelled),
                }
            } else {
                match authorization.state {
                    FinixState::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixState::Succeeded => Ok(EventType::PaymentIntentAuthorizationSuccess),
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::PaymentIntentAuthorizationFailure)
                    }
                }
            }
        }
        FinixEmbedded::Transfers { transfers } => {
            let transfer = transfers.get_first_event()?;

            if transfer.payment_type == Some(FinixPaymentType::Reversal) {
                match transfer.state {
                    FinixState::Succeeded => Ok(EventType::RefundSuccess),
                    // HS: `EventNotSupported` (see doc comment above).
                    FinixState::Pending => Ok(EventType::IncomingWebhookEventUnspecified),
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::RefundFailure)
                    }
                }
            } else {
                match transfer.state {
                    FinixState::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixState::Succeeded => Ok(EventType::PaymentIntentSuccess),
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::PaymentIntentFailure)
                    }
                }
            }
        }
        FinixEmbedded::Disputes { disputes } => {
            let dispute = disputes.get_first_event()?;

            match dispute.state {
                FinixDisputeState::Pending | FinixDisputeState::Unknown => {
                    Ok(EventType::DisputeOpened)
                }
                FinixDisputeState::Inquiry => Ok(EventType::DisputeChallenged),
                FinixDisputeState::Lost => Ok(EventType::DisputeLost),
                FinixDisputeState::Won => Ok(EventType::DisputeWon),
            }
        }
    }
}

/// Ports HS `FinixWebhookBody::get_webhook_object_reference_id`:
/// - Authorizations -> `PaymentId(ConnectorTransactionId(authorization.id))`
/// - Transfers (REVERSAL) -> `RefundId(ConnectorRefundId(transfer.id))`
/// - Transfers (FEE) -> error (HS: `WebhookEventTypeNotFound`, platform fee ignored)
/// - Transfers (other) -> `PaymentId(ConnectorTransactionId(transfer.id))`
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
                    connector_preprocessing_id: None,
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
                        merchant_transaction_id: None,
                    },
                ))),
                // finix platform fee ignored (HS: Err(WebhookEventTypeNotFound))
                Some(FinixPaymentType::Fee) => Err(error_stack::report!(
                    WebhookError::WebhookReferenceIdNotFound
                )),
                _ => Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(transfer.id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    },
                ))),
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
    }
}

/// Which HS flow a webhook payment resource belongs to (`FinixFlow` in HS).
/// Webhooks only ever carry authorizations or transfers, so HS's `Capture` arm
/// is unreachable from the webhook path and is not ported.
#[derive(Clone, Copy, Debug)]
pub(super) enum FinixWebhookFlow {
    Auth,
    Transfer,
}

/// Ports HS `get_attempt_status` (finix/transformers.rs) for the webhook path.
fn get_finix_webhook_attempt_status(
    state: &FinixState,
    flow: FinixWebhookFlow,
    is_void: Option<bool>,
) -> AttemptStatus {
    if is_void == Some(true) {
        return match state {
            FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                AttemptStatus::VoidFailed
            }
            FinixState::Pending => AttemptStatus::Voided,
            FinixState::Succeeded => AttemptStatus::Voided,
        };
    }
    match (flow, state) {
        (FinixWebhookFlow::Auth, FinixState::Pending) => AttemptStatus::AuthenticationPending,
        (FinixWebhookFlow::Auth, FinixState::Succeeded) => AttemptStatus::Authorized,
        (
            FinixWebhookFlow::Auth,
            FinixState::Failed | FinixState::Canceled | FinixState::Unknown,
        ) => AttemptStatus::AuthorizationFailed,
        (FinixWebhookFlow::Transfer, FinixState::Pending) => AttemptStatus::Pending,
        (FinixWebhookFlow::Transfer, FinixState::Succeeded) => AttemptStatus::Charged,
        (
            FinixWebhookFlow::Transfer,
            FinixState::Failed | FinixState::Canceled | FinixState::Unknown,
        ) => AttemptStatus::Failure,
    }
}

/// Ports HS `impl From<FinixState> for RefundStatus` (finix/transformers.rs).
fn get_finix_webhook_refund_status(state: &FinixState) -> RefundStatus {
    match state {
        FinixState::Pending => RefundStatus::Pending,
        FinixState::Succeeded => RefundStatus::Success,
        FinixState::Failed | FinixState::Canceled | FinixState::Unknown => RefundStatus::Failure,
    }
}

/// Dispute status per the HS webhook event mapping (PENDING -> DisputeOpened,
/// INQUIRY -> DisputeChallenged, LOST -> DisputeLost, WON -> DisputeWon).
fn get_finix_webhook_dispute_status(state: &FinixDisputeState) -> common_enums::DisputeStatus {
    match state {
        FinixDisputeState::Pending | FinixDisputeState::Unknown => {
            common_enums::DisputeStatus::DisputeOpened
        }
        FinixDisputeState::Inquiry => common_enums::DisputeStatus::DisputeChallenged,
        FinixDisputeState::Lost => common_enums::DisputeStatus::DisputeLost,
        FinixDisputeState::Won => common_enums::DisputeStatus::DisputeWon,
    }
}

/// Builds the payment webhook content for authorizations and (non-reversal)
/// transfers. Status mapping ports HS `get_attempt_status` with
/// `FinixFlow::Auth` / `FinixFlow::Transfer` respectively.
pub(super) fn build_finix_payment_webhook_response(
    connector: &dyn common_utils::types::AmountConvertor<Output = MinorUnit>,
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let (resource, flow) = match &body.webhook_embedded {
        FinixEmbedded::Authorizations { authorizations } => {
            (authorizations.get_first_event()?, FinixWebhookFlow::Auth)
        }
        FinixEmbedded::Transfers { transfers } => {
            (transfers.get_first_event()?, FinixWebhookFlow::Transfer)
        }
        FinixEmbedded::Disputes { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a payment webhook, but found a dispute webhook");
        }
    };

    let status = get_finix_webhook_attempt_status(&resource.state, flow, resource.is_void);

    Ok(WebhookDetailsResponse {
        connector_returned_payment_method_details: None,
        resource_id: Some(ResponseId::ConnectorTransactionId(resource.id.clone())),
        status,
        connector_response_reference_id: Some(resource.id.clone()),
        connector_request_reference_id: Some(resource.id),
        mandate_reference: None,
        error_code: resource.failure_code,
        error_message: resource.failure_message.clone(),
        error_reason: resource.failure_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: resource
            .captured_amount
            .map(|amount| {
                crate::utils::webhook_amount_to_money(
                    connector.convert_back(amount, resource.currency),
                    resource.currency,
                )
            })
            .transpose()?,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}

/// Builds the refund webhook content for REVERSAL transfers. Status mapping
/// ports HS `impl From<FinixState> for RefundStatus`.
pub(super) fn build_finix_refund_webhook_response(
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let transfer = match &body.webhook_embedded {
        FinixEmbedded::Transfers { transfers } => transfers.get_first_event()?,
        FinixEmbedded::Authorizations { .. } | FinixEmbedded::Disputes { .. } => {
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
        status: get_finix_webhook_refund_status(&transfer.state),
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
        FinixEmbedded::Authorizations { .. } | FinixEmbedded::Transfers { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a dispute webhook, but found other webhooks");
        }
    };

    Ok(DisputeWebhookDetailsResponse {
        amount: domain_types::utils::convert_amount_for_webhook(
            &common_utils::types::StringMinorUnitForConnector,
            &common_utils::types::Money {
                amount: dispute.amount,
                currency: dispute.currency,
            },
        )?,
        currency: dispute.currency,
        dispute_id: dispute.id,
        status: get_finix_webhook_dispute_status(&dispute.state),
        stage: common_enums::DisputeStage::Dispute,
        connector_response_reference_id: None,
        dispute_message: dispute.reason,
        connector_reason_code: None,
        additional_details: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
