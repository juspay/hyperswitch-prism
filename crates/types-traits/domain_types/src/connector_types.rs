use std::collections::HashMap;

use common_enums::{
    AttemptStatus, AuthenticationType, AuthorizationStatus, Currency, DisputeStatus, EventClass,
    PaymentChannel, PaymentMethod, PaymentMethodType,
};
use common_utils::{
    errors,
    ext_traits::{OptionExt, ValueExt},
    pii::IpAddress,
    types::{MinorUnit, Money, StringMajorUnit, StringMinorUnit},
    CustomResult, CustomerId, Email, SecretSerdeValue,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
use time::PrimitiveDateTime;

use crate::{
    errors::{IntegrationError, IntegrationErrorContext, WebhookError},
    mandates::{CustomerAcceptance, MandateData},
    payment_address::{self, Address, AddressDetails, PhoneDetails},
    payment_method_data::{self, Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{self, ConnectorResponseData},
    router_request_types::{
        self, AcceptDisputeIntegrityObject, AuthoriseIntegrityObject, BrowserInformation,
        CaptureIntegrityObject, CreateOrderIntegrityObject, DefendDisputeIntegrityObject,
        PaymentMethodTokenIntegrityObject, PaymentSynIntegrityObject, PaymentVoidIntegrityObject,
        PaymentVoidPostCaptureIntegrityObject, RefundIntegrityObject, RefundSyncIntegrityObject,
        RepeatPaymentIntegrityObject, SetupMandateIntegrityObject, SubmitEvidenceIntegrityObject,
        SyncRequestType,
    },
    router_response_types::RedirectForm,
    types::{
        ConnectorInfo, Connectors, PaymentMethodDataType, PaymentMethodDetails,
        PaymentMethodTypeMetadata, SupportedPaymentMethods,
    },
    utils::{missing_field_err, Error, ForeignTryFrom},
};
use grpc_api_types::payments::connector_specific_config::Config as AuthType;
use url::Url;

// snake case for enum variants
#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    EnumIter,
    EnumString,
    serde::Deserialize,
    Eq,
    Hash,
    PartialEq,
    Serialize,
)]
#[strum(serialize_all = "snake_case")]
pub enum ConnectorEnum {
    Adyen,
    Forte,
    Razorpay,
    RazorpayV2,
    Fiserv,
    Elavon,
    Xendit,
    Checkout,
    Authorizedotnet,
    Bamboraapac,
    Mifinity,
    Phonepe,
    Cashfree,
    Paytm,
    Fiuu,
    Payu,
    Cashtocode,
    Novalnet,
    Nexinets,
    Noon,
    Braintree,
    Volt,
    Calida,
    Cryptopay,
    Helcim,
    Dlocal,
    Placetopay,
    Rapyd,
    Aci,
    Trustpay,
    Stripe,
    Cybersource,
    Worldpay,
    Worldpayvantiv,
    Worldpayxml,
    Multisafepay,
    Payload,
    Fiservemea,
    Paysafe,
    Datatrans,
    Bluesnap,
    Authipay,
    Silverflow,
    Celero,
    Paypal,
    Stax,
    Billwerk,
    Hipay,
    Trustpayments,
    Redsys,
    Globalpay,
    Nuvei,
    Iatapay,
    Nmi,
    Shift4,
    Paybox,
    Barclaycard,
    Nexixpay,
    Mollie,
    Airwallex,
    Tsys,
    Bankofamerica,
    Powertranz,
    Getnet,
    Jpmorgan,
    Bambora,
    Payme,
    Revolut,
    Gigadat,
    Loonio,
    Wellsfargo,
    Hyperpg,
    Zift,
    Revolv3,
    Ppro,
    Fiservcommercehub,
    Truelayer,
    Peachpayments,
    Finix,
    Trustly,
    Itaubank,
    Axisbank,
}

impl ForeignTryFrom<grpc_api_types::payments::Connector> for ConnectorEnum {
    type Error = IntegrationError;

    fn foreign_try_from(
        connector: grpc_api_types::payments::Connector,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match connector {
            grpc_api_types::payments::Connector::Adyen => Ok(Self::Adyen),
            grpc_api_types::payments::Connector::Forte => Ok(Self::Forte),
            grpc_api_types::payments::Connector::Razorpay => Ok(Self::Razorpay),
            grpc_api_types::payments::Connector::Fiserv => Ok(Self::Fiserv),
            grpc_api_types::payments::Connector::Elavon => Ok(Self::Elavon),
            grpc_api_types::payments::Connector::Xendit => Ok(Self::Xendit),
            grpc_api_types::payments::Connector::Checkout => Ok(Self::Checkout),
            grpc_api_types::payments::Connector::Authorizedotnet => Ok(Self::Authorizedotnet),
            grpc_api_types::payments::Connector::Bamboraapac => Ok(Self::Bamboraapac),
            grpc_api_types::payments::Connector::Phonepe => Ok(Self::Phonepe),
            grpc_api_types::payments::Connector::Cashfree => Ok(Self::Cashfree),
            grpc_api_types::payments::Connector::Paytm => Ok(Self::Paytm),
            grpc_api_types::payments::Connector::Fiuu => Ok(Self::Fiuu),
            grpc_api_types::payments::Connector::Payu => Ok(Self::Payu),
            grpc_api_types::payments::Connector::Cashtocode => Ok(Self::Cashtocode),
            grpc_api_types::payments::Connector::Novalnet => Ok(Self::Novalnet),
            grpc_api_types::payments::Connector::Nexinets => Ok(Self::Nexinets),
            grpc_api_types::payments::Connector::Noon => Ok(Self::Noon),
            grpc_api_types::payments::Connector::Mifinity => Ok(Self::Mifinity),
            grpc_api_types::payments::Connector::Braintree => Ok(Self::Braintree),
            grpc_api_types::payments::Connector::Volt => Ok(Self::Volt),
            grpc_api_types::payments::Connector::Calida => Ok(Self::Calida),
            grpc_api_types::payments::Connector::Cryptopay => Ok(Self::Cryptopay),
            grpc_api_types::payments::Connector::Helcim => Ok(Self::Helcim),
            grpc_api_types::payments::Connector::Dlocal => Ok(Self::Dlocal),
            grpc_api_types::payments::Connector::Placetopay => Ok(Self::Placetopay),
            grpc_api_types::payments::Connector::Rapyd => Ok(Self::Rapyd),
            grpc_api_types::payments::Connector::Aci => Ok(Self::Aci),
            grpc_api_types::payments::Connector::Trustpay => Ok(Self::Trustpay),
            grpc_api_types::payments::Connector::Stripe => Ok(Self::Stripe),
            grpc_api_types::payments::Connector::Cybersource => Ok(Self::Cybersource),
            grpc_api_types::payments::Connector::Worldpay => Ok(Self::Worldpay),
            grpc_api_types::payments::Connector::Worldpayxml => Ok(Self::Worldpayxml),
            grpc_api_types::payments::Connector::Multisafepay => Ok(Self::Multisafepay),
            grpc_api_types::payments::Connector::Payload => Ok(Self::Payload),
            grpc_api_types::payments::Connector::Fiservemea => Ok(Self::Fiservemea),
            grpc_api_types::payments::Connector::Paysafe => Ok(Self::Paysafe),
            grpc_api_types::payments::Connector::Datatrans => Ok(Self::Datatrans),
            grpc_api_types::payments::Connector::Bluesnap => Ok(Self::Bluesnap),
            grpc_api_types::payments::Connector::Authipay => Ok(Self::Authipay),
            grpc_api_types::payments::Connector::Silverflow => Ok(Self::Silverflow),
            grpc_api_types::payments::Connector::Celero => Ok(Self::Celero),
            grpc_api_types::payments::Connector::Paypal => Ok(Self::Paypal),
            grpc_api_types::payments::Connector::Stax => Ok(Self::Stax),
            grpc_api_types::payments::Connector::Billwerk => Ok(Self::Billwerk),
            grpc_api_types::payments::Connector::Hipay => Ok(Self::Hipay),
            grpc_api_types::payments::Connector::Trustpayments => Ok(Self::Trustpayments),
            grpc_api_types::payments::Connector::Globalpay => Ok(Self::Globalpay),
            grpc_api_types::payments::Connector::Nuvei => Ok(Self::Nuvei),
            grpc_api_types::payments::Connector::Iatapay => Ok(Self::Iatapay),
            grpc_api_types::payments::Connector::Nmi => Ok(Self::Nmi),
            grpc_api_types::payments::Connector::Shift4 => Ok(Self::Shift4),
            grpc_api_types::payments::Connector::Barclaycard => Ok(Self::Barclaycard),
            grpc_api_types::payments::Connector::Redsys => Ok(Self::Redsys),
            grpc_api_types::payments::Connector::Nexixpay => Ok(Self::Nexixpay),
            grpc_api_types::payments::Connector::Mollie => Ok(Self::Mollie),
            grpc_api_types::payments::Connector::Airwallex => Ok(Self::Airwallex),
            grpc_api_types::payments::Connector::Tsys => Ok(Self::Tsys),
            grpc_api_types::payments::Connector::Bankofamerica => Ok(Self::Bankofamerica),
            grpc_api_types::payments::Connector::Powertranz => Ok(Self::Powertranz),
            grpc_api_types::payments::Connector::Getnet => Ok(Self::Getnet),
            grpc_api_types::payments::Connector::Jpmorgan => Ok(Self::Jpmorgan),
            grpc_api_types::payments::Connector::Bambora => Ok(Self::Bambora),
            grpc_api_types::payments::Connector::Payme => Ok(Self::Payme),
            grpc_api_types::payments::Connector::Revolut => Ok(Self::Revolut),
            grpc_api_types::payments::Connector::Gigadat => Ok(Self::Gigadat),
            grpc_api_types::payments::Connector::Loonio => Ok(Self::Loonio),
            grpc_api_types::payments::Connector::Wellsfargo => Ok(Self::Wellsfargo),
            grpc_api_types::payments::Connector::Hyperpg => Ok(Self::Hyperpg),
            grpc_api_types::payments::Connector::Zift => Ok(Self::Zift),
            grpc_api_types::payments::Connector::Revolv3 => Ok(Self::Revolv3),
            grpc_api_types::payments::Connector::Ppro => Ok(Self::Ppro),
            grpc_api_types::payments::Connector::Fiservcommercehub => Ok(Self::Fiservcommercehub),
            grpc_api_types::payments::Connector::Truelayer => Ok(Self::Truelayer),
            grpc_api_types::payments::Connector::Peachpayments => Ok(Self::Peachpayments),
            grpc_api_types::payments::Connector::Finix => Ok(Self::Finix),
            grpc_api_types::payments::Connector::Trustly => Ok(Self::Trustly),
            grpc_api_types::payments::Connector::Itaubank => Ok(Self::Itaubank),
            grpc_api_types::payments::Connector::Axisbank => Ok(Self::Axisbank),
            grpc_api_types::payments::Connector::Unspecified => {
                Err(IntegrationError::InvalidDataFormat {
                    field_name: "connector",
                    context: IntegrationErrorContext::default(),
                }
                .into())
            }
            _ => Err(IntegrationError::InvalidDataFormat {
                field_name: "connector",
                context: IntegrationErrorContext::default(),
            }
            .into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct PaymentId(pub String);

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct UpdateHistory {
    pub connector_mandate_id: Option<String>,
    pub payment_method_id: String,
    pub original_payment_id: Option<PaymentId>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Eq, PartialEq)]
pub struct ConnectorMandateReferenceId {
    connector_mandate_id: Option<String>,
    payment_method_id: Option<String>,
    update_history: Option<Vec<UpdateHistory>>,
    mandate_metadata: Option<SecretSerdeValue>,
    connector_mandate_request_reference_id: Option<String>,
}

impl ConnectorMandateReferenceId {
    pub fn new(
        connector_mandate_id: Option<String>,
        payment_method_id: Option<String>,
        update_history: Option<Vec<UpdateHistory>>,
        mandate_metadata: Option<SecretSerdeValue>,
        connector_mandate_request_reference_id: Option<String>,
    ) -> Self {
        Self {
            connector_mandate_id,
            payment_method_id,
            update_history,
            mandate_metadata,
            connector_mandate_request_reference_id,
        }
    }

    pub fn get_connector_mandate_id(&self) -> Option<String> {
        self.connector_mandate_id.clone()
    }

    pub fn get_payment_method_id(&self) -> Option<&String> {
        self.payment_method_id.as_ref()
    }

    pub fn get_update_history(&self) -> Option<&Vec<UpdateHistory>> {
        self.update_history.as_ref()
    }

    pub fn get_mandate_metadata(&self) -> Option<SecretSerdeValue> {
        self.mandate_metadata.clone()
    }

    pub fn get_connector_mandate_request_reference_id(&self) -> Option<String> {
        self.connector_mandate_request_reference_id.clone()
    }
}

pub trait RawConnectorRequestResponse {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>);
    fn get_raw_connector_response(&self) -> Option<Secret<String>>;
    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>);
    fn get_raw_connector_request(&self) -> Option<Secret<String>>;
}

pub trait ConnectorResponseHeaders {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>);
    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap>;
    fn get_connector_response_headers_as_map(&self) -> HashMap<String, String> {
        self.get_connector_response_headers()
            .map(|headers| {
                headers
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|v| (name.to_string(), v.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Eq, PartialEq)]
pub struct NetworkTokenWithNTIRef {
    pub network_transaction_id: String,
    pub token_exp_month: Option<Secret<String>>,
    pub token_exp_year: Option<Secret<String>>,
}

#[derive(Eq, PartialEq, Debug, serde::Deserialize, serde::Serialize, Clone)]
pub enum MandateReferenceId {
    ConnectorMandateId(ConnectorMandateReferenceId), // mandate_id sent by connector
    NetworkMandateId(String), // network_txns_id sent by Issuer to connector, Used for PG agnostic mandate txns along with card data
    NetworkTokenWithNTI(NetworkTokenWithNTIRef), // network_txns_id sent by Issuer to connector, Used for PG agnostic mandate txns along with network token data
}

#[derive(Default, Eq, PartialEq, Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct MandateIds {
    pub mandate_id: Option<String>,
    pub mandate_reference_id: Option<MandateReferenceId>,
}

impl MandateIds {
    pub fn is_network_transaction_id_flow(&self) -> bool {
        matches!(
            self.mandate_reference_id,
            Some(MandateReferenceId::NetworkMandateId(_))
        )
    }

    pub fn new(mandate_id: String) -> Self {
        Self {
            mandate_id: Some(mandate_id),
            mandate_reference_id: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PaymentsSyncData {
    pub connector_transaction_id: ResponseId,
    pub encoded_data: Option<String>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub sync_type: SyncRequestType,
    pub mandate_id: Option<MandateIds>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub currency: Currency,
    pub payment_experience: Option<common_enums::PaymentExperience>,
    pub amount: MinorUnit,
    pub all_keys_required: Option<bool>,
    pub integrity_object: Option<PaymentSynIntegrityObject>,
    pub split_payments: Option<SplitPaymentsRequest>,
    pub setup_future_usage: Option<common_enums::FutureUsage>,
}

impl PaymentsSyncData {
    /// Returns true if payment should be automatically captured, false for manual capture.
    ///
    /// Maps capture methods to boolean intent:
    /// - Automatic/SequentialAutomatic/None → true (auto capture)
    /// - Manual/ManualMultiple/Scheduled → false (manual capture)
    ///
    /// Note: This is a pure getter, not a validation. Connectors that don't support
    /// specific capture methods should validate explicitly during request building.
    pub fn is_auto_capture(&self) -> bool {
        !matches!(
            self.capture_method,
            Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        )
    }
    pub fn get_connector_transaction_id(&self) -> CustomResult<String, IntegrationError> {
        match self.connector_transaction_id.clone() {
            ResponseId::ConnectorTransactionId(txn_id) => Ok(txn_id),
            _ => Err(errors::ValidationError::IncorrectValueProvided {
                field_name: "connector_transaction_id",
            })
            .attach_printable("Expected connector transaction ID not found")
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?,
        }
    }
    pub fn is_mandate_payment(&self) -> bool {
        matches!(
            self.setup_future_usage,
            Some(common_enums::FutureUsage::OffSession)
        )
    }
}

#[derive(Debug, Clone)]
pub struct PaymentFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub customer_id: Option<CustomerId>,
    pub connector_customer: Option<String>,
    pub payment_id: String,
    pub attempt_id: String,
    pub status: AttemptStatus,
    pub payment_method: PaymentMethod,
    pub description: Option<String>,
    pub return_url: Option<String>,
    pub address: payment_address::PaymentAddress,
    pub auth_type: AuthenticationType,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub amount_captured: Option<i64>,
    // minor amount for amount frameworka
    pub minor_amount_captured: Option<MinorUnit>,
    pub minor_amount_capturable: Option<MinorUnit>,
    pub amount: Option<Money>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub session_token: Option<String>,
    pub reference_id: Option<String>,
    pub connector_order_id: Option<String>,
    pub preprocessing_id: Option<String>,
    ///for switching between two different versions of the same connector
    pub connector_api_version: Option<String>,
    /// Contains a reference ID that should be sent in the connector request
    pub connector_request_reference_id: String,
    pub test_mode: Option<bool>,
    pub connector_http_status_code: Option<u16>,
    pub connector_response_headers: Option<http::HeaderMap>,
    pub external_latency: Option<u128>,
    pub connectors: Connectors,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub vault_headers: Option<HashMap<String, Secret<String>>>,
    /// This field is used to store various data regarding the response from connector
    pub connector_response: Option<ConnectorResponseData>,
    pub recurring_mandate_payment_data: Option<RecurringMandatePaymentData>,
    pub order_details: Option<Vec<payment_address::OrderDetailsWithAmount>>,
    // stores the authorized amount in case of partial authorization
    pub minor_amount_authorized: Option<MinorUnit>,
    pub l2_l3_data: Option<Box<L2L3Data>>,
}

impl PaymentFlowData {
    pub fn set_status(&mut self, status: AttemptStatus) {
        self.status = status;
    }

    pub fn get_billing(&self) -> Result<&Address, Error> {
        self.address
            .get_payment_method_billing()
            .ok_or_else(missing_field_err("billing"))
    }

    pub fn get_billing_country(&self) -> Result<common_enums::CountryAlpha2, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|a| a.address.as_ref())
            .and_then(|ad| ad.country)
            .ok_or_else(missing_field_err(
                "payment_method_data.billing.address.country",
            ))
    }

    pub fn get_billing_phone(&self) -> Result<&PhoneDetails, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|a| a.phone.as_ref())
            .ok_or_else(missing_field_err("billing.phone"))
    }

