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
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
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
    RapydCreateOrderRequest, RapydCreateOrderResponse, RapydIncomingWebhook, RapydPaymentsRequest,
    RapydPaymentsResponse as RapydCaptureResponse, RapydPaymentsResponse as RapydPSyncResponse,
    RapydPaymentsResponse as RapydVoidResponse, RapydPaymentsResponse as RapydAuthorizeResponse,
    RapydRefundRequest, RapydRepeatPaymentRequest, RapydRepeatPaymentResponse,
    RapydSetupMandateRequest, RapydSetupMandateResponse, RapydWebhookData, RapydWebhookSecret,
    RefundResponse, RefundResponse as RapydRSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
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
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Rapyd<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"wh_probe_001","type":"PAYMENT_COMPLETED","data":{"id":"payment_probe_001","amount":10.00,"status":"CLO","next_action":"not_applicable","currency_code":"USD","transaction_id":"","merchant_reference_id":"probe_ref_001"},"trigger_operation_id":"probe_operation_001","status":"NEW","created_at":1700000000}"#
    }

    /// Source verification, fail closed.
    ///
    /// Documented formula (https://docs.rapyd.net/en/webhook-authentication.html):
    /// `signature = BASE64 ( HASH ( url_path + salt + timestamp + access_key + secret_key + body_string ) )`
    /// where HASH is HMAC-SHA256 keyed with the secret key and hex-encoded,
    /// `url_path` is the entire URL the webhook was delivered to, and — unlike
    /// a request signature — the HTTP method is not part of the message.
    ///
    /// The keys come from the webhook secret only (the JSON object
    /// `{"access_key": "...", "secret_key": "..."}`); the connector account
    /// config is never used in its place.
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let webhook_secret = connector_webhook_secret
            .ok_or_else(|| Report::new(WebhookError::WebhookVerificationSecretNotFound))?;
        // `serde_json::from_slice`, with the serde error dropped: neither it
        // nor the secret bytes may reach the error (and from there the log).
        let keys: RapydWebhookSecret = serde_json::from_slice(&webhook_secret.secret)
            .map_err(|_| Report::new(WebhookError::WebhookVerificationSecretInvalid))?;

        let header = |name: &str| {
            request
                .headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
                .filter(|value| !value.is_empty())
        };
        let (Some(signature), Some(salt), Some(timestamp)) =
            (header("signature"), header("salt"), header("timestamp"))
        else {
            return Ok(false);
        };

        // The webhook URL: the forwarded URI when it is absolute, else
        // `https://` + the `host` header + the URI.
        let webhook_url = match request.uri.as_deref().filter(|uri| !uri.is_empty()) {
            Some(uri) if uri.starts_with("https://") || uri.starts_with("http://") => {
                uri.to_owned()
            }
            Some(uri) => match header("host") {
                Some(host) => format!("https://{host}{uri}"),
                None => return Ok(false),
            },
            None => return Ok(false),
        };

        // The header is BASE64(hex(digest)).
        let Some(expected_digest) = BASE64_ENGINE_URL_SAFE
            .decode(signature)
            .ok()
            .and_then(|hex_digest| hex::decode(hex_digest).ok())
        else {
            return Ok(false);
        };

        let mut message = Vec::with_capacity(
            webhook_url.len()
                + salt.len()
                + timestamp.len()
                + keys.access_key.peek().len()
                + keys.secret_key.peek().len()
                + request.body.len(),
        );
        message.extend_from_slice(webhook_url.as_bytes());
        message.extend_from_slice(salt.as_bytes());
        message.extend_from_slice(timestamp.as_bytes());
        message.extend_from_slice(keys.access_key.peek().as_bytes());
        message.extend_from_slice(keys.secret_key.peek().as_bytes());
        message.extend_from_slice(&request.body);

        common_utils::crypto::HmacSha256
            .verify_signature(
                keys.secret_key.peek().as_bytes(),
                &expected_digest,
                &message,
            )
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        parse_rapyd_webhook(&request.body)?.event_type()
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        parse_rapyd_webhook(&request.body)?.event_reference()
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        // Every event without a payment / refund / dispute event type is
        // routed here; only a payment object is read as one.
        match parse_rapyd_webhook(&request.body)?.into_data()? {
            Some(RapydWebhookData::Payment(payment)) => {
                transformers::build_rapyd_payment_webhook_details(*payment, &request.body)
            }
            Some(RapydWebhookData::Refund(_)) | Some(RapydWebhookData::Dispute(_)) | None => {
                Err(Report::new(WebhookError::WebhooksNotImplemented {
                    operation: "process_payment_webhook",
                }))
            }
        }
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        match parse_rapyd_webhook(&request.body)?.into_data()? {
            Some(RapydWebhookData::Refund(refund)) => Ok(
                transformers::build_rapyd_refund_webhook_details(refund, &request.body),
            ),
            Some(RapydWebhookData::Payment(_)) | Some(RapydWebhookData::Dispute(_)) | None => {
                Err(Report::new(WebhookError::WebhooksNotImplemented {
                    operation: "process_refund_webhook",
                }))
            }
        }
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        match parse_rapyd_webhook(&request.body)?.into_data()? {
            Some(RapydWebhookData::Dispute(dispute)) => {
                transformers::build_rapyd_dispute_webhook_details(dispute, &request.body)
            }
            Some(RapydWebhookData::Payment(_)) | Some(RapydWebhookData::Refund(_)) | None => {
                Err(Report::new(WebhookError::WebhooksNotImplemented {
                    operation: "process_dispute_webhook",
                }))
            }
        }
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        match parse_rapyd_webhook(&request.body)?.into_data()? {
            Some(RapydWebhookData::Payment(payment)) => Ok(payment),
            Some(RapydWebhookData::Refund(refund)) => Ok(Box::new(refund)),
            Some(RapydWebhookData::Dispute(dispute)) => Ok(Box::new(dispute)),
            None => Err(Report::new(WebhookError::WebhookResourceObjectNotFound)),
        }
    }
}

