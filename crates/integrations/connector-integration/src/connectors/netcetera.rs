// Netcetera 3DS authentication connector.
//
// SCAFFOLD: This is an authentication-only (EMVCo 3DS) connector. It wires up
// the three authentication flows (PreAuthenticate / Authenticate /
// PostAuthenticate) and a stub Authorize flow (required by the
// `ConnectorServiceTrait` bound) that returns a `NotImplemented` error. The
// transformer request/response bodies are minimal stubs; the real EMVCo 3DS
// request/response porting is a separate, later step.

pub mod netcetera_types;
pub mod transformers;

use std::fmt::Debug;

use common_utils::{errors::CustomResult, events, ext_traits::BytesExt};
use domain_types::{
    connector_flow::{Authenticate, Authorize, PostAuthenticate, PreAuthenticate},
    connector_types::{
        ConnectorSpecifications, PaymentFlowData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;

use transformers::{
    NetceteraAuthenticateRequest, NetceteraAuthenticateResponse, NetceteraAuthorizeRequest,
    NetceteraAuthorizeResponse, NetceteraErrorResponse, NetceteraPostAuthenticateRequest,
    NetceteraPostAuthenticateResponse, NetceteraPreAuthenticateRequest,
    NetceteraPreAuthenticateResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

/// The template segment in the configured Netcetera 3DS Server host that must be replaced with
/// the per-merchant endpoint prefix (e.g. `flowbird`) before the request is dispatched.
const MERCHANT_ENDPOINT_PREFIX_TEMPLATE: &str = "{{merchant_endpoint_prefix}}";

/// Resolve the per-request Netcetera 3DS Server host by substituting the
/// `{{merchant_endpoint_prefix}}` template segment in the configured `base_url` with the
/// `endpoint_prefix` from the merchant's Netcetera MCA metadata (forwarded by the router on
/// `PaymentFlowData.connector_feature_data`).
///
/// `base_url(&connectors)` returns a borrowed `&str` from config, so the substitution cannot
/// happen there — it must be done per-request here, where the request's `connector_feature_data`
/// is available. If the template is absent the base host is returned unchanged; if it is present
/// but no `endpoint_prefix` is configured, a clear error is raised (an unsubstituted template
/// would produce an invalid URL).
fn resolve_netcetera_base_url(
    base_url: &str,
    connector_feature_data: Option<hyperswitch_masking::Secret<serde_json::Value>>,
) -> CustomResult<String, IntegrationError> {
    let base_url = base_url.trim_end_matches('/');
    if !base_url.contains(MERCHANT_ENDPOINT_PREFIX_TEMPLATE) {
        return Ok(base_url.to_string());
    }

    let netcetera_meta: netcetera_types::NetceteraMeta = connector_feature_data
        .map(|data| crate::utils::to_connector_meta_from_secret(Some(data)))
        .transpose()?
        .unwrap_or_default();

    let endpoint_prefix = netcetera_meta.endpoint_prefix.ok_or_else(|| {
        error_stack::report!(IntegrationError::InvalidConnectorConfig {
            config: "netcetera.endpoint_prefix",
            context: Default::default(),
        })
    })?;

    Ok(base_url.replace(MERCHANT_ENDPOINT_PREFIX_TEMPLATE, &endpoint_prefix))
}

// ---------------------------------------------------------------------------
// Marker traits
// ---------------------------------------------------------------------------
// The flows implemented directly below (Authorize, PreAuthenticate,
// Authenticate, PostAuthenticate) need their marker traits hand-written; all
// other flows get marker traits from `macro_connector_flow_status_impls!`.

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Netcetera<T>
{
}

// Marker traits not tied to a flow that the status macro does not emit.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Netcetera<T>
{
    // Sequence the 3DS authentication steps for a ThreeDs + Card payment.
    // Without this override the default returns `Authorize`, which would skip
    // 3DS entirely and the authentication flows would never fire. Modeled on
    // Cybersource (`cybersource.rs`).
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                // Initial request: run the 3DS version / method call.
                (RedirectState::InitialRequest, _) => AuthenticationStep::PreAuthenticate,

                // After PreAuthenticate (DDC), the browser returns with params:
                // run the authentication (AReq).
                (RedirectState::RedirectWithParams, None) => AuthenticationStep::Authenticate,

                (RedirectState::RedirectWithParams, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::Authorize
                }

                // Browser returns from the ACS challenge without params:
                // fetch the challenge result (RReq).
                (RedirectState::RedirectWithoutParams, None) => {
                    AuthenticationStep::PostAuthenticate
                }

                (
                    RedirectState::RedirectWithoutParams,
                    Some(AuthenticationStep::PostAuthenticate),
                ) => AuthenticationStep::Authorize,

                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Netcetera<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Netcetera<T>
{
}

// ---------------------------------------------------------------------------
// ConnectorCommon
// ---------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Netcetera<T>
{
    fn id(&self) -> &'static str {
        "netcetera"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // STUB: EMVCo porting must derive the real authentication headers
        // (mTLS / API key) from the connector configuration.
        Ok(Vec::new())
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        // Real Netcetera 3DS Server host (configured per-environment under
        // `[connectors] netcetera.base_url`). The configured value contains a
        // `{{merchant_endpoint_prefix}}` template segment which is substituted
        // per-request in `resolve_netcetera_base_url` using the merchant's
        // `endpoint_prefix` (from `NetceteraMeta` / `connector_feature_data`).
        // `base_url` returns a borrowed `&str` from config, so it CANNOT do the
        // per-merchant substitution itself — it returns the raw templated host and
        // each flow's `get_url` resolves the prefix before dispatching.
        connectors.netcetera.base_url.as_str()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        match res
            .response
            .parse_struct::<NetceteraErrorResponse>("NetceteraErrorResponse")
        {
            Ok(error_response) => {
                with_error_response_body!(event_builder, error_response);
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_response
                        .error_code
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: error_response
                        .error_description
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: error_response.error_detail.or(error_response.error_description),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            Err(_) => crate::utils::handle_json_response_deserialization_failure(res, "netcetera"),
        }
    }
}

// ---------------------------------------------------------------------------
// Prerequisites (Bridge structs + connector struct + new())
// ---------------------------------------------------------------------------

macros::create_all_prerequisites!(
    connector_name: Netcetera,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: NetceteraAuthorizeRequest,
            response_body: NetceteraAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: NetceteraPreAuthenticateRequest<T>,
            response_body: NetceteraPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: NetceteraAuthenticateRequest<T>,
            response_body: NetceteraAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: NetceteraPostAuthenticateRequest,
            response_body: NetceteraPostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }
    }
);

// ---------------------------------------------------------------------------
// Authorize (stub: returns NotImplemented before any HTTP call)
// ---------------------------------------------------------------------------

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Netcetera,
    curl_request: Json(NetceteraAuthorizeRequest),
    curl_response: NetceteraAuthorizeResponse,
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
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Authorize is not supported for an authentication-only connector.
            Err(IntegrationError::not_implemented(
                "Authorize flow is not supported by the Netcetera (3DS authentication-only) connector",
                Default::default(),
            ))?
        }
    }
);

