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

/// Barclaycard Flex session response — the capture context JWT for SDK initialization.
/// The Flex v2 sessions endpoint returns a raw JWT string with content-type application/jwt,
/// so we implement a custom Deserialize that handles both raw strings and JSON objects.
#[derive(Debug, Serialize)]
pub struct BarclaycardClientAuthResponse {
    pub capture_context: String,
}

impl<'de> Deserialize<'de> for BarclaycardClientAuthResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BarclaycardClientAuthVisitor;

        impl<'de> serde::de::Visitor<'de> for BarclaycardClientAuthVisitor {
            type Value = BarclaycardClientAuthResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JWT string or a JSON object with keyId")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(BarclaycardClientAuthResponse {
                    capture_context: v.to_string(),
                })
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(BarclaycardClientAuthResponse { capture_context: v })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut key_id = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "keyId" {
                        key_id = Some(map.next_value::<String>()?);
                    } else {
                        let _ = map.next_value::<serde_json::Value>()?;
                    }
                }
                Ok(BarclaycardClientAuthResponse {
                    capture_context: key_id.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_any(BarclaycardClientAuthVisitor)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum BarclaycardErrorResponse {
    Authentication(BarclaycardAuthenticationErrorResponse),
    Server(BarclaycardServerErrorResponse),
    Standard(BarclaycardStandardErrorResponse),
}