    pub fn get_optional_billing(&self) -> Option<&Address> {
        self.address.get_payment_method_billing()
    }

    pub fn get_optional_payment_billing(&self) -> Option<&Address> {
        self.address.get_payment_billing()
    }

    pub fn get_optional_shipping(&self) -> Option<&Address> {
        self.address.get_shipping()
    }

    pub fn get_optional_shipping_first_name(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.first_name)
        })
    }

    pub fn get_optional_shipping_last_name(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.last_name)
        })
    }

    pub fn get_optional_shipping_line1(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.line1)
        })
    }

    pub fn get_optional_shipping_line2(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.line2)
        })
    }

    pub fn get_optional_shipping_city(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.city)
        })
    }

    pub fn get_optional_shipping_state(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.state)
        })
    }

    pub fn get_optional_shipping_full_name(&self) -> Option<Secret<String>> {
        self.get_optional_shipping()
            .and_then(|shipping_details| shipping_details.address.as_ref())
            .and_then(|shipping_address| shipping_address.get_optional_full_name())
    }

    pub fn get_optional_shipping_country(&self) -> Option<common_enums::CountryAlpha2> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.country)
        })
    }

    pub fn get_optional_shipping_zip(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.zip)
        })
    }

    pub fn get_optional_shipping_email(&self) -> Option<Email> {
        self.address
            .get_shipping()
            .and_then(|shipping_address| shipping_address.clone().email)
    }

    pub fn get_optional_shipping_phone_number(&self) -> Option<Secret<String>> {
        self.address
            .get_shipping()
            .and_then(|shipping_address| shipping_address.clone().phone)
            .and_then(|phone_details| phone_details.get_number_with_country_code().ok())
    }

    pub fn get_optional_shipping_line3(&self) -> Option<Secret<String>> {
        self.address.get_shipping().and_then(|shipping_address| {
            shipping_address
                .clone()
                .address
                .and_then(|shipping_details| shipping_details.line3)
        })
    }

    pub fn get_description(&self) -> Result<String, Error> {
        self.description
            .clone()
            .ok_or_else(missing_field_err("description"))
    }
    pub fn get_billing_address(&self) -> Result<&AddressDetails, Error> {
        self.address
            .get_payment_method_billing()
            .as_ref()
            .and_then(|a| a.address.as_ref())
            .ok_or_else(missing_field_err("billing.address"))
    }

    pub fn get_billing_address_from_payment_address(&self) -> Result<&AddressDetails, Error> {
        self.address
            .get_payment_billing()
            .as_ref()
            .and_then(|a| a.address.as_ref())
            .ok_or_else(missing_field_err("billing.address"))
    }

    pub fn get_connector_feature_data(&self) -> Result<SecretSerdeValue, Error> {
        self.connector_feature_data
            .clone()
            .ok_or_else(missing_field_err("connector_feature_data"))
    }

    pub fn get_connector_meta(&self) -> Result<SecretSerdeValue, Error> {
        self.get_connector_feature_data()
    }

    pub fn get_session_token(&self) -> Result<String, Error> {
        self.session_token
            .clone()
            .ok_or_else(missing_field_err("session_token"))
    }

    pub fn get_access_token(&self) -> Result<String, Error> {
        self.access_token
            .as_ref()
            .map(|token_data| token_data.access_token.clone().expose())
            .ok_or_else(missing_field_err("access_token"))
    }

    pub fn get_access_token_data(&self) -> Result<ServerAuthenticationTokenResponseData, Error> {
        self.access_token
            .clone()
            .ok_or_else(missing_field_err("access_token"))
    }

    pub fn set_access_token(
        mut self,
        access_token: Option<ServerAuthenticationTokenResponseData>,
    ) -> Self {
        self.access_token = access_token;
        self
    }

    pub fn get_billing_first_name(&self) -> Result<Secret<String>, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.first_name.clone())
            })
            .ok_or_else(missing_field_err(
                "payment_method_data.billing.address.first_name",
            ))
    }

    pub fn get_billing_full_name(&self) -> Result<Secret<String>, Error> {
        self.get_optional_billing()
            .and_then(|billing_details| billing_details.address.as_ref())
            .and_then(|billing_address| billing_address.get_optional_full_name())
            .ok_or_else(missing_field_err(
                "payment_method_data.billing.address.first_name",
            ))
    }

    pub fn get_billing_last_name(&self) -> Result<Secret<String>, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.last_name.clone())
            })
            .ok_or_else(missing_field_err(
                "payment_method_data.billing.address.last_name",
            ))
    }

    pub fn get_billing_line1(&self) -> Result<Secret<String>, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.line1.clone())
            })
            .ok_or_else(missing_field_err(
                "payment_method_data.billing.address.line1",
            ))
    }
    pub fn get_billing_city(&self) -> Result<Secret<String>, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.city)
            })
            .ok_or_else(missing_field_err(
                "payment_method_data.billing.address.city",
            ))
    }

    pub fn get_billing_email(&self) -> Result<Email, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| billing_address.email.clone())
            .ok_or_else(missing_field_err("payment_method_data.billing.email"))
    }

    pub fn get_billing_phone_number(&self) -> Result<Secret<String>, Error> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| billing_address.clone().phone)
            .map(|phone_details| phone_details.get_number_with_country_code())
            .transpose()?
            .ok_or_else(missing_field_err("payment_method_data.billing.phone"))
    }

    pub fn get_optional_billing_line1(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.line1)
            })
    }

    pub fn get_optional_billing_line2(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.line2)
            })
    }

    pub fn get_optional_billing_line3(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.line3)
            })
    }

    pub fn get_optional_billing_city(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.city)
            })
    }

    pub fn get_optional_billing_country(&self) -> Option<common_enums::CountryAlpha2> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.country)
            })
    }

    pub fn get_optional_billing_zip(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.zip)
            })
    }

    pub fn get_optional_billing_state(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.state)
            })
    }

    pub fn get_optional_billing_first_name(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.first_name)
            })
    }

    pub fn get_optional_billing_last_name(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .address
                    .and_then(|billing_details| billing_details.last_name)
            })
    }

    pub fn get_optional_billing_phone_number(&self) -> Option<Secret<String>> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| {
                billing_address
                    .clone()
                    .phone
                    .and_then(|phone_data| phone_data.number)
            })
    }

    pub fn get_optional_billing_email(&self) -> Option<Email> {
        self.address
            .get_payment_method_billing()
            .and_then(|billing_address| billing_address.clone().email)
    }
    pub fn to_connector_meta<T>(&self) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned,
    {
        self.get_connector_meta()?
            .parse_value(std::any::type_name::<T>())
            .change_context(IntegrationError::NoConnectorMetaData {
                context: Default::default(),
            })
    }

    pub fn is_three_ds(&self) -> bool {
        matches!(self.auth_type, AuthenticationType::ThreeDs)
    }

    pub fn get_shipping_address(&self) -> Result<&AddressDetails, Error> {
        self.address
            .get_shipping()
            .and_then(|a| a.address.as_ref())
            .ok_or_else(missing_field_err("shipping.address"))
    }

    pub fn get_shipping_address_with_phone_number(&self) -> Result<&Address, Error> {
        self.address
            .get_shipping()
            .ok_or_else(missing_field_err("shipping"))
    }

    pub fn get_customer_id(&self) -> Result<CustomerId, Error> {
        self.customer_id
            .to_owned()
            .ok_or_else(missing_field_err("customer_id"))
    }
    pub fn get_connector_customer_id(&self) -> Result<String, Error> {
        self.connector_customer
            .to_owned()
            .ok_or_else(missing_field_err("connector_customer_id"))
    }
    pub fn get_preprocessing_id(&self) -> Result<String, Error> {
        self.preprocessing_id
            .to_owned()
            .ok_or_else(missing_field_err("preprocessing_id"))
    }

    pub fn get_reference_id(&self) -> Result<String, Error> {
        self.reference_id
            .to_owned()
            .ok_or_else(missing_field_err("merchant_order_id"))
    }

    pub fn get_optional_billing_full_name(&self) -> Option<Secret<String>> {
        self.get_optional_billing()
            .and_then(|billing_details| billing_details.address.as_ref())
            .and_then(|billing_address| billing_address.get_optional_full_name())
    }

    pub fn set_order_reference_id(mut self, reference_id: Option<String>) -> Self {
        if reference_id.is_some() && self.reference_id.is_none() {
            self.reference_id = reference_id;
        }
        self
    }
    pub fn set_session_token_id(mut self, session_token_id: Option<String>) -> Self {
        if session_token_id.is_some() && self.session_token.is_none() {
            self.session_token = session_token_id;
        }
        self
    }
    pub fn set_connector_customer_id(mut self, connector_customer_id: Option<String>) -> Self {
        if connector_customer_id.is_some() && self.connector_customer.is_none() {
            self.connector_customer = connector_customer_id;
        }
        self
    }

    pub fn set_access_token_id(mut self, access_token_id: Option<String>) -> Self {
        if let (Some(token_id), None) = (access_token_id, &self.access_token) {
            self.access_token = Some(ServerAuthenticationTokenResponseData {
                access_token: token_id.into(),
                token_type: None,
                expires_in: None,
            });
        }
        self
    }

    pub fn get_return_url(&self) -> Option<String> {
        self.return_url.clone()
    }

    // Helper methods for additional headers
    pub fn get_header(&self, key: &str) -> Option<&Secret<String>> {
        self.vault_headers.as_ref().and_then(|h| h.get(key))
    }

    pub fn get_optional_payment_billing_full_name(&self) -> Option<Secret<String>> {
        self.get_optional_payment_billing()
            .and_then(|billing_details| billing_details.address.as_ref())
            .and_then(|billing_address| billing_address.get_optional_full_name())
    }

    pub fn get_payment_billing_full_name(&self) -> Result<Secret<String>, Error> {
        self.get_optional_payment_billing()
            .and_then(|billing_details| billing_details.address.as_ref())
            .and_then(|billing_address| billing_address.get_optional_full_name())
            .ok_or_else(missing_field_err("address.billing first_name & last_name"))
    }

    pub fn get_recurring_mandate_payment_data(&self) -> Result<RecurringMandatePaymentData, Error> {
        self.recurring_mandate_payment_data
            .to_owned()
            .ok_or_else(missing_field_err("recurring_mandate_payment_data"))
    }
}

impl RawConnectorRequestResponse for PaymentFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }
}

impl ConnectorResponseHeaders for PaymentFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct PaymentVoidData {
    pub connector_transaction_id: String,
    pub cancellation_reason: Option<String>,
    pub integrity_object: Option<PaymentVoidIntegrityObject>,
    pub raw_connector_response: Option<String>,
    pub browser_info: Option<BrowserInformation>,
    pub amount: Option<MinorUnit>,
    pub currency: Option<Currency>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub metadata: Option<SecretSerdeValue>,
    pub merchant_order_id: Option<String>,
}

impl PaymentVoidData {
    // fn get_amount(&self) -> Result<i64, Error> {
    //     self.amount.ok_or_else(missing_field_err("amount"))
    // }
    // fn get_currency(&self) -> Result<common_enums::Currency, Error> {
    //     self.currency.ok_or_else(missing_field_err("currency"))
    // }
    pub fn get_cancellation_reason(&self) -> Result<String, Error> {
        self.cancellation_reason
            .clone()
            .ok_or_else(missing_field_err("cancellation_reason"))
    }
    // fn get_browser_info(&self) -> Result<BrowserInformation, Error> {
    //     self.browser_info
    //         .clone()
    //         .ok_or_else(missing_field_err("browser_info"))
    // }
    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }
    pub fn get_ip_address_as_optional(&self) -> Option<Secret<String, IpAddress>> {
        self.browser_info.clone().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        })
    }

    pub fn get_ip_address(&self) -> Result<Secret<String, IpAddress>, Error> {
        self.get_ip_address_as_optional()
            .ok_or_else(missing_field_err("browser_info.ip_address"))
    }
}

#[derive(Debug, Clone)]
pub struct PaymentsCancelPostCaptureData {
    pub connector_transaction_id: String,
    pub cancellation_reason: Option<String>,
    pub integrity_object: Option<PaymentVoidPostCaptureIntegrityObject>,
    pub raw_connector_response: Option<String>,
    pub browser_info: Option<BrowserInformation>,
}

impl PaymentsCancelPostCaptureData {
    pub fn get_cancellation_reason(&self) -> Result<String, Error> {
        self.cancellation_reason
            .clone()
            .ok_or_else(missing_field_err("cancellation_reason"))
    }

    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }
}

#[derive(Debug, Clone)]
pub struct PaymentsAuthorizeData<T: PaymentMethodDataTypes> {
    pub payment_method_data: PaymentMethodData<T>,
    /// total amount (original_amount + surcharge_amount + tax_on_surcharge_amount)
    /// If connector supports separate field for surcharge amount, consider using below functions defined on `PaymentsAuthorizeData` to fetch original amount and surcharge amount separately
    /// ```text
    /// get_original_amount()
    /// get_surcharge_amount()
    /// get_tax_on_surcharge_amount()
    /// get_total_surcharge_amount() // returns surcharge_amount + tax_on_surcharge_amount
    /// ```
    pub amount: MinorUnit,
    pub order_tax_amount: Option<MinorUnit>,
    pub email: Option<Email>,
    pub customer_name: Option<String>,
    pub currency: Currency,
    pub confirm: bool,
    pub billing_descriptor: Option<BillingDescriptor>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub router_return_url: Option<String>,
    pub webhook_url: Option<String>,
    pub complete_authorize_url: Option<String>,
    // Mandates
    pub mandate_id: Option<MandateIds>,
    pub setup_future_usage: Option<common_enums::FutureUsage>,
    pub off_session: Option<bool>,
    pub browser_info: Option<BrowserInformation>,
    pub order_category: Option<String>,
    pub session_token: Option<String>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub customer_acceptance: Option<CustomerAcceptance>,
    pub enrolled_for_3ds: Option<bool>,
    pub related_transaction_id: Option<String>,
    pub payment_experience: Option<common_enums::PaymentExperience>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub customer_id: Option<CustomerId>,
    pub request_incremental_authorization: Option<bool>,
    pub metadata: Option<SecretSerdeValue>,
    pub authentication_data: Option<router_request_types::AuthenticationData>,
    pub split_payments: Option<SplitPaymentsRequest>,
    // New amount for amount frame work
    pub minor_amount: MinorUnit,
    /// Merchant's identifier for the payment/invoice. This will be sent to the connector
    /// if the connector provides support to accept multiple reference ids.
    /// In case the connector supports only one reference id, Hyperswitch's Payment ID will be sent as reference.
    pub merchant_order_id: Option<String>,
    pub shipping_cost: Option<MinorUnit>,
    pub merchant_account_id: Option<String>,
    pub integrity_object: Option<AuthoriseIntegrityObject>,
    pub merchant_config_currency: Option<Currency>,
    pub all_keys_required: Option<bool>,
    pub request_extended_authorization: Option<bool>,
    pub enable_overcapture: Option<bool>,
    pub setup_mandate_details: Option<MandateData>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub connector_testing_data: Option<SecretSerdeValue>,
    pub payment_channel: Option<PaymentChannel>,
    pub enable_partial_authorization: Option<bool>,
    pub locale: Option<String>,
    pub redirect_response: Option<ContinueRedirectionResponse>,
    pub threeds_method_comp_ind: Option<ThreeDsCompletionIndicator>,
    pub continue_redirection_url: Option<Url>,
    pub tokenization: Option<common_enums::Tokenization>,
}

