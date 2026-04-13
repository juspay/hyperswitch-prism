pub mod transformers;

use common_enums;
use common_utils::{
    consts, errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, EventType,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData, SetupMandateRequestData, SubmitEvidenceData,
        WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        self, AcceptDispute, ConnectorServiceTrait, DisputeDefend, IncomingWebhook,
        PaymentAuthorizeV2, PaymentCapture, PaymentOrderCreate, PaymentSyncV2, PaymentTokenV2,
        PaymentVoidPostCaptureV2, PaymentVoidV2, RefundSyncV2, RefundV2, RepeatPaymentV2,
        ServerAuthentication, ServerSessionAuthentication, SetupMandateV2, SubmitEvidenceV2,
        ValidationTrait,
    },
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;

use self::transformers::{
    get_trans_id, AuthorizedotnetAuthorizeResponse, AuthorizedotnetCaptureRequest,
    AuthorizedotnetCaptureResponse, AuthorizedotnetCreateConnectorCustomerRequest,
    AuthorizedotnetCreateConnectorCustomerResponse, AuthorizedotnetCreateSyncRequest,
    AuthorizedotnetPSyncResponse, AuthorizedotnetPaymentsRequest, AuthorizedotnetRSyncRequest,
    AuthorizedotnetRSyncResponse, AuthorizedotnetRefundRequest, AuthorizedotnetRefundResponse,
    AuthorizedotnetRepeatPaymentRequest, AuthorizedotnetRepeatPaymentResponse,
    AuthorizedotnetSetupMandateRequest, AuthorizedotnetSetupMandateResponse,
    AuthorizedotnetVoidRequest, AuthorizedotnetVoidResponse, AuthorizedotnetWebhookEventType,
    AuthorizedotnetWebhookObjectId,
};
use super::macros;
use crate::{types::ResponseRouterData, with_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Authorizedotnet<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Authorizedotnet,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorServiceTrait<T> for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ServerSessionAuthentication for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ServerAuthentication for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SetupMandateV2<T> for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ValidationTrait for Authorizedotnet<T>
{
    fn should_create_connector_customer(&self) -> bool {
        true
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    IncomingWebhook for Authorizedotnet<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // If no webhook secret is provided, cannot verify
        let webhook_secret = match connector_webhook_secret {
            Some(secrets) => secrets.secret,
            None => return Ok(false),
        };

        // Extract X-ANET-Signature header (case-insensitive)
        let signature_header = match request
            .headers
            .get("X-ANET-Signature")
            .or_else(|| request.headers.get("x-anet-signature"))
        {
            Some(header) => header,
            None => {
                tracing::warn!(
                    target: "authorizedotnet_webhook",
                    "Missing X-ANET-Signature header in webhook request from Authorize.Net - verification failed but continuing processing"
                );
                return Ok(false); // Missing signature -> verification fails but continue processing
            }
        };

        // Parse "sha512=<hex>" format
        let signature_hex = match signature_header.strip_prefix("sha512=") {
            Some(hex) => hex,
            None => {
                tracing::warn!(
                    target: "authorizedotnet_webhook",
                    "Invalid signature format in X-ANET-Signature header, expected 'sha512=<hex>' but got: '{}' - verification failed but continuing processing",
                    signature_header
                );
                return Ok(false); // Invalid format -> verification fails but continue processing
            }
        };

        // Decode hex signature
        let expected_signature = match hex::decode(signature_hex) {
            Ok(sig) => sig,
            Err(hex_error) => {
                tracing::warn!(
                    target: "authorizedotnet_webhook",
                    "Failed to decode hex signature from X-ANET-Signature header: '{}', error: {} - verification failed but continuing processing",
                    signature_hex,
                    hex_error
                );
                return Ok(false); // Invalid hex -> verification fails but continue processing
            }
        };

        // Compute HMAC-SHA512 of request body
        use common_utils::crypto::{HmacSha512, SignMessage};
        let crypto_algorithm = HmacSha512;
        let computed_signature = match crypto_algorithm.sign_message(&webhook_secret, &request.body)
        {
            Ok(sig) => sig,
            Err(crypto_error) => {
                tracing::error!(
                    target: "authorizedotnet_webhook",
                    "Failed to compute HMAC-SHA512 signature for webhook verification, error: {:?} - verification failed but continuing processing",
                    crypto_error
                );
                return Ok(false); // Crypto error -> verification fails but continue processing
            }
        };

        // Constant-time comparison to prevent timing attacks
        Ok(computed_signature == expected_signature)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let webhook_body: AuthorizedotnetWebhookEventType = request
            .body
            .parse_struct("AuthorizedotnetWebhookEventType")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable_lazy(|| {
                "Failed to parse webhook event type from Authorize.Net webhook body"
            })?;

        let event_type = match webhook_body.event_type {
            transformers::AuthorizedotnetIncomingWebhookEventType::AuthorizationCreated => {
                EventType::PaymentIntentAuthorizationSuccess
            }
            transformers::AuthorizedotnetIncomingWebhookEventType::PriorAuthCapture
            | transformers::AuthorizedotnetIncomingWebhookEventType::CaptureCreated => {
                EventType::PaymentIntentCaptureSuccess
            }
            transformers::AuthorizedotnetIncomingWebhookEventType::AuthCapCreated => {
                EventType::PaymentIntentSuccess // Combined auth+capture
            }
            transformers::AuthorizedotnetIncomingWebhookEventType::VoidCreated => {
                EventType::PaymentIntentCancelled
            }
            transformers::AuthorizedotnetIncomingWebhookEventType::RefundCreated => {
                EventType::RefundSuccess
            }
            transformers::AuthorizedotnetIncomingWebhookEventType::CustomerCreated
            | transformers::AuthorizedotnetIncomingWebhookEventType::CustomerPaymentProfileCreated => {
                EventType::MandateActive
            }
            transformers::AuthorizedotnetIncomingWebhookEventType::Unknown => {
                tracing::warn!(
                    target: "authorizedotnet_webhook",
                    "Received unknown webhook event type from Authorize.Net - rejecting webhook"
                );
                return Err(
                    report!(WebhookError::WebhookEventTypeNotFound)
                        .attach_printable("Unknown webhook event type"),
                );
            }
        };
        Ok(event_type)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let webhook_body: AuthorizedotnetWebhookObjectId = request
            .body
            .parse_struct("AuthorizedotnetWebhookObjectId")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable_lazy(|| {
                "Failed to parse Authorize.Net payment webhook body structure"
            })?;

        let transaction_id = get_trans_id(&webhook_body).attach_printable_lazy(|| {
            format!(
                "Failed to extract transaction ID from payment webhook for event: {:?}",
                webhook_body.event_type
            )
        })?;
        let status = transformers::SyncStatus::from(webhook_body.event_type.clone());

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(transaction_id.clone())),
            status: common_enums::AttemptStatus::from(status),
            status_code: 200,
            mandate_reference: None,
            connector_response_reference_id: Some(transaction_id),
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            response_headers: None,
            minor_amount_captured: None,
            amount_captured: None,
            error_reason: None,
            network_txn_id: None,
            payment_method_update: None,
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let webhook_body: AuthorizedotnetWebhookObjectId = request
            .body
            .parse_struct("AuthorizedotnetWebhookObjectId")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable_lazy(|| {
                "Failed to parse Authorize.Net refund webhook body structure"
            })?;

        let transaction_id = get_trans_id(&webhook_body).attach_printable_lazy(|| {
            format!(
                "Failed to extract transaction ID from refund webhook for event: {:?}",
                webhook_body.event_type
            )
        })?;

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(transaction_id.clone()),
            status: common_enums::RefundStatus::Success, // Authorize.Net only sends successful refund webhooks
            status_code: 200,
            connector_response_reference_id: Some(transaction_id),
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            response_headers: None,
        })
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SubmitEvidenceV2 for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> DisputeDefend
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> RefundSyncV2
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> AcceptDispute
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    RepeatPaymentV2<T> for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentVoidPostCaptureV2 for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentOrderCreate for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentAuthorizeV2<T> for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentSyncV2
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentVoidV2
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> RefundV2
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentCapture
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentTokenV2<T> for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SourceVerification for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Authorizedotnet<T>
{
}

