use crate::connectors::{
    axisbank::AxisbankRouterData,
    juspay_upi_stack::{
        constants::*,
        crypto::sign_jws,
        transformers::{
            construct_upi_deeplink, map_refund_status, map_transaction_status,
            sanitize_merchant_request_id, sanitize_remarks,
        },
        types::{
            OuterResponseCode, Refund360Request, Refund360ResponsePayload, RegisterIntentRequest,
            Status360Request,
        },
    },
};
use crate::types::ResponseRouterData;
use common_enums as enums;
use domain_types::{
    connector_flow::{Authorize, PSync, Refund, RSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData, RefundSyncData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// Re-export for use in the connector (axisbank.rs calls axisbank::get_current_timestamp_ms via this module alias)
// Also makes get_current_timestamp_ms available locally in this module.
pub use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;

/// Auth configuration for Axis Bank
#[derive(Debug, Clone)]
pub struct AxisbankAuthConfig {
    pub merchant_id: String,
    pub merchant_channel_id: String,
    pub merchant_kid: String,
    pub juspay_kid: String,
    pub merchant_private_key: Secret<String>,
    pub juspay_public_key: Secret<String>,
    pub base_url: String,
}

impl TryFrom<&ConnectorSpecificConfig> for AxisbankAuthConfig {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Axisbank {
                merchant_id,
                merchant_channel_id,
                merchant_kid,
                juspay_kid,
                merchant_private_key,
                juspay_public_key,
                base_url,
            } => Ok(Self {
                merchant_id: merchant_id.peek().clone(),
                merchant_channel_id: merchant_channel_id.peek().clone(),
                merchant_kid: merchant_kid.peek().clone(),
                juspay_kid: juspay_kid.peek().clone(),
                merchant_private_key: merchant_private_key.clone(),
                juspay_public_key: juspay_public_key.clone(),
                base_url: base_url.clone().unwrap_or_default(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some("Check connector_specific_config in merchant connector account configuration".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/register-intent".to_string()),
                    additional_context: Some("Expected Axisbank variant with fields: merchant_id, merchant_channel_id, merchant_kid, juspay_kid, merchant_private_key, juspay_public_key".to_string()),
                },
            }
            .into()),
        }
    }
}

use crate::connectors::juspay_upi_stack::types::JuspayUpiAuthConfig as SharedAuthConfig;

impl From<AxisbankAuthConfig> for SharedAuthConfig {
    fn from(config: AxisbankAuthConfig) -> Self {
        let jwe_kid = config.merchant_kid.clone();
        let merchant_private_key = config.merchant_private_key.clone();
        Self {
            merchant_id: config.merchant_id,
            merchant_channel_id: config.merchant_channel_id,
            merchant_kid: config.merchant_kid,
            juspay_kid: config.juspay_kid,
            merchant_private_key: config.merchant_private_key,
            juspay_public_key: config.juspay_public_key,
            use_jwe: true,  // Axis Bank UAT uses JWE encryption for responses
            jwe_kid: Some(jwe_kid),
            juspay_jwe_public_key: None,
            merchant_jwe_private_key: Some(merchant_private_key),
        }
    }
}

// ============================================
// AUTHORIZE FLOW - Register Intent
// ============================================

pub type AxisbankPaymentsRequest = crate::connectors::juspay_upi_stack::types::JwsObject;
pub type AxisbankPaymentsResponse = crate::connectors::juspay_upi_stack::types::RegisterIntentResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for AxisbankPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;

        // Convert amount from minor to major units
        let amount = wrapper
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify amount and currency values are valid".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/register-intent".to_string()),
                    additional_context: Some("Amount must be a positive integer in minor units (paise). Currency should be INR for UPI transactions.".to_string()),
                },
            })?;

        // Get intent expiry from metadata or use default
        let intent_expiry = router_data
            .request
            .metadata
            .as_ref()
            .and_then(|m| m.peek().get("intent_expiry_minutes").cloned())
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| DEFAULT_INTENT_EXPIRY_MINUTES.to_string());

        // Sanitize merchant request ID
        let merchant_request_id = sanitize_merchant_request_id(
            &router_data.resource_common_data.connector_request_reference_id,
        );
        // UPI Request ID - must be strictly alphanumeric (no hyphens, dots, or underscores)
        // Per Juspay docs: "Constraint: 35 character alphanumeric"
        // Strip all non-alphanumeric characters from merchant_request_id
        let upi_request_id: String = merchant_request_id
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();

        // Build remarks from description
        let remarks = router_data
            .resource_common_data
            .description
            .as_ref()
            .map(|d| sanitize_remarks(d))
            .or_else(|| Some("Payment".to_string()));

        // Get reference URL
        let ref_url = router_data.request.router_return_url.clone();

        // Build the raw payload
        let register_intent = RegisterIntentRequest {
            merchant_request_id,
            upi_request_id,
            amount: amount.get_amount_as_string(),
            flow: FLOW_TRANSACTION.to_string(),
            intent_request_expiry_minutes: Some(intent_expiry),
            remarks,
            ref_url,
            iat: get_current_timestamp_ms(),
        };

        // Serialize payload to JSON
        let payload_json = serde_json::to_string(&register_intent).change_context(
            IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify all request fields have valid formats and lengths".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/register-intent".to_string()),
                    additional_context: Some("upi_request_id must be max 35 alphanumeric characters (hyphens/dots stripped). merchant_request_id should be unique per transaction. Amount must be string formatted.".to_string()),
                },
            },
        )?;

        // Sign with JWS
        let jws_object = sign_jws(&payload_json, &auth.merchant_private_key, &auth.merchant_kid)?;

        Ok(Self {
            protected: jws_object.protected,
            payload: jws_object.payload,
            signature: jws_object.signature,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static>
    TryFrom<
        ResponseRouterData<
            AxisbankPaymentsResponse,
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        >,
    >
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<
            AxisbankPaymentsResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let response = resp.response;
        let http_code = resp.http_code;
        let router_data = resp.router_data;

        if response.response_code.is_failure() {
            // Failed outer response
            let status = map_outer_response_code(response.response_code.clone());
            Ok(RouterDataV2 {
                response: Err(ErrorResponse {
                    code: format!("{:?}", response.response_code),
                    message: response.status.clone(),
                    reason: Some(response.status.clone()),
                    status_code: http_code,
                    attempt_status: Some(status),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                ..router_data
            })
        } else if let Some(payload) = response.payload {
            // Construct UPI deeplink
            let deeplink = construct_upi_deeplink(&payload);

            let redirect_form = RedirectForm::Uri { uri: deeplink };

            let response_data = PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    payload.gateway_transaction_id.clone()
                ),
                redirection_data: Some(Box::new(redirect_form)),
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                connector_response_reference_id: Some(payload.merchant_request_id.clone()),
                incremental_authorization_allowed: None,
                status_code: http_code,
            };

            Ok(RouterDataV2 {
                response: Ok(response_data),
                resource_common_data: PaymentFlowData {
                    status: enums::AttemptStatus::AuthenticationPending,
                    ..router_data.resource_common_data
                },
                ..router_data
            })
        } else {
            // Success outer code but no payload - treat as failure
            let status = map_outer_response_code(response.response_code.clone());
            let response_data = PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
            };
            Ok(RouterDataV2 {
                response: Ok(response_data),
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                ..router_data
            })
        }
    }
}

