mod test;
pub mod transformers;
use std::{
    fmt::Debug,
    marker::{Send, Sync},
    sync::LazyLock,
};

use base64::Engine;
use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, EventClass, PaymentMethod, PaymentMethodType,
};
use common_utils::{
    consts,
    crypto::{self, GenerateDigest, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder,
        DefendDispute, IncrementalAuthorization, PSync, PostAuthenticate, PreAuthenticate, Refund,
        RepeatPayment, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorSpecifications,
        ConnectorWebhookSecrets, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        DisputeWebhookReference, EventContext, MandateReference, MandateWebhookReference,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        SetupMandateRequestData, SubmitEvidenceData, SupportedPaymentMethodsExt,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        self, CardSpecificFeatures, ConnectorInfo, Connectors, FeatureStatus,
        PaymentMethodDataType, PaymentMethodDetails, PaymentMethodSpecificFeatures,
        SupportedPaymentMethods,
    },
    utils,
};
use error_stack::{report, ResultExt};
use hex;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, is_mandate_supported, ConnectorValidation},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as adyen, AdyenAuthenticateRequest, AdyenAuthenticationResponse, AdyenCaptureRequest,
    AdyenCaptureResponse, AdyenClientAuthRequest, AdyenClientAuthResponse,
    AdyenDefendDisputeRequest, AdyenDefendDisputeResponse, AdyenDisputeAcceptRequest,
    AdyenDisputeAcceptResponse, AdyenDisputeSubmitEvidenceRequest, AdyenIncrementalAuthRequest,
    AdyenIncrementalAuthResponse, AdyenNotificationRequestItemWH, AdyenOrderCreateRequest,
    AdyenOrderCreateResponse, AdyenPSyncResponse, AdyenPaymentRequest, AdyenPaymentResponse,
    AdyenPostAuthenticateRequest, AdyenPostAuthenticationResponse, AdyenPreAuthenticateRequest,
    AdyenPreAuthenticationResponse, AdyenRedirectRequest, AdyenRefundRequest, AdyenRefundResponse,
    AdyenRepeatPaymentRequest, AdyenRepeatPaymentResponse, AdyenReversalRequest,
    AdyenReversalResponse, AdyenSubmitEvidenceResponse, AdyenVoidRequest, AdyenVoidResponse,
    SetupMandateRequest, SetupMandateResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::IntegrationErrorContext;
use domain_types::errors::WebhookError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_API_KEY: &str = "X-Api-Key";
    pub(crate) const IDEMPOTENCY_KEY: &str = "Idempotency-Key";
}

// Type alias for non-generic trait implementations

macros::create_amount_converter_wrapper!(connector_name: Adyen, amount_type: MinorUnit);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Adyen<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Adyen,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AdyenPaymentRequest<T>,
            response_body: AdyenPaymentResponse,
            response_router_data: connector,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AdyenRedirectRequest,
            response_body: AdyenPSyncResponse,
            response_router_data: connector,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: AdyenPreAuthenticateRequest<T>,
            response_body: AdyenPreAuthenticationResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: AdyenAuthenticateRequest,
            response_body: AdyenAuthenticationResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: AdyenPostAuthenticateRequest,
            response_body: AdyenPostAuthenticationResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: AdyenCaptureRequest,
            response_body: AdyenCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AdyenVoidRequest,
            response_body: AdyenVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AdyenRefundRequest,
            response_body: AdyenRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: SetupMandateRequest<T>,
            response_body: SetupMandateResponse,
            response_router_data: connector,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: AdyenRepeatPaymentRequest,
            response_body: AdyenRepeatPaymentResponse,
            response_router_data: connector,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: Accept,
            request_body: AdyenDisputeAcceptRequest,
            response_body: AdyenDisputeAcceptResponse,
            router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,

        ),
        (
            flow: SubmitEvidence,
            request_body: AdyenDisputeSubmitEvidenceRequest,
            response_body: AdyenSubmitEvidenceResponse,
            router_data: RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,

        ),
        (
            flow: DefendDispute,
            request_body: AdyenDefendDisputeRequest,
            response_body: AdyenDefendDisputeResponse,
            router_data: RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: AdyenClientAuthRequest,
            response_body: AdyenClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: AdyenOrderCreateRequest,
            response_body: AdyenOrderCreateResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: IncrementalAuthorization,
            request_body: AdyenIncrementalAuthRequest,
            response_body: AdyenIncrementalAuthResponse,
            router_data: RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: AdyenReversalRequest,
            response_body: AdyenReversalResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self
                .get_auth_header(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            header.append(&mut api_key);
            Ok(header)
        }

        /// `build_headers` plus an `Idempotency-Key` derived from the request's own reference, so a
        /// retry of the same logical request reuses the key and Adyen replays the first response
        /// instead of acting twice (https://docs.adyen.com/development-resources/api-idempotency).
        /// The key is the lowercase hex SHA-256 of `"{operation}:{reference}"`: exactly 64
        /// characters (Adyen's maximum), and `operation` is a per-endpoint constant so two different
        /// calls sharing one attempt reference never share a key. Not sent on `/payments/details`.
        pub fn build_headers_with_idempotency<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            operation: &str,
            reference: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = self.build_headers(req)?;
            // An empty reference (merchant_capture_id / merchant_void_id / merchant_refund_id
            // unset) would give every such call on the merchant account the same key, and
            // Adyen would replay the first stored response. Without a caller-supplied id there
            // is no stable per-operation identity, so no key is sent.
            if reference.trim().is_empty() {
                return Ok(header);
            }
            let digest = crypto::Sha256
                .generate_digest(format!("{operation}:{reference}").as_bytes())
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: Some(ADYEN_IDEMPOTENCY_DOC_URL.to_string()),
                        additional_context: Some(
                            "failed to derive the Adyen Idempotency-Key digest".to_string(),
                        ),
                    },
                })?;
            header.push((
                headers::IDEMPOTENCY_KEY.to_string(),
                hex::encode(digest).into(),
            ));
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.adyen.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.adyen.base_url
        }

        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> Option<&'a str> {
            req.resource_common_data.connectors.adyen.dispute_base_url.as_deref()
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.adyen.base_url
        }
    }
);

