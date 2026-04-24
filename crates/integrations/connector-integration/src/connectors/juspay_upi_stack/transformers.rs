//! Transformer utilities for Juspay UPI Merchant Stack
//!
//! This module provides helper functions for:
//! - Constructing UPI deeplinks
//! - Mapping response status codes
//! - Building request/response structures
//! - Generic request builders and response handlers shared across all UPI bank connectors supported by Juspay UPI Stack

use crate::connectors::juspay_upi_stack::{constants::*, crypto::sign_jws, types::*};
use common_enums as enums;
use common_utils::errors::CustomResult;
use common_utils::SecretSerdeValue;
use domain_types::{
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, PeekInterface};

/// Construct a UPI deeplink from register intent response parameters
///
/// Keys are sorted alphabetically as per UPI specification.
/// Template: upi://pay?am=<amount>&cu=<currency>&mc=<payeeMcc>&mode=<mode>&pa=<payeeVpa>&pn=<payeeName>&tid=<gatewayTransactionId>&tn=<remarks>&tr=<orderId>&url=<refUrl>
pub fn construct_upi_deeplink(params: &RegisterIntentResponsePayload) -> String {
    use std::collections::BTreeMap;
    use urlencoding::encode;

    let mut params_map: BTreeMap<&str, String> = BTreeMap::new();

    // Required parameters (sorted alphabetically by key)
    params_map.insert("am", params.amount.clone());
    params_map.insert("cu", params.currency.clone());
    params_map.insert("mc", params.payee_mcc.clone());
    params_map.insert("pa", params.payee_vpa.clone());
    params_map.insert("pn", params.payee_name.clone());
    params_map.insert("tid", params.gateway_transaction_id.clone());
    params_map.insert("tr", params.order_id.clone());

    // Mode (default to "00" if not present)
    params_map.insert(
        "mode",
        params
            .txn_initiation_mode
            .clone()
            .unwrap_or_else(|| DEFAULT_TXN_INITIATION_MODE.to_string()),
    );

    // Optional parameters - only add if present
    if let Some(ref remarks) = params.remarks {
        if !remarks.is_empty() {
            params_map.insert("tn", remarks.clone());
        }
    }

    if let Some(ref ref_url) = params.ref_url {
        if !ref_url.is_empty() {
            params_map.insert("url", ref_url.clone());
        }
    }

    // Build query string with URL-encoded values
    let query: Vec<String> = params_map
        .iter()
        .map(|(k, v)| format!("{}={}", k, encode(v)))
        .collect();

    format!("upi://pay?{}", query.join("&"))
}

/// Map transaction status from gateway response to internal AttemptStatus
/// Uses explicit exhaustive matching on OuterResponseCode and GatewayResponseCode enums
pub fn map_transaction_status(
    outer_response_code: OuterResponseCode,
    gateway_response_code: Option<&str>,
) -> enums::AttemptStatus {
    use crate::connectors::juspay_upi_stack::types::GatewayResponseCode;

    match outer_response_code {
        // Terminal failure states
        OuterResponseCode::Failure
        | OuterResponseCode::RequestExpired
        | OuterResponseCode::Dropout
        | OuterResponseCode::InvalidData
        | OuterResponseCode::Unauthorized
        | OuterResponseCode::InvalidMerchant
        | OuterResponseCode::DeviceFingerprintMismatch
        | OuterResponseCode::InternalServerError
        | OuterResponseCode::InvalidTransactionId
        | OuterResponseCode::UninitiatedRequest
        | OuterResponseCode::InvalidRefundAmount
        | OuterResponseCode::BadRequest => enums::AttemptStatus::Failure,

        OuterResponseCode::RequestNotFound
        | OuterResponseCode::RequestPending
        | OuterResponseCode::ServiceUnavailable
        | OuterResponseCode::GatewayTimeout
        | OuterResponseCode::DuplicateRequest => enums::AttemptStatus::Pending,

        OuterResponseCode::Success => {
            if let Some(code) = gateway_response_code {
                let gateway = GatewayResponseCode::parse(code);
                match gateway {
                    GatewayResponseCode::Success => enums::AttemptStatus::Charged,
                    GatewayResponseCode::Pending
                    | GatewayResponseCode::Deemed
                    | GatewayResponseCode::MandatePaused
                    | GatewayResponseCode::MandateCompleted => enums::AttemptStatus::Pending,
                    GatewayResponseCode::Declined
                    | GatewayResponseCode::Expired
                    | GatewayResponseCode::BeneAddrIncorrect
                    | GatewayResponseCode::IntentExpired
                    | GatewayResponseCode::ValidationError
                    | GatewayResponseCode::MandateRevoked
                    | GatewayResponseCode::MandateDeclined
                    | GatewayResponseCode::MandateExpired
                    | GatewayResponseCode::Unknown(_) => enums::AttemptStatus::Failure,
                }
            } else {
                // No gateway code yet - still pending
                enums::AttemptStatus::Pending
            }
        }
    }
}