impl<T: PaymentMethodDataTypes> PaymentsAuthorizeData<T> {
    /// Returns true if payment should be automatically captured, false for manual capture.
    ///
    /// Maps capture methods to boolean intent:
    /// - Automatic/SequentialAutomatic/None → true (auto capture)
    /// - Manual/ManualMultiple/Scheduled → false (manual capture)
    ///
    /// Note: This is a pure getter, not a validation. Connectors that don't support
    /// specific capture methods should validate explicitly during request building.
    pub fn is_auto_capture(&self) -> bool {
        !matches!(
            self.capture_method,
            Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        )
    }
    pub fn get_email(&self) -> Result<Email, Error> {
        self.email.clone().ok_or_else(missing_field_err("email"))
    }
    pub fn get_optional_email(&self) -> Option<Email> {
        self.email.clone()
    }
    pub fn get_browser_info(&self) -> Result<BrowserInformation, Error> {
        self.browser_info
            .clone()
            .ok_or_else(missing_field_err("browser_info"))
    }
    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }

    pub fn get_customer_id(&self) -> Result<CustomerId, Error> {
        self.customer_id
            .to_owned()
            .ok_or_else(missing_field_err("customer_id"))
    }

    pub fn get_card(&self) -> Result<Card<T>, Error> {
        match &self.payment_method_data {
            PaymentMethodData::Card(card) => Ok(card.clone()),
            _ => Err(missing_field_err("card")()),
        }
    }

    pub fn get_complete_authorize_url(&self) -> Result<String, Error> {
        self.complete_authorize_url
            .clone()
            .ok_or_else(missing_field_err("complete_authorize_url"))
    }

    pub fn connector_mandate_id(&self) -> Option<String> {
        self.mandate_id
            .as_ref()
            .and_then(|mandate_ids| match &mandate_ids.mandate_reference_id {
                Some(MandateReferenceId::ConnectorMandateId(connector_mandate_ids)) => {
                    connector_mandate_ids.get_connector_mandate_id()
                }
                Some(MandateReferenceId::NetworkMandateId(_))
                | None
                | Some(MandateReferenceId::NetworkTokenWithNTI(_)) => None,
            })
    }

    pub fn get_optional_network_transaction_id(&self) -> Option<String> {
        self.mandate_id
            .as_ref()
            .and_then(|mandate_ids| match &mandate_ids.mandate_reference_id {
                Some(MandateReferenceId::NetworkMandateId(network_transaction_id)) => {
                    Some(network_transaction_id.clone())
                }
                Some(MandateReferenceId::ConnectorMandateId(_))
                | Some(MandateReferenceId::NetworkTokenWithNTI(_))
                | None => None,
            })
    }

    pub fn is_mandate_payment(&self) -> bool {
        ((self.customer_acceptance.is_some() || self.setup_mandate_details.is_some())
            && self.setup_future_usage == Some(common_enums::FutureUsage::OffSession))
            || self
                .mandate_id
                .as_ref()
                .and_then(|mandate_ids| mandate_ids.mandate_reference_id.as_ref())
                .is_some()
    }
    // fn is_cit_mandate_payment(&self) -> bool {
    //     (self.customer_acceptance.is_some() || self.setup_mandate_details.is_some())
    //         && self.setup_future_usage == Some(storage_enums::FutureUsage::OffSession)
    // }
    pub fn get_webhook_url(&self) -> Result<String, Error> {
        self.webhook_url
            .clone()
            .ok_or_else(missing_field_err("webhook_url"))
    }
    pub fn get_router_return_url(&self) -> Result<String, Error> {
        self.router_return_url
            .clone()
            .ok_or_else(missing_field_err("return_url"))
    }
    pub fn is_wallet(&self) -> bool {
        matches!(self.payment_method_data, PaymentMethodData::Wallet(_))
    }
    pub fn is_card(&self) -> bool {
        matches!(self.payment_method_data, PaymentMethodData::Card(_))
    }

    pub fn get_payment_method_type(&self) -> Result<PaymentMethodType, Error> {
        self.payment_method_type
            .to_owned()
            .ok_or_else(missing_field_err("payment_method_type"))
    }

    pub fn get_connector_mandate_id(&self) -> Result<String, Error> {
        self.connector_mandate_id()
            .ok_or_else(missing_field_err("connector_mandate_id"))
    }
    pub fn get_ip_address_as_optional(&self) -> Option<Secret<String, IpAddress>> {
        self.browser_info.clone().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        })
    }

    pub fn get_ip_address(&self) -> Result<Secret<String, IpAddress>, Error> {
        self.get_ip_address_as_optional()
            .ok_or_else(missing_field_err("browser_info.ip_address"))
    }
    // fn get_original_amount(&self) -> i64 {
    //     self.surcharge_details
    //         .as_ref()
    //         .map(|surcharge_details| surcharge_details.original_amount.get_amount_as_i64())
    //         .unwrap_or(self.amount)
    // }
    // fn get_surcharge_amount(&self) -> Option<i64> {
    //     self.surcharge_details
    //         .as_ref()
    //         .map(|surcharge_details| surcharge_details.surcharge_amount.get_amount_as_i64())
    // }
    // fn get_tax_on_surcharge_amount(&self) -> Option<i64> {
    //     self.surcharge_details.as_ref().map(|surcharge_details| {
    //         surcharge_details
    //             .tax_on_surcharge_amount
    //             .get_amount_as_i64()
    //     })
    // }
    // fn get_total_surcharge_amount(&self) -> Option<i64> {
    //     self.surcharge_details.as_ref().map(|surcharge_details| {
    //         surcharge_details
    //             .get_total_surcharge_amount()
    //             .get_amount_as_i64()
    //     })
    // }

    pub fn is_customer_initiated_mandate_payment(&self) -> bool {
        (self.customer_acceptance.is_some() || self.setup_mandate_details.is_some())
            && self.setup_future_usage == Some(common_enums::FutureUsage::OffSession)
    }

    pub fn get_metadata_as_object(&self) -> Option<SecretSerdeValue> {
        self.metadata.clone().and_then(|meta_data| {
            let inner = meta_data.expose();
            match inner {
                serde_json::Value::Null
                | serde_json::Value::Bool(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::String(_)
                | serde_json::Value::Array(_) => None,
                serde_json::Value::Object(_) => Some(SecretSerdeValue::new(inner)),
            }
        })
    }

    // fn get_authentication_data(&self) -> Result<AuthenticationData, Error> {
    //     self.authentication_data
    //         .clone()
    //         .ok_or_else(missing_field_err("authentication_data"))
    // }

    // fn get_connector_mandate_request_reference_id(&self) -> Result<String, Error> {
    //     self.mandate_id
    //         .as_ref()
    //         .and_then(|mandate_ids| match &mandate_ids.mandate_reference_id {
    //             Some(MandateReferenceId::ConnectorMandateId(connector_mandate_ids)) => {
    //                 connector_mandate_ids.get_connector_mandate_request_reference_id()
    //             }
    //             Some(MandateReferenceId::NetworkMandateId(_))
    //             | None
    //             | Some(MandateReferenceId::NetworkTokenWithNTI(_)) => None,
    //         })
    //         .ok_or_else(missing_field_err("connector_mandate_request_reference_id"))
    // }

    pub fn set_session_token(mut self, session_token: Option<String>) -> Self {
        self.session_token = session_token;
        self
    }

    pub fn set_access_token(mut self, access_token: Option<String>) -> Self {
        self.access_token = access_token.map(|token| ServerAuthenticationTokenResponseData {
            access_token: token.into(),
            token_type: None,
            expires_in: None,
        });
        self
    }

    pub fn get_access_token_optional(&self) -> Option<String> {
        self.access_token
            .as_ref()
            .map(|token_data| token_data.access_token.clone().expose())
    }

    pub fn get_connector_testing_data(&self) -> Option<SecretSerdeValue> {
        self.connector_testing_data.clone()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ResponseId {
    ConnectorTransactionId(String),
    EncodedData(String),
    #[default]
    NoResponseId,
}
impl ResponseId {
    pub fn get_connector_transaction_id(&self) -> CustomResult<String, errors::ValidationError> {
        match self {
            Self::ConnectorTransactionId(txn_id) => Ok(txn_id.to_string()),
            _ => Err(errors::ValidationError::IncorrectValueProvided {
                field_name: "connector_transaction_id",
            })
            .attach_printable("Expected connector transaction ID not found"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentsResponseData {
    TransactionResponse {
        resource_id: ResponseId,
        redirection_data: Option<Box<RedirectForm>>,
        connector_metadata: Option<serde_json::Value>,
        mandate_reference: Option<Box<MandateReference>>,
        network_txn_id: Option<String>,
        connector_response_reference_id: Option<String>,
        incremental_authorization_allowed: Option<bool>,
        status_code: u16,
    },
    ClientAuthenticationTokenResponse {
        session_data: ClientAuthenticationTokenData,
        status_code: u16,
    },
    PreAuthenticateResponse {
        authentication_data: Option<router_request_types::AuthenticationData>,
        /// For Device Data Collection
        redirection_data: Option<Box<RedirectForm>>,
        connector_response_reference_id: Option<String>,
        status_code: u16,
    },
    AuthenticateResponse {
        resource_id: Option<ResponseId>,
        /// For friction flow
        redirection_data: Option<Box<RedirectForm>>,
        /// For frictionles flow
        authentication_data: Option<router_request_types::AuthenticationData>,
        connector_response_reference_id: Option<String>,
        status_code: u16,
    },
    PostAuthenticateResponse {
        authentication_data: Option<router_request_types::AuthenticationData>,
        connector_response_reference_id: Option<String>,
        status_code: u16,
    },
    MultipleCaptureResponse {
        capture_sync_response_list: HashMap<String, CaptureSyncResponse>,
    },
    IncrementalAuthorizationResponse {
        status: AuthorizationStatus,
        connector_authorization_id: Option<String>,
        status_code: u16,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MandateReference {
    pub connector_mandate_id: Option<String>,
    pub payment_method_id: Option<String>,
    pub connector_mandate_request_reference_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PaymentMethodUpdate {
    Card(CardDetailUpdate),
}

#[derive(Debug, Clone)]
pub struct CardDetailUpdate {
    pub card_exp_month: Option<String>,
    pub card_exp_year: Option<String>,
    pub last4_digits: Option<String>,
    pub issuer_country: Option<String>,
    pub card_issuer: Option<String>,
    pub card_network: Option<String>,
    pub card_holder_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureSyncResponse {
    Success {
        resource_id: ResponseId,
        status: common_enums::AttemptStatus,
        connector_response_reference_id: Option<String>,
        amount: Option<MinorUnit>,
    },
    Error {
        code: String,
        message: String,
        reason: Option<String>,
        status_code: u16,
        amount: Option<MinorUnit>,
    },
}

#[derive(Debug, Clone)]
pub struct PaymentCreateOrderData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub integrity_object: Option<CreateOrderIntegrityObject>,
    pub metadata: Option<SecretSerdeValue>,
    pub webhook_url: Option<String>,
    pub payment_method_type: Option<common_enums::PaymentMethodType>,
}

#[derive(Debug, Clone)]
pub struct PaymentCreateOrderResponse {
    pub connector_order_id: String,
    /// Optional SDK session data for wallet flows (Apple Pay, Google Pay) and other SDK types
    pub session_data: Option<ClientAuthenticationTokenData>,
}

#[derive(Debug, Clone)]
pub struct PaymentMethodTokenizationData<T: PaymentMethodDataTypes> {
    pub payment_method_data: PaymentMethodData<T>,
    pub browser_info: Option<BrowserInformation>,
    pub currency: Currency,
    pub amount: MinorUnit,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub customer_acceptance: Option<CustomerAcceptance>,
    pub setup_future_usage: Option<common_enums::FutureUsage>,
    pub setup_mandate_details: Option<MandateData>,
    pub mandate_id: Option<MandateIds>,
    pub integrity_object: Option<PaymentMethodTokenIntegrityObject>,
    pub split_payments: Option<SplitPaymentsRequest>,
    pub connector_feature_data: Option<common_utils::pii::SecretSerdeValue>,
}

#[derive(Debug, Clone)]
pub struct PaymentMethodTokenResponse {
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct PaymentsPreAuthenticateData<T: PaymentMethodDataTypes> {
    pub payment_method_data: Option<PaymentMethodData<T>>,
    pub amount: MinorUnit,
    pub email: Option<Email>,
    pub currency: Option<Currency>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub router_return_url: Option<Url>,
    pub continue_redirection_url: Option<Url>,
    pub browser_info: Option<BrowserInformation>,
    pub enrolled_for_3ds: bool,
    pub redirect_response: Option<ContinueRedirectionResponse>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub mandate_reference: Option<MandateReferenceId>,
}

impl<T: PaymentMethodDataTypes> PaymentsPreAuthenticateData<T> {
    pub fn is_auto_capture(&self) -> Result<bool, Error> {
        match self.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | None
            | Some(common_enums::CaptureMethod::SequentialAutomatic) => Ok(true),
            Some(common_enums::CaptureMethod::Manual) => Ok(false),
            Some(common_enums::CaptureMethod::ManualMultiple)
            | Some(common_enums::CaptureMethod::Scheduled) => {
                Err(IntegrationError::CaptureMethodNotSupported {
                    context: Default::default(),
                }
                .into())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaymentsAuthenticateData<T: PaymentMethodDataTypes> {
    pub payment_method_data: Option<PaymentMethodData<T>>,
    pub amount: MinorUnit,
    pub email: Option<Email>,
    pub currency: Option<Currency>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub router_return_url: Option<Url>,
    pub continue_redirection_url: Option<Url>,
    pub browser_info: Option<BrowserInformation>,
    pub enrolled_for_3ds: bool,
    pub redirect_response: Option<ContinueRedirectionResponse>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub authentication_data: Option<router_request_types::AuthenticationData>,
}

impl<T: PaymentMethodDataTypes> PaymentsAuthenticateData<T> {
    pub fn is_auto_capture(&self) -> Result<bool, Error> {
        match self.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | None
            | Some(common_enums::CaptureMethod::SequentialAutomatic) => Ok(true),
            Some(common_enums::CaptureMethod::Manual) => Ok(false),
            Some(common_enums::CaptureMethod::ManualMultiple)
            | Some(common_enums::CaptureMethod::Scheduled) => {
                Err(IntegrationError::CaptureMethodNotSupported {
                    context: Default::default(),
                }
                .into())
            }
        }
    }

    pub fn get_browser_info(&self) -> Result<BrowserInformation, Error> {
        self.browser_info
            .clone()
            .ok_or_else(missing_field_err("browser_info"))
    }

    pub fn get_continue_redirection_url(&self) -> Result<Url, Error> {
        self.continue_redirection_url
            .clone()
            .ok_or_else(missing_field_err("continue_redirection_url"))
    }
}

#[derive(Debug, Clone)]
pub struct PaymentsIncrementalAuthorizationData {
    pub minor_amount: MinorUnit,
    pub currency: Currency,
    pub reason: Option<String>,
    pub connector_transaction_id: ResponseId,
    pub connector_feature_data: Option<SecretSerdeValue>,
}

#[derive(Debug, Clone)]
pub struct ClientAuthenticationTokenRequestData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub country: Option<common_enums::CountryAlpha2>,
    pub order_details: Option<Vec<payment_address::OrderDetailsWithAmount>>,
    pub email: Option<Email>,
    pub customer_name: Option<Secret<String>>,
    pub order_tax_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    /// The specific payment method type for which the session token is being generated
    pub payment_method_type: Option<PaymentMethodType>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Indicates if 3DS method data was successfully completed or not
pub enum ThreeDsCompletionIndicator {
    /// 3DS method successfully completed
    #[serde(rename = "Y")]
    Success,
    /// 3DS method was not successful
    #[serde(rename = "N")]
    Failure,
    /// 3DS method URL was unavailable
    #[serde(rename = "U")]
    NotAvailable,
}

#[derive(Debug, Clone)]
pub struct PaymentsPostAuthenticateData<T: PaymentMethodDataTypes> {
    pub payment_method_data: Option<PaymentMethodData<T>>,
    pub amount: MinorUnit,
    pub email: Option<Email>,
    pub currency: Option<Currency>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub router_return_url: Option<Url>,
    pub continue_redirection_url: Option<Url>,
    pub browser_info: Option<BrowserInformation>,
    pub enrolled_for_3ds: bool,
    pub redirect_response: Option<ContinueRedirectionResponse>,
    pub capture_method: Option<common_enums::CaptureMethod>,
}

impl<T: PaymentMethodDataTypes> PaymentsPostAuthenticateData<T> {
    pub fn is_auto_capture(&self) -> Result<bool, Error> {
        match self.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | None
            | Some(common_enums::CaptureMethod::SequentialAutomatic) => Ok(true),
            Some(common_enums::CaptureMethod::Manual) => Ok(false),
            Some(common_enums::CaptureMethod::ManualMultiple)
            | Some(common_enums::CaptureMethod::Scheduled) => {
                Err(IntegrationError::CaptureMethodNotSupported {
                    context: Default::default(),
                }
                .into())
            }
        }
    }
    pub fn get_redirect_response_payload(
        &self,
    ) -> Result<common_utils::pii::SecretSerdeValue, Error> {
        self.redirect_response
            .as_ref()
            .and_then(|res| res.payload.to_owned())
            .ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "request.redirect_response.payload",
                    context: Default::default(),
                }
                .into(),
            )
    }
}

#[derive(Debug, Clone)]
pub struct ContinueRedirectionResponse {
    pub params: Option<Secret<String>>,
    pub payload: Option<SecretSerdeValue>,
}

#[derive(Debug, Clone)]
pub struct ServerSessionAuthenticationTokenRequestData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub browser_info: Option<BrowserInformation>,
}

impl ServerSessionAuthenticationTokenRequestData {
    pub fn get_browser_info(&self) -> Result<BrowserInformation, Error> {
        self.browser_info
            .clone()
            .ok_or_else(missing_field_err("browser_info"))
    }
}

#[derive(Debug, Clone)]
pub struct ServerSessionAuthenticationTokenResponseData {
    pub session_token: String,
}

#[derive(Debug, Clone)]
pub struct ServerAuthenticationTokenRequestData {
    pub grant_type: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ServerAuthenticationTokenResponseData {
    pub access_token: Secret<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ConnectorCustomerData {
    pub customer_id: Option<Secret<String>>,
    pub email: Option<Secret<Email>>,
    pub name: Option<Secret<String>>,
    pub description: Option<String>,
    pub phone: Option<Secret<String>>,
    pub preprocessing_id: Option<String>,
    pub split_payments: Option<SplitPaymentsRequest>,
}

#[derive(Debug, Clone)]
pub struct ConnectorCustomerResponse {
    pub connector_customer_id: String,
}

#[derive(Debug, Clone)]
pub struct MandateRevokeRequestData {
    pub mandate_id: Secret<String>,
    pub connector_mandate_id: Option<Secret<String>>,
    pub payment_method_type: Option<common_enums::PaymentMethodType>,
}

#[derive(Debug, Clone)]
pub struct MandateRevokeResponseData {
    pub mandate_status: common_enums::MandateStatus,
    pub status_code: u16,
}

#[derive(Debug, Default, Clone)]
pub struct RefundSyncData {
    pub connector_transaction_id: String,
    pub connector_refund_id: String,
    pub reason: Option<String>,
    pub refund_connector_metadata: Option<SecretSerdeValue>,
    pub refund_status: common_enums::RefundStatus,
    pub all_keys_required: Option<bool>,
    pub integrity_object: Option<RefundSyncIntegrityObject>,
    pub browser_info: Option<BrowserInformation>,
    /// Charges associated with the payment
    pub split_refunds: Option<SplitRefundsRequest>,
    pub connector_feature_data: Option<SecretSerdeValue>,
}

impl RefundSyncData {
    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }
}

#[derive(Debug, Clone)]
pub struct RefundsResponseData {
    pub connector_refund_id: String,
    pub refund_status: common_enums::RefundStatus,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct RefundFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub status: common_enums::RefundStatus,
    pub refund_id: Option<String>,
    pub connectors: Connectors,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
    pub raw_connector_request: Option<Secret<String>>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub test_mode: Option<bool>,
    pub payment_method: Option<PaymentMethod>,
}

impl RawConnectorRequestResponse for RefundFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }
}

impl ConnectorResponseHeaders for RefundFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

impl RefundFlowData {
    pub fn get_access_token(&self) -> Result<String, Error> {
        self.access_token
            .as_ref()
            .map(|token_data| token_data.access_token.clone().expose())
            .ok_or_else(missing_field_err("access_token"))
    }

    pub fn get_access_token_data(&self) -> Result<ServerAuthenticationTokenResponseData, Error> {
        self.access_token
            .clone()
            .ok_or_else(missing_field_err("access_token"))
    }

    pub fn set_access_token(
        mut self,
        access_token: Option<ServerAuthenticationTokenResponseData>,
    ) -> Self {
        self.access_token = access_token;
        self
    }
}

#[derive(Debug, Clone)]
pub struct RedirectDetailsResponse {
    pub resource_id: Option<ResponseId>,
    pub status: Option<AttemptStatus>,
    pub response_amount: Option<Money>,
    pub connector_response_reference_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_reason: Option<String>,
    pub raw_connector_response: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WebhookDetailsResponse {
    pub resource_id: Option<ResponseId>,
    pub status: AttemptStatus,
    pub connector_response_reference_id: Option<String>,
    pub mandate_reference: Option<Box<MandateReference>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_reason: Option<String>,
    pub raw_connector_response: Option<String>,
    pub status_code: u16,
    pub response_headers: Option<http::HeaderMap>,
    pub transformation_status: common_enums::WebhookTransformationStatus,
    pub amount_captured: Option<i64>,
    // minor amount for amount framework
    pub minor_amount_captured: Option<MinorUnit>,
    pub network_txn_id: Option<String>,
    pub payment_method_update: Option<PaymentMethodUpdate>,
}

#[derive(Debug, Clone)]
pub struct RefundWebhookDetailsResponse {
    pub connector_refund_id: Option<String>,
    pub status: common_enums::RefundStatus,
    pub connector_response_reference_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub raw_connector_response: Option<String>,
    pub status_code: u16,
    pub response_headers: Option<http::HeaderMap>,
}

#[derive(Debug, Clone)]
pub struct DisputeWebhookDetailsResponse {
    pub amount: StringMinorUnit,
    pub currency: Currency,
    pub dispute_id: String,
    pub status: DisputeStatus,
    pub stage: common_enums::DisputeStage,
    pub connector_response_reference_id: Option<String>,
    pub dispute_message: Option<String>,
    pub raw_connector_response: Option<String>,
    pub status_code: u16,
    pub response_headers: Option<http::HeaderMap>,
    /// connector_reason
    pub connector_reason_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpMethod {
    Options,
    Get,
    Post,
    Put,
    Delete,
    Head,
    Trace,
    Connect,
    Patch,
}

#[derive(Debug, Clone)]
pub struct RequestDetails {
    pub method: HttpMethod,
    pub uri: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub query_params: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConnectorWebhookSecrets {
    pub secret: Vec<u8>,
    pub additional_secret: Option<Secret<String>>,
}

#[derive(Debug, Clone)]
pub struct ConnectorRedirectResponseSecrets {
    pub secret: Vec<u8>,
    pub additional_secret: Option<Secret<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    // Payment intent events
    PaymentIntentFailure,
    PaymentIntentSuccess,
    PaymentIntentProcessing,
    PaymentIntentPartiallyFunded,
    PaymentIntentCancelled,
    PaymentIntentCancelFailure,
    PaymentIntentAuthorizationSuccess,
    PaymentIntentAuthorizationFailure,
    PaymentIntentCaptureSuccess,
    PaymentIntentCaptureFailure,
    PaymentIntentExpired,
    PaymentActionRequired,

    // Source events
    SourceChargeable,
    SourceTransactionCreated,

    // Refund events
    RefundFailure,
    RefundSuccess,

    // Dispute events
    DisputeOpened,
    DisputeExpired,
    DisputeAccepted,
    DisputeCancelled,
    DisputeChallenged,
    DisputeWon,
    DisputeLost,

    // Mandate events
    MandateActive,
    MandateFailed,
    MandateRevoked,

    // Misc events
    EndpointVerification,
    ExternalAuthenticationAres,
    FrmApproved,
    FrmRejected,

    // Payout events
    PayoutSuccess,
    PayoutFailure,
    PayoutProcessing,
    PayoutCancelled,
    PayoutCreated,
    PayoutExpired,
    PayoutReversed,

    // Recovery events
    RecoveryPaymentFailure,
    RecoveryPaymentSuccess,
    RecoveryPaymentPending,
    RecoveryInvoiceCancel,
    IncomingWebhookEventUnspecified,

    // Legacy broad categories (for backward compatibility)
    Payment,
    Refund,
    Dispute,
}

impl EventType {
    /// Returns true if this event type is payment-related
    pub fn is_payment_event(&self) -> bool {
        matches!(
            self,
            Self::PaymentIntentFailure
                | Self::PaymentIntentSuccess
                | Self::PaymentIntentProcessing
                | Self::PaymentIntentPartiallyFunded
                | Self::PaymentIntentCancelled
                | Self::PaymentIntentCancelFailure
                | Self::PaymentIntentAuthorizationSuccess
                | Self::PaymentIntentAuthorizationFailure
                | Self::PaymentIntentCaptureSuccess
                | Self::PaymentIntentCaptureFailure
                | Self::PaymentIntentExpired
                | Self::PaymentActionRequired
                | Self::SourceChargeable
                | Self::SourceTransactionCreated
                | Self::Payment
        )
    }

    /// Returns true if this event type is refund-related
    pub fn is_refund_event(&self) -> bool {
        matches!(
            self,
            Self::RefundFailure | Self::RefundSuccess | Self::Refund
        )
    }

    /// Returns true if this event type is dispute-related
    pub fn is_dispute_event(&self) -> bool {
        matches!(
            self,
            Self::DisputeOpened
                | Self::DisputeExpired
                | Self::DisputeAccepted
                | Self::DisputeCancelled
                | Self::DisputeChallenged
                | Self::DisputeWon
                | Self::DisputeLost
                | Self::Dispute
        )
    }

    /// Returns true if this event type is mandate-related
    pub fn is_mandate_event(&self) -> bool {
        matches!(
            self,
            Self::MandateActive | Self::MandateFailed | Self::MandateRevoked
        )
    }

    /// Returns true if this event type is payout-related
    pub fn is_payout_event(&self) -> bool {
        matches!(
            self,
            Self::PayoutSuccess
                | Self::PayoutFailure
                | Self::PayoutProcessing
                | Self::PayoutCancelled
                | Self::PayoutCreated
                | Self::PayoutExpired
                | Self::PayoutReversed
        )
    }

    /// Returns true if this event type is recovery-related
    pub fn is_recovery_event(&self) -> bool {
        matches!(
            self,
            Self::RecoveryPaymentFailure
                | Self::RecoveryPaymentSuccess
                | Self::RecoveryPaymentPending
                | Self::RecoveryInvoiceCancel
        )
    }

    /// Returns true if this event type is miscellaneous
    pub fn is_misc_event(&self) -> bool {
        matches!(
            self,
            Self::EndpointVerification
                | Self::ExternalAuthenticationAres
                | Self::FrmApproved
                | Self::FrmRejected
                | Self::IncomingWebhookEventUnspecified
        )
    }
}

impl ForeignTryFrom<grpc_api_types::payments::WebhookEventType> for EventType {
    type Error = WebhookError;

    fn foreign_try_from(
        value: grpc_api_types::payments::WebhookEventType,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::WebhookEventType::PaymentIntentFailure => {
                Ok(Self::PaymentIntentFailure)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentSuccess => {
                Ok(Self::PaymentIntentSuccess)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentProcessing => {
                Ok(Self::PaymentIntentProcessing)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentPartiallyFunded => {
                Ok(Self::PaymentIntentPartiallyFunded)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentCancelled => {
                Ok(Self::PaymentIntentCancelled)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentCancelFailure => {
                Ok(Self::PaymentIntentCancelFailure)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentAuthorizationSuccess => {
                Ok(Self::PaymentIntentAuthorizationSuccess)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentAuthorizationFailure => {
                Ok(Self::PaymentIntentAuthorizationFailure)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentCaptureSuccess => {
                Ok(Self::PaymentIntentCaptureSuccess)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentCaptureFailure => {
                Ok(Self::PaymentIntentCaptureFailure)
            }
            grpc_api_types::payments::WebhookEventType::PaymentIntentExpired => {
                Ok(Self::PaymentIntentExpired)
            }
            grpc_api_types::payments::WebhookEventType::PaymentActionRequired => {
                Ok(Self::PaymentActionRequired)
            }
            grpc_api_types::payments::WebhookEventType::SourceChargeable => {
                Ok(Self::SourceChargeable)
            }
            grpc_api_types::payments::WebhookEventType::SourceTransactionCreated => {
                Ok(Self::SourceTransactionCreated)
            }
            grpc_api_types::payments::WebhookEventType::WebhookRefundFailure => {
                Ok(Self::RefundFailure)
            }
            grpc_api_types::payments::WebhookEventType::WebhookRefundSuccess => {
                Ok(Self::RefundSuccess)
            }
            grpc_api_types::payments::WebhookEventType::WebhookDisputeOpened => {
                Ok(Self::DisputeOpened)
            }
            grpc_api_types::payments::WebhookEventType::WebhookDisputeExpired => {
                Ok(Self::DisputeExpired)
            }
            grpc_api_types::payments::WebhookEventType::WebhookDisputeAccepted => {
                Ok(Self::DisputeAccepted)
            }
            grpc_api_types::payments::WebhookEventType::WebhookDisputeCancelled => {
                Ok(Self::DisputeCancelled)
            }
            grpc_api_types::payments::WebhookEventType::WebhookDisputeChallenged => {
                Ok(Self::DisputeChallenged)
            }
            grpc_api_types::payments::WebhookEventType::WebhookDisputeWon => Ok(Self::DisputeWon),
            grpc_api_types::payments::WebhookEventType::WebhookDisputeLost => Ok(Self::DisputeLost),
            grpc_api_types::payments::WebhookEventType::MandateActive => Ok(Self::MandateActive),
            grpc_api_types::payments::WebhookEventType::MandateFailed => Ok(Self::MandateFailed),
            grpc_api_types::payments::WebhookEventType::MandateRevoked => Ok(Self::MandateRevoked),
            grpc_api_types::payments::WebhookEventType::EndpointVerification => {
                Ok(Self::EndpointVerification)
            }
            grpc_api_types::payments::WebhookEventType::ExternalAuthenticationAres => {
                Ok(Self::ExternalAuthenticationAres)
            }
            grpc_api_types::payments::WebhookEventType::FrmApproved => Ok(Self::FrmApproved),
            grpc_api_types::payments::WebhookEventType::FrmRejected => Ok(Self::FrmRejected),
            grpc_api_types::payments::WebhookEventType::PayoutSuccess => Ok(Self::PayoutSuccess),
            grpc_api_types::payments::WebhookEventType::PayoutFailure => Ok(Self::PayoutFailure),
            grpc_api_types::payments::WebhookEventType::PayoutProcessing => {
                Ok(Self::PayoutProcessing)
            }
            grpc_api_types::payments::WebhookEventType::PayoutCancelled => {
                Ok(Self::PayoutCancelled)
            }
            grpc_api_types::payments::WebhookEventType::PayoutCreated => Ok(Self::PayoutCreated),
            grpc_api_types::payments::WebhookEventType::PayoutExpired => Ok(Self::PayoutExpired),
            grpc_api_types::payments::WebhookEventType::PayoutReversed => Ok(Self::PayoutReversed),
            grpc_api_types::payments::WebhookEventType::RecoveryPaymentFailure => {
                Ok(Self::RecoveryPaymentFailure)
            }
            grpc_api_types::payments::WebhookEventType::RecoveryPaymentSuccess => {
                Ok(Self::RecoveryPaymentSuccess)
            }
            grpc_api_types::payments::WebhookEventType::RecoveryPaymentPending => {
                Ok(Self::RecoveryPaymentPending)
            }
            grpc_api_types::payments::WebhookEventType::RecoveryInvoiceCancel => {
                Ok(Self::RecoveryInvoiceCancel)
            }
            grpc_api_types::payments::WebhookEventType::Unspecified => {
                Ok(Self::IncomingWebhookEventUnspecified)
            }
        }
    }
}

impl ForeignTryFrom<EventType> for grpc_api_types::payments::WebhookEventType {
    type Error = IntegrationError;

    fn foreign_try_from(value: EventType) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            EventType::PaymentIntentFailure => Ok(Self::PaymentIntentFailure),
            EventType::PaymentIntentSuccess => Ok(Self::PaymentIntentSuccess),
            EventType::PaymentIntentProcessing => Ok(Self::PaymentIntentProcessing),
            EventType::PaymentIntentPartiallyFunded => Ok(Self::PaymentIntentPartiallyFunded),
            EventType::PaymentIntentCancelled => Ok(Self::PaymentIntentCancelled),
            EventType::PaymentIntentCancelFailure => Ok(Self::PaymentIntentCancelFailure),
            EventType::PaymentIntentAuthorizationSuccess => {
                Ok(Self::PaymentIntentAuthorizationSuccess)
            }
            EventType::PaymentIntentAuthorizationFailure => {
                Ok(Self::PaymentIntentAuthorizationFailure)
            }
            EventType::PaymentIntentCaptureSuccess => Ok(Self::PaymentIntentCaptureSuccess),
            EventType::PaymentIntentCaptureFailure => Ok(Self::PaymentIntentCaptureFailure),
            EventType::PaymentIntentExpired => Ok(Self::PaymentIntentExpired),
            EventType::PaymentActionRequired => Ok(Self::PaymentActionRequired),
            EventType::SourceChargeable => Ok(Self::SourceChargeable),
            EventType::SourceTransactionCreated => Ok(Self::SourceTransactionCreated),
            EventType::RefundFailure => Ok(Self::WebhookRefundFailure),
            EventType::RefundSuccess => Ok(Self::WebhookRefundSuccess),
            EventType::DisputeOpened => Ok(Self::WebhookDisputeOpened),
            EventType::DisputeExpired => Ok(Self::WebhookDisputeExpired),
            EventType::DisputeAccepted => Ok(Self::WebhookDisputeAccepted),
            EventType::DisputeCancelled => Ok(Self::WebhookDisputeCancelled),
            EventType::DisputeChallenged => Ok(Self::WebhookDisputeChallenged),
            EventType::DisputeWon => Ok(Self::WebhookDisputeWon),
            EventType::DisputeLost => Ok(Self::WebhookDisputeLost),
            EventType::MandateActive => Ok(Self::MandateActive),
            EventType::MandateFailed => Ok(Self::MandateFailed),
            EventType::MandateRevoked => Ok(Self::MandateRevoked),
            EventType::EndpointVerification => Ok(Self::EndpointVerification),
            EventType::ExternalAuthenticationAres => Ok(Self::ExternalAuthenticationAres),
            EventType::FrmApproved => Ok(Self::FrmApproved),
            EventType::FrmRejected => Ok(Self::FrmRejected),
            EventType::PayoutSuccess => Ok(Self::PayoutSuccess),
            EventType::PayoutFailure => Ok(Self::PayoutFailure),
            EventType::PayoutProcessing => Ok(Self::PayoutProcessing),
            EventType::PayoutCancelled => Ok(Self::PayoutCancelled),
            EventType::PayoutCreated => Ok(Self::PayoutCreated),
            EventType::PayoutExpired => Ok(Self::PayoutExpired),
            EventType::PayoutReversed => Ok(Self::PayoutReversed),
            EventType::RecoveryPaymentFailure => Ok(Self::RecoveryPaymentFailure),
            EventType::RecoveryPaymentSuccess => Ok(Self::RecoveryPaymentSuccess),
            EventType::RecoveryPaymentPending => Ok(Self::RecoveryPaymentPending),
            EventType::RecoveryInvoiceCancel => Ok(Self::RecoveryInvoiceCancel),
            EventType::IncomingWebhookEventUnspecified => Ok(Self::Unspecified),

            // Legacy broad categories (for backward compatibility)
            EventType::Payment => Ok(Self::PaymentIntentSuccess), // Map broad Payment to PaymentIntentSuccess
            EventType::Refund => Ok(Self::WebhookRefundSuccess), // Map broad Refund to WebhookRefundSuccess
            EventType::Dispute => Ok(Self::WebhookDisputeOpened), // Map broad Dispute to WebhookDisputeOpened
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::HttpMethod> for HttpMethod {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::HttpMethod,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match value {
            grpc_api_types::payments::HttpMethod::Unspecified => Ok(Self::Get), // Default
            grpc_api_types::payments::HttpMethod::Get => Ok(Self::Get),
            grpc_api_types::payments::HttpMethod::Post => Ok(Self::Post),
            grpc_api_types::payments::HttpMethod::Put => Ok(Self::Put),
            grpc_api_types::payments::HttpMethod::Delete => Ok(Self::Delete),
        }
    }
}

impl ForeignTryFrom<grpc_api_types::payments::RequestDetails> for RequestDetails {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::RequestDetails,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        let method = HttpMethod::foreign_try_from(value.method())?;

        Ok(Self {
            method,
            uri: value.uri,
            headers: value.headers,
            body: value.body,
            query_params: value.query_params,
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::WebhookSecrets> for ConnectorWebhookSecrets {
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::WebhookSecrets,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            secret: value.secret.into(),
            additional_secret: value.additional_secret.map(Secret::new),
        })
    }
}

impl ForeignTryFrom<grpc_api_types::payments::RedirectResponseSecrets>
    for ConnectorRedirectResponseSecrets
{
    type Error = IntegrationError;

    fn foreign_try_from(
        value: grpc_api_types::payments::RedirectResponseSecrets,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        Ok(Self {
            secret: value.secret.into(),
            additional_secret: value.additional_secret.map(Secret::new),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct RefundsData {
    pub refund_id: String,
    pub connector_transaction_id: String,
    pub connector_refund_id: Option<String>,
    pub customer_id: Option<String>,
    pub currency: Currency,
    pub payment_amount: i64,
    pub reason: Option<String>,
    pub webhook_url: Option<String>,
    pub refund_amount: i64,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub refund_connector_metadata: Option<SecretSerdeValue>,
    pub minor_payment_amount: MinorUnit,
    pub minor_refund_amount: MinorUnit,
    pub refund_status: common_enums::RefundStatus,
    pub merchant_account_id: Option<String>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub integrity_object: Option<RefundIntegrityObject>,
    pub browser_info: Option<BrowserInformation>,
    /// Charges associated with the payment
    pub split_refunds: Option<SplitRefundsRequest>,
}

impl RefundsData {
    #[track_caller]
    pub fn get_connector_refund_id(&self) -> Result<String, Error> {
        self.connector_refund_id
            .clone()
            .get_required_value("connector_refund_id")
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })
    }
    pub fn get_webhook_url(&self) -> Result<String, Error> {
        self.webhook_url
            .clone()
            .ok_or_else(missing_field_err("webhook_url"))
    }
    pub fn get_connector_feature_data(&self) -> Result<SecretSerdeValue, Error> {
        self.connector_feature_data
            .clone()
            .ok_or_else(missing_field_err("connector_feature_data"))
    }
    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }
    pub fn get_ip_address_as_optional(&self) -> Option<Secret<String, IpAddress>> {
        self.browser_info.clone().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        })
    }

    pub fn get_ip_address(&self) -> Result<Secret<String, IpAddress>, Error> {
        self.get_ip_address_as_optional()
            .ok_or_else(missing_field_err("browser_info.ip_address"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct MultipleCaptureRequestData {
    pub capture_sequence: i64,
    pub capture_reference: String,
}

#[derive(Debug, Default, Clone)]
pub struct PaymentsCaptureData {
    pub amount_to_capture: i64,
    pub minor_amount_to_capture: MinorUnit,
    pub currency: Currency,
    pub connector_transaction_id: ResponseId,
    pub multiple_capture_data: Option<MultipleCaptureRequestData>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub integrity_object: Option<CaptureIntegrityObject>,
    pub browser_info: Option<BrowserInformation>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub metadata: Option<SecretSerdeValue>,
    pub merchant_order_id: Option<String>,
}

impl PaymentsCaptureData {
    pub fn is_multiple_capture(&self) -> bool {
        self.multiple_capture_data.is_some()
    }
    pub fn get_connector_transaction_id(&self) -> CustomResult<String, IntegrationError> {
        match self.connector_transaction_id.clone() {
            ResponseId::ConnectorTransactionId(txn_id) => Ok(txn_id),
            _ => Err(errors::ValidationError::IncorrectValueProvided {
                field_name: "connector_transaction_id",
            })
            .attach_printable("Expected connector transaction ID not found")
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?,
        }
    }
    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }
    pub fn get_ip_address_as_optional(&self) -> Option<Secret<String, IpAddress>> {
        self.browser_info.clone().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        })
    }

    pub fn get_ip_address(&self) -> Result<Secret<String, IpAddress>, Error> {
        self.get_ip_address_as_optional()
            .ok_or_else(missing_field_err("browser_info.ip_address"))
    }
}

#[derive(Debug, Clone)]
pub struct SetupMandateRequestData<T: PaymentMethodDataTypes> {
    pub currency: Currency,
    pub payment_method_data: PaymentMethodData<T>,
    pub amount: Option<i64>,
    pub confirm: bool,
    pub billing_descriptor: Option<BillingDescriptor>,
    pub customer_acceptance: Option<CustomerAcceptance>,
    pub mandate_id: Option<MandateIds>,
    pub setup_future_usage: Option<common_enums::FutureUsage>,
    pub off_session: Option<bool>,
    pub setup_mandate_details: Option<MandateData>,
    pub router_return_url: Option<String>,
    pub webhook_url: Option<String>,
    pub browser_info: Option<BrowserInformation>,
    pub email: Option<Email>,
    pub customer_name: Option<String>,
    pub return_url: Option<String>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub request_incremental_authorization: bool,
    pub metadata: Option<SecretSerdeValue>,
    pub complete_authorize_url: Option<String>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub merchant_order_id: Option<String>,
    pub minor_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    pub customer_id: Option<CustomerId>,
    pub integrity_object: Option<SetupMandateIntegrityObject>,
    pub payment_channel: Option<PaymentChannel>,
    pub enable_partial_authorization: Option<bool>,
    pub locale: Option<String>,
    pub connector_testing_data: Option<SecretSerdeValue>,
}

impl<T: PaymentMethodDataTypes> SetupMandateRequestData<T> {
    pub fn get_connector_testing_data(&self) -> Option<SecretSerdeValue> {
        self.connector_testing_data.clone()
    }

    pub fn get_browser_info(&self) -> Result<BrowserInformation, Error> {
        self.browser_info
            .clone()
            .ok_or_else(missing_field_err("browser_info"))
    }
    pub fn get_email(&self) -> Result<Email, Error> {
        self.email.clone().ok_or_else(missing_field_err("email"))
    }
    pub fn is_card(&self) -> bool {
        matches!(self.payment_method_data, PaymentMethodData::Card(_))
    }
    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }
    pub fn get_webhook_url(&self) -> Result<String, Error> {
        self.webhook_url
            .clone()
            .ok_or_else(missing_field_err("webhook_url"))
    }
    pub fn get_router_return_url(&self) -> Result<String, Error> {
        self.router_return_url
            .clone()
            .ok_or_else(missing_field_err("return_url"))
    }
    pub fn get_ip_address_as_optional(&self) -> Option<Secret<String, IpAddress>> {
        self.browser_info.clone().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        })
    }

    pub fn get_ip_address(&self) -> Result<Secret<String, IpAddress>, Error> {
        self.get_ip_address_as_optional()
            .ok_or_else(missing_field_err("browser_info.ip_address"))
    }
}

#[derive(Debug, Clone)]
pub struct RepeatPaymentData<T: PaymentMethodDataTypes> {
    pub mandate_reference: MandateReferenceId,
    pub amount: i64,
    pub minor_amount: MinorUnit,
    pub currency: Currency,
    pub merchant_order_id: Option<String>,
    pub metadata: Option<SecretSerdeValue>,
    pub webhook_url: Option<String>,
    pub integrity_object: Option<RepeatPaymentIntegrityObject>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub browser_info: Option<BrowserInformation>,
    pub email: Option<Email>,
    pub payment_method_type: Option<PaymentMethodType>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub off_session: Option<bool>,
    pub router_return_url: Option<String>,
    pub split_payments: Option<SplitPaymentsRequest>,
    pub recurring_mandate_payment_data: Option<router_data::RecurringMandatePaymentData>,
    pub shipping_cost: Option<MinorUnit>,
    pub mit_category: Option<common_enums::MitCategory>,
    pub enable_partial_authorization: Option<bool>,
    pub billing_descriptor: Option<BillingDescriptor>,
    pub payment_method_data: PaymentMethodData<T>,
    pub authentication_data: Option<router_request_types::AuthenticationData>,
    pub locale: Option<String>,
    pub connector_testing_data: Option<SecretSerdeValue>,
    pub merchant_account_id: Option<Secret<String>>,
    pub merchant_configured_currency: Option<Currency>,
}

impl<T: PaymentMethodDataTypes> RepeatPaymentData<T> {
    pub fn get_connector_testing_data(&self) -> Option<SecretSerdeValue> {
        self.connector_testing_data.clone()
    }

    pub fn get_mandate_reference(&self) -> &MandateReferenceId {
        &self.mandate_reference
    }
    /// Returns true if payment should be automatically captured, false for manual capture.
    ///
    /// Maps capture methods to boolean intent:
    /// - Automatic/SequentialAutomatic/None → true (auto capture)
    /// - Manual/ManualMultiple/Scheduled → false (manual capture)
    ///
    /// Note: This is a pure getter, not a validation. Connectors that don't support
    /// specific capture methods should validate explicitly during request building.
    pub fn is_auto_capture(&self) -> bool {
        !matches!(
            self.capture_method,
            Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled)
        )
    }
    pub fn get_optional_language_from_browser_info(&self) -> Option<String> {
        self.browser_info
            .clone()
            .and_then(|browser_info| browser_info.language)
    }
    pub fn get_webhook_url(&self) -> Result<String, Error> {
        self.webhook_url
            .clone()
            .ok_or_else(missing_field_err("webhook_url"))
    }
    pub fn get_router_return_url(&self) -> Result<String, Error> {
        self.router_return_url
            .clone()
            .ok_or_else(missing_field_err("return_url"))
    }
    pub fn get_email(&self) -> Result<Email, Error> {
        self.email.clone().ok_or_else(missing_field_err("email"))
    }
    pub fn get_recurring_mandate_payment_data(
        &self,
    ) -> Result<router_data::RecurringMandatePaymentData, Error> {
        self.recurring_mandate_payment_data
            .to_owned()
            .ok_or_else(missing_field_err("recurring_mandate_payment_data"))
    }
    pub fn get_ip_address_as_optional(&self) -> Option<Secret<String, IpAddress>> {
        self.browser_info.clone().and_then(|browser_info| {
            browser_info
                .ip_address
                .map(|ip| Secret::new(ip.to_string()))
        })
    }
    pub fn connector_mandate_id(&self) -> Option<String> {
        match &self.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ids) => {
                connector_mandate_ids.get_connector_mandate_id()
            }
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => None,
        }
    }
    pub fn get_optional_email(&self) -> Option<Email> {
        self.email.clone()
    }

    pub fn get_network_mandate_id(&self) -> Option<String> {
        match &self.mandate_reference {
            MandateReferenceId::NetworkMandateId(network_mandate_id) => {
                Some(network_mandate_id.to_string())
            }
            MandateReferenceId::ConnectorMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AcceptDisputeData {
    pub connector_dispute_id: String,
    pub integrity_object: Option<AcceptDisputeIntegrityObject>,
}

#[derive(Debug, Clone)]
pub struct DisputeFlowData {
    pub dispute_id: Option<String>,
    pub connector_dispute_id: String,
    pub connectors: Connectors,
    pub defense_reason_code: Option<String>,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
}

impl RawConnectorRequestResponse for DisputeFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }
}

impl ConnectorResponseHeaders for DisputeFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct VerifyWebhookSourceFlowData {
    pub connectors: Connectors,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
}

impl RawConnectorRequestResponse for VerifyWebhookSourceFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }
}

impl ConnectorResponseHeaders for VerifyWebhookSourceFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct DisputeResponseData {
    pub connector_dispute_id: String,
    pub dispute_status: DisputeStatus,
    pub connector_dispute_status: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone, Default)]
pub struct SubmitEvidenceData {
    pub dispute_id: Option<String>,
    pub connector_dispute_id: String,
    pub integrity_object: Option<SubmitEvidenceIntegrityObject>,
    pub access_activity_log: Option<String>,
    pub billing_address: Option<String>,

    pub cancellation_policy: Option<Vec<u8>>,
    pub cancellation_policy_file_type: Option<String>,
    pub cancellation_policy_provider_file_id: Option<String>,
    pub cancellation_policy_disclosure: Option<String>,
    pub cancellation_rebuttal: Option<String>,

    pub customer_communication: Option<Vec<u8>>,
    pub customer_communication_file_type: Option<String>,
    pub customer_communication_provider_file_id: Option<String>,
    pub customer_email_address: Option<String>,
    pub customer_name: Option<String>,
    pub customer_purchase_ip: Option<String>,

    pub customer_signature: Option<Vec<u8>>,
    pub customer_signature_file_type: Option<String>,
    pub customer_signature_provider_file_id: Option<String>,

    pub product_description: Option<String>,

    pub receipt: Option<Vec<u8>>,
    pub receipt_file_type: Option<String>,
    pub receipt_provider_file_id: Option<String>,

    pub refund_policy: Option<Vec<u8>>,
    pub refund_policy_file_type: Option<String>,
    pub refund_policy_provider_file_id: Option<String>,
    pub refund_policy_disclosure: Option<String>,
    pub refund_refusal_explanation: Option<String>,

    pub service_date: Option<String>,
    pub service_documentation: Option<Vec<u8>>,
    pub service_documentation_file_type: Option<String>,
    pub service_documentation_provider_file_id: Option<String>,

    pub shipping_address: Option<String>,
    pub shipping_carrier: Option<String>,
    pub shipping_date: Option<String>,
    pub shipping_documentation: Option<Vec<u8>>,
    pub shipping_documentation_file_type: Option<String>,
    pub shipping_documentation_provider_file_id: Option<String>,
    pub shipping_tracking_number: Option<String>,

    pub invoice_showing_distinct_transactions: Option<Vec<u8>>,
    pub invoice_showing_distinct_transactions_file_type: Option<String>,
    pub invoice_showing_distinct_transactions_provider_file_id: Option<String>,

    pub recurring_transaction_agreement: Option<Vec<u8>>,
    pub recurring_transaction_agreement_file_type: Option<String>,
    pub recurring_transaction_agreement_provider_file_id: Option<String>,

    pub uncategorized_file: Option<Vec<u8>>,
    pub uncategorized_file_type: Option<String>,
    pub uncategorized_file_provider_file_id: Option<String>,
    pub uncategorized_text: Option<String>,
}

/// The trait that provides specifications about the connector
pub trait ConnectorSpecifications {
    /// Details related to payment method supported by the connector
    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        None
    }

    /// Supported webhooks flows
    fn get_supported_webhook_flows(&self) -> Option<&'static [EventClass]> {
        None
    }

    /// About the connector
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        None
    }
}

#[macro_export]
macro_rules! capture_method_not_supported {
    ($connector:expr, $capture_method:expr) => {
        Err(errors::IntegrationError::NotSupported {
            message: format!("{} for selected payment method", $capture_method),
            connector: $connector,
            context: Default::default(),
        }
        .into())
    };
    ($connector:expr, $capture_method:expr, $payment_method_type:expr) => {
        Err(errors::IntegrationError::NotSupported {
            message: format!("{} for {}", $capture_method, $payment_method_type),
            connector: $connector,
            context: Default::default(),
        }
        .into())
    };
}

#[macro_export]
macro_rules! payment_method_not_supported {
    ($connector:expr, $payment_method:expr, $payment_method_type:expr) => {
        Err(errors::IntegrationError::NotSupported {
            message: format!(
                "Payment method {} with type {} is not supported",
                $payment_method, $payment_method_type
            ),
            connector: $connector,
            context: Default::default(),
        }
        .into())
    };
}

impl<T: PaymentMethodDataTypes> From<PaymentMethodData<T>> for PaymentMethodDataType {
    fn from(pm_data: PaymentMethodData<T>) -> Self {
        match pm_data {
            PaymentMethodData::Card(_) => Self::Card,
            PaymentMethodData::CardRedirect(card_redirect_data) => match card_redirect_data {
                payment_method_data::CardRedirectData::Knet {} => Self::Knet,
                payment_method_data::CardRedirectData::Benefit {} => Self::Benefit,
                payment_method_data::CardRedirectData::MomoAtm {} => Self::MomoAtm,
                payment_method_data::CardRedirectData::CardRedirect {} => Self::CardRedirect,
            },
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                payment_method_data::WalletData::BluecodeRedirect { .. } => Self::Bluecode,
                payment_method_data::WalletData::AliPayQr(_) => Self::AliPayQr,
                payment_method_data::WalletData::AliPayRedirect(_) => Self::AliPayRedirect,
                payment_method_data::WalletData::AliPayHkRedirect(_) => Self::AliPayHkRedirect,
                payment_method_data::WalletData::MomoRedirect(_) => Self::MomoRedirect,
                payment_method_data::WalletData::KakaoPayRedirect(_) => Self::KakaoPayRedirect,
                payment_method_data::WalletData::GoPayRedirect(_) => Self::GoPayRedirect,
                payment_method_data::WalletData::GcashRedirect(_) => Self::GcashRedirect,
                payment_method_data::WalletData::ApplePay(_) => Self::ApplePay,
                payment_method_data::WalletData::ApplePayRedirect(_) => Self::ApplePayRedirect,
                payment_method_data::WalletData::ApplePayThirdPartySdk(_) => {
                    Self::ApplePayThirdPartySdk
                }
                payment_method_data::WalletData::DanaRedirect {} => Self::DanaRedirect,
                payment_method_data::WalletData::GooglePay(_) => Self::GooglePay,
                payment_method_data::WalletData::GooglePayRedirect(_) => Self::GooglePayRedirect,
                payment_method_data::WalletData::GooglePayThirdPartySdk(_) => {
                    Self::GooglePayThirdPartySdk
                }
                payment_method_data::WalletData::MbWayRedirect(_) => Self::MbWayRedirect,
                payment_method_data::WalletData::MobilePayRedirect(_) => Self::MobilePayRedirect,
                payment_method_data::WalletData::PaypalRedirect(_) => Self::PaypalRedirect,
                payment_method_data::WalletData::PaypalSdk(_) => Self::PaypalSdk,
                payment_method_data::WalletData::SamsungPay(_) => Self::SamsungPay,
                payment_method_data::WalletData::TwintRedirect {} => Self::TwintRedirect,
                payment_method_data::WalletData::VippsRedirect {} => Self::VippsRedirect,
                payment_method_data::WalletData::TouchNGoRedirect(_) => Self::TouchNGoRedirect,
                payment_method_data::WalletData::WeChatPayRedirect(_) => Self::WeChatPayRedirect,
                payment_method_data::WalletData::WeChatPayQr(_) => Self::WeChatPayQr,
                payment_method_data::WalletData::CashappQr(_) => Self::CashappQr,
                payment_method_data::WalletData::SwishQr(_) => Self::SwishQr,
                payment_method_data::WalletData::Mifinity(_) => Self::Mifinity,
                payment_method_data::WalletData::AmazonPayRedirect(_) => Self::AmazonPayRedirect,
                payment_method_data::WalletData::Paze(_) => Self::Paze,
                payment_method_data::WalletData::RevolutPay(_) => Self::RevolutPay,
                payment_method_data::WalletData::MbWay(_) => Self::MbWay,
                payment_method_data::WalletData::Satispay(_) => Self::Satispay,
                payment_method_data::WalletData::Wero(_) => Self::Wero,
                payment_method_data::WalletData::LazyPayRedirect(_) => Self::LazyPayRedirect,
                payment_method_data::WalletData::PhonePeRedirect(_) => Self::PhonePeRedirect,
                payment_method_data::WalletData::BillDeskRedirect(_) => Self::BillDeskRedirect,
                payment_method_data::WalletData::CashfreeRedirect(_) => Self::CashfreeRedirect,
                payment_method_data::WalletData::PayURedirect(_) => Self::PayURedirect,
                payment_method_data::WalletData::EaseBuzzRedirect(_) => Self::EaseBuzzRedirect,
            },
            PaymentMethodData::PayLater(pay_later_data) => match pay_later_data {
                payment_method_data::PayLaterData::KlarnaRedirect { .. } => Self::KlarnaRedirect,
                payment_method_data::PayLaterData::KlarnaSdk { .. } => Self::KlarnaSdk,
                payment_method_data::PayLaterData::AffirmRedirect {} => Self::AffirmRedirect,
                payment_method_data::PayLaterData::AfterpayClearpayRedirect { .. } => {
                    Self::AfterpayClearpayRedirect
                }
                payment_method_data::PayLaterData::PayBrightRedirect {} => Self::PayBrightRedirect,
                payment_method_data::PayLaterData::WalleyRedirect {} => Self::WalleyRedirect,
                payment_method_data::PayLaterData::AlmaRedirect {} => Self::AlmaRedirect,
                payment_method_data::PayLaterData::AtomeRedirect {} => Self::AtomeRedirect,
            },
            PaymentMethodData::BankRedirect(bank_redirect_data) => match bank_redirect_data {
                payment_method_data::BankRedirectData::BancontactCard { .. } => {
                    Self::BancontactCard
                }
                payment_method_data::BankRedirectData::Bizum {} => Self::Bizum,
                payment_method_data::BankRedirectData::Blik { .. } => Self::Blik,
                payment_method_data::BankRedirectData::Eps { .. } => Self::Eps,
                payment_method_data::BankRedirectData::Giropay { .. } => Self::Giropay,
                payment_method_data::BankRedirectData::Ideal { .. } => Self::Ideal,
                payment_method_data::BankRedirectData::Interac { .. } => Self::Interac,
                payment_method_data::BankRedirectData::OnlineBankingCzechRepublic { .. } => {
                    Self::OnlineBankingCzechRepublic
                }
                payment_method_data::BankRedirectData::OnlineBankingFinland { .. } => {
                    Self::OnlineBankingFinland
                }
                payment_method_data::BankRedirectData::OnlineBankingPoland { .. } => {
                    Self::OnlineBankingPoland
                }
                payment_method_data::BankRedirectData::OnlineBankingSlovakia { .. } => {
                    Self::OnlineBankingSlovakia
                }
                payment_method_data::BankRedirectData::OpenBankingUk { .. } => Self::OpenBankingUk,
                payment_method_data::BankRedirectData::Przelewy24 { .. } => Self::Przelewy24,
                payment_method_data::BankRedirectData::Sofort { .. } => Self::Sofort,
                payment_method_data::BankRedirectData::Trustly { .. } => Self::Trustly,
                payment_method_data::BankRedirectData::OnlineBankingFpx { .. } => {
                    Self::OnlineBankingFpx
                }
                payment_method_data::BankRedirectData::OnlineBankingThailand { .. } => {
                    Self::OnlineBankingThailand
                }
                payment_method_data::BankRedirectData::LocalBankRedirect {} => {
                    Self::LocalBankRedirect
                }
                payment_method_data::BankRedirectData::Eft { .. } => Self::Eft,
                payment_method_data::BankRedirectData::OpenBanking {} => Self::OpenBanking,
                payment_method_data::BankRedirectData::Netbanking { .. } => Self::Netbanking,
            },
            PaymentMethodData::BankDebit(bank_debit_data) => match bank_debit_data {
                payment_method_data::BankDebitData::AchBankDebit { .. } => Self::AchBankDebit,
                payment_method_data::BankDebitData::SepaBankDebit { .. } => Self::SepaBankDebit,
                payment_method_data::BankDebitData::BecsBankDebit { .. } => Self::BecsBankDebit,
                payment_method_data::BankDebitData::BacsBankDebit { .. } => Self::BacsBankDebit,
                payment_method_data::BankDebitData::SepaGuaranteedBankDebit { .. } => {
                    Self::SepaGuaranteedBankDebit
                }
            },
            PaymentMethodData::BankTransfer(bank_transfer_data) => match *bank_transfer_data {
                payment_method_data::BankTransferData::AchBankTransfer { .. } => {
                    Self::AchBankTransfer
                }
                payment_method_data::BankTransferData::SepaBankTransfer { .. } => {
                    Self::SepaBankTransfer
                }
                payment_method_data::BankTransferData::BacsBankTransfer { .. } => {
                    Self::BacsBankTransfer
                }
                payment_method_data::BankTransferData::MultibancoBankTransfer { .. } => {
                    Self::MultibancoBankTransfer
                }
                payment_method_data::BankTransferData::PermataBankTransfer { .. } => {
                    Self::PermataBankTransfer
                }
                payment_method_data::BankTransferData::BcaBankTransfer { .. } => {
                    Self::BcaBankTransfer
                }
                payment_method_data::BankTransferData::BniVaBankTransfer { .. } => {
                    Self::BniVaBankTransfer
                }
                payment_method_data::BankTransferData::BriVaBankTransfer { .. } => {
                    Self::BriVaBankTransfer
                }
                payment_method_data::BankTransferData::CimbVaBankTransfer { .. } => {
                    Self::CimbVaBankTransfer
                }
                payment_method_data::BankTransferData::DanamonVaBankTransfer { .. } => {
                    Self::DanamonVaBankTransfer
                }
                payment_method_data::BankTransferData::MandiriVaBankTransfer { .. } => {
                    Self::MandiriVaBankTransfer
                }
                payment_method_data::BankTransferData::Pix { .. } => Self::Pix,
                payment_method_data::BankTransferData::Pse {} => Self::Pse,
                payment_method_data::BankTransferData::LocalBankTransfer { .. } => {
                    Self::LocalBankTransfer
                }
                payment_method_data::BankTransferData::InstantBankTransfer { .. } => {
                    Self::InstantBankTransfer
                }
                payment_method_data::BankTransferData::InstantBankTransferFinland { .. } => {
                    Self::InstantBankTransferFinland
                }
                payment_method_data::BankTransferData::InstantBankTransferPoland { .. } => {
                    Self::InstantBankTransferPoland
                }
                payment_method_data::BankTransferData::IndonesianBankTransfer { .. } => {
                    Self::IndonesianBankTransfer
                }
            },
            PaymentMethodData::Crypto(_) => Self::Crypto,
            PaymentMethodData::MandatePayment => Self::MandatePayment,
            PaymentMethodData::Reward => Self::Reward,
            PaymentMethodData::Upi(_) => Self::Upi,
            PaymentMethodData::Voucher(voucher_data) => match voucher_data {
                payment_method_data::VoucherData::Boleto(_) => Self::Boleto,
                payment_method_data::VoucherData::Efecty => Self::Efecty,
                payment_method_data::VoucherData::PagoEfectivo => Self::PagoEfectivo,
                payment_method_data::VoucherData::RedCompra => Self::RedCompra,
                payment_method_data::VoucherData::RedPagos => Self::RedPagos,
                payment_method_data::VoucherData::Alfamart(_) => Self::Alfamart,
                payment_method_data::VoucherData::Indomaret(_) => Self::Indomaret,
                payment_method_data::VoucherData::Oxxo => Self::Oxxo,
                payment_method_data::VoucherData::SevenEleven(_) => Self::SevenEleven,
                payment_method_data::VoucherData::Lawson(_) => Self::Lawson,
                payment_method_data::VoucherData::MiniStop(_) => Self::MiniStop,
                payment_method_data::VoucherData::FamilyMart(_) => Self::FamilyMart,
                payment_method_data::VoucherData::Seicomart(_) => Self::Seicomart,
                payment_method_data::VoucherData::PayEasy(_) => Self::PayEasy,
            },
            PaymentMethodData::RealTimePayment(real_time_payment_data) => {
                match *real_time_payment_data {
                    payment_method_data::RealTimePaymentData::DuitNow {} => Self::DuitNow,
                    payment_method_data::RealTimePaymentData::Fps {} => Self::Fps,
                    payment_method_data::RealTimePaymentData::PromptPay {} => Self::PromptPay,
                    payment_method_data::RealTimePaymentData::VietQr {} => Self::VietQr,
                }
            }
            PaymentMethodData::GiftCard(gift_card_data) => match *gift_card_data {
                payment_method_data::GiftCardData::Givex(_) => Self::Givex,
                payment_method_data::GiftCardData::PaySafeCard {} => Self::PaySafeCar,
            },
            PaymentMethodData::PaymentMethodToken(_) => Self::PaymentMethodToken,
            PaymentMethodData::OpenBanking(data) => match data {
                payment_method_data::OpenBankingData::OpenBankingPIS {} => Self::OpenBanking,
            },
            PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Self::CardDetailsForNetworkTransactionId
            }
            PaymentMethodData::NetworkToken(_) => Self::NetworkToken,
            PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
                Self::NetworkToken
            }
            PaymentMethodData::MobilePayment(mobile_payment_data) => match mobile_payment_data {
                payment_method_data::MobilePaymentData::DirectCarrierBilling { .. } => {
                    Self::DirectCarrierBilling
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisputeDefendData {
    pub dispute_id: String,
    pub connector_dispute_id: String,
    pub defense_reason_code: String,
    pub integrity_object: Option<DefendDisputeIntegrityObject>,
}

pub trait SupportedPaymentMethodsExt {
    fn add(
        &mut self,
        payment_method: PaymentMethod,
        payment_method_type: PaymentMethodType,
        payment_method_details: PaymentMethodDetails,
    );
}

impl SupportedPaymentMethodsExt for SupportedPaymentMethods {
    fn add(
        &mut self,
        payment_method: PaymentMethod,
        payment_method_type: PaymentMethodType,
        payment_method_details: PaymentMethodDetails,
    ) {
        if let Some(payment_method_data) = self.get_mut(&payment_method) {
            payment_method_data.insert(payment_method_type, payment_method_details);
        } else {
            let mut payment_method_type_metadata = PaymentMethodTypeMetadata::new();
            payment_method_type_metadata.insert(payment_method_type, payment_method_details);

            self.insert(payment_method, payment_method_type_metadata);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
/// Fee information for Split Payments to be charged on the payment being collected
pub enum SplitPaymentsRequest {
    /// StripeSplitPayment
    StripeSplitPayment(StripeSplitPaymentRequest),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// Fee information for Split Payments to be charged on the payment being collected for Stripe
pub struct StripeSplitPaymentRequest {
    /// Stripe's charge type
    pub charge_type: common_enums::PaymentChargeType,

    /// Platform fees to be collected on the payment
    pub application_fees: Option<MinorUnit>,

    /// Identifier for the reseller's account where the funds were transferred
    pub transfer_account_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ConnectorChargeResponseData {
    /// StripeChargeResponseData
    StripeSplitPayment(StripeChargeResponseData),
}

/// Fee information to be charged on the payment being collected via Stripe
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StripeChargeResponseData {
    /// Identifier for charge created for the payment
    pub charge_id: Option<String>,

    /// Type of charge (connector specific)
    pub charge_type: common_enums::PaymentChargeType,

    /// Platform fees collected on the payment
    pub application_fees: Option<MinorUnit>,

    /// Identifier for the reseller's account where the funds were transferred
    pub transfer_account_id: String,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub enum SplitRefundsRequest {
    StripeSplitRefund(StripeSplitRefund),
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct StripeSplitRefund {
    pub charge_id: String,
    pub transfer_account_id: String,
    pub charge_type: common_enums::PaymentChargeType,
    pub options: ChargeRefundsOptions,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ChargeRefundsOptions {
    Destination(DestinationChargeRefund),
    Direct(DirectChargeRefund),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DirectChargeRefund {
    pub revert_platform_fee: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DestinationChargeRefund {
    pub revert_platform_fee: bool,
    pub revert_transfer: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct RecurringMandatePaymentData {
    pub payment_method_type: Option<PaymentMethodType>, //required for making recurring payment using saved payment method through stripe
    pub original_payment_authorized_amount: Option<MinorUnit>,
    pub original_payment_authorized_currency: Option<Currency>,
    pub mandate_metadata: Option<SecretSerdeValue>,
}

pub trait RecurringMandateData {
    fn get_original_payment_amount(&self) -> Result<MinorUnit, Error>;
    fn get_original_payment_currency(&self) -> Result<Currency, Error>;
}

impl RecurringMandateData for RecurringMandatePaymentData {
    fn get_original_payment_amount(&self) -> Result<MinorUnit, Error> {
        self.original_payment_authorized_amount
            .ok_or_else(missing_field_err("original_payment_authorized_amount"))
    }
    fn get_original_payment_currency(&self) -> Result<Currency, Error> {
        self.original_payment_authorized_currency
            .ok_or_else(missing_field_err("original_payment_authorized_currency"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct L2L3Data {
    pub order_info: Option<OrderInfo>,
    pub tax_info: Option<TaxInfo>,
    pub customer_info: Option<CustomerInfo>,
    pub shipping_details: Option<AddressDetails>,
    pub billing_details: Option<AddressDetails>,
}
#[derive(Debug, Clone)]
pub struct OrderInfo {
    pub order_date: Option<time::PrimitiveDateTime>,
    pub order_details: Option<Vec<payment_address::OrderDetailsWithAmount>>,
    pub merchant_order_reference_id: Option<String>,
    pub discount_amount: Option<MinorUnit>,
    pub shipping_cost: Option<MinorUnit>,
    pub duty_amount: Option<MinorUnit>,
}

#[derive(Debug, Clone)]
pub struct TaxInfo {
    pub tax_status: Option<common_enums::TaxStatus>,
    pub customer_tax_registration_id: Option<Secret<String>>,
    pub merchant_tax_registration_id: Option<Secret<String>>,
    pub shipping_amount_tax: Option<MinorUnit>,
    pub order_tax_amount: Option<MinorUnit>,
}

#[derive(Debug, Clone)]
pub struct CustomerInfo {
    pub customer_id: Option<CustomerId>,
    pub customer_email: Option<common_utils::pii::Email>,
    pub customer_name: Option<Secret<String>>,
    pub customer_phone_number: Option<Secret<String>>,
    pub customer_phone_country_code: Option<String>,
}

impl L2L3Data {
    pub fn get_shipping_country(&self) -> Option<common_enums::enums::CountryAlpha2> {
        self.shipping_details
            .as_ref()
            .and_then(|address| address.country)
    }

    pub fn get_shipping_city(&self) -> Option<Secret<String>> {
        self.shipping_details
            .as_ref()
            .and_then(|address| address.city.clone())
    }

    pub fn get_shipping_state(&self) -> Option<Secret<String>> {
        self.shipping_details
            .as_ref()
            .and_then(|address| address.state.clone())
    }

    pub fn get_shipping_origin_zip(&self) -> Option<Secret<String>> {
        self.shipping_details
            .as_ref()
            .and_then(|address| address.origin_zip.clone())
    }

    pub fn get_shipping_zip(&self) -> Option<Secret<String>> {
        self.shipping_details
            .as_ref()
            .and_then(|address| address.zip.clone())
    }

    pub fn get_shipping_address_line1(&self) -> Option<Secret<String>> {
        self.shipping_details
            .as_ref()
            .and_then(|address| address.line1.clone())
    }

    pub fn get_shipping_address_line2(&self) -> Option<Secret<String>> {
        self.shipping_details
            .as_ref()
            .and_then(|address| address.line2.clone())
    }

    pub fn get_order_date(&self) -> Option<time::PrimitiveDateTime> {
        self.order_info.as_ref().and_then(|order| order.order_date)
    }

    pub fn get_order_details(&self) -> Option<Vec<payment_address::OrderDetailsWithAmount>> {
        self.order_info
            .as_ref()
            .and_then(|order| order.order_details.clone())
    }

    pub fn get_merchant_order_reference_id(&self) -> Option<String> {
        self.order_info
            .as_ref()
            .and_then(|order| order.merchant_order_reference_id.clone())
    }

    pub fn get_discount_amount(&self) -> Option<MinorUnit> {
        self.order_info
            .as_ref()
            .and_then(|order| order.discount_amount)
    }

    pub fn get_shipping_cost(&self) -> Option<MinorUnit> {
        self.order_info
            .as_ref()
            .and_then(|order| order.shipping_cost)
    }

    pub fn get_duty_amount(&self) -> Option<MinorUnit> {
        self.order_info.as_ref().and_then(|order| order.duty_amount)
    }

    pub fn get_tax_status(&self) -> Option<common_enums::TaxStatus> {
        self.tax_info.as_ref().and_then(|tax| tax.tax_status)
    }

    pub fn get_customer_tax_registration_id(&self) -> Option<Secret<String>> {
        self.tax_info
            .as_ref()
            .and_then(|tax| tax.customer_tax_registration_id.clone())
    }

    pub fn get_merchant_tax_registration_id(&self) -> Option<Secret<String>> {
        self.tax_info
            .as_ref()
            .and_then(|tax| tax.merchant_tax_registration_id.clone())
    }

    pub fn get_shipping_amount_tax(&self) -> Option<MinorUnit> {
        self.tax_info
            .as_ref()
            .and_then(|tax| tax.shipping_amount_tax)
    }

    pub fn get_order_tax_amount(&self) -> Option<MinorUnit> {
        self.tax_info.as_ref().and_then(|tax| tax.order_tax_amount)
    }

    pub fn get_customer_id(&self) -> Option<CustomerId> {
        self.customer_info
            .as_ref()
            .and_then(|customer| customer.customer_id.clone())
    }

    pub fn get_customer_email(&self) -> Option<common_utils::pii::Email> {
        self.customer_info
            .as_ref()
            .and_then(|customer| customer.customer_email.clone())
    }

    pub fn get_customer_name(&self) -> Option<Secret<String>> {
        self.customer_info
            .as_ref()
            .and_then(|customer| customer.customer_name.clone())
    }

    pub fn get_customer_phone_number(&self) -> Option<Secret<String>> {
        self.customer_info
            .as_ref()
            .and_then(|customer| customer.customer_phone_number.clone())
    }

    pub fn get_customer_phone_country_code(&self) -> Option<String> {
        self.customer_info
            .as_ref()
            .and_then(|customer| customer.customer_phone_country_code.clone())
    }
    pub fn get_billing_city(&self) -> Option<Secret<String>> {
        self.billing_details
            .as_ref()
            .and_then(|billing| billing.city.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequestMetadata {
    pub supported_networks: Vec<String>,
    pub merchant_capabilities: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkNextAction {
    /// The type of next action
    pub next_action: NextActionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextActionCall {
    /// The next action call is Post Session Tokens
    PostSessionTokens,
    /// The next action call is confirm
    Confirm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApplePaySessionResponse {
    ///  We get this session response, when third party sdk is involved
    ThirdPartySdk(ThirdPartySdkSessionResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartySdkSessionResponse {
    pub secrets: SecretInfoToInitiateSdk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecretInfoToInitiateSdk {
    // Authorization secrets used by client to initiate sdk
    pub display: Secret<String>,
    // Authorization secrets used by client for payment
    pub payment: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sdk_type")]
#[serde(rename_all = "snake_case")]
pub enum ClientAuthenticationTokenData {
    /// The session response structure for Google Pay
    GooglePay(Box<GpayClientAuthenticationResponse>),
    /// The session response structure for PayPal
    Paypal(Box<PaypalClientAuthenticationResponse>),
    /// The session response structure for Apple Pay
    ApplePay(Box<ApplepayClientAuthenticationResponse>),
    /// Generic connector-specific SDK initialization data
    ConnectorSpecific(Box<ConnectorSpecificClientAuthenticationResponse>),
}

/// Per-connector SDK initialization data — discriminated by connector
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "connector")]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSpecificClientAuthenticationResponse {
    /// Stripe SDK initialization data
    Stripe(StripeClientAuthenticationResponse),
    /// Adyen SDK initialization data — session_id + session_data for Adyen Drop-in/Components
    Adyen(AdyenClientAuthenticationResponse),
    /// Checkout.com SDK initialization data — payment_session_token + payment_session_secret for Frames/Flow
    Checkout(CheckoutClientAuthenticationResponse),
    /// Cybersource SDK initialization data — capture_context JWT for Flex Microform SDK
    Cybersource(CybersourceClientAuthenticationResponse),
    /// Nuvei SDK initialization data — session_token for client-side SDK operations
    Nuvei(NuveiClientAuthenticationResponse),
    /// Mollie SDK initialization data — checkout_url for client-side redirect/components
    Mollie(MollieClientAuthenticationResponse),
    /// Globalpay SDK initialization data — access_token for client-side SDK operations
    Globalpay(GlobalpayClientAuthenticationResponse),
    /// Bluesnap SDK initialization data — pfToken for Hosted Payment Fields initialization
    Bluesnap(BluesnapClientAuthenticationResponse),
    /// Rapyd SDK initialization data — checkout_id and redirect_url for client-side checkout
    Rapyd(RapydClientAuthenticationResponse),
    /// Shift4 SDK initialization data — client_secret for client-side SDK
    Shift4(Shift4ClientAuthenticationResponse),
    /// Bank of America SDK initialization data — capture_context JWT for Flex Microform
    BankOfAmerica(BankOfAmericaClientAuthenticationResponse),
    /// Wellsfargo SDK initialization data — capture_context JWT for Flex Microform
    Wellsfargo(WellsfargoClientAuthenticationResponse),
    /// Fiserv SDK initialization data — session_id for client-side SDK
    Fiserv(FiservClientAuthenticationResponse),
    /// Elavon SDK initialization data — session_token for Converge Hosted Payments Lightbox
    Elavon(ElavonClientAuthenticationResponse),
    /// Noon SDK initialization data — order_id + checkout_url
    Noon(NoonClientAuthenticationResponse),
    /// Paysafe SDK initialization data — payment_handle_token for client-side SDK
    Paysafe(PaysafeClientAuthenticationResponse),
    /// Bamboraapac SDK initialization data — token for client-side SDK
    Bamboraapac(BamboraapacClientAuthenticationResponse),
    /// Jpmorgan SDK initialization data — transaction_id + request_id
    Jpmorgan(JpmorganClientAuthenticationResponse),
    /// Billwerk SDK initialization data — session_id for checkout session
    Billwerk(BillwerkClientAuthenticationResponse),
    /// Datatrans SDK initialization data — transaction_id for Secure Fields initialization
    Datatrans(DatatransClientAuthenticationResponse),
    /// Bambora SDK initialization data — token for Custom Checkout initialization
    Bambora(BamboraClientAuthenticationResponse),
    /// Payload SDK initialization data — client_token for Payload.js Checkout/Secure Input SDK
    Payload(PayloadClientAuthenticationResponse),
    /// Multisafepay SDK initialization data — api_token for Payment Components initialization
    Multisafepay(MultisafepayClientAuthenticationResponse),
    /// Nexinets SDK initialization data — order_id for client-side hosted payment page initialization
    Nexinets(NexinetsClientAuthenticationResponse),
    /// Nexixpay SDK initialization data — security_token and hosted_page URL for HPP initialization
    Nexixpay(NexixpayClientAuthenticationResponse),
}

/// Stripe's client_secret for browser-side stripe.confirmPayment()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeClientAuthenticationResponse {
    pub client_secret: Secret<String>,
}

/// Adyen's session_id and session_data for browser-side Adyen Drop-in/Components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdyenClientAuthenticationResponse {
    /// The unique identifier of the session
    pub session_id: String,
    /// The session data required to initialize the Adyen SDK
    pub session_data: Secret<String>,
}

/// Checkout.com's payment_session_token and payment_session_secret for Frames/Flow SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutClientAuthenticationResponse {
    /// The payment session identifier
    pub payment_session_id: String,
    /// The base64-encoded token for client-side SDK initialization
    pub payment_session_token: Secret<String>,
    /// The secret for secure client-side operations
    pub payment_session_secret: Secret<String>,
}

/// Cybersource's capture_context JWT for Flex Microform SDK initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CybersourceClientAuthenticationResponse {
    /// The capture context JWT token for client-side Flex Microform SDK
    pub capture_context: Secret<String>,
    /// URL to the Flex Microform JavaScript library (extracted from JWT payload)
    pub client_library: String,
    /// Subresource Integrity hash for the client library (extracted from JWT payload)
    pub client_library_integrity: String,
}

/// Nuvei's session_token for client-side SDK operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NuveiClientAuthenticationResponse {
    /// The session token for Nuvei client-side SDK
    pub session_token: Secret<String>,
}

/// Mollie's checkout_url for client-side redirect or Mollie Components initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MollieClientAuthenticationResponse {
    /// The payment ID created on Mollie's side
    pub payment_id: String,
    /// The checkout URL for client-side redirect to complete payment
    pub checkout_url: Secret<String>,
}

/// Globalpay's access_token for client-side SDK initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalpayClientAuthenticationResponse {
    /// The OAuth access token for client-side operations
    pub access_token: Secret<String>,
    /// The token type (e.g., "Bearer")
    pub token_type: Option<String>,
    /// The number of seconds until the token expires
    pub expires_in: Option<i64>,
}

/// Bluesnap's pfToken for client-side Hosted Payment Fields initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluesnapClientAuthenticationResponse {
    /// The Hosted Payment Fields token for client-side SDK initialization
    pub pf_token: Secret<String>,
}

/// Rapyd's checkout_id and redirect_url for client-side checkout page initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydClientAuthenticationResponse {
    /// The checkout page identifier
    pub checkout_id: String,
    /// The redirect URL for the client-side checkout experience
    pub redirect_url: String,
}

/// Shift4's client_secret for client-side SDK initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift4ClientAuthenticationResponse {
    /// The client secret for Shift4 SDK
    pub client_secret: Secret<String>,
}

/// Bank of America's capture_context JWT for Flex Microform SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankOfAmericaClientAuthenticationResponse {
    /// The capture context JWT token
    pub capture_context: Secret<String>,
}

/// Wellsfargo's capture_context JWT for Flex Microform SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WellsfargoClientAuthenticationResponse {
    /// The capture context JWT token
    pub capture_context: Secret<String>,
}

/// Fiserv's session_id for client-side SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiservClientAuthenticationResponse {
    /// The session ID for Fiserv client-side SDK
    pub session_id: Secret<String>,
}

/// Elavon's session_token for Converge Hosted Payments Lightbox initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElavonClientAuthenticationResponse {
    /// The transaction auth token for Converge Lightbox
    pub session_token: Secret<String>,
}

/// Noon's order_id and checkout_url for client-side checkout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoonClientAuthenticationResponse {
    /// The Noon order identifier
    pub order_id: u64,
    /// The checkout URL for client-side redirect
    pub checkout_url: Secret<String>,
}

/// Paysafe's payment_handle_token for client-side SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaysafeClientAuthenticationResponse {
    /// The payment handle token for Paysafe client-side SDK
    pub payment_handle_token: Secret<String>,
}

/// Bamboraapac's token for client-side SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BamboraapacClientAuthenticationResponse {
    /// The tokenization token for client-side SDK
    pub token: Secret<String>,
}

/// Jpmorgan's transaction_id and request_id for client-side SDK initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpmorganClientAuthenticationResponse {
    /// The transaction identifier
    pub transaction_id: String,
    /// The request identifier
    pub request_id: String,
}

/// Billwerk's session_id for checkout session initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillwerkClientAuthenticationResponse {
    /// The checkout session identifier
    pub session_id: String,
}

/// Datatrans's transaction_id for client-side Secure Fields initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatatransClientAuthenticationResponse {
    /// The transaction ID returned from Secure Fields init, used as a client auth token
    pub transaction_id: Secret<String>,
}

/// Bambora's token for client-side Custom Checkout SDK initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BamboraClientAuthenticationResponse {
    /// The tokenization token returned from Bambora's tokenization API
    pub token: Secret<String>,
}

/// Payload's client_token for Payload.js Checkout/Secure Input SDK initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadClientAuthenticationResponse {
    /// The client token ID returned from POST /access_tokens for client-side SDK initialization
    pub client_token: Secret<String>,
}

/// Multisafepay's api_token for client-side Payment Components initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisafepayClientAuthenticationResponse {
    /// The API token for encrypting sensitive payment details (valid for 600 seconds)
    pub api_token: Secret<String>,
}

/// Nexinets' order_id for client-side hosted payment page initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexinetsClientAuthenticationResponse {
    /// The order ID that serves as the client authentication token for hosted checkout
    pub order_id: String,
}

/// Nexixpay's security_token and hosted_page URL for HPP (Hosted Payment Page) initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexixpayClientAuthenticationResponse {
    /// The security token for authenticating client-side hosted payment page requests
    pub security_token: Secret<String>,
    /// The hosted payment page URL for client-side redirect
    pub hosted_page: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GpayClientAuthenticationResponse {
    /// Google pay session response for non third party sdk
    GooglePaySession(GooglePaySessionResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct GooglePaySessionResponse {
    /// The merchant info
    pub merchant_info: GpayMerchantInfo,
    /// Is shipping address required
    pub shipping_address_required: bool,
    /// Is email required
    pub email_required: bool,
    /// Shipping address parameters
    pub shipping_address_parameters: GpayShippingAddressParameters,
    /// List of the allowed payment methods
    pub allowed_payment_methods: Vec<GpayAllowedPaymentMethods>,
    /// The transaction info Google Pay requires
    pub transaction_info: GpayTransactionInfo,
    /// Identifier for the delayed session response
    pub delayed_session_token: bool,
    /// The name of the connector
    pub connector: String,
    /// The next action for the sdk (ex: calling confirm or sync call)
    pub sdk_next_action: SdkNextAction,
    /// Secrets for sdk display and payment
    pub secrets: Option<SecretInfoToInitiateSdk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayTransactionInfo {
    /// The country code
    pub country_code: common_enums::CountryAlpha2,
    /// The currency code
    pub currency_code: Currency,
    /// The total price status (ex: 'FINAL')
    pub total_price_status: String,
    /// The total price
    pub total_price: MinorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct GpayShippingAddressParameters {
    /// Is shipping phone number required
    pub phone_number_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayAllowedPaymentMethods {
    /// The type of payment method
    #[serde(rename = "type")]
    pub payment_method_type: String,
    /// The parameters Google Pay requires
    pub parameters: GpayAllowedMethodsParameters,
    /// The tokenization specification for Google Pay
    pub tokenization_specification: GpayTokenizationSpecification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayTokenizationSpecification {
    /// The token specification type(ex: PAYMENT_GATEWAY)
    #[serde(rename = "type")]
    pub token_specification_type: String,
    /// The parameters for the token specification Google Pay
    pub parameters: GpayTokenParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayTokenParameters {
    /// The name of the connector
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// The merchant ID registered in the connector associated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_merchant_id: Option<String>,
    /// The protocol version for encryption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    /// The public key provided by the merchant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayAllowedMethodsParameters {
    /// The list of allowed auth methods (ex: 3DS, No3DS, PAN_ONLY etc)
    pub allowed_auth_methods: Vec<String>,
    /// The list of allowed card networks (ex: AMEX,JCB etc)
    pub allowed_card_networks: Vec<String>,
    /// Is billing address required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address_required: Option<bool>,
    /// Billing address parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address_parameters: Option<GpayBillingAddressParameters>,
    /// Whether assurance details are required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assurance_details_required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayBillingAddressParameters {
    /// Is billing phone number required
    pub phone_number_required: bool,
    /// Billing address format
    pub format: GpayBillingAddressFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpayBillingAddressFormat {
    FULL,
    MIN,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayMerchantInfo {
    /// The merchant Identifier that needs to be passed while invoking Gpay SDK
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_id: Option<String>,
    /// The name of the merchant that needs to be displayed on Gpay PopUp
    pub merchant_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpayMetaData {
    pub merchant_info: GpayMerchantInfo,
    pub allowed_payment_methods: Vec<GpayAllowedPaymentMethods>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpaySessionTokenData {
    #[serde(rename = "google_pay")]
    pub data: GpayMetaData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct ApplepayClientAuthenticationResponse {
    /// Session object for Apple Pay
    /// The session_response will be null for iOS devices because the Apple Pay session call is skipped, as there is no web domain involved
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_response: Option<ApplePaySessionResponse>,
    /// Payment request object for Apple Pay
    pub payment_request_data: Option<ApplePayPaymentRequest>,
    /// The session token is w.r.t this connector
    pub connector: String,
    /// Identifier for the delayed session response
    pub delayed_session_token: bool,
    /// The next action for the sdk (ex: calling confirm or sync call)
    pub sdk_next_action: SdkNextAction,
    /// The connector transaction id
    pub connector_reference_id: Option<String>,
    /// The public key id is to invoke third party sdk
    pub connector_sdk_public_key: Option<String>,
    /// The connector merchant id
    pub connector_merchant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplePayPaymentRequest {
    /// The code for country
    pub country_code: common_enums::CountryAlpha2,
    /// The code for currency
    pub currency_code: Currency,
    /// Represents the total for the payment.
    pub total: AmountInfo,
    /// The list of merchant capabilities(ex: whether capable of 3ds or no-3ds)
    pub merchant_capabilities: Option<Vec<String>>,
    /// The list of supported networks
    pub supported_networks: Option<Vec<String>>,
    pub merchant_identifier: Option<String>,
    /// The required billing contact fields for connector
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_billing_contact_fields: Option<ApplePayBillingContactFields>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The required shipping contacht fields for connector
    pub required_shipping_contact_fields: Option<ApplePayShippingContactFields>,
    /// Recurring payment request for apple pay Merchant Token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_payment_request: Option<ApplePayRecurringPaymentRequest>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApplePayRecurringPaymentRequest {
    /// A description of the recurring payment that Apple Pay displays to the user in the payment sheet
    pub payment_description: String,
    /// The regular billing cycle for the recurring payment, including start and end dates, an interval, and an interval count
    pub regular_billing: ApplePayRegularBillingRequest,
    /// A localized billing agreement that the payment sheet displays to the user before the user authorizes the payment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_agreement: Option<String>,
    /// A URL to a web page where the user can update or delete the payment method for the recurring payment
    pub management_u_r_l: Url,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApplePayRegularBillingRequest {
    /// The amount of the recurring payment
    pub amount: StringMajorUnit,
    /// The label that Apple Pay displays to the user in the payment sheet with the recurring details
    pub label: String,
    /// The time that the payment occurs as part of a successful transaction
    pub payment_timing: ApplePayPaymentTiming,
    /// The date of the first payment
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "common_utils::custom_serde::iso8601::option")]
    pub recurring_payment_start_date: Option<PrimitiveDateTime>,
    /// The date of the final payment
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "common_utils::custom_serde::iso8601::option")]
    pub recurring_payment_end_date: Option<PrimitiveDateTime>,
    /// The amount of time — in calendar units, such as day, month, or year — that represents a fraction of the total payment interval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_payment_interval_unit: Option<RecurringPaymentIntervalUnit>,
    /// The number of interval units that make up the total payment interval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_payment_interval_count: Option<i32>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurringPaymentIntervalUnit {
    Year,
    Month,
    Day,
    Hour,
    Minute,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplePayPaymentTiming {
    /// A value that specifies that the payment occurs when the transaction is complete
    Immediate,
    /// A value that specifies that the payment occurs on a regular basis
    Recurring,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplePayBillingContactFields(pub Vec<ApplePayAddressParameters>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplePayShippingContactFields(pub Vec<ApplePayAddressParameters>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplePayAddressParameters {
    PostalAddress,
    Phone,
    Email,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmountInfo {
    /// The label must be the name of the merchant.
    pub label: String,
    /// A value that indicates whether the line item(Ex: total, tax, discount, or grand total) is final or pending.
    #[serde(rename = "type")]
    pub total_type: Option<String>,
    /// The total amount
    pub amount: MinorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct PaypalClientAuthenticationResponse {
    /// Name of the connector
    pub connector: String,
    /// The session token for PayPal
    pub session_token: String,
    /// The next action for the sdk (ex: calling confirm or sync call)
    pub sdk_next_action: SdkNextAction,
    /// Authorization token used by client to initiate sdk
    pub client_token: Option<String>,
    /// The transaction info Paypal requires
    pub transaction_info: Option<PaypalTransactionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalTransactionInfo {
    /// Paypal flow type
    pub flow: PaypalFlow,
    /// Currency code
    pub currency_code: Currency,
    /// Total price
    pub total_price: MinorUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaypalFlow {
    Checkout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalSdkMetaData {
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaypalClientAuthenticationTokenData {
    #[serde(rename = "paypal_sdk")]
    pub data: PaypalSdkMetaData,
}

/// Billing Descriptor information to be sent to the payment gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingDescriptor {
    /// name to be put in billing description
    pub name: Option<Secret<String>>,
    /// city to be put in billing description
    pub city: Option<Secret<String>>,
    /// phone to be put in billing description
    pub phone: Option<Secret<String>>,
    /// a short description for the payment
    pub statement_descriptor: Option<String>,
    /// Concatenated with the prefix (shortened descriptor) or statement descriptor that’s set on the account to form the complete statement descriptor.
    pub statement_descriptor_suffix: Option<String>,
    /// A reference to be shown on billing description
    pub reference: Option<String>,
}
impl ForeignTryFrom<grpc_api_types::payments::connector_specific_config::Config> for ConnectorEnum {
    type Error = IntegrationError;
    fn foreign_try_from(
        auth_type: grpc_api_types::payments::connector_specific_config::Config,
    ) -> Result<Self, error_stack::Report<Self::Error>> {
        match auth_type {
            AuthType::Adyen(_) => Ok(Self::Adyen),
            AuthType::Airwallex(_) => Ok(Self::Airwallex),
            AuthType::Bambora(_) => Ok(Self::Bambora),
            AuthType::Bankofamerica(_) => Ok(Self::Bankofamerica),
            AuthType::Billwerk(_) => Ok(Self::Billwerk),
            AuthType::Bluesnap(_) => Ok(Self::Bluesnap),
            AuthType::Braintree(_) => Ok(Self::Braintree),
            AuthType::Cashtocode(_) => Ok(Self::Cashtocode),
            AuthType::Cryptopay(_) => Ok(Self::Cryptopay),
            AuthType::Cybersource(_) => Ok(Self::Cybersource),
            AuthType::Datatrans(_) => Ok(Self::Datatrans),
            AuthType::Dlocal(_) => Ok(Self::Dlocal),
            AuthType::Elavon(_) => Ok(Self::Elavon),
            AuthType::Fiserv(_) => Ok(Self::Fiserv),
            AuthType::Fiservemea(_) => Ok(Self::Fiservemea),
            AuthType::Forte(_) => Ok(Self::Forte),
            AuthType::Getnet(_) => Ok(Self::Getnet),
            AuthType::Globalpay(_) => Ok(Self::Globalpay),
            AuthType::Hipay(_) => Ok(Self::Hipay),
            AuthType::Helcim(_) => Ok(Self::Helcim),
            AuthType::Iatapay(_) => Ok(Self::Iatapay),
            AuthType::Jpmorgan(_) => Ok(Self::Jpmorgan),
            AuthType::Mifinity(_) => Ok(Self::Mifinity),
            AuthType::Mollie(_) => Ok(Self::Mollie),
            AuthType::Multisafepay(_) => Ok(Self::Multisafepay),
            AuthType::Nexinets(_) => Ok(Self::Nexinets),
            AuthType::Nexixpay(_) => Ok(Self::Nexixpay),
            AuthType::Nmi(_) => Ok(Self::Nmi),
            AuthType::Noon(_) => Ok(Self::Noon),
            AuthType::Novalnet(_) => Ok(Self::Novalnet),
            AuthType::Nuvei(_) => Ok(Self::Nuvei),
            AuthType::Paybox(_) => Ok(Self::Paybox),
            AuthType::Payme(_) => Ok(Self::Payme),
            AuthType::Payu(_) => Ok(Self::Payu),
            AuthType::Powertranz(_) => Ok(Self::Powertranz),
            AuthType::Rapyd(_) => Ok(Self::Rapyd),
            AuthType::Redsys(_) => Ok(Self::Redsys),
            AuthType::Shift4(_) => Ok(Self::Shift4),
            AuthType::Stax(_) => Ok(Self::Stax),
            AuthType::Stripe(_) => Ok(Self::Stripe),
            AuthType::Trustpay(_) => Ok(Self::Trustpay),
            AuthType::Tsys(_) => Ok(Self::Tsys),
            AuthType::Volt(_) => Ok(Self::Volt),
            AuthType::Wellsfargo(_) => Ok(Self::Wellsfargo),
            AuthType::Worldpay(_) => Ok(Self::Worldpay),
            AuthType::Worldpayvantiv(_) => Ok(Self::Worldpayvantiv),
            AuthType::Xendit(_) => Ok(Self::Xendit),
            AuthType::Phonepe(_) => Ok(Self::Phonepe),
            AuthType::Cashfree(_) => Ok(Self::Cashfree),
            AuthType::Paytm(_) => Ok(Self::Paytm),
            AuthType::Calida(_) => Ok(Self::Calida),
            AuthType::Payload(_) => Ok(Self::Payload),
            AuthType::Paypal(_) => Ok(Self::Paypal),
            AuthType::Authipay(_) => Ok(Self::Authipay),
            AuthType::Silverflow(_) => Ok(Self::Silverflow),
            AuthType::Celero(_) => Ok(Self::Celero),
            AuthType::Trustpayments(_) => Ok(Self::Trustpayments),
            AuthType::Paysafe(_) => Ok(Self::Paysafe),
            AuthType::Barclaycard(_) => Ok(Self::Barclaycard),
            AuthType::Worldpayxml(_) => Ok(Self::Worldpayxml),
            AuthType::Revolut(_) => Ok(Self::Revolut),
            AuthType::Loonio(_) => Ok(Self::Loonio),
            AuthType::Gigadat(_) => Ok(Self::Gigadat),
            AuthType::Hyperpg(_) => Ok(Self::Hyperpg),
            AuthType::Peachpayments(_) => Ok(Self::Peachpayments),
            AuthType::Zift(_) => Ok(Self::Zift),
            AuthType::Trustly(_) => Ok(Self::Trustly),
            AuthType::Truelayer(_) => Ok(Self::Truelayer),
            AuthType::Fiservcommercehub(_) => Ok(Self::Fiservcommercehub),
            AuthType::Itaubank(_) => Ok(Self::Itaubank),
            AuthType::Axisbank(_) => Ok(Self::Axisbank),
            AuthType::Screenstream(_) => Err(error_stack::Report::new(
                IntegrationError::InvalidDataFormat {
                    field_name: "connector",
                    context: IntegrationErrorContext::default(),
                },
            )),
            AuthType::Ebanx(_) => Err(error_stack::Report::new(
                IntegrationError::InvalidDataFormat {
                    field_name: "connector",
                    context: IntegrationErrorContext::default(),
                },
            )),
            AuthType::Fiuu(_) => Ok(Self::Fiuu),
            AuthType::Globepay(_) => Err(error_stack::Report::new(
                IntegrationError::InvalidDataFormat {
                    field_name: "connector",
                    context: IntegrationErrorContext::default(),
                },
            )),
            AuthType::Coinbase(_) => Err(error_stack::Report::new(
                IntegrationError::InvalidDataFormat {
                    field_name: "connector",
                    context: IntegrationErrorContext::default(),
                },
            )),
            AuthType::Coingate(_) => Err(error_stack::Report::new(
                IntegrationError::InvalidDataFormat {
                    field_name: "connector",
                    context: IntegrationErrorContext::default(),
                },
            )),
            AuthType::Revolv3(_) => Ok(Self::Revolv3),
            AuthType::Authorizedotnet(_) => Ok(Self::Authorizedotnet),
            AuthType::Ppro(_) => Ok(Self::Ppro),
        }
    }
}
