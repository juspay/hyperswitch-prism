pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::BASE64_ENGINE_URL_SAFE_NO_PAD, errors::CustomResult, events, ext_traits::ByteSliceExt,
    types::StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        FrmChargebackReceived, FrmPaymentOutcome, FrmRefundProcessed, PostRiskCheck,
        PreAuthenticate, PreRiskCheck, ServerAuthenticationToken,
    },
    connector_types::{
        PaymentFlowData, PaymentsPreAuthenticateData, PaymentsResponseData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    frm::frm_types::{
        FrmChargebackReceivedRequest, FrmChargebackReceivedResponse, FrmFlowData,
        FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse, FrmRefundProcessedRequest,
        FrmRefundProcessedResponse, PostRiskCheckRequest, PostRiskCheckResponse,
        PreRiskCheckRequest, PreRiskCheckResponse,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::{RedirectForm, Response},
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as kount;
use transformers::{
    KountEvaluateOrderRequest, KountOrderResponse, KountRefundUpdateRequest,
    KountRefundUpdateResponse, KountTokenRequest, KountTokenResponse, KountUpdateOrderRequest,
    KountUpdateOrderResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Kount endpoints / constants.
const KOUNT_LOGIN_BASE_URL: &str = "https://login.kount.com";
/// Sandbox OAuth authorization-server id (from the Kount integration guide).
const KOUNT_SANDBOX_AUTH_SERVER_ID: &str = "ausdppkujzCPQuIrY357";
const KOUNT_ORDERS_PATH: &str = "/commerce/v2/orders";
const FORM_URL_ENCODED: &str = "application/x-www-form-urlencoded";

/// Kount Web Client SDK (`@kount/kount-web-client-sdk`), hosted as a browser
/// ESM bundle by jsDelivr. Self-contained `<script type="module">` load — no
/// bundler required on the merchant page.
const KOUNT_WEB_SDK_URL: &str =
    "https://cdn.jsdelivr.net/npm/@kount/kount-web-client-sdk@2.2.3/+esm";

/// Build the Device Data Collection (DDC) HTML returned by the PreAuthenticate
/// step. Rendered in the shopper's browser; makes **no** server-side call to
/// Kount. Follows the Kount Web Client SDK contract: `clientID` is the
/// Kount-assigned merchant/client id, `environment` is `TEST`/`PROD`, callbacks
/// live inside the config object, and the session id is passed as the second
/// argument to `kountSDK(config, sessionID)`.
///
/// `return_url` is the merchant's own continuation URL: on `collect-end` the
/// form posts there so the browser returns to the merchant flow. Kount is not
/// involved in this hop and never receives the URL — DDC correlates purely by
/// `sessionID`. When `return_url` is `None` the form self-submits.
pub fn build_ddc_html(
    client_id: &str,
    session_id: &str,
    sandbox: bool,
    return_url: Option<&str>,
) -> String {
    let environment = if sandbox { "TEST" } else { "PROD" };
    // Contextual output-encoding: `client_id` (from the access-token JWT) and
    // `session_id` are interpolated into a JS string literal; `return_url` into
    // an HTML attribute. Encode each for its context so no value can break out.
    let client_id = js_string_escape(client_id);
    let session_id = js_string_escape(session_id);
    let form_action = return_url
        .map(|url| format!(r#" action="{}""#, html_attr_escape(url)))
        .unwrap_or_default();
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"></head><body>
<form id="kount-ddc-form" method="POST"{form_action}></form>
<script type="module">
  import kountSDK from "{KOUNT_WEB_SDK_URL}";
  const kountConfig = {{
    clientID: "{client_id}",
    environment: "{environment}",
    isSinglePageApp: false,
    isDebugEnabled: false,
    callbacks: {{
      "collect-end": function () {{ document.getElementById("kount-ddc-form").submit(); }}
    }}
  }};
  kountSDK(kountConfig, "{session_id}");
</script>
</body></html>"#
    )
}

/// Escape a value for safe inclusion inside a double-quoted JavaScript string
/// literal (prevents breaking out of the string or the surrounding `<script>`).
fn js_string_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape a value for safe inclusion inside a double-quoted HTML attribute.
fn html_attr_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Decode the (unverified) OAuth access-token JWT claims. Reads the payload
/// segment only — no signature verification.
fn access_token_claims(access_token: &str) -> Option<serde_json::Value> {
    use base64::Engine;
    let payload_segment = access_token.split('.').nth(1)?;
    let decoded = BASE64_ENGINE_URL_SAFE_NO_PAD.decode(payload_segment).ok()?;
    serde_json::from_slice(&decoded).ok()
}

/// Extract the Kount-assigned client/merchant id from the OAuth access token
/// (the JWT `client_id` claim) for use as the DDC SDK `clientID`.
pub fn client_id_from_access_token(access_token: &str) -> Option<String> {
    access_token_claims(access_token)?
        .get("client_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Whether the access token was issued by the Kount **sandbox** authorization
/// server — this drives the DDC SDK `environment` (`TEST` vs `PROD`) so it always
/// matches the environment the Orders API call runs against. The token's `iss`
/// claim embeds the authorization-server id; sandbox uses
/// [`KOUNT_SANDBOX_AUTH_SERVER_ID`]. Defaults to sandbox (`TEST`) when the issuer
/// cannot be determined, which is the safe default.
fn access_token_is_sandbox(access_token: &str) -> bool {
    access_token_claims(access_token)
        .and_then(|c| {
            c.get("iss")
                .and_then(|v| v.as_str())
                .map(|iss| iss.contains(KOUNT_SANDBOX_AUTH_SERVER_ID))
        })
        .unwrap_or(true)
}

/// Resolve the Kount order id for an Update Order (`PATCH .../orders/{id}`) call.
/// Prefers the Kount-assigned `frm_transaction_id` (the `order.orderId` returned
/// by Evaluate Order); falls back to the connector transaction id.
fn kount_order_id(
    frm_transaction_id: Option<&str>,
    connector_transaction_id: Option<&str>,
) -> CustomResult<String, IntegrationError> {
    frm_transaction_id
        .or(connector_transaction_id)
        .map(|s| s.to_string())
        .ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "frm_transaction_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Kount Update Order requires the Kount order id \
                         (frm_transaction_id from Evaluate Order) or a connector_transaction_id"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into(),
        )
}

// Kount Orders amounts are sent as strings in the smallest currency unit.
macros::create_amount_converter_wrapper!(connector_name: Kount, amount_type: StringMinorUnit);

macros::create_all_prerequisites!(
    connector_name: Kount,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: KountTokenRequest,
            response_body: KountTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: PreRiskCheck,
            request_body: KountEvaluateOrderRequest,
            response_body: KountOrderResponse,
            router_data: RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ),
        (
            flow: FrmPaymentOutcome,
            request_body: KountUpdateOrderRequest,
            response_body: KountUpdateOrderResponse,
            router_data: RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        ),
        (
            flow: FrmRefundProcessed,
            request_body: KountRefundUpdateRequest,
            response_body: KountRefundUpdateResponse,
            router_data: RouterDataV2<FrmRefundProcessed, FrmFlowData, FrmRefundProcessedRequest, FrmRefundProcessedResponse>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        /// Bearer header for the Kount Orders API, using the access token that
        /// the framework threads onto `FrmFlowData.access_token` from the
        /// request's `state.access_token`.
        fn frm_bearer_header(
            &self,
            token: Option<&hyperswitch_masking::Secret<String>>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = token.ok_or(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "missing Kount bearer token: FrmFlowData.access_token was not \
                         populated (provide it via the request state.access_token)"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            })?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", token.peek()).into_masked(),
                ),
            ])
        }
    }
);