// ---------------------------------------------------------------------------
// PreAuthenticate
// ---------------------------------------------------------------------------

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Netcetera,
    curl_request: Json(NetceteraPreAuthenticateRequest<T>),
    curl_response: NetceteraPreAuthenticateResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Netcetera 3DS Server version / 3DS method endpoint (PRes).
            let base_url = resolve_netcetera_base_url(
                self.base_url(&req.resource_common_data.connectors),
                req.resource_common_data.connector_feature_data.clone(),
            )?;
            Ok(format!("{base_url}/3ds/versioning"))
        }
    }
);

// ---------------------------------------------------------------------------
// Authenticate
// ---------------------------------------------------------------------------

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Netcetera,
    curl_request: Json(NetceteraAuthenticateRequest<T>),
    curl_response: NetceteraAuthenticateResponse,
    flow_name: Authenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Netcetera 3DS Server authentication endpoint (AReq -> ARes).
            let base_url = resolve_netcetera_base_url(
                self.base_url(&req.resource_common_data.connectors),
                req.resource_common_data.connector_feature_data.clone(),
            )?;
            Ok(format!("{base_url}/3ds/authentication"))
        }
    }
);

// ---------------------------------------------------------------------------
// PostAuthenticate
// ---------------------------------------------------------------------------

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Netcetera,
    curl_request: Json(NetceteraPostAuthenticateRequest),
    curl_response: NetceteraPostAuthenticateResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Netcetera 3DS Server results fetch endpoint (RReq -> RRes).
            let base_url = resolve_netcetera_base_url(
                self.base_url(&req.resource_common_data.connectors),
                req.resource_common_data.connector_feature_data.clone(),
            )?;
            Ok(format!("{base_url}/3ds/results"))
        }
    }
);

// ---------------------------------------------------------------------------
// Unsupported / not-implemented flows
// ---------------------------------------------------------------------------

macros::macro_connector_flow_status_impls!(
    connector: Netcetera,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PaymentMethodToken,
    ],
    not_supported: [
        PSync,
        Capture,
        Void,
        VoidPC,
        Refund,
        RSync,
        VoidPostRefund,
        SetupMandate,
        RepeatPayment,
        IncrementalAuthorization,
        MandateRevoke,
        CreateOrder,
        CreateConnectorCustomer,
        ClientAuthenticationToken,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        Accept,
        SubmitEvidence,
        DefendDispute,
    ],
);
