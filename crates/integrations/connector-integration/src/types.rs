use std::fmt::Debug;

use domain_types::{
    connector_types::{ConnectorEnum, PayoutConnectorEnum, SurchargeConnectorEnum},
    payment_method_data::PaymentMethodDataTypes,
};
use interfaces::connector_types::{BoxedConnector, BoxedPayoutConnector, BoxedSurchargeConnector};

use crate::{connectors, payout_connectors, surcharge_connectors};

#[derive(Clone)]
pub struct ConnectorData<T: PaymentMethodDataTypes + Debug + Default + Send + Sync + 'static> {
    pub connector: BoxedConnector<T>,
    pub connector_name: ConnectorEnum,
}

#[derive(Clone)]
pub struct SurchargeConnectorData {
    pub connector: BoxedSurchargeConnector,
    pub connector_name: SurchargeConnectorEnum,
}

impl<T: PaymentMethodDataTypes + Debug + Default + Send + Sync + 'static + serde::Serialize>
    ConnectorData<T>
{
    pub fn get_connector_by_name(connector_name: &ConnectorEnum) -> Self {
        let connector = Self::convert_connector(*connector_name);
        Self {
            connector,
            connector_name: *connector_name,
        }
    }

    fn convert_connector(connector_name: ConnectorEnum) -> BoxedConnector<T> {
        match connector_name {
            ConnectorEnum::Forte => Box::new(connectors::Forte::new()),
            ConnectorEnum::Adyen => Box::new(connectors::Adyen::new()),
            ConnectorEnum::Bluesnap => Box::new(connectors::Bluesnap::new()),
            ConnectorEnum::Razorpay => Box::new(connectors::Razorpay::new()),
            ConnectorEnum::RazorpayV2 => Box::new(connectors::RazorpayV2::new()),
            ConnectorEnum::Fiserv => Box::new(connectors::Fiserv::new()),
            ConnectorEnum::Elavon => Box::new(connectors::Elavon::new()),
            ConnectorEnum::Xendit => Box::new(connectors::Xendit::new()),
            ConnectorEnum::Checkout => Box::new(connectors::Checkout::new()),
            ConnectorEnum::Authorizedotnet => Box::new(connectors::Authorizedotnet::new()),
            ConnectorEnum::Mifinity => Box::new(connectors::Mifinity::new()),
            ConnectorEnum::Phonepe => Box::new(connectors::Phonepe::new()),
            ConnectorEnum::Cashfree => Box::new(connectors::Cashfree::new()),
            ConnectorEnum::Fiuu => Box::new(connectors::Fiuu::new()),
            ConnectorEnum::Payu => Box::new(connectors::Payu::new()),
            ConnectorEnum::Paytm => Box::new(connectors::Paytm::new()),
            ConnectorEnum::Cashtocode => Box::new(connectors::Cashtocode::new()),
            ConnectorEnum::Novalnet => Box::new(connectors::Novalnet::new()),
            ConnectorEnum::Netcetera => Box::new(connectors::Netcetera::new()),
            ConnectorEnum::Nexinets => Box::new(connectors::Nexinets::new()),
            ConnectorEnum::Noon => Box::new(connectors::Noon::new()),
            ConnectorEnum::Volt => Box::new(connectors::Volt::new()),
            ConnectorEnum::Braintree => Box::new(connectors::Braintree::new()),
            ConnectorEnum::Calida => Box::new(connectors::Calida::new()),
            ConnectorEnum::Cryptopay => Box::new(connectors::Cryptopay::new()),
            ConnectorEnum::Helcim => Box::new(connectors::Helcim::new()),
            ConnectorEnum::Multisafepay => Box::new(connectors::Multisafepay::new()),
            ConnectorEnum::Iatapay => Box::new(connectors::Iatapay::new()),
            ConnectorEnum::Nmi => Box::new(connectors::Nmi::new()),
            ConnectorEnum::Nexixpay => Box::new(connectors::Nexixpay::new()),
            ConnectorEnum::Authipay => Box::new(connectors::Authipay::new()),
            ConnectorEnum::Stax => Box::new(connectors::Stax::new()),
            ConnectorEnum::Fiservemea => Box::new(connectors::Fiservemea::new()),
            ConnectorEnum::Datatrans => Box::new(connectors::Datatrans::new()),
            ConnectorEnum::Silverflow => Box::new(connectors::Silverflow::new()),
            ConnectorEnum::Celero => Box::new(connectors::Celero::new()),
            ConnectorEnum::Globalpay => Box::new(connectors::Globalpay::new()),
            ConnectorEnum::Dlocal => Box::new(connectors::Dlocal::new()),
            ConnectorEnum::Hipay => Box::new(connectors::Hipay::new()),
            ConnectorEnum::Placetopay => Box::new(connectors::Placetopay::new()),
            ConnectorEnum::Trustpayments => Box::new(connectors::Trustpayments::new()),
            ConnectorEnum::Rapyd => Box::new(connectors::Rapyd::new()),
            ConnectorEnum::Redsys => Box::new(connectors::Redsys::new()),
            ConnectorEnum::Aci => Box::new(connectors::Aci::new()),
            ConnectorEnum::Trustpay => Box::new(connectors::Trustpay::new()),
            ConnectorEnum::Stripe => Box::new(connectors::Stripe::new()),
            ConnectorEnum::Cybersource => Box::new(connectors::Cybersource::new()),
            ConnectorEnum::Worldpay => Box::new(connectors::Worldpay::new()),
            ConnectorEnum::Worldpayvantiv => Box::new(connectors::Worldpayvantiv::new()),
            ConnectorEnum::Worldpayxml => Box::new(connectors::Worldpayxml::new()),
            ConnectorEnum::Payload => Box::new(connectors::Payload::new()),
            ConnectorEnum::Paysafe => Box::new(connectors::Paysafe::new()),
            ConnectorEnum::Paypal => Box::new(connectors::Paypal::new()),
            ConnectorEnum::Peachpayments => Box::new(connectors::Peachpayments::new()),
            ConnectorEnum::Finix => Box::new(connectors::Finix::new()),
            ConnectorEnum::Fiservcommercehub => Box::new(connectors::Fiservcommercehub::new()),
            ConnectorEnum::Revolv3 => Box::new(connectors::Revolv3::new()),
            ConnectorEnum::Mollie => Box::new(connectors::Mollie::new()),
            ConnectorEnum::Gigadat => Box::new(connectors::Gigadat::new()),
            ConnectorEnum::Paybox => Box::new(connectors::Paybox::new()),
            ConnectorEnum::Loonio => Box::new(connectors::Loonio::new()),
            ConnectorEnum::Barclaycard => Box::new(connectors::Barclaycard::new()),
            ConnectorEnum::Billwerk => Box::new(connectors::Billwerk::new()),
            ConnectorEnum::Payme => Box::new(connectors::Payme::new()),
            ConnectorEnum::Nuvei => Box::new(connectors::Nuvei::new()),
            ConnectorEnum::Airwallex => Box::new(connectors::Airwallex::new()),
            ConnectorEnum::Bambora => Box::new(connectors::Bambora::new()),
            ConnectorEnum::Shift4 => Box::new(connectors::Shift4::new()),
            ConnectorEnum::AbsaSanlam => Box::new(connectors::AbsaSanlam::new()),
            ConnectorEnum::Bamboraapac => Box::new(connectors::Bamboraapac::new()),
            ConnectorEnum::Tsys => Box::new(connectors::Tsys::new()),
            ConnectorEnum::Bankofamerica => Box::new(connectors::Bankofamerica::new()),
            ConnectorEnum::Powertranz => Box::new(connectors::Powertranz::new()),
            ConnectorEnum::Getnet => Box::new(connectors::Getnet::new()),
            ConnectorEnum::Jpmorgan => Box::new(connectors::Jpmorgan::new()),
            ConnectorEnum::Revolut => Box::new(connectors::Revolut::new()),
            ConnectorEnum::Wellsfargo => Box::new(connectors::Wellsfargo::new()),
            ConnectorEnum::Hyperpg => Box::new(connectors::Hyperpg::new()),
            ConnectorEnum::Zift => Box::new(connectors::Zift::new()),
            ConnectorEnum::Ppro => Box::new(connectors::Ppro::new()),
            ConnectorEnum::Truelayer => Box::new(connectors::Truelayer::new()),
            ConnectorEnum::Trustly => Box::new(connectors::Trustly::new()),
            ConnectorEnum::Itaubank => Box::new(connectors::Itaubank::new()),
            ConnectorEnum::PinelabsOnline => Box::new(connectors::PinelabsOnline::new()),
            ConnectorEnum::Easebuzz => Box::new(connectors::Easebuzz::new()),
            ConnectorEnum::Imerchantsolutions => Box::new(connectors::Imerchantsolutions::new()),
            ConnectorEnum::Axisbank => Box::new(connectors::Axisbank::new()),
            ConnectorEnum::TsysTransit => Box::new(connectors::TsysTransit::new()),
            ConnectorEnum::TwocTwopPaco => Box::new(connectors::TwocTwopPaco::new()),
            ConnectorEnum::Juspay => Box::new(connectors::Juspay::<T>::new()),
            ConnectorEnum::Payconex => Box::new(connectors::Payconex::<T>::new()),
            ConnectorEnum::Tamara => Box::new(connectors::Tamara::<T>::new()),
            ConnectorEnum::Hyperswitch => Box::new(connectors::Hyperswitch::<T>::new()),
            ConnectorEnum::Qwikcilver => Box::new(connectors::Qwikcilver::<T>::new()),
        }
    }
}

