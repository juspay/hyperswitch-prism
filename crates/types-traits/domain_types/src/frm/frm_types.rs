use crate::{
    connector_types::{
        ConnectorResponseHeaders, CustomerInfo, DomainData, RawConnectorRequestResponse,
        ServerAuthenticationTokenResponseData,
    },
    mandates::MandateAmountData,
    payment_address::{OrderDetailsWithAmount, PaymentAddress},
    payment_method_data::{CustomerDocumentDetails, DefaultPCIHolder, PaymentMethodData},
    router_request_types::BrowserInformation,
    types::Connectors,
};
use common_enums::{AttemptStatus, FrmDecision, PaymentMethodType};
use common_utils::types::Money;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FrmFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub connectors: Arc<Connectors>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub raw_connector_response: Option<Secret<String>>,
    pub typed_connector_response: Option<String>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
    pub connector_response_headers: Option<http::HeaderMap>,
}

impl RawConnectorRequestResponse for FrmFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }

    fn set_typed_connector_response(&mut self, response: Option<String>) {
        self.typed_connector_response = response;
    }

    fn get_typed_connector_response(&self) -> Option<String> {
        self.typed_connector_response.clone()
    }

    fn set_typed_connector_request(&mut self, request: Option<String>) {
        self.typed_connector_request = request;
    }

    fn get_typed_connector_request(&self) -> Option<String> {
        self.typed_connector_request.clone()
    }
}

impl ConnectorResponseHeaders for FrmFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

/// Merchant details used for FRM risk scoring.
#[derive(Debug, Clone, Default)]
pub struct MerchantDetails {
    pub merchant_id: Option<String>,
    pub merchant_category_code: Option<u32>,
}

/// Request data for pre-risk check
#[derive(Debug, Clone)]
pub struct PreRiskCheckRequest {
    pub amount: Money,
    pub customer_info: Option<CustomerInfo>,
    pub payment_method: Option<PaymentMethodData<DefaultPCIHolder>>,
    pub browser_info: Option<BrowserInformation>,
    pub merchant_transaction_id: Option<String>,
    pub order_details: Option<Vec<OrderDetailsWithAmount>>,
    pub address: Option<PaymentAddress>,
    pub metadata: Option<Secret<String>>,
    pub connector_feature_data: Option<Secret<String>>,
    pub test_mode: Option<bool>,
    /// Recurring / subscription details for risk scoring (shared MandateAmountData;
    /// `amount` is the per-period billing amount, `frequency` the billing period).
    pub mandate_details: Option<MandateAmountData>,
    /// Merchant details (id + MCC) for risk scoring.
    pub merchant_details: Option<MerchantDetails>,
    /// Payment method sub-type (e.g. `Card`, `GooglePay`, `UpiCollect`) for risk scoring.
    pub payment_method_type: Option<PaymentMethodType>,
    /// Payer's identity document (e.g. Brazilian CPF/CNPJ in `document_number`).
    /// The buyer's document, when distinct, is sent as `DocumentKind.CUSTOMER_CPF` —
    /// ClearSale maps CPF/CNPJ → billing.primaryDocument and CUSTOMER_CPF →
    /// shipping.primaryDocument (falling back to billing when absent).
    pub document_details: Option<CustomerDocumentDetails>,
    /// Domain-specific order context (travel, hotel, airline…) as a typed
    /// object. Replaces the earlier free-form merchant_transient_info JSON.
    pub domain_data: Option<DomainData>,
}

/// Response data for pre-risk check
#[derive(Debug, Clone)]
pub struct PreRiskCheckResponse {
    pub frm_decision: Option<FrmDecision>,
    pub risk_score: Option<i32>,
    pub reason: Option<String>,
    pub frm_transaction_id: Option<String>,
    pub status_code: u16,
}

/// Request data for post-risk check
#[derive(Debug, Clone)]
pub struct PostRiskCheckRequest {
    pub amount: Money,
    pub customer_info: Option<CustomerInfo>,
    pub payment_method: Option<PaymentMethodData<DefaultPCIHolder>>,
    pub merchant_transaction_id: Option<String>,
    pub order_details: Option<Vec<OrderDetailsWithAmount>>,
    pub metadata: Option<Secret<String>>,
    pub connector_feature_data: Option<Secret<String>>,
    pub test_mode: Option<bool>,
    pub payment_status: Option<AttemptStatus>,
    pub connector_transaction_id: Option<String>,
    pub payment_connector: Option<grpc_api_types::payments::Connector>,
    pub address: Option<PaymentAddress>,
}

/// Response data for post-risk check
#[derive(Debug, Clone)]
pub struct PostRiskCheckResponse {
    pub frm_decision: Option<FrmDecision>,
    pub risk_score: Option<i32>,
    pub reason: Option<String>,
    pub frm_transaction_id: Option<String>,
    pub status_code: u16,
}