// =============================================================================
// CONNECTOR COMMON
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Kount<T>
{
    fn id(&self) -> &'static str {
        "kount"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.kount.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Basic auth used by the OAuth token endpoint; `api_key` is the
        // base64(clientId:clientSecret) "API Key".
        let auth = kount::KountAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {}", auth.api_key.expose()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: kount::KountErrorResponse =
            res.response
                .parse_struct("KountErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code,
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Kount<T>
{
}

// =============================================================================
// AGGREGATE + BASE (NON-FLOW) TRAITS
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Kount<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Kount<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }

    fn next_authentication_step(
        &self,
        _auth_type: common_enums::AuthenticationType,
        _payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        _completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};
        // Kount runs PreAuthenticate (DDC) first; the composite loop breaks once
        // the DDC `redirection_data` is present. FRM risk checks run separately
        // via the FraudAndRiskManagementService composite flow.
        match redirect_state {
            RedirectState::InitialRequest => AuthenticationStep::PreAuthenticate,
            _ => AuthenticationStep::Authorize,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Kount<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Kount<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Kount<T>
{
}

// =============================================================================
// REAL FLOW: PreAuthenticate = Device Data Collection (no outbound call)
// =============================================================================
// Returns only the Kount Web Client SDK HTML for the shopper's browser; makes no
// server-side call to Kount. `build_request_v2` returns `None` and
// `should_trigger_handle_response_without_body` opts the flow into the harness's
// "build the response locally" path, where `handle_response_v2` synthesises the
// DDC HTML. `clientID`/`environment` come from the access token (threaded via
// `state.access_token`); `sessionID` derives from `connector_request_reference_id`
// so it matches the Evaluate Order `deviceSessionId`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Kount<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Kount<T>
{
    fn should_trigger_handle_response_without_body(&self) -> bool {
        true
    }

    fn build_request_v2(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        // No outbound call: the DDC HTML is built locally in `handle_response_v2`.
        Ok(None)
    }

    fn get_url(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        // Never called (build_request_v2 returns None); present to satisfy the trait.
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "pre_authenticate_url",
            IntegrationErrorContext {
                additional_context: Some(
                    "Kount PreAuthenticate makes no outbound call (local DDC HTML only)".to_owned(),
                ),
                ..Default::default()
            },
        )
        .into())
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
        _event_builder: Option<&mut events::Event>,
        _res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        // sessionID derives from the same id Evaluate Order uses as deviceSessionId.
        let session_id =
            kount::to_session_id(&data.resource_common_data.connector_request_reference_id);
        // Access token threaded via state.access_token → PaymentFlowData.access_token.
        let token = data
            .resource_common_data
            .access_token
            .as_ref()
            .map(|t| t.access_token.peek().to_owned());
        let client_id = token
            .as_deref()
            .and_then(client_id_from_access_token)
            .unwrap_or_default();
        // DDC `environment` follows the access token's environment, not a hardcode.
        let sandbox = token
            .as_deref()
            .map(access_token_is_sandbox)
            .unwrap_or(true);
        let return_url = data.request.router_return_url.as_ref().map(|u| u.as_str());
        let html = build_ddc_html(&client_id, &session_id, sandbox, return_url);

        let mut router_data = data.clone();
        router_data.resource_common_data.status =
            common_enums::AttemptStatus::DeviceDataCollectionPending;
        router_data.response = Ok(PaymentsResponseData::PreAuthenticateResponse {
            resource_id: None,
            authentication_data: None,
            redirection_data: Some(Box::new(RedirectForm::Html { html_data: html })),
            connector_response_reference_id: Some(
                data.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            status_code: 200,
        });
        Ok(router_data)
    }
}

// ===== PAYOUT (no-op) IMPLEMENTATIONS =====
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Kount,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== NOT-IMPLEMENTED PAYMENT FLOW STUBS =====
// All ConnectorServiceTrait flows except ServerAuthenticationToken and
// PreAuthenticate (which are real flows).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Kount,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        Authorize,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PaymentMethodEligibility,
        PSync,
        PaymentMethodToken,
        VoidPC,
        VoidPostRefund,
        Void,
        RSync,
        Refund,
        RepeatPayment,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);

