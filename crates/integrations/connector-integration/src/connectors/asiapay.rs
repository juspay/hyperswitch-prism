pub mod transformers;

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
};
use domain_types::{
    connector_flow::{Capture, PSync, RSync, Refund, Void},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RequestDetails, ResponseId, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
// Needed by the `create_amount_converter_wrapper!` expansion below.
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    decode::BodyDecoding,
    verification::ConnectorSourceVerificationSecrets,
    webhooks::{IncomingWebhook, IncomingWebhookFlowError},
};
use serde::Serialize;
use transformers::{
    self as asiapay, AsiapayAuthType, AsiapayCaptureRequest, AsiapayCaptureResponse,
    AsiapayPSyncRequest, AsiapayPSyncResponse, AsiapayRSyncRequest, AsiapayRSyncResponse,
    AsiapayRefundRequest, AsiapayRefundResponse, AsiapaySuccessCode, AsiapayVoidRequest,
    AsiapayVoidResponse,
};

use crate::{connectors::macros, types::ResponseRouterData};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// AsiaPay sends and receives amounts as major-unit strings with two decimals
// (`Number (12,2)`, e.g. `"80.00"`), so the converter type is StringMajorUnit.
macros::create_amount_converter_wrapper!(connector_name: Asiapay, amount_type: StringMajorUnit);

fn preprocess_merchant_api_response(
    bytes: bytes::Bytes,
    status_code: u16,
) -> CustomResult<bytes::Bytes, errors::ConnectorError> {
    // A WAF/proxy can answer an otherwise successful HTTP response with an
    // HTML page. URL-encoded parsing is permissive enough to turn markup into
    // an empty map, so reject it before a Refund/Void mapper can mistake
    // missing fields for an ordinary Pending response. Never echo the body
    // because HTML can contain payment or card data.
    if bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| byte == b'<')
    {
        return Err(error_stack::report!(
            errors::ConnectorError::response_handling_failed_with_context(
                status_code,
                Some(
                    "asiapay: merchant-API response body was HTML rather than a URL-encoded response"
                        .to_string(),
                ),
            )
        ));
    }

    let parsed: std::collections::HashMap<String, String> =
        serde_urlencoded::from_bytes(&bytes).map_err(|_| {
            error_stack::report!(errors::ConnectorError::unexpected_response_error_with_context(
                status_code,
                Some(
                    "asiapay: merchant-API response body was not valid application/x-www-form-urlencoded"
                        .to_string(),
                ),
            ))
        })?;
    serde_json::to_vec(&parsed)
        .map(bytes::Bytes::from)
        .change_context(
            errors::ConnectorError::response_handling_failed_with_context(
                status_code,
                Some(
                    "asiapay: failed to re-encode the parsed merchant-API response body"
                        .to_string(),
                ),
            ),
        )
}

// =============================================================================
// PREREQUISITES (flow bridges, connector struct, shared helpers)
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Asiapay,
    generic_type: T,
    api: [
        (
            flow: Capture,
            request_body: AsiapayCaptureRequest,
            response_body: AsiapayCaptureResponse,
            // The Merchant API answers a URL-encoded `k=v&...` body. The flow
            // opts into `preprocess_response`, whose member function re-encodes
            // the body as JSON before the framework deserializer runs, so no
            // `response_format` switch is needed here.
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AsiapayVoidRequest,
            response_body: AsiapayVoidResponse,
            // Same URL-encoded merchant-API body shape as Capture/Refund —
            // handled by the shared preprocess_response member function below.
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AsiapayRefundRequest,
            response_body: AsiapayRefundResponse,
            // Same URL-encoded merchant-API body shape as Capture — handled by
            // the shared preprocess_response member function below.
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AsiapayPSyncRequest,
            response_body: AsiapayPSyncResponse,
            // AsiaPay's Merchant API Query answers an XML body, deserialized
            // via quick-xml (same mechanism worldpayxml uses).
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            request_body: AsiapayRSyncRequest,
            response_body: AsiapayRSyncResponse,
            // RSync is the SAME Merchant API Query as PSync (keyed by payRef),
            // answering the same XML envelope — deserialized via quick-xml.
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            // BodyKey auth: AsiaPay credentials travel inside the URL-encoded
            // request body, so the only header is the content type.
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            header.append(&mut self.get_auth_header(&req.connector_config)?);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.asiapay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.asiapay.base_url
        }

        /// Merchant API (`orderApi.jsp`: Capture/Void/Refund) responses are
        /// URL-encoded `k=v&...` bodies. Re-encode them as JSON for the flow
        /// response deserializers. Authorize no longer reaches this hook: it
        /// is a local native-form flow with no outbound gateway response.
        pub fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            status_code: u16,
        ) -> CustomResult<bytes::Bytes, errors::ConnectorError> {
            preprocess_merchant_api_response(bytes, status_code)
        }
    }
);

