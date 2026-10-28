//! Kgp — Omise credit-card connector.
//!
//! Two hosts, two keys, form-urlencoded:
//!   - `vault.omise.co` + public key  → `POST /tokens`
//!   - `api.omise.co`   + secret key  → charges / capture / reverse / refunds
//!
//! Wire protocol: `grace/rulesbook/codegen/references/kgp/technical_specification.md`.

pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::SignMessage,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PaymentMethodToken, PSync, RSync, Refund,
        Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, ConnectorWebhookSecrets, EventType,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        PaymentVoidData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RequestDetails, WebhookResourceReference,
    },
    errors::{self, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use common_utils::types::MinorUnit;
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as kgp, KgpAuthorizeResponse, KgpCaptureRequest, KgpCaptureResponse,
    KgpCustomerRequest, KgpCustomerResponse, KgpPaymentsRequest, KgpRefundRequest,
    KgpRefundResponse, KgpRefundSyncResponse,
    KgpSyncResponse, KgpToken, KgpTokenRequest, KgpVoidRequest, KgpVoidResponse,
};

use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const OMISE_SIGNATURE: &str = "Omise-Signature";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

// ============================================================================
// FOUNDATION SETUP — create_all_prerequisites!
// ============================================================================

macros::create_all_prerequisites!(
    connector_name: Kgp,
    generic_type: T,
    api: [
        (
            flow: PaymentMethodToken,
            request_body: KgpTokenRequest<T>,
            response_body: KgpToken,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: Authorize,
            request_body: KgpPaymentsRequest,
            response_body: KgpAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: KgpSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: KgpCaptureRequest,
            response_body: KgpCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: KgpVoidRequest,
            response_body: KgpVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: KgpRefundRequest,
            response_body: KgpRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: KgpRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: CreateConnectorCustomer,
            request_body: KgpCustomerRequest,
            response_body: KgpCustomerResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        /// Basic auth with the Omise **secret** key (api.omise.co).
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut auth = self
                .get_auth_header(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            header.append(&mut auth);
            Ok(header)
        }

        /// Basic auth with the Omise **public** key (vault.omise.co tokens).
        pub fn build_vault_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let auth = kgp::KgpAuthType::try_from(&req.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;
            let public_key = auth.public_key.ok_or_else(|| {
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                }
            })?;
            let encoded = BASE64_ENGINE.encode(format!("{}:", public_key.peek()));
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Basic {encoded}").into_masked(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.kgp.base_url
        }

        pub fn connector_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.kgp.base_url
        }

        /// vault.omise.co — tokenization host. Falls back to the production
        /// vault host when `secondary_base_url` is not configured.
        pub fn connector_base_url_vault<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data
                .connectors
                .kgp
                .secondary_base_url
                .clone()
                .unwrap_or_else(|| "https://vault.omise.co/".to_string())
        }
    }
);

// ============================================================================
// CONNECTOR COMMON
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Kgp<T>
{
    fn id(&self) -> &'static str {
        "kgp"
    }

    // Omise amounts are integer minor units on the wire.
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    // Omise speaks form-urlencoded, not JSON.
    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.kgp.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = kgp::KgpAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        // HTTP Basic with empty password: base64("<secret_key>:")
        let encoded = BASE64_ENGINE.encode(format!("{}:", auth.secret_key.peek()));
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {encoded}").into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: kgp::KgpErrorResponse = res
            .response
            .parse_struct("KgpErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "kgp: error body did not match the documented error shape",
            ))?;

        with_error_response_body!(event_builder, response);

        // Omise error codes are request-level (invalid_charge, etc.), not
        // terminal payment declines — leave attempt_status as None so the
        // router does not freeze a live payment as Failure on a PSync 4xx.
        let attempt_status: Option<FlowStatus> = None;

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.message.clone(),
            attempt_status,
            typed_connector_response: macros::serialize_typed_connector_payload(
                &response,
                "typed_connector_response",
            ),
            ..Default::default()
        })
    }
}

