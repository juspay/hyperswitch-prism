use common_enums::{AttemptStatus, RefundStatus};
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, RSync, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes, UpiData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::PinelabsOnlineRouterData;
use crate::types::ResponseRouterData;

// ========== Newtype Wrappers for Macro ==========
// The create_all_prerequisites! macro creates unique "Templating" structs for
// each response type name. Pinelabs uses the same underlying API response format
// across flows, so we use newtype wrappers (not type aliases) to ensure each is
// a distinct Rust type and avoid conflicting TryFrom impls.

/// Response type for Authorize flow
pub type PinelabsOnlineAuthorizeResponse = PinelabsOnlineResponse;
/// Response type for PSync flow
pub type PinelabsOnlinePSyncResponse = PinelabsOnlineResponse;

/// Newtype wrapper for CreateOrder response — distinct type for macro Templating
#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineCreateOrderResponse(pub PinelabsOnlineResponse);

/// Newtype wrapper for Capture response — distinct type for TryFrom impls
#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineCaptureResponse(pub PinelabsOnlineResponse);

/// Newtype wrapper for Void response — distinct type for TryFrom impls
#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineVoidResponse(pub PinelabsOnlineResponse);

/// Newtype wrapper for RSync response — reuses refund response format
/// PineLabs returns the same refund response structure for both Refund and RSync
#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineRSyncResponse(pub PinelabsOnlineRefundResponse);

// ========== Auth Type ==========

pub struct PinelabsOnlineAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for PinelabsOnlineAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::PinelabsOnline {
                client_id,
                client_secret,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// ========== Common Types ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAmount {
    pub value: i64,
    pub currency: String,
}

// ========== Authorize Request Types ==========

#[derive(Debug, Serialize)]
pub struct PinelabsOnlineOrderRequest {
    pub order_amount: PaymentAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_amount: Option<PaymentAmount>,
    pub merchant_order_reference: String,
    pub pre_auth: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_details: Option<PurchaseDetails>,
}

#[derive(Debug, Serialize)]
pub struct PurchaseDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<CustomerInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_metadata: Option<MerchantMetadata>,
}

#[derive(Debug, Serialize)]
pub struct CustomerInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobile_number: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MerchantMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key2: Option<String>,
}

// ========== Phase 2: Transaction Request Types (POST /orders/{order_id}/payments) ==========

#[derive(Debug, Serialize)]
pub struct PinelabsOnlineTransactionRequest {
    pub payments: Vec<PinelabsOnlinePayment>,
}