// =============================================================================
// FLOW IMPLEMENTATIONS
// =============================================================================
// Card DPC and LINE Pay CPTB are both local Authorize flows. UCS builds and
// signs the same typed form request, returns it as a native auto-submit HTML
// POST form, and performs no gateway request; the payer's browser posts it.
// Request construction runs in validate_request so typing errors stay
// IntegrationError. Rendering stays in handle_response_v2, which returns 200
// with AuthenticationPending until PSync/Data Feed supplies gateway truth.
macros::macro_connector_local_flow_implementation!(
    connector: Asiapay,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    handle_response: asiapay::handle_authorize_response,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        validate_request: asiapay::validate_authorize_request,
    }
);

// Capture: Merchant API (`actionType=Capture`) — captures a previously
// authorized amount (`payType=H`) by `payRef`, partial capture allowed.
// Response is URL-encoded, so `preprocess_response: true` is set: the shared
// preprocessor above re-encodes the `k=v&...` body as JSON.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayCaptureRequest),
    curl_response: AsiapayCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/eng/merchant/api/orderApi.jsp"))
        }
    }
);

// Void: amountless Merchant API `actionType=OnlineReversal` — reverses a
// not-yet-settled payment (authorize-hold `payType=H`, or a same-day sale) by
// `payRef`. `actionType=Void` cannot be used for the authorize-hold case. Only valid
// BEFORE AsiaPay's settlement cut-off: once the batch has settled the gateway
// rejects the void (`resultCode=-1`) and the remedy is a Refund. Response is
// URL-encoded, so `preprocess_response: true` reuses the shared merchant-API
// preprocessor, exactly like Capture/Refund.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayVoidRequest),
    curl_response: AsiapayVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/eng/merchant/api/orderApi.jsp"))
        }
    }
);

// Refund: Merchant API (`actionType=RequestRefund`) — refunds (fully or
// partially) a settled payment by `payRef`. The gateway only ACCEPTS the
// refund asynchronously (`orderStatus=RequestRefund` → `RefundStatus::Pending`);
// RSync observes the terminal state. Response is URL-encoded, so
// `preprocess_response: true` reuses the shared merchant-API preprocessor,
// exactly like Capture.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayRefundRequest),
    curl_response: AsiapayRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/eng/merchant/api/orderApi.jsp"))
        }
    }
);

// PSync: Merchant API Query (`actionType=Query`) — polls the current state of
// a payment by orderRef (merchant reference). Response is XML, handled by the
// `response_format: xml` prerequisite above (quick-xml), so NO
// `preprocess_response: true` here: that hook only serves URL-encoded
// Merchant API mutation responses, not XML.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayPSyncRequest),
    curl_response: AsiapayPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
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
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/eng/merchant/api/orderApi.jsp"))
        }
    }
);

