pub mod crypto;
pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, request::Method, types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as boost, BoostPaymentInitRequest, BoostPaymentInitResponse, BoostPaymentSyncResponse,
    BoostReversalRequest, BoostReversalResponse,
    BoostReversalResponse as BoostReversalSyncResponse, BoostWebhookBody,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const USER_AGENT: &str = "User-Agent";
}

// BCPG's staging environment sits behind a CDN whose bot-protection blocks requests
// with no User-Agent header (reqwest sends none by default) — direct Postman calls
// work because Postman sets its own. A realistic browser UA avoids that block.
const BOOST_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Boost<T>
{
    fn id(&self) -> &'static str {
        "boost"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.boost.base_url.as_ref()
    }

    // NOTE: BCPG doesn't use a static auth header — every request is signed
    // per-request with an HMAC-SHA256 over `method + path + body` (see
    // `BoostAuthType::build_signed_auth_header` in transformers.rs), which needs
    // the request URL/body that this trait method doesn't have access to. The
    // real `Authorization` header is built in `build_headers`/`build_headers_refund`
    // below and used by every flow's `get_headers`. This impl only exists to
    // satisfy the `ConnectorCommon` trait and is not relied upon for real requests.
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = boost::BoostAuthType::try_from(auth_type).change_context(
            errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure this merchant account's Boost connector with a client_id \
                         and merchant_secret."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "ConnectorCommon::get_auth_header (unused placeholder — Boost signs \
                         per-request in build_headers/build_headers_refund instead) could not \
                         build BoostAuthType from the provided connector config."
                            .to_string(),
                    ),
                },
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.client_id.peek().to_owned().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        // BCPG normally returns its documented JSON error shape (section 3.8), but
        // intermediary infra (CDN/WAF block pages, gateway timeouts, etc.) can return
        // plain HTML/text instead. Degrade gracefully in that case rather than failing
        // the whole error path with a deserialization error that hides the real status.
        match res
            .response
            .parse_struct::<boost::BoostErrorResponse>("BoostErrorResponse")
        {
            Ok(response) => {
                with_error_response_body!(event_builder, response);

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: response
                        .error
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: response
                        .message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response.message,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Err(_) => {
                let raw_body = String::from_utf8_lossy(&res.response).to_string();
                tracing::warn!(
                    status_code = res.status_code,
                    body = %raw_body,
                    "Boost returned a non-JSON error body (likely an intermediary CDN/WAF/gateway response, not BCPG itself)"
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: common_utils::consts::NO_ERROR_CODE.to_string(),
                    message: common_utils::consts::NO_ERROR_MESSAGE.to_string(),
                    reason: Some(raw_body.chars().take(500).collect()),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
        }
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Boost<T>
{
}

// =============================================================================
// AMOUNT CONVERTER
// =============================================================================
// BCPG amounts are unquoted JSON decimals (e.g. "amount": 1.00), i.e. major-unit
// floats, not minor-unit integers or quoted strings.
macros::create_amount_converter_wrapper!(connector_name: Boost, amount_type: FloatMajorUnit);

// =============================================================================
// PREREQUISITES: struct, flow bridges, shared header-signing helpers
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Boost,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: BoostPaymentInitRequest,
            response_body: BoostPaymentInitResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: BoostPaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BoostReversalRequest,
            response_body: BoostReversalResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: BoostReversalSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
            let method = self.get_http_method();
            let body = match method {
                Method::Get => String::default(),
                Method::Post | Method::Put | Method::Delete | Method::Patch => self
                    .get_request_body(req)?
                    .map(|content| content.content.get_inner_value().peek().to_owned())
                    .unwrap_or_default(),
            };
            self.build_headers_with_body(req, &body)
        }

        /// Same as `build_headers`, but signs a body the caller already built
        /// instead of calling `get_request_body` again. Authorize's
        /// `build_request_v2` uses this so `get_request_body` — which runs
        /// Boost's card encryption — is invoked exactly once per request; see
        /// `crypto.rs` for why a second, independent encryption call would
        /// produce a body that doesn't match what was signed.
        pub fn build_headers_with_body<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
            body: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
            let method = self.get_http_method();
            let path = (self.get_url(req)?).replace(self.connector_base_url_payments(req), "");
            let auth = boost::BoostAuthType::try_from(&req.connector_config).change_context(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure this merchant account's Boost connector with a \
                             client_id and merchant_secret."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(format!(
                            "Failed to extract Boost client_id/merchant_secret while building \
                             {method} {path} headers for a payment flow (Authorize/PSync)."
                        )),
                    },
                },
            )?;
            let authorization = auth
                .build_signed_auth_header(&method.to_string(), &path, body)
                .change_context(errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(format!(
                            "Failed to compute the HMAC-SHA256 request signature for \
                             {method} {path} (Boost payment flow)."
                        )),
                    },
                })?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (headers::ACCEPT.to_string(), "application/json".to_string().into()),
                (headers::USER_AGENT.to_string(), BOOST_USER_AGENT.to_string().into()),
                (headers::AUTHORIZATION.to_string(), authorization.into_masked()),
            ])
        }

        pub fn build_headers_refund<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, RefundFlowData, Req, Res>,
        {
            let method = self.get_http_method();
            let body = match method {
                Method::Get => String::default(),
                Method::Post | Method::Put | Method::Delete | Method::Patch => self
                    .get_request_body(req)?
                    .map(|content| content.content.get_inner_value().peek().to_owned())
                    .unwrap_or_default(),
            };
            let path = (self.get_url(req)?).replace(self.connector_base_url_refunds(req), "");
            let auth = boost::BoostAuthType::try_from(&req.connector_config).change_context(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure this merchant account's Boost connector with a \
                             client_id and merchant_secret."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(format!(
                            "Failed to extract Boost client_id/merchant_secret while building \
                             {method} {path} headers for a refund flow (Refund/RSync)."
                        )),
                    },
                },
            )?;
            let authorization = auth
                .build_signed_auth_header(&method.to_string(), &path, &body)
                .change_context(errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(format!(
                            "Failed to compute the HMAC-SHA256 request signature for \
                             {method} {path} (Boost refund flow)."
                        )),
                    },
                })?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (headers::ACCEPT.to_string(), "application/json".to_string().into()),
                (headers::USER_AGENT.to_string(), BOOST_USER_AGENT.to_string().into()),
                (headers::AUTHORIZATION.to_string(), authorization.into_masked()),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.boost.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.boost.base_url
        }
    }
);

