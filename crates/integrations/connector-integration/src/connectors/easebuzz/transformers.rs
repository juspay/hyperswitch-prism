use common_enums::{AttemptStatus, RefundStatus};
use common_utils::types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, PSync, RSync, Refund},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{self, IntegrationError},
    payment_method_data::{
        BankRedirectData, PaymentMethodData, PaymentMethodDataTypes, UpiData, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::easebuzz::EasebuzzRouterData, types::ResponseRouterData};

/// Extract the inner string from a `StringMajorUnit` for use in hash computation.
fn string_major_unit_to_string(amount: &StringMajorUnit) -> String {
    amount.get_amount_as_string()
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;
    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

// ============================================================================
// Authentication
// ============================================================================

#[derive(Debug, Clone)]
pub struct EasebuzzAuthType {
    pub api_key: Secret<String>,
    pub api_salt: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for EasebuzzAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Easebuzz {
                api_key, api_salt, ..
            } => Ok(Self {
                api_key: api_key.clone(),
                api_salt: api_salt.clone(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Expected ConnectorSpecificConfig::Easebuzz variant with api_key and api_salt fields".to_string()
                        ),
                        ..Default::default()
                    },
                }
            )),
        }
    }
}

// ============================================================================
// Error Response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EasebuzzErrorResponse {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
    pub status: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl Default for EasebuzzErrorResponse {
    fn default() -> Self {
        Self {
            code: "UNKNOWN_ERROR".to_string(),
            message: "Unknown error occurred".to_string(),
            status: None,
            error: None,
        }
    }
}

// ============================================================================
// InitiateLink Types (Step 1 — get access_key)
// ============================================================================

/// Compute SHA-512 hash for initiateLink request.
/// Formula: SHA512(key|txnid|amount|productinfo|firstname|email|udf1|udf2|...|udf10|salt)
pub fn compute_initiate_link_hash(
    key: &str,
    txnid: &str,
    amount: &str,
    productinfo: &str,
    firstname: &str,
    email: &str,
    salt: &Secret<String>,
) -> String {
    use sha2::{Digest, Sha512};
    // udf1..udf10 are empty strings
    let empty = "";
    let salt_str = salt.peek();
    let input = format!(
        "{key}|{txnid}|{amount}|{productinfo}|{firstname}|{email}|{e}|{e}|{e}|{e}|{e}|{e}|{e}|{e}|{e}|{e}|{salt_str}",
        e = empty
    );
    let mut hasher = Sha512::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Request to POST /payment/initiateLink
#[derive(Debug, Serialize)]
pub struct EasebuzzInitiateLinkRequest {
    pub key: Secret<String>,
    pub txnid: String,
    pub amount: StringMajorUnit,
    pub productinfo: String,
    pub firstname: String,
    pub phone: String,
    pub email: String,
    pub surl: String,
    pub furl: String,
    pub hash: String,
    pub request_flow: String,
}

/// Response from POST /payment/initiateLink
#[derive(Debug, Deserialize, Serialize)]
pub struct EasebuzzInitiateLinkResponse {
    pub status: i32,
    pub data: String,
    pub error_desc: Option<String>,
}

// -- CreateOrder request transformation (initiateLink) --

impl
    TryFrom<
        &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    > for EasebuzzInitiateLinkRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = EasebuzzAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Easebuzz requires api_key and api_salt in ConnectorSpecificConfig"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        let amount_str = StringMajorUnitForConnector
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert amount to major units for Easebuzz initiateLink"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let txnid = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Extract customer info from metadata or use defaults
        let metadata = router_data.request.metadata.as_ref().and_then(|m| {
            serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(m.peek().clone())
                .ok()
        });

        let get_field = |key: &str, default: &str| -> String {
            metadata
                .as_ref()
                .and_then(|m| m.get(key))
                .and_then(|v| v.as_str())
                .unwrap_or(default)
                .to_string()
        };

        let productinfo = get_field("productinfo", "Payment");
        let firstname = get_field("firstname", "Customer");
        let email = get_field("email", "customer@example.com");
        let phone = get_field("phone", "9999999999");

        let return_url = router_data
            .resource_common_data
            .return_url
            .clone()
            .unwrap_or_else(|| "https://example.com/return".to_string());

        let key_str = auth.api_key.peek().to_string();
        let amount_for_hash = string_major_unit_to_string(&amount_str);

        let hash = compute_initiate_link_hash(
            &key_str,
            &txnid,
            &amount_for_hash,
            &productinfo,
            &firstname,
            &email,
            &auth.api_salt,
        );

        Ok(Self {
            key: auth.api_key.clone(),
            txnid,
            amount: amount_str,
            productinfo,
            firstname,
            phone,
            email,
            surl: return_url.clone(),
            furl: return_url,
            hash,
            request_flow: "SEAMLESS".to_string(),
        })
    }
}

