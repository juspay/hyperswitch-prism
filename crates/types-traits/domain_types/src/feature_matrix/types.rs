use super::feature_matrix_types::{
    FeatureMatrixConnector, FeatureMatrixError, FeatureMatrixPaymentMethod, FeatureMatrixResponse,
};
use crate::{
    connector_types::ConnectorEnum,
    types::{FeatureStatus, IntegrationStatus},
    utils::{ForeignFrom, ForeignTryFrom},
};
use common_enums::{EventClass, PaymentMethodType as DomainPaymentMethodType};
use grpc_api_types::payments::{
    feature_matrix_connector::IntegrationStatus as GrpcIntegrationStatus,
    CardNetwork as GrpcCardNetwork, Connector as GrpcConnector, CountryAlpha2 as GrpcCountryAlpha2,
    Currency as GrpcCurrency, EventClass as GrpcEventClass, FeatureStatus as GrpcFeatureStatus,
    PaymentMethodType as GrpcPaymentMethodType,
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

impl ForeignFrom<DomainPaymentMethodType> for GrpcPaymentMethodType {
    fn foreign_from(payment_method_type: DomainPaymentMethodType) -> Self {
        match payment_method_type {
            DomainPaymentMethodType::Ach => Self::Ach,
            DomainPaymentMethodType::Affirm => Self::Affirm,
            DomainPaymentMethodType::AfterpayClearpay => Self::AfterpayClearpay,
            DomainPaymentMethodType::Alfamart => Self::Alfamart,
            DomainPaymentMethodType::AliPay => Self::AliPay,
            DomainPaymentMethodType::AliPayHk => Self::AliPayHk,
            DomainPaymentMethodType::Alma => Self::Alma,
            DomainPaymentMethodType::Tamara => Self::TamaraPayLater,
            DomainPaymentMethodType::AmazonPay => Self::AmazonPay,
            DomainPaymentMethodType::ApplePay => Self::ApplePay,
            DomainPaymentMethodType::Atome => Self::Atome,
            DomainPaymentMethodType::Bluecode => Self::Bluecode,
            DomainPaymentMethodType::Bacs => Self::Bacs,
            DomainPaymentMethodType::BancontactCard => Self::BancontactCard,
            DomainPaymentMethodType::Becs => Self::Becs,
            DomainPaymentMethodType::Benefit => Self::Benefit,
            DomainPaymentMethodType::Bizum => Self::Bizum,
            DomainPaymentMethodType::BillDesk => Self::BillDesk,
            DomainPaymentMethodType::Blik => Self::Blik,
            DomainPaymentMethodType::Boleto => Self::Boleto,
            DomainPaymentMethodType::BcaBankTransfer => Self::BcaBankTransfer,
            DomainPaymentMethodType::BniVa => Self::BniVa,
            DomainPaymentMethodType::BriVa => Self::BriVa,
            DomainPaymentMethodType::CardRedirect => Self::CardRedirect,
            DomainPaymentMethodType::CimbVa => Self::CimbVa,
            DomainPaymentMethodType::ClassicReward => Self::ClassicReward,
            DomainPaymentMethodType::Card => Self::Card,
            DomainPaymentMethodType::CryptoCurrency => Self::CryptoCurrency,
            DomainPaymentMethodType::Cashapp => Self::Cashapp,
            DomainPaymentMethodType::Cashfree => Self::CashFree,
            DomainPaymentMethodType::Dana => Self::Dana,
            DomainPaymentMethodType::DanamonVa => Self::DanamonVa,
            DomainPaymentMethodType::DuitNow => Self::DuitNow,
            DomainPaymentMethodType::Efecty => Self::Efecty,
            DomainPaymentMethodType::EaseBuzz => Self::EaseBuzz,
            DomainPaymentMethodType::Eft => Self::Eft,
            DomainPaymentMethodType::Eps => Self::Eps,
            DomainPaymentMethodType::Fps => Self::Fps,
            DomainPaymentMethodType::Evoucher => Self::Evoucher,
            DomainPaymentMethodType::Giropay => Self::Giropay,
            DomainPaymentMethodType::Givex => Self::Givex,
            DomainPaymentMethodType::GooglePay => Self::GooglePay,
            DomainPaymentMethodType::GoPay => Self::GoPay,
            DomainPaymentMethodType::Gcash => Self::Gcash,
            DomainPaymentMethodType::Ideal => Self::Ideal,
            DomainPaymentMethodType::Interac => Self::Interac,
            DomainPaymentMethodType::Indomaret => Self::Indomaret,
            DomainPaymentMethodType::Klarna => Self::KlarnaPayLater,
            DomainPaymentMethodType::KakaoPay => Self::KakaoPay,
            DomainPaymentMethodType::LocalBankRedirect => Self::LocalBankRedirect,
            DomainPaymentMethodType::MandiriVa => Self::MandiriVa,
            DomainPaymentMethodType::Knet => Self::Knet,
            DomainPaymentMethodType::LazyPay => Self::LazyPay,
            DomainPaymentMethodType::MbWay => Self::MbWay,
            DomainPaymentMethodType::MobilePay => Self::MobilePay,
            DomainPaymentMethodType::Momo => Self::Momo,
            DomainPaymentMethodType::MomoAtm => Self::MomoAtm,
            DomainPaymentMethodType::Multibanco => Self::Multibanco,
            DomainPaymentMethodType::NetworkToken => Self::NetworkToken,
            DomainPaymentMethodType::OnlineBankingThailand => Self::OnlineBankingThailand,
            DomainPaymentMethodType::OnlineBankingCzechRepublic => Self::OnlineBankingCzechRepublic,
            DomainPaymentMethodType::OnlineBankingFinland => Self::OnlineBankingFinland,
            DomainPaymentMethodType::OnlineBankingFpx => Self::OnlineBankingFpx,
            DomainPaymentMethodType::OnlineBankingPoland => Self::OnlineBankingPoland,
            DomainPaymentMethodType::OnlineBankingSlovakia => Self::OnlineBankingSlovakia,
            DomainPaymentMethodType::Oxxo => Self::Oxxo,
            DomainPaymentMethodType::PagoEfectivo => Self::PagoEfectivo,
            DomainPaymentMethodType::PermataBankTransfer => Self::PermataBankTransfer,
            DomainPaymentMethodType::OpenBankingUk => Self::OpenBankingUk,
            DomainPaymentMethodType::OpenBanking => Self::OpenBanking,
            DomainPaymentMethodType::PayBright => Self::PayBright,
            DomainPaymentMethodType::Paypal => Self::PayPal,
            DomainPaymentMethodType::PayU => Self::PayU,
            DomainPaymentMethodType::Paze => Self::Paze,
            DomainPaymentMethodType::PhonePe => Self::PhonePe,
            DomainPaymentMethodType::Pix => Self::Pix,
            DomainPaymentMethodType::PaySafeCard => Self::PaySafeCard,
            DomainPaymentMethodType::Przelewy24 => Self::Przelewy24,
            DomainPaymentMethodType::PromptPay => Self::PromptPay,
            DomainPaymentMethodType::Pse => Self::Pse,
            DomainPaymentMethodType::RedCompra => Self::RedCompra,
            DomainPaymentMethodType::RedPagos => Self::RedPagos,
            DomainPaymentMethodType::SamsungPay => Self::SamsungPay,
            DomainPaymentMethodType::Satispay => Self::Satispay,
            DomainPaymentMethodType::Sepa => Self::Sepa,
            DomainPaymentMethodType::SepaBankTransfer => Self::SepaBankTransfer,
            DomainPaymentMethodType::Sofort => Self::Sofort,
            DomainPaymentMethodType::Swish => Self::Swish,
            DomainPaymentMethodType::TouchNGo => Self::TouchNGo,
            DomainPaymentMethodType::Trustly => Self::TrustlyBankRedirect,
            DomainPaymentMethodType::Twint => Self::Twint,
            DomainPaymentMethodType::UpiCollect => Self::UpiCollect,
            DomainPaymentMethodType::UpiIntent => Self::UpiIntent,
            DomainPaymentMethodType::UpiQr => Self::UpiQr,
            DomainPaymentMethodType::Vipps => Self::Vipps,
            DomainPaymentMethodType::VietQr => Self::VietQr,
            DomainPaymentMethodType::Venmo => Self::Venmo,
            DomainPaymentMethodType::Walley => Self::Walley,
            DomainPaymentMethodType::WeChatPay => Self::WeChatPay,
            DomainPaymentMethodType::Wero => Self::Wero,
            DomainPaymentMethodType::Netbanking => Self::Netbanking,
            DomainPaymentMethodType::SevenEleven => Self::SevenEleven,
            DomainPaymentMethodType::Lawson => Self::Lawson,
            DomainPaymentMethodType::MiniStop => Self::MiniStop,
            DomainPaymentMethodType::FamilyMart => Self::FamilyMart,
            DomainPaymentMethodType::Seicomart => Self::Seicomart,
            DomainPaymentMethodType::PayEasy => Self::PayEasy,
            DomainPaymentMethodType::LocalBankTransfer => Self::LocalBankTransfer,
            DomainPaymentMethodType::Mifinity => Self::MifinityWallet,
            DomainPaymentMethodType::OpenBankingPIS => Self::OpenBankingPis,
            DomainPaymentMethodType::DirectCarrierBilling => Self::DirectCarrierBilling,
            DomainPaymentMethodType::InstantBankTransfer => Self::InstantBankTransfer,
            DomainPaymentMethodType::InstantBankTransferFinland => Self::InstantBankTransferFinland,
            DomainPaymentMethodType::InstantBankTransferPoland => Self::InstantBankTransferPoland,
            DomainPaymentMethodType::RevolutPay => Self::RevolutPay,
            DomainPaymentMethodType::SepaGuaranteedDebit => Self::SepaGuaranteedDebit,
            DomainPaymentMethodType::IndonesianBankTransfer => Self::IndonesianBankTransfer,
            DomainPaymentMethodType::Skrill => Self::Skrill,
            DomainPaymentMethodType::Paysera => Self::Paysera,
            DomainPaymentMethodType::QwikcilverWallet => Self::QwikcilverWallet,
        }
    }
}

impl From<FeatureMatrixPaymentMethod> for grpc_api_types::payments::FeatureMatrixPaymentMethod {
    fn from(payment_method: FeatureMatrixPaymentMethod) -> Self {
        Self {
            payment_method_type: GrpcPaymentMethodType::foreign_from(
                payment_method.payment_method_type,
            )
            .into(),
            payment_method_type_display_name: payment_method.payment_method_type_display_name,
            mandates: grpc_feature_status(payment_method.mandates).into(),
            refunds: grpc_feature_status(payment_method.refunds).into(),
            supported_capture_methods: payment_method
                .supported_capture_methods
                .into_iter()
                .map(grpc_api_types::payments::CaptureMethod::foreign_from)
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
                .map(GrpcCardNetwork::foreign_from)
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