// RSync: Merchant API Query (`actionType=Query`) — the SAME endpoint as
// PSync, but keyed by `payRef` (AsiaPay's Query accepts orderRef XOR payRef;
// payRef keys the precise payment the refund applies to). AsiaPay issues no
// distinct refund identifier: the Refund response stores the payment's payRef
// on `connector_refund_id`, so RSync queries by that carrier. The refund is
// ACCEPTED asynchronously by `RequestRefund`; RSync observes the terminal
// state (`Refunded` / `PartialRefunded` / `Accepted_Adj` → Success,
// `Rejected` → Failure). Response is the same XML envelope as PSync, handled
// by the `response_format: xml` prerequisite (quick-xml), so NO
// `preprocess_response: true` here either.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiapayRSyncRequest),
    curl_response: AsiapayRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/eng/merchant/api/orderApi.jsp"))
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Asiapay<T>
{
    fn id(&self) -> &'static str {
        "asiapay"
    }

    // AsiaPay amounts are major-unit strings ("80.00"), i.e. the connector's
    // currency unit is the BASE unit; the concrete conversion type
    // (StringMajorUnit) is set on the `AsiapayAmountConvertor` above.
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.asiapay.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        // BodyKey auth: AsiaPay credentials (merchantId / loginId / password /
        // secureHash) travel inside the URL-encoded request body, never in
        // HTTP headers.
        Ok(Vec::new())
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        // AsiaPay's error/result payloads are in-band and URL-encoded; every
        // endpoint answers HTTP 200 and carries the outcome in the body keys
        // (`errMsg`, `prc`, `src`, `successcode` / `resultCode` /
        // `orderStatus`). This path is only reached for genuine
        // transport-level failures, whose bodies may not even be AsiaPay's
        // shape (a WAF/gateway HTML page), so parse leniently: an unparseable
        // body degrades to the NO_ERROR_* sentinels instead of becoming a
        // hard deserialization error.
        let response: asiapay::AsiapayErrorResponse = serde_urlencoded::from_bytes(&res.response)
            .unwrap_or_else(|_| asiapay::AsiapayErrorResponse::default());

        Ok(ErrorResponse {
            status_code: res.status_code,
            // Never `.unwrap_or_default()` here: an empty `code`/`message` is
            // indistinguishable from a connector that really returned "". The
            // NO_ERROR_CODE / NO_ERROR_MESSAGE sentinels are what the rest of
            // the platform greps for.
            code: response.prc.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .err_msg
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            // This `build_error_response` is shared by EVERY flow of this
            // connector - Authorize, PSync, Capture, Void, Refund, RSync - so
            // attempt_status stays None: a status forced here (e.g. a terminal
            // payment failure) would be wrong for the other flows.
            // Flow-specific classification belongs in each flow's own
            // response handler (exemplars: connectors/flywire.rs,
            // connectors/noon.rs `attempt_status: None`).
            attempt_status: None,
            ..Default::default()
        })
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Asiapay<T>
{
}
// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================
// Auto-generated by add_connector.sh using the macro-based pattern. All flow
// traits not implemented above are stubbed via
// `macros::macro_connector_flow_status_impls!` with `not_implemented` status,
// which emits both the marker-trait impl and a stub `ConnectorIntegrationV2`
// impl per flow.
//
// To implement a flow:
//   1. Remove that flow's name from the `not_implemented` list below.
//   2. Add a manual marker-trait impl, e.g.
//        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
//            connector_types::PaymentAuthorizeV2<T> for Asiapay<T> {}
//   3. Add a `macros::macro_connector_implementation!(...)` block with the
//      flow's request/response types, HTTP method, and `get_url`/`get_headers`.
//      For a flow with NO outbound HTTP call (everything decided locally), use
//      `macros::macro_connector_local_flow_implementation!` instead - its keys
//      are connector / flow_name / resource_common_data / flow_request /
//      flow_response / handle_response / generic_type / [bounds], and it wires
//      `CallConnectorAction::HandleResponseWithoutBuildRequest` for you.
//
// See crates/integrations/connector-integration/src/connectors/xendit.rs for a
// reference implementation that follows this pattern.
// =============================================================================

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Asiapay<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Asiapay<T>
{
}