// -- CreateOrder response transformation (initiateLink → access_key) --

impl ForeignTryFrom<(EasebuzzInitiateLinkResponse, Self, u16, bool)>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<IntegrationError>;

    fn foreign_try_from(
        (response, data, status_code, _): (EasebuzzInitiateLinkResponse, Self, u16, bool),
    ) -> Result<Self, Self::Error> {
        if response.status != 1 {
            let err_msg = response
                .error_desc
                .unwrap_or_else(|| "InitiateLink failed".to_string());
            return Ok(Self {
                response: Err(ErrorResponse {
                    status_code,
                    code: response.status.to_string(),
                    message: err_msg.clone(),
                    reason: Some(err_msg),
                    attempt_status: Some(AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..data
            });
        }

        // response.data is the access_key — store it as connector_order_id
        let access_key = response.data.clone();
        let order_response = PaymentCreateOrderResponse {
            connector_order_id: access_key.clone(),
            session_data: None,
        };

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                connector_order_id: Some(access_key),
                ..data.resource_common_data
            },
            ..data
        })
    }
}

// ============================================================================
// Seamless Payment Request Types (Step 2)
// ============================================================================

/// Easebuzz Seamless Transaction Request
/// Sent to `POST /initiate_seamless_payment/`
#[derive(Debug, Serialize)]
pub struct EasebuzzPaymentsRequest {
    /// Access key from initiateLink step
    pub access_key: String,
    /// Payment mode: UPI, NB, MW (wallet), CARD, etc.
    pub payment_mode: String,
    /// UPI VPA for UPI Collect flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upi_va: Option<Secret<String>>,
    /// UPI QR flag (set to "1" for QR flow)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upi_qr: Option<String>,
    /// Bank code for NB and Wallet (MW) flows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<String>,
    /// Request mode for S2S response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_mode: Option<String>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Status from EaseBuzz seamless transaction response
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EasebuzzPaymentStatus {
    Success,
    Failure,
    Bounced,
    #[serde(other)]
    Unknown,
}

/// EaseBuzz Seamless Transaction Response
/// Uses serde_json::Value to handle both JSON and HTML (redirect) responses
#[derive(Debug, Deserialize, Serialize)]
pub struct EasebuzzPaymentsResponse(pub serde_json::Value);

// ============================================================================
// Request Transformation
// ============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EasebuzzRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for EasebuzzPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EasebuzzRouterData<
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

        // Determine payment mode and payment-method-specific fields
        let (payment_mode, upi_va, upi_qr, bank_code) =
            match &router_data.request.payment_method_data {
                PaymentMethodData::Upi(upi_data) => match upi_data {
                    UpiData::UpiCollect(collect_data) => {
                        let vpa = collect_data
                            .vpa_id
                            .as_ref()
                            .map(|v| Secret::new(v.peek().to_string()));
                        ("UPI".to_string(), vpa, None, None)
                    }
                    UpiData::UpiIntent(_) => ("UPI".to_string(), None, None, None),
                    UpiData::UpiQr(_) => ("UPI".to_string(), None, Some("1".to_string()), None),
                },
                PaymentMethodData::Wallet(WalletData::EaseBuzzRedirect(_)) => {
                    ("MW".to_string(), None, None, None)
                }
                PaymentMethodData::BankRedirect(BankRedirectData::Netbanking { issuer }) => {
                    ("NB".to_string(), None, None, Some(issuer.to_string()))
                }
                _ => {
                    return Err(error_stack::report!(IntegrationError::not_implemented(
                        "This payment method is not supported for Easebuzz"
                    )));
                }
            };