#[derive(Debug, Serialize)]
pub struct PinelabsOnlinePayment {
    pub merchant_payment_reference: String,
    pub payment_method: String,
    pub payment_amount: PaymentAmount,
    pub payment_option: PinelabsOnlinePaymentOption,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_info: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_validation_details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub convenience_fee_breakdown: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct PinelabsOnlinePaymentOption {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upi_details: Option<PinelabsOnlineUpiDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_details: Option<PinelabsOnlineCardDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netbanking_details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_token_details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upi_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardless_details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct PinelabsOnlineUpiDetails {
    pub txn_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<PinelabsOnlinePayer>,
}

#[derive(Debug, Serialize)]
pub struct PinelabsOnlinePayer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpa: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PinelabsOnlineCardDetails {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_4_digit: Option<String>,
    pub expiry_month: String,
    pub expiry_year: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_txn_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diners_token_reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diners_token_requester_merchant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_mobile_number: Option<String>,
}

// ========== Authorize Response Types ==========

/// PineLabs returns either a success or error — untagged deserialization
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PinelabsOnlineResponse {
    Success(PinelabsOnlineTransactionResponse),
    Error(PinelabsOnlineErrorResponse),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineTransactionResponse {
    #[serde(rename = "event_type")]
    pub event_type: Option<String>,
    #[serde(rename = "data")]
    pub data: TransactionData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionData {
    pub order_id: Option<String>,
    pub merchant_order_reference: Option<String>,
    pub status: Option<String>,
    pub challenge_url: Option<String>,
    pub merchant_id: Option<String>,
    pub order_amount: Option<PaymentAmount>,
    pub pre_auth: Option<bool>,
    pub payments: Option<Vec<ResponsePaymentInfo>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsePaymentInfo {
    pub id: Option<String>,
    pub status: Option<String>,
    pub merchant_payment_reference: Option<String>,
    pub payment_method: Option<String>,
    pub payment_amount: Option<PaymentAmount>,
    pub acquirer_data: Option<AcquirerData>,
    pub error_detail: Option<PinelabsOnlineErrorResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AcquirerData {
    pub approval_code: Option<String>,
    pub acquirer_reference: Option<String>,
    pub rrn: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinelabsOnlineErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
}

// ========== Capture Request ==========

#[derive(Debug, Serialize)]
pub struct PinelabsOnlineCaptureRequest {
    pub capture_amount: PaymentAmount,
    pub merchant_capture_reference: String,
}

// ========== Refund Request ==========

#[derive(Debug, Serialize)]
pub struct PinelabsOnlineRefundRequest {
    pub merchant_order_reference: String,
    pub order_amount: PaymentAmount,
}

// ========== Refund Response ==========

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PinelabsOnlineRefundResponse {
    Success(PinelabsOnlineRefundSuccessResponse),
    Error(PinelabsOnlineErrorResponse),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineRefundSuccessResponse {
    pub data: RefundResponseData,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RefundResponseData {
    pub order_id: Option<String>,
    pub parent_order_id: Option<String>,
    pub merchant_order_reference: Option<String>,
    #[serde(rename = "type")]
    pub refund_type: Option<String>,
    pub status: Option<String>,
    pub merchant_id: Option<String>,
    pub order_amount: Option<PaymentAmount>,
    pub payments: Option<Vec<RefundPaymentDetails>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RefundPaymentDetails {
    pub id: Option<String>,
    pub status: Option<String>,
    pub payment_amount: Option<PaymentAmount>,
}

// ========== Status Mapping ==========

fn get_payment_status(status: &str, pre_auth: Option<bool>) -> AttemptStatus {
    match (status, pre_auth) {
        ("AUTHORIZED", Some(true)) => AttemptStatus::Authorized,
        ("AUTHORIZED", Some(false)) => AttemptStatus::Pending,
        ("CANCELLED", Some(true)) => AttemptStatus::Voided,
        ("PROCESSED", _) => AttemptStatus::Charged,
        ("FAILED", _) => AttemptStatus::Failure,
        _ => AttemptStatus::AuthenticationPending,
    }
}

fn get_capture_status(status: &str) -> Option<AttemptStatus> {
    match status {
        "PROCESSED" => Some(AttemptStatus::Charged),
        "FAILED" => Some(AttemptStatus::CaptureFailed),
        "PENDING" => Some(AttemptStatus::CaptureInitiated),
        "PARTIALLY_CAPTURED" => Some(AttemptStatus::PartialCharged),
        _ => None,
    }
}

fn get_void_status(status: &str) -> Option<AttemptStatus> {
    match status {
        "FAILED" => Some(AttemptStatus::VoidFailed),
        "PENDING" => Some(AttemptStatus::VoidInitiated),
        "CANCELLED" => Some(AttemptStatus::Voided),
        _ => None,
    }
}

fn get_refund_status(status: &str) -> RefundStatus {
    match status {
        "PROCESSED" => RefundStatus::Success,
        "FAILED" => RefundStatus::Failure,
        _ => RefundStatus::Pending,
    }
}

// ========== AccessToken Request/Response Types ==========

#[derive(Debug, Serialize)]
pub struct PinelabsOnlineAccessTokenRequest {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub grant_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineAccessTokenResponse {
    pub access_token: Secret<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<String>,
}

/// Error response from the token endpoint
/// Pinelabs returns two different error formats:
/// - OAuth error: {"error": "...", "error_description": "..."}
/// - API error:   {"status": 401, "type": "", "message": "...", "traceId": "..."}
#[derive(Debug, Deserialize, Serialize)]
pub struct PinelabsOnlineAccessTokenErrorResponse {
    pub error: Option<String>,
    pub error_description: Option<String>,
    pub message: Option<String>,
    #[serde(alias = "traceId")]
    pub trace_id: Option<String>,
}

// ========== TryFrom: AccessToken Request ==========

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        PinelabsOnlineRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for PinelabsOnlineAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PinelabsOnlineRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PinelabsOnlineAuthType::try_from(&item.router_data.connector_config)?;
        Ok(Self {
            client_id: auth.client_id,
            client_secret: auth.client_secret,
            grant_type: item.router_data.request.grant_type,
        })
    }
}

// ========== TryFrom: AccessToken Response ==========

impl<F, T> TryFrom<ResponseRouterData<PinelabsOnlineAccessTokenResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PinelabsOnlineAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // PineLabs returns `expires_at` (ISO timestamp), need to convert to `expires_in` (seconds)
        let expires_in = item
            .response
            .expires_at
            .as_ref()
            .and_then(|expires_at_str| {
                time::OffsetDateTime::parse(
                    expires_at_str,
                    &time::format_description::well_known::Rfc3339,
                )
                .ok()
                .map(|expires_at| {
                    let now = time::OffsetDateTime::now_utc();
                    let duration = expires_at - now;
                    // Subtract a small buffer (60 seconds) to avoid using an expired token
                    duration.whole_seconds().max(0) - 60
                })
            });

        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in,
                token_type: None,
            }),
            ..item.router_data
        })
    }
}

// ========== TryFrom: CreateOrder Request ==========

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        PinelabsOnlineRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for PinelabsOnlineOrderRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PinelabsOnlineRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item.router_data.request.amount.get_amount_as_i64();
        let currency = item.router_data.request.currency.to_string();

        // CreateOrder doesn't have capture_method — default to pre_auth=false
        let is_pre_auth = false;

        let customer = CustomerInfo {
            email_id: None,
            first_name: None,
            last_name: None,
            mobile_number: None,
        };

        let purchase_details = PurchaseDetails {
            customer: Some(customer),
            merchant_metadata: None,
        };

        Ok(Self {
            order_amount: PaymentAmount {
                value: amount,
                currency,
            },
            base_amount: None,
            merchant_order_reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            pre_auth: is_pre_auth,
            callback_url: item.router_data.request.webhook_url.clone(),
            purchase_details: Some(purchase_details),
        })
    }
}

