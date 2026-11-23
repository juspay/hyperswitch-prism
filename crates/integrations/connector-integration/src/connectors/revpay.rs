pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::StringMajorUnitWithTwoDecimals,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RedirectDetailsResponse, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RequestDetails, ResponseId,
        WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
    webhooks::{IncomingWebhook, IncomingWebhookFlowError},
};
use serde::Serialize;
use transformers as revpay;

use crate::webhooks::revpay::Revpay as RevpayWebhook;
use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};
use transformers::{
    RevpayPSyncRequest, RevpayPSyncResponse, RevpayPaymentsRequest, RevpayPaymentsResponse,
    RevpayRSyncRequest, RevpayRSyncResponse, RevpayRefundRequest, RevpayRefundResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// revPAY amounts are major-unit strings with exactly two decimal places
// ("52.40"). The shared converter preserves that exact wire value for both
// JSON serialization and signature generation.
macros::create_amount_converter_wrapper!(
    connector_name: Revpay,
    amount_type: StringMajorUnitWithTwoDecimals
);

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
    amount_converters: [amount_converter: StringMajorUnitWithTwoDecimals],
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

    // `RevpayAmountConvertor` converts the framework's MinorUnit amount to the
    // exact wire string in the request transformers, so the currency unit here
    // is `Minor`.
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
    // revPAY's Inquiry syncs on the merchant's Reference_Number (carried as
    // `connector_request_reference_id`), NOT on the gateway Transaction_ID —
    // the default "require connector_transaction_id" gate would skip PSync
    // whenever the caller only holds the merchant reference. Defer the check
    // to the request transformer, which errors out on an empty reference.
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        if !matches!(
            &data.connector_transaction_id,
            ResponseId::ConnectorTransactionId(reference)
                if !reference.trim().is_empty()
        ) {
            return Err(error_stack::report!(
                IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "revPAY Payment Status Inquiry requires the original Authorize \
                             Reference_Number in connector_transaction_id; a gateway \
                             Transaction_ID cannot replace it"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Provide the original Authorize merchant reference as \
                             connector_transaction_id on the payment sync request"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
            ));
        }
        Ok(())
    }
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
}

// ===== INCOMING WEBHOOK — Backend_URL payment notification (Flow 5) =====
// revPAY POSTs the final payment result server-to-server as
// application/x-www-form-urlencoded (NOT JSON). Verification recomputes the
// in-body SHA-512 signature (§3.3) with the merchant key taken from the
// connector account config — the same credential extraction the flows use.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> IncomingWebhook
    for Revpay<T>
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
        // Delegates to the webhook parser so HandleEvent and ParseEvent share one implementation.
        interfaces::webhooks::IncomingWebhookEventParser::<T>::get_event_type(
            RevpayWebhook::<T>::new(),
            request,
        )
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
        let money = body
            .money()
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(WebhookDetailsResponse {
            raw_connector_status: None,
            amount: Some(money.clone()),
            resource_id: Some(ResponseId::ConnectorTransactionId(
                body.reference_number.clone(),
            )),
            status,
            connector_response_reference_id: Some(body.transaction_id.clone()),
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
            captured_money: if status == common_enums::AttemptStatus::Charged {
                Some(money)
            } else {
                None
            },
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
        _error_kind: Option<IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, error_stack::Report<WebhookError>> {
        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: Some(b"OK".to_vec()),
        })
    }

    /// Documented success notification (Merchant API §3.29), including the
    /// signature for its published test key. This is a field-probe fixture;
    /// live notifications are verified against the configured merchant key.
    fn sample_webhook_body(&self) -> &'static [u8] {
        b"Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MB2U&Transaction_ID=RT000000007222&Reference_Number=RF201774111&Bank_Reference_Number=BR000001&Amount=52.40&Currency=MYR&Transaction_Description=Order+RF201774111&Fraud_Score=0&Response_Code=00&Error_Description=&Key_Index=1&Signature=fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad&Request_Datetime=2020-01-01+12%3A00%3A00&Response_Datetime=2020-01-01+12%3A00%3A05"
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Revpay<T>
{
    fn verify_redirect_response_source(
        &self,
        request: &RequestDetails,
        secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, IntegrationError> {
        let body = parse_revpay_redirect_response(request)?;
        let secrets = secrets.ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "redirect_response_secrets",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "revPAY redirect signature verification requires the merchant key"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;
        let merchant_key = match secrets {
            ConnectorSourceVerificationSecrets::RedirectResponseSecret(secret) => secret.secret,
            _ => {
                return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "redirect_response_secrets",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "revPAY expected redirect-response secrets containing the merchant key"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        };
        let merchant_key = std::str::from_utf8(&merchant_key).map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "redirect_response_secrets.secret",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "revPAY merchant keys must be valid UTF-8 strings".to_string(),
                    ),
                    ..Default::default()
                },
            })
        })?;
        Ok(body.verifies_with_merchant_key(merchant_key))
    }

    fn process_redirect_response(
        &self,
        request: &RequestDetails,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
        let body = parse_revpay_redirect_response(request)?;
        let failure = body.failure_details();
        let status = body.attempt_status();
        let money = body
            .money()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "Amount",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "revPAY redirect Amount must use the documented two-decimal format"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let reference_number = body.reference_number;

        Ok(RedirectDetailsResponse {
            // The compatibility response maps `resource_id` to
            // `connector_transaction_id`. Keep the merchant Reference_Number
            // here because it is the stable identifier used by PSync and can
            // exist before revPAY assigns a Transaction_ID.
            resource_id: Some(ResponseId::ConnectorTransactionId(reference_number.clone())),
            status: Some(status),
            response_money: Some(money),
            // The same compatibility response maps this field to
            // `merchant_order_id`, whose value is also Reference_Number.
            connector_response_reference_id: Some(reference_number),
            error_code: failure.as_ref().map(|(code, _)| code.clone()),
            error_message: failure.as_ref().map(|(_, message)| message.clone()),
            error_reason: failure.map(|(_, message)| message),
            raw_connector_response: request.query_params.clone(),
            connector_feature_data: None,
        })
    }
}

