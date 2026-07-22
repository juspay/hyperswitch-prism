use super::feature_matrix_types::{
    FeatureMatrixConnector, FeatureMatrixError, FeatureMatrixPaymentMethod, FeatureMatrixResponse,
};
use crate::{
    connector_types::ConnectorEnum,
    types::{FeatureStatus, IntegrationStatus},
    utils::ForeignTryFrom,
};
use common_enums::{CaptureMethod, CardNetwork, EventClass};
use grpc_api_types::payments::{
    feature_matrix_connector::IntegrationStatus as GrpcIntegrationStatus,
    CaptureMethod as GrpcCaptureMethod, CardNetwork as GrpcCardNetwork, Connector as GrpcConnector,
    CountryAlpha2 as GrpcCountryAlpha2, Currency as GrpcCurrency, EventClass as GrpcEventClass,
    FeatureStatus as GrpcFeatureStatus,
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
            connector_name: grpc_connector(connector.name).into(),
            display_name: connector.display_name,
            description: connector.description,
            base_url: connector.base_url,
            category: connector.category,
            integration_status: grpc_integration_status(connector.integration_status).into(),
            supported_payment_methods: connector
                .supported_payment_methods
                .into_iter()
                .map(Into::into)
                .collect(),
            supported_webhook_flows: connector
                .supported_webhook_flows
                .into_iter()
                .map(grpc_event_class)
                .map(Into::into)
                .collect(),
        }
    }
}

fn grpc_connector(connector: ConnectorEnum) -> GrpcConnector {
    GrpcConnector::from_str_name(&connector.to_string().to_ascii_uppercase())
        .unwrap_or(GrpcConnector::Unspecified)
}

fn grpc_event_class(event_class: EventClass) -> GrpcEventClass {
    match event_class {
        EventClass::Payments => GrpcEventClass::Payments,
        EventClass::Refunds => GrpcEventClass::Refunds,
        EventClass::Disputes => GrpcEventClass::Disputes,
    }
}

fn grpc_integration_status(integration_status: IntegrationStatus) -> GrpcIntegrationStatus {
    match integration_status {
        IntegrationStatus::Live => GrpcIntegrationStatus::Live,
        IntegrationStatus::Sandbox => GrpcIntegrationStatus::Sandbox,
        IntegrationStatus::Beta => GrpcIntegrationStatus::Beta,
        IntegrationStatus::Alpha => GrpcIntegrationStatus::Alpha,
    }
}

fn grpc_feature_status(feature_status: FeatureStatus) -> GrpcFeatureStatus {
    match feature_status {
        FeatureStatus::NotSupported => GrpcFeatureStatus::NotSupported,
        FeatureStatus::Supported => GrpcFeatureStatus::Supported,
    }
}

fn grpc_capture_method(capture_method: CaptureMethod) -> GrpcCaptureMethod {
    match capture_method {
        CaptureMethod::Automatic => GrpcCaptureMethod::Automatic,
        CaptureMethod::Manual => GrpcCaptureMethod::Manual,
        CaptureMethod::ManualMultiple => GrpcCaptureMethod::ManualMultiple,
        CaptureMethod::Scheduled => GrpcCaptureMethod::Scheduled,
        CaptureMethod::SequentialAutomatic => GrpcCaptureMethod::SequentialAutomatic,
    }
}

fn grpc_card_network(card_network: CardNetwork) -> GrpcCardNetwork {
    match card_network {
        CardNetwork::Visa => GrpcCardNetwork::Visa,
        CardNetwork::Mastercard => GrpcCardNetwork::Mastercard,
        CardNetwork::AmericanExpress => GrpcCardNetwork::Amex,
        CardNetwork::JCB => GrpcCardNetwork::Jcb,
        CardNetwork::DinersClub => GrpcCardNetwork::Diners,
        CardNetwork::Discover => GrpcCardNetwork::Discover,
        CardNetwork::CartesBancaires => GrpcCardNetwork::CartesBancaires,
        CardNetwork::UnionPay => GrpcCardNetwork::Unionpay,
        CardNetwork::Interac => GrpcCardNetwork::InteracCard,
        CardNetwork::RuPay => GrpcCardNetwork::Rupay,
        CardNetwork::Maestro => GrpcCardNetwork::Maestro,
        CardNetwork::Star => GrpcCardNetwork::Star,
        CardNetwork::Pulse => GrpcCardNetwork::Pulse,
        CardNetwork::Accel => GrpcCardNetwork::Accel,
        CardNetwork::Nyce => GrpcCardNetwork::Nyce,
    }
}

impl From<FeatureMatrixPaymentMethod> for grpc_api_types::payments::FeatureMatrixPaymentMethod {
    fn from(payment_method: FeatureMatrixPaymentMethod) -> Self {
        Self {
            payment_method_type: payment_method.payment_method_type,
            payment_method_type_display_name: payment_method.payment_method_type_display_name,
            mandates: grpc_feature_status(payment_method.mandates).into(),
            refunds: grpc_feature_status(payment_method.refunds).into(),
            supported_capture_methods: payment_method
                .supported_capture_methods
                .into_iter()
                .map(grpc_capture_method)
                .map(Into::into)
                .collect(),
            three_ds: payment_method
                .three_ds
                .map(grpc_feature_status)
                .map(Into::into),
            no_three_ds: payment_method
                .no_three_ds
                .map(grpc_feature_status)
                .map(Into::into),
            supported_card_networks: payment_method
                .supported_card_networks
                .unwrap_or_default()
                .into_iter()
                .map(grpc_card_network)
                .map(Into::into)
                .collect(),
            supported_countries: payment_method
                .supported_countries
                .unwrap_or_default()
                .into_iter()
                .map(|country| {
                    GrpcCountryAlpha2::foreign_try_from(country)
                        .unwrap_or(GrpcCountryAlpha2::Unspecified)
                        .into()
                })
                .collect(),
            supported_currencies: payment_method
                .supported_currencies
                .unwrap_or_default()
                .into_iter()
                .map(|currency| {
                    GrpcCurrency::foreign_try_from(currency)
                        .unwrap_or(GrpcCurrency::Unspecified)
                        .into()
                })
                .collect(),
        }
    }
}