        // access_key comes from the CreateOrder (initiateLink) step
        let access_key = router_data
            .resource_common_data
            .connector_order_id
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Call PaymentService.CreateOrder first to obtain an access_key, then pass it as connector_order_id in the Authorize request".to_string()
                    ),
                    additional_context: Some(
                        "Easebuzz seamless API requires a two-step flow: POST /payment/initiateLink returns an access_key which must be passed to POST /initiate_seamless_payment/".to_string()
                    ),
                    ..Default::default()
                },
            })?;

        Ok(Self {
            access_key,
            payment_mode,
            upi_va,
            upi_qr,
            bank_code,
            request_mode: Some("S2S".to_string()),
        })
    }
}

// ============================================================================
// Response Transformation
// ============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<EasebuzzPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EasebuzzPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let raw_value = &item.response.0;
        let router_data = item.router_data;

        // If the response is not a JSON object, treat as HTML redirect (UPI pending)
        let obj = match raw_value.as_object() {
            Some(obj) => obj,
            None => {
                let txn_id = router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone();
                return Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(txn_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: None,
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Pending,
                        ..router_data.resource_common_data
                    },
                    ..router_data
                });
            }
        };

        let get_str = |key: &str| -> Option<String> {
            obj.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
        };

        // Map status
        let status = match get_str("status").as_deref() {
            Some("success") | Some("SUCCESS") => AttemptStatus::Charged,
            Some("failure") | Some("FAILURE") | Some("failed") | Some("FAILED") => {
                AttemptStatus::Failure
            }
            _ => AttemptStatus::Pending,
        };

        // Check for error
        let error_field = get_str("error");
        if let Some(ref err) = error_field {
            if !err.is_empty() && err != "0" {
                let error_message = get_str("error_Message");
                return Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..router_data.resource_common_data
                    },
                    response: Err(ErrorResponse {
                        status_code: item.http_code,
                        code: err.clone(),
                        message: error_message
                            .clone()
                            .unwrap_or_else(|| "Payment failed".to_string()),
                        reason: error_message,
                        attempt_status: Some(AttemptStatus::Failure),
                        connector_transaction_id: get_str("easepayid"),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    ..router_data
                });
            }
        }

        let transaction_id = get_str("easepayid")
            .or_else(|| get_str("txnid"))
            .unwrap_or_else(|| {
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        // Build redirect data if redirect URL is available
        let redirect_url = get_str("return_url")
            .or_else(|| get_str("intent_link"))
            .or_else(|| get_str("qr_url"));

        let redirection_data = redirect_url.map(|url| {
            Box::new(RedirectForm::Form {
                endpoint: url,
                method: common_utils::request::Method::Get,
                form_fields: std::collections::HashMap::new(),
            })
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: get_str("txnid"),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================================================
// Capture Types
// ============================================================================

/// Easebuzz Direct Authorization Capture Request
/// POST to `/payment/v1/capture/direct`
#[derive(Debug, Serialize)]
pub struct EasebuzzCaptureRequest {
    /// Merchant API key
    pub key: Secret<String>,
    /// Easebuzz transaction ID (easepayid) from authorize step
    pub txnid: String,
    /// Transaction amount in major units (rupees), e.g. "100.0"
    pub amount: String,
    /// HMAC-SHA512 hash for authentication
    pub hash: String,
}

/// Compute SHA-512 hash for Easebuzz Capture
/// Formula: sha512(key|txnid|amount|salt)
fn compute_easebuzz_capture_hash(key: &str, txnid: &str, amount: &str, salt: &str) -> String {
    use sha2::{Digest, Sha512};
    let input = format!("{key}|{txnid}|{amount}|{salt}");
    let mut hasher = Sha512::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EasebuzzRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for EasebuzzCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EasebuzzRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = EasebuzzAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Easebuzz requires api_key and api_salt in ConnectorSpecificConfig"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        // Purpose: API requires original transaction reference for capture
        let txnid = router_data
            .request
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: errors::IntegrationErrorContext {
                    additional_context: Some("Easebuzz capture/refund requires the easepayid from the original authorize response".to_string()),
                    ..Default::default()
                },
            })?;

        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert capture amount for Easebuzz".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let amount_for_hash = string_major_unit_to_string(&amount);

        let key_str = auth.api_key.peek().to_string();
        let salt_str = auth.api_salt.peek().to_string();

        let hash = compute_easebuzz_capture_hash(&key_str, &txnid, &amount_for_hash, &salt_str);

        Ok(Self {
            key: auth.api_key.clone(),
            txnid,
            amount: amount_for_hash,
            hash,
        })
    }
}

/// Inner data payload within the successful AuthZ response
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EasebuzzAuthZData {
    pub status: String,
    pub txnid: Option<String>,
    pub easepayid: Option<String>,
    pub error: Option<String>,
    #[serde(rename = "error_Message")]
    pub error_message: Option<String>,
}

/// Successful AuthZ capture response: ValidAuthZResponse(EasebuzzOnlyAuthZResponse)
#[derive(Debug, Deserialize, Serialize)]
pub struct EasebuzzOnlyAuthZResponse {
    #[serde(rename = "data")]
    pub data: EasebuzzAuthZData,
}

/// Error AuthZ capture response: EasebuzzRedirectAuthzErrorResponse
#[derive(Debug, Deserialize, Serialize)]
pub struct EasebuzzAuthZErrorResponse {
    #[serde(rename = "error_Message")]
    pub error_message: Option<String>,
    pub error: Option<String>,
    pub status: Option<String>,
}

/// Top-level response from POST /payment/v1/capture/direct
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EasebuzzCaptureResponse {
    /// Successful: contains nested _data with status
    Success(EasebuzzOnlyAuthZResponse),
    /// Error: authorization failed
    Error(EasebuzzAuthZErrorResponse),
}

impl TryFrom<ResponseRouterData<EasebuzzCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EasebuzzCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        match &item.response {
            EasebuzzCaptureResponse::Success(success_resp) => {
                let txn_data = &success_resp.data;

                // Check for error field in _data
                if let Some(ref err) = txn_data.error {
                    if !err.is_empty() && err != "0" {
                        return Ok(Self {
                            resource_common_data: PaymentFlowData {
                                status: AttemptStatus::Failure,
                                ..router_data.resource_common_data
                            },
                            response: Err(ErrorResponse {
                                status_code: item.http_code,
                                code: err.clone(),
                                message: txn_data
                                    .error_message
                                    .clone()
                                    .unwrap_or_else(|| "Capture failed".to_string()),
                                reason: txn_data.error_message.clone(),
                                attempt_status: Some(AttemptStatus::Failure),
                                connector_transaction_id: txn_data.easepayid.clone(),
                                network_decline_code: None,
                                network_advice_code: None,
                                network_error_message: None,
                            }),
                            ..router_data
                        });
                    }
                }

                // Map status from _data.status using getTxnStatus logic
                let status = match txn_data.status.to_lowercase().as_str() {
                    "success" => AttemptStatus::Charged,
                    "initiated" | "pending" | "in_process" => AttemptStatus::Pending,
                    _ => AttemptStatus::Failure,
                };

                let transaction_id = txn_data
                    .easepayid
                    .clone()
                    .or_else(|| txn_data.txnid.clone())
                    .unwrap_or_else(|| {
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone()
                    });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: txn_data.txnid.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..router_data
                })
            }
            EasebuzzCaptureResponse::Error(err_resp) => {
                let err_msg = err_resp
                    .error_message
                    .clone()
                    .or_else(|| err_resp.error.clone())
                    .unwrap_or_else(|| "Authorization capture failed".to_string());

                let err_code = err_resp
                    .status
                    .clone()
                    .unwrap_or_else(|| "AUTHORIZATION_FAILED".to_string());

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..router_data.resource_common_data
                    },
                    response: Err(ErrorResponse {
                        status_code: item.http_code,
                        code: err_code,
                        message: err_msg.clone(),
                        reason: Some(err_msg),
                        attempt_status: Some(AttemptStatus::Failure),
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    ..router_data
                })
            }
        }
    }
}

