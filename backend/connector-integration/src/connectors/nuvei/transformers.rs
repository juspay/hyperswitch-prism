use common_utils::{pii, request::Method, types::StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors,
    payment_method_data::{
        BankRedirectData, BankTransferData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use super::NuveiRouterData;
use crate::types::ResponseRouterData;

// Auth Type
#[derive(Debug, Clone)]
pub struct NuveiAuthType {
    pub(super) merchant_id: Secret<String>,
    pub(super) merchant_site_id: Secret<String>,
    pub(super) merchant_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NuveiAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nuvei {
                merchant_id,
                merchant_site_id,
                merchant_secret,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.clone(),
                merchant_site_id: merchant_site_id.clone(),
                merchant_secret: merchant_secret.clone(),
            }),
            _ => Err(errors::ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

impl NuveiAuthType {
    pub fn generate_checksum(&self, params: &[&str]) -> String {
        use sha2::{Digest, Sha256};

        let mut concatenated = params.join("");
        concatenated.push_str(self.merchant_secret.peek());

        let mut hasher = Sha256::new();
        hasher.update(concatenated.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn get_timestamp(
    ) -> common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss> {
        // Generate timestamp in YYYYMMDDHHmmss format using common_utils date_time
        common_utils::date_time::DateTime::from(common_utils::date_time::now())
    }
}

// Session Token Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Session Token Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenResponse {
    pub session_token: Option<String>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
    pub version: Option<String>,
    pub client_request_id: Option<String>,
}

// URL Details for redirect URLs
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiUrlDetails {
    pub success_url: String,
    pub failure_url: String,
    pub pending_url: String,
}

// Payment Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Option<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_token_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_unique_id: Option<String>,
    pub payment_option: NuveiPaymentOption<T>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<NuveiCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternative_payment_method: Option<NuveiAlternativePaymentMethod>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    pub card_holder_name: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlternativePaymentMethodType {
    #[serde(rename = "apmgw_Giropay")]
    Giropay,
    #[serde(rename = "apmgw_Sofort")]
    Sofort,
    #[serde(rename = "apmgw_iDeal")]
    Ideal,
    #[serde(rename = "apmgw_EPS")]
    Eps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NuveiBIC {
    #[serde(rename = "ABNANL2A")]
    Abnamro,
    #[serde(rename = "ASNBNL21")]
    AsnBank,
    #[serde(rename = "BUNQNL2A")]
    Bunq,
    #[serde(rename = "INGBNL2A")]
    Ing,
    #[serde(rename = "KNABNL2H")]
    Knab,
    #[serde(rename = "RABONL2U")]
    Rabobank,
    #[serde(rename = "RBRBNL21")]
    Regiobank,
    #[serde(rename = "SNSBNL2A")]
    SnsBank,
    #[serde(rename = "TRIONL2U")]
    TriodosBank,
    #[serde(rename = "FVLBNL22")]
    VanLanschotBankiers,
    #[serde(rename = "MOYONL21")]
    Moneyou,
}

impl TryFrom<common_enums::BankNames> for NuveiBIC {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(bank: common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank {
            common_enums::BankNames::AbnAmro => Ok(Self::Abnamro),
            common_enums::BankNames::AsnBank => Ok(Self::AsnBank),
            common_enums::BankNames::Bunq => Ok(Self::Bunq),
            common_enums::BankNames::Ing => Ok(Self::Ing),
            common_enums::BankNames::Knab => Ok(Self::Knab),
            common_enums::BankNames::Rabobank => Ok(Self::Rabobank),
            common_enums::BankNames::Regiobank => Ok(Self::Regiobank),
            common_enums::BankNames::SnsBank => Ok(Self::SnsBank),
            common_enums::BankNames::TriodosBank => Ok(Self::TriodosBank),
            common_enums::BankNames::VanLanschot => Ok(Self::VanLanschotBankiers),
            common_enums::BankNames::Moneyou => Ok(Self::Moneyou),
            _ => Err(errors::ConnectorError::NotImplemented(format!(
                "Bank not supported by Nuvei iDEAL: {}",
                bank
            ))
            .into()),
        }
    }
}
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiAlternativePaymentMethod {
    Ach {
        #[serde(rename = "paymentMethod")]
        payment_method: String,
        #[serde(rename = "AccountNumber")]
        account_number: Secret<String>,
        #[serde(rename = "RoutingNumber")]
        routing_number: Secret<String>,
        #[serde(rename = "SECCode", skip_serializing_if = "Option::is_none")]
        sec_code: Option<String>,
    },
    Redirect {
        #[serde(rename = "paymentMethod")]
        payment_method: AlternativePaymentMethodType,
        #[serde(rename = "BIC")]
        bank_id: Option<NuveiBIC>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiDeviceDetails {
    pub ip_address: Secret<String, pii::IpAddress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiBillingAddress {
    // Required fields per Nuvei documentation
    pub email: pii::Email,
    pub country: String,
    // Optional fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
}

// Payment Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    pub auth_code: Option<String>,
    pub session_token: Option<String>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    #[serde(rename = "paymentOption")]
    pub payment_option: Option<PaymentOption>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentOption {
    #[serde(rename = "redirectUrl")]
    pub redirect_url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiPaymentStatus {
    Success,
    Failed,
    Error,
    #[default]
    Processing,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiTransactionStatus {
    #[serde(alias = "Approved", alias = "APPROVED")]
    Approved,
    #[serde(alias = "Declined", alias = "DECLINED")]
    Declined,
    #[serde(alias = "Filter Error", alias = "ERROR", alias = "Error")]
    Error,
    #[serde(alias = "Redirect", alias = "REDIRECT")]
    Redirect,
    #[serde(alias = "Pending", alias = "PENDING")]
    Pending,
    #[serde(alias = "Processing", alias = "PROCESSING")]
    #[default]
    Processing,
}

// Transaction Type for initPayment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum TransactionType {
    Auth,
    #[default]
    Sale,
}

impl TransactionType {
    fn get_from_capture_method(
        capture_method: Option<common_enums::CaptureMethod>,
        amount: &StringMajorUnit,
    ) -> Self {
        let amount_value = amount.get_amount_as_string().parse::<f64>();
        if capture_method == Some(common_enums::CaptureMethod::Manual) || amount_value == Ok(0.0) {
            Self::Auth
        } else {
            Self::Sale
        }
    }
}

// Sync Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Sync Response (getTransactionDetails has different structure than payment response)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncResponse {
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub internal_request_id: Option<i64>,
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
    pub version: Option<String>,
    pub transaction_details: Option<NuveiTransactionDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiTransactionDetails {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub auth_code: Option<String>,
    pub client_unique_id: Option<String>,
    pub date: Option<String>,
    pub original_transaction_date: Option<String>,
    pub credited: Option<String>,
    pub acquiring_bank_name: Option<String>,
    pub transaction_type: Option<String>,
}

// Capture Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCaptureRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Capture Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCaptureResponse {
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub transaction_id: Option<String>,
    pub status: NuveiPaymentStatus,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
}

// Refund Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Refund Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
}

// Refund Sync Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Refund Sync Response (separate type to avoid macro conflicts)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
}

// Void Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiVoidRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Void Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiVoidResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
}

// Error Response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiErrorResponse {
    pub reason: Option<String>,
    pub err_code: Option<String>,
    pub status: Option<String>,
}

// Session Token Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateSessionToken,
                PaymentFlowData,
                domain_types::connector_types::SessionTokenRequestData,
                domain_types::connector_types::SessionTokenResponseData,
            >,
            T,
        >,
    > for NuveiSessionTokenRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateSessionToken,
                PaymentFlowData,
                domain_types::connector_types::SessionTokenRequestData,
                domain_types::connector_types::SessionTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Generate checksum for getSessionToken: merchantId + merchantSiteId + clientRequestId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            time_stamp,
            checksum,
        })
    }
}