// =============================================================================
// REAL FLOW: ServerAuthenticationToken (OAuth client-credentials)
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Kount<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Kount,
    curl_request: FormUrlEncoded(KountTokenRequest),
    curl_response: KountTokenResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            FORM_URL_ENCODED
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = kount::KountAuthType::try_from(&req.connector_config)?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    FORM_URL_ENCODED.to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Basic {}", auth.api_key.expose()).into_masked(),
                ),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Auth-server id is account/environment specific; use the configured
            // value and fall back to the sandbox server only when unset.
            let auth = kount::KountAuthType::try_from(&req.connector_config)?;
            let auth_server_id = auth
                .auth_server_id
                .as_deref()
                .unwrap_or(KOUNT_SANDBOX_AUTH_SERVER_ID);
            Ok(format!(
                "{KOUNT_LOGIN_BASE_URL}/oauth2/{auth_server_id}/v1/token"
            ))
        }
    }
);

// =============================================================================
// REAL FLOW: PreRiskCheck = Evaluate Order (POST /commerce/v2/orders)
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PreRiskCheckV2 for Kount<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kount,
    curl_request: Json(KountEvaluateOrderRequest),
    curl_response: KountOrderResponse,
    flow_name: PreRiskCheck,
    resource_common_data: FrmFlowData,
    flow_request: PreRiskCheckRequest,
    flow_response: PreRiskCheckResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.frm_bearer_header(
                req.resource_common_data
                    .access_token
                    .as_ref()
                    .map(|t| &t.access_token),
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ) -> CustomResult<String, IntegrationError> {
            // `?riskInquiry=true` is required for Kount to return a risk decision
            // (omniscore/decision); without it the order is logged but riskInquiry is null.
            Ok(format!(
                "{}{}?riskInquiry=true",
                req.resource_common_data.connectors.kount.base_url, KOUNT_ORDERS_PATH
            ))
        }
    }
);