// ============================================================================
// Refund Types
// ============================================================================

/// Compute SHA-512 hash for Easebuzz Refund
/// Formula: sha512(key|merchantRefundId|easebuzzId|refundAmount|salt)
fn compute_easebuzz_refund_hash(
    key: &str,
    merchant_refund_id: &str,
    easebuzz_id: &str,
    refund_amount: &str,
    salt: &str,
) -> String {
    use sha2::{Digest, Sha512};
    let input = format!("{key}|{merchant_refund_id}|{easebuzz_id}|{refund_amount}|{salt}");
    let mut hasher = Sha512::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Easebuzz Refund Request
/// POST to `https://dashboard.easebuzz.in/transaction/v2/refund`
#[derive(Debug, Serialize)]
pub struct EasebuzzRefundRequest {
    /// Merchant API key
    pub key: Secret<String>,
    /// Merchant's refund reference ID
    pub merchant_refund_id: String,
    /// Easebuzz transaction ID (easepayid from authorize/capture)
    pub easebuzz_id: String,
    /// Amount to refund in major units (rupees), e.g. "50.0"
    pub refund_amount: String,
    /// HMAC-SHA512 hash: sha512(key|merchantRefundId|easebuzzId|refundAmount|salt)
    pub hash: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EasebuzzRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for EasebuzzRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EasebuzzRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = EasebuzzAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Easebuzz requires api_key and api_salt in ConnectorSpecificConfig"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        let easebuzz_id = router_data.request.connector_transaction_id.clone();
        let merchant_refund_id = router_data.request.refund_id.clone();

        let refund_amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert refund amount for Easebuzz".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let refund_amount_str = string_major_unit_to_string(&refund_amount);

        let key_str = auth.api_key.peek().to_string();
        let salt_str = auth.api_salt.peek().to_string();

        let hash = compute_easebuzz_refund_hash(
            &key_str,
            &merchant_refund_id,
            &easebuzz_id,
            &refund_amount_str,
            &salt_str,
        );

        Ok(Self {
            key: auth.api_key.clone(),
            merchant_refund_id,
            easebuzz_id,
            refund_amount: refund_amount_str,
            hash,
        })
    }
}