// Session Token Response Transformation
impl TryFrom<ResponseRouterData<NuveiSessionTokenResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::CreateSessionToken,
        PaymentFlowData,
        domain_types::connector_types::SessionTokenRequestData,
        domain_types::connector_types::SessionTokenResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiSessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract session token
        let session_token =
            response
                .session_token
                .clone()
                .ok_or(errors::ConnectorError::MissingRequiredField {
                    field_name: "session_token",
                })?;

        let session_response_data = domain_types::connector_types::SessionTokenResponseData {
            session_token: session_token.clone(),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Pending,
                session_token: Some(session_token),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(session_response_data),
            ..router_data.clone()
        })
    }
}

// Sync Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for NuveiSyncRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();

        // Per Hyperswitch pattern: ALWAYS send both transaction_id AND client_unique_id
        let client_unique_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(errors::ConnectorError::MissingConnectorTransactionID.into());
            }
        };

        // Generate checksum for getTransactionDetails: merchantId + merchantSiteId + transactionId + clientUniqueId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &transaction_id,
            &client_unique_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiPaymentRequest<T>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: NuveiRouterData<
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

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // Extract payment method data
        let payment_option = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = router_data
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .or(router_data.request.customer_name.clone().map(Secret::new))
                    .ok_or(errors::ConnectorError::MissingRequiredField {
                        field_name: "billing_address.first_name and billing_address.last_name or customer_name",
                    })?;

                NuveiPaymentOption {
                    card: Some(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                    }),
                    alternative_payment_method: None,
                }
            }
            PaymentMethodData::BankTransfer(bank_transfer_data) => {
                match bank_transfer_data.as_ref() {
                    BankTransferData::AchBankTransfer {} => {
                        // For ACH Bank Transfer, Nuvei requires account_number and routing_number
                        // These should be provided in the request metadata as ACH details
                        let metadata = router_data.request.metadata.as_ref().ok_or(
                            errors::ConnectorError::MissingRequiredField {
                                field_name: "metadata for ACH details",
                            },
                        )?;

                        let ach_data = metadata.peek().get("ach").ok_or(
                            errors::ConnectorError::MissingRequiredField {
                                field_name: "ach in metadata",
                            },
                        )?;

                        let account_number = ach_data
                            .get("account_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or(errors::ConnectorError::MissingRequiredField {
                                field_name: "account_number",
                            })?;

                        let routing_number = ach_data
                            .get("routing_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or(errors::ConnectorError::MissingRequiredField {
                                field_name: "routing_number",
                            })?;

                        let sec_code = ach_data
                            .get("sec_code")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .map(String::from);

                        NuveiPaymentOption {
                            card: None,
                            alternative_payment_method: Some(NuveiAlternativePaymentMethod::Ach {
                                payment_method: "apmgw_ACH".to_string(),
                                account_number: Secret::new(account_number.to_string()),
                                routing_number: Secret::new(routing_number.to_string()),
                                sec_code,
                            }),
                        }
                    }
                    other => {
                        return Err(errors::ConnectorError::NotSupported {
                            message: format!("{:?} is not supported for Nuvei", other),
                            connector: "nuvei",
                        }
                        .into())
                    }
                }
            }
            PaymentMethodData::BankRedirect(ref redirect_data) => {
                let payment_method = match redirect_data {
                    BankRedirectData::Eps { .. } => AlternativePaymentMethodType::Eps,
                    BankRedirectData::Giropay { .. } => AlternativePaymentMethodType::Giropay,
                    BankRedirectData::Ideal { bank_name } => {
                        if let Some(ref bank) = bank_name {
                            let _ = NuveiBIC::try_from(*bank)?;
                        }
                        AlternativePaymentMethodType::Ideal
                    }
                    BankRedirectData::Sofort { .. } => AlternativePaymentMethodType::Sofort,
                    other => {
                        return Err(errors::ConnectorError::NotSupported {
                            message: format!(
                                "Bank redirect method {:?} not supported by Nuvei",
                                other
                            ),
                            connector: "nuvei",
                        }
                        .into())
                    }
                };

                let bank_id = match redirect_data {
                    BankRedirectData::Ideal { bank_name } => bank_name
                        .as_ref()
                        .map(|bank| NuveiBIC::try_from(*bank))
                        .transpose()?,
                    _ => None,
                };

                NuveiPaymentOption {
                    card: None,
                    alternative_payment_method: Some(NuveiAlternativePaymentMethod::Redirect {
                        payment_method,
                        bank_id,
                    }),
                }
            }
            _ => {
                return Err(errors::ConnectorError::NotSupported {
                    message: "Payment method not supported".to_string(),
                    connector: "nuvei",
                }
                .into())
            }
        };

        // Extract billing address - Nuvei requires email, firstName, lastName, and country
        // Try to get email from billing, if not available, try from request email field
        let email = router_data
            .resource_common_data
            .get_optional_billing_email()
            .or_else(|| router_data.request.email.clone())
            .ok_or(errors::ConnectorError::MissingRequiredField {
                field_name: "billing_address.email",
            })?;

        let country = router_data
            .resource_common_data
            .get_optional_billing_country()
            .ok_or(errors::ConnectorError::MissingRequiredField {
                field_name: "billing_address.country",
            })?;

        // Get first and last name from billing (optional fields)
        let first_name = router_data
            .resource_common_data
            .get_optional_billing_first_name();

        let last_name = router_data
            .resource_common_data
            .get_optional_billing_last_name();

        // Use state code conversion (e.g., "California" -> "CA") for US/CA
        let state = router_data
            .resource_common_data
            .get_optional_billing_state();

        // Get address_line3 directly from billing address
        let address_line3 = router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing| billing.address.as_ref())
            .and_then(|addr| addr.line3.clone());

        let billing_address = NuveiBillingAddress {
            email,
            first_name,
            last_name,
            country: country.to_string(),
            phone: router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            city: router_data.resource_common_data.get_optional_billing_city(),
            address: router_data
                .resource_common_data
                .get_optional_billing_line1(),
            address_line2: router_data
                .resource_common_data
                .get_optional_billing_line2(),
            address_line3,
            zip: router_data.resource_common_data.get_optional_billing_zip(),
            state,
        };

        // Get device details - ipAddress is required by Nuvei
        let ip_address = router_data
            .request
            .browser_info
            .as_ref()
            .ok_or(errors::ConnectorError::MissingRequiredField {
                field_name: "browser_info",
            })?
            .ip_address
            .ok_or(errors::ConnectorError::MissingRequiredField {
                field_name: "browser_info.ip_address",
            })?;

        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address.to_string()),
        };

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        let currency = router_data.request.currency;

        // Extract session token from PaymentFlowData
        // The CreateSessionToken flow runs before Authorize and populates this field
        let session_token = router_data
            .resource_common_data
            .session_token
            .clone()
            .ok_or(errors::ConnectorError::MissingRequiredField {
                field_name: "session_token",
            })?;

        // Determine transaction type based on capture method
        let transaction_type =
            TransactionType::get_from_capture_method(router_data.request.capture_method, &amount);

        // Build urlDetails from router_return_url if available
        let url_details =
            router_data
                .request
                .router_return_url
                .as_ref()
                .map(|url| NuveiUrlDetails {
                    success_url: url.clone(),
                    failure_url: url.clone(),
                    pending_url: url.clone(),
                });

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            session_token: Some(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            user_token_id: None,
            client_unique_id: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            payment_option,
            transaction_type,
            device_details,
            billing_address,
            url_details,
            time_stamp,
            checksum,
        })
    }
}

