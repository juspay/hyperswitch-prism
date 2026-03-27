mod transformers;
use super::macros;

use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateOrder,
        CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke, PSync,
        PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorWebhookSecrets, DisputeDefendData, DisputeFlowData,
        DisputeResponseData, EventType, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RedirectDetailsResponse, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, SessionTokenRequestData,
        SessionTokenResponseData, SetupMandateRequestData, SubmitEvidenceData,
        WebhookDetailsResponse,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit};
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};

use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorResponseTransformationError;
use domain_types::errors::{IntegrationError, WebhookError};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
};
use serde::Serialize;
use transformers as revolut;
use transformers::{
    RevolutCaptureRequest, RevolutOrderCreateRequest, RevolutOrderCreateResponse,
    RevolutOrderCreateResponse as RevolutPSyncResponse,
    RevolutOrderCreateResponse as RevolutCaptureResponse, RevolutRefundRequest,
    RevolutRefundResponse, RevolutRefundResponse as RevolutRSyncResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Revolut<T>
{
    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, IntegrationError> {
        // Revolut does not support source verification for redirect responses
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        _request: &RequestDetails,
    ) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
        Ok(RedirectDetailsResponse {
            resource_id: None,
            status: None,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            response_amount: None,
            raw_connector_response: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Revolut<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Revolut<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_header = request
            .headers
            .get("revolut-signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Missing incoming webhook signature for Revolut")?;

        // Revolut signature format is "v1=hex_signature".
        // We need to split by '=' and take the second part.
        let signature_parts: Vec<&str> = signature_header.split('=').collect();

        let hex_signature = signature_parts
            .get(1)
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Invalid signature format for Revolut")?;

        hex::decode(hex_signature)
            .attach_printable("Failed to decode hex signature")
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        // 1. Get the Timestamp
        let timestamp = request
            .headers
            .get("revolut-request-timestamp")
            .ok_or_else(|| report!(WebhookError::WebhookBodyDecodingFailed))
            .attach_printable("Missing timestamp header for Revolut")?;

        // 2. Get the Raw Body
        let body = std::str::from_utf8(&request.body)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification message parsing failed for Revolut")?;

        // 3. Construct the signing string: "v1.{timestamp}.{body}"
        let message = format!("v1.{}.{}", timestamp, body);

        Ok(message.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Revolut uses HMAC-SHA256
        let algorithm = crypto::HmacSha256;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => {
                // If webhook secrets are not provided, take them from connector_account_details
                let auth = revolut::RevolutAuthType::try_from(
                    connector_account_details
                        .as_ref()
                        .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))?,
                )
                .map_err(|e| e.change_context(WebhookError::WebhookSourceVerificationFailed))?;

                ConnectorWebhookSecrets {
                    secret: auth
                        .signing_secret
                        .as_ref()
                        .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))?
                        .peek()
                        .as_bytes()
                        .to_vec(),
                    additional_secret: None,
                }
            }
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification failed for Revolut")
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let notif: revolut::RevolutWebhookBody = request
            .body
            .parse_struct("RevolutWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        match notif.event {
            revolut::RevolutWebhookEvent::OrderCompleted => Ok(EventType::PaymentIntentSuccess),
            revolut::RevolutWebhookEvent::OrderAuthorised => {
                Ok(EventType::PaymentIntentAuthorizationSuccess)
            }
            revolut::RevolutWebhookEvent::OrderCancelled => Ok(EventType::PaymentIntentCancelled),
            revolut::RevolutWebhookEvent::OrderFailed => Ok(EventType::PaymentIntentFailure),
            revolut::RevolutWebhookEvent::OrderPaymentAuthenticated => {
                Ok(EventType::PaymentIntentAuthorizationSuccess)
            }
            revolut::RevolutWebhookEvent::OrderPaymentDeclined => {
                Ok(EventType::PaymentIntentAuthorizationFailure)
            }
            revolut::RevolutWebhookEvent::OrderPaymentFailed => Ok(EventType::PaymentIntentFailure),
            revolut::RevolutWebhookEvent::PayoutInitiated => Ok(EventType::PayoutCreated),
            revolut::RevolutWebhookEvent::PayoutCompleted => Ok(EventType::PayoutSuccess),
            revolut::RevolutWebhookEvent::PayoutFailed => Ok(EventType::PayoutFailure),
            revolut::RevolutWebhookEvent::DisputeActionRequired => Ok(EventType::DisputeOpened),
            revolut::RevolutWebhookEvent::DisputeUnderReview => Ok(EventType::DisputeOpened),
            revolut::RevolutWebhookEvent::DisputeWon => Ok(EventType::DisputeWon),
            revolut::RevolutWebhookEvent::DisputeLost => Ok(EventType::DisputeLost),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let notif: revolut::RevolutWebhookBody = request
            .body
            .parse_struct("RevolutWebhookBody")
            .attach_printable("Failed to parse Revolut webhook body")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let response = WebhookDetailsResponse::try_from(notif)
            .change_context(WebhookError::WebhookResponseEncodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Revolut<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Revolut<T>
{
    fn should_do_order_create(&self) -> bool {
        false
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Revolut<T>
{
    fn id(&self) -> &'static str {
        "revolut"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = revolut::RevolutAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.secret_api_key.peek()).into(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.revolut.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
        let response: revolut::RevolutErrorResponse = res
            .response
            .parse_struct("RevolutErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "revolut: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let (code, message, attempt_status) = match response {
            revolut::RevolutErrorResponse::StandardError { code, message, .. } => {
                let attempt_status = match code.as_str() {
                    "unauthenticated" => AttemptStatus::AuthenticationFailed,
                    "unauthorized" => AttemptStatus::AuthorizationFailed,
                    "not_found" => AttemptStatus::Failure,
                    "invalid_request" => AttemptStatus::Failure,
                    "payment_declined" => AttemptStatus::Failure,
                    _ => AttemptStatus::Pending,
                };
                (code, message, attempt_status)
            }
            revolut::RevolutErrorResponse::ErrorIdResponse { error_id, code, .. } => {
                let (error_code, attempt_status) = match code {
                    Some(numeric_code) => {
                        let status = match numeric_code {
                            1024 => AttemptStatus::Failure,
                            _ => AttemptStatus::Pending,
                        };
                        (numeric_code.to_string(), status)
                    }
                    None => ("UNKNOWN_ERROR".to_string(), AttemptStatus::Failure),
                };
                (
                    error_code,
                    format!("Error ID: {}", error_id),
                    attempt_status,
                )
            }
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: Some(attempt_status),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

macros::create_amount_converter_wrapper!(connector_name: Revolut, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Revolut,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: RevolutOrderCreateRequest,
            response_body: RevolutOrderCreateResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: RevolutPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: RevolutCaptureRequest,
            response_body: RevolutCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RevolutRefundRequest,
            response_body: RevolutRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RevolutRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    "Revolut-Api-Version".to_string(),
                    "2024-09-01".to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.revolut.base_url.to_string()
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_request: Json(RevolutOrderCreateRequest),
    curl_response: RevolutOrderCreateResponse,
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
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/orders"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_response: RevolutOrderCreateResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/orders/{order_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_request: Json(RevolutCaptureRequest),
    curl_response: RevolutCaptureResponse,
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
            let mut headers = self.build_headers(req)?;

            headers.push((
                "Revolut-Api-Version".to_string(),
                "2025-10-16".to_string().into(),
            ));
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/orders/{order_id}/capture"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_request: Json(RevolutRefundRequest),
    curl_response: RevolutRefundResponse,
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
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    "Revolut-Api-Version".to_string(),
                    "2024-09-01".to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut api_key);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_transaction_id.clone();
            let base_url = req.resource_common_data.connectors.revolut.base_url.to_string();
            Ok(format!("{base_url}/api/orders/{order_id}/refund"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolut,
    curl_response: RevolutRSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    "Revolut-Api-Version".to_string(),
                    "2024-09-01".to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut api_key);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_refund_id.clone();
            let base_url = req.resource_common_data.connectors.revolut.base_url.to_string();
            Ok(format!("{base_url}/api/orders/{order_id}"))
        }
    }
);
