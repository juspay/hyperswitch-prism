use std::collections::HashMap;

use common_enums::{AttemptStatus, CountryAlpha2, Currency, RefundStatus};
use common_utils::{pii::UpiVpaMaskingStrategy, types::FloatMajorUnit, Method};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, RealTimePaymentData, UpiData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::ResponseRouterData;

// ===== AUTHENTICATION =====
#[derive(Debug, Clone)]
pub struct IatapayAuthType {
    pub(super) client_id: Secret<String>,
    pub(super) merchant_id: Secret<String>,
    pub(super) client_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for IatapayAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Iatapay {
                client_id,
                merchant_id,
                client_secret,
                ..
            } => Ok(Self {
                client_id: client_id.to_owned(),
                merchant_id: merchant_id.to_owned(),
                client_secret: client_secret.to_owned(),
            }),
            _ => Err(Report::new(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IatapayErrorResponse {
    pub status: Option<u16>,
    pub error: String,
    pub message: Option<String>,
    pub reason: Option<String>,
}

// ===== OAUTH 2.0 ACCESS TOKEN STRUCTURES =====
#[derive(Debug, Clone, Serialize)]
pub struct IatapayAuthUpdateRequest {
    pub grant_type: String,
    pub scope: String,
}

impl IatapayAuthUpdateRequest {
    pub fn new(grant_type: String) -> Self {
        Self {
            grant_type,
            scope: "payment".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IatapayAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IatapayAccessTokenErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

// ===== STATUS ENUM =====
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IatapayPaymentStatus {
    Created,
    Initiated,
    Authorized,
    Settled,
    Cleared,
    Failed,
    #[serde(rename = "UNEXPECTED SETTLED")]
    UnexpectedSettled,
}

impl From<IatapayPaymentStatus> for AttemptStatus {
    fn from(status: IatapayPaymentStatus) -> Self {
        match status {
            IatapayPaymentStatus::Created => Self::AuthenticationPending,
            IatapayPaymentStatus::Initiated => Self::Pending,
            IatapayPaymentStatus::Authorized
            | IatapayPaymentStatus::Settled
            | IatapayPaymentStatus::Cleared => Self::Charged,
            IatapayPaymentStatus::Failed | IatapayPaymentStatus::UnexpectedSettled => Self::Failure,
        }
    }
}

// ===== REQUEST STRUCTURES =====
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IatapayPaymentsRequest {
    pub merchant_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_payment_id: Option<String>,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub country: CountryAlpha2,
    pub locale: String,
    pub redirect_urls: RedirectUrls,
    pub notification_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer_info: Option<PayerInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectUrls {
    pub success_url: String,
    pub failure_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayerInfo {
    pub token_id: Secret<String>,
}

// ===== RESPONSE STRUCTURES =====
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IatapayPaymentsResponse {
    pub status: IatapayPaymentStatus,
    #[serde(rename = "iataPaymentId")]
    pub iata_payment_id: Option<String>,
    pub merchant_payment_id: Option<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout_methods: Option<CheckoutMethods>,
    pub failure_code: Option<String>,
    pub failure_details: Option<String>,
}

// Type alias for PSync response (same structure as authorize response)
pub type IatapaySyncResponse = IatapayPaymentsResponse;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutMethods {
    pub redirect: RedirectMethod,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectMethod {
    pub redirect_url: String,
}

// ===== HELPER FUNCTIONS =====

/// Determine country code from payment method data
fn get_country_from_payment_method<T>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<CountryAlpha2, Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    match payment_method_data {
        // UPI methods → India
        PaymentMethodData::Upi(_) => Ok(CountryAlpha2::IN),

        // Bank Redirect methods
        PaymentMethodData::BankRedirect(redirect_data) => match redirect_data {
            // iDEAL → Netherlands
            BankRedirectData::Ideal { .. } => Ok(CountryAlpha2::NL),
            // LocalBankRedirect → Austria
            BankRedirectData::LocalBankRedirect { .. } => Ok(CountryAlpha2::AT),
            _ => Err(Report::new(IntegrationError::NotSupported {
                message: "Unsupported bank redirect type for Iatapay".to_string(),
                connector: "Iatapay",
                context: Default::default(),
            })),
        },

        // Real-time payment methods
        PaymentMethodData::RealTimePayment(rtp_data) => match &**rtp_data {
            // DuitNow → Malaysia
            RealTimePaymentData::DuitNow {} => Ok(CountryAlpha2::MY),
            // FPS → Hong Kong
            RealTimePaymentData::Fps {} => Ok(CountryAlpha2::HK),
            // PromptPay → Thailand
            RealTimePaymentData::PromptPay {} => Ok(CountryAlpha2::TH),
            // VietQR → Vietnam
            RealTimePaymentData::VietQr {} => Ok(CountryAlpha2::VN),
        },

        _ => Err(Report::new(IntegrationError::NotSupported {
            message: "Payment method not supported by Iatapay".to_string(),
            connector: "Iatapay",
            context: Default::default(),
        })),
    }
}

/// Extract VPA ID from UPI Collect payment method
fn get_vpa_id_from_upi(upi_data: &UpiData) -> Option<Secret<String, UpiVpaMaskingStrategy>> {
    match upi_data {
        UpiData::UpiCollect(collect_data) => collect_data.vpa_id.clone(),
        _ => None,
    }
}

// ===== REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::iatapay::IatapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for IatapayPaymentsRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: crate::connectors::iatapay::IatapayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method_data = &item.router_data.request.payment_method_data;

        // Determine country from payment method
        let country = get_country_from_payment_method(payment_method_data)?;

        // Format locale as "en-{country}"
        let locale = format!("en-{country}");

        // Extract merchant ID from connector auth
        let auth = IatapayAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id.clone();

        // Extract payer info (only for UPI Collect)
        let (country, payer_info) = match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Upi(upi_data) => (
                CountryAlpha2::IN,
                get_vpa_id_from_upi(&upi_data).map(|vpa_id| PayerInfo {
                    token_id: Secret::new(vpa_id.expose()),
                }),
            ),
            PaymentMethodData::BankRedirect(bank_redirect_data) => match bank_redirect_data {
                BankRedirectData::Ideal { .. } => (CountryAlpha2::NL, None),
                BankRedirectData::LocalBankRedirect {} => (CountryAlpha2::AT, None),
                BankRedirectData::BancontactCard { .. }
                | BankRedirectData::Bizum {}
                | BankRedirectData::Blik { .. }
                | BankRedirectData::Eft { .. }
                | BankRedirectData::Eps { .. }
                | BankRedirectData::Giropay { .. }
                | BankRedirectData::Interac { .. }
                | BankRedirectData::OnlineBankingCzechRepublic { .. }
                | BankRedirectData::OnlineBankingFinland { .. }
                | BankRedirectData::OnlineBankingPoland { .. }
                | BankRedirectData::OnlineBankingSlovakia { .. }
                | BankRedirectData::OpenBankingUk { .. }
                | BankRedirectData::Przelewy24 { .. }
                | BankRedirectData::Sofort { .. }
                | BankRedirectData::Trustly { .. }
                | BankRedirectData::OnlineBankingFpx { .. }
                | BankRedirectData::OnlineBankingThailand { .. }
                | BankRedirectData::Netbanking { .. }
                | BankRedirectData::OpenBanking { .. } => Err(IntegrationError::not_implemented(
                    domain_types::utils::get_unimplemented_payment_method_error_message("iatapay"),
                ))?,
            },
            PaymentMethodData::Card(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::CardToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    domain_types::utils::get_unimplemented_payment_method_error_message("iatapay"),
                ))?
            }
        };

        // Get return URL and webhook URL
        let return_url = item.router_data.request.router_return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "router_return_url",
                context: Default::default(),
            },
        )?;

        let webhook_url = item.router_data.request.webhook_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "webhook_url",
                context: Default::default(),
            },
        )?;

