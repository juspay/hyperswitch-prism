pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{
        FrmChargebackReceived, FrmPaymentOutcome, FrmRefundProcessed, PostRiskCheck, PreRiskCheck,
        ServerAuthenticationToken,
    },
    connector_types::{
        EventType, PaymentWebhookReference, RequestDetails, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
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
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as clearsale;
use transformers::{
    ClearSaleAuthenticateRequest, ClearSaleAuthenticateResponse, ClearSaleOrderCreateRequest,
    ClearSaleOrderCreateResponse, ClearSaleOrderReanalysisRequest,
    ClearSaleOrderReanalysisResponse, ClearSaleStatusUpdateRequest, ClearSaleStatusUpdateResponse,
    ClearSaleWebhookPayload,
};

use super::super::connectors::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

/// Login endpoint (spec §3).
const CLEARSALE_AUTHENTICATE_PATH: &str = "/v1/authenticate";
/// Order create / re-analysis endpoint (spec §4).
const CLEARSALE_ORDERS_PATH: &str = "/v1/orders/";

// Credentials come from the proto `ClearsaleBrConfig` oneof entry
// (`CLEARSALE_BR` = 153, field `clearsale_br` = 164): `username`/`password` are
// the ClearSale account login name and password, both sent on
// POST /v1/authenticate.

// ClearSale amounts are JSON numbers in the major currency unit — `99.99` is
// R$99.99, not 9999 centavos (spec §7).
macros::create_amount_converter_wrapper!(connector_name: ClearSaleBr, amount_type: FloatMajorUnit);

macros::create_all_prerequisites!(
    connector_name: ClearSaleBr,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: ClearSaleAuthenticateRequest,
            response_body: ClearSaleAuthenticateResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: PreRiskCheck,
            request_body: ClearSaleOrderCreateRequest,
            response_body: ClearSaleOrderCreateResponse,
            router_data: RouterDataV2<PreRiskCheck, FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse>,
        ),
        (
            flow: PostRiskCheck,
            request_body: ClearSaleOrderReanalysisRequest,
            response_body: ClearSaleOrderReanalysisResponse,
            router_data: RouterDataV2<PostRiskCheck, FrmFlowData, PostRiskCheckRequest, PostRiskCheckResponse>,
        ),
        (
            flow: FrmPaymentOutcome,
            request_body: ClearSaleStatusUpdateRequest,
            response_body: ClearSaleStatusUpdateResponse,
            router_data: RouterDataV2<FrmPaymentOutcome, FrmFlowData, FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        /// Bearer header for the ClearSale v1 API, using the token that the
        /// framework threads onto `FrmFlowData.access_token` from the request's
        /// `state.access_token` (minted by `ServerAuthenticationToken`; spec §3
        /// demands the token be cached and reused until ExpirationDate).
        fn frm_bearer_header(
            &self,
            token: Option<&hyperswitch_masking::Secret<String>>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = token.ok_or(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "missing ClearSale bearer token: FrmFlowData.access_token was not \
                         populated (provide it via the request state.access_token)"
                            .to_owned(),
                    ),
                    suggested_action: Some(
                        "Supply the ClearSale login token in the request state.access_token"
                            .to_owned(),
                    ),
                    doc_url: Some(clearsale::CLEARSALE_DOC_URL.to_owned()),
                },
            })?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/json".to_string().into(),
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
    for ClearSaleBr<T>
{
    fn id(&self) -> &'static str {
        "clearsale_br"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // ClearSale takes `totalValue`/`value` in the major unit.
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.clearsale_br.base_url
    }

    /// Content-negotiation headers common to every ClearSale call. There is no
    /// static credential header: order endpoints authenticate with the Bearer
    /// token from `FrmFlowData.access_token` (see `frm_bearer_header`), and the
    /// login endpoint takes the credentials in the request body.
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Validate the config shape early so a misrouted config fails here
        // rather than deep inside a flow.
        let _auth = clearsale::ClearSaleAuthType::try_from(auth_type)?;
        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (
                headers::ACCEPT.to_string(),
                "application/json".to_string().into(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: clearsale::ClearSaleErrorResponse = res
            .response
            .parse_struct("ClearSaleErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "failed to parse the ClearSale error body as ClearSaleErrorResponse"
                            .to_owned(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);

        let message = response.message();
        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            // ClearSale error bodies carry only a message; the HTTP status is
            // the only machine-readable code, so it doubles as the error code.
            status_code: res.status_code,
            code: res.status_code.to_string(),
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
    for ClearSaleBr<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for ClearSaleBr<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Send + Serialize>
    connector_types::VerifyRedirectResponse for ClearSaleBr<T>
{
}

// =============================================================================
// VALIDATION
// =============================================================================
// ClearSale's login token must be bootstrapped: `should_do_access_token`
// returns true so the composite layer runs ServerAuthenticationToken and
// threads the token onto `state.access_token` before the FRM flows.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for ClearSaleBr<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

// =============================================================================
// WEBHOOKS (spec §5.5)
// =============================================================================
// ClearSale POSTs `{"code","date","type":"status"}` to the integrator URL. The
// payload carries no status and no documented signature; the integrator must
// follow up with `GET /v1/orders/{code}/status`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for ClearSaleBr<T>
{
    /// No webhook signature is documented by ClearSale (spec §11 open question
    /// 2). For now every verified-source check accepts; the payload is logged
    /// by the event pipeline. TODO: add signature verification if the merchant
    /// pack documents one.
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        Ok(true)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // The payload carries no status — only `{"code","date","type":"status"}`
        // — so the event is always "an order status changed; go fetch it".
        let _payload: ClearSaleWebhookPayload = request
            .body
            .parse_struct("ClearSaleWebhookPayload")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        Ok(EventType::PaymentIntentProcessing)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        // `code` is the order reference the merchant created the order with —
        // it is the correlation key stored at order-create time.
        let payload: ClearSaleWebhookPayload = request
            .body
            .parse_struct("ClearSaleWebhookPayload")
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: None,
                merchant_transaction_id: Some(payload.code),
            },
        )))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        // TODO: the webhook carries no decision — the honest handling is a
        // follow-up `GET /v1/orders/{code}/status` and mapping
        // `ClearSaleStatusGetResponse.status` through
        // `ClearSaleStatusEnum::to_frm_decision`. There is no public FRM-sync
        // RPC yet (spec §11 open question 5), so for now the event is surfaced
        // as "processing" with the order code as the resource id.
        let payload: ClearSaleWebhookPayload = request
            .body
            .parse_struct("ClearSaleWebhookPayload")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(payload.code)),
            status: common_enums::AttemptStatus::Pending,
            connector_response_reference_id: None,
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: String::from_utf8(request.body).ok(),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"code":"ORD-1234","date":"2024-01-01T00:00:00Z","type":"status"}"#
    }
}

