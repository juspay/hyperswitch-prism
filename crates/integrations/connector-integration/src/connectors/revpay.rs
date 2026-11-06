pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RequestDetails, ResponseId,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as revpay;

use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};
use transformers::{
    RevpayPSyncRequest, RevpayPSyncResponse, RevpayPaymentsRequest, RevpayPaymentsResponse,
    RevpayRSyncRequest, RevpayRSyncResponse, RevpayRefundRequest, RevpayRefundResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// revPAY amounts are major-unit strings with exactly two decimal places
// ("52.40"). The workspace `StringTwoDecimalUnit` renders the implied-decimal
// form instead ("5240"), so the request transformer builds the exact wire
// string in integer space before signing — see
// `revpay::transformers::format_revpay_amount`.
macros::create_amount_converter_wrapper!(connector_name: Revpay, amount_type: StringTwoDecimalUnit);

// =============================================================================
// FLOW IMPLEMENTATIONS
// =============================================================================

macros::create_all_prerequisites!(
    connector_name: Revpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: RevpayPaymentsRequest<T>,
            response_body: RevpayPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: RevpayPSyncRequest,
            response_body: RevpayPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RevpayRefundRequest,
            response_body: RevpayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: RevpayRSyncRequest,
            response_body: RevpayRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // revPAY authenticates via an in-body SHA-512 signature, so the
            // headers only carry the content type (get_auth_header is empty).
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data.connectors.revpay.base_url.as_ref()
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data.connectors.revpay.base_url.as_ref()
        }
    }
);

// Authorize flow — POST {base}/v1/payment
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revpay,
    curl_request: Json(RevpayPaymentsRequest<T>),
    curl_response: RevpayPaymentsResponse,
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
            Ok(format!("{}/v1/payment", self.connector_base_url_payments(req)))
        }
    }
);

// PSync flow — POST {base}/v1/inquiry (Payment Status Inquiry, JSON variant)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revpay,
    curl_request: Json(RevpayPSyncRequest),
    curl_response: RevpayPSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/inquiry", self.connector_base_url_payments(req)))
        }
    }
);

// Refund flow — POST {base}/v1/refund (§2.5). Synchronous accept/reject: HTTP
// is always 200, the outcome lives in the body `Response_Code`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revpay,
    curl_request: Json(RevpayRefundRequest),
    curl_response: RevpayRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/refund", self.connector_base_url_refunds(req)))
        }
    }
);

// RSync flow — POST {base}/v1/inquiry (Flow 4). revPAY 3.1.10 has NO
// dedicated refund-inquiry endpoint, so per product decision this reuses the
// payment inquiry keyed on the REFUND's own Reference_Number with the refund
// amount; outcome is reported synchronously through the refund status (same
// posture as PSync — never an in-band Err).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revpay,
    curl_request: Json(RevpayRSyncRequest),
    curl_response: RevpayRSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/inquiry", self.connector_base_url_refunds(req)))
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Revpay<T>
{
    fn id(&self) -> &'static str {
        "revpay"
    }

    // The concrete wire encoding is built in the request transformer
    // (`format_revpay_amount`), starting from the framework's MinorUnit
    // amount, so the currency unit here is `Minor`.
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.revpay.base_url.as_ref()
    }

    // revPAY has no HTTP auth header: authentication is an in-body SHA-512
    // signature (`Revpay_Merchant_ID`, Merchant Key, `Key_Index`) computed
    // per request in the transformers. `get_auth_header` therefore only
    // validates that the correct credential variant is present, so a
    // misconfigured account fails at request-build time instead of at the
    // gateway.
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        revpay::RevpayAuthType::try_from(auth_type)?;
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: revpay::RevpayErrorResponse = res
            .response
            .parse_struct("RevpayErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "revpay: error body did not match the documented error shape",
            ))?;

        with_error_response_body!(event_builder, response);

        // `attempt_status` is `Option<FlowStatus>`, and this method is shared by
        // EVERY flow of this connector - Authorize, PSync, Capture, Void, Refund,
        // RSync. Two ways to get this wrong:
        //
        //   1. Hardcoding a terminal payment failure
        //      (`Some(FlowStatus::Payment(AttemptStatus::Failure))`). A 4xx on
        //      PSync for an already-charged payment then reports a CHARGED
        //      payment as FAILURE.
        //   2. Hardcoding `None` for everything. A hard-declined refund then
        //      stays Pending and the framework retries it forever.
        //
        // revPAY answers real declines with HTTP 200 plus a body
        // `Response_Code`, which is handled in-band by the Authorize
        // response transformer — so a non-2xx here is a transport/gateway
        // fault with no provable terminal attempt state.
        let attempt_status: Option<FlowStatus> = None;

        Ok(ErrorResponse {
            status_code: res.status_code,
            // Prefer revPAY's own Response_Code / Error_Description fields,
            // falling back to a generic code/message shape — never
            // `.unwrap_or_default()`: an empty `code`/`message` is
            // indistinguishable from a connector that really returned "". The
            // NO_ERROR_CODE / NO_ERROR_MESSAGE sentinels are what the rest of
            // the platform greps for.
            code: response
                .response_code
                .clone()
                .or_else(|| response.code.clone())
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .error_description
                .clone()
                .or_else(|| response.message.clone())
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response
                .error_description
                .clone()
                .or_else(|| response.message.clone()),
            attempt_status,
            typed_connector_response: macros::serialize_typed_connector_payload(
                &response,
                "typed_connector_response",
            ),
            // `ErrorResponse` has 13 fields and `impl Default`
            // (crates/types-traits/domain_types/src/router_data.rs). Use the
            // functional-update tail so a new field upstream does not break this.
            ..Default::default()
        })
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Revpay<T>
{
}
// =============================================================================
// DYNAMICALLY GENERATED IMPLEMENTATIONS
// =============================================================================
// Auto-generated by add_connector.sh using the macro-based pattern. All flow
// traits are stubbed via `macros::macro_connector_flow_status_impls!` with
// `not_implemented` status, which emits both the marker-trait impl and a stub
// `ConnectorIntegrationV2` impl per flow.
//
// To implement a flow:
//   1. Remove that flow's name from the `not_implemented` list below.
//   2. Add a manual marker-trait impl, e.g.
//        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
//            connector_types::PaymentAuthorizeV2<T> for Revpay<T> {}
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
    connector_types::ConnectorServiceTrait<T> for Revpay<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Revpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Revpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Revpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Revpay<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Revpay<T>
{
    // revPAY's Inquiry syncs on the merchant's Reference_Number (carried as
    // `connector_request_reference_id`), NOT on the gateway Transaction_ID —
    // the default "require connector_transaction_id" gate would skip PSync
    // whenever the caller only holds the merchant reference. Defer the check
    // to the request transformer, which errors out on an empty reference.
    fn validate_psync_reference_id(
        &self,
        _data: &PaymentsSyncData,
        payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        if payment_flow_data
            .connector_request_reference_id
            .trim()
            .is_empty()
        {
            return Err(error_stack::report!(
                IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
            ));
        }
        Ok(())
    }
}

