use crate::connectors::reddot::transformers::deserialize_response_code;
use common_utils::types::StringMajorUnit;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

/// RDP notify_url callback body. Same field shapes as the query_redirection
/// data response (`response_code` serialized the same string-or-number way),
/// so the status mapping intentionally mirrors the PSync transform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReddotWebhookBody {
    pub mid: Option<String>,
    /// Connector order ID sent at authorization and echoed by RDP.
    pub order_id: Option<String>,
    /// RDP's gateway-side id for this payment (`transaction_id`).
    pub transaction_id: Option<String>,
    /// Echo of the caller transaction reference sent at authorization.
    pub merchant_reference: Option<String>,
    #[serde(deserialize_with = "deserialize_response_code")]
    pub response_code: String,
    pub response_msg: Option<String>,
    pub authorized_amount: Option<StringMajorUnit>,
    pub authorized_ccy: Option<String>,
    pub response_status: Option<String>,
    // ---- remaining fields of the notify_url record (same shape as the
    // query_redirection data response) — modeled so the webhook's
    // `raw_connector_response` round-trip is lossless, like the psync one.
    pub request_mid: Option<String>,
    pub request_amount: Option<String>,
    pub request_ccy: Option<String>,
    pub acquirer_transaction_id: Option<String>,
    pub acquirer_authorized_amount: Option<String>,
    pub acquirer_authorized_ccy: Option<String>,
    pub acquirer_response_code: Option<String>,
    pub acquirer_response_msg: Option<String>,
    pub acquirer_authorization_code: Option<String>,
    pub acquirer_mpi_eci: Option<String>,
    pub created_timestamp: Option<String>,
    pub acquirer_created_timestamp: Option<String>,
    pub request_timestamp: Option<String>,
    pub first_6: Option<Secret<String>>,
    pub last_4: Option<Secret<String>>,
    pub payer_name: Option<Secret<String>>,
    pub payer_id: Option<String>,
    pub exp_date: Option<Secret<String>>,
    pub transaction_type: Option<String>,
    pub payment_mode: Option<String>,
    pub signature: Option<String>,
}