/// Map refund status from gateway response
pub fn map_refund_status(
    refund_type: &str,
    gateway_response_code: &str,
    gateway_response_status: &str,
) -> enums::RefundStatus {
    let refund_status = if refund_type.eq_ignore_ascii_case("UDIR") {
        RefundStatus::from_udir_gateway_code(gateway_response_code, gateway_response_status)
    } else {
        RefundStatus::from_offline_gateway_code(gateway_response_code, gateway_response_status)
    };

    match refund_status {
        RefundStatus::Success => enums::RefundStatus::Success,
        RefundStatus::Pending => enums::RefundStatus::Pending,
        RefundStatus::Deemed => enums::RefundStatus::Pending,
        RefundStatus::Failed => enums::RefundStatus::Failure,
    }
}

/// Sanitize merchant request ID to meet API constraints
/// Max 35 chars, alphanumeric + hyphen + dot + underscore only
pub fn sanitize_merchant_request_id(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.' || *c == '_')
        .collect();

    if sanitized.len() > 35 {
        sanitized[..35].to_string()
    } else {
        sanitized
    }
}

/// Convert minor units (paise) to major units (rupees) with 2 decimal places
pub fn minor_to_major_amount(amount: i64) -> String {
    let major = amount / 100;
    let minor = amount % 100;
    format!("{}.{:02}", major, minor.abs())
}

/// Build error response from Juspay UPI API error
pub fn build_error_response(
    status_code: u16,
    response_code: &str,
    response_message: &str,
) -> ErrorResponse {
    use crate::connectors::juspay_upi_stack::constants::*;

    let attempt_status = match response_code {
        // Failure cases
        RESPONSE_CODE_UNAUTHORIZED
        | RESPONSE_CODE_REQUEST_EXPIRED
        | RESPONSE_CODE_DROPOUT
        | RESPONSE_CODE_FAILURE
        | RESPONSE_CODE_BAD_REQUEST
        | RESPONSE_CODE_INVALID_DATA
        | RESPONSE_CODE_INVALID_MERCHANT
        | RESPONSE_CODE_DEVICE_FINGERPRINT_MISMATCH
        | RESPONSE_CODE_INTERNAL_SERVER_ERROR
        | RESPONSE_CODE_INVALID_TRANSACTION_ID
        | RESPONSE_CODE_UNINITIATED_REQUEST
        | RESPONSE_CODE_INVALID_REFUND_AMOUNT => Some(enums::AttemptStatus::Failure),
        // Non-terminal / pending cases - no attempt_status override
        RESPONSE_CODE_SUCCESS
        | RESPONSE_CODE_REQUEST_NOT_FOUND
        | RESPONSE_CODE_REQUEST_PENDING
        | RESPONSE_CODE_SERVICE_UNAVAILABLE
        | RESPONSE_CODE_GATEWAY_TIMEOUT
        | RESPONSE_CODE_DUPLICATE_REQUEST => None,
        // Any other unknown code defaults to failure
        _ => Some(enums::AttemptStatus::Failure),
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

/// Extract merchant_id and merchant_channel_id from metadata.
/// Shared across all UPI bank connectors — every bank reads these from metadata.
pub fn extract_merchant_identifiers_from_metadata(
    metadata: &Option<SecretSerdeValue>,
) -> Result<(String, String), error_stack::Report<IntegrationError>> {
    let metadata_value = metadata
        .as_ref()
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "metadata",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Provide merchant_id and merchant_channel_id in request metadata".to_string(),
                ),
                doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack".to_string()),
                additional_context: Some(
                    "metadata must contain 'merchant_id' and 'merchant_channel_id' fields"
                        .to_string(),
                ),
            },
        })?
        .peek();

    let merchant_id = metadata_value
        .get("merchant_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "metadata.merchant_id",
            context: IntegrationErrorContext {
                suggested_action: Some("Add 'merchant_id' field to request metadata".to_string()),
                doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack".to_string()),
                additional_context: Some(
                    "merchant_id is required for all Juspay UPI Stack bank connectors".to_string(),
                ),
            },
        })?
        .to_string();

    let merchant_channel_id = metadata_value
        .get("merchant_channel_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "metadata.merchant_channel_id",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Add 'merchant_channel_id' field to request metadata".to_string(),
                ),
                doc_url: Some("https://juspay.io/in/docs/upi-merchant-stack".to_string()),
                additional_context: Some(
                    "merchant_channel_id is required for all Juspay UPI Stack bank connectors"
                        .to_string(),
                ),
            },
        })?
        .to_string();

    Ok((merchant_id, merchant_channel_id))
}