// ── FRM Notification Requests ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FrmPaymentOutcomeRequest {
    pub connector_transaction_id: Option<String>,
    pub amount: Money,
    pub frm_transaction_id: Option<String>,
    pub payment_status: Option<AttemptStatus>,
    pub merchant_transaction_id: Option<String>,
    pub frm_decision: Option<FrmDecision>,
    /// Merchant details (id + MCC) for the Update Order call.
    pub merchant_details: Option<MerchantDetails>,
    /// Connector-specific feature data (e.g. AVS/CVV verification results) for the Update Order call.
    pub connector_feature_data: Option<Secret<String>>,
}

#[derive(Debug, Clone)]
pub struct FrmRefundProcessedRequest {
    pub connector_transaction_id: Option<String>,
    pub amount: Money,
    pub frm_transaction_id: Option<String>,
    pub connector_refund_id: Option<String>,
    pub merchant_refund_id: Option<String>,
    pub refund_reason: Option<String>,
    pub frm_decision: Option<FrmDecision>,
    /// Merchant details (id + MCC) for the Update Order call.
    pub merchant_details: Option<MerchantDetails>,
    /// Connector-specific feature data for the notify call.
    pub connector_feature_data: Option<Secret<String>>,
}

#[derive(Debug, Clone)]
pub struct FrmChargebackReceivedRequest {
    pub connector_transaction_id: Option<String>,
    pub amount: Money,
    pub frm_transaction_id: Option<String>,
    pub connector_dispute_id: Option<String>,
    pub merchant_dispute_id: Option<String>,
    pub chargeback_reason: Option<String>,
    pub frm_decision: Option<FrmDecision>,
    /// Connector-specific feature data for the notify call.
    pub connector_feature_data: Option<Secret<String>>,
}

// ── FRM Notification Responses ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FrmPaymentOutcomeResponse {
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct FrmRefundProcessedResponse {
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct FrmChargebackReceivedResponse {
    pub status_code: u16,
}

// ────────────────────────────────────────────────────────────────────────────
// Generic FRM request metadata
//
// Connectors across FRM providers (ClearSale, Kount, nSure, …) all receive a
// JSON blob on `PreRiskCheckRequest::metadata` / `PostRiskCheckRequest::
// metadata`. The merchant-caller (euler-api-txns / Hyperswitch) fills it with
// a provider-agnostic shape — session id, free-form custom fields, and the
// travel/hospitality blocks (passengers, travel segments, accommodations) used
// by transport and travel merchants. Decoding it once here keeps each
// connector's transformers free of a duplicated, near-identical JSON parse.
// ────────────────────────────────────────────────────────────────────────────

/// The shared FRM request-metadata carrier (euler `UcsFrmMetadata`). All
/// fields are optional; a connector reads only the blocks it maps onto its own
/// wire contract and ignores the rest.
///
/// Serialized camelCase on the wire to match euler's JSON encoding of
/// `UTCTime`/`Day` as ISO-8601 strings and the Haskell field names. euler's
/// `UcsFrmMetadata` actually encodes its keys in snake_case (`camelTo2 '_'`
/// on the Haskell record fields), so every key also carries its snake_case
/// spelling as an `alias` — callers sending either casing decode the same.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrmMetadata {
    /// Merchant's own order identifier (free-form, not the UCS reference).
    #[serde(default, alias = "merchant_order_id")]
    pub merchant_order_id: Option<String>,
    /// End-user IP as supplied by the merchant.
    #[serde(default, alias = "user_ip")]
    pub user_ip: Option<String>,
    /// Device/session id from the provider's device-fingerprint SDK.
    #[serde(default, rename = "sessionId", alias = "session_id")]
    pub session_id: Option<String>,
    /// Free-form merchant key/value pairs; providers map a subset onto their
    /// own custom-field contract.
    #[serde(default, alias = "custom_fields")]
    pub custom_fields: Option<std::collections::HashMap<String, serde_json::Value>>,
    // Travel/hotel blocks (passengers / travelSegments / accommodations) are
    // intentionally NOT consumed here — they ride on the typed
    // `PreRiskCheckRequest::domain_data` (DomainData.travel_data / hotel_data).
}

impl FrmMetadata {
    /// Decode the metadata blob carried on an FRM risk-check request. A
    /// missing or malformed blob yields the default (all-empty) metadata so
    /// the connector never has to special-case "no metadata".
    pub fn parse(raw: Option<&Secret<String>>) -> Self {
        raw.and_then(|data| serde_json::from_str(data.peek()).ok())
            .unwrap_or_default()
    }

    /// A string-valued custom field by key; nested/non-string values are
    /// ignored. Convenience over digging into `custom_fields` by hand.
    pub fn custom_field_str(&self, key: &str) -> Option<String> {
        self.custom_fields.as_ref().and_then(|fields| {
            fields
                .get(key)
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
    }

    /// The raw custom-fields map, when present — for providers (e.g. Kount)
    /// that forward the whole map with their own key/value validation.
    pub fn custom_fields_map(
        &self,
    ) -> Option<&std::collections::HashMap<String, serde_json::Value>> {
        self.custom_fields.as_ref()
    }
}