// Basic connector implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for Authorizedotnet<T>
{
    fn id(&self) -> &'static str {
        "authorizedotnet"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.authorizedotnet.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: transformers::ResponseMessages =
            res.response.parse_struct("ResponseMessages").map_err(|_| {
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "authorizedotnet: response body did not match the expected format; confirm API version and connector documentation.")
            })?;

        with_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .message
                .first()
                .map(|m| m.code.clone())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            message: response
                .message
                .first()
                .map(|m| m.text.clone())
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }
}

// Define connector prerequisites
macros::create_all_prerequisites!(
    connector_name: Authorizedotnet,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AuthorizedotnetPaymentsRequest<T>,
            response_body: AuthorizedotnetAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AuthorizedotnetCreateSyncRequest,
            response_body: AuthorizedotnetPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: AuthorizedotnetCaptureRequest,
            response_body: AuthorizedotnetCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AuthorizedotnetVoidRequest,
            response_body: AuthorizedotnetVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AuthorizedotnetRefundRequest,
            response_body: AuthorizedotnetRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: AuthorizedotnetRSyncRequest,
            response_body: AuthorizedotnetRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: AuthorizedotnetRepeatPaymentRequest,
            response_body: AuthorizedotnetRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: CreateConnectorCustomer,
            request_body: AuthorizedotnetCreateConnectorCustomerRequest<T>,
            response_body: AuthorizedotnetCreateConnectorCustomerResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: SetupMandate,
            request_body: AuthorizedotnetSetupMandateRequest<T>,
            response_body: AuthorizedotnetSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            _status_code: u16,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            // Check if the bytes begin with UTF-8 BOM (EF BB BF)
            let encoding = encoding_rs::UTF_8;
            let intermediate_response_bytes = encoding.decode_with_bom_removal(&bytes);
            let processed_bytes = bytes::Bytes::copy_from_slice(intermediate_response_bytes.0.as_bytes());

            Ok(processed_bytes)
        }
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

        pub fn connector_base_url_payments<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            let base_url = &req.resource_common_data.connectors.authorizedotnet.base_url;
            base_url.to_string()
        }

        pub fn connector_base_url_refunds<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.authorizedotnet.base_url.to_string()
        }
    }
);

// Implement the specific flows
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetPaymentsRequest),
    curl_response: AuthorizedotnetAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetCreateSyncRequest),
    curl_response: AuthorizedotnetPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetCaptureRequest),
    curl_response: AuthorizedotnetCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetVoidRequest),
    curl_response: AuthorizedotnetVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetRefundRequest),
    curl_response: AuthorizedotnetRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// Implement RSync flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetRSyncRequest),
    curl_response: AuthorizedotnetRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_refunds(req).to_string())
        }

    }
);

// Implement SetupMandate flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetSetupMandateRequest),
    curl_response: AuthorizedotnetSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetRepeatPaymentRequest),
    curl_response: AuthorizedotnetRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true, // Keeping true for Authorize.net which needs BOM handling
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Implement CreateConnectorCustomer flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetCreateConnectorCustomerRequest),
    curl_response: AuthorizedotnetCreateConnectorCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Authorizedotnet<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Authorizedotnet<T>
{
}