// ============================================================
// GENERIC REQUEST BUILDERS
// All UPI bank connectors share the same request structure.
// Each bank calls these with its JuspayUpiAuthConfig extracted
// from the bank-specific ConnectorSpecificConfig variant.
// ============================================================

/// Build a JWS-signed Authorize (Register Intent) request body.
///
/// Banks call this from their `TryFrom` impl after extracting their auth config.
pub fn build_authorize_request<T: PaymentMethodDataTypes + serde::Serialize>(
    router_data: &RouterDataV2<
        domain_types::connector_flow::Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    auth: &JuspayUpiAuthConfig,
    amount_str: String,
) -> Result<JwsObject, error_stack::Report<IntegrationError>> {
    use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;

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
        &router_data
            .resource_common_data
            .connector_request_reference_id,
    );
    // UPI Request ID — must be strictly alphanumeric (max 35 chars, no hyphens/dots/underscores)
    let upi_request_id: String = merchant_request_id
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    // Build remarks from description
    let remarks = router_data.resource_common_data.description.clone();

    let ref_url = router_data.request.router_return_url.clone();

    let register_intent = RegisterIntentRequest {
        merchant_request_id,
        upi_request_id,
        amount: amount_str,
        flow: RegisterIntentFlowType::Transaction,
        intent_request_expiry_minutes: Some(intent_expiry),
        remarks,
        ref_url,
        iat: get_current_timestamp_ms(),
    };

    let payload_json = serde_json::to_string(&register_intent)
        .change_context(IntegrationError::RequestEncodingFailed {
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Verify all request fields have valid formats and lengths".to_string(),
            ),
            doc_url: Some(
                "https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/register-intent"
                    .to_string(),
            ),
            additional_context: Some(
                "upi_request_id must be max 35 alphanumeric characters. Amount must be a string."
                    .to_string(),
            ),
        },
    })?;

    sign_jws(
        &payload_json,
        &auth.merchant_private_key,
        &auth.merchant_kid,
    )
}

/// Build a JWS-signed PSync (Status 360) request body.
pub fn build_psync_request(
    merchant_request_id: String,
    auth: &JuspayUpiAuthConfig,
) -> Result<JwsObject, error_stack::Report<IntegrationError>> {
    use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;

    let status_request = Status360Request {
        merchant_request_id,
        transaction_type: Status360TransactionType::MerchantCreditedViaPay,
        iat: get_current_timestamp_ms(),
    };

    let payload_json =
        serde_json::to_string(&status_request).change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Verify merchant_request_id is valid and transaction_type is correctly set"
                        .to_string(),
                ),
                doc_url: Some(
                    "https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/transaction-status-360"
                        .to_string(),
                ),
                additional_context: Some(
                    "transaction_type should be 'MERCHANT_CREDITED_VIA_PAY' for payment status queries."
                        .to_string(),
                ),
            },
        })?;

    sign_jws(
        &payload_json,
        &auth.merchant_private_key,
        &auth.merchant_kid,
    )
}

