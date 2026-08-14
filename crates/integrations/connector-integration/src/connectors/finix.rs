pub mod transformers;

use super::macros;
use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_enums::{PaymentMethod, PaymentMethodType};
use common_utils::{
    crypto::{self, SignMessage},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, PaymentMethodToken, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::*,
    connector_types::{RefundFlowData, RefundSyncData, RefundsResponseData},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as finix, FinixAuthorizeRequest, FinixAuthorizeResponse, FinixCaptureRequest,
    FinixCaptureResponse, FinixCreateIdentityRequest, FinixCreatePaymentInstrumentRequest,
    FinixIdentityResponse, FinixInstrumentResponse, FinixPSyncResponse, FinixRSyncResponse,
    FinixRefundRequest, FinixRefundResponse, FinixRepeatPaymentRequest, FinixRepeatPaymentResponse,
    FinixSetupMandateRequest, FinixSetupMandateResponse, FinixVoidRequest, FinixVoidResponse,
};

use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const FINIX_REFERRER_SOURCE_HEADER: &str = "X-Finix-Referrer-Source";
const FINIX_REFERRER_SOURCE_VALUE: &str = "PLUGIN_HYPERSWITCH";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Finix<T>
{
    fn id(&self) -> &'static str {
        "finix"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.finix.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = finix::FinixAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let encoded_auth = auth.generate_basic_auth();
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {}", encoded_auth).into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: finix::FinixErrorResponse = res
            .response
            .parse_struct("FinixErrorResponse")
            .change_context(
            crate::utils::response_deserialization_fail(res.status_code, "finix: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.get_code(),
            message: response.get_message(),
            reason: None,
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

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Finix<T>
{
}

// =============================================================================
// VALIDATION TRAIT IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Finix<T>
{
    /// Enable automatic connector customer creation before payment
    /// When enabled, UCS will automatically call CreateConnectorCustomer before Authorize
    fn should_create_connector_customer(&self) -> bool {
        true
    }

    /// Enable automatic payment method tokenization before payment
    /// When enabled, UCS will automatically call PaymentMethodToken before Authorize
    fn should_do_payment_method_token(
        &self,
        payment_method: PaymentMethod,
        payment_method_type: Option<PaymentMethodType>,
        _is_wallet_pre_decrypted: bool,
    ) -> bool {
        // Check for specific wallet types that need tokenization
        let is_google_pay_wallet = payment_method == PaymentMethod::Wallet
            && matches!(payment_method_type, Some(PaymentMethodType::GooglePay));

        // Card and BankDebit always need tokenization
        matches!(
            payment_method,
            PaymentMethod::Card | PaymentMethod::BankDebit
        ) || is_google_pay_wallet
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Finix<T>
{
}

// ===== FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Finix<T>
{
}

// Ported 1:1 from hyperswitch `impl webhooks::IncomingWebhook for Finix`
// (crates/hyperswitch_connectors/src/connectors/finix.rs). Hyperswitch (Direct
// gateway) is the source of truth: HMAC-SHA256 over `"{timestamp}:{raw body}"`
// with the hex-encoded signature carried in the comma-separated
// `Finix-Signature: timestamp=...,sig=...` header.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Finix<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret.ok_or_else(|| {
            tracing::warn!(
                target: "finix_webhook",
                "no webhook secret configured for Finix source verification"
            );
            error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
        })?;

        let signature_header = finix::decode_finix_signature(&request.headers)?;

        // HS hex-decodes the `sig` value before comparing
        // (get_webhook_source_verification_signature).
        let signature = hex::decode(&signature_header.sig)
            .change_context(WebhookError::WebhookSignatureNotFound)
            .attach_printable("Finix-Signature `sig` value is not valid hex")?;

        // HS signs `"{timestamp}:{raw body}"` — the RAW request bytes, never a
        // re-serialization (get_webhook_source_verification_message).
        let message = format!(
            "{}:{}",
            signature_header.timestamp,
            String::from_utf8_lossy(&request.body)
        )
        .into_bytes();

        let signed_message = crypto::HmacSha256
            .sign_message(&connector_webhook_secrets.secret, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("failed to compute the HMAC-SHA256 over the Finix webhook message")?;

        Ok(signed_message.eq(&signature))
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // HS: an empty body or a body that trims to `{}` is Finix's
        // endpoint-verification probe (get_webhook_event_type).
        if finix::is_endpoint_verification_body(&request.body) {
            return Ok(EventType::EndpointVerification);
        }
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::get_finix_webhook_event_type(&webhook_body)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        // Endpoint-verification probes carry no resource; HS's Direct gateway
        // also resolves no reference for them (`get_webhook_object_reference_id`
        // fails to parse `{}` and the router `.ok()`s it away).
        if finix::is_endpoint_verification_body(&request.body) {
            return Ok(None);
        }
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::get_finix_webhook_reference(&webhook_body)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::build_finix_payment_webhook_response(&webhook_body, &request.body)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::build_finix_refund_webhook_response(&webhook_body, &request.body)
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::build_finix_dispute_webhook_response(&webhook_body, &request.body)
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        Ok(Box::new(webhook_body))
    }

    /// A minimal, structurally valid Finix transfer webhook (field-probe input).
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"type":"updated","entity":"transfer","_embedded":{"transfers":[{"id":"TRsample000000000000000","amount":1000,"currency":"USD","state":"SUCCEEDED","tags":{},"type":"DEBIT"}]}}"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Finix<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Finix,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Finix<T>
{
}

// ===== CONNECTOR INTEGRATION V2 IMPLEMENTATIONS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Finix<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Finix,
    generic_type: T,
    api: [
        (
            flow: CreateConnectorCustomer,
            request_body: FinixCreateIdentityRequest,
            response_body: FinixIdentityResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: FinixCreatePaymentInstrumentRequest,
            response_body: FinixInstrumentResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: Authorize,
            request_body: FinixAuthorizeRequest,
            response_body: FinixAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: FinixPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: FinixCaptureRequest,
            response_body: FinixCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            response_body: FinixRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: FinixVoidRequest,
            response_body: FinixVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: FinixRefundRequest,
            response_body: FinixRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: FinixSetupMandateRequest,
            response_body: FinixSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: FinixRepeatPaymentRequest,
            response_body: FinixRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                FINIX_REFERRER_SOURCE_HEADER.to_string(),
                FINIX_REFERRER_SOURCE_VALUE.to_string().into(),
                ),
            ];
            let mut auth_headers = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth_headers);
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.finix.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.finix.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixCreateIdentityRequest),
    curl_response: FinixIdentityResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
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
            Ok(format!("{}/identities", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixCreatePaymentInstrumentRequest),
    curl_response: FinixInstrumentResponse,
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
            Ok(format!("{}/payment_instruments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixAuthorizeRequest),
    curl_response: FinixAuthorizeResponse,
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
            // Route based on capture_method: AUTOMATIC -> /transfers, MANUAL -> /authorizations
            let endpoint = match req.request.capture_method {
                Some(common_enums::CaptureMethod::Automatic) |
                Some(common_enums::CaptureMethod::SequentialAutomatic) => "transfers",
                _ => "authorizations"
};
            Ok(format!("{}/{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_response: FinixPSyncResponse,
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
            let connector_transaction_id = req.request.get_connector_transaction_id()?;
            // Route based on ID type using FinixId enum
            let finix_id = transformers::FinixId::from(connector_transaction_id);
            let endpoint = match finix_id {
                transformers::FinixId::Auth(_) => "authorizations",
                transformers::FinixId::Transfer(_) => "transfers"
};
            Ok(format!("{}/{}/{}", self.connector_base_url_payments(req), endpoint, finix_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixVoidRequest),
    curl_response: FinixVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Put,
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
            let connector_transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/authorizations/{}", self.connector_base_url_payments(req), connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_response: FinixRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                )
            ];
            let mut auth_headers = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth_headers);
            Ok(headers)
        }

        fn get_url(&self, req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>) -> CustomResult<String, IntegrationError> {
            let connector_refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/transfers/{}", self.connector_base_url_refunds(req), connector_refund_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixCaptureRequest),
    curl_response: FinixCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Put,
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
            let connector_transaction_id = req.request.get_connector_transaction_id()?;
            Ok(format!("{}/authorizations/{}", self.connector_base_url_payments(req), connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixRefundRequest),
    curl_response: FinixRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                )
            ];
            let mut auth_headers = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth_headers);
            Ok(headers)
        }

        fn get_url(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<String, IntegrationError> {
            let connector_transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/transfers/{}/reversals", self.connector_base_url_refunds(req), connector_transaction_id))
        }
    }
);

// SetupMandate (SetupRecurring) - stores payment instrument for recurring payments
// Finix requires an identity (connector customer) to create a payment instrument
// The flow creates a payment instrument and returns its ID as the connector_mandate_id
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixSetupMandateRequest),
    curl_response: FinixSetupMandateResponse,
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
            Ok(format!("{}/payment_instruments", self.connector_base_url_payments(req)))
        }
    }
);

// RepeatPayment (MIT) - charge a previously stored Payment Instrument.
// Routes to /transfers for auto-capture and /authorizations for manual capture,
// matching the Authorize flow's behaviour.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixRepeatPaymentRequest),
    curl_response: FinixRepeatPaymentResponse,
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
            let endpoint = match req.request.capture_method {
                Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled) => "authorizations",
                _ => "transfers",
            };
            Ok(format!("{}/{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Finix,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        GetConnectorCustomer,
        Accept,
        DefendDispute,
        VoidPC,
        SubmitEvidence,
    ],
    not_supported: [
        VoidPostRefund,
        MandateRevoke,
        ServerAuthenticationToken,
        Authenticate,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
    ],
);