/// Easebuzz Refund Response
/// Response from POST /transaction/v2/refund
#[derive(Debug, Deserialize, Serialize)]
pub struct EasebuzzRefundResponse {
    /// Refund initiation success flag
    pub status: bool,
    /// Failure reason (if any)
    pub reason: Option<String>,
    /// Easebuzz transaction ID
    pub easebuzz_id: Option<String>,
    /// Easebuzz refund ID
    pub refund_id: Option<String>,
    /// Confirmed refund amount
    pub refund_amount: Option<serde_json::Value>,
}

impl TryFrom<ResponseRouterData<EasebuzzRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EasebuzzRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = item.router_data;

        if !response.status {
            let reason = response
                .reason
                .clone()
                .unwrap_or_else(|| "Refund initiation failed".to_string());

            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: RefundStatus::Failure,
                    ..router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: "REFUND_FAILED".to_string(),
                    message: reason.clone(),
                    reason: Some(reason),
                    attempt_status: None,
                    connector_transaction_id: response.easebuzz_id.clone(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data
            });
        }

        // status = true means refund initiated successfully (pending)
        let connector_refund_id = response
            .refund_id
            .clone()
            .or_else(|| response.easebuzz_id.clone())
            .unwrap_or_else(|| {
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone()
            });

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: RefundStatus::Pending,
                ..router_data.resource_common_data
            },
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status: RefundStatus::Pending,
                status_code: item.http_code,
            }),
            ..router_data
        })
    }
}

// ============================================================================
// PSync Types
// ============================================================================

/// Easebuzz Transaction Sync Request
/// POST to `https://dashboard.easebuzz.in/transaction/v1/retrieve`
#[derive(Debug, Serialize)]
pub struct EasebuzzSyncRequest {
    /// Transaction ID (merchant's txnid, NOT easepayid)
    pub txnid: String,
    /// Transaction amount in major units (rupees as float string, e.g. "100.00")
    pub amount: String,
    /// Customer email (defaults to "mail@gmail.com" if empty)
    pub email: String,
    /// Customer phone (defaults to "9999999999" if empty)
    pub phone: String,
    /// Merchant API key
    pub key: Secret<String>,
    /// HMAC-SHA512 hash: sha512(key|txnid|amount|email|phone|salt)
    pub hash: String,
}

/// Easebuzz Seamless Txn Response (embedded in TxnSyncSuccessMessage)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EasebuzzSeamlessTxnResponse {
    pub status: String,
    pub txnid: Option<String>,
    pub easepayid: Option<String>,
    pub error: Option<String>,
    #[serde(rename = "error_Message")]
    pub error_message: Option<String>,
}

