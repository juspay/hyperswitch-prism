pub mod itaubank;
pub use self::itaubank::ItaubankPayouts;

pub mod nuvei;
pub use self::nuvei::NuveiPayouts;

pub mod loonio;
pub use self::loonio::LoonioPayouts;

pub mod paypal;
pub use self::paypal::PaypalPayouts;

pub mod stripe;
pub use self::stripe::StripePayouts;

pub mod deutschebank;
pub use self::deutschebank::DeutschebankPayouts;

pub mod truelayer;
pub use self::truelayer::TruelayerPayouts;

pub mod worldpayxml;
pub use self::worldpayxml::WorldpayxmlPayouts;

pub mod cybersource;
pub use self::cybersource::CybersourcePayouts;

pub mod gigadat;
pub use self::gigadat::GigadatPayouts;

pub mod santander;
pub use self::santander::SantanderPayouts;

pub mod trustly;
pub use self::trustly::TrustlyPayouts;

pub mod gotyme_sanlam;
pub use self::gotyme_sanlam::GotymeSanlamPayouts;

pub mod mifinity;
pub use self::mifinity::MifinityPayouts;

/// Providers whose payout webhook normalization has moved to payout connectors.
pub fn get_webhook_connector<T>(
    connector: domain_types::connector_types::ConnectorEnum,
) -> Option<crate::types::PayoutConnectorData<T>>
where
    T: domain_types::payment_method_data::PaymentMethodDataTypes
        + std::fmt::Debug
        + Default
        + Send
        + Sync
        + serde::Serialize
        + 'static,
{
    use domain_types::connector_types::{ConnectorEnum, PayoutConnectorEnum};
    let payout = match connector {
        ConnectorEnum::Truelayer => PayoutConnectorEnum::Truelayer,
        ConnectorEnum::Trustly => PayoutConnectorEnum::Trustly,
        _ => return None,
    };
    Some(crate::types::PayoutConnectorData::get_connector_by_name(
        &payout,
    ))
}
