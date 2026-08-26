//! Tap Payments — MENA card acquiring (charges / pre-auth).
//!
//! Scope for this foundation: the **Authorize** and **PSync** flows.
//!
//! Tap has two authorisation endpoints and the connector selects between them by capture method:
//!
//! * `POST /charges` — the auto-capture charge. Used when `capture_method` is anything other than
//!   `Manual`. The charge is authorised and captured in the one call.
//! * `POST /authorize` — the pre-authorisation. Used when `capture_method == Manual`; the funds are
//!   only ring-fenced and a later Capture (follow-up flow) moves the money.
//!
//! Both answer the same `chargeResponse` envelope (`id`, `status`, `response.{code,message}`, an
//! optional `transaction.url` for a 3DS redirect), so both legs deserialise into one
//! [`transformers::TapChargeResponse`] type and share one status map.
//!
//! * `GET /charges/{id}` — the sync (**PSync**), keyed by the connector transaction id. Same
//!   response envelope, same status map.
//!
//! Auth is a static Bearer secret key, so it is built in [`ConnectorCommon::get_auth_header`]
//! rather than per request. Amounts are decimal **major** units (AED 10.00 → `10.00`), converted
//! with [`common_utils::types::FloatMajorUnit`].
//!
//! **Card handling.** Tap does not accept a raw PAN on `/charges`: the card must be a token
//! (`source.id = tok_…`) or an RSA-encrypted blob built with the merchant's `tapPublicKey`. RSA
//! encryption is deferred to a follow-up, so a raw-card payment is refused here with a clear
//! `NotImplemented` error and a token source is passed through when supplied. See
//! [`transformers::build_tap_source`].
//!
//! Reference: `grace/rulesbook/codegen/references/tap/technical_specification.md`

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt as _;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as tap, TapCaptureRequest, TapCaptureResponse, TapChargeResponse, TapPaymentRequest,
    TapRefundRequest, TapRefundResponse, TapRefundSyncResponse, TapSyncResponse, TapVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

const TAP_JSON_MEDIA_TYPE: &str = "application/json";

