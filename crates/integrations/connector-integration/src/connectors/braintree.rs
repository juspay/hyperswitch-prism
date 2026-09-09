pub mod transformers;
use std::fmt::Debug;

use base64::Engine;
use common_enums::{CurrencyUnit, PaymentMethod, PaymentMethodType};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, SignMessage},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::{StringMajorUnit, StringMinorUnit},
    ParsingError,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken, RSync, Refund,
        RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, EventType, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, SetupMandateRequestData, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::Report;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as braintree, BraintreeAuthResponse, BraintreeCancelRequest, BraintreeCancelResponse,
    BraintreeCaptureRequest, BraintreeCaptureResponse, BraintreeClientTokenRequest,
    BraintreePSyncRequest, BraintreePSyncResponse, BraintreePaymentsRequest,
    BraintreePaymentsResponse, BraintreeRSyncRequest, BraintreeRSyncResponse,
    BraintreeRefundRequest, BraintreeRefundResponse, BraintreeRepeatPaymentRequest,
    BraintreeRepeatPaymentResponse, BraintreeSessionResponse, BraintreeSetupMandateRequest,
    BraintreeSetupMandateResponse, BraintreeTokenRequest, BraintreeTokenResponse,
    BraintreeVoidPCRequest, BraintreeVoidPCResponse,
};

use super::macros;
use crate::{finalize_connector_response, types::ResponseRouterData, with_error_response_body};
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;
use error_stack::ResultExt;
pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BRAINTREE_VERSION: &str = "Braintree-Version";
pub const BRAINTREE_VERSION_VALUE: &str = "2019-01-01";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Braintree<T>
{
    fn id(&self) -> &'static str {
        "braintree"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.braintree.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = braintree::BraintreeAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let auth_key = format!("{}:{}", auth.public_key.peek(), auth.private_key.peek());
        let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth_header.into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: Result<braintree::ErrorResponses, Report<ParsingError>> =
            res.response.parse_struct("Braintree Error Response");

        match response {
            Ok(braintree::ErrorResponses::BraintreeApiErrorResponse(response)) => {
                with_error_response_body!(event_builder, response);

                let typed = macros::serialize_typed_connector_payload(
                    &response,
                    "typed_connector_response",
                );
                let error_object = response.api_error_response.errors;
                let error = error_object.errors.first().or(error_object
                    .transaction
                    .as_ref()
                    .and_then(|transaction_error| {
                        transaction_error.errors.first().or(transaction_error
                            .credit_card
                            .as_ref()
                            .and_then(|credit_card_error| credit_card_error.errors.first()))
                    }));
                let (code, message) = error.map_or(
                    (NO_ERROR_CODE.to_string(), NO_ERROR_MESSAGE.to_string()),
                    |error| (error.code.clone(), error.message.clone()),
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code,
                    message,
                    reason: Some(response.api_error_response.message),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Ok(braintree::ErrorResponses::BraintreeErrorResponse(response)) => {
                with_error_response_body!(event_builder, response);
                let typed = macros::serialize_typed_connector_payload(
                    &response,
                    "typed_connector_response",
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: NO_ERROR_CODE.to_string(),
                    message: NO_ERROR_MESSAGE.to_string(),
                    reason: Some(response.errors),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Err(_) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}));
                }
                domain_types::utils::handle_json_response_deserialization_failure(res, "braintree")
            }
        }
    }
}

