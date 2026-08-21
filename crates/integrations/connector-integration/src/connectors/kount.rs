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
    KountEvaluateOrderRequest, KountFrmPaymentOutcomeResponse, KountFrmRefundProcessedResponse,
    KountPreRiskCheckResponse, KountRefundUpdateRequest, KountTokenRequest, KountTokenResponse,
    KountUpdateOrderRequest,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Kount endpoints / constants.
/// Production OAuth token endpoint path (Equifax PingFederate), appended to the
/// login host configured as `kount.secondary_base_url`.
const KOUNT_TOKEN_PATH_PROD: &str = "/as/token";
/// Sandbox OAuth authorization-server id (from the Kount integration guide),
/// used to build the Okta token path when the connector config carries no
/// `auth_server_id`.
const KOUNT_SANDBOX_AUTH_SERVER_ID: &str = "ausdppkujzCPQuIrY357";
const KOUNT_ORDERS_PATH: &str = "/commerce/v2/orders";
const FORM_URL_ENCODED: &str = "application/x-www-form-urlencoded";

/// Kount Web Client SDK (`@kount/kount-web-client-sdk`), hosted as a browser
/// ESM bundle by jsDelivr. Self-contained `<script type="module">` load — no
/// bundler required on the merchant page.
const KOUNT_WEB_SDK_URL: &str =
    "https://cdn.jsdelivr.net/npm/@kount/kount-web-client-sdk@2.2.3/+esm";

/// Build the Device Data Collection (DDC) script snippet returned by the
/// PreAuthenticate step. Rendered in the shopper's browser; makes **no**
/// server-side call to Kount. Follows the Kount Web Client SDK contract:
/// `clientID` is the Kount-assigned merchant/client id, `environment` is
/// `TEST`/`PROD`, callbacks live inside the config object, and the session id
/// is passed as the second argument to `kountSDK(config, sessionID)`.
///
/// Returns only the `<script>` tag — no wrapping HTML document and no
/// `<form>`. The embedding page owns the client-side contract: on
/// `collect-end` the script submits `#kount-ddc-form` if the page provides
/// one (with its own `action` pointing wherever it wants to continue on
/// completion), falling back to the first `<form>` on the page otherwise.
/// This function takes no `return_url`/continuation URL — Kount is not
/// involved in that hop and never receives
/// it; DDC correlates purely by `sessionID`.
pub fn build_ddc_script(client_id: &str, session_id: &str, sandbox: bool) -> String {
    let environment = if sandbox { "TEST" } else { "PROD" };
    // Contextual output-encoding: `client_id` (from the access-token JWT) and
    // `session_id` are interpolated into a JS string literal — encode for that
    // context so no value can break out of the string.
    let client_id = js_string_escape(client_id);
    let session_id = js_string_escape(session_id);
    format!(
        r#"<script type="module">
  import kountSDK from "{KOUNT_WEB_SDK_URL}";
  const kountConfig = {{
    clientID: "409067439386406",
    environment: "{environment}",
    isSinglePageApp: false,
    isDebugEnabled: false,
    callbacks: {{
      "collect-end": function () {{ const f = document.getElementById("kount-ddc-form"); if (f) {{ f.submit(); }} else {{ document.querySelector("form").submit(); }} }}
    }}
  }};
  kountSDK(kountConfig, "{session_id}");
</script>"#
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
            response_body: KountPreRiskCheckResponse,
            router_data: RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ),
        (
            flow: FrmPaymentOutcome,
            request_body: KountUpdateOrderRequest,
            response_body: KountFrmPaymentOutcomeResponse,
            router_data: RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        ),
        (
            flow: FrmRefundProcessed,
            request_body: KountRefundUpdateRequest,
            response_body: KountFrmRefundProcessedResponse,
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
        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
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
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
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
        use domain_types::connector_types::RawConnectorRequestResponse;

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
        // DDC `environment` follows the deployment environment: only a production
        // deployment gets PROD, everything else (development/sandbox) gets TEST.
        // Deliberately `!matches!(.., Production)` rather than listing the non-prod
        // variants, so a future `Env` variant defaults to TEST — the safe direction.
        let sandbox = !matches!(
            common_utils::consts::Env::current_env(),
            common_utils::consts::Env::Production
        );
        let script = build_ddc_script(&client_id, &session_id, sandbox);

        let mut router_data = data.clone();
        router_data.resource_common_data.status =
            common_enums::AttemptStatus::DeviceDataCollectionPending;
        router_data.response = Ok(PaymentsResponseData::PreAuthenticateResponse {
            resource_id: None,
            authentication_data: None,
            redirection_data: Some(Box::new(RedirectForm::Script {
                script_data: script,
            })),
            connector_response_reference_id: Some(
                data.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            status_code: 200,
        });
        router_data
            .resource_common_data
            .set_typed_connector_response(None);
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
            // token endpoint host is never guessed. The path is environment
            // specific and is selected below.
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
                                "Kount needs secondary_base_url (the OAuth login host: \
                                 https://login.equifax.com in production, \
                                 https://login.kount.com in sandbox) to build the token \
                                 endpoint"
                                    .to_owned(),
                            ),
                            suggested_action: Some(
                                "Set kount.secondary_base_url in the connector config".to_owned(),
                            ),
                            doc_url: Some(kount::KOUNT_DOC_URL.to_owned()),
                        },
                    }
                })?;
            // Sandbox and production do not share a token endpoint shape: production
            // is Equifax PingFederate (`/as/token`), sandbox is Kount's Okta
            // authorization server, whose path embeds the account's auth-server id.
            // Matched with a `_` arm rather than by listing the non-prod variants so
            // a future `Env` variant defaults to sandbox — the safe direction, same
            // reasoning as the DDC `environment` flag above.
            //
            // NOTE: the host comes from config and the path from `Env`, so the two
            // must agree; see the comment on `kount.secondary_base_url` in the
            // environment config files.
            match common_utils::consts::Env::current_env() {
                common_utils::consts::Env::Production => {
                    Ok(format!("{login_base_url}{KOUNT_TOKEN_PATH_PROD}"))
                }
                _ => {
                    let auth = kount::KountAuthType::try_from(&req.connector_config)?;
                    let auth_server_id = auth
                        .auth_server_id
                        .as_deref()
                        .unwrap_or(KOUNT_SANDBOX_AUTH_SERVER_ID);
                    Ok(format!("{login_base_url}/oauth2/{auth_server_id}/v1/token"))
                }
            }
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
    curl_response: KountPreRiskCheckResponse,
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
    curl_response: KountFrmPaymentOutcomeResponse,
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
    curl_response: KountFrmRefundProcessedResponse,
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
