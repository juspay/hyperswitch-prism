use crate::{
    connector_types::{ConnectorResponseHeaders, RawConnectorRequestResponse},
    types::Connectors,
};
use common_enums::Currency;
use common_utils::types::MinorUnit;
use hyperswitch_masking::Secret;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct SurchargeFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub connector_request_reference_id: String,
    pub connectors: std::sync::Arc<Connectors>,
    pub raw_connector_response: Option<Secret<String>>,
    pub typed_connector_response: Option<String>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
    pub connector_response_headers: Option<http::HeaderMap>,
}

impl RawConnectorRequestResponse for SurchargeFlowData {
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

impl ConnectorResponseHeaders for SurchargeFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

/// Strategy for handling calculated surcharge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurchargeStrategy {
    /// Apply the calculated surcharge to the payment
    Apply,
    /// Do not apply, just return the calculated amount
    Waive,
}

impl From<grpc_api_types::surcharge::SurchargeStrategy> for SurchargeStrategy {
    fn from(value: grpc_api_types::surcharge::SurchargeStrategy) -> Self {
        match value {
            grpc_api_types::surcharge::SurchargeStrategy::Unspecified
            | grpc_api_types::surcharge::SurchargeStrategy::Apply => Self::Apply,
            grpc_api_types::surcharge::SurchargeStrategy::Waive => Self::Waive,
        }
    }
}

/// Request data for surcharge calculation
#[derive(Debug, Clone)]
pub struct SurchargeCalculateRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub previous_connector_surcharge_id: Option<String>,
    pub surcharge_strategy: Option<SurchargeStrategy>,
    pub card_bin: String,
    pub postal_code: Secret<String>,
    pub country: Option<common_enums::CountryAlpha2>,
}

/// Request data for surcharge payment succeeded notification
#[derive(Debug, Clone)]
pub struct SurchargePaymentSucceededRequest {
    pub connector_surcharge_id: String,
}

/// Request data for surcharge refund succeeded notification
#[derive(Debug, Clone)]
pub struct SurchargeRefundSucceededRequest {
    pub connector_surcharge_id: String,
}

/// Response data for surcharge payment succeeded notification
#[derive(Debug, Clone)]
pub struct SurchargePaymentSucceededResponse {
    pub status_code: u16,
}

/// Response data for surcharge refund succeeded notification
#[derive(Debug, Clone)]
pub struct SurchargeRefundSucceededResponse {
    pub status_code: u16,
}

/// Integrity object for surcharge payment succeeded notification
#[derive(Debug, Clone, PartialEq)]
pub struct SurchargePaymentSucceededIntegrityObject {
    pub connector_surcharge_id: String,
}

/// Integrity object for surcharge refund succeeded notification
#[derive(Debug, Clone, PartialEq)]
pub struct SurchargeRefundSucceededIntegrityObject {
    pub connector_surcharge_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurchargeCalculateIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

/// Response data from surcharge calculation
#[derive(Debug, Clone)]
pub struct SurchargeCalculateResponse {
    pub connector_response_reference_id: Option<String>,
    pub surcharge_amount: MinorUnit,
    pub surcharge_rate_percent: f64,
    pub connector_surcharge_id: String,
    pub currency: Currency,
}