/// Auto-capture charge endpoint. Also the capture target: a pre-auth is captured by re-posting a
/// charge that references the authorised source.
const CHARGES_ENDPOINT: &str = "/charges";
/// Pre-authorisation endpoint (`capture_method == Manual`). `GET /authorize/{id}/void` voids a
/// pre-auth.
const AUTHORIZE_ENDPOINT: &str = "/authorize";
/// Refund create/sync endpoint (`POST /refunds`, `GET /refunds/{id}`).
const REFUNDS_ENDPOINT: &str = "/refunds";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Tap<T>
{
    fn id(&self) -> &'static str {
        "tap"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // Tap wire amounts are decimal major units; UCS still hands us MinorUnit and the
        // FloatMajorUnit converter does the conversion.
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        TAP_JSON_MEDIA_TYPE
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.tap.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = tap::TapAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        match res
            .response
            .parse_struct::<tap::TapErrorResponse>("TapErrorResponse")
        {
            Ok(response) => {
                with_error_response_body!(event_builder, response);

                let first = response.errors.first();
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: first
                        .map(|error| error.code.clone())
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: first
                        .map(|error| error.description.clone())
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: first.map(|error| error.description.clone()),
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
                    "tap returned a body that is not a TapErrorResponse envelope"
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Tap<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Tap, amount_type: FloatMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Tap,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: TapPaymentRequest,
            response_body: TapChargeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: TapSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: TapCaptureRequest,
            response_body: TapCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: TapVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TapRefundRequest,
            response_body: TapRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: TapRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        /// The two Tap request headers: `Content-Type` and the Bearer `Authorization`.
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            headers.extend(self.get_auth_header(&req.connector_config)?);
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.tap.base_url
        }

        /// Refund-side counterpart of [`Self::connector_base_url_payments`]. Tap serves refunds
        /// from the same host; the split exists only because `RefundFlowData` is a different
        /// common-data type.
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.tap.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tap,
    curl_request: Json(TapPaymentRequest),
    curl_response: TapChargeResponse,
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
            let endpoint = if tap::is_manual_capture(&req.request) {
                AUTHORIZE_ENDPOINT
            } else {
                CHARGES_ENDPOINT
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tap,
    curl_response: TapSyncResponse,
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
            let transaction_id = tap::sync_transaction_id(&req.request)?;
            Ok(format!(
                "{}{}/{}",
                self.connector_base_url_payments(req),
                CHARGES_ENDPOINT,
                transaction_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tap,
    curl_request: Json(TapCaptureRequest),
    curl_response: TapCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
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
            // Tap captures a pre-auth by re-posting to /charges referencing the authorised charge.
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                CHARGES_ENDPOINT
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tap,
    curl_response: TapVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
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
            // POST /authorize/{id}/void with an empty body.
            Ok(format!(
                "{}{}/{}/void",
                self.connector_base_url_payments(req),
                AUTHORIZE_ENDPOINT,
                req.request.connector_transaction_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tap,
    curl_request: Json(TapRefundRequest),
    curl_response: TapRefundResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_refunds(req),
                REFUNDS_ENDPOINT
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tap,
    curl_response: TapRefundSyncResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}{}/{}",
                self.connector_base_url_refunds(req),
                REFUNDS_ENDPOINT,
                req.request.connector_refund_id
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Tap<T>
{
}

/// Parse a raw webhook body into a [`tap::TapWebhookBody`], mapping any decode failure to
/// [`WebhookError::WebhookBodyDecodingFailed`].
fn parse_tap_webhook_body(
    body: &[u8],
) -> Result<tap::TapWebhookBody, error_stack::Report<errors::WebhookError>> {
    body.parse_struct::<tap::TapWebhookBody>("TapWebhookBody")
        .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Tap<T>
{
    /// Tap does not sign its webhooks (no HMAC/secret is available); euler trusts the body and
    /// re-confirms via a follow-up sync. There is therefore nothing to verify here — accept the
    /// source and let the downstream sync be the source of truth.
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        Ok(true)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"chg_probe_001","object":"charge","status":"CAPTURED","reference":{"transaction":"probe_txn_001"},"response":{"code":"000","message":"Captured"}}"#
    }

    /// Charge webhooks (`AUTHORIZE`/`CHARGE`) map to a payment event off the shared attempt-status
    /// map; refund webhooks map to a refund event off the shared refund-status map. Non-terminal
    /// states map to the processing/action-required variants, never a bare success/failure.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body = parse_tap_webhook_body(&request.body)?;
        let event = match body.classify() {
            tap::TapWebhookKind::Charge => match body.payment_attempt_status() {
                common_enums::AttemptStatus::Charged => EventType::PaymentIntentSuccess,
                common_enums::AttemptStatus::Authorized => {
                    EventType::PaymentIntentAuthorizationSuccess
                }
                common_enums::AttemptStatus::Voided => EventType::PaymentIntentCancelled,
                common_enums::AttemptStatus::Failure => EventType::PaymentIntentFailure,
                common_enums::AttemptStatus::AuthenticationPending => {
                    EventType::PaymentActionRequired
                }
                _ => EventType::PaymentIntentProcessing,
            },
            tap::TapWebhookKind::Refund => match body.refund_status() {
                common_enums::RefundStatus::Success => EventType::RefundSuccess,
                common_enums::RefundStatus::Failure => EventType::RefundFailure,
                _ => EventType::RefundProcessing,
            },
        };
        Ok(event)
    }

    /// The object reference in the webhook: a charge webhook yields a payment reference
    /// (charge id + merchant txn id from `reference.transaction`); a refund webhook yields a refund
    /// reference (`ref_…` id, merchant refund id from `metadata.refund_id`, and the merchant txn id
    /// from `metadata.txn_id`).
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body = parse_tap_webhook_body(&request.body)?;
        let reference = match body.classify() {
            tap::TapWebhookKind::Charge => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(body.id.clone()),
                    merchant_transaction_id: body.merchant_transaction_id(),
                })
            }
            tap::TapWebhookKind::Refund => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(body.id.clone()),
                    merchant_refund_id: body.merchant_refund_id(),
                    connector_transaction_id: body
                        .metadata
                        .as_ref()
                        .and_then(|meta| meta.txn_id.clone()),
                    merchant_transaction_id: body.merchant_transaction_id(),
                })
            }
        };
        Ok(Some(reference))
    }

    /// Parse a Tap **charge** webhook into a [`WebhookDetailsResponse`]. Status is mapped via the
    /// shared [`tap::TapWebhookBody::payment_attempt_status`] (which delegates to `map_attempt_status`);
    /// resource id is the charge id and the merchant reference is `reference.transaction`.
    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let body = parse_tap_webhook_body(&request.body)?;
        let status = body.payment_attempt_status();
        let is_failure = status == common_enums::AttemptStatus::Failure;
        let merchant_reference = body.merchant_transaction_id();

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(body.id.clone())),
            status,
            connector_response_reference_id: Some(body.id.clone()),
            connector_request_reference_id: merchant_reference,
            mandate_reference: None,
            error_code: is_failure
                .then(|| body.result_code().map(ToString::to_string))
                .flatten(),
            error_message: is_failure.then(|| body.result_message()).flatten(),
            error_reason: is_failure.then(|| body.result_message()).flatten(),
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

    /// Parse a Tap **refund** webhook into a [`RefundWebhookDetailsResponse`]. Status is mapped via
    /// the shared [`tap::TapWebhookBody::refund_status`] (which delegates to `map_refund_status`);
    /// `connector_refund_id` is the `ref_…` id and the merchant refund ref is `metadata.refund_id`.
    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let body = parse_tap_webhook_body(&request.body)?;
        let status = body.refund_status();
        let is_failure = status == common_enums::RefundStatus::Failure;

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(body.id.clone()),
            // Tap's refund webhook echoes the UCS refund reference in metadata.refund_id.
            merchant_transaction_id: body.merchant_refund_id(),
            status,
            connector_response_reference_id: Some(body.id.clone()),
            error_code: is_failure
                .then(|| body.result_code().map(ToString::to_string))
                .flatten(),
            error_message: is_failure.then(|| body.result_message()).flatten(),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Tap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Tap<T>
{
}

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Tap,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Tap,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PaymentMethodToken,
        SetupMandate,
        RepeatPayment
    ],
    not_supported: [
        VoidPC,
        VoidPostRefund,
        IncrementalAuthorization,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        MandateRevoke,
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
