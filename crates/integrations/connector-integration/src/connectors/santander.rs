pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    errors::{self},
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

use self::transformers::SantanderErrorResponse;
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

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: Result<SantanderErrorResponse, _> =
            res.response.parse_struct("SantanderErrorResponse");

        match response {
            Ok(error_res) => {
                event_builder.map(|i| i.set_connector_response(&error_res));
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_res.error_code(res.status_code),
                    message: error_res.error_message(res.status_code),
                    reason: error_res.error_reason(),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            Err(error) => {
                tracing::error!(
                    "Failed to parse error response from Santander. Status: {}, Error: {:?}, Raw: {:?}",
                    res.status_code,
                    error,
                    res.response
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: res.status_code.to_string(),
                    message: "Failed to parse error response from connector".to_string(),
                    reason: Some(format!("Raw response: {:?}", res.response)),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
        }
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
