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