/// Build a JWS-signed Refund (Refund 360) request body.
pub fn build_refund_request(
    refunds_data: &RefundsData,
    auth: &JuspayUpiAuthConfig,
) -> Result<JwsObject, error_stack::Report<IntegrationError>> {
    use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;
    use crate::connectors::juspay_upi_stack::types::{
        AdjustmentCode, AdjustmentFlag, Refund360Type,
    };

    // Determine refund type (default to Offline for safety)
    let refund_type = refunds_data
        .connector_feature_data
        .as_ref()
        .and_then(|m| m.peek().get("refund_type").cloned())
        .and_then(|v| {
            v.as_str().map(|s| match s.to_uppercase().as_str() {
                "UDIR" => Refund360Type::Udir,
                "ONLINE" => Refund360Type::Online,
                _ => Refund360Type::Offline,
            })
        })
        .unwrap_or(Refund360Type::Offline);

    // Get adjustment code and flag for UDIR refunds
    let (adj_code, adj_flag) = if matches!(refund_type, Refund360Type::Udir) {
        (
            Some(AdjustmentCode::GoodsNotProvided),
            Some(AdjustmentFlag::Ref),
        )
    } else {
        (None, None)
    };

    // Convert minor units (paise) to rupees with 2 decimal places using integer arithmetic
    let amount_minor = refunds_data.minor_refund_amount.get_amount_as_i64();
    let refund_amount = minor_to_major_amount(amount_minor);

    let refund_request = Refund360Request {
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
        iat: get_current_timestamp_ms(),
    };

    let payload_json =
        serde_json::to_string(&refund_request).change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Verify refund fields are valid: original_merchant_request_id, refund_amount, and refund_type"
                        .to_string(),
                ),
                doc_url: Some(
                    "https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/refund-360"
                        .to_string(),
                ),
                additional_context: Some(
                    "refund_type can be 'ONLINE', 'OFFLINE', or 'UDIR'. refund_amount must be in rupees with 2 decimal places (e.g., '100.00')."
                        .to_string(),
                ),
            },
        })?;

    sign_jws(
        &payload_json,
        &auth.merchant_private_key,
        &auth.merchant_kid,
    )
}

/// Build a JWS-signed RSync (Refund Status 360) request body.
pub fn build_rsync_request(
    connector_transaction_id: String,
    connector_refund_id: String,
    auth: &JuspayUpiAuthConfig,
) -> Result<JwsObject, error_stack::Report<IntegrationError>> {
    use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;
    use crate::connectors::juspay_upi_stack::types::Refund360Type;

    let refund_sync = Refund360Request {
        original_merchant_request_id: connector_transaction_id,
        refund_request_id: connector_refund_id,
        refund_type: Refund360Type::Offline, // Default for sync
        refund_amount: "0.00".to_string(),   // Not needed for status check
        remarks: "Status check".to_string(),
        adj_code: None,
        adj_flag: None,
        iat: get_current_timestamp_ms(),
    };

    let payload_json =
        serde_json::to_string(&refund_sync).change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Verify connector_refund_id is provided for refund status query".to_string(),
                ),
                doc_url: Some(
                    "https://juspay.io/in/docs/upi-merchant-stack/docs/transactions/refund-360"
                        .to_string(),
                ),
                additional_context: Some(
                    "For RSync, connector_refund_id is the refund_request_id from the original Refund request."
                        .to_string(),
                ),
            },
        })?;

    sign_jws(
        &payload_json,
        &auth.merchant_private_key,
        &auth.merchant_kid,
    )
}

// ============================================================
// GENERIC RESPONSE HANDLERS
// All UPI bank connectors produce the same RouterDataV2 shape
// from the shared JuspayUpiApiResponse<T> types.
// ============================================================

