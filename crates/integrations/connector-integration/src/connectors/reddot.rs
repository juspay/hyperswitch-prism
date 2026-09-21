pub mod transformers;

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit};
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RequestDetails, ResponseId,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{self, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, Secret};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as reddot, ReddotAuthorizeRequest, ReddotAuthorizeResponse, ReddotPSyncRequest,
    ReddotPSyncResponse, ReddotRefundRequest, ReddotRefundResponse, ReddotRefundSyncRequest,
    ReddotRefundSyncResponse, ReddotWebhookBody,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

macros::create_amount_converter_wrapper!(connector_name: Reddot, amount_type: StringMajorUnit);

// =============================================================================
// FLOW PREREQUISITES (struct, bridges, amount converters, shared helpers)
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Reddot,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ReddotAuthorizeRequest<T>,
            response_body: ReddotAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: ReddotPSyncRequest,
            response_body: ReddotPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: ReddotRefundRequest,
            response_body: ReddotRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: ReddotRefundSyncRequest,
            response_body: ReddotRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.reddot.base_url
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Reddot<T>
{
    fn id(&self) -> &'static str {
        "reddot"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.reddot.base_url.as_ref()
    }

    // NOTE: Red Dot Payment sends no auth headers — credentials (`mid` /
    // `secret_key` signatures) travel in the JSON request body only, so the
    // `ConnectorCommon::get_auth_header` default (empty list) is used.

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        // RDP's edge (Imperva Incapsula WAF) serves `403 + HTML` to blocked
        // clients — a strict JSON parse of that page would surface a
        // misleading RESPONSE_DESERIALIZATION_FAILED, so fall back to a
        // synthesized error that keeps the HTTP status and the body snippet.
        let response: reddot::ReddotErrorResponse = if res.response.is_empty() {
            reddot::ReddotErrorResponse {
                response_status: "ERROR".to_string(),
                response_code: res.status_code.to_string(),
                response_msg: "Red Dot returned an empty error body".to_string(),
            }
        } else {
            res.response
                .parse_struct("ReddotErrorResponse")
                .ok()
                .unwrap_or_else(|| {
                    reddot::ReddotErrorResponse::from_non_json_body(
                        res.status_code,
                        res.response.as_ref(),
                    )
                })
        };

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.response_code,
            message: response.response_msg,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            // Capture the raw body regardless of the return_raw_connector_data
            // config flag (mirrors maya's build_error_response); on a WAF
            // block this is the Incapsula HTML page.
            raw_connector_response: Some(Secret::new(String::from_utf8_lossy(&res.response).to_string())),
            typed_connector_response: typed,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

// =============================================================================
// BASE TRAIT IMPLEMENTATIONS
// =============================================================================

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Reddot<T>
{
}

// ===== IMPLEMENTED FLOW MARKERS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Reddot<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Reddot<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Reddot<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Reddot<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Reddot<T>
{
}

