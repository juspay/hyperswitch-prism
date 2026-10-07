pub mod transformers;

use base64::Engine;
use common_utils::{
    crypto::VerifySignature, errors::CustomResult, events, ext_traits::ByteSliceExt,
    StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, EventContext, EventType, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, SetupMandateRequestData, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use ring::hmac;
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    CaptureRequest, RapydAuthType, RapydClientAuthRequest, RapydClientAuthResponse,
    RapydCreateOrderRequest, RapydCreateOrderResponse, RapydErrorFlow, RapydIncomingWebhook,
    RapydPaymentsRequest, RapydPaymentsResponse as RapydCaptureResponse,
    RapydPaymentsResponse as RapydPSyncResponse, RapydPaymentsResponse as RapydVoidResponse,
    RapydPaymentsResponse as RapydAuthorizeResponse, RapydRefundRequest, RapydRepeatPaymentRequest,
    RapydRepeatPaymentResponse, RapydSetupMandateRequest, RapydSetupMandateResponse,
    RapydWebhookSecret, RefundResponse, RefundResponse as RapydRSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    // Headers of a webhook Rapyd delivers.
    pub(crate) const WEBHOOK_SIGNATURE: &str = "signature";
    pub(crate) const WEBHOOK_SALT: &str = "salt";
    pub(crate) const WEBHOOK_TIMESTAMP: &str = "timestamp";
    pub(crate) const HOST: &str = "host";
}

pub const BASE64_ENGINE_URL_SAFE: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Rapyd<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Rapyd,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Rapyd<T>
{
}
/// Value of a webhook request header, matched case-insensitively; an empty
/// value is an absent one.
fn webhook_header<'a>(request: &'a RequestDetails, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
}

fn required_webhook_header<'a>(
    request: &'a RequestDetails,
    name: &'static str,
) -> Result<&'a str, Report<WebhookError>> {
    webhook_header(request, name).ok_or_else(|| {
        error_stack::report!(WebhookError::WebhookMissingRequiredField { field: name })
    })
}

/// The URL the webhook was delivered to, which Rapyd signs as `url_path`:
/// "the entire URL that was configured for your company to receive webhooks".
/// The request URI is used as is when it is absolute; a path is completed with
/// the `host` header.
fn webhook_url(request: &RequestDetails) -> Result<String, Report<WebhookError>> {
    let uri = request
        .uri
        .as_deref()
        .map(str::trim)
        .filter(|uri| !uri.is_empty())
        .ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "uri" })
        })?;
    if uri.starts_with("https://") || uri.starts_with("http://") {
        return Ok(uri.to_string());
    }
    let host = required_webhook_header(request, headers::HOST)?;
    Ok(format!("https://{host}{uri}"))
}

/// The MAC carried by the `signature` header, which is the Base64 form of the
/// hex-encoded HMAC. `None` when the header is not in that form.
fn decode_webhook_signature(signature_header: &str) -> Option<Vec<u8>> {
    BASE64_ENGINE_URL_SAFE
        .decode(signature_header)
        .ok()
        .and_then(|hex_mac| hex::decode(hex_mac).ok())
}

