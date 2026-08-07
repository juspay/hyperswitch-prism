pub mod transformers;

use common_utils::{
    crypto::{self, SignMessage, VerifySignature},
    date_time,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::{FloatMajorUnit, MinorUnit},
};
use domain_types::router_data::ConnectorSpecificConfig;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, MandateReference, PaymentFlowData,
        PaymentVoidData, PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    self as dlocal, DlocalPaymentStatus, DlocalPaymentsCaptureRequest, DlocalPaymentsRequest,
    DlocalPaymentsResponse, DlocalPaymentsResponse as DlocalPaymentsSyncResponse,
    DlocalPaymentsResponse as DlocalPaymentsCaptureResponse,
    DlocalPaymentsResponse as DlocalPaymentsVoidResponse, DlocalRefundRequest,
    DlocalRepeatPaymentRequest, DlocalRepeatPaymentResponse, DlocalSetupMandateRequest,
    DlocalSetupMandateResponse, DlocalWebhookBody, RefundResponse,
    RefundResponse as RefundSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;

const VERSION: &str = "2.1";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Dlocal<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Dlocal,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Dlocal<T>
{
    /// The signature dLocal sends with the IPN: the hex value in the
    /// `Authorization: V2-HMAC-SHA256, Signature: <hex>` header, decoded to bytes.
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let authorization = get_header_case_insensitive(request, headers::AUTHORIZATION)
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;
        // Format: "V2-HMAC-SHA256, Signature: <hex>"
        let signature_hex = authorization
            .rsplit("Signature:")
            .next()
            .map(str::trim)
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;
        hex::decode(signature_hex).change_context(WebhookError::WebhookSignatureNotFound)
    }

    /// The message dLocal signs for the IPN is identical to the outbound request
    /// signing scheme reused in `build_headers`: `X-Login + X-Date + rawBody`.
    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let x_login = get_header_case_insensitive(request, headers::X_LOGIN)
            .ok_or_else(|| report!(WebhookError::WebhookSourceVerificationFailed))?;
        let x_date = get_header_case_insensitive(request, headers::X_DATE)
            .ok_or_else(|| report!(WebhookError::WebhookSourceVerificationFailed))?;
        let mut message = format!("{x_login}{x_date}").into_bytes();
        message.extend_from_slice(&request.body);
        Ok(message)
    }

    /// dLocal signs the IPN with the same V2-HMAC-SHA256 scheme as outbound
    /// requests: `HMAC_SHA256(secret, X-Login + X-Date + rawBody)`. We recompute
    /// it with the configured webhook secret and compare (constant-time, via
    /// `crypto::HmacSha256::verify_signature`).
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => return Ok(false),
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        crypto::HmacSha256
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"E-probe-001","external_id":"probe_order_001","status":"ACTIVE","status_code":"200","payment_method_id":"RG","payment_method_type":"WALLET","payment_method_flow":"REDIRECT"}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: DlocalWebhookBody = request
            .body
            .parse_struct("DlocalWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(EventType::from(&body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: DlocalWebhookBody = request
            .body
            .parse_struct("DlocalWebhookBody")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                // dLocal payment `id` is the connector transaction id.
                connector_transaction_id: Some(body.id),
                // `order_id` is used for payment objects; `external_id` is used
                // for enrollment objects.
                merchant_transaction_id: body.order_id.or(body.external_id),
            },
        )))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let body: DlocalWebhookBody = request
            .body
            .parse_struct("DlocalWebhookBody")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;
        Ok(Box::new(body))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body: DlocalWebhookBody = request
            .body
            .parse_struct("DlocalWebhookBody")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        let is_enrollment_webhook = body.is_enrollment_webhook();
        let status = if is_enrollment_webhook && matches!(&body.status, DlocalPaymentStatus::Active)
        {
            common_enums::AttemptStatus::Charged
        } else {
            common_enums::AttemptStatus::from(body.status.clone())
        };
        // Only surface error details for genuine failures. dLocal sends
        // `status_code`/`status_detail` on success too (PAID -> "200" / "The payment
        // was paid."); copying those into error_* makes HS treat a CHARGED webhook as
        // an errored response (UE_9000) and skip persisting the connector mandate id.
        let is_failure = matches!(
            &body.status,
            DlocalPaymentStatus::Rejected | DlocalPaymentStatus::Cancelled
        );

        let connector_mandate_id =
            if is_enrollment_webhook && matches!(&body.status, DlocalPaymentStatus::Active) {
                Some(body.id.clone())
            } else {
                body.enrollment_id_from_successful_payment()
            };

        let mandate_reference = connector_mandate_id.map(|enrollment_id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(enrollment_id),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(body.id.clone())),
            status,
            connector_response_reference_id: body
                .order_id
                .clone()
                .or_else(|| body.external_id.clone()),
            connector_request_reference_id: body
                .order_id
                .clone()
                .or_else(|| body.external_id.clone()),
            mandate_reference,
            error_code: is_failure
                .then(|| body.status_code.as_ref().map(ToString::to_string))
                .flatten(),
            error_message: is_failure.then(|| body.status_detail.clone()).flatten(),
            error_reason: is_failure.then_some(body.status_detail).flatten(),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }
}