// ============================================
// PSYNC FLOW - Status 360
// ============================================

pub type AxisbankSyncRequest = crate::connectors::juspay_upi_stack::types::JwsObject;
pub type AxisbankSyncResponse = crate::connectors::juspay_upi_stack::types::Status360Response;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for AxisbankSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;

        // Get the connector transaction ID
        let merchant_request_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingRequiredField {
                field_name: "connector_transaction_id",
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify connector_transaction_id is provided in the PSync request".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/transaction-status-360".to_string()),
                    additional_context: Some("connector_transaction_id must be the original merchant_request_id from the Register Intent response. Used to query transaction status.".to_string()),
                },
            })?;

        // Build status request
        let status_request = Status360Request {
            merchant_request_id,
            transaction_type: TRANSACTION_TYPE_PAY.to_string(),
            iat: get_current_timestamp_ms(),
        };

        // Serialize and sign
        let payload_json = serde_json::to_string(&status_request)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify merchant_request_id is valid and transaction_type is correctly set".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/transaction-status-360".to_string()),
                    additional_context: Some("transaction_type should be 'PAY' for payment status queries. merchant_request_id must match the original transaction request ID. iat is auto-generated timestamp.".to_string()),
                },
            })?;

        let jws_object = sign_jws(&payload_json, &auth.merchant_private_key, &auth.merchant_kid)?;

        Ok(Self {
            protected: jws_object.protected,
            payload: jws_object.payload,
            signature: jws_object.signature,
        })
    }
}