fn parse_revpay_redirect_response(
    request: &RequestDetails,
) -> CustomResult<revpay::RevpayWebhookBody, IntegrationError> {
    let query = request.query_params.as_deref().ok_or_else(|| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "request_details.query_params",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "revPAY returns the payment result as Return_URL query parameters".to_string(),
                ),
                ..Default::default()
            },
        })
    })?;

    // Euler serializes callback query parameters as a JSON object, while the
    // HTTP flow can also provide the original form-encoded query string. A
    // payload beginning with JSON syntax must parse as JSON; malformed JSON
    // is rejected rather than silently reinterpreted as a form query.
    let trimmed_query = query.trim_start();
    let form = if trimmed_query.starts_with('{') || trimmed_query.starts_with('[') {
        match serde_json::from_str::<serde_json::Value>(query).change_context(
            IntegrationError::InvalidDataFormat {
                field_name: "request_details.query_params",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "revPAY redirect parameters contain malformed JSON".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )? {
            serde_json::Value::Object(values) => {
                let mut serializer = url::form_urlencoded::Serializer::new(String::new());
                for (key, value) in values {
                    let value = match value {
                        serde_json::Value::String(value) => value,
                        serde_json::Value::Number(value) => value.to_string(),
                        serde_json::Value::Bool(value) => value.to_string(),
                        // Optional callback fields are represented as JSON
                        // null by Euler. Omitting them preserves their absence;
                        // encoding them as empty strings changes the payload.
                        serde_json::Value::Null => continue,
                        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                            return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                                field_name: "request_details.query_params",
                                context: IntegrationErrorContext {
                                    additional_context: Some(format!(
                                        "revPAY redirect field `{key}` must be a primitive value"
                                    )),
                                    ..Default::default()
                                },
                            }))
                        }
                    };
                    serializer.append_pair(&key, &value);
                }
                serializer.finish()
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                    field_name: "request_details.query_params",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "revPAY redirect JSON parameters must be an object".to_string(),
                        ),
                        ..Default::default()
                    },
                }))
            }
        }
    } else {
        query.to_string()
    };

    serde_urlencoded::from_str(&form).change_context(IntegrationError::InvalidDataFormat {
        field_name: "request_details.query_params",
        context: IntegrationErrorContext {
            additional_context: Some(
                "revPAY redirect parameters do not match the documented callback schema"
                    .to_string(),
            ),
            ..Default::default()
        },
    })
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
