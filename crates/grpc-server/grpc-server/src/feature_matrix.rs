use std::str::FromStr;

use common_enums::PaymentMethodType;
use connector_integration::types::ConnectorData;
use domain_types::{
    connector_types::ConnectorEnum,
    payment_method_data::DefaultPCIHolder,
    types::{PaymentMethodDetails, PaymentMethodSpecificFeatures, SupportedPaymentMethods},
};
use interfaces::connector_types::ConnectorServiceTrait;
use serde::Serialize;
use strum::IntoEnumIterator;
use ucs_env::configs::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureMatrixError {
    InvalidConnectorName(String),
    ConnectorNotConfigured(ConnectorEnum),
}

impl FeatureMatrixError {
    pub fn message(&self) -> String {
        match self {
            Self::InvalidConnectorName(connector_name) => {
                format!("Invalid connector name: {connector_name}")
            }
            Self::ConnectorNotConfigured(connector_name) => {
                format!("Feature matrix is not configured for connector: {connector_name}")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureMatrixResponse {
    connector_count: usize,
    connectors: Vec<FeatureMatrixConnector>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureMatrixConnector {
    name: String,
    display_name: String,
    description: String,
    base_url: String,
    category: String,
    integration_status: &'static str,
    supported_payment_methods: Vec<FeatureMatrixPaymentMethod>,
    supported_webhook_flows: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureMatrixPaymentMethod {
    payment_method: String,
    payment_method_type: String,
    payment_method_type_display_name: String,
    mandates: String,
    refunds: String,
    supported_capture_methods: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    three_ds: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    no_three_ds: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supported_card_networks: Option<Vec<String>>,
    supported_countries: Option<Vec<String>>,
    supported_currencies: Option<Vec<String>>,
}

pub fn build_feature_matrix(
    connector_names: Vec<String>,
    config: &Config,
) -> Result<FeatureMatrixResponse, FeatureMatrixError> {
    let connectors = if connector_names.is_empty() {
        ConnectorEnum::iter()
            .filter_map(|connector_name| {
                build_feature_matrix_connector(connector_name, config).ok()
            })
            .collect()
    } else {
        connector_names
            .into_iter()
            .map(|connector_name| {
                let connector = parse_connector_name(&connector_name)?;
                build_feature_matrix_connector(connector, config)
            })
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(FeatureMatrixResponse {
        connector_count: connectors.len(),
        connectors,
    })
}

fn parse_connector_name(connector_name: &str) -> Result<ConnectorEnum, FeatureMatrixError> {
    let normalized_connector_name = connector_name.trim().to_ascii_lowercase().replace('-', "_");

    ConnectorEnum::from_str(&normalized_connector_name)
        .map_err(|_| FeatureMatrixError::InvalidConnectorName(connector_name.to_string()))
}

fn build_feature_matrix_connector(
    connector_name: ConnectorEnum,
    config: &Config,
) -> Result<FeatureMatrixConnector, FeatureMatrixError> {
    let connector_data = ConnectorData::<DefaultPCIHolder>::get_connector_by_name(&connector_name);
    let connector = *connector_data.connector;

    build_connector_response(
        connector_name,
        connector,
        connector.base_url(&config.connectors),
    )
}

fn build_connector_response(
    connector_name: ConnectorEnum,
    connector: &(dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync),
    base_url: &str,
) -> Result<FeatureMatrixConnector, FeatureMatrixError> {
    let connector_info = connector
        .get_connector_about()
        .ok_or(FeatureMatrixError::ConnectorNotConfigured(connector_name))?;

    let supported_payment_methods = connector
        .get_supported_payment_methods()
        .map(build_supported_payment_methods)
        .unwrap_or_default();

    let supported_webhook_flows = connector
        .get_supported_webhook_flows()
        .map(|event_classes| {
            event_classes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(FeatureMatrixConnector {
        name: connector_name.to_string().to_ascii_uppercase(),
        display_name: connector_info.display_name.to_string(),
        description: connector_info.description.to_string(),
        base_url: base_url.to_string(),
        category: connector_info.connector_type.to_string(),
        integration_status: "beta",
        supported_payment_methods,
        supported_webhook_flows,
    })
}

fn build_supported_payment_methods(
    supported_payment_methods: &SupportedPaymentMethods,
) -> Vec<FeatureMatrixPaymentMethod> {
    let mut payment_methods = supported_payment_methods
        .iter()
        .flat_map(|(payment_method, payment_method_type_metadata)| {
            payment_method_type_metadata.iter().flat_map(
                move |(payment_method_type, payment_method_details)| {
                    let payment_method_name = payment_method.to_string();

                    match payment_method_type {
                        PaymentMethodType::Card => vec![
                            build_payment_method_response(
                                payment_method_name.clone(),
                                "credit".to_string(),
                                "Credit Card".to_string(),
                                payment_method_details,
                            ),
                            build_payment_method_response(
                                payment_method_name,
                                "debit".to_string(),
                                "Debit Card".to_string(),
                                payment_method_details,
                            ),
                        ],
                        payment_method_type => vec![build_payment_method_response(
                            payment_method_name,
                            payment_method_type.to_string(),
                            payment_method_type.to_display_name(),
                            payment_method_details,
                        )],
                    }
                },
            )
        })
        .collect::<Vec<_>>();

    payment_methods.sort_by(|left, right| {
        left.payment_method
            .cmp(&right.payment_method)
            .then_with(|| left.payment_method_type.cmp(&right.payment_method_type))
    });

    payment_methods
}

fn build_payment_method_response(
    payment_method: String,
    payment_method_type: String,
    payment_method_type_display_name: String,
    payment_method_details: &PaymentMethodDetails,
) -> FeatureMatrixPaymentMethod {
    let (three_ds, no_three_ds, supported_card_networks) =
        match &payment_method_details.specific_features {
            Some(PaymentMethodSpecificFeatures::Card(card_features)) => (
                Some(card_features.three_ds.to_string()),
                Some(card_features.no_three_ds.to_string()),
                Some(
                    card_features
                        .supported_card_networks
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                ),
            ),
            None => (None, None, None),
        };

    FeatureMatrixPaymentMethod {
        payment_method,
        payment_method_type,
        payment_method_type_display_name,
        mandates: payment_method_details.mandates.to_string(),
        refunds: payment_method_details.refunds.to_string(),
        supported_capture_methods: payment_method_details
            .supported_capture_methods
            .iter()
            .map(ToString::to_string)
            .collect(),
        three_ds,
        no_three_ds,
        supported_card_networks,
        supported_countries: non_empty_strings(&payment_method_details.supported_countries),
        supported_currencies: non_empty_strings(&payment_method_details.supported_currencies),
    }
}

fn non_empty_strings<T: ToString>(items: &[T]) -> Option<Vec<String>> {
    (!items.is_empty()).then(|| items.iter().map(ToString::to_string).collect())
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
            name: connector.name,
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
            supported_countries: payment_method.supported_countries.unwrap_or_default(),
            supported_currencies: payment_method.supported_currencies.unwrap_or_default(),
        }
    }
}
