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

/// Register payout webhook normalization here as providers migrate.
/// No providers are registered by this framework change.
pub fn get_webhook_connector<T>(
    _connector: domain_types::connector_types::PayoutConnectorEnum,
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
    None
}
pub mod checkout;
pub use checkout::CheckoutPayouts;