// =============================================================================
// AUTHORIZE — POST /v1/payments/init (paymentMethod="card", hosted/3DS redirect)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Boost,
    curl_request: Json(BoostPaymentInitRequest),
    curl_response: BoostPaymentInitResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/v1/payments/init", self.connector_base_url_payments(req)))
        }

        // Overridden so `get_request_body` — which runs Boost's RSA-OAEP/
        // AES-GCM card encryption for the Direct Card Payment path — is
        // called exactly once. The default `build_request_v2` calls it once
        // directly and a second time indirectly (via `get_headers` ->
        // `build_headers`, to sign the body), and OAEP's randomized padding
        // means those two calls never produce the same ciphertext, so the
        // signed body and the sent body would silently diverge. Building the
        // body once here and threading it into signing keeps both in sync.
        fn build_request_v2(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Option<common_utils::request::Request>, errors::IntegrationError> {
            let request_data = self.get_request_body(req)?;
            let body_str = request_data
                .as_ref()
                .map(|content| content.content.get_inner_value().peek().to_owned())
                .unwrap_or_default();
            let headers = self.build_headers_with_body(req, &body_str)?;
            let (body, typed_request_value) = match request_data {
                Some(data) => (
                    Some(data.content),
                    data.typed_request.map(|msv| msv.inner().clone()),
                ),
                None => (None, None),
            };
            Ok(Some(
                common_utils::request::RequestBuilder::new()
                    .method(Method::Post)
                    .url(self.get_url(req)?.as_str())
                    .attach_default_headers()
                    .headers(headers)
                    .set_optional_body(body)
                    .set_typed_connector_request(typed_request_value)
                    .add_certificate(self.get_certificate(req)?)
                    .add_certificate_key(self.get_certificate_key(req)?)
                    .add_ca_certificate_pem(self.get_ca_certificate(req)?)
                    .build(),
            ))
        }
    }
);

// =============================================================================
// PSYNC — GET /v1/payments/refs/{referenceId}
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Boost,
    curl_response: BoostPaymentSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            // BCPG's Init response never echoes back the merchant's referenceId, so
            // it must be recovered from the same field the connector sent it in:
            // resource_common_data.connector_request_reference_id.
            let reference_id = req.resource_common_data.connector_request_reference_id.clone();
            Ok(format!(
                "{}/v1/payments/refs/{}",
                self.connector_base_url_payments(req),
                reference_id
            ))
        }
    }
);

// =============================================================================
// REFUND — POST /v1/reversals
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Boost,
    curl_request: Json(BoostReversalRequest),
    curl_response: BoostReversalResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers_refund(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/v1/reversals", self.connector_base_url_refunds(req)))
        }
    }
);

