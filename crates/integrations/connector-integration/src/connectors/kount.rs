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
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name='viewport' content='width=device-width, initial-scale=1'>
    <style type='text/css'>
        * {{
            padding: 0;
            margin: 0;
            box-sizing: border-box;
        }}

        #brandCard {{
            width: 100%;
            height: 100%;
            background-color: #ffffff;
            align-items: center;
            justify-content: center;
            display: flex;
            box-sizing: border-box;
            flex-flow: column wrap;
            position: absolute;
        }}

        body {{
            background-color: #ffffff;
            height: 100%;
            width: 100%;
            padding: 20px;
            font-family: Arial, Helvetica, Sans-Serif;
        }}

        html {{
            background-color: #f4f4f4;
            height: 100%;
        }}

        #content {{
            width: 100%;
            height: 100%;
            position: fixed;
            overflow: hidden;
            margin: 0 auto;
        }}

        .brandLoader {{
            height: 100%;
            width: 100%;
            display: flex;
            box-sizing: border-box;
            flex-flow: column wrap;
            background-color: #404040;
            align-items: center;
            justify-content: center;
            border-radius: 0px;
            flex: 0 0 auto;
        }}

        .statement {{
            font-size: 14px;
            font-family: system-ui;
            color: #444444;
            margin-top: 24px;
        }}

        #loading {{
            width: 48px;
            height: 48px;
            viewbox: 0 0 16 16;
            -webkit-animation: rotation 1.8s infinite linear;
        }}

        @-webkit-keyframes rotation {{
            from {{
                -webkit-transform: rotate(0deg);
            }}

            to {{
                -webkit-transform: rotate(359deg);
            }}
        }}

        #brandingText {{
            width: auto;
            height: 14px;
            margin-top: 16px;
        }}
    </style>
</head>
<body>
    <div class='brandLoader' id='brandLoader'>
        <div id='brandCard'>
            <img id='loading' src='data:image/png;base64,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' />
            <div class='statement'>Processing your payment</div>
        </div>
    </div>

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
</body>
</html>"#
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
                    suggested_action: Some(
                        "Send the Kount order id as frm_transaction_id (from the Pre Risk Check \
                         response), or a connector_transaction_id, on the notify request"
                            .to_owned(),
                    ),
                    doc_url: Some(kount::KOUNT_DOC_URL.to_owned()),
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
                    suggested_action: Some(
                        "Supply the Kount OAuth token in the request state.access_token".to_owned(),
                    ),
                    doc_url: Some(kount::KOUNT_DOC_URL.to_owned()),
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
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some(
                            "failed to parse the Kount error body as KountErrorResponse".to_owned(),
                        ),
                    },
                })?;

        with_error_response_body!(event_builder, response);

        let (code, message) = (response.code(), response.message());
        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            reason: Some(message.clone()),
            message,
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
        _redirect_state: connector_types::RedirectState,
        _completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        // Kount only runs PreAuthenticate (DDC); the composite loop breaks once
        // the DDC `redirection_data` is present. FRM risk checks run separately
        // via the FraudAndRiskManagementService composite flow.
        connector_types::AuthenticationStep::PreAuthenticate
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
    fn get_call_connector_action(&self) -> common_enums::CallConnectorAction {
        common_enums::CallConnectorAction::HandleResponseWithoutBuildRequest
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
                    "Kount PreAuthenticate makes no outbound call (local DDC HTML only); \
                     get_url is unreachable because build_request_v2 returns None"
                        .to_owned(),
                ),
                suggested_action: Some(
                    "No action required: the DDC HTML is built locally in handle_response_v2"
                        .to_owned(),
                ),
                doc_url: Some(kount::KOUNT_DOC_URL.to_owned()),
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
        // sessionID = hash(merchant_transaction_id), matching the Evaluate Order
        // deviceSessionId (which hashes the same merchant transaction id). Falls
        // back to the connector request reference when it is absent.
        let session_ref = data
            .request
            .merchant_transaction_id
            .clone()
            .unwrap_or_else(|| {
                data.resource_common_data
                    .connector_request_reference_id
                    .clone()
            });
        let session_id = kount::hash_session_id(&session_ref);
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
        GetConnectorCustomer,
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
            // The OAuth login host is configured via `secondary_base_url` (the
            // Orders API host is the primary `base_url`); it is required, so the
            // token endpoint host is never guessed. Auth-server id is
            // account/environment specific, falling back to the sandbox server
            // only when unset.
            let login_base_url = req
                .resource_common_data
                .connectors
                .kount
                .secondary_base_url
                .as_deref()
                .ok_or_else(|| {
                    IntegrationError::InvalidConnectorConfig {
                        config: "secondary_base_url",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Kount needs secondary_base_url (the OAuth login host, e.g. \
                                 https://login.kount.com) to build the token endpoint"
                                    .to_owned(),
                            ),
                            suggested_action: Some(
                                "Set kount.secondary_base_url in the connector config".to_owned(),
                            ),
                            doc_url: Some(kount::KOUNT_DOC_URL.to_owned()),
                        },
                    }
                })?;
            let auth = kount::KountAuthType::try_from(&req.connector_config)?;
            let auth_server_id = auth
                .auth_server_id
                .as_deref()
                .unwrap_or(KOUNT_SANDBOX_AUTH_SERVER_ID);
            Ok(format!(
                "{login_base_url}/oauth2/{auth_server_id}/v1/token"
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

// PostRiskCheck is unused by Kount: the post-decision Update Order is driven by
// the Notify flows (FrmPaymentOutcome / FrmRefundProcessed) above, not by
// PostRiskCheck. ChargebackReceived is not supported.
macros::frm_flow_not_implemented!(
    connector: Kount,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: PostRiskCheck,
    request: PostRiskCheckRequest,
    response: PostRiskCheckResponse,
    flow_name: "post_risk_check",
);
macros::frm_flow_not_implemented!(
    connector: Kount,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: FrmChargebackReceived,
    request: FrmChargebackReceivedRequest,
    response: FrmChargebackReceivedResponse,
    flow_name: "frm_chargeback_received",
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PostRiskCheckV2 for Kount<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmChargebackReceivedV2 for Kount<T>
{
}
