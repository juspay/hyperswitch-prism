use common_enums::{CountryAlpha2, Currency, EventClass, PaymentMethodType};
use grpc_api_types::payments::{CountryAlpha2 as GrpcCountryAlpha2, Currency as GrpcCurrency};
use serde::{Serialize, Serializer};

use crate::{
    connector_types::ConnectorEnum,
    types::{
        ConnectorInfo, PaymentMethodDetails, PaymentMethodSpecificFeatures, SupportedPaymentMethods,
    },
    utils::ForeignTryFrom,
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
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub base_url: String,
    pub category: String,
    pub integration_status: &'static str,
    pub supported_payment_methods: Vec<FeatureMatrixPaymentMethod>,
    pub supported_webhook_flows: Vec<String>,
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
            .map(|event_classes| {
                event_classes
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Self {
            name: connector_name.to_string().to_ascii_uppercase(),
            display_name: connector_info.display_name.to_string(),
            description: connector_info.description.to_string(),
            base_url: base_url.to_string(),
            category: connector_info.connector_type.to_string(),
            integration_status: "beta",
            supported_payment_methods,
            supported_webhook_flows,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureMatrixPaymentMethod {
    pub payment_method: String,
    pub payment_method_type: String,
    pub payment_method_type_display_name: String,
    pub mandates: String,
    pub refunds: String,
    pub supported_capture_methods: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_three_ds: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_card_networks: Option<Vec<String>>,
    #[serde(serialize_with = "serialize_supported_countries")]
    pub supported_countries: Option<Vec<GrpcCountryAlpha2>>,
    #[serde(serialize_with = "serialize_supported_currencies")]
    pub supported_currencies: Option<Vec<GrpcCurrency>>,
}

impl FeatureMatrixPaymentMethod {
    pub fn from_supported_payment_methods(
        supported_payment_methods: &SupportedPaymentMethods,
    ) -> Vec<Self> {
        let mut payment_methods = supported_payment_methods
            .iter()
            .flat_map(|(payment_method, payment_method_type_metadata)| {
                payment_method_type_metadata.iter().flat_map(
                    move |(payment_method_type, payment_method_details)| {
                        let payment_method_name = payment_method.to_string();

                        match payment_method_type {
                            PaymentMethodType::Card => vec![
                                Self::from_payment_method_details(
                                    payment_method_name.clone(),
                                    "credit".to_string(),
                                    "Credit Card".to_string(),
                                    payment_method_details,
                                ),
                                Self::from_payment_method_details(
                                    payment_method_name,
                                    "debit".to_string(),
                                    "Debit Card".to_string(),
                                    payment_method_details,
                                ),
                            ],
                            payment_method_type => vec![Self::from_payment_method_details(
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

    fn from_payment_method_details(
        payment_method: String,
        payment_method_type: String,
        payment_method_type_display_name: String,
        payment_method_details: &PaymentMethodDetails,
    ) -> Self {
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

        Self {
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
            supported_countries: supported_countries(&payment_method_details.supported_countries),
            supported_currencies: supported_currencies(
                &payment_method_details.supported_currencies,
            ),
        }
    }
}

fn supported_countries(countries: &[CountryAlpha2]) -> Option<Vec<GrpcCountryAlpha2>> {
    (!countries.is_empty()).then(|| {
        countries
            .iter()
            .map(|country| {
                GrpcCountryAlpha2::foreign_try_from(*country)
                    .unwrap_or(GrpcCountryAlpha2::Unspecified)
            })
            .collect()
    })
}

fn supported_currencies(currencies: &[Currency]) -> Option<Vec<GrpcCurrency>> {
    (!currencies.is_empty()).then(|| {
        currencies
            .iter()
            .map(|currency| {
                GrpcCurrency::foreign_try_from(*currency).unwrap_or(GrpcCurrency::Unspecified)
            })
            .collect()
    })
}

fn serialize_supported_countries<S>(
    countries: &Option<Vec<GrpcCountryAlpha2>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    countries
        .as_ref()
        .map(|countries| {
            countries
                .iter()
                .map(|country| country.as_str_name())
                .collect::<Vec<_>>()
        })
        .serialize(serializer)
}

fn serialize_supported_currencies<S>(
    currencies: &Option<Vec<GrpcCurrency>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    currencies
        .as_ref()
        .map(|currencies| {
            currencies
                .iter()
                .map(|currency| currency.as_str_name())
                .collect::<Vec<_>>()
        })
        .serialize(serializer)
}
