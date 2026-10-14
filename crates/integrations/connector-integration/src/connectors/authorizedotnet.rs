pub mod transformers;

use common_enums;
use common_utils::{
    consts,
    crypto::{HmacSha512, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, RSync, Refund, RepeatPayment,
        ServerSessionAuthenticationToken, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, ConnectorSpecifications,
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        self, ConnectorServiceTrait, IncomingWebhook, PaymentAuthorizeV2, PaymentCapture,
        PaymentSyncV2, PaymentVoidPostCaptureV2, PaymentVoidV2, RefundSyncV2, RefundV2,
        RepeatPaymentV2, ServerSessionAuthentication, SetupMandateV2, ValidationTrait,
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
    AuthorizedotnetSdkSessionTokenRequest, AuthorizedotnetSdkSessionTokenResponse,
    AuthorizedotnetSetupMandateRequest, AuthorizedotnetSetupMandateResponse,
    AuthorizedotnetVoidPCRequest, AuthorizedotnetVoidPCResponse, AuthorizedotnetVoidRequest,
    AuthorizedotnetVoidResponse, AuthorizedotnetWebhookEventType, AuthorizedotnetWebhookObjectId,
};
use super::macros;
use crate::{types::ResponseRouterData, utils, with_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
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

    fn should_do_session_token(
        &self,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ServerSessionAuthentication for Authorizedotnet<T>
{
    // Default trait methods are sufficient for the SDKSessionToken
    // (ServerSessionAuthenticationToken) flow; no custom logic required.
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

        // Parse "sha512=<hex>". The algorithm prefix is matched case-insensitively
        // (techspec "Webhook Authentication & Signature Verification" step 1; the
        // Hyperswitch connector lower-cases the header and splits at the first '=').
        let signature_hex = match signature_header.split_once('=') {
            Some((algorithm, hex)) if algorithm.eq_ignore_ascii_case("sha512") => hex,
            Some(_) | None => {
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

        // HMAC-SHA512 over the raw request body bytes, keyed with the Signature Key as
        // stored. `verify_signature` does the comparison in constant time.
        match HmacSha512.verify_signature(&webhook_secret, &expected_signature, &request.body) {
            Ok(verified) => Ok(verified),
            Err(crypto_error) => {
                tracing::error!(
                    target: "authorizedotnet_webhook",
                    "Failed to verify HMAC-SHA512 signature of the webhook, error: {:?} - verification failed but continuing processing",
                    crypto_error
                );
                Ok(false) // Crypto error -> verification fails but continue processing
            }
        }
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"webhookId":"probe_webhook_001","eventType":"net.authorize.payment.authcapture.created","payload":{"id":"probe_txn_001","responseCode":1,"authCode":"probe_auth"}}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // An empty body is the endpoint check, not an event (techspec "IncomingWebhook",
        // "Expected result (empty body)").
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }

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
            // Authorize.Net delivers many event types this connector does not act on
            // (fraud, subscription, settlement, ...). They are acknowledged as
            // not-supported, as hyperswitch does, instead of failing the delivery.
            transformers::AuthorizedotnetIncomingWebhookEventType::Unknown => {
                EventType::IncomingWebhookEventUnspecified
            }
        };
        Ok(event_type)
    }

    /// Reference for `EventService.ParseEvent` (stateless: no secret, no verification).
    /// `payload.id` is the transId of the transaction the event is about - for
    /// `refund.created` that is the refund transaction itself - and
    /// `payload.merchantReferenceId` echoes the `refId` of the request
    /// (techspec "IncomingWebhook").
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        // Endpoint verification carries no resource.
        if request.body.is_empty() {
            return Ok(None);
        }

        let webhook_body: AuthorizedotnetWebhookObjectId = request
            .body
            .parse_struct("AuthorizedotnetWebhookObjectId")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable_lazy(|| {
                "Failed to parse Authorize.Net webhook body for the event reference"
            })?;

        match webhook_body.event_type {
            transformers::AuthorizedotnetWebhookEvent::RefundCreated => {
                let connector_refund_id = webhook_body
                    .payload
                    .id
                    .ok_or(WebhookError::WebhookReferenceIdNotFound)
                    .attach_printable("payload.id missing in refund.created webhook")?;
                // The payload has no link from the refund to its original payment.
                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(connector_refund_id),
                        merchant_refund_id: None,
                        connector_transaction_id: None,
                        merchant_transaction_id: None,
                    },
                )))
            }
            transformers::AuthorizedotnetWebhookEvent::AuthorizationCreated
            | transformers::AuthorizedotnetWebhookEvent::PriorAuthCapture
            | transformers::AuthorizedotnetWebhookEvent::AuthCapCreated
            | transformers::AuthorizedotnetWebhookEvent::CaptureCreated
            | transformers::AuthorizedotnetWebhookEvent::VoidCreated => {
                // `priorAuthCapture` and `voidTransaction` act on the existing
                // transaction rather than minting a new one: a single transId carries
                // the whole lifecycle (`authorizedPendingCapture` ->
                // `capturedPendingSettlement` -> `settledSuccessfully`, or `voided` -
                // which is what `transformers::SyncStatus` models). So `payload.id` here
                // is the same transId the Authorize response stored, and a lookup by
                // `connector_transaction_id` resolves. `refund.created`, handled above,
                // is the one event that mints a new transId, and the payload carries no
                // link from it back to the payment.
                let connector_transaction_id = webhook_body
                    .payload
                    .id
                    .ok_or(WebhookError::WebhookReferenceIdNotFound)
                    .attach_printable("payload.id missing in payment webhook")?;
                Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(connector_transaction_id),
                        merchant_transaction_id: webhook_body.payload.merchant_reference_id,
                    },
                )))
            }
            // Customer profile events identify a profile, not a payment or a refund.
            transformers::AuthorizedotnetWebhookEvent::CustomerCreated
            | transformers::AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => Ok(None),
            // An event type this connector does not act on has no resource to resolve.
            transformers::AuthorizedotnetWebhookEvent::Unknown => Ok(None),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let webhook_body: AuthorizedotnetWebhookObjectId = request
            .body
            .parse_struct("AuthorizedotnetWebhookObjectId")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable_lazy(|| {
                "Failed to parse Authorize.Net payment webhook body structure"
            })?;

        let transaction_id = match webhook_body.event_type {
            // An event type this connector does not act on is acknowledged with
            // whatever id it carries; a missing id must not fail the delivery.
            transformers::AuthorizedotnetWebhookEvent::Unknown => webhook_body.payload.id.clone(),
            _ => Some(get_trans_id(&webhook_body).attach_printable_lazy(|| {
                format!(
                    "Failed to extract transaction ID from payment webhook for event: {:?}",
                    webhook_body.event_type
                )
            })?),
        };
        let status = transformers::SyncStatus::from(webhook_body.event_type.clone());
        // The merchant's own reference is payload.merchantReferenceId (the echoed refId);
        // it is absent when the request carried no refId, and is never the transId.
        let merchant_reference_id = webhook_body.payload.merchant_reference_id.clone();

        Ok(WebhookDetailsResponse {
            connector_returned_payment_method_details: None,
            resource_id: transaction_id
                .clone()
                .map(ResponseId::ConnectorTransactionId),
            status: common_enums::AttemptStatus::from(status),
            status_code: 200,
            mandate_reference: None,
            connector_response_reference_id: transaction_id,
            connector_request_reference_id: merchant_reference_id,
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            response_headers: None,
            amount_captured: None,
            error_reason: None,
            network_txn_id: None,
            payment_method_update: None,
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
            merchant_transaction_id: None,
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
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> RefundSyncV2
    for Authorizedotnet<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    RepeatPaymentV2<T> for Authorizedotnet<T>
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
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentVoidPostCaptureV2 for Authorizedotnet<T>
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
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Authorize.Net prefixes its JSON with a UTF-8 BOM; this path does not go
        // through `preprocess_response_bytes`, so strip it here.
        let (body, _) = encoding_rs::UTF_8.decode_with_bom_removal(&res.response);

        if body.trim().is_empty() {
            return Ok(ErrorResponse {
                status_code: res.status_code,
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                attempt_status: None,
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

        // The error body is the regular envelope: `messages` nested at the root, with
        // an optional `transactionResponse` whose `errors[0]` takes precedence.
        let response: transformers::AuthorizedotnetPaymentsResponse = body
            .as_bytes()
            .parse_struct("AuthorizedotnetPaymentsResponse")
            .change_context(utils::response_deserialization_fail(
                res.status_code,
                "authorizedotnet: response body did not match the expected format; confirm API version and connector documentation.",
            ))?;

        with_response_body!(event_builder, response);
        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        let transaction_error = match response.transaction_response.as_ref() {
            Some(transformers::TransactionResponse::AuthorizedotnetTransactionResponse(
                transaction_response,
            )) => transaction_response
                .errors
                .as_ref()
                .and_then(|errors| errors.first())
                .map(|error| (error.error_code.clone(), error.error_text.clone())),
            Some(transformers::TransactionResponse::AuthorizedotnetTransactionResponseError(_))
            | None => None,
        };
        let (code, message) = transaction_error
            .or_else(|| {
                response
                    .messages
                    .message
                    .first()
                    .map(|message| (message.code.clone(), message.text.clone()))
            })
            .unwrap_or_else(|| {
                (
                    consts::NO_ERROR_CODE.to_string(),
                    consts::NO_ERROR_MESSAGE.to_string(),
                )
            });

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
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
            request_body: AuthorizedotnetRepeatPaymentRequest<T>,
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
        ),
        (
            flow: VoidPC,
            request_body: AuthorizedotnetVoidPCRequest,
            response_body: AuthorizedotnetVoidPCResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        (
            flow: ServerSessionAuthenticationToken,
            request_body: AuthorizedotnetSdkSessionTokenRequest,
            response_body: AuthorizedotnetSdkSessionTokenResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
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

        pub fn connector_base_url_merchant_auth<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> String {
            let base_url = &req.resource_common_data.connectors.authorizedotnet.base_url;
            base_url.to_string()
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

// Implement VoidPostCapture (Reverse) flow
// Authorize.net's voidTransaction works on captured-but-not-yet-settled transactions
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetVoidPCRequest),
    curl_response: AuthorizedotnetVoidPCResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Implement SDKSessionToken (ServerSessionAuthenticationToken) flow.
// Backed by Authorize.Net's `getMerchantDetailsRequest`, returning the merchant's
// `publicClientKey` which the front-end Accept.js / AcceptUI SDK consumes.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authorizedotnet,
    curl_request: Json(AuthorizedotnetSdkSessionTokenRequest),
    curl_response: AuthorizedotnetSdkSessionTokenResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_merchant_auth(req).to_string())
        }
    }
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Authorizedotnet<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Authorizedotnet,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        GetConnectorCustomer,
        IncrementalAuthorization,
        CreateOrder,
        ServerAuthenticationToken,
        PaymentMethodToken,
        ClientAuthenticationToken,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        Accept,
        SubmitEvidence,
        DefendDispute,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
    ],
);