impl SurchargeConnectorData {
    pub fn get_connector_by_name(connector_name: &SurchargeConnectorEnum) -> Self {
        let connector = Self::convert_connector(*connector_name);
        Self {
            connector,
            connector_name: *connector_name,
        }
    }

    fn convert_connector(connector_name: SurchargeConnectorEnum) -> BoxedSurchargeConnector {
        match connector_name {
            SurchargeConnectorEnum::Interpayments => {
                Box::new(surcharge_connectors::InterPayments::new())
            }
        }
    }
}

#[derive(Clone)]
pub struct PayoutConnectorData {
    pub connector: BoxedPayoutConnector,
    pub connector_name: PayoutConnectorEnum,
}

impl PayoutConnectorData {
    pub fn get_connector_by_name(connector_name: &PayoutConnectorEnum) -> Self {
        let connector = Self::convert_connector(*connector_name);
        Self {
            connector,
            connector_name: *connector_name,
        }
    }

    fn convert_connector(connector_name: PayoutConnectorEnum) -> BoxedPayoutConnector {
        match connector_name {
            PayoutConnectorEnum::Loonio => Box::new(payout_connectors::LoonioPayouts::new()),
            PayoutConnectorEnum::Paypal => Box::new(payout_connectors::PaypalPayouts::new()),
            PayoutConnectorEnum::Itaubank => Box::new(payout_connectors::ItaubankPayouts::new()),
            PayoutConnectorEnum::Worldpayxml => {
                Box::new(payout_connectors::WorldpayxmlPayouts::new())
            }
            PayoutConnectorEnum::Cybersource => {
                Box::new(payout_connectors::CybersourcePayouts::new())
            }
        }
    }
}

