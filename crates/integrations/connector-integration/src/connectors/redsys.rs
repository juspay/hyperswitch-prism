use std::fmt::Debug;

use serde::Serialize;

use crate::{
    connectors::macros, finalize_connector_response, types::ResponseRouterData,
    with_error_response_body,
};
use common_enums::CurrencyUnit;
use common_utils::consts::NO_ERROR_MESSAGE;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::{ByteSliceExt, XmlExt},
};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        ConnectorWebhookSecrets, DisputeWebhookDetailsResponse, EventContext, EventType,
        PaymentFlowData, PaymentVoidData, PaymentWebhookReference, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};

mod requests;
mod responses;
pub mod transformers;

use requests::{
    RedsysAuthenticateRequest, RedsysAuthorizeRequest, RedsysCaptureRequest,
    RedsysPreAuthenticateRequest, RedsysRefundRequest, RedsysRepeatPaymentRequest,
    RedsysSetupMandateRequest, RedsysVoidRequest,
};
use responses::{
    RedsysAuthenticateResponse, RedsysAuthorizeResponse, RedsysCaptureResponse,
    RedsysPreAuthenticateResponse, RedsysRefundResponse, RedsysRepeatPaymentResponse,
    RedsysSetupMandateResponse, RedsysVoidResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Redsys<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Redsys<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Redsys<T>
{
    fn validate_psync_reference_id(
        &self,
        _data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        // Defer reference validation to PSync request construction and the connector.
        Ok(())
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Redsys<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Redsys<T>
{
    // Redsys EMV 3DS leg map (spec:## API Call Sequences; HS redsys PreAuthenticate/Authenticate/
    // Authorize+CompleteAuthorize):
    // - PreAuthenticate = iniciaPeticionREST `Ds_Merchant_EMV3DS.threeDSInfo = CardData` (no charge).
    // - Authenticate    = trataPeticionREST `AuthenticationData`; frictionless may answer Ds_Response
    //   0000 (charged), so the composite flow stops on Charged/Authorized and Authorize is not re-run.
    // - Authorize       = trataPeticionREST `ChallengeResponse{cres}` — the charging call.
    // The "third leg" (ChallengeResponse) settles money, so it lives in Authorize, never in
    // PostAuthenticate (which must not move money); PostAuthenticate stays not_implemented.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
        _payment_method_data: Option<
            &domain_types::payment_method_data::PaymentMethodData<
                domain_types::payment_method_data::DefaultPCIHolder,
            >,
        >,
        _authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};

        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,

                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authenticate
                }

                (RedirectState::InitialRequest, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::Authorize
                }

                (RedirectState::RedirectWithParams, None) => AuthenticationStep::Authenticate,

                (RedirectState::RedirectWithoutParams, None) => AuthenticationStep::Authorize,

                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}

// Redsys online notification (spec "## Webhook Events"): three POST parameters, signed in the
// body with the terminal key. There is no event-type field and no chargeback notification.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::webhooks::IncomingWebhook for Redsys<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let body = transformers::parse_redsys_webhook_body(&request.body)?;
        let params = transformers::decode_redsys_webhook_parameters(&body)?;
        // plan UD-05: Redsys has no separate webhook secret; notifications are signed with the
        // terminal key. A configured webhook secret wins, else the RedsysConfig sha256_pwd.
        let key = match connector_webhook_secret.filter(|secrets| !secrets.secret.is_empty()) {
            Some(secrets) => String::from_utf8(secrets.secret)
                .change_context(WebhookError::WebhookVerificationSecretInvalid)?,
            None => {
                let config = connector_account_details
                    .ok_or(WebhookError::WebhookVerificationSecretNotFound)?;
                transformers::RedsysAuthType::try_from(&config)
                    .change_context(WebhookError::WebhookVerificationSecretNotFound)?
                    .sha256_pwd
                    .expose()
            }
        };
        transformers::verify_redsys_signature(&body, &params.ds_order, &key)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body = transformers::parse_redsys_webhook_body(&request.body)?;
        let params = transformers::decode_redsys_webhook_parameters(&body)?;
        Ok(match transformers::get_redsys_webhook_outcome(&params)? {
            transformers::RedsysWebhookOutcome::Payment(event, _)
            | transformers::RedsysWebhookOutcome::Refund(event, _) => event,
        })
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body = transformers::parse_redsys_webhook_body(&request.body)?;
        let params = transformers::decode_redsys_webhook_parameters(&body)?;
        // Ds_Order is the resource id of every flow; a refund shares its payment's order (TH-16)
        let reference = match params.ds_transaction_type {
            transformers::RedsysWebhookTransactionType::Refund => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(params.ds_order.clone()),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(params.ds_order),
                    merchant_transaction_id: None,
                })
            }
            transformers::RedsysWebhookTransactionType::Payment
            | transformers::RedsysWebhookTransactionType::Preauthorization
            | transformers::RedsysWebhookTransactionType::Confirmation
            | transformers::RedsysWebhookTransactionType::Validation
            | transformers::RedsysWebhookTransactionType::Cancellation
            | transformers::RedsysWebhookTransactionType::SeparateCancellation
            | transformers::RedsysWebhookTransactionType::SeparateConfirmation
            | transformers::RedsysWebhookTransactionType::Unknown => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(params.ds_order),
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                })
            }
        };
        Ok(Some(reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = transformers::parse_redsys_webhook_body(&request.body)?;
        let params = transformers::decode_redsys_webhook_parameters(&body)?;
        let status = match transformers::get_redsys_webhook_outcome(&params)? {
            transformers::RedsysWebhookOutcome::Payment(_, status) => status,
            transformers::RedsysWebhookOutcome::Refund(..) => {
                return Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                    .attach_printable("redsys: refund notification routed to payment processing")
            }
        };
        let is_failure = domain_types::utils::is_payment_failure(status);
        Ok(WebhookDetailsResponse {
            raw_connector_status: None,
            amount: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(params.ds_order.clone())),
            status,
            connector_response_reference_id: Some(params.ds_order),
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code: is_failure.then(|| params.ds_response.0.clone()),
            error_message: is_failure.then(|| NO_ERROR_MESSAGE.to_string()),
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            captured_money: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = transformers::parse_redsys_webhook_body(&request.body)?;
        let params = transformers::decode_redsys_webhook_parameters(&body)?;
        let status = match transformers::get_redsys_webhook_outcome(&params)? {
            transformers::RedsysWebhookOutcome::Refund(_, status) => status,
            transformers::RedsysWebhookOutcome::Payment(..) => {
                return Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                    .attach_printable("redsys: payment notification routed to refund processing")
            }
        };
        let is_failure = status == common_enums::RefundStatus::Failure;
        Ok(RefundWebhookDetailsResponse {
            raw_connector_status: None,
            connector_refund_id: Some(params.ds_order.clone()),
            merchant_transaction_id: None,
            status,
            connector_response_reference_id: Some(params.ds_order),
            error_code: is_failure.then(|| params.ds_response.0.clone()),
            error_message: is_failure.then(|| NO_ERROR_MESSAGE.to_string()),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        // spec "### Event Types": Redsys sends no chargeback/dispute notification (handled
        // bank-side, out of band), so there is nothing to map.
        Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
            operation: "process_dispute_webhook",
        }))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let body = transformers::parse_redsys_webhook_body(&request.body)?;
        Ok(Box::new(transformers::decode_redsys_webhook_parameters(
            &body,
        )?))
    }

    /// The payment_succeeded fixture of connector_specs/redsys/webhook_payload.json (field-probe input)
    fn sample_webhook_body(&self) -> &'static [u8] {
        b"Ds_SignatureVersion=HMAC_SHA256_V1&Ds_MerchantParameters=eyJEc19EYXRlIjoiMTIlMkYxMSUyRjIwMjYiLCJEc19Ib3VyIjoiMTAlM0EwMCIsIkRzX1NlY3VyZVBheW1lbnQiOiIxIiwiRHNfQW1vdW50IjoiMTAwMCIsIkRzX0N1cnJlbmN5IjoiOTc4IiwiRHNfT3JkZXIiOiJ3aDAwMDAwMDAwMDEiLCJEc19NZXJjaGFudENvZGUiOiI5OTkwMDg4ODEiLCJEc19UZXJtaW5hbCI6IjEiLCJEc19SZXNwb25zZSI6IjAwMDAiLCJEc19NZXJjaGFudERhdGEiOiIiLCJEc19UcmFuc2FjdGlvblR5cGUiOiIwIiwiRHNfQ29uc3VtZXJMYW5ndWFnZSI6IjEiLCJEc19BdXRob3Jpc2F0aW9uQ29kZSI6IjM3MjY2MyJ9&Ds_Signature=JKyDQPLGG6nGQNqLVyU7Jz1vRDPfiAZMVlq2B9cWImg%3D"
    }
}