// Response Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map transaction status to attempt status
        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => {
                if router_data.request.is_auto_capture()? {
                    common_enums::AttemptStatus::Charged
                } else {
                    common_enums::AttemptStatus::Authorized
                }
            }
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Redirect) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Pending
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Pending
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
        };

        // Get connector transaction ID
        let connector_transaction_id = response
            .transaction_id
            .clone()
            .or(response.order_id.clone())
            .ok_or(errors::ConnectorError::MissingConnectorTransactionID)?;

        let redirection_data = response
            .payment_option
            .as_ref()
            .and_then(|payment_option| payment_option.redirect_url.clone())
            .and_then(|url| Url::parse(&url).ok())
            .map(|url| Box::new(RedirectForm::from((url, Method::Get))));

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: response.client_request_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Capture Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NuveiCaptureRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(errors::ConnectorError::MissingConnectorTransactionID.into());
            }
        };

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        let currency = router_data.request.currency;

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId + amount + currency + relatedTransactionId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &client_unique_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &related_transaction_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// PSync Response Transformation
impl TryFrom<ResponseRouterData<NuveiSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: response
                        .transaction_details
                        .as_ref()
                        .and_then(|td| td.transaction_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract transaction details
        let transaction_details = response.transaction_details.as_ref().ok_or(
            errors::ConnectorError::MissingRequiredField {
                field_name: "transaction_details",
            },
        )?;

        // Map transaction status to attempt status
        let status = match transaction_details.transaction_status {
            Some(NuveiTransactionStatus::Approved) => {
                // For PSync, we need to determine if it was authorized or captured
                // Check transaction_type: "Auth" means authorized only, "Sale" means captured
                match transaction_details.transaction_type.as_deref() {
                    Some("Auth") => common_enums::AttemptStatus::Authorized,
                    Some("Sale") | Some("Settle") => common_enums::AttemptStatus::Charged,
                    _ => common_enums::AttemptStatus::Charged, // Default to Charged for unknown types
                }
            }
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Redirect) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Pending
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Pending
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
        };

        // Get connector transaction ID from transaction_details
        let connector_transaction_id = transaction_details
            .transaction_id
            .clone()
            .ok_or(errors::ConnectorError::MissingConnectorTransactionID)?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: transaction_details.client_unique_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Capture Response Transformation