/// The envelope of a Rapyd notification; a body that is not one is an error.
fn parse_rapyd_webhook(body: &[u8]) -> Result<RapydIncomingWebhook, Report<WebhookError>> {
    body.parse_struct("RapydIncomingWebhook")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
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
        // Non-terminal by default: PSync, RSync, Capture and Void must not fail
        // an attempt because the call about it was refused. Flows whose 4xx is
        // terminal override `get_error_response_v2`.
        self.build_error_response_with_status(res, event_builder, None)
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

        /// Parses the Rapyd envelope of a non-2xx response and builds the
        /// `ErrorResponse` through the connector's single error builder, with
        /// the `attempt_status` the calling flow decides.
        pub fn build_error_response_with_status(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            attempt_status: Option<FlowStatus>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: Result<transformers::RapydErrorEnvelope, Report<common_utils::errors::ParsingError>> =
                res.response.parse_struct("rapyd ErrorResponse");

            match response {
                Ok(response_data) => {
                    with_error_response_body!(event_builder, response_data);
                    let typed_connector_response = macros::serialize_typed_connector_payload(
                        &response_data,
                        "typed_connector_response",
                    );
                    Ok(ErrorResponse {
                        typed_connector_response,
                        ..transformers::build_rapyd_error_response_from_envelope(
                            &response_data,
                            res.status_code,
                            attempt_status,
                        )
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
        // A Create Payment that Rapyd answers with a 4xx envelope (issuer
        // decline, validation or authentication error) created no payment:
        // the attempt is failed, not left in an unknown state. 5xx responses
        // keep the macro default (`get_5xx_error_response`, no status).
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = (400..500)
                .contains(&res.status_code)
                .then_some(FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            self.build_error_response_with_status(res, event_builder, attempt_status)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
        // A Create Refund that Rapyd refuses with an HTTP 400 envelope
        // (PAYMENT_NOT_COMPLETED, ERROR_REFUND_AMOUNT_EXCEEDS_PAYMENT_AMOUNT, …)
        // created no refund: the refund is failed, not left pending. Every
        // other status code (401, 5xx, …) and an unparsable body keep the
        // status unset.
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = (res.status_code == 400)
                .then_some(FlowStatus::Refund(common_enums::RefundStatus::Failure));
            self.build_error_response_with_status(res, event_builder, attempt_status)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
// card-on-file verification. The returned payment id is surfaced as the
// connector_mandate_id for subsequent RepeatPayment (MIT) calls.
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
            // Reuse /v1/payments — `save_payment_method: true` + inline customer
            // object in the body yields a reusable `card_*` token without
            // requiring the complete_payment_url whitelist that the
            // /v1/customers endpoint enforces on sandbox accounts.
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
        // A card verification that Rapyd answers with a 4xx envelope (issuer
        // decline, validation or authentication error) created no payment and
        // stored no card: the setup is failed, not left in an unknown state.
        // 5xx responses keep the macro default (`get_5xx_error_response`, no
        // status).
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = (400..500)
                .contains(&res.status_code)
                .then_some(FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            self.build_error_response_with_status(res, event_builder, attempt_status)
        }
    }
);

// RepeatPayment (MIT) – Rapyd has no dedicated recurring endpoint. It reuses
// `/v1/payments` with the stored card id (`card_…`) as `payment_method`, or
// with card details carrying the network reference id of the initial
// payment, plus the `initiation_type` of the merchant-initiated payment.
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
        // A merchant-initiated Create Payment that Rapyd answers with a 4xx
        // envelope (issuer decline, validation or authentication error)
        // created no payment: the attempt is failed, not left in an unknown
        // state. 5xx responses keep the macro default
        // (`get_5xx_error_response`, no status).
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = (400..500)
                .contains(&res.status_code)
                .then_some(FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            self.build_error_response_with_status(res, event_builder, attempt_status)
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