// ========== TryFrom: CreateOrder Response ==========

impl TryFrom<ResponseRouterData<PinelabsOnlineCreateOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PinelabsOnlineCreateOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.0 {
            PinelabsOnlineResponse::Success(response) => {
                let order_id =
                    response
                        .data
                        .order_id
                        .ok_or(ConnectorError::ResponseHandlingFailed {
                            context: Default::default(),
                        })?;

                Ok(Self {
                    response: Ok(PaymentCreateOrderResponse {
                        connector_order_id: order_id.clone(),
                        session_data: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Pending,
                        reference_id: Some(order_id),
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            PinelabsOnlineResponse::Error(error) => {
                let response = Err(ErrorResponse {
                    status_code: item.http_code,
                    code: error.code.unwrap_or_else(|| "UNKNOWN".to_string()),
                    message: error.message.unwrap_or_else(|| "Unknown error".to_string()),
                    reason: None,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

// ========== TryFrom: Authorize Request (Phase 2 — POST /orders/{order_id}/payments) ==========

fn get_pinelabs_payment_method_string<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<String, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Upi(_) => Ok("UPI".to_string()),
        PaymentMethodData::Card(_) => Ok("CARD".to_string()),
        PaymentMethodData::BankRedirect(_) => Ok("NETBANKING".to_string()),
        PaymentMethodData::Wallet(_) => Ok("WALLET".to_string()),
        _ => Err(IntegrationError::NotSupported {
            message: format!(
                "Payment method not supported for PineLabs Online: {:?}",
                std::mem::discriminant(payment_method_data)
            ),
            connector: "PinelabsOnline",
            context: Default::default(),
        }),
    }
}

fn build_payment_option<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<PinelabsOnlinePaymentOption, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Upi(upi_data) => {
            let (txn_mode, payer) = match upi_data {
                UpiData::UpiCollect(collect_data) => {
                    let vpa = collect_data.vpa_id.as_ref().map(|v| v.peek().to_string());
                    (
                        "COLLECT".to_string(),
                        Some(PinelabsOnlinePayer {
                            vpa,
                            account_type: None,
                        }),
                    )
                }
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => ("INTENT".to_string(), None),
            };
            Ok(PinelabsOnlinePaymentOption {
                upi_details: Some(PinelabsOnlineUpiDetails { txn_mode, payer }),
                card_details: None,
                netbanking_details: None,
                wallet_details: None,
                card_token_details: None,
                upi_data: None,
                cardless_details: None,
            })
        }
        PaymentMethodData::Card(card_data) => {
            let card_details = build_card_details(card_data)?;
            Ok(PinelabsOnlinePaymentOption {
                upi_details: None,
                card_details: Some(card_details),
                netbanking_details: None,
                wallet_details: None,
                card_token_details: None,
                upi_data: None,
                cardless_details: None,
            })
        }
        _ => Err(IntegrationError::NotSupported {
            message: format!(
                "Payment method not supported for PineLabs Online Phase 2: {:?}",
                std::mem::discriminant(payment_method_data)
            ),
            connector: "PinelabsOnline",
            context: Default::default(),
        }),
    }
}

/// Build PineLabs card_details from the connector-service Card type.
/// Mirrors Haskell `getCardDetails` for non-token-based card transactions.
fn build_card_details<T: PaymentMethodDataTypes>(
    card: &Card<T>,
) -> Result<PinelabsOnlineCardDetails, IntegrationError> {
    let name = card
        .card_holder_name
        .as_ref()
        .map(|n| n.peek().to_string())
        .unwrap_or_else(|| "Name".to_string());
    let card_number = Some(card.card_number.peek().to_string());
    let cvv = Some(card.card_cvc.peek().to_string());
    let expiry_month = card.card_exp_month.peek().to_string();
    let expiry_year = card.get_expiry_year_4_digit().peek().to_string();

    Ok(PinelabsOnlineCardDetails {
        name,
        card_number,
        cvv,
        last_4_digit: None,
        expiry_month,
        expiry_year,
        token: None,
        cryptogram: None,
        token_txn_type: None,
        diners_token_reference_id: None,
        diners_token_requester_merchant_id: None,
        registered_mobile_number: None,
    })
}

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        PinelabsOnlineRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PinelabsOnlineTransactionRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PinelabsOnlineRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item.router_data.request.minor_amount.0;
        let currency = item.router_data.request.currency.to_string();

        let payment_method_data = &item.router_data.request.payment_method_data;
        let payment_method_str = get_pinelabs_payment_method_string(payment_method_data)?;
        let payment_option = build_payment_option(payment_method_data)?;

        let payment = PinelabsOnlinePayment {
            merchant_payment_reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_method: payment_method_str,
            payment_amount: PaymentAmount {
                value: amount,
                currency,
            },
            payment_option,
            device_info: None,
            risk_validation_details: None,
            offer_data: None,
            convenience_fee_breakdown: None,
        };

        Ok(Self {
            payments: vec![payment],
        })
    }
}

// ========== TryFrom: Capture Request ==========

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        PinelabsOnlineRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PinelabsOnlineCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PinelabsOnlineRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item.router_data.request.minor_amount_to_capture.0;
        let currency = item.router_data.request.currency.to_string();

        Ok(Self {
            capture_amount: PaymentAmount {
                value: amount,
                currency,
            },
            merchant_capture_reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

// ========== TryFrom: Refund Request ==========

impl<T: PaymentMethodDataTypes + Debug + Send + Sync + 'static + Serialize>
    TryFrom<
        PinelabsOnlineRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PinelabsOnlineRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PinelabsOnlineRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item.router_data.request.minor_refund_amount.0;
        let currency = item.router_data.request.currency.to_string();

        Ok(Self {
            merchant_order_reference: item.router_data.request.refund_id.clone(),
            order_amount: PaymentAmount {
                value: amount,
                currency,
            },
        })
    }
}

// ========== TryFrom: Payment Response (Authorize + PSync) ==========

impl<F, Req> TryFrom<ResponseRouterData<PinelabsOnlineResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PinelabsOnlineResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            PinelabsOnlineResponse::Success(response) => {
                let status_str = response.data.status.as_deref().unwrap_or("PENDING");
                let pre_auth = response.data.pre_auth;
                let status = get_payment_status(status_str, pre_auth);

                let connector_transaction_id = response.data.order_id.clone();

                let redirection_data = response
                    .data
                    .challenge_url
                    .as_ref()
                    .and_then(|url_str| url::Url::parse(url_str).ok())
                    .map(|url| {
                        Box::new(domain_types::router_response_types::RedirectForm::from((
                            url,
                            common_utils::request::Method::Get,
                        )))
                    });

                let response = Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: connector_transaction_id
                        .map(ResponseId::ConnectorTransactionId)
                        .unwrap_or(ResponseId::NoResponseId),
                    redirection_data,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: response.data.merchant_order_reference,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
            PinelabsOnlineResponse::Error(error) => {
                let response = Err(ErrorResponse {
                    status_code: item.http_code,
                    code: error.code.unwrap_or_else(|| "UNKNOWN".to_string()),
                    message: error.message.unwrap_or_else(|| "Unknown error".to_string()),
                    reason: None,
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

// ========== TryFrom: Capture Response ==========

impl<F, T> TryFrom<ResponseRouterData<PinelabsOnlineCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PinelabsOnlineCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.0 {
            PinelabsOnlineResponse::Success(response) => {
                let status_str = response.data.status.as_deref().unwrap_or("PENDING");
                let status =
                    get_capture_status(status_str).unwrap_or(AttemptStatus::CaptureInitiated);

                let connector_transaction_id = response.data.order_id.clone();

                let response = Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: connector_transaction_id
                        .map(ResponseId::ConnectorTransactionId)
                        .unwrap_or(ResponseId::NoResponseId),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: response.data.merchant_order_reference,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
            PinelabsOnlineResponse::Error(error) => {
                let response = Err(ErrorResponse {
                    status_code: item.http_code,
                    code: error.code.unwrap_or_else(|| "UNKNOWN".to_string()),
                    message: error.message.unwrap_or_else(|| "Unknown error".to_string()),
                    reason: None,
                    attempt_status: Some(AttemptStatus::CaptureFailed),
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::CaptureFailed,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

// ========== TryFrom: Void Response ==========

impl<F, T> TryFrom<ResponseRouterData<PinelabsOnlineVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PinelabsOnlineVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.0 {
            PinelabsOnlineResponse::Success(response) => {
                let status_str = response.data.status.as_deref().unwrap_or("PENDING");
                let status = get_void_status(status_str).unwrap_or(AttemptStatus::VoidInitiated);

                let connector_transaction_id = response.data.order_id.clone();

                let response = Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: connector_transaction_id
                        .map(ResponseId::ConnectorTransactionId)
                        .unwrap_or(ResponseId::NoResponseId),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: response.data.merchant_order_reference,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
            PinelabsOnlineResponse::Error(error) => {
                let response = Err(ErrorResponse {
                    status_code: item.http_code,
                    code: error.code.unwrap_or_else(|| "UNKNOWN".to_string()),
                    message: error.message.unwrap_or_else(|| "Unknown error".to_string()),
                    reason: None,
                    attempt_status: Some(AttemptStatus::VoidFailed),
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::VoidFailed,
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

// ========== TryFrom: Refund Response ==========

impl<F> TryFrom<ResponseRouterData<PinelabsOnlineRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PinelabsOnlineRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            PinelabsOnlineRefundResponse::Success(response) => {
                let status_str = response.data.status.as_deref().unwrap_or("PENDING");
                let refund_status = get_refund_status(status_str);

                let connector_refund_id = response.data.order_id;

                let response = Ok(RefundsResponseData {
                    connector_refund_id: connector_refund_id.unwrap_or_default(),
                    refund_status,
                    status_code: item.http_code,
                });

                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
            PinelabsOnlineRefundResponse::Error(error) => {
                let response = Err(ErrorResponse {
                    status_code: item.http_code,
                    code: error.code.unwrap_or_else(|| "UNKNOWN".to_string()),
                    message: error.message.unwrap_or_else(|| "Unknown error".to_string()),
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });

                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
        }
    }
}

// ========== TryFrom: RSync Response ==========
// PineLabs uses the same response format for both Refund and RSync
// (GET /api/pay/v1/orders/reference/{merchant_order_reference})

impl TryFrom<ResponseRouterData<PinelabsOnlineRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PinelabsOnlineRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.0 {
            PinelabsOnlineRefundResponse::Success(response) => {
                let status_str = response.data.status.as_deref().unwrap_or("PENDING");
                let refund_status = get_refund_status(status_str);

                let connector_refund_id = response.data.order_id;

                let response = Ok(RefundsResponseData {
                    connector_refund_id: connector_refund_id.unwrap_or_default(),
                    refund_status,
                    status_code: item.http_code,
                });

                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
            PinelabsOnlineRefundResponse::Error(error) => {
                let response = Err(ErrorResponse {
                    status_code: item.http_code,
                    code: error.code.unwrap_or_else(|| "UNKNOWN".to_string()),
                    message: error.message.unwrap_or_else(|| "Unknown error".to_string()),
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                });

                Ok(Self {
                    response,
                    ..item.router_data
                })
            }
        }
    }
}
