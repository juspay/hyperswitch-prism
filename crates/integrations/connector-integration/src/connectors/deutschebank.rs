// Deutsche Bank is a payouts-only connector in prism. This file exists so that the
// `ConnectorEnum::Deutschebank` payment-side dispatch arm in `types.rs` compiles —
// every payment-side flow returns `not_implemented`. The real implementation lives
// in `payout_connectors/deutschebank.rs`.

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events};
use domain_types::{
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_response_types::Response,
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

use super::macros;

macros::create_all_prerequisites!(
    connector_name: Deutschebank,
    generic_type: T,
    api: [],
    amount_converters: [],
    member_functions: {}
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Deutschebank<T>
{
    fn id(&self) -> &'static str {
        "deutschebank"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.deutschebank.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: res.status_code.to_string(),
            message: "Deutsche Bank payment flows are not supported in prism".to_string(),
            reason: Some(format!("Raw response: {:?}", res.response)),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Deutschebank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Deutschebank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Deutschebank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Deutschebank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Deutschebank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Deutschebank<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Deutschebank,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [],
    not_supported: [
        Authorize,
        PSync,
        Refund,
        RSync,
        SetupMandate,
        RepeatPayment,
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