/// Handle Authorize (Register Intent) response — shared across all UPI bank connectors.
pub fn handle_authorize_response<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static,
>(
    response: RegisterIntentResponse,
    http_code: u16,
    router_data: RouterDataV2<
        domain_types::connector_flow::Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<
    RouterDataV2<
        domain_types::connector_flow::Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    error_stack::Report<ConnectorError>,
> {
    if response.response_code.is_failure() {
        let status = enums::AttemptStatus::Failure;
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
        let deeplink = construct_upi_deeplink(&payload);
        let redirect_form = RedirectForm::Uri { uri: deeplink };

        let response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(payload.merchant_request_id.clone()),
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
        // Success outer code but no payload — treat as pending
        let status = enums::AttemptStatus::Pending;
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

/// Handle PSync (Status 360) response — shared across all UPI bank connectors.
pub fn handle_psync_response(
    response: Status360Response,
    http_code: u16,
    router_data: RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        PaymentsSyncData,
        PaymentsResponseData,
    >,
) -> Result<
    RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        PaymentsSyncData,
        PaymentsResponseData,
    >,
    error_stack::Report<ConnectorError>,
> {
    if response.response_code.is_failure() {
        let status = enums::AttemptStatus::Failure;
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

    let status = if let Some(ref payload) = response.payload {
        map_transaction_status(
            response.response_code.clone(),
            Some(&payload.gateway_response_code),
        )
    } else {
        map_transaction_status(response.response_code.clone(), None)
    };

    let response_data = PaymentsResponseData::TransactionResponse {
        resource_id: response
            .payload
            .as_ref()
            .map(|p| ResponseId::ConnectorTransactionId(p.merchant_request_id.clone()))
            .unwrap_or(ResponseId::NoResponseId),
        redirection_data: None,
        connector_metadata: None,
        mandate_reference: None,
        network_txn_id: None,
        connector_response_reference_id: response
            .payload
            .as_ref()
            .map(|p| p.merchant_request_id.clone()),
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

/// Handle Refund (Refund 360) response — shared across all UPI bank connectors.
pub fn handle_refund_response(
    response: Refund360Response,
    http_code: u16,
    router_data: RouterDataV2<
        domain_types::connector_flow::Refund,
        RefundFlowData,
        RefundsData,
        RefundsResponseData,
    >,
) -> Result<
    RouterDataV2<
        domain_types::connector_flow::Refund,
        RefundFlowData,
        RefundsData,
        RefundsResponseData,
    >,
    error_stack::Report<ConnectorError>,
> {
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

/// Handle RSync (Refund Status 360) response — shared across all UPI bank connectors.
pub fn handle_rsync_response(
    response: Refund360Response,
    http_code: u16,
    router_data: RouterDataV2<
        domain_types::connector_flow::RSync,
        RefundFlowData,
        RefundSyncData,
        RefundsResponseData,
    >,
) -> Result<
    RouterDataV2<
        domain_types::connector_flow::RSync,
        RefundFlowData,
        RefundSyncData,
        RefundsResponseData,
    >,
    error_stack::Report<ConnectorError>,
> {
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

/// Build the standard request headers for Juspay UPI Merchant Stack APIs.
///
/// This function constructs the common headers shared across all banks in the UPI Stack
/// (Axis Bank, YES Bank, Kotak, RBL, AU Bank, etc.). The headers are:
/// - content-type: application/json
/// - x-merchant-id
/// - x-merchant-channel-id
/// - x-timestamp: current Unix timestamp in milliseconds
/// - jpupi-routing-id: the transaction/request ID (value differs per flow)
///
/// Banks can use this function and optionally add additional headers (e.g., signature headers).
pub fn build_request_headers(
    merchant_id: &str,
    merchant_channel_id: &str,
    routing_id: &str,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    use crate::connectors::juspay_upi_stack::constants::*;
    use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;

    let headers = vec![
        (
            CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        ),
        (X_MERCHANT_ID.to_string(), merchant_id.to_string().into()),
        (
            X_MERCHANT_CHANNEL_ID.to_string(),
            merchant_channel_id.to_string().into(),
        ),
        (X_TIMESTAMP.to_string(), get_current_timestamp_ms().into()),
        (JPUP_ROUTING_ID.to_string(), routing_id.to_string().into()),
    ];
    Ok(headers)
}