impl TryFrom<ResponseRouterData<AxisbankSyncResponse, RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<AxisbankSyncResponse, RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = resp.response;
        let http_code = resp.http_code;
        let router_data = resp.router_data;

        // Check for failure response code
        if response.response_code.is_failure() {
            let status = map_outer_response_code(response.response_code.clone());
            return Ok(RouterDataV2 {
                response: Err(ErrorResponse {
                    code: format!("{:?}", response.response_code),
                    message: response.status.clone(),
                    reason: Some(response.status.clone()),
                    status_code: http_code,
                    attempt_status: Some(status),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                ..router_data
            });
        }

        // Map status
        let status = if let Some(ref payload) = response.payload {
            map_transaction_status(response.response_code.clone(), Some(&payload.gateway_response_code))
        } else {
            map_transaction_status(response.response_code.clone(), None)
        };

        let response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::NoResponseId,
            redirection_data: None,
            connector_metadata: None,
            mandate_reference: None,
            network_txn_id: None,
            connector_response_reference_id: response.payload.as_ref()
                .map(|p| p.gateway_transaction_id.clone()),
            incremental_authorization_allowed: None,
            status_code: http_code,
        };

        Ok(RouterDataV2 {
            response: Ok(response_data),
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// ============================================
// REFUND/RSYNC FLOW - Refund 360
// ============================================

/// Request body for Refund 360 API
#[derive(Debug, Serialize)]
pub struct AxisbankRefundRequest {
    pub protected: String,
    pub payload: String,
    pub signature: String,
}

/// Response from Refund 360 API
#[derive(Debug, Deserialize, Serialize)]
pub struct AxisbankRefundResponse {
    pub status: String,
    #[serde(rename = "responseCode")]
    pub response_code: String,
    #[serde(rename = "responseMessage")]
    pub response_message: String,
    pub payload: Option<Refund360ResponsePayload>,
}

/// Response from Refund 360 API (used for RSync flow to avoid Templating name collision)
#[derive(Debug, Deserialize, Serialize)]
pub struct AxisbankRefundSyncResponse {
    pub status: String,
    #[serde(rename = "responseCode")]
    pub response_code: String,
    #[serde(rename = "responseMessage")]
    pub response_message: String,
    pub payload: Option<Refund360ResponsePayload>,
}

/// Request body for Refund 360 API (used for RSync flow to avoid Templating name collision)
#[derive(Debug, Serialize)]
pub struct AxisbankRefundSyncRequest {
    /// JWS signed payload (protected.payload.signature)
    pub protected: String,
    pub payload: String,
    pub signature: String,
}

fn build_refund_request(
    refunds_data: &RefundsData,
    _auth: &AxisbankAuthConfig,
) -> Result<Refund360Request, error_stack::Report<IntegrationError>> {
    // Determine refund type (default to OFFLINE for safety)
    let refund_type = refunds_data
        .refund_connector_metadata
        .as_ref()
        .and_then(|m| m.peek().get("refund_type").cloned())
        .and_then(|v| v.as_str().map(|s| s.to_uppercase()))
        .unwrap_or_else(|| REFUND_TYPE_OFFLINE.to_string());

    // Get adjustment code and flag for UDIR refunds
    let (adj_code, adj_flag) = if refund_type == REFUND_TYPE_UDIR {
        (
            Some(ADJ_CODE_GOODS_NOT_PROVIDED.to_string()),
            Some(ADJ_FLAG_REF.to_string()),
        )
    } else {
        (None, None)
    };

    // Convert amount from minor to major units
    let amount_in_rupees = (refunds_data.minor_refund_amount.get_amount_as_i64() as f64) / 100.0;
    let refund_amount = format!("{:.2}", amount_in_rupees);

    Ok(Refund360Request {
        original_merchant_request_id: refunds_data.connector_transaction_id.clone(),
        refund_request_id: refunds_data.refund_id.clone(),
        refund_type,
        refund_amount,
        remarks: refunds_data
            .reason
            .clone()
            .unwrap_or_else(|| "Refund".to_string()),
        adj_code,
        adj_flag,
        merchant_refund_vpa: None,
        original_transaction_timestamp: None,
        iat: get_current_timestamp_ms(),
    })
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AxisbankRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;

        let refund_request = build_refund_request(&router_data.request, &auth)?;

        let payload_json = serde_json::to_string(&refund_request).change_context(
            IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify refund fields are valid: original_merchant_request_id, refund_amount, and refund_type".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/refund-360".to_string()),
                    additional_context: Some("refund_type can be 'ONLINE', 'OFFLINE', or 'UDIR'. For UDIR, adj_code and adj_flag are required. refund_amount must be in rupees with 2 decimal places (e.g., '100.00').".to_string()),
                },
            },
        )?;

        let jws_object = sign_jws(&payload_json, &auth.merchant_private_key, &auth.merchant_kid)?;

        Ok(Self {
            protected: jws_object.protected,
            payload: jws_object.payload,
            signature: jws_object.signature,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        AxisbankRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for AxisbankRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: AxisbankRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &wrapper.router_data;
        let auth = AxisbankAuthConfig::try_from(&router_data.connector_config)?;

        // For RSync, use connector_refund_id (not refund_id — RefundSyncData has no refund_id)
        let refund_sync = Refund360Request {
            original_merchant_request_id: router_data
                .request
                .connector_transaction_id
                .clone(),
            refund_request_id: router_data.request.connector_refund_id.clone(),
            refund_type: REFUND_TYPE_OFFLINE.to_string(), // Default for sync
            refund_amount: "0.00".to_string(),            // Not needed for status check
            remarks: "Status check".to_string(),
            adj_code: None,
            adj_flag: None,
            merchant_refund_vpa: None,
            original_transaction_timestamp: None,
            iat: get_current_timestamp_ms(),
        };

        let payload_json = serde_json::to_string(&refund_sync).change_context(
            IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some("Verify connector_refund_id is provided for refund status query".to_string()),
                    doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/refund-360".to_string()),
                    additional_context: Some("For RSync, connector_refund_id is the refund_request_id from the original Refund request. original_merchant_request_id is the parent transaction ID.".to_string()),
                },
            },
        )?;

        let jws_object = sign_jws(&payload_json, &auth.merchant_private_key, &auth.merchant_kid)?;

        Ok(Self {
            protected: jws_object.protected,
            payload: jws_object.payload,
            signature: jws_object.signature,
        })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            AxisbankRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    > for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<
            AxisbankRefundResponse,
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = resp.response;
        let http_code = resp.http_code;
        let router_data = resp.router_data;

        // Map refund status
        let status = if let Some(ref payload) = response.payload {
            map_refund_status(
                &payload.refund_type,
                &payload.gateway_response_code,
                &payload.gateway_response_status,
            )
        } else {
            enums::RefundStatus::Failure
        };

        let response_data = RefundsResponseData {
            connector_refund_id: response
                .payload
                .as_ref()
                .map(|p| p.refund_request_id.clone())
                .unwrap_or_default(),
            refund_status: status,
            status_code: http_code,
        };

        Ok(RouterDataV2 {
            response: Ok(response_data),
            ..router_data
        })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            AxisbankRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    > for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        resp: ResponseRouterData<
            AxisbankRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = resp.response;
        let http_code = resp.http_code;
        let router_data = resp.router_data;

        let status = if let Some(ref payload) = response.payload {
            map_refund_status(
                &payload.refund_type,
                &payload.gateway_response_code,
                &payload.gateway_response_status,
            )
        } else {
            enums::RefundStatus::Failure
        };

        let response_data = RefundsResponseData {
            connector_refund_id: response
                .payload
                .as_ref()
                .map(|p| p.refund_request_id.clone())
                .unwrap_or_default(),
            refund_status: status,
            status_code: http_code,
        };

        Ok(RouterDataV2 {
            response: Ok(response_data),
            ..router_data
        })
    }
}