macros::macro_connector_payout_implementation!(
    connector: Redsys,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

macros::create_amount_converter_wrapper!(connector_name: Redsys, amount_type: StringMinorUnit);
macros::create_all_prerequisites!(
    connector_name: Redsys,
    generic_type: T,
    api: [
        (
            flow: PreAuthenticate,
            request_body: RedsysPreAuthenticateRequest,
            response_body: RedsysPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: RedsysAuthenticateRequest,
            response_body: RedsysAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authorize,
            request_body: RedsysAuthorizeRequest,
            response_body: RedsysAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: RedsysCaptureRequest,
            response_body: RedsysCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: RedsysVoidRequest,
            response_body: RedsysVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RedsysRefundRequest,
            response_body: RedsysRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: RedsysSetupMandateRequest,
            response_body: RedsysSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: RedsysRepeatPaymentRequest,
            response_body: RedsysRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.redsys.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.redsys.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Redsys<T>
{
    fn id(&self) -> &'static str {
        "redsys"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.redsys.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: responses::RedsysErrorResponse = res
            .response
            .parse_struct("RedsysErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "redsys: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.clone(),
            message: response.error_code.clone(),
            reason: Some(response.error_code),
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

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysPreAuthenticateRequest),
    curl_response: RedsysPreAuthenticateResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/sis/rest/iniciaPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysAuthenticateRequest),
    curl_response: RedsysAuthenticateResponse,
    flow_name: Authenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
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
            Ok(format!("{}/sis/rest/trataPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysAuthorizeRequest),
    curl_response: RedsysAuthorizeResponse,
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
            Ok(format!("{}/sis/rest/trataPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for Redsys<T>
{
    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}/apl02/services/SerClsWSConsulta",
            self.connector_base_url_payments(req)
        ))
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        let transaction_type = None;
        let auth = transformers::RedsysAuthType::try_from(&req.connector_config)?;
        // G-PSync-01: Consulta is keyed by the Ds_Order Authorize returned (connector_transaction_id,
        // HS redsys/transformers.rs:2119-2124); an empty Ds_Order is rejected by Redsys with XML0013.
        let connector_transaction_id = req
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .ok()
            .filter(|order_id| !order_id.is_empty())
            .ok_or(IntegrationError::MissingConnectorTransactionID {
                context: transformers::redsys_error_context(
                    "Send the connector_transaction_id (Ds_Order) returned by Authorize",
                    "Redsys consultaOperaciones requires a non-empty Ds_Order (XML0013)",
                ),
            })?;
        let body =
            transformers::construct_sync_request(connector_transaction_id, transaction_type, auth)?;

        let headers = vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "text/xml; charset=utf-8".to_string().into(),
            ),
            (
                "SOAPAction".to_string(),
                transformers::REDSYS_SOAP_ACTION.to_string().into(),
            ),
        ];

        let url = format!(
            "{}/apl02/services/SerClsWSConsulta",
            self.connector_base_url_payments(req)
        );

        Ok(Some(
            common_utils::request::RequestBuilder::new()
                .method(common_utils::request::Method::Post)
                .url(&url)
                .attach_default_headers()
                .headers(headers)
                .set_body(common_utils::request::RequestContent::RawBytes(body))
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ConnectorError,
    > {
        let response = String::from_utf8(res.response.to_vec()).change_context(
            crate::utils::response_deserialization_fail(res.status_code, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
        )?;
        let response_data = lowercase_xml_markup(&html_escape::decode_html_entities(&response));
        let response = response_data
            .parse_xml::<responses::RedsysSyncResponse>()
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "redsys: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysCaptureRequest),
    curl_response: RedsysCaptureResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/sis/rest/trataPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysVoidRequest),
    curl_response: RedsysVoidResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/sis/rest/trataPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysRefundRequest),
    curl_response: RedsysRefundResponse,
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
            Ok(format!("{}/sis/rest/trataPeticionREST", self.connector_base_url_refunds(req)))
        }
    }
);

// SetupMandate: TRANSACTIONTYPE 7 card validation storing the card (DS_MERCHANT_IDENTIFIER=REQUIRED)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysSetupMandateRequest),
    curl_response: RedsysSetupMandateResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/sis/rest/trataPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);

// RepeatPayment: MIT TRANSACTIONTYPE 0/1 with EXCEP_SCA=MIT + DIRECTPAYMENT=true, by stored
// DS_MERCHANT_IDENTIFIER or PAN + DS_MERCHANT_COF_TXNID (techspec "### RepeatPayment")
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Redsys,
    curl_request: Json(RedsysRepeatPaymentRequest),
    curl_response: RedsysRepeatPaymentResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/sis/rest/trataPeticionREST", self.connector_base_url_payments(req)))
        }
    }
);