impl TryFrom<ResponseRouterData<NuveiCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map transaction status to attempt status
        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::AttemptStatus::Charged,
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Charged
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Charged
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
        };

        // Get connector transaction ID
        let connector_transaction_id = response
            .transaction_id
            .clone()
            .ok_or(errors::ConnectorError::MissingConnectorTransactionID)?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Refund Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for NuveiRefundRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                common_utils::types::MinorUnit::new(router_data.request.refund_amount),
                router_data.request.currency,
            )
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        let currency = router_data.request.currency;

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId + amount + currency + relatedTransactionId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &client_unique_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &related_transaction_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Refund Sync Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for NuveiRefundSyncRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();

        // Per Hyperswitch pattern: ALWAYS send both transaction_id AND client_unique_id
        // NOTE: For RSync to work correctly, we need the ORIGINAL clientUniqueId from refund creation
        // Using current connector_request_reference_id may not match the original
        let client_unique_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let transaction_id = router_data.request.connector_transaction_id.clone();

        if transaction_id.is_empty() {
            return Err(errors::ConnectorError::MissingConnectorTransactionID.into());
        }

        // Generate checksum for getTransactionDetails: merchantId + merchantSiteId + transactionId + clientUniqueId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &transaction_id,
            &client_unique_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Refund Response Transformation