/// EaseBuzzTxnSyncResponse — top-level sync response
#[derive(Debug, Deserialize, Serialize)]
pub struct EasebuzzSyncResponse {
    /// API call success flag
    pub status: bool,
    /// Response payload: success message or error text
    pub msg: EasebuzzTxnSyncMsg,
}

/// TxnSyncMessageType — union of success/error variants
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EasebuzzTxnSyncMsg {
    /// Success variant: full transaction details
    Success(EasebuzzSeamlessTxnResponse),
    /// Error variant: plain text or structured error
    Error(serde_json::Value),
}

/// Compute SHA-512 hash for Easebuzz TxnSync
/// Formula: sha512(key|txnid|amount|email|phone|salt)
fn compute_easebuzz_sync_hash(
    key: &str,
    txnid: &str,
    amount: &str,
    email: &str,
    phone: &str,
    salt: &str,
) -> String {
    use sha2::{Digest, Sha512};
    let input = format!("{key}|{txnid}|{amount}|{email}|{phone}|{salt}");
    let mut hasher = Sha512::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// PSync Request Transformation
// ============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EasebuzzRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for EasebuzzSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EasebuzzRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = EasebuzzAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Easebuzz requires api_key and api_salt in ConnectorSpecificConfig"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        // Use merchant txnid (connector_request_reference_id), not easepayid
        let txnid = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert sync amount for Easebuzz".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let amount_str = string_major_unit_to_string(&amount);

        // Email/phone from billing address, with defaults matching initiateLink
        let email = router_data
            .resource_common_data
            .get_optional_billing_email()
            .map(|e| e.peek().to_string())
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "customer@example.com".to_string());

        let phone = router_data
            .resource_common_data
            .get_optional_billing_phone_number()
            .map(|p| p.peek().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "9999999999".to_string());

        let key_str = auth.api_key.peek().to_string();
        let salt_str = auth.api_salt.peek().to_string();

        let hash =
            compute_easebuzz_sync_hash(&key_str, &txnid, &amount_str, &email, &phone, &salt_str);

        Ok(Self {
            txnid,
            amount: amount_str,
            email,
            phone,
            key: auth.api_key,
            hash,
        })
    }
}

// ============================================================================
// PSync Response Transformation
// ============================================================================

impl TryFrom<ResponseRouterData<EasebuzzSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<EasebuzzSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = item.router_data;

        match &response.msg {
            EasebuzzTxnSyncMsg::Success(txn_resp) => {
                let attempt_status = match txn_resp.status.to_lowercase().as_str() {
                    "success" => AttemptStatus::Charged,
                    "initiated" | "pending" | "in_process" => AttemptStatus::Pending,
                    _ => AttemptStatus::Failure,
                };

                let transaction_id = txn_resp
                    .easepayid
                    .clone()
                    .or_else(|| txn_resp.txnid.clone())
                    .unwrap_or_else(|| {
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone()
                    });

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: txn_resp.txnid.clone(),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..router_data
                })
            }
            EasebuzzTxnSyncMsg::Error(err_val) => {
                let err_msg = err_val.to_string();
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::Failure,
                        ..router_data.resource_common_data
                    },
                    response: Err(ErrorResponse {
                        status_code: item.http_code,
                        code: "SYNC_ERROR".to_string(),
                        message: err_msg.clone(),
                        reason: Some(err_msg),
                        attempt_status: Some(AttemptStatus::Failure),
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    ..router_data
                })
            }
        }
    }
}

// ============================================================================
// RSync Types
// ============================================================================