// ===== INCOMING WEBHOOK — Backend_URL payment notification (Flow 5) =====
// revPAY POSTs the final payment result server-to-server as
// application/x-www-form-urlencoded (NOT JSON). Verification recomputes the
// in-body SHA-512 signature (§3.3) with the merchant key taken from the
// connector account config — the same credential extraction the flows use.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Revpay<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Euler does not send `connector_webhook_secret` on this path — the
        // Merchant Key lives on the connector account config (same idiom as
        // payhere/grabpay).
        let connector_account_details = connector_account_details
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookVerificationSecretNotFound))?;
        let auth = revpay::RevpayAuthType::try_from(&connector_account_details)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;

        let body: revpay::RevpayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        // Signature AND key-index equality; `false` (never a panic) on any
        // mismatch, so the framework treats the payload as untrusted.
        Ok(body.verifies_against(&auth))
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: revpay::RevpayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        Ok(body.event_type())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: revpay::RevpayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: Some(body.transaction_id),
                // Reference_Number is the merchant-assigned reference the
                // original Payment call was keyed by.
                merchant_transaction_id: Some(body.reference_number),
                connector_preprocessing_id: None,
            },
        )))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let body: revpay::RevpayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;
        Ok(Box::new(body))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body: revpay::RevpayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let status = body.attempt_status();
        let failure = body.failure_details();

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(body.transaction_id)),
            status,
            connector_response_reference_id: Some(body.reference_number.clone()),
            // The payment was keyed by Reference_Number on Authorize — echo it
            // so the framework correlates this notification with the attempt.
            connector_request_reference_id: Some(body.reference_number),
            mandate_reference: None,
            error_code: failure.as_ref().map(|(code, _)| code.clone()),
            error_message: failure.map(|(_, message)| message),
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    // revPAY's ACK contract (§2.1.3): the merchant must answer with the plain
    // body "OK"; anything else triggers retries (2 attempts at 5-minute
    // intervals). The framework surfaces the ack body via this hook.
    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, error_stack::Report<WebhookError>> {
        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: Some(b"OK".to_vec()),
        })
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        // Form-urlencoded payment-result notification (all values dummy).
        b"Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MBB&Transaction_ID=RT000000000001&Reference_Number=RF000000001&Bank_Reference_Number=BR000001&Amount=1.00&Currency=MYR&Transaction_Description=probe&Response_Code=00&Error_Description=&Key_Index=1&Signature=0000&Request_Datetime=2024-01-01%2000%3A00%3A00&Response_Datetime=2024-01-01%2000%3A00%3A01"
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Revpay<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// `VerifyRedirectResponse: SourceVerification + BodyDecoding`
// (crates/types-traits/interfaces/src/connector_types.rs), so both supertraits
// must be impl'd or the VerifyRedirectResponse impl above does not compile.
// SourceVerification is emitted here; BodyDecoding already comes from
// template-generation/connector.rs.template - do NOT emit a second one here,
// two impls of the same trait for the same type do not compile.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Revpay<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Revpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Emits marker-trait impls AND stub ConnectorIntegrationV2 impls for every
// flow listed. Each stub's get_url returns
// IntegrationError::connector_flow_not_implemented(...).
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Revpay,
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
