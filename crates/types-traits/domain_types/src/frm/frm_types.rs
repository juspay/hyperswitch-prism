use crate::{
    connector_types::{
        ConnectorResponseHeaders, CustomerInfo, RawConnectorRequestResponse,
        ServerAuthenticationTokenResponseData,
    },
    mandates::MandateAmountData,
    payment_address::{OrderDetailsWithAmount, PaymentAddress},
    payment_method_data::{DefaultPCIHolder, PaymentMethodData},
    router_request_types::BrowserInformation,
    types::Connectors,
};
use common_enums::{AttemptStatus, FrmDecision, PaymentMethodType};
use common_utils::types::Money;
use hyperswitch_masking::Secret;

#[derive(Debug, Clone)]
pub struct FrmFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub connectors: Connectors,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
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
    pub mandate_info: Option<MandateAmountData>,
    /// Merchant details (id + MCC) for risk scoring.
    pub merchant_details: Option<MerchantDetails>,
    /// Payment method sub-type (e.g. `Card`, `GooglePay`, `UpiCollect`) for risk scoring.
    pub payment_method_type: Option<PaymentMethodType>,
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