macros::macro_connector_payout_implementation!(
    connector: Adyen,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

fn build_env_specific_endpoint(
    base_url: &str,
    test_mode: Option<bool>,
    connector_config: &ConnectorSpecificConfig,
) -> CustomResult<String, IntegrationError> {
    if test_mode.unwrap_or(true) {
        Ok(base_url.to_string())
    } else {
        let endpoint_prefix = match connector_config {
            ConnectorSpecificConfig::Adyen {
                endpoint_prefix, ..
            } => endpoint_prefix.as_deref(),
            _ => None,
        }
        .ok_or(IntegrationError::InvalidConnectorConfig {
            config: "endpoint_prefix",
            context: Default::default(),
        })?;
        Ok(base_url.replace("{{merchant_endpoint_prefix}}", endpoint_prefix))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Adyen<T>
{
    fn id(&self) -> &'static str {
        "adyen"
    }
    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = adyen::AdyenAuthType::try_from(auth_type).map_err(|_| {
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
        })?;
        Ok(vec![(
            headers::X_API_KEY.to_string(),
            auth.api_key.into_masked(),
        )])
    }
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.adyen.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Flows outside this connector's flow-aware error mapping (disputes,
        // IncrementalAuthorization, VoidPC, CreateOrder, ClientAuthenticationToken) keep a
        // non-terminal `attempt_status: None`.
        build_adyen_error_response(res, event_builder, AdyenErrorFlow::Unclassified)
    }
}

/// Which flow an Adyen error body belongs to. The same HTTP error means different things per
/// flow (a validation error on `/captures` is a capture failure, not a payment failure), so
/// `attempt_status` is chosen per flow rather than left unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdyenErrorFlow {
    /// `POST /payments` for the Authorize flow only; decided by HTTP status and body presence
    /// (see `adyen_authorize_error_attempt_status`). SetupMandate and RepeatPayment use `Payment`.
    Authorize,
    Payment,
    Capture,
    Void,
    Refund,
    Sync,
    Authentication,
    Unclassified,
}

/// Adyen error `704`: "request already processed or in progress" (idempotency replay while the
/// original is still running). https://docs.adyen.com/development-resources/api-idempotency
const ADYEN_ERROR_CODE_ALREADY_PROCESSING: &str = "704";
/// Adyen error `703`: idempotency store unavailable, transient.
const ADYEN_ERROR_CODE_IDEMPOTENCY_UNAVAILABLE: &str = "703";
/// Adyen error `705`: rate limited, transient.
const ADYEN_ERROR_CODE_RATE_LIMITED: &str = "705";
const ADYEN_IDEMPOTENCY_DOC_URL: &str =
    "https://docs.adyen.com/development-resources/api-idempotency";

/// `attempt_status` for an Authorize (`POST /payments`) error, decided by HTTP status and whether
/// the body carried a `ServiceError` `errorCode`, never by `errorType`
/// (https://docs.adyen.com/development-resources/response-handling):
/// - `703`, `704`, `705`, or HTTP 408 / 409 / 429: transient, duplicate or rate-limited; the
///   original may still be in flight, so `Pending`.
/// - 4xx (except 408 / 409 / 429): Adyen does not accept or store a request that fails
///   validation, so `Failure`.
/// - HTTP 500 with a `ServiceError` body (any `errorType`, e.g. `701` internal): the request was
///   rejected with an error message and is not stored or charged, so `Failure`.
/// - any other status, including 5xx with no parseable body: nothing tells us whether Adyen
///   processed the request, so `Pending` (resolved by PSync / the `AUTHORISATION` webhook).
fn adyen_authorize_error_attempt_status(
    status_code: u16,
    error_code: Option<&str>,
) -> domain_types::router_data::FlowStatus {
    use domain_types::router_data::FlowStatus;
    let is_transient_code = matches!(
        error_code,
        Some(
            ADYEN_ERROR_CODE_ALREADY_PROCESSING
                | ADYEN_ERROR_CODE_IDEMPOTENCY_UNAVAILABLE
                | ADYEN_ERROR_CODE_RATE_LIMITED
        )
    );
    let status = match status_code {
        _ if is_transient_code => AttemptStatus::Pending,
        408 | 409 | 429 => AttemptStatus::Pending,
        400..=499 => AttemptStatus::Failure,
        500 if error_code.is_some() => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    };
    FlowStatus::Payment(status)
}