// ===== INCOMING WEBHOOK (AsiaPay "Data Feed") =====
// Webhooks are NOT a ConnectorIntegrationV2 flow: no connector_flow marker, no
// macro, no entry in `macro_connector_flow_status_impls!` — a plain
// `impl IncomingWebhook`, required by ConnectorServiceTrait. Two RPCs, two
// phases (tech spec §7): `EventService.ParseEvent` is stateless
// (`get_event_type` + `get_webhook_event_reference`), `EventService.HandleEvent`
// has credentials (`verify_webhook_source` + `process_payment_webhook`).
// `EventService/HandleEvent` is deliberately NOT declared in this connector's
// specs.json — it may only be declared with a real captured sandbox
// webhook_payload.json, which does not exist yet, so webhook certification is
// deferred.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> IncomingWebhook
    for Asiapay<T>
{
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body = asiapay::parse_asiapay_webhook_body(&request.body)?;

        // Object reference (tech spec §7): AsiaPay's payment-gateway reference
        // (PayRef) is the lookup key, falling back to the merchant order
        // reference (Ref/orderRef) when the gateway reference is absent. Ref
        // additionally fills the merchant slot — it IS the orderRef sent on
        // the original Authorize.
        match body.pay_ref.clone().or_else(|| body.order_ref.clone()) {
            Some(transaction_id) => Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(transaction_id),
                    merchant_transaction_id: body.order_ref,
                    connector_preprocessing_id: None,
                },
            ))),
            // A feed carrying neither id is not actionable.
            None => Ok(None),
        }
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        // Stateless: the payload alone decides (this is called in BOTH the
        // ParseEvent and the HandleEvent phase — never touch secrets here).
        let body = asiapay::parse_asiapay_webhook_body(&request.body)?;
        Ok(asiapay::map_asiapay_webhook_event_type(&body))
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        // Euler never sends `connector_webhook_secret`: the Secure Hash Secret
        // — and the merchantId for the identity cross-check — is sourced from
        // the connector account config instead, same pattern as
        // payhere/grabpay/revolut.
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        let body = asiapay::parse_asiapay_webhook_body(&request.body)?;

        let connector_account_details = connector_account_details
            .ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookVerificationSecretNotFound)
            })
            .attach_printable(
                "asiapay: connector account details are required to verify a data-feed webhook \
                 (they carry the Secure Hash Secret and the merchantId)",
            )?;
        let auth = AsiapayAuthType::try_from(&connector_account_details)
            .change_context(errors::WebhookError::WebhookVerificationSecretInvalid)?;

        // Merchant identity cross-check: the MerchantId posted on the feed
        // must be the merchant these credentials belong to.
        let posted_merchant_id = body
            .merchant_id
            .as_ref()
            .ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookSourceVerificationFailed)
            })
            .attach_printable(
                "asiapay: data-feed body is missing `MerchantId`; cannot cross-check the \
                 merchant identity",
            )?;
        if posted_merchant_id != auth.merchant_id.peek() {
            return Ok(false);
        }

        let posted_hash = body
            .secure_hash
            .as_ref()
            .ok_or_else(|| error_stack::report!(errors::WebhookError::WebhookSignatureNotFound))
            .attach_printable(
                "asiapay: data-feed body carried no `secureHash`; cannot verify the webhook \
                 source",
            )?;

        let computed_hash = asiapay::compute_asiapay_webhook_secure_hash(
            &body,
            auth.secure_hash_secret.peek(),
            auth.secure_hash_algorithm,
        )?;

        // The digest is hex on both sides; compare case-insensitively because
        // the docs do not pin the gateway's hex casing.
        Ok(computed_hash.eq_ignore_ascii_case(posted_hash))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let raw_body = String::from_utf8_lossy(&request.body).to_string();
        let body = asiapay::parse_asiapay_webhook_body(&request.body)?;

        // successcode: 0 = succeeded, 1 = failure, 2 = pending, others = error
        // (AsiaPay payment status classification). The data feed carries NO
        // payType, so a successful authorize-hold (payType=H) cannot be told
        // apart from a completed sale here: map `Succeeded` to the generic
        // success status (Charged) — order-level sync (PSync, Merchant API
        // Query orderStatus: Authorized vs Captured) remains the authority on
        // preauth vs capture. Exhaustive over the named AsiapaySuccessCode
        // variants on purpose — no `_ =>` arm.
        let status = match body
            .success_code
            .as_deref()
            .map(AsiapaySuccessCode::from_wire)
        {
            Some(AsiapaySuccessCode::Succeeded) => AttemptStatus::Charged,
            Some(AsiapaySuccessCode::Failed) => AttemptStatus::Failure,
            // "2" (pending), an unrecognised value, or a missing successcode:
            // not proof of failure — stay non-terminal; PSync reconciles.
            Some(AsiapaySuccessCode::Pending) | Some(AsiapaySuccessCode::Other) | None => {
                AttemptStatus::Pending
            }
        };

        // prc identifies the bank/payment outcome; src the system component;
        // remark is the human-readable message. Never `.unwrap_or_default()`:
        // the NO_ERROR_* sentinels keep a missing code/message distinguishable
        // from "" (same convention as this connector's build_error_response).
        let (error_code, error_message, error_reason) = if status == AttemptStatus::Failure {
            let prc = body.prc.clone();
            let src = body.src.clone();
            (
                Some(prc.clone().unwrap_or_else(|| NO_ERROR_CODE.to_string())),
                Some(
                    body.remark
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                ),
                match (prc, src) {
                    (Some(prc), Some(src)) => Some(format!("prc={prc}, src={src}")),
                    (Some(prc), None) => Some(format!("prc={prc}")),
                    (None, src) => src,
                },
            )
        } else {
            (None, None, None)
        };

        // Echoed amount/currency — present only when both keys are usable
        // (PSync stays the amount authority for this connector).
        let amount =
            asiapay::parse_asiapay_webhook_amount(body.amt.as_deref(), body.cur.as_deref())?;

        let order_ref = body.order_ref.clone();

        Ok(WebhookDetailsResponse {
            raw_connector_status: None,
            amount: amount.clone(),
            // PayRef is AsiaPay's payment-gateway reference; by contract it is
            // present on the feed, but if a quirk drops it the outcome is
            // still surfaced (PSync can recover the reference via orderRef).
            resource_id: Some(
                body.pay_ref
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
            ),
            status,
            // Ref is the orderRef we sent on Authorize — both reference slots
            // carry it (as calida does).
            connector_response_reference_id: order_ref.clone(),
            connector_request_reference_id: order_ref,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(raw_body),
            status_code: 200,
            response_headers: None,
            captured_money: if status == AttemptStatus::Charged {
                amount
            } else {
                None
            },
            // AuthId echoes the acquirer authorization id (mirrors the
            // Authorize/PSync handlers).
            network_txn_id: body.auth_id.clone(),
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_refund_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        // AsiaPay sends no refund data feed; refund outcomes are tracked via
        // RSync (Merchant API Query keyed by payRef).
        Err(error_stack::report!(
            errors::WebhookError::WebhooksNotImplemented {
                operation: "process_refund_webhook",
            }
        ))
    }

    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, error_stack::Report<errors::WebhookError>> {
        // AsiaPay's Data Feed requires the receiver to print `OK` as the ACK
        // (tech spec §7) — without it AsiaPay assumes delivery was lost and
        // the merchant must confirm the transaction via the Query API
        // (PSync/RSync).
        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: Some(b"OK".to_vec()),
        })
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        b"prc=0&src=0&Ord=12345678&Ref=probe_ord_001&PayRef=13907042&successcode=0&Amt=1.00&Cur=702&Holder=TEST+CARD&AuthId=907042&AlertCode=&remark=&eci=07&payerAuth=U&sourceIp=65.1.52.128&ipCountry=IN&payMethod=VISA&TxTime=2026-06-09+22%3A46%3A06.0&cardIssuingCountry=HK&channelType=DPS&MerchantId=12109740&secureHash=probe_secure_hash"
    }
}

