//! Citigate connector.
//!
//! Citigate is a single-endpoint, verb-in-body JSON gateway: every operation is a
//! `POST` to `/orion/interface/json.ashx` and the operation is selected by the
//! `TransTypeID` body field, never by the URL or HTTP method. Credentials
//! (`MerchantName` / `MerchantPassword`) travel in the body too, so
//! [`ConnectorCommon::get_auth_header`] contributes no headers.
//!
//! Implemented scope: Card / Authorize (Purchase), one-time, non-3DS.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{self as citigate, CitigatePaymentsRequest, CitigatePaymentsResponse};

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

/// Path shared by every Citigate operation, appended to the configured base URL.
const CITIGATE_JSON_INTERFACE_PATH: &str = "/orion/interface/json.ashx";

// Citigate expects the amount in the smallest denomination of the currency with no
// decimal point, and quotes it as a JSON string.
macros::create_amount_converter_wrapper!(connector_name: Citigate, amount_type: StringMinorUnit);

// ===== MACRO PREREQUISITES =====
macros::create_all_prerequisites!(
    connector_name: Citigate,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: CitigatePaymentsRequest<T>,
            response_body: CitigatePaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // Citigate has no authentication headers: MerchantName / MerchantPassword
            // are members of the request body.
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.citigate.base_url
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Citigate<T>
{
    fn id(&self) -> &'static str {
        "citigate"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.citigate.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Validate that the configured credentials have the shape Citigate needs, but
        // emit no headers: the credentials are injected into the request body.
        citigate::CitigateAuthType::try_from(auth_type)?;
        Ok(Vec::new())
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Citigate answers every outcome with HTTP 200 and the same envelope, so the
        // error path parses the very same struct as the success path.
        let response: CitigatePaymentsResponse = res
            .response
            .parse_struct("CitigatePaymentsResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "citigate: response body did not match the expected TransactionResponse format.",
            ))?;

        with_error_response_body!(event_builder, response);

        Ok(response.to_error_response(res.status_code))
    }
}

// ===== FLOW-SPECIFIC CONNECTOR INTEGRATION IMPLEMENTATIONS =====

// Authorize Flow — Purchase, PaymentTypeID = 1, TransTypeID = 0.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Citigate,
    curl_request: Json(CitigatePaymentsRequest),
    curl_response: CitigatePaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                CITIGATE_JSON_INTERFACE_PATH
            ))
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Citigate<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Citigate<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Citigate<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Citigate<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Citigate<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// Citigate's only signed callbacks belong to the 3DS redirect and the opt-in
// refund/fraud notification services, both out of scope for this integration.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Citigate<T>
{
}

// ===== BODY DECODING IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Citigate<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
macros::macro_connector_payout_implementation!(
    connector: Citigate,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Every flow other than Authorize is stubbed: the Citigate JSON interface does
// support capture / cancel / refund / status-check on the same endpoint, but they
// are out of scope for this integration.
macros::macro_connector_flow_status_impls!(
    connector: Citigate,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PSync,
        PaymentMethodToken,
        VoidPC,
        Void,
        RSync,
        Refund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence,
        GetConnectorCustomer,
        VoidPostRefund
    ],
);