// RDP notify_url callbacks are neither encrypted nor verifiable in this
// integration (no documented/usable signature scheme for callbacks), so the
// webhook is treated as an UNVERIFIED trigger — the caller (Euler) must follow
// up with a mandatory PSync for authoritative status. This deliberately
// mirrors maya/glomopay; euler routes RDP through EventService/parse-only via
// `ucsParseEventOnlyGateways` and consumes only the event type + references.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Reddot<T>
{
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        Ok(false)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body: ReddotWebhookBody = request
            .body
            .parse_struct("ReddotWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(match body.response_code.as_str() {
            "0" => EventType::PaymentIntentSuccess,
            "-01" => EventType::PaymentIntentProcessing,
            _ => EventType::PaymentIntentFailure,
        })
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body: ReddotWebhookBody = request
            .body
            .parse_struct("ReddotWebhookBody")
            .change_context(errors::WebhookError::WebhookResourceObjectNotFound)?;

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                // RDP's gateway-side id for this payment.
                connector_transaction_id: body.transaction_id,
                // `merchant_reference` = full Juspay txnId echoed by RDP —
                // this is the integrity-key euler's verifyTxnId compares
                // against txnDetail.txnId (TrackerVerification.hs).
                // Fallback to `order_id` (our random 16-hex id) never matches
                // a Juspay txnId, so it degrades to a loud integrity failure
                // when the acquirer didn't round-trip merchant_reference.
                merchant_transaction_id: body.merchant_reference.or(body.order_id),
            },
        )))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let body: ReddotWebhookBody = request
            .body
            .parse_struct("ReddotWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        // Same response_code semantics as the PSync transform: RDP sale txns
        // are auto-captured, so success is Charged; `-01` is still in-flight;
        // `-7995`/`-7997` are 3DS-auth failures → AUTHENTICATION_FAILED;
        // everything else → AUTHORIZATION_FAILED.
        let (status, error_code, error_message, error_reason) = match body.response_code.as_str() {
            "0" => (AttemptStatus::Charged, None, None, None),
            "-01" => (AttemptStatus::Pending, None, None, None),
            "-7995" | "-7997" => (
                AttemptStatus::AuthenticationFailed,
                Some(body.response_code.clone()),
                Some(body.response_msg.clone().unwrap_or_default()),
                body.response_msg.clone(),
            ),
            error_code => (
                AttemptStatus::AuthorizationFailed,
                Some(error_code.to_string()),
                Some(body.response_msg.clone().unwrap_or_default()),
                body.response_msg.clone(),
            ),
        };

        let resource_id = match body.transaction_id {
            Some(transaction_id) => ResponseId::ConnectorTransactionId(transaction_id),
            None => ResponseId::NoResponseId,
        };

        Ok(WebhookDetailsResponse {
            resource_id: Some(resource_id),
            status,
            // merchant_reference = full Juspay txnId echoed by RDP; order_id
            // (our random 16-hex id) is the last-resort reference.
            connector_response_reference_id: body.merchant_reference.clone(),
            connector_request_reference_id: body.merchant_reference.or(body.order_id),
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Reddot<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// RDP error responses (and unsigned callbacks) carry no signature to verify;
// this is the default marker impl used by comparable no-signature connectors.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Reddot<T>
{
}

// ===== BODY DECODING IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Reddot<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Reddot,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
//
// To implement a flow:
//   1. Remove that flow's name from the `not_implemented` list below.
//   2. Add a manual marker-trait impl, e.g.
//        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
//            connector_types::PaymentAuthorizeV2<T> for Reddot<T> {}
//   3. Add a `macros::macro_connector_implementation!(...)` block with the
//      flow's request/response types, HTTP method, and `get_url`/`get_headers`.
//
// See crates/integrations/connector-integration/src/connectors/xendit.rs for a
// reference implementation that follows this pattern.
// RDP serves two host families per environment (mirrors RDP's own docs):
//   base_url            — the Redirect/Payment family (`secure-dev` /
//                         `secure`): /service/payment-api +
//                         /service/Merchant_processor/query_redirection
//   secondary_base_url  — the Merchant API family (`test` / `connect`):
//                         /instanpanel/api/payment (refund) +
//                         /instanpanel/api/enquiry (merchant enquiry)
// so flows keyed to instanpanel MUST NOT be built off `base_url`.
fn instanpanel_base_url(connectors: &Connectors) -> CustomResult<&str, IntegrationError> {
    connectors
        .reddot
        .secondary_base_url
        .as_deref()
        .ok_or(IntegrationError::InvalidConnectorConfig {
            config: "secondary_base_url",
            context: errors::IntegrationErrorContext {
                additional_context: Some(
                    "reddot Merchant API (Refund/Enquiry) lives on the instanpanel host family, distinct from the payment family"
                        .to_string(),
                ),
                suggested_action: Some(
                    "Set reddot.secondary_base_url (e.g. https://test.reddotpayment.com for UAT, https://connect.reddotpayment.com for Prod) in connector config"
                        .to_string(),
                ),
                doc_url: Some(
                    "https://developers.reddotpayment.com/merchant/#capture-refund-void"
                        .to_string(),
                ),
            },
        })
        .map_err(error_stack::Report::from)
}

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Reddot,
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
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        Void,
        VoidPostRefund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);

// =============================================================================
// AUTHORIZE FLOW (RDP Redirect API, SOP mode — POST /service/payment-api)
// =============================================================================
// RDP credentials travel inside the JSON body (`mid` + computed `signature`),
// so no connector-specific headers are needed: reqwest's `.json()` body sets
// `Content-Type: application/json` automatically.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Reddot,
    curl_request: Json(ReddotAuthorizeRequest),
    curl_response: ReddotAuthorizeResponse,
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
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/service/payment-api",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// =============================================================================
// PSYNC FLOW (RDP Redirection Enquiry — POST /service/Merchant_processor/query_redirection)
// =============================================================================
// Credentials again travel inside the JSON body (`request_mid` + computed
// generic `signature`), so no connector-specific headers are needed. The
// `/service/`-prefixed path variant is the sandbox-verified server-to-server
// endpoint (the bare `/Merchant_processor/query_redirection` path is prone to
// Incapsula bot-blocking for headless clients).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Reddot,
    curl_request: Json(ReddotPSyncRequest),
    curl_response: ReddotPSyncResponse,
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
            _req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/service/Merchant_processor/query_redirection",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// =============================================================================
// REFUND FLOW (RDP Merchant API — POST /instanpanel/api/payment)
// =============================================================================
// Body is application/x-www-form-urlencoded (FormUrlEncoded); credentials and
// the MD5 merchant-API signature travel inside the body.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Reddot,
    curl_request: FormUrlEncoded(ReddotRefundRequest),
    curl_response: ReddotRefundResponse,
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
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/instanpanel/api/payment",
                instanpanel_base_url(&req.resource_common_data.connectors)?
            ))
        }
    }
);

// =============================================================================
// REFUND SYNC FLOW (RDP Enquiry API — same endpoint/signature scheme as psync)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Reddot,
    curl_request: Json(ReddotPSyncRequest),
    curl_response: ReddotRefundSyncResponse,
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
            _req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/instanpanel/api/enquiry",
                instanpanel_base_url(&req.resource_common_data.connectors)?
            ))
        }
    }
);