/// dLocal IPN headers may arrive with their canonical casing (`X-Login`) or
/// lowercased depending on the proxy; look them up case-insensitively.
fn get_header_case_insensitive(request: &RequestDetails, name: &str) -> Option<String> {
    request
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Dlocal<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Dlocal<T>
{
}
pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_DATE: &str = "X-Date";
    pub(crate) const X_LOGIN: &str = "X-Login";
    pub(crate) const X_TRANS_KEY: &str = "X-Trans-Key";
    pub(crate) const X_VERSION: &str = "X-Version";
}

macros::create_all_prerequisites!(
    connector_name: Dlocal,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: DlocalPaymentsRequest<T>,
            response_body: DlocalPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: DlocalPaymentsSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: DlocalRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Capture,
            request_body: DlocalPaymentsCaptureRequest,
            response_body: DlocalPaymentsCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: DlocalPaymentsVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: DlocalRepeatPaymentRequest,
            response_body: DlocalRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: DlocalSetupMandateRequest<T>,
            response_body: DlocalSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let date = date_time::date_as_yyyymmddthhmmssmmmz()
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
            let auth = dlocal::DlocalAuthType::try_from(&req.connector_config)?;

            let sign_req: String = match self.get_request_body(req)? {
                Some(dlocal_req) => format!(
                    "{}{}{}",
                    auth.x_login.peek(),
                    date,
                    dlocal_req.get_inner_value().peek().to_owned()
                ),
                None => format!("{}{}", auth.x_login.peek(), date)
};

            let authz = crypto::HmacSha256::sign_message(
                &crypto::HmacSha256,
                auth.secret.peek().as_bytes(),
                sign_req.as_bytes(),
            )
            .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })
            .attach_printable("Failed to sign the message")?;
            let auth_string: String = format!("V2-HMAC-SHA256, Signature: {}", hex::encode(authz));
            let headers = vec![
                (
                    headers::AUTHORIZATION.to_string(),
                    auth_string.into_masked(),
                ),
                (headers::X_LOGIN.to_string(), auth.x_login.into_masked()),
                (
                    headers::X_TRANS_KEY.to_string(),
                    auth.x_trans_key.into_masked(),
                ),
                (headers::X_VERSION.to_string(), VERSION.to_string().into()),
                (headers::X_DATE.to_string(), date.into()),
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
            ];
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.dlocal.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.dlocal.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Dlocal<T>
{
    fn id(&self) -> &'static str {
        "dlocal"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.dlocal.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: dlocal::DlocalErrorResponse = res
            .response
            .parse_struct("Dlocal ErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "dlocal: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code.to_string(),
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_request: Json(DlocalPaymentsRequest),
    curl_response: DlocalPaymentsResponse,
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
            let base_url = self.connector_base_url_payments(req);
            // hyperswitch dlocal routes all Authorize traffic to /secure_payments. Mirror
            // that for the in-scope HS-parity flows (Card + Voucher/OXXO) while leaving
            // wallet / bank-transfer / bank-debit on /payments, where they currently work.
            match &req.request.payment_method_data {
                PaymentMethodData::Card(_) | PaymentMethodData::Voucher(_) => {
                    Ok(format!("{base_url}secure_payments"))
                }
                _ => Ok(format!("{base_url}payments")),
            }
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_response: DlocalPaymentsResponse,
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
            let connector_transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;

            if req.request.amount == MinorUnit::new(0) {
                Ok(format!(
                    "{}enrollments/{connector_transaction_id}",
                    self.connector_base_url_payments(req),
                ))
            } else {
                Ok(format!(
                    "{}payments/{connector_transaction_id}/status",
                    self.connector_base_url_payments(req),
                ))
            }
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_request: Json(DlocalRefundRequest),
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}refunds", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_response: RefundResponse,
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
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!(
                "{}refunds/{refund_id}/status",
                self.connector_base_url_refunds(req),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_request: Json(DlocalPaymentsCaptureRequest),
    curl_response: DlocalPaymentsResponse,
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
            Ok(format!("{}payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_response: DlocalPaymentsResponse,
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
            Ok(format!(
                "{}payments/{}/cancel",
                self.connector_base_url_payments(req),
                req.request.connector_transaction_id.clone(),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_request: Json(DlocalRepeatPaymentRequest),
    curl_response: DlocalRepeatPaymentResponse,
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
            Ok(format!("{}payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Dlocal,
    curl_request: Json(DlocalSetupMandateRequest),
    curl_response: DlocalSetupMandateResponse,
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
            let base_url = self.connector_base_url_payments(req);
            match &req.request.payment_method_data {
                // dLocal card tokenization uses the same /secure_payments endpoint as
                // the card authorize flow with `card.save: true` and a minimal verify
                // amount (dLocal rejects amounts <= 1.00 with code 5016 "Amount too low").
                PaymentMethodData::Card(_) => Ok(format!("{base_url}secure_payments")),
                // GCash recurring setup uses the dLocal Enrollment API directly.
                PaymentMethodData::Wallet(_) => {
                    Ok(format!("{base_url}enrollments"))
                }
                _ => Ok(format!("{base_url}payments")),
            }
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Dlocal,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SubmitEvidence,
        DefendDispute,
        PaymentMethodToken,
        ClientAuthenticationToken,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        VoidPC,
        CreateOrder,
        Accept,
        ServerSessionAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        ServerAuthenticationToken,
    ],
);