fn redirect_details_from_query_params(
    query_params: Option<&str>,
) -> domain_types::connector_types::RedirectDetailsResponse {
    // AsiaPay can return either `success/fail/cancel?Ref=...`-style query data.
    // Decode form semantics so the exact orderRef used by PSync survives both
    // percent-escaped characters and `+` spaces. Keep the unsigned return
    // informational: only identity is propagated, never payment status.
    let ref_id = query_params.and_then(|query| {
        url::form_urlencoded::parse(query.as_bytes())
            .find_map(|(key, value)| (key == "Ref").then(|| value.into_owned()))
    });

    domain_types::connector_types::RedirectDetailsResponse {
        resource_id: ref_id.clone().map(ResponseId::ConnectorTransactionId),
        status: None,
        response_money: None,
        connector_response_reference_id: ref_id,
        error_code: None,
        error_message: None,
        error_reason: None,
        raw_connector_response: None,
        connector_feature_data: None,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Asiapay<T>
{
    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, errors::IntegrationError> {
        // CPTB return URL is unsigned and display-only — never trust it for state.
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        request: &RequestDetails,
        _connector_feature_data: Option<&Secret<String>>,
    ) -> CustomResult<
        domain_types::connector_types::RedirectDetailsResponse,
        errors::IntegrationError,
    > {
        let query_params = request.query_params.as_deref().or_else(|| {
            request.uri.as_deref().and_then(|uri| {
                uri.split_once('?')
                    .map(|(_, query)| query.split('#').next().unwrap_or(query))
            })
        });
        Ok(redirect_details_from_query_params(query_params))
    }
}

// ===== FLOW MARKER TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Asiapay<T>
{
    fn validate_psync_reference_id(
        &self,
        _data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), errors::IntegrationError> {
        // AsiaPay's Query is keyed by `orderRef` — the merchant reference
        // carried on `connector_request_reference_id`, NOT
        // `connector_transaction_id` (payRef). The PSync request transformer
        // raises IntegrationError::MissingRequiredField when the carrier is
        // empty, so nothing is rejected here that that check would allow.
        Ok(())
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Asiapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Asiapay<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// `VerifyRedirectResponse: SourceVerification + BodyDecoding`
// (crates/types-traits/interfaces/src/connector_types.rs), so both supertraits
// must be impl'd or the VerifyRedirectResponse impl above does not compile.
// SourceVerification is emitted here; BodyDecoding already comes from
// template-generation/connector.rs.template - do NOT emit a second one here,
// two impls of the same trait for the same type do not compile.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Asiapay<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Asiapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Asiapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        GetConnectorCustomer,
        MandateRevoke,
        Authenticate,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        VoidPostRefund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn merchant_api_rejects_html_without_rejecting_urlencoded_data() {
        let html = bytes::Bytes::from_static(b"  \n<html><body>synthetic error</body></html>");
        assert!(preprocess_merchant_api_response(html, 200).is_err());

        let form = bytes::Bytes::from_static(b"resultCode=0&orderStatus=Accepted");
        let parsed = preprocess_merchant_api_response(form, 200)
            .expect("valid Merchant API form data must still be converted");
        let parsed: serde_json::Value =
            serde_json::from_slice(&parsed).expect("Merchant API conversion must produce JSON");
        assert_eq!(parsed["resultCode"], "0");
        assert_eq!(parsed["orderStatus"], "Accepted");
    }

    #[test]
    fn redirect_decodes_and_propagates_order_ref_for_psync() {
        for (query, expected) in [
            ("ignored=value&Ref=A%2FB", "A/B"),
            ("Ref=A+B", "A B"),
            ("Ref=order%20with%20%25%26", "order with %&"),
        ] {
            let response = redirect_details_from_query_params(Some(query));
            assert!(matches!(
                response.resource_id,
                Some(ResponseId::ConnectorTransactionId(ref id)) if id == expected
            ));
            assert_eq!(
                response.connector_response_reference_id.as_deref(),
                Some(expected)
            );
            assert!(response.status.is_none());
            assert!(response.response_money.is_none());
        }
    }
}