/// `attempt_status` for an Adyen error body (https://docs.adyen.com/development-resources/response-handling).
/// Authorize is delegated to `adyen_authorize_error_attempt_status`.
/// - `704` (already processed / in progress): the original request may still succeed, so the
///   money-moving flows stay pending; Sync and Authentication stay unset.
/// - HTTP 429 / 5xx, `703` or `errorType: internal`: transient, nothing is known to have failed.
/// - `errorType` validation / security / configuration on a 4xx: Adyen rejected the request and
///   did not act on it, so it is a terminal failure of that flow. Sync never fails the payment.
/// - anything else stays unset (non-terminal).
fn adyen_error_attempt_status(
    flow: AdyenErrorFlow,
    status_code: u16,
    error_code: Option<&str>,
    error_type: Option<&adyen::AdyenErrorType>,
) -> Option<domain_types::router_data::FlowStatus> {
    use domain_types::router_data::FlowStatus;
    if flow == AdyenErrorFlow::Authorize {
        return Some(adyen_authorize_error_attempt_status(
            status_code,
            error_code,
        ));
    }
    let pending = || match flow {
        AdyenErrorFlow::Authorize
        | AdyenErrorFlow::Payment
        | AdyenErrorFlow::Capture
        | AdyenErrorFlow::Void => Some(FlowStatus::Payment(AttemptStatus::Pending)),
        AdyenErrorFlow::Refund => Some(FlowStatus::Refund(common_enums::RefundStatus::Pending)),
        AdyenErrorFlow::Sync | AdyenErrorFlow::Authentication | AdyenErrorFlow::Unclassified => {
            None
        }
    };
    if error_code == Some(ADYEN_ERROR_CODE_ALREADY_PROCESSING) {
        return pending();
    }
    let is_transient = status_code == 429
        || (500..=599).contains(&status_code)
        || error_code == Some(ADYEN_ERROR_CODE_IDEMPOTENCY_UNAVAILABLE)
        || matches!(error_type, Some(adyen::AdyenErrorType::Internal));
    if is_transient {
        return pending();
    }
    let is_rejected = (400..=499).contains(&status_code)
        && matches!(
            error_type,
            Some(
                adyen::AdyenErrorType::Validation
                    | adyen::AdyenErrorType::Security
                    | adyen::AdyenErrorType::Configuration
            )
        );
    if !is_rejected {
        return None;
    }
    match flow {
        AdyenErrorFlow::Authorize | AdyenErrorFlow::Payment => {
            Some(FlowStatus::Payment(AttemptStatus::Failure))
        }
        AdyenErrorFlow::Capture => Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
        AdyenErrorFlow::Void => Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
        AdyenErrorFlow::Refund => Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
        AdyenErrorFlow::Authentication => {
            Some(FlowStatus::Payment(AttemptStatus::AuthenticationFailed))
        }
        AdyenErrorFlow::Sync | AdyenErrorFlow::Unclassified => None,
    }
}

/// Parses an Adyen error body (`ServiceError`) into an [`ErrorResponse`] whose
/// `attempt_status` is chosen per [`AdyenErrorFlow`].
pub fn build_adyen_error_response(
    res: Response,
    event_builder: Option<&mut events::Event>,
    flow: AdyenErrorFlow,
) -> CustomResult<ErrorResponse, ConnectorError> {
    if res.response.is_empty() {
        return Ok(ErrorResponse {
            status_code: res.status_code,
            code: consts::NO_ERROR_CODE.to_string(),
            message: consts::NO_ERROR_MESSAGE.to_string(),
            reason: None,
            attempt_status: adyen_error_attempt_status(flow, res.status_code, None, None),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        });
    }

    let response: Result<
        adyen::AdyenErrorResponse,
        error_stack::Report<common_utils::errors::ParsingError>,
    > = res.response.parse_struct("ErrorResponse");

    match response {
        Ok(response) => {
            with_error_response_body!(event_builder, response);
            let typed =
                macros::serialize_typed_connector_payload(&response, "typed_connector_response");
            let attempt_status = adyen_error_attempt_status(
                flow,
                res.status_code,
                Some(response.error_code.as_str()),
                response.error_type.as_ref(),
            );
            // Mirror hyperswitch: message -> title -> placeholder.
            let message = response
                .message
                .or(response.title)
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.error_code,
                message: message.clone(),
                reason: Some(message),
                attempt_status,
                connector_transaction_id: response.psp_reference,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
                typed_connector_response: typed,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        }
        Err(error_msg) => {
            if let Some(event) = event_builder {
                event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}));
            }
            tracing::error!(deserialization_error =? error_msg);
            let status_code = res.status_code;
            utils::handle_json_response_deserialization_failure(res, "adyen").map(
                |mut error_response| {
                    // Authorize: a non-JSON body carries no `errorCode`, so the status follows
                    // the HTTP status alone (5xx -> Pending) instead of staying unset.
                    if flow == AdyenErrorFlow::Authorize {
                        error_response.attempt_status =
                            Some(adyen_authorize_error_attempt_status(status_code, None));
                    }
                    error_response
                },
            )
        }
    }
}

const ADYEN_API_VERSION: &str = "v68";
/// `Idempotency-Key` operation for `POST /payments` (see `build_headers_with_idempotency`).
const ADYEN_IDEMPOTENCY_OP_PAYMENTS: &str = "payments";
/// `Idempotency-Key` operation for the zero-value tokenization `POST /payments` (SetupMandate).
const ADYEN_IDEMPOTENCY_OP_SETUP: &str = "setup";
/// `Idempotency-Key` operation for the merchant-initiated `POST /payments` (RepeatPayment).
const ADYEN_IDEMPOTENCY_OP_MIT: &str = "mit";
/// `Idempotency-Key` operation for `POST /payments/{paymentPspReference}/captures`.
const ADYEN_IDEMPOTENCY_OP_CAPTURES: &str = "captures";
/// `Idempotency-Key` operation for the standalone-authentication `POST /payments`. Deliberately
/// not `payments`: that leg and Authorize share a `connector_request_reference_id`, so one
/// operation name would give both the same key and Adyen would replay the authentication
/// response for the money call.
const ADYEN_IDEMPOTENCY_OP_AUTHN: &str = "authn";
/// `Idempotency-Key` operation for `POST /payments/{paymentPspReference}/cancels`.
const ADYEN_IDEMPOTENCY_OP_CANCELS: &str = "cancels";
/// `Idempotency-Key` operation for `POST /payments/{paymentPspReference}/refunds`.
const ADYEN_IDEMPOTENCY_OP_REFUNDS: &str = "refunds";
const ADYEN_CANCELS_DOC_URL: &str =
    "https://docs.adyen.com/api-explorer/Checkout/68/post/payments/-paymentPspReference-/cancels";