        // Convert amount from MinorUnit to FloatMajorUnit
        let amount = domain_types::utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.amount,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            merchant_id,
            merchant_payment_id: Some(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            amount,
            currency: item.router_data.request.currency,
            country,
            locale,
            redirect_urls: RedirectUrls {
                success_url: return_url.clone(),
                failure_url: return_url,
            },
            notification_url: webhook_url,
            payer_info,
        })
    }
}

// ===== RESPONSE TRANSFORMER =====
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<IatapayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<IatapayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to standard status
        let status = AttemptStatus::from(response.status.clone());

        // Handle failure cases
        if let Some(failure_code) = &response.failure_code {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: failure_code.clone(),
                    message: response.failure_details.clone().unwrap_or_default(),
                    reason: response.failure_details.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.iata_payment_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Build payment response data based on checkout methods
        let payments_response_data = match &response.checkout_methods {
            Some(checkout_methods) => {
                let form_fields = HashMap::new();
                let (connector_metadata, redirection_data) = match checkout_methods
                    .redirect
                    .redirect_url
                    .to_lowercase()
                    .ends_with("qr")
                {
                    true => {
                        // QR code flow - store in metadata
                        let mut metadata_map = HashMap::new();
                        metadata_map.insert(
                            "qr_code_url".to_string(),
                            Value::String(checkout_methods.redirect.redirect_url.clone()),
                        );
                        let metadata_value = serde_json::to_value(metadata_map).change_context(
                            crate::utils::response_handling_fail_for_connector(
                                item.http_code,
                                "iatapay",
                            ),
                        )?;
                        (Some(metadata_value), None)
                    }
                    false => {
                        // Standard redirect flow
                        (
                            None,
                            Some(Box::new(RedirectForm::Form {
                                endpoint: checkout_methods.redirect.redirect_url.clone(),
                                method: Method::Get,
                                form_fields,
                            })),
                        )
                    }
                };

                PaymentsResponseData::TransactionResponse {
                    resource_id: match response.iata_payment_id.clone() {
                        Some(id) => ResponseId::ConnectorTransactionId(id),
                        None => ResponseId::NoResponseId,
                    },
                    redirection_data,
                    mandate_reference: None,
                    connector_metadata,
                    network_txn_id: None,
                    connector_response_reference_id: response.merchant_payment_id.clone(),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }
            }
            None => PaymentsResponseData::TransactionResponse {
                resource_id: match response.iata_payment_id.clone() {
                    Some(id) => ResponseId::ConnectorTransactionId(id),
                    None => ResponseId::NoResponseId,
                },
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: response.merchant_payment_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            },
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

// ===== PSYNC RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<IatapaySyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<IatapaySyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to standard status
        let status = AttemptStatus::from(response.status.clone());

        // Handle failure cases
        if let Some(failure_code) = &response.failure_code {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: failure_code.clone(),
                    message: response.failure_details.clone().unwrap_or_default(),
                    reason: response.failure_details.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: response.iata_payment_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Determine redirection data or QR code metadata (for PSync, these should be None typically)
        let (redirection_data, connector_metadata) = (None, None);

        // Build success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: match response.iata_payment_id.clone() {
                Some(id) => ResponseId::ConnectorTransactionId(id),
                None => ResponseId::NoResponseId,
            },
            redirection_data,
            mandate_reference: None,
            connector_metadata,
            network_txn_id: None,
            connector_response_reference_id: response.merchant_payment_id.clone(),
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

// ===== REFUND STATUS ENUM =====
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IatapayRefundStatus {
    Created,
    Locked,
    Initiated,
    Authorized,
    Settled,
    Cleared,
    Failed,
}

impl From<IatapayRefundStatus> for RefundStatus {
    fn from(status: IatapayRefundStatus) -> Self {
        match status {
            IatapayRefundStatus::Created
            | IatapayRefundStatus::Locked
            | IatapayRefundStatus::Initiated
            | IatapayRefundStatus::Authorized => Self::Pending,
            IatapayRefundStatus::Settled | IatapayRefundStatus::Cleared => Self::Success,
            IatapayRefundStatus::Failed => Self::Failure,
        }
    }
}

// ===== REFUND REQUEST STRUCTURE =====
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IatapayRefundRequest {
    pub merchant_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_refund_id: Option<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_transfer_description: Option<String>,
    pub notification_url: String,
}

// ===== REFUND RESPONSE STRUCTURE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IatapayRefundResponse {
    pub iata_refund_id: String,
    pub status: IatapayRefundStatus,
    pub merchant_refund_id: Option<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
    pub bank_transfer_description: Option<String>,
    pub failure_code: Option<String>,
    pub failure_details: Option<String>,
    pub lock_reason: Option<String>,
    pub creation_date_time: Option<String>,
    pub finish_date_time: Option<String>,
    pub update_date_time: Option<String>,
    pub clearance_date_time: Option<String>,
    pub iata_payment_id: Option<String>,
    pub merchant_payment_id: Option<String>,
    pub payment_amount: Option<FloatMajorUnit>,
    pub merchant_id: Option<Secret<String>>,
    pub account_country: Option<String>,
}

// Type alias for RSync response (same structure as Refund response)
pub type IatapayRefundSyncResponse = IatapayRefundResponse;

// ===== REFUND REQUEST TRANSFORMER =====
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::iatapay::IatapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for IatapayRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: crate::connectors::iatapay::IatapayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        // Extract merchant_id from auth
        let auth = IatapayAuthType::try_from(&router_data.connector_config)?;
        let merchant_id = auth.merchant_id.clone();

        // Convert amount using FloatMajorUnit
        let amount = domain_types::utils::convert_amount(
            connector.amount_converter,
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            merchant_id,
            merchant_refund_id: Some(router_data.request.refund_id.clone()),
            amount,
            currency: router_data.request.currency.to_string(),
            bank_transfer_description: router_data.request.reason.clone(),
            notification_url: router_data.request.webhook_url.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "webhook_url",
                    context: Default::default(),
                },
            )?,
        })
    }
}