// =============================================================================
// RSYNC — GET /v1/reversals/{reversalUuid}
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Boost,
    curl_response: BoostReversalSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers_refund(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let reversal_uuid = req.request.connector_refund_id.clone();
            Ok(format!(
                "{}/v1/reversals/{}",
                self.connector_base_url_refunds(req),
                reversal_uuid
            ))
        }
    }
);

// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================
// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Boost<T>
{
}

// ===== FLOW MARKER TRAIT IMPLEMENTATIONS =====
// Required by ConnectorServiceTrait's supertrait bounds; not auto-generated by
// create_all_prerequisites! itself.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Boost<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Boost<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Boost<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Boost<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Boost<T>
{
}

// ===== INCOMING WEBHOOK IMPLEMENTATION =====
// BCPG POSTs a single payment-result payload (success or failure) to the
// `callbackUrl` supplied at Authorize/Init (see BoostPaymentInitRequest's
// callback_url field, wired from PaymentsAuthorizeData::webhook_url). The
// payload's `status` field reuses the exact same vocabulary as PSync's
// `status`, so this reuses BoostPaymentStatus and its existing
// `From<BoostPaymentStatus> for AttemptStatus` mapping rather than
// duplicating status logic.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Boost<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"timestamp":"2025-09-09T01:22:50.650398229Z","uuid":"5e3cc113-0426-44e2-985b-f90bdddd06e8","amount":1,"currency":"MYR","created":"2025-09-09T01:22:06.739Z","description":"Payment","status":"succeeded","paymentMethod":"card","referenceId":"Llof2a6I0EdVmURL7YoL"}"#
    }

    fn get_webhook_integrity_checks(&self) -> Vec<WebhookIntegrityCheck> {
        // BCPG's callback payload carries no signature/HMAC of any kind (see
        // verify_webhook_source below), so this webhook can never be
        // cryptographically verified. Require the platform to corroborate the
        // claimed connector_transaction_id and amount against an authenticated
        // PSync call (GET /v1/payments/refs/{referenceId}) before acting on
        // it. NOT declaring Currency here: WebhookDetailsResponse has no
        // currency field to expose (only amount_captured/minor_amount_captured),
        // so there is nothing for a caller to actually compare — declaring the
        // check without backing data would be misleading.
        vec![
            WebhookIntegrityCheck::ConnectorTransactionId,
            WebhookIntegrityCheck::Amount,
        ]
    }

    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        // BCPG's integration guideline (section 4.1, Message Level Security)
        // only documents outgoing merchant->BCPG request signing (HMAC-SHA256
        // over method+path+body, see BoostAuthType::build_signed_auth_header).
        // There is no documented inbound signature header, shared secret, or
        // verification mechanism for callbackUrl POSTs. Hardcode to false
        // (unverified) so callers fall back to the authenticated-PSync
        // corroboration required by get_webhook_integrity_checks above,
        // rather than fabricating a verification scheme BCPG never specified.
        Ok(false)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let payload: BoostWebhookBody = request
            .body
            .parse_struct("BoostWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse Boost webhook body to determine event type")?;
        Ok(payload.get_event_type())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let payload: BoostWebhookBody = request
            .body
            .parse_struct("BoostWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
            .attach_printable(
                "Failed to parse Boost webhook body to build the webhook reference",
            )?;
        Ok(Some(payload.into_webhook_event_reference()))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let payload: BoostWebhookBody = request
            .body
            .parse_struct("BoostWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse Boost webhook body")?;
        Ok(payload.into_webhook_details_response(200, &request.body))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<errors::WebhookError>,
    > {
        let payload: BoostWebhookBody = request
            .body
            .parse_struct("BoostWebhookBody")
            .change_context(errors::WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to parse Boost webhook resource object")?;
        Ok(Box::new(payload))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Boost<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// Non-generic marker trait required by VerifyRedirectResponse for webhook
// signature verification.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Boost<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Boost,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. `not_implemented` surfaces connector_flow_not_implemented
// (BCPG documents an API for this, it's just out of scope for this pass);
// `not_supported` surfaces connector_flow_not_supported (BCPG's documented
// API has no equivalent concept at all).
//
// Authorize, PSync, Refund, and RSync are implemented for real above (card-3DS
// redirect flow) and therefore appear in neither list.
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Boost,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        // Same /v1/reversals endpoint as Refund; deferred to a follow-up.
        Void,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
        SetupMandate,
        RepeatPayment,
        MandateRevoke
    ],
    not_supported: [
        Capture,
        VoidPC,
        VoidPostRefund,
        IncrementalAuthorization,
        CreateOrder,
        Authenticate,
        PreAuthenticate,
        PostAuthenticate,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        Accept,
        SubmitEvidence,
        DefendDispute
    ],
);
