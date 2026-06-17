use crate::{
    connector_types::{ConnectorResponseHeaders, CustomerInfo, RawConnectorRequestResponse},
    errors::IntegrationError,
    payment_address::OrderDetailsWithAmount,
    payment_method_data::{DefaultPCIHolder, PaymentMethodData},
    router_request_types::BrowserInformation,
    types::Connectors,
    utils::{extract_merchant_id_from_metadata, ForeignFrom, ForeignTryFrom},
};
use common_enums::{AttemptStatus, Currency, FrmDecision};
use common_utils::types::MinorUnit;
use error_stack::ResultExt;
use hyperswitch_masking::Secret;

impl ForeignFrom<grpc_api_types::frm::FrmDecision> for FrmDecision {
    fn foreign_from(value: grpc_api_types::frm::FrmDecision) -> Self {
        match value {
            grpc_api_types::frm::FrmDecision::Approve => Self::Approve,
            grpc_api_types::frm::FrmDecision::Reject => Self::Reject,
            grpc_api_types::frm::FrmDecision::Unspecified
            | grpc_api_types::frm::FrmDecision::Review => Self::Review,
            grpc_api_types::frm::FrmDecision::Error => Self::Error,
        }
    }
}

impl ForeignFrom<FrmDecision> for grpc_api_types::frm::FrmDecision {
    fn foreign_from(value: FrmDecision) -> Self {
        match value {
            FrmDecision::Approve => Self::Approve,
            FrmDecision::Reject => Self::Reject,
            FrmDecision::Review => Self::Review,
            FrmDecision::Error => Self::Error,
        }
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::frm::FrmServicePreRiskCheckRequest,
        Connectors,
        &common_utils::metadata::MaskedMetadata,
    )> for FrmFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (_value, connectors, metadata): (
            grpc_api_types::frm::FrmServicePreRiskCheckRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::FrmServicePreRiskCheckRequest> for PreRiskCheckRequest {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::FrmServicePreRiskCheckRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Amount is required for pre-risk check".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in pre-risk check request".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        Ok(Self {
            amount: MinorUnit::new(amount.minor_amount),
            currency,
            customer_info: None,
            payment_method: None,
            browser_info: None,
            merchant_transaction_id: value.merchant_transaction_id,
            order_details: None,
            address: None,
            metadata: value.metadata,
            connector_feature_data: value.connector_feature_data,
            test_mode: value.test_mode,
        })
    }
}

impl
    ForeignTryFrom<(
        grpc_api_types::frm::FrmServicePostRiskCheckRequest,
        Connectors,
        &common_utils::metadata::MaskedMetadata,
    )> for FrmFlowData
{
    type Error = IntegrationError;

    fn foreign_try_from(
        (_value, connectors, metadata): (
            grpc_api_types::frm::FrmServicePostRiskCheckRequest,
            Connectors,
            &common_utils::metadata::MaskedMetadata,
        ),
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let merchant_id = extract_merchant_id_from_metadata(metadata)?;

        Ok(Self {
            merchant_id,
            connectors,
            raw_connector_response: None,
            raw_connector_request: None,
            connector_response_headers: None,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::frm::FrmServicePostRiskCheckRequest> for PostRiskCheckRequest {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::frm::FrmServicePostRiskCheckRequest,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let amount = value.amount.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some("Amount is required for post-risk check".to_owned()),
                    ..Default::default()
                },
            })
        })?;

        let currency = {
            let curr = grpc_api_types::payments::Currency::try_from(amount.currency)
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "currency",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Invalid currency in post-risk check request".to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            common_enums::Currency::foreign_try_from(curr)?
        };

        let payment_status = value.payment_status.and_then(|status| {
            grpc_api_types::payments::PaymentStatus::try_from(status)
                .ok()
                .and_then(|payment_status| AttemptStatus::foreign_try_from(payment_status).ok())
        });

        let payment_connector = value
            .payment_connector
            .and_then(|c| grpc_api_types::payments::Connector::try_from(c).ok());

        Ok(Self {
            amount: MinorUnit::new(amount.minor_amount),
            currency,
            customer_info: None,
            payment_method: None,
            merchant_transaction_id: value.merchant_transaction_id,
            order_details: None,
            metadata: value.metadata,
            connector_feature_data: value.connector_feature_data,
            test_mode: value.test_mode,
            payment_status,
            connector_transaction_id: value.connector_transaction_id,
            payment_connector,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FrmFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub connectors: Connectors,
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

/// Request data for pre-risk check
#[derive(Debug, Clone)]
pub struct PreRiskCheckRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub customer_info: Option<CustomerInfo>,
    pub payment_method: Option<PaymentMethodData<DefaultPCIHolder>>,
    pub browser_info: Option<BrowserInformation>,
    pub merchant_transaction_id: Option<String>,
    pub order_details: Option<Vec<OrderDetailsWithAmount>>,
    pub address: Option<crate::payment_address::PaymentAddress>,
    pub metadata: Option<Secret<String>>,
    pub connector_feature_data: Option<Secret<String>>,
    pub test_mode: Option<bool>,
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
    pub amount: MinorUnit,
    pub currency: Currency,
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