//marker traits
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Braintree<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Braintree,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Braintree<T>
{
    fn should_do_payment_method_token(
        &self,
        payment_method: PaymentMethod,
        _payment_method_type: Option<PaymentMethodType>,
        _is_wallet_decrypted_network_token: bool,
    ) -> bool {
        matches!(payment_method, PaymentMethod::Card)
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Braintree<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret.ok_or_else(|| {
            tracing::warn!(
                target: "braintree_webhook",
                "no webhook secret configured for Braintree source verification"
            );
            error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
        })?;

        let notif = braintree::get_webhook_object_from_body(&request.body)
            .inspect_err(|error| {
                tracing::warn!(
                    target: "braintree_webhook",
                    ?error,
                    "failed to decode the Braintree webhook body for source verification"
                );
            })
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        // `bt_signature` is `pubkey1|sig1&pubkey2|sig2&...`; split into (public_key, signature) pairs.
        let signature_pairs: Vec<(&str, &str)> = notif
            .bt_signature
            .split('&')
            .map(|pair| pair.split_once('|').unwrap_or(("", "")))
            .collect();

        // `additional_secret` holds the merchant's Braintree public key.
        let public_key = connector_webhook_secrets
            .additional_secret
            .as_ref()
            .ok_or_else(|| {
                tracing::warn!(
                    target: "braintree_webhook",
                    "missing Braintree public key (additional_secret) for source verification"
                );
                error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
            })?;

        let extracted_signature =
            braintree::get_matching_webhook_signature(&signature_pairs, public_key.peek())
                .ok_or_else(|| {
                    tracing::warn!(
                        target: "braintree_webhook",
                        "no bt_signature entry matched the merchant Braintree public key"
                    );
                    error_stack::report!(WebhookError::WebhookSignatureNotFound)
                })?;

        let message = notif.bt_payload.as_bytes();

        // Signing key is the SHA1 digest of the private key (`secret`), then HMAC-SHA1 over the payload.
        let sha1_hash_key = ring::digest::digest(
            &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
            &connector_webhook_secrets.secret,
        );

        let signed_message = crypto::HmacSha1
            .sign_message(sha1_hash_key.as_ref(), message)
            .inspect_err(|error| {
                tracing::warn!(
                    target: "braintree_webhook",
                    ?error,
                    "failed to compute the HMAC-SHA1 signature over the Braintree bt_payload"
                );
            })
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        let payload_sign = hex::encode(signed_message);

        Ok(payload_sign.as_bytes().eq(extracted_signature.as_bytes()))
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        Ok(braintree::get_status(notif.kind.as_str()))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        braintree::get_webhook_reference(&notif)
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        braintree::build_webhook_dispute_response(&notif, &request.body)
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        Ok(Box::new(notif))
    }

    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, Report<WebhookError>> {
        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: Some(b"[accepted]".to_vec()),
        })
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        // form-urlencoded `bt_signature=<pubkey>|<sig>&bt_payload=<base64 dispute_opened XML>`.
        // Dummy values only; `bt_payload` base64 decodes to a minimal `dispute_opened` notification.
        br#"bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48a2luZD5kaXNwdXRlX29wZW5lZDwva2luZD48dGltZXN0YW1wPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC90aW1lc3RhbXA%2BPGRpc3B1dGU%2BPGFtb3VudF9kaXNwdXRlZD4xMDAwPC9hbW91bnRfZGlzcHV0ZWQ%2BPGN1cnJlbmN5X2lzb19jb2RlPlVTRDwvY3VycmVuY3lfaXNvX2NvZGU%2BPGlkPmR1bW15X2Rpc3B1dGVfaWRfMDAxPC9pZD48a2luZD5DSEFSR0VCQUNLPC9raW5kPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjxyZWFzb24%2BZnJhdWQ8L3JlYXNvbj48cmVhc29uX2NvZGU%2BODM8L3JlYXNvbl9jb2RlPjx0cmFuc2FjdGlvbj48YW1vdW50PjEwLjAwPC9hbW91bnQ%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjwvdHJhbnNhY3Rpb24%2BPC9kaXNwdXRlPjwvbm90aWZpY2F0aW9uPg%3D%3D"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Braintree<T>
{
}
macros::create_all_prerequisites!(
    connector_name: Braintree,
    generic_type: T,
    api: [
        (
            flow: PaymentMethodToken,
            request_body: BraintreeTokenRequest<T>,
            response_body: BraintreeTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: PSync,
            request_body: BraintreePSyncRequest,
            response_body: BraintreePSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: BraintreeCaptureRequest,
            response_body: BraintreeCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: BraintreeCancelRequest,
            response_body: BraintreeCancelResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: BraintreeVoidPCRequest,
            response_body: BraintreeVoidPCResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: BraintreeClientTokenRequest,
            response_body: BraintreeSessionResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData , PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BraintreeRefundRequest,
            response_body: BraintreeRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: BraintreeRSyncRequest,
            response_body: BraintreeRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: BraintreeRepeatPaymentRequest,
            response_body: BraintreeRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: BraintreeSetupMandateRequest<T>,
            response_body: BraintreeSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit,
        amount_converter_webhooks: StringMinorUnit
        ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (
                    BRAINTREE_VERSION.to_string(),
                    BRAINTREE_VERSION_VALUE.to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.braintree.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.braintree.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.braintree.base_url
        }
    }
);

// Manual implementation for Authorize with conditional response body
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    > for Braintree<T>
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let connector_router_data = BraintreeRouterData {
            connector: self.to_owned(),
            router_data: req.to_owned(),
        };
        let connector_req: BraintreePaymentsRequest =
            BraintreePaymentsRequest::try_from(connector_router_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            common_utils::request::RequestContent::Json(Box::new(connector_req)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ConnectorError,
    > {
        match data.request.is_auto_capture() {
            true => {
                let response: BraintreePaymentsResponse = res
                    .response
                    .parse_struct("Braintree PaymentsResponse")
                    .change_context(
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                        "braintree: response body did not match the expected format; confirm API version and connector documentation."),
                    )?;
                finalize_connector_response!(event_builder, response, data, res.status_code)
            }
            false => {
                let response: BraintreeAuthResponse = res
                    .response
                    .parse_struct("Braintree AuthResponse")
                    .change_context(
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                        "braintree: response body did not match the expected format; confirm API version and connector documentation."),
                    )?;
                finalize_connector_response!(event_builder, response, data, res.status_code)
            }
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeRepeatPaymentRequest),
    curl_response: BraintreeRepeatPaymentResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreePSyncRequest),
    curl_response: BraintreePSyncResponse,
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
        Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeCaptureRequest),
    curl_response: BraintreeCaptureResponse,
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
         Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeCancelRequest),
    curl_response: BraintreeCancelResponse,
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
             Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeVoidPCRequest),
    curl_response: BraintreeVoidPCResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeClientTokenRequest),
    curl_response: BraintreeSessionResponse,
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
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData , PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData , PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
             Ok(self.connector_base_url_merchant_auth(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeTokenRequest),
    curl_response: BraintreeTokenResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
             Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// SetupMandate (SetupRecurring) - tokenize the card and surface the resulting
// Braintree paymentMethod.id as connector_mandate_id. RepeatPayment then
// consumes that id via its existing MandatePayment request path.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeSetupMandateRequest<T>),
    curl_response: BraintreeSetupMandateResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeRefundRequest),
    curl_response: BraintreeRefundResponse,
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
          Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeRSyncRequest),
    curl_response: BraintreeRSyncResponse,
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
             Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// ConnectorIntegrationV2 implementations for authentication flows

macros::macro_connector_flow_status_impls!(
    connector: Braintree,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        CreateOrder,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        SubmitEvidence,
        DefendDispute,
        Accept,
        MandateRevoke,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