impl TryFrom<ResponseRouterData<NuveiRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map transaction status to refund status
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Pending) => common_enums::RefundStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Success
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::RefundStatus::Success
                } else {
                    common_enums::RefundStatus::Failure
                }
            }
        };

        // Get connector refund ID
        let connector_refund_id = response
            .transaction_id
            .clone()
            .ok_or(errors::ConnectorError::MissingConnectorTransactionID)?;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// Refund Sync Response Transformation
impl TryFrom<ResponseRouterData<NuveiRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map transaction status to refund status
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Pending) => common_enums::RefundStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Success
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::RefundStatus::Success
                } else {
                    common_enums::RefundStatus::Failure
                }
            }
        };

        // Get connector refund ID
        let connector_refund_id = response
            .transaction_id
            .clone()
            .ok_or(errors::ConnectorError::MissingConnectorTransactionID)?;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// Void Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NuveiVoidRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        // Extract amount and currency from the request
        // For void, we need to send the original transaction amount and currency
        let minor_amount =
            router_data
                .request
                .amount
                .ok_or(errors::ConnectorError::MissingRequiredField {
                    field_name: "amount",
                })?;

        let currency =
            router_data
                .request
                .currency
                .ok_or(errors::ConnectorError::MissingRequiredField {
                    field_name: "currency",
                })?;

        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(errors::ConnectorError::RequestEncodingFailed)?;

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId + amount + currency + relatedTransactionId + "" + "" + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &client_unique_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &related_transaction_id,
            "", // authCode (empty)
            "", // comment (empty)
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Void Response Transformation
impl TryFrom<ResponseRouterData<NuveiVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::VoidFailed),
                    connector_transaction_id: response.transaction_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Map transaction status to attempt status
        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::AttemptStatus::Voided,
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::VoidFailed,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::VoidFailed,
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Voided
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Voided
                } else {
                    common_enums::AttemptStatus::VoidFailed
                }
            }
        };

        // Get connector transaction ID
        let connector_transaction_id = response
            .transaction_id
            .clone()
            .ok_or(errors::ConnectorError::MissingConnectorTransactionID)?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
