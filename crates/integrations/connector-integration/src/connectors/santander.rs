use common_enums::CurrencyUnit;
use common_utils::errors::CustomResult;
use domain_types::{
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig},
    types::Connectors,
};
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_types::{self},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;

use std::fmt::Debug;

use super::macros;

// ===== MACRO PREREQUISITES =====
macros::create_all_prerequisites!(
    connector_name: Santander,
    generic_type: T,
    api: [],
    amount_converters: [],
    member_functions: {}
);

// ===== CONNECTOR COMMON IMPL =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Santander<T>
{
    fn id(&self) -> &'static str {
        "santander"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.santander.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        Ok(vec![])
    }
}

// ===== VALIDATION TRAIT =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Santander<T>
{
}

// ===== CONNECTOR SERVICE TRAIT =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Santander<T>
{
}

// ===== NO-OP PAYMENT TRAIT IMPLS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Santander<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Santander<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Santander<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Santander<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Santander,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Authorize,
        PSync,
        Refund,
        RSync,
        SetupMandate,
        RepeatPayment,
    ],
    not_supported: [
        VoidPostRefund,
        Void,
        VoidPC,
        Capture,
        ClientAuthenticationToken,
        MandateRevoke,
        CreateOrder,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        IncrementalAuthorization,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        Accept,
        SubmitEvidence,
        DefendDispute,
        CreateConnectorCustomer,
    ],
);