// ============================================================================
// BASE (NON-FLOW) TRAIT IMPLEMENTATIONS
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Kgp<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Kgp<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Kgp<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Kgp<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        let secrets = connector_webhook_secret.ok_or_else(|| {
            error_stack::report!(errors::WebhookError::WebhookVerificationSecretNotFound)
        })?;

        // Header lookup is case-insensitive — HashMap key casing varies by
        // caller (same approach as grabpay's get_webhook_header).
        let sig_header = request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(headers::OMISE_SIGNATURE))
            .map(|(_, value)| value.as_str())
            .and_then(kgp::parse_kgp_signature)
            .ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookSignatureNotFound)
            })?;

        // Omise signs "{timestamp}.{raw body}" with HMAC-SHA256, hex-encoded.
        let message = format!(
            "{}.{}",
            sig_header.timestamp,
            String::from_utf8_lossy(&request.body)
        )
        .into_bytes();

        let signed = common_utils::crypto::HmacSha256
            .sign_message(&secrets.secret, &message)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)?;

        Ok(hex::encode(&signed).eq(&sig_header.signature))
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let event: kgp::KgpWebhookEvent = serde_json::from_slice(&request.body)
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let key = event.key.unwrap_or_default();
        let status = event.data.as_ref().and_then(|d| d.status.as_deref());

        // Omise keys: charge.create | charge.complete | charge.capture |
        // charge.expire | charge.fail | refund.create | refund.close | refund.fail.
        // Only terminal keys map to Success/Failure — charge.create is the
        // initial webhook and must not mark a payment Charged.
        if key.starts_with("charge.create") {
            Ok(EventType::PaymentIntentProcessing)
        } else if key.starts_with("charge.complete") {
            match status {
                Some("successful") => Ok(EventType::PaymentIntentSuccess),
                Some("failed") | Some("expired") => Ok(EventType::PaymentIntentFailure),
                Some("reversed") => Ok(EventType::PaymentIntentCancelled),
                Some("pending") | _ => Ok(EventType::PaymentIntentProcessing),
            }
        } else if key.starts_with("charge.capture") {
            Ok(EventType::PaymentIntentCaptureSuccess)
        } else if key.starts_with("charge.expire") {
            Ok(EventType::PaymentIntentExpired)
        } else if key.starts_with("charge.fail") {
            Ok(EventType::PaymentIntentFailure)
        } else if key.starts_with("refund.create") {
            Ok(EventType::RefundProcessing)
        } else if key.starts_with("refund.close") {
            Ok(EventType::RefundSuccess)
        } else if key.starts_with("refund.fail") {
            Ok(EventType::RefundFailure)
        } else {
            Ok(EventType::IncomingWebhookEventUnspecified)
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let event: kgp::KgpWebhookEvent = serde_json::from_slice(&request.body)
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;
        Ok(kgp::kgp_webhook_reference(&event))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Kgp<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Kgp<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Kgp,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// The 8 implemented flows (PaymentMethodToken, Authorize, PSync, Capture, Void,
// Refund, RSync, CreateConnectorCustomer) are removed from
// `not_implemented` below and have real `macro_connector_implementation!` blocks.
// Everything else stays stubbed.
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Kgp,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        DefendDispute,
        GetConnectorCustomer,
        MandateRevoke,
        Authenticate,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        RepeatPayment,
        VoidPC,
        VoidPostRefund,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
    not_supported: [
        // Omise has no native incremental-auth / mandate / dispute defend surface
        // in this connector scope.
    ],
);

// ============================================================================
// FLOW IMPLEMENTATIONS
// ============================================================================

// ----- PaymentMethodToken: POST {vault}/tokens (public key) -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_request: FormUrlEncoded(KgpTokenRequest),
    curl_response: KgpToken,
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
            self.build_vault_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}tokens", self.connector_base_url_vault(req)))
        }
    }
);

// ----- Authorize: POST {api}/charges -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_request: FormUrlEncoded(KgpPaymentsRequest),
    curl_response: KgpAuthorizeResponse,
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
            Ok(format!("{}charges", self.connector_base_url_payments(req)))
        }
    }
);

// ----- PSync: GET {api}/charges/{id} -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_response: KgpSyncResponse,
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
            // Pre-Flight Gate §15: never re-derive a lookup key we could have
            // read. A missing connector_transaction_id carrier is a hard
            // MissingRequiredField, not a default / unwrap_or.
            let connector_payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}charges/{connector_payment_id}",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ----- Capture: POST {api}/charges/{id}/capture -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_request: FormUrlEncoded(KgpCaptureRequest),
    curl_response: KgpCaptureResponse,
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
            let id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}charges/{id}/capture",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ----- Void: POST {api}/charges/{id}/reverse (uncaptured only) -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_request: FormUrlEncoded(KgpVoidRequest),
    curl_response: KgpVoidResponse,
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
            let id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}charges/{id}/reverse",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ----- Refund: POST {api}/charges/{id}/refunds -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> connector_types::RefundV2
    for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_request: FormUrlEncoded(KgpRefundRequest),
    curl_response: KgpRefundResponse,
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
            let id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}charges/{id}/refunds",
                self.connector_base_url(req)
            ))
        }
    }
);

// ----- RSync: GET {api}/charges/{charge_id}/refunds/{refund_id} -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_response: KgpRefundSyncResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Nested refund resource. Flat GET {api}/refunds/{id} 404s on Omise.
            Ok(format!(
                "{}charges/{}/refunds/{}",
                self.connector_base_url(req),
                req.request.connector_transaction_id,
                req.request.connector_refund_id,
            ))
        }
    }
);

// ----- CreateConnectorCustomer: POST {api}/customers -----
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Kgp<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Kgp,
    curl_request: FormUrlEncoded(KgpCustomerRequest),
    curl_response: KgpCustomerResponse,
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
            Ok(format!("{}customers", self.connector_base_url_payments(req)))
        }
    }
);
