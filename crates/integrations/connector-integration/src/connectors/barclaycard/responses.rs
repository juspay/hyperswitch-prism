use common_utils::types::SemanticVersion;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardErrorInformationResponse {
    pub id: String,
    pub error_information: BarclaycardErrorInformation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BarclaycardErrorInformation {
    pub reason: Option<String>,
    pub message: Option<String>,
    pub details: Option<Vec<Details>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub field: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BarclaycardPaymentsResponse {
    ClientReferenceInformation(Box<BarclaycardClientReferenceResponse>),
    ErrorInformation(Box<BarclaycardErrorInformationResponse>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardClientReferenceResponse {
    pub id: String,
    pub status: Option<BarclaycardPaymentStatus>,
    pub client_reference_information: ClientReferenceInformation,
    pub processor_information: Option<ClientProcessorInformation>,
    pub risk_information: Option<ClientRiskInformation>,
    pub error_information: Option<BarclaycardErrorInformation>,
    pub token_information: Option<BarclaycardTokenInformation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardTokenInformation {
    pub payment_instrument: Option<BarclaycardPaymentInstrument>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BarclaycardPaymentInstrument {
    pub id: Secret<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientReferenceInformation {
    pub code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BarclaycardPaymentStatus {
    Authorized,
    Succeeded,
    Failed,
    Voided,
    Reversed,
    Pending,
    Declined,
    Rejected,
    AuthorizedPendingReview,
    AuthorizedRiskDeclined,
    Transmitted,
    InvalidRequest,
    ServerError,
    PendingReview,
    Cancelled,
    StatusNotReceived,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientProcessorInformation {
    pub avs: Option<Avs>,
    pub card_verification: Option<CardVerification>,
    pub network_transaction_id: Option<Secret<String>>,
    pub approval_code: Option<String>,
    pub merchant_advice: Option<MerchantAdvice>,
    pub response_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantAdvice {
    pub code: Option<String>,
    pub code_raw: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardVerification {
    pub result_code: Option<String>,
    pub result_code_raw: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Avs {
    pub code: Option<String>,
    pub code_raw: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientRiskInformation {
    pub rules: Option<Vec<ClientRiskInformationRules>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientRiskInformationRules {
    pub name: Option<Secret<String>>,
}

pub type BarclaycardAuthorizeResponse = BarclaycardPaymentsResponse;
pub type BarclaycardCaptureResponse = BarclaycardPaymentsResponse;
pub type BarclaycardVoidResponse = BarclaycardPaymentsResponse;
pub type BarclaycardRepeatPaymentResponse = BarclaycardPaymentsResponse;

// SetupMandate response - same as payments response but includes tokenInformation
pub type BarclaycardSetupMandateResponse = BarclaycardPaymentsResponse;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardTransactionResponse {
    pub id: String,
    pub application_information: ApplicationInformation,
    pub client_reference_information: Option<ClientReferenceInformation>,
    pub processor_information: Option<ClientProcessorInformation>,
    pub error_information: Option<BarclaycardErrorInformation>,
    pub risk_information: Option<ClientRiskInformation>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInformation {
    pub status: Option<BarclaycardPaymentStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardRefundResponse {
    pub id: String,
    pub status: BarclaycardRefundStatus,
    pub error_information: Option<BarclaycardErrorInformation>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BarclaycardRefundStatus {
    Succeeded,
    Transmitted,
    Failed,
    Pending,
    Voided,
    Cancelled,
    #[serde(rename = "201")]
    TwoZeroOne,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardRsyncResponse {
    pub id: String,
    pub application_information: Option<RsyncApplicationInformation>,
    pub error_information: Option<BarclaycardErrorInformation>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RsyncApplicationInformation {
    pub status: Option<BarclaycardRefundStatus>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardStandardErrorResponse {
    pub error_information: Option<ErrorInformation>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
    pub details: Option<Vec<Details>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ErrorInformation {
    pub message: String,
    pub reason: String,
    pub details: Option<Vec<Details>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardServerErrorResponse {
    pub status: Option<String>,
    pub message: Option<String>,
    pub reason: Option<Reason>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Reason {
    SystemError,
    ServerTimeout,
    ServiceTimeout,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BarclaycardAuthenticationErrorResponse {
    pub response: AuthenticationErrorInformation,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AuthenticationErrorInformation {
    pub rmsg: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum BarclaycardErrorResponse {
    Authentication(BarclaycardAuthenticationErrorResponse),
    Server(BarclaycardServerErrorResponse),
    Standard(BarclaycardStandardErrorResponse),
}

// --- 3DS External Authentication responses ---
// Mirror Cybersource's AuthSetup / Authenticate / PostAuthenticate responses.
// `BarclaycardErrorInformationResponse` is already defined at the top of this module and
// is reused as the error arm of the auth-flow response enums below.

/// PreAuthenticate (`risk/v1/authentication-setups`) response: carries the access token,
/// device-data-collection URL and reference id used to drive the DDC redirect.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardConsumerAuthInformationResponse {
    pub access_token: Secret<String>,
    pub device_data_collection_url: String,
    pub reference_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAuthSetupInfoResponse {
    pub id: String,
    pub client_reference_information: ClientReferenceInformation,
    pub consumer_authentication_information: BarclaycardConsumerAuthInformationResponse,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BarclaycardAuthSetupResponse {
    ClientAuthSetupInfo(Box<ClientAuthSetupInfoResponse>),
    ErrorInformation(Box<BarclaycardErrorInformationResponse>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum BarclaycardParesStatus {
    #[serde(rename = "C")]
    CardChallenged,
    #[serde(rename = "R")]
    AuthenticationRejected,
    #[serde(rename = "Y")]
    AuthenticationSuccessful,
    #[serde(rename = "A")]
    AuthenticationAttempted,
    #[serde(rename = "N")]
    AuthenticationFailed,
    #[serde(rename = "U")]
    AuthenticationNotCompleted,
}

impl From<BarclaycardParesStatus> for common_enums::TransactionStatus {
    fn from(status: BarclaycardParesStatus) -> Self {
        match status {
            BarclaycardParesStatus::AuthenticationSuccessful => Self::Success,
            BarclaycardParesStatus::AuthenticationAttempted => Self::NotVerified,
            BarclaycardParesStatus::AuthenticationFailed => Self::Failure,
            BarclaycardParesStatus::AuthenticationNotCompleted => Self::VerificationNotPerformed,
            BarclaycardParesStatus::CardChallenged => Self::ChallengeRequired,
            BarclaycardParesStatus::AuthenticationRejected => Self::Rejected,
        }
    }
}

/// Enrollment status returned by the Authenticate (`risk/v1/authentications`) call.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BarclaycardAuthEnrollmentStatus {
    PendingAuthentication,
    AuthenticationSuccessful,
    AuthenticationFailed,
}

impl From<BarclaycardAuthEnrollmentStatus> for common_enums::AttemptStatus {
    fn from(item: BarclaycardAuthEnrollmentStatus) -> Self {
        match item {
            BarclaycardAuthEnrollmentStatus::PendingAuthentication => Self::AuthenticationPending,
            BarclaycardAuthEnrollmentStatus::AuthenticationSuccessful => {
                Self::AuthenticationSuccessful
            }
            BarclaycardAuthEnrollmentStatus::AuthenticationFailed => Self::AuthenticationFailed,
        }
    }
}

/// Flattened validate fields shared by the Authenticate and PostAuthenticate responses.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardConsumerAuthValidateResponse {
    /// Payer authentication response status. Applicable for Mastercard and Visa.
    pub pares_status: Option<BarclaycardParesStatus>,
    pub ucaf_collection_indicator: Option<String>,
    pub cavv: Option<Secret<String>>,
    pub ucaf_authentication_data: Option<Secret<String>>,
    pub xid: Option<String>,
    pub specification_version: Option<SemanticVersion>,
    pub directory_server_transaction_id: Option<Secret<String>>,
    pub acs_transaction_id: Option<String>,
    pub three_d_s_server_transaction_id: Option<String>,
    pub indicator: Option<String>,
    pub ecommerce_indicator: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarclaycardConsumerAuthInformationEnrollmentResponse {
    pub access_token: Option<Secret<String>>,
    pub step_up_url: Option<String>,
    pub authentication_transaction_id: Option<String>,
    // Flattened so three_ds_data is segregated into its own struct.
    #[serde(flatten)]
    pub validate_response: BarclaycardConsumerAuthValidateResponse,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAuthCheckInfoResponse {
    pub id: String,
    pub client_reference_information: ClientReferenceInformation,
    pub consumer_authentication_information: BarclaycardConsumerAuthInformationEnrollmentResponse,
    pub status: BarclaycardAuthEnrollmentStatus,
    pub error_information: Option<BarclaycardErrorInformation>,
}

/// Authenticate (`risk/v1/authentications`) response. Reused for PostAuthenticate
/// (`risk/v1/authentication-results`), mirroring Cybersource.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BarclaycardAuthenticateResponse {
    ClientAuthCheckInfo(Box<ClientAuthCheckInfoResponse>),
    ErrorInformation(Box<BarclaycardErrorInformationResponse>),
}

pub type BarclaycardPostAuthenticateResponse = BarclaycardAuthenticateResponse;