// ============================================
// ERROR HANDLING
// ============================================

/// Error response structure from Axis Bank API
#[derive(Debug, Deserialize)]
pub struct AxisbankErrorResponse {
    pub status: String,
    #[serde(rename = "responseCode")]
    pub response_code: String,
    #[serde(rename = "responseMessage")]
    pub response_message: String,
}

/// Map outer response code (when no payload or outer failure)
pub fn map_outer_response_code(response_code: OuterResponseCode) -> enums::AttemptStatus {
    match response_code {
        OuterResponseCode::RequestNotFound => enums::AttemptStatus::Pending,
        OuterResponseCode::RequestExpired
        | OuterResponseCode::Dropout
        | OuterResponseCode::Failure
        | OuterResponseCode::BadRequest
        | OuterResponseCode::InvalidData
        | OuterResponseCode::Unauthorized
        | OuterResponseCode::InvalidMerchant
        | OuterResponseCode::DeviceFingerprintMismatch
        | OuterResponseCode::InternalServerError
        | OuterResponseCode::InvalidTransactionId
        | OuterResponseCode::UninitiatedRequest
        | OuterResponseCode::InvalidRefundAmount
        | OuterResponseCode::Success
        | OuterResponseCode::RequestPending
        | OuterResponseCode::ServiceUnavailable
        | OuterResponseCode::GatewayTimeout
        | OuterResponseCode::DuplicateRequest => enums::AttemptStatus::Failure,
    }
}

/// Build error response from API error
pub fn build_error_response(
    status_code: u16,
    response_code: &str,
    response_message: &str,
) -> ErrorResponse {
    let attempt_status = match response_code {
        RESPONSE_CODE_UNAUTHORIZED => Some(enums::AttemptStatus::Failure),
        RESPONSE_CODE_REQUEST_EXPIRED => Some(enums::AttemptStatus::Failure),
        RESPONSE_CODE_DROPOUT => Some(enums::AttemptStatus::Failure),
        _ => None,
    };

    ErrorResponse {
        status_code,
        code: response_code.to_string(),
        message: response_message.to_string(),
        reason: Some(response_message.to_string()),
        attempt_status,
        connector_transaction_id: None,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    }
}