/// Trait abstracting over connector data types
pub trait ConnectorDataProvider: Sized {
    type ConnectorEnumType: Copy;

    /// Convert variant to this type
    fn from_connector_variant(
        variant: &domain_types::connector_types::ConnectorVariant,
    ) -> Option<Self>;
}

impl<T: PaymentMethodDataTypes + Debug + Default + Send + Sync + 'static + serde::Serialize>
    ConnectorDataProvider for ConnectorData<T>
{
    type ConnectorEnumType = ConnectorEnum;

    fn from_connector_variant(
        variant: &domain_types::connector_types::ConnectorVariant,
    ) -> Option<Self> {
        variant
            .as_payment()
            .map(|c| Self::get_connector_by_name(&c))
    }
}

impl ConnectorDataProvider for SurchargeConnectorData {
    type ConnectorEnumType = SurchargeConnectorEnum;

    fn from_connector_variant(
        variant: &domain_types::connector_types::ConnectorVariant,
    ) -> Option<Self> {
        variant
            .as_surcharge()
            .map(|c| Self::get_connector_by_name(&c))
    }
}

impl ConnectorDataProvider for PayoutConnectorData {
    type ConnectorEnumType = PayoutConnectorEnum;

    fn from_connector_variant(
        variant: &domain_types::connector_types::ConnectorVariant,
    ) -> Option<Self> {
        variant
            .as_payout()
            .or_else(|| {
                variant
                    .as_payment()
                    .and_then(|c| PayoutConnectorEnum::try_from(c).ok())
            })
            .map(|c| Self::get_connector_by_name(&c))
    }
}

pub struct ResponseRouterData<Response, RouterData> {
    pub response: Response,
    pub router_data: RouterData,
    pub http_code: u16,
}