/// Compute SHA-512 hash for Easebuzz Refund Sync
/// Formula: sha512(key|easebuzzId|salt)
fn compute_easebuzz_refund_sync_hash(key: &str, easebuzz_id: &str, salt: &str) -> String {
    use sha2::{Digest, Sha512};
    let input = format!("{key}|{easebuzz_id}|{salt}");
    let mut hasher = Sha512::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Easebuzz Refund Sync Request
/// POST to `https://dashboard.easebuzz.in/refund/v1/retrieve`
#[derive(Debug, Serialize)]
pub struct EasebuzzRefundSyncRequest {
    /// Merchant API key
    pub key: Secret<String>,
    /// Easebuzz transaction ID (easepayid from original payment)
    pub easebuzz_id: String,
    /// HMAC-SHA512 hash: sha512(key|easebuzzId|salt)
    pub hash: String,
    /// Merchant refund reference ID
    pub merchant_refund_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        EasebuzzRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for EasebuzzRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: EasebuzzRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = EasebuzzAuthType::try_from(&router_data.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Easebuzz requires api_key and api_salt in ConnectorSpecificConfig"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        // easebuzz_id is the connector transaction ID from the original payment
        let easebuzz_id = router_data.request.connector_transaction_id.clone();

        // merchant_refund_id is the connector refund ID returned during Refund flow
        let merchant_refund_id = router_data.request.connector_refund_id.clone();

        let key_str = auth.api_key.peek().to_string();
        let salt_str = auth.api_salt.peek().to_string();

        let hash = compute_easebuzz_refund_sync_hash(&key_str, &easebuzz_id, &salt_str);

        Ok(Self {
            key: auth.api_key,
            easebuzz_id,
            hash,
            merchant_refund_id,
        })
    }
}

/// Easebuzz Refund Sync Success Response inner data
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EasebuzzRefundSyncSuccessData {
    /// API call success flag
    pub status: bool,
    /// Transaction ID
    pub txnid: Option<String>,
    /// Easebuzz transaction ID
    pub easebuzz_id: Option<String>,
    /// Array of refund details
    pub refunds: Option<Vec<EasebuzzRefundSyncItem>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EasebuzzRefundSyncItem {
    pub refund_id: Option<String>,
    pub refund_status: Option<String>,
    pub merchant_refund_id: Option<String>,
    pub refund_amount: Option<String>,
}

/// Top-level Easebuzz Refund Sync Response
/// The response is a union of Success / Failure / ValidationError variants
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EasebuzzRefundSyncResponse {
    /// Success variant: contains refund status data
    Success(EasebuzzRefundSyncSuccessData),
    /// Failure / validation-error variant
    Error(serde_json::Value),
}

impl TryFrom<ResponseRouterData<EasebuzzRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<EasebuzzRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        match &item.response {
            EasebuzzRefundSyncResponse::Success(success_data) => {
                // Find the matching refund in the refunds array
                let target_refund_id = &router_data.request.connector_refund_id;
                let refund_item = success_data
                    .refunds
                    .as_ref()
                    .and_then(|refunds| {
                        refunds.iter().find(|r| {
                            r.refund_id.as_deref() == Some(target_refund_id.as_str())
                                || r.merchant_refund_id.as_deref()
                                    == Some(target_refund_id.as_str())
                        })
                    })
                    .or_else(|| success_data.refunds.as_ref().and_then(|r| r.first()));

                let refund_status = match refund_item
                    .and_then(|r| r.refund_status.as_deref())
                    .unwrap_or("")
                    .to_lowercase()
                    .as_str()
                {
                    "refunded" | "settled" => RefundStatus::Success,
                    "cancelled" | "reverse chargeback" | "failed" => RefundStatus::Failure,
                    _ => RefundStatus::Pending,
                };

                let connector_refund_id = refund_item
                    .and_then(|r| r.refund_id.clone())
                    .unwrap_or_else(|| router_data.request.connector_refund_id.clone());

                Ok(Self {
                    resource_common_data: RefundFlowData {
                        status: refund_status,
                        ..router_data.resource_common_data
                    },
                    response: Ok(RefundsResponseData {
                        connector_refund_id,
                        refund_status,
                        status_code: item.http_code,
                    }),
                    ..router_data
                })
            }
            EasebuzzRefundSyncResponse::Error(err_val) => {
                let err_msg = err_val.to_string();
                Ok(Self {
                    resource_common_data: RefundFlowData {
                        status: RefundStatus::Failure,
                        ..router_data.resource_common_data
                    },
                    response: Err(ErrorResponse {
                        status_code: item.http_code,
                        code: "REFUND_SYNC_ERROR".to_string(),
                        message: err_msg.clone(),
                        reason: Some(err_msg),
                        attempt_status: None,
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    }),
                    ..router_data
                })
            }
        }
    }
}