// =============================================================================
// FRM SERVICE TRAIT
// =============================================================================
// Not generic over `T` for the same reason as nSure/Kount: FrmServiceTrait
// requires `PaymentPreAuthenticateV2<DefaultPCIHolder>` (fixed), which
// `ClearSaleBr<T>` only provides for the matching `T`.
impl connector_types::FrmServiceTrait
    for ClearSaleBr<domain_types::payment_method_data::DefaultPCIHolder>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PreRiskCheckV2 for ClearSaleBr<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PostRiskCheckV2 for ClearSaleBr<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmPaymentOutcomeV2 for ClearSaleBr<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmRefundProcessedV2 for ClearSaleBr<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::FrmChargebackReceivedV2 for ClearSaleBr<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for ClearSaleBr<T>
{
}

// ClearSale has no device-data-collection step — its device signal is the
// `sessionID` collected by the merchant-side Behavior Analytics SDK (spec §10)
// and passed through the request `metadata` (`sessionId`) — so
// PreAuthenticate is stubbed alongside every non-FRM payment flow this
// connector does not implement.
macros::macro_connector_flow_status_impls!(
    connector: ClearSaleBr,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        Authenticate,
        Authorize,
        Capture,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        CreateOrder,
        CreatePaymentMethod,
        DefendDispute,
        GetConnectorCustomer,
        GetPaymentMethod,
        IncrementalAuthorization,
        MandateRevoke,
        PaymentMethodEligibility,
        PaymentMethodToken,
        PostAuthenticate,
        PreAuthenticate,
        PSync,
        Recharge,
        Refund,
        RepeatPayment,
        RSync,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence,
        Void,
        VoidPC,
        VoidPostRefund
    ],
);