/// Checkout version used by the standalone authentication legs: `authenticationData` does not
/// exist on v68 (measured: Adyen answers `702 ... unknown fields: [authenticationData]`).
const ADYEN_AUTHENTICATION_API_VERSION: &str = "v71";
const ADYEN_AMOUNT_UPDATES_DOC_URL: &str =
    "https://docs.adyen.com/api-explorer/Checkout/68/post/payments/-paymentPspReference-/amountUpdates";

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenPaymentRequest),
    curl_response: AdyenPaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize ],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_with_idempotency(
                req,
                ADYEN_IDEMPOTENCY_OP_PAYMENTS,
                &req.resource_common_data.connector_request_reference_id,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments"))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authorize)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authorize)
        }
    }
);

macros::macro_connector_implementation!(
    // get_error_response_v2 is overridden below with the Authentication-flow error mapping, so it
    // must not also come from the defaults.
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenPreAuthenticateRequest),
    curl_response: AdyenPreAuthenticationResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize ],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // This leg is a `POST /payments` that mints a pspReference, so a retry without a key
            // would start a second authentication for the same attempt.
            self.build_headers_with_idempotency(
                req,
                ADYEN_IDEMPOTENCY_OP_AUTHN,
                &req.resource_common_data.connector_request_reference_id,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_AUTHENTICATION_API_VERSION}/payments"))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authentication)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authentication)
        }
    }
);

macros::macro_connector_implementation!(
    // get_error_response_v2 is overridden below with the Authentication-flow error mapping, so it
    // must not also come from the defaults.
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenAuthenticateRequest),
    curl_response: AdyenAuthenticationResponse,
    flow_name: Authenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize ],
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
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_AUTHENTICATION_API_VERSION}/payments/details"))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authentication)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authentication)
        }
    }
);

macros::macro_connector_implementation!(
    // get_error_response_v2 is overridden below with the Authentication-flow error mapping, so it
    // must not also come from the defaults.
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenPostAuthenticateRequest),
    curl_response: AdyenPostAuthenticationResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize ],
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
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_AUTHENTICATION_API_VERSION}/payments/details"))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authentication)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Authentication)
        }
    }
);