// ===== REFUND RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<IatapayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<IatapayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        let response = item.response;

        let refund_status = RefundStatus::from(response.status.clone());

        // Check if refund failed and return error response
        router_data.response = if refund_status == RefundStatus::Failure {
            Err(ErrorResponse {
                status_code: item.http_code,
                code: response
                    .failure_code
                    .unwrap_or_else(|| "REFUND_FAILED".to_string()),
                message: response
                    .failure_details
                    .clone()
                    .unwrap_or_else(|| "Refund failed".to_string()),
                reason: response.failure_details,
                attempt_status: None,
                connector_transaction_id: Some(response.iata_refund_id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: response.iata_refund_id.clone(),
                refund_status,
                status_code: item.http_code,
            })
        };

        Ok(router_data)
    }
}

// ===== REFUND SYNC RESPONSE TRANSFORMER =====
impl TryFrom<ResponseRouterData<IatapayRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<IatapayRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        let response = item.response;

        // Map status using the same logic as Refund
        let refund_status = RefundStatus::from(response.status.clone());

        // Check if refund failed and return error response
        router_data.response = if refund_status == RefundStatus::Failure {
            Err(ErrorResponse {
                status_code: item.http_code,
                code: response
                    .failure_code
                    .unwrap_or_else(|| "REFUND_FAILED".to_string()),
                message: response
                    .failure_details
                    .clone()
                    .unwrap_or_else(|| "Refund failed".to_string()),
                reason: response.failure_details,
                attempt_status: None,
                connector_transaction_id: Some(response.iata_refund_id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: response.iata_refund_id.clone(),
                refund_status,
                status_code: item.http_code,
            })
        };

        Ok(router_data)
    }
}

// ===== OAUTH 2.0 ACCESS TOKEN TRANSFORMERS =====
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        crate::connectors::iatapay::IatapayRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerAuthenticationToken,
                PaymentFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                domain_types::connector_types::ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for IatapayAuthUpdateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: crate::connectors::iatapay::IatapayRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerAuthenticationToken,
                PaymentFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                domain_types::connector_types::ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(item.router_data.request.grant_type.clone()))
    }
}

impl TryFrom<ResponseRouterData<IatapayAuthUpdateResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::ServerAuthenticationToken,
        PaymentFlowData,
        domain_types::connector_types::ServerAuthenticationTokenRequestData,
        domain_types::connector_types::ServerAuthenticationTokenResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<IatapayAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let mut router_data = item.router_data;

        router_data.response = Ok(
            domain_types::connector_types::ServerAuthenticationTokenResponseData {
                access_token: response.access_token,
                token_type: Some("Bearer".to_string()),
                expires_in: Some(response.expires_in),
            },
        );

        Ok(router_data)
    }
}
