//! Transformer utilities for Juspay UPI Merchant Stack
//!
//! This module provides helper functions for:
//! - Constructing UPI deeplinks
//! - Mapping response status codes
//! - Building request/response structures

use crate::connectors::juspay_upi_stack::{
    constants::*,
    types::*,
};
use common_enums as enums;
use domain_types::{
    router_data::ErrorResponse,
};

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
    params_map.insert("mode", params.txn_initiation_mode.clone().unwrap_or_else(|| DEFAULT_TXN_INITIATION_MODE.to_string()));
    
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
pub fn map_transaction_status(
    outer_response_code: &str,
    gateway_response_code: Option<&str>,
) -> enums::AttemptStatus {
    // First check outer response code
    match outer_response_code {
        RESPONSE_CODE_REQUEST_NOT_FOUND => return enums::AttemptStatus::Pending,
        RESPONSE_CODE_REQUEST_EXPIRED => return enums::AttemptStatus::Failure,
        RESPONSE_CODE_DROPOUT => return enums::AttemptStatus::Failure,
        RESPONSE_CODE_FAILURE => return enums::AttemptStatus::Failure,
        _ => {}
    }
    
    // If SUCCESS, check gateway response code
    if outer_response_code == RESPONSE_CODE_SUCCESS {
        if let Some(code) = gateway_response_code {
            match code {
                GATEWAY_RESPONSE_CODE_SUCCESS => return enums::AttemptStatus::Charged,
                GATEWAY_RESPONSE_CODE_PENDING => return enums::AttemptStatus::Pending,
                _ => return enums::AttemptStatus::Failure,
            }
        }
        // If no gateway code, treat as pending (waiting for webhook/psync)
        return enums::AttemptStatus::Pending;
    }
    
    enums::AttemptStatus::Failure
}

/// Map refund status from gateway response
pub fn map_refund_status(
    refund_type: &str,
    gateway_response_code: &str,
    gateway_response_status: &str,
) -> enums::RefundStatus {
    let refund_status = if refund_type == REFUND_TYPE_UDIR {
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

/// Truncate remarks to max 50 alphanumeric characters
pub fn sanitize_remarks(remarks: &str) -> String {
    let alphanumeric_only: String = remarks
        .chars()
        .take(50)
        .collect();
    alphanumeric_only
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