/// Redsys SOAP tags are mixed-case (`Ds_ErrorCode`); the sync structs use lowercase serde
/// names. Fold only the markup between `<` and `>` so text values (error codes, Ds_Order,
/// Ds_AuthorisationCode) keep the connector's exact case.
fn lowercase_xml_markup(xml: &str) -> String {
    let mut in_tag = false;
    xml.chars()
        .map(|c| {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ => {}
            }
            if in_tag {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

// Manual implementation for RSync since it uses SOAP XML
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for Redsys<T>
{
    fn get_url(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}/apl02/services/SerClsWSConsulta",
            self.connector_base_url_refunds(req)
        ))
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        let transaction_type = Some("3".to_owned());
        let auth = transformers::RedsysAuthType::try_from(&req.connector_config)?;
        // G-RSync-01: refunds are queried by the payment's Ds_Order (HS redsys/transformers.rs:2367-2368);
        // an empty Ds_Order is rejected by Redsys with XML0013.
        let connector_transaction_id = Some(req.request.connector_transaction_id.clone())
            .filter(|order_id| !order_id.is_empty())
            .ok_or(IntegrationError::MissingConnectorTransactionID {
                context: transformers::redsys_error_context(
                    "Send the connector_transaction_id (Ds_Order) of the refunded payment",
                    "Redsys consultaOperaciones requires a non-empty Ds_Order (XML0013)",
                ),
            })?;
        let body =
            transformers::construct_sync_request(connector_transaction_id, transaction_type, auth)?;

        let headers = vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "text/xml; charset=utf-8".to_string().into(),
            ),
            (
                "SOAPAction".to_string(),
                transformers::REDSYS_SOAP_ACTION.to_string().into(),
            ),
        ];

        let url = format!(
            "{}/apl02/services/SerClsWSConsulta",
            self.connector_base_url_refunds(req)
        );

        Ok(Some(
            common_utils::request::RequestBuilder::new()
                .method(common_utils::request::Method::Post)
                .url(&url)
                .attach_default_headers()
                .headers(headers)
                .set_body(common_utils::request::RequestContent::RawBytes(body))
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ConnectorError,
    > {
        let response = String::from_utf8(res.response.to_vec()).change_context(
            crate::utils::response_deserialization_fail(res.status_code, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
        )?;
        let response_data = lowercase_xml_markup(&html_escape::decode_html_entities(&response));
        let response = response_data
            .parse_xml::<responses::RedsysSyncResponse>()
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "redsys: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }
}

// ============================================================================
// Stub Implementations for Unsupported Flows
// ============================================================================

macros::macro_connector_flow_status_impls!(
    connector: Redsys,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        VoidPC,
        PostAuthenticate,
        PaymentMethodToken,
        IncrementalAuthorization,
    ],
    not_supported: [
        VoidPostRefund,
        ClientAuthenticationToken,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        CreateOrder,
        Accept,
        DefendDispute,
        SubmitEvidence,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        MandateRevoke,
    ],
);