/// Signed content of a webhook:
/// `url_path + salt + timestamp + access_key + secret_key + body`. Unlike the
/// request signature it has no HTTP method.
/// <https://docs.rapyd.net/en/webhook-authentication.html>
fn webhook_signature_message(
    request: &RequestDetails,
    keys: &RapydWebhookSecret,
) -> Result<Vec<u8>, Report<WebhookError>> {
    let url_path = webhook_url(request)?;
    let salt = required_webhook_header(request, headers::WEBHOOK_SALT)?;
    let timestamp = required_webhook_header(request, headers::WEBHOOK_TIMESTAMP)?;
    Ok([
        url_path.as_bytes(),
        salt.as_bytes(),
        timestamp.as_bytes(),
        keys.access_key.peek().as_bytes(),
        keys.secret_key.peek().as_bytes(),
        request.body.as_slice(),
    ]
    .concat())
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Rapyd<T>
{
    /// Verifies the `signature` header against the body and the delivery URL.
    ///
    /// The key is the account's API key pair, taken only from the webhook
    /// secret (a JSON `{"access_key", "secret_key"}`) and never from the
    /// credentials of the call. A signature that does not decode or does not
    /// match is `Ok(false)`; a missing secret, header or URI is an error. The
    /// event type, status and ids are all read from the signed body.
    /// <https://docs.rapyd.net/en/webhook-authentication.html>
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let connector_webhook_secret = connector_webhook_secret
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookVerificationSecretNotFound))?;
        let keys = RapydWebhookSecret::try_from(&connector_webhook_secret)?;
        let signature_header = webhook_header(&request, headers::WEBHOOK_SIGNATURE)
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;
        let Some(signature) = decode_webhook_signature(signature_header) else {
            return Ok(false);
        };
        let message = webhook_signature_message(&request, &keys)?;
        // Constant-time comparison of the raw MAC.
        common_utils::crypto::HmacSha256
            .verify_signature(keys.secret_key.peek().as_bytes(), &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let signature_header = webhook_header(request, headers::WEBHOOK_SIGNATURE)
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;
        decode_webhook_signature(signature_header)
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookSourceVerificationFailed))
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let keys = RapydWebhookSecret::try_from(connector_webhook_secret)?;
        webhook_signature_message(request, &keys)
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        RapydIncomingWebhook::from_body(&request.body)?.event_type()
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        RapydIncomingWebhook::from_body(&request.body)?.reference()
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        RapydIncomingWebhook::from_body(&request.body)?.payment_details(&request.body)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        RapydIncomingWebhook::from_body(&request.body)?.refund_details(&request.body)
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        RapydIncomingWebhook::from_body(&request.body)?.dispute_details(&request.body)
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Rapyd<T>
{
    fn id(&self) -> &'static str {
        "rapyd"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = RapydAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        // Return basic auth headers - signature will be added in get_headers method
        Ok(vec![(
            "access_key".to_string(),
            auth.access_key.into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.rapyd.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Flow-blind entry point: the non-terminal class, so `attempt_status` stays `None`.
        self.build_flow_error_response(res, event_builder, RapydErrorFlow::PaymentSync)
    }
}

macros::create_all_prerequisites!(
    connector_name: Rapyd,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: RapydPaymentsRequest<T>,
            response_body: RapydAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: RapydPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: CaptureRequest,
            response_body: RapydCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: RapydVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RapydRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RapydRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: RapydClientAuthRequest,
            response_body: RapydClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: RapydCreateOrderRequest,
            response_body: RapydCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: SetupMandate,
            request_body: RapydSetupMandateRequest<T>,
            response_body: RapydSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: RapydRepeatPaymentRequest<T>,
            response_body: RapydRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            http_method: &str,
            url_path: &str,
            body: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let auth = RapydAuthType::try_from(&req.connector_config)?;
            let timestamp = common_utils::date_time::now_unix_timestamp();
            let salt = common_utils::crypto::generate_cryptographically_secure_random_string(12);

            let signature = self.generate_signature(
                &auth,
                http_method,
                url_path,
                body,
                timestamp,
                &salt,
            )?;

            let headers = vec![
                (headers::CONTENT_TYPE.to_string(), "application/json".to_string().into()),
                ("access_key".to_string(), auth.access_key.into_masked()),
                ("salt".to_string(), salt.into()),
                ("timestamp".to_string(), timestamp.to_string().into()),
                ("signature".to_string(), signature.into()),
            ];
            Ok(headers)
        }

        /// Turns a non-2xx Rapyd body into an `ErrorResponse` for the given flow class.
        ///
        /// `code` = `status.error_code`, `message` = `status.status`, `reason` =
        /// `status.message`. A card-network decline is reported as
        /// `ERROR_PROCESSING_CARD - [NN]`; `NN` becomes `network_decline_code` and
        /// `status.message` becomes `network_error_message`:
        /// <https://docs.rapyd.net/en/card-network-errors.html>
        pub fn build_flow_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            error_flow: RapydErrorFlow,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // Status-only envelope: the `data` object of a rejected call is not
            // deserialized, so a null member in it cannot fail the parse.
            let response: Result<
                transformers::RapydErrorEnvelope,
                Report<common_utils::errors::ParsingError>,
            > = res.response.parse_struct("rapyd ErrorResponse");

            match response {
                Ok(response_data) => {
                    with_error_response_body!(event_builder, response_data);
                    let typed = macros::serialize_typed_connector_payload(
                        &response_data,
                        "typed_connector_response",
                    );
                    let status = response_data.status;
                    // A null, absent or empty `error_code` is reported as NO_ERROR_CODE.
                    let code = status
                        .error_code
                        .filter(|error_code| !error_code.is_empty())
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
                    let attempt_status = error_flow.error_attempt_status(res.status_code, &code);
                    let network_decline_code =
                        transformers::card_network_error_code(&code).map(ToString::to_string);
                    let network_error_message = network_decline_code
                        .as_ref()
                        .and_then(|_| status.message.clone());
                    Ok(ErrorResponse {
                        status_code: res.status_code,
                        code,
                        message: status
                            .status
                            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                        reason: status.message,
                        attempt_status,
                        connector_transaction_id: None,
                        network_advice_code: None,
                        network_decline_code,
                        network_error_message,
                        typed_connector_response: typed,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    })
                }
                Err(error_msg) => {
                    if let Some(event) = event_builder {
                        event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}))
                    };
                    tracing::error!(deserialization_error =? error_msg);
                    domain_types::utils::handle_json_response_deserialization_failure(res, "rapyd")
                }
            }
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn generate_signature(
            &self,
            auth: &RapydAuthType,
            http_method: &str,
            url_path: &str,
            body: &str,
            timestamp: i64,
            salt: &str,
        ) -> CustomResult<String, IntegrationError> {
            let RapydAuthType {
            access_key,
            secret_key
} = auth;
        let to_sign = format!(
            "{http_method}{url_path}{salt}{timestamp}{}{}{body}",
            access_key.peek(),
            secret_key.peek()
        );
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.peek().as_bytes());
        let tag = hmac::sign(&key, to_sign.as_bytes());
        let hmac_sign = hex::encode(tag);
        let signature_value = BASE64_ENGINE_URL_SAFE.encode(hmac_sign);
        Ok(signature_value)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydPaymentsRequest),
    curl_response: RapydAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: a rejected create is a failed attempt.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::PaymentCreate)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            // Get the exact request body that will be sent
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_response: RapydPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: an unanswered sync says nothing about the payment.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::PaymentSync)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = "";
            self.build_headers(req, "get", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.get_connector_transaction_id()?;
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(CaptureRequest),
    curl_response: RapydCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: a rejected capture leaves the payment as it was.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::PaymentModify)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.get_connector_transaction_id()?;
            Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_response: RapydVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Delete,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: a rejected cancel leaves the payment as it
        // was. Only an `ACT` payment can be cancelled; a `CLO` one is refunded:
        // <https://docs.rapyd.net/en/cancel-payment.html>
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::PaymentModify)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = "";
            self.build_headers(req, "delete", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), req.request.connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydRefundRequest),
    curl_response: RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: a Create Refund rejected with HTTP 400 ends the refund.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::RefundCreate)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_refunds(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/refunds", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_response: RapydRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: a failed refund lookup says nothing about the refund.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::RefundSync)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_refunds(req))
                .unwrap_or(&url);
            let body = "";
            self.build_headers(req, "get", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/refunds/{}", self.connector_base_url_refunds(req), req.request.connector_refund_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(RapydCreateOrderRequest),
    curl_response: RapydCreateOrderResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/checkout", self.connector_base_url_payments(req)))
        }
    }
);

