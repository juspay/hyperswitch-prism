use super::feature_matrix_types::{
    FeatureMatrixConnector, FeatureMatrixError, FeatureMatrixPaymentMethod, FeatureMatrixResponse,
};
use tonic::Status;

impl From<FeatureMatrixError> for Status {
    fn from(error: FeatureMatrixError) -> Self {
        match &error {
            FeatureMatrixError::InvalidConnectorName(_) => {
                Status::invalid_argument(error.message())
            }
            FeatureMatrixError::ConnectorNotConfigured(_) => Status::unimplemented(error.message()),
        }
    }
}

impl From<FeatureMatrixResponse> for grpc_api_types::payments::FeatureMatrixResponse {
    fn from(response: FeatureMatrixResponse) -> Self {
        Self {
            connector_count: u32::try_from(response.connector_count).unwrap_or(u32::MAX),
            connectors: response.connectors.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<FeatureMatrixConnector> for grpc_api_types::payments::FeatureMatrixConnector {
    fn from(connector: FeatureMatrixConnector) -> Self {
        Self {
            connector_name: connector.name,
            display_name: connector.display_name,
            description: connector.description,
            base_url: connector.base_url,
            category: connector.category,
            integration_status: connector.integration_status.to_string(),
            supported_payment_methods: connector
                .supported_payment_methods
                .into_iter()
                .map(Into::into)
                .collect(),
            supported_webhook_flows: connector.supported_webhook_flows,
        }
    }
}

impl From<FeatureMatrixPaymentMethod> for grpc_api_types::payments::FeatureMatrixPaymentMethod {
    fn from(payment_method: FeatureMatrixPaymentMethod) -> Self {
        Self {
            payment_method: payment_method.payment_method,
            payment_method_type: payment_method.payment_method_type,
            payment_method_type_display_name: payment_method.payment_method_type_display_name,
            mandates: payment_method.mandates,
            refunds: payment_method.refunds,
            supported_capture_methods: payment_method.supported_capture_methods,
            three_ds: payment_method.three_ds,
            no_three_ds: payment_method.no_three_ds,
            supported_card_networks: payment_method.supported_card_networks.unwrap_or_default(),
            supported_countries: payment_method
                .supported_countries
                .unwrap_or_default()
                .into_iter()
                .map(i32::from)
                .collect(),
            supported_currencies: payment_method
                .supported_currencies
                .unwrap_or_default()
                .into_iter()
                .map(i32::from)
                .collect(),
        }
    }
}