macros::macro_connector_implementation!(
    // get_error_response_v2 is overridden below with the Sync-flow error mapping, so it must not
    // also come from the defaults.
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenRedirectRequest),
    curl_response: AdyenPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize ],
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
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/details"))
        }
        // A failed `/payments/details` call (e.g. 422 `14_0391` invalid redirectResult) says
        // nothing about the payment itself, so the Sync arm leaves `attempt_status` unset and an
        // inconclusive sync never stamps a terminal status. No Idempotency-Key: the call only
        // reads back the redirect outcome.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Sync)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Sync)
        }
        fn build_request_v2(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
            // For wallet redirects, encoded_data may be None
            // In such cases, gracefully skip the psync request
            if req.request.encoded_data.clone().is_some() {
                // Build the request normally if encoded_data is present
                let url = self.get_url(req)?;
                let headers = self.get_headers(req)?;
                let request_data = ConnectorIntegrationV2::<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>::get_request_body(self, req)?;
                let (body, typed_request_value) = match request_data {
                    Some(data) => (
                        Some(data.content),
                        data.typed_request.map(|msv| msv.inner().clone()),
                    ),
                    None => (None, None),
                };

                Ok(Some(
                    common_utils::request::RequestBuilder::new()
                        .method(common_utils::request::Method::Post)
                        .url(&url)
                        .attach_default_headers()
                        .headers(headers)
                        .set_optional_body(body)
                        .set_typed_connector_request(typed_request_value)
                        .build(),
                ))
            } else {
                // For wallet redirects without encoded_data, return None
                // This allows the system to rely on webhooks for payment status
                Ok(None)
            }
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenCaptureRequest),
    curl_response: AdyenCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // The Idempotency-Key reference is the same `reference` the capture body sends: the
        // capture id for a multiple capture, else the attempt reference. A retried capture
        // therefore replays the first `/captures` response instead of capturing twice.
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let reference = req
                .request
                .multiple_capture_data
                .as_ref()
                .map(|data| data.capture_reference.as_str())
                .unwrap_or(req.resource_common_data.connector_request_reference_id.as_str());
            self.build_headers_with_idempotency(req, ADYEN_IDEMPOTENCY_OP_CAPTURES, reference)
        }
        // A 4xx validation/security/configuration error on `/captures` means Adyen did not act
        // on the capture (CaptureFailed); `704`, 429 and 5xx leave it Pending.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Capture)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Capture)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = match &req.request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) => id,
                _ => {
                    return Err(
                        IntegrationError::MissingConnectorTransactionID { context: Default::default() }.into(),
                    )
                }
            };
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/{id}/captures"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenIncrementalAuthRequest),
    curl_response: AdyenIncrementalAuthResponse,
    flow_name: IncrementalAuthorization,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsIncrementalAuthorizationData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Propagate the original authorization's connector_transaction_id \
                             (Adyen pspReference) to the IncrementalAuthorization request."
                                .to_string(),
                        ),
                        doc_url: Some(ADYEN_AMOUNT_UPDATES_DOC_URL.to_string()),
                        additional_context: Some(
                            "connector_transaction_id is required as the paymentPspReference path \
                             segment for the /amountUpdates endpoint."
                                .to_string(),
                        ),
                    },
                })?;
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/{id}/amountUpdates"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Adyen<T>
{
    /// Routes a 3DS card payment through the native 3DS2 ladder.
    ///
    /// Both client returns (the 3DS method iframe and the ACS challenge) come back "with
    /// params", so `redirect_state` alone cannot tell leg 2 from leg 3; `completed_step` is the
    /// disambiguator. The flag cannot be read here: this method gets no connector config, and
    /// `Adyen<T>` (generated by `create_all_prerequisites!`) holds only bridges and amount
    /// converters. The request builders are therefore the place that decides whether
    /// `nativeThreeDS: preferred` is sent.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                // Entering the ladder is deliberately left to an explicit PreAuthenticate call,
                // for two reasons. This hook receives no connector config, so it cannot tell
                // whether `native_three_ds` is on for the merchant; and `completed_step` does
                // not survive between composite requests - `AuthorizeCompositeState::default()`
                // is rebuilt per request in composite-service/src/payments.rs - so the shopper's
                // second and third returns both arrive as `(RedirectWithParams, None)` and would
                // fall through to Authorize, authorising without the authentication result.
                // Hyperswitch does not use the composite service: it drives these legs through
                // its own ConnectorSpecifications predicates and the PaymentMethodAuthentication
                // RPCs, which is the path this flow is verified on. Routing from here needs the
                // composite loop to carry `completed_step` across requests first.
                (RedirectState::InitialRequest, None) => AuthenticationStep::Authorize,
                (RedirectState::RedirectWithParams, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authenticate
                }
                (RedirectState::RedirectWithParams, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::PostAuthenticate
                }
                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }

    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        if data.encoded_data.is_some() {
            return Ok(());
        }
        Err(IntegrationError::MissingRequiredField {
            field_name: "encoded_data",
            context: Default::default(),
        }
        .into())
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Adyen<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenOrderCreateRequest),
    curl_response: AdyenOrderCreateResponse,
    flow_name: CreateOrder,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentCreateOrderData,
    flow_response: PaymentCreateOrderResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/orders"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenVoidRequest),
    curl_response: AdyenVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // The Idempotency-Key reference is the same `reference` the cancel body sends (seeded
        // from merchant_void_id), so a retried cancel replays the first `/cancels` response.
        // With merchant_void_id unset the reference is empty and no key is sent (guard in
        // build_headers_with_idempotency).
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_with_idempotency(
                req,
                ADYEN_IDEMPOTENCY_OP_CANCELS,
                &req.resource_common_data.connector_request_reference_id,
            )
        }
        // A 4xx validation/security/configuration error on `/cancels` means Adyen did not act
        // on the cancel (VoidFailed); `704`, 429 and 5xx leave it Pending.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Void)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Void)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = &req.request.connector_transaction_id;
            if id.trim().is_empty() {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Propagate the original authorization's connector_transaction_id \
                             (Adyen pspReference) to the Void request."
                                .to_string(),
                        ),
                        doc_url: Some(ADYEN_CANCELS_DOC_URL.to_string()),
                        additional_context: Some(
                            "connector_transaction_id is required as the paymentPspReference path \
                             segment for the /cancels endpoint."
                                .to_string(),
                        ),
                    },
                }
                .into());
            }
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/{id}/cancels"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenReversalRequest),
    curl_response: AdyenReversalResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_transaction_id.clone();
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/{id}/reversals"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDefendDisputeRequest),
    curl_response: AdyenDefendDisputeResponse,
    flow_name: DefendDispute,
    resource_common_data: DisputeFlowData,
    flow_request: DisputeDefendData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TODO: Add build_env_specific_endpoint when DisputeFlowData has test_mode and connector_feature_data fields
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?;
            Ok(format!("{dispute_url}ca/services/DisputeService/v30/defendDispute"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Adyen<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body.clone()).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let hmac_signature = notif
            .additional_data
            .hmac_signature
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Missing hmacSignature in Adyen webhook additional_data")?;

        Ok(hmac_signature.as_bytes().to_vec())
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body.clone()).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        // Adyen HMAC message format: pspReference:originalReference:merchantAccountCode:merchantReference:amount.value:amount.currency:eventCode:success
        let message = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            notif.psp_reference,
            notif.original_reference.as_deref().unwrap_or(""),
            notif.merchant_account_code,
            notif.merchant_reference,
            notif.amount.value,
            notif.amount.currency,
            notif.event_code,
            notif.success
        );

        Ok(message.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Adyen Standard webhooks: HMAC-SHA256 over the 8-field colon-joined preimage, keyed by
        // the hex-decoded HMAC key, Base64 in additionalData.hmacSignature.
        // https://docs.adyen.com/development-resources/webhooks/secure-webhooks/verify-hmac-signatures
        let algorithm = crypto::HmacSha256;

        let connector_webhook_secrets = connector_webhook_secret
            .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))
            .attach_printable("Missing webhook secret for Adyen verification")?;

        // G-IncomingWebhook-01: an empty primary key never verifies anything.
        if connector_webhook_secrets.secret.is_empty() {
            return Err(report!(WebhookError::WebhookVerificationSecretNotFound))
                .attach_printable("Empty webhook secret for Adyen verification");
        }

        let primary_key = hex::decode(&connector_webhook_secrets.secret)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)
            .attach_printable("Adyen webhook HMAC key is not hex-encoded")?;

        // Key rotation: Adyen asks merchants to keep accepting the previous key while a new one
        // propagates, so a hex additional_secret is tried as a second key. A non-hex
        // additional_secret is not an Adyen HMAC key and is skipped.
        let rotation_key = connector_webhook_secrets
            .additional_secret
            .as_ref()
            .filter(|key| !key.peek().is_empty())
            .and_then(|key| hex::decode(key.peek()).ok());

        let signature_text =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        // G-IncomingWebhook-03: the signature is Base64 text; compare raw HMAC bytes.
        let signature = consts::BASE64_ENGINE
            .decode(&signature_text)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Adyen hmacSignature is not valid Base64")?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        // verify_signature is a constant-time HMAC comparison.
        let mut source_verified = false;
        for key in std::iter::once(primary_key).chain(rotation_key) {
            let key_matches = algorithm
                .verify_signature(&key, &signature, &message)
                .change_context(WebhookError::WebhookSourceVerificationFailed)
                .attach_printable("Failed to verify Adyen HMAC signature")?;
            source_verified |= key_matches;
        }

        Ok(source_verified)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"notificationItems":[{"NotificationRequestItem":{"pspReference":"probe_ref_001","merchantReference":"probe_order_001","merchantAccountCode":"ProbeAccount","eventCode":"AUTHORISATION","success":"true","amount":{"currency":"USD","value":1000},"additionalData":{}}}]}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        if request.body.is_empty() {
            return Ok(domain_types::connector_types::EventType::EndpointVerification);
        }
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;
        transformers::get_adyen_webhook_event_type(
            notif.event_code,
            notif.success,
            notif.additional_data.dispute_status,
        )
        .map_err(|e| report!(e))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        use transformers::WebhookEventCode;

        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let reference = match notif.event_code {
            // Capture/cancellation/adjustment events: psp_reference is the event's own PSP ref;
            // original_reference is the parent authorisation's PSP ref — that's the lookup key.
            WebhookEventCode::Capture
            | WebhookEventCode::CaptureFailed
            | WebhookEventCode::Cancellation
            | WebhookEventCode::TechnicalCancel
            | WebhookEventCode::AuthorisationAdjustment => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: notif.original_reference,
                    merchant_transaction_id: Some(notif.merchant_reference),
                    connector_preprocessing_id: None,
                })
            }
            // AUTHORISATION-like events carry both the Adyen PSP reference and
            // the merchant reference. Return both so Prism remains a transparent
            // parse layer; HS is responsible for choosing the lookup key.
            WebhookEventCode::Authorisation | WebhookEventCode::OfferClosed => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(notif.psp_reference),
                    merchant_transaction_id: Some(notif.merchant_reference),
                    connector_preprocessing_id: None,
                })
            }
            // RECURRING_CONTRACT is a mandate event: pspReference is the stored-payment token
            // (recurringDetailReference), not a payment id (spec "RECURRING_CONTRACT — decision").
            WebhookEventCode::RecurringContract => {
                WebhookResourceReference::Mandate(MandateWebhookReference {
                    connector_mandate_id: Some(notif.psp_reference),
                    merchant_transaction_id: Some(notif.merchant_reference),
                })
            }
            // EXPIRE: the expired authorisation is originalReference when Adyen sends it,
            // otherwise the event's own pspReference.
            WebhookEventCode::Expire => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(
                        notif
                            .original_reference
                            .filter(|reference| !reference.is_empty())
                            .unwrap_or(notif.psp_reference),
                    ),
                    merchant_transaction_id: Some(notif.merchant_reference),
                    connector_preprocessing_id: None,
                })
            }
            // Refund events carry the refund PSP reference, merchant refund
            // reference, and parent payment PSP reference. Return all of them.
            WebhookEventCode::Refund
            | WebhookEventCode::CancelOrRefund
            | WebhookEventCode::RefundFailed
            | WebhookEventCode::RefundReversed => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(notif.psp_reference),
                    merchant_refund_id: Some(notif.merchant_reference),
                    connector_transaction_id: notif.original_reference,
                    merchant_transaction_id: None,
                })
            }
            // Dispute events carry the dispute PSP reference and the parent
            // payment PSP reference (originalReference). Return both so Prism
            // stays a transparent parse layer; the HS bridge prefers the
            // parent payment reference (connector_transaction_id) for the
            // payment lookup, matching the Direct path.
            WebhookEventCode::NotificationOfChargeback
            | WebhookEventCode::Chargeback
            | WebhookEventCode::ChargebackReversed
            | WebhookEventCode::PrearbitrationWon
            | WebhookEventCode::SecondChargeback
            | WebhookEventCode::PrearbitrationLost
            | WebhookEventCode::RequestForInformation
            | WebhookEventCode::InformationSupplied
            | WebhookEventCode::PrearbitrationOpen
            | WebhookEventCode::PrearbitrationAccepted
            | WebhookEventCode::PrearbitrationDeclined
            | WebhookEventCode::PrearbitrationIssuerWithdrawn
            | WebhookEventCode::SchemeArbitration
            | WebhookEventCode::SchemeArbitrationWon
            | WebhookEventCode::SchemeArbitrationLost
            | WebhookEventCode::DisputeDefensePeriodEnded
            | WebhookEventCode::IssuerResponseTimeframeExpired => {
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(notif.psp_reference),
                    connector_transaction_id: notif.original_reference,
                })
            }
            WebhookEventCode::NotificationOfFraud | WebhookEventCode::IssuerComments => {
                return Ok(None);
            }
            // Unknown: no actionable reference.
            WebhookEventCode::Unknown => return Ok(None),
        };

        Ok(Some(reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let capture_method = event_context.and_then(|context| context.capture_method);
        let status = transformers::get_adyen_payment_webhook_event(
            notif.event_code.clone(),
            notif.success.clone(),
            capture_method,
        )?;

        let is_recurring_contract = matches!(
            notif.event_code,
            transformers::WebhookEventCode::RecurringContract
        );

        // Failure events carry `reason` and, for refusals, additionalData.refusalReasonCode;
        // success events carry neither, so they are read only on a failure status.
        let (error_code, error_message, error_reason) = match status {
            // RECURRING_CONTRACT has no payment status; success "false" means the contract was
            // not created, and the failure carries `reason`.
            _ if is_recurring_contract => {
                if notif.success == "true" {
                    (None, None, None)
                } else {
                    let message = notif
                        .reason
                        .clone()
                        .filter(|reason| !reason.is_empty())
                        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
                    (
                        Some(
                            notif
                                .additional_data
                                .refusal_reason_code
                                .clone()
                                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                        ),
                        Some(message.clone()),
                        Some(message),
                    )
                }
            }
            AttemptStatus::Failure | AttemptStatus::CaptureFailed | AttemptStatus::VoidFailed => {
                let message = notif
                    .reason
                    .clone()
                    .filter(|reason| !reason.is_empty())
                    .or_else(|| notif.additional_data.refusal_reason.clone())
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
                (
                    Some(
                        notif
                            .additional_data
                            .refusal_reason_code
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                    ),
                    Some(message.clone()),
                    Some(message),
                )
            }
            _ => (None, None, None),
        };

        // Modification events (capture, cancel, adjustment) carry the payment in
        // originalReference; pspReference is the modification itself. Keep one id per payment.
        let payment_reference = match notif.event_code {
            transformers::WebhookEventCode::Capture
            | transformers::WebhookEventCode::CaptureFailed
            | transformers::WebhookEventCode::Cancellation
            | transformers::WebhookEventCode::TechnicalCancel
            | transformers::WebhookEventCode::AuthorisationAdjustment => notif
                .original_reference
                .clone()
                .filter(|reference| !reference.is_empty())
                .unwrap_or_else(|| notif.psp_reference.clone()),
            _ => notif.psp_reference.clone(),
        };

        // RECURRING_CONTRACT: the payment is originalPsp (initial payment) when present, never
        // pspReference, which is the token; the token is the mandate's connector_mandate_id
        // (it equals additionalData recurring.recurringDetailReference).
        let (resource_id, mandate_reference) = if is_recurring_contract {
            (
                notif
                    .original_psp
                    .clone()
                    .filter(|reference| !reference.is_empty())
                    .map(ResponseId::ConnectorTransactionId),
                Some(Box::new(MandateReference {
                    connector_mandate_id: Some(notif.psp_reference.clone()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
            )
        } else {
            (
                Some(ResponseId::ConnectorTransactionId(payment_reference)),
                transformers::get_adyen_mandate_reference_from_webhook(&notif),
            )
        };
        let network_txn_id = transformers::get_adyen_network_txn_id_from_webhook(&notif);
        let payment_method_update =
            transformers::get_adyen_payment_method_update_from_webhook(&notif);

        Ok(WebhookDetailsResponse {
            connector_returned_payment_method_details: None,
            resource_id,
            status,
            connector_response_reference_id: Some(notif.psp_reference.clone()),
            connector_request_reference_id: Some(notif.psp_reference),
            error_code,
            mandate_reference,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            error_reason,
            network_txn_id,
            payment_method_update,
            sender_payment_instrument_id: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let (error_code, error_message) = if transformers::get_adyen_refund_webhook_event(
            notif.event_code.clone(),
            notif.success.clone(),
        )? == common_enums::RefundStatus::Failure
        {
            // REFUND_FAILED carries additionalData.refusalReason / refusalReasonCode; fall back
            // to the top-level `reason` text when present (spec "Refund-specific additionalData").
            (
                Some(
                    notif
                        .additional_data
                        .refusal_reason_code
                        .clone()
                        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                ),
                Some(
                    notif
                        .reason
                        .clone()
                        .filter(|reason| !reason.is_empty())
                        .or_else(|| notif.additional_data.refusal_reason.clone())
                        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                ),
            )
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(notif.psp_reference.clone()),
            merchant_transaction_id: None,
            status: transformers::get_adyen_refund_webhook_event(notif.event_code, notif.success)?,
            connector_response_reference_id: Some(notif.psp_reference.clone()),
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::DisputeWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let request_body_copy = request.body.clone();
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;
        let (stage, status) = transformers::get_dispute_stage_and_status(
            notif.event_code,
            notif.additional_data.dispute_status,
        )?;

        let amount = utils::convert_amount_for_webhook(
            self.amount_converter,
            notif.amount.value,
            notif.amount.currency,
        )?;

        Ok(
            domain_types::connector_types::DisputeWebhookDetailsResponse {
                amount,
                currency: notif.amount.currency,
                dispute_id: notif.psp_reference.clone(),
                stage,
                status,
                connector_response_reference_id: Some(notif.psp_reference.clone()),
                dispute_message: notif.reason,
                connector_reason_code: notif.additional_data.chargeback_reason_code,
                additional_details: None,
                raw_connector_response: Some(
                    String::from_utf8_lossy(&request_body_copy).to_string(),
                ),
                status_code: 200,
                response_headers: None,
            },
        )
    }

    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, error_stack::Report<WebhookError>> {
        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: Some(b"[accepted]".to_vec()),
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenRefundRequest),
    curl_response: AdyenRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // The Idempotency-Key reference is the same `reference` the refund body sends (the
        // refund id), so a retried refund replays the first `/refunds` response instead of
        // refunding twice, while two partial refunds of one payment get distinct keys.
        // With merchant_refund_id unset the reference is empty and no key is sent (guard in
        // build_headers_with_idempotency).
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_with_idempotency(
                req,
                ADYEN_IDEMPOTENCY_OP_REFUNDS,
                &req.request.refund_id,
            )
        }
        // A 4xx validation/security/configuration error on `/refunds` means Adyen did not act
        // on the refund (Refund Failure); `704`, 429 and 5xx leave it Pending.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Refund)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Refund)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req.request.connector_transaction_id.clone();

            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_refunds(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!(
                "{endpoint}{ADYEN_API_VERSION}/payments/{connector_payment_id}/refunds",
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(SetupMandateRequest),
    curl_response: SetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_with_idempotency(
                req,
                ADYEN_IDEMPOTENCY_OP_SETUP,
                &req.resource_common_data.connector_request_reference_id,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments"))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Payment)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Payment)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDisputeAcceptRequest),
    curl_response: AdyenDisputeAcceptResponse,
    flow_name: Accept,
    resource_common_data: DisputeFlowData,
    flow_request: AcceptDisputeData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TODO: Add build_env_specific_endpoint when DisputeFlowData has test_mode and connector_feature_data fields
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?;
            Ok(format!("{dispute_url}ca/services/DisputeService/v30/acceptDispute"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDisputeSubmitEvidenceRequest),
    curl_response: AdyenSubmitEvidenceResponse,
    flow_name: SubmitEvidence,
    resource_common_data: DisputeFlowData,
    flow_request: SubmitEvidenceData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TODO: Add build_env_specific_endpoint when DisputeFlowData has test_mode and connector_feature_data fields
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?;
            Ok(format!("{dispute_url}ca/services/DisputeService/v30/supplyDefenseDocument"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Adyen,
    curl_request: Json(AdyenRepeatPaymentRequest),
    curl_response: AdyenRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_with_idempotency(
                req,
                ADYEN_IDEMPOTENCY_OP_MIT,
                &req.resource_common_data.connector_request_reference_id,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments"))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Payment)
        }
        fn get_5xx_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            build_adyen_error_response(res, event_builder, AdyenErrorFlow::Payment)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenClientAuthRequest),
    curl_response: AdyenClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_merchant_auth(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/sessions"))
        }
    }
);

static ADYEN_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> = LazyLock::new(|| {
    let adyen_supported_capture_methods = vec![
        CaptureMethod::Automatic,
        CaptureMethod::Manual,
        CaptureMethod::ManualMultiple,
        // CaptureMethod::Scheduled,
    ];

    let adyen_supported_card_network = vec![
        CardNetwork::AmericanExpress,
        CardNetwork::CartesBancaires,
        CardNetwork::UnionPay,
        CardNetwork::DinersClub,
        CardNetwork::Discover,
        CardNetwork::Interac,
        CardNetwork::JCB,
        CardNetwork::Maestro,
        CardNetwork::Mastercard,
        CardNetwork::Visa,
    ];

    let mut adyen_supported_payment_methods = SupportedPaymentMethods::new();

    adyen_supported_payment_methods.add(
        PaymentMethod::Card,
        PaymentMethodType::Card,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: Some(PaymentMethodSpecificFeatures::Card(CardSpecificFeatures {
                three_ds: FeatureStatus::Supported,
                no_three_ds: FeatureStatus::Supported,
                supported_card_networks: adyen_supported_card_network.clone(),
            })),
        },
    );

    // Bank Debit - ACH
    adyen_supported_payment_methods.add(
        PaymentMethod::BankDebit,
        PaymentMethodType::Ach,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Bank Debit - SEPA
    adyen_supported_payment_methods.add(
        PaymentMethod::BankDebit,
        PaymentMethodType::Sepa,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Bank Debit - BACS
    adyen_supported_payment_methods.add(
        PaymentMethod::BankDebit,
        PaymentMethodType::Bacs,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Wallet - Apple Pay (PSP decryption via applePayToken, or merchant/Hyperswitch decryption as
    // a scheme card with mpiData). Mandates via storePaymentMethod, as Hyperswitch declares.
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::ApplePay,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Wallet - Google Pay (PSP decryption via googlePayToken, or merchant/Hyperswitch decryption).
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::GooglePay,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Wallet - AliPayHk (Redirect)
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::AliPayHk,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Wallet - WeChatPay (Redirect)
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::WeChatPay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Wallet - MobilePay (Redirect)
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::MobilePay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Wallet - MbWay (Redirect)
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::MbWay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    adyen_supported_payment_methods
});

static ADYEN_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Adyen", 
    description: "Adyen is a Dutch payment company with the status of an acquiring bank that allows businesses to accept e-commerce, mobile, and point-of-sale payments. It is listed on the stock exchange Euronext Amsterdam.",
    connector_type: types::PaymentConnectorCategory::PaymentGateway
};

static ADYEN_SUPPORTED_WEBHOOK_FLOWS: &[EventClass] = &[
    EventClass::Payments,
    EventClass::Refunds,
    EventClass::Disputes,
];

impl ConnectorSpecifications for Adyen<DefaultPCIHolder> {
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&ADYEN_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&ADYEN_SUPPORTED_PAYMENT_METHODS)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [EventClass]> {
        Some(ADYEN_SUPPORTED_WEBHOOK_FLOWS)
    }
}

impl ConnectorValidation for Adyen<DefaultPCIHolder> {
    fn validate_mandate_payment(
        &self,
        pm_type: Option<PaymentMethodType>,
        pm_data: PaymentMethodData<DefaultPCIHolder>,
    ) -> CustomResult<(), IntegrationError> {
        let mandate_supported_pmd = std::collections::HashSet::from([
            PaymentMethodDataType::Card,
            PaymentMethodDataType::ApplePay,
            PaymentMethodDataType::GooglePay,
            PaymentMethodDataType::AchBankDebit,
            PaymentMethodDataType::SepaBankDebit,
            PaymentMethodDataType::BecsBankDebit,
        ]);
        is_mandate_supported(pm_data, pm_type, mandate_supported_pmd, self.id())
    }

    fn is_webhook_source_verification_mandatory(&self) -> bool {
        false
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Adyen,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        MandateRevoke,
        PaymentMethodToken,
    ],
    not_supported: [
        // Adyen has no refund-status API; refund outcomes arrive only via REFUND / REFUND_FAILED webhooks.
        RSync,
        VoidPostRefund,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
);