// SetupMandate flow – reuses the standard `/v1/payments` endpoint for
// card-on-file verification. The saved-card id (`card_*`) it returns is
// surfaced as the connector_mandate_id for later RepeatPayment (MIT) calls.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydSetupMandateRequest),
    curl_response: RapydSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: a rejected card save is a failed attempt.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::PaymentCreate)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd SetupMandate: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd SetupMandate: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Reuse /v1/payments — `save_payment_method: true` in the body
            // yields a reusable `card_*` id.
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
    }
);

// RepeatPayment (MIT) – Rapyd has no dedicated recurring endpoint. It reuses
// `/v1/payments` with either the saved-card id (the card_* id returned by
// SetupMandate) as `payment_method`, or the card with the network reference
// id of the initial payment, and an `initiation_type` for the MIT category.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydRepeatPaymentRequest),
    curl_response: RapydRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // Flow-aware `attempt_status`: a rejected merchant-initiated create is a failed attempt.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, RapydErrorFlow::PaymentCreate)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd RepeatPayment: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd RepeatPayment: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(RapydClientAuthRequest),
    curl_response: RapydClientAuthResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_merchant_auth(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/checkout", self.connector_base_url_merchant_auth(req)))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Rapyd,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        PaymentMethodToken,
        SubmitEvidence,
        DefendDispute,
        Accept,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
    ],
);