// =============================================================================
// REAL FLOW: FrmPaymentOutcome (Notify: payment succeeded) = Update Order
//            PATCH /commerce/v2/orders/{orderId}
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmPaymentOutcomeV2 for Kount<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kount,
    curl_request: Json(KountUpdateOrderRequest),
    curl_response: KountUpdateOrderResponse,
    flow_name: FrmPaymentOutcome,
    resource_common_data: FrmFlowData,
    flow_request: FrmPaymentOutcomeRequest,
    flow_response: FrmPaymentOutcomeResponse,
    http_method: Patch,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.frm_bearer_header(
                req.resource_common_data
                    .access_token
                    .as_ref()
                    .map(|t| &t.access_token),
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}/{}",
                req.resource_common_data.connectors.kount.base_url,
                KOUNT_ORDERS_PATH,
                kount_order_id(req.request.frm_transaction_id.as_deref(), req.request.connector_transaction_id.as_deref())?
            ))
        }
    }
);

// =============================================================================
// REAL FLOW: FrmRefundProcessed (Notify: refund) = Update Order
//            PATCH /commerce/v2/orders/{orderId}
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmRefundProcessedV2 for Kount<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kount,
    curl_request: Json(KountRefundUpdateRequest),
    curl_response: KountRefundUpdateResponse,
    flow_name: FrmRefundProcessed,
    resource_common_data: FrmFlowData,
    flow_request: FrmRefundProcessedRequest,
    flow_response: FrmRefundProcessedResponse,
    http_method: Patch,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<FrmRefundProcessed, FrmFlowData, FrmRefundProcessedRequest, FrmRefundProcessedResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.frm_bearer_header(
                req.resource_common_data
                    .access_token
                    .as_ref()
                    .map(|t| &t.access_token),
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<FrmRefundProcessed, FrmFlowData, FrmRefundProcessedRequest, FrmRefundProcessedResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}/{}",
                req.resource_common_data.connectors.kount.base_url,
                KOUNT_ORDERS_PATH,
                kount_order_id(req.request.frm_transaction_id.as_deref(), req.request.connector_transaction_id.as_deref())?
            ))
        }
    }
);

// =============================================================================
// FRM SERVICE TRAIT + NOT-IMPLEMENTED FRM FLOW STUBS
// =============================================================================
// FrmServiceTrait requires the remaining FRM markers. `expand_flow_status_impl!`
// has no arms for FRM flows, so these stubs are hand-written.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmServiceTrait for Kount<T>
{
}

macro_rules! kount_frm_not_implemented {
    ($flow:ty, $req:ty, $resp:ty, $name:literal) => {
        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
            ConnectorIntegrationV2<$flow, FrmFlowData, $req, $resp> for Kount<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<$flow, FrmFlowData, $req, $resp>,
            ) -> CustomResult<String, IntegrationError> {
                Err(IntegrationError::connector_flow_not_implemented(
                    ConnectorCommon::id(self),
                    $name,
                    IntegrationErrorContext {
                        additional_context: Some(format!(
                            "Kount does not implement the `{}` flow",
                            $name
                        )),
                        ..Default::default()
                    },
                )
                .into())
            }
        }
    };
}

// PostRiskCheck is unused by Kount: the post-decision Update Order is driven by
// the Notify flows (FrmPaymentOutcome / FrmRefundProcessed) above, not by
// PostRiskCheck. ChargebackReceived is not supported.
kount_frm_not_implemented!(
    PostRiskCheck,
    PostRiskCheckRequest,
    PostRiskCheckResponse,
    "post_risk_check"
);
kount_frm_not_implemented!(
    FrmChargebackReceived,
    FrmChargebackReceivedRequest,
    FrmChargebackReceivedResponse,
    "frm_chargeback_received"
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PostRiskCheckV2 for Kount<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmChargebackReceivedV2 for Kount<T>
{
}
