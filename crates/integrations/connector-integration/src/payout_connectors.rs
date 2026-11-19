/// Single-currency request shapes cannot reinterpret source money as target money.
pub(crate) fn require_matching_payout_currency(
    money: &common_utils::types::Money,
    destination_currency: common_enums::Currency,
    connector: &'static str,
) -> Result<(), error_stack::Report<domain_types::errors::IntegrationError>> {
    if money.currency() != destination_currency {
        return Err(domain_types::errors::IntegrationError::NotSupported {
            message: "Cross-currency payout amount mapping is not supported by this integration".to_owned(),
            connector,
            context: domain_types::errors::IntegrationErrorContext {
                additional_context: Some(format!(
                    "Source money is denominated in {:?}, but the requested destination currency is {:?}; unit conversion does not perform FX",
                    money.currency(), destination_currency,
                )),
                suggested_action: Some("Use matching currencies or an explicitly supported provider FX/quote flow".to_owned()),
                doc_url: None,
            },
        }.into());
    }
    Ok(())
}

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
pub mod checkout;
pub use checkout::CheckoutPayouts;