// =============================================================================
// REAL FLOW: ServerAuthenticationToken — POST /v1/authenticate (login)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ClearSaleBr,
    curl_request: Json(ClearSaleAuthenticateRequest),
    curl_response: ClearSaleAuthenticateResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                req.resource_common_data.connectors.clearsale_br.base_url,
                CLEARSALE_AUTHENTICATE_PATH
            ))
        }
    }
);

// =============================================================================
// REAL FLOW: PreRiskCheck — POST /v1/orders/ (send order)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ClearSaleBr,
    curl_request: Json(ClearSaleOrderCreateRequest),
    curl_response: ClearSaleOrderCreateResponse,
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
            Ok(format!(
                "{}{}",
                req.resource_common_data.connectors.clearsale_br.base_url,
                CLEARSALE_ORDERS_PATH
            ))
        }
    }
);

// =============================================================================
// REAL FLOW: PostRiskCheck — PUT /v1/orders/ (re-analysis; same body as POST)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ClearSaleBr,
    curl_request: Json(ClearSaleOrderReanalysisRequest),
    curl_response: ClearSaleOrderReanalysisResponse,
    flow_name: PostRiskCheck,
    resource_common_data: FrmFlowData,
    flow_request: PostRiskCheckRequest,
    flow_response: PostRiskCheckResponse,
    http_method: Put,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostRiskCheck, FrmFlowData, PostRiskCheckRequest, PostRiskCheckResponse>,
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
            req: &RouterDataV2<PostRiskCheck, FrmFlowData, PostRiskCheckRequest, PostRiskCheckResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                req.resource_common_data.connectors.clearsale_br.base_url,
                CLEARSALE_ORDERS_PATH
            ))
        }
    }
);

// =============================================================================
// REAL FLOW: FrmPaymentOutcome — PUT /v1/orders/{code}/status (PGA / PGR push)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ClearSaleBr,
    curl_request: Json(ClearSaleStatusUpdateRequest),
    curl_response: ClearSaleStatusUpdateResponse,
    flow_name: FrmPaymentOutcome,
    resource_common_data: FrmFlowData,
    flow_request: FrmPaymentOutcomeRequest,
    flow_response: FrmPaymentOutcomeResponse,
    http_method: Put,
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
            let order_code = req
                .request
                .merchant_transaction_id
                .as_deref()
                .or(req.request.frm_transaction_id.as_deref())
                .or(req.request.connector_transaction_id.as_deref())
                .ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "merchant_transaction_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "ClearSale addresses the status push by the order `code`: \
                                 PUT /v1/orders/{code}/status"
                                    .to_owned(),
                            ),
                            suggested_action: Some(
                                "Send merchant_transaction_id on the FRM notification".to_owned(),
                            ),
                            doc_url: Some(clearsale::CLEARSALE_DOC_URL.to_owned()),
                        },
                    }
                })?;
            Ok(format!(
                "{}{}{}/status",
                req.resource_common_data.connectors.clearsale_br.base_url,
                CLEARSALE_ORDERS_PATH,
                order_code
            ))
        }
    }
);

// =============================================================================
// NOT-IMPLEMENTED FRM FLOWS
// =============================================================================
// FrmRefundProcessed: ClearSale Total Garantido has no merchant refund-notify
// API — the order lifecycle ends at the risk decision plus the PGA/PGR
// payment push.
macros::frm_flow_not_implemented!(
    connector: ClearSaleBr,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: FrmRefundProcessed,
    request: FrmRefundProcessedRequest,
    response: FrmRefundProcessedResponse,
    flow_name: "frm_refund_processed",
);

// FrmChargebackReceived: ClearSale's chargeback API is the separate /v2
// surface (spec §2), deliberately out of scope for this foundation.
macros::frm_flow_not_implemented!(
    connector: ClearSaleBr,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    flow: FrmChargebackReceived,
    request: FrmChargebackReceivedRequest,
    response: FrmChargebackReceivedResponse,
    flow_name: "frm_chargeback_received",
);
