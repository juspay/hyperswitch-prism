use common_enums::{
    CaptureMethod, CardNetwork, CountryAlpha2, Currency, EventClass, PaymentMethodType,
};
use serde::{Serialize, Serializer};

use crate::{
    connector_types::ConnectorEnum,
    types::{
        ConnectorInfo, FeatureStatus, IntegrationStatus, PaymentMethodDetails,
        PaymentMethodSpecificFeatures, SupportedPaymentMethods,
    },
};

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
    pub connector_count: usize,
    pub connectors: Vec<FeatureMatrixConnector>,
}

impl FeatureMatrixResponse {
    pub fn new(connectors: Vec<FeatureMatrixConnector>) -> Self {
        Self {
            connector_count: connectors.len(),
            connectors,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureMatrixConnector {
    #[serde(serialize_with = "serialize_connector_name")]
    pub name: ConnectorEnum,
    pub display_name: String,
    pub description: String,
    pub base_url: String,
    pub category: String,
    pub integration_status: IntegrationStatus,
    pub supported_payment_methods: Vec<FeatureMatrixPaymentMethod>,
    pub supported_webhook_flows: Vec<EventClass>,
}

impl FeatureMatrixConnector {
    pub fn from_connector_details(
        connector_name: ConnectorEnum,
        connector_info: &ConnectorInfo,
        base_url: &str,
        supported_payment_methods: Option<&'static SupportedPaymentMethods>,
        supported_webhook_flows: Option<&'static [EventClass]>,
    ) -> Self {
        let supported_payment_methods = supported_payment_methods
            .map(FeatureMatrixPaymentMethod::from_supported_payment_methods)
            .unwrap_or_default();

        let supported_webhook_flows = supported_webhook_flows
            .map(|event_classes| event_classes.to_vec())
            .unwrap_or_default();

        Self {
            name: connector_name,
            display_name: connector_info.display_name.to_string(),
            description: connector_info.description.to_string(),
            base_url: base_url.to_string(),
            category: connector_info.connector_type.to_string(),
            integration_status: connector_info.integration_status,
            supported_payment_methods,
            supported_webhook_flows,
        }
    }
}

fn serialize_connector_name<S>(
    connector_name: &ConnectorEnum,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    connector_name
        .to_string()
        .to_ascii_uppercase()
        .serialize(serializer)
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureMatrixPaymentMethod {
    pub payment_method_type: String,
    pub payment_method_type_display_name: String,
    pub mandates: FeatureStatus,
    pub refunds: FeatureStatus,
    pub supported_capture_methods: Vec<CaptureMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<FeatureStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_three_ds: Option<FeatureStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_card_networks: Option<Vec<CardNetwork>>,
    pub supported_countries: Option<Vec<CountryAlpha2>>,
    pub supported_currencies: Option<Vec<Currency>>,
}

impl FeatureMatrixPaymentMethod {
    pub fn from_supported_payment_methods(
        supported_payment_methods: &SupportedPaymentMethods,
    ) -> Vec<Self> {
        let mut payment_methods = supported_payment_methods
            .values()
            .flat_map(|payment_method_type_metadata| {
                payment_method_type_metadata.iter().flat_map(
                    |(payment_method_type, payment_method_details)| match payment_method_type {
                        PaymentMethodType::Card => vec![
                            Self::from_payment_method_details(
                                "credit".to_string(),
                                "Credit Card".to_string(),
                                payment_method_details,
                            ),
                            Self::from_payment_method_details(
                                "debit".to_string(),
                                "Debit Card".to_string(),
                                payment_method_details,
                            ),
                        ],
                        payment_method_type => vec![Self::from_payment_method_details(
                            payment_method_type.to_string(),
                            payment_method_type.to_display_name(),
                            payment_method_details,
                        )],
                    },
                )
            })
            .collect::<Vec<_>>();

        payment_methods
            .sort_by(|left, right| left.payment_method_type.cmp(&right.payment_method_type));

        payment_methods
    }

    fn from_payment_method_details(
        payment_method_type: String,
        payment_method_type_display_name: String,
        payment_method_details: &PaymentMethodDetails,
    ) -> Self {
        let (three_ds, no_three_ds, supported_card_networks) =
            match &payment_method_details.specific_features {
                Some(PaymentMethodSpecificFeatures::Card(card_features)) => (
                    Some(card_features.three_ds),
                    Some(card_features.no_three_ds),
                    Some(card_features.supported_card_networks.clone()),
                ),
                None => (None, None, None),
            };

        Self {
            payment_method_type,
            payment_method_type_display_name,
            mandates: payment_method_details.mandates,
            refunds: payment_method_details.refunds,
            supported_capture_methods: payment_method_details.supported_capture_methods.clone(),
            three_ds,
            no_three_ds,
            supported_card_networks,
            supported_countries: (!payment_method_details.supported_countries.is_empty())
                .then(|| payment_method_details.supported_countries.clone()),
            supported_currencies: (!payment_method_details.supported_currencies.is_empty())
                .then(|| payment_method_details.supported_currencies.clone()),
        }
    }
}
