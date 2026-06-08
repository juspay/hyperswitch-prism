pub mod transformers;

use std::{self, fmt::Debug};

use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, Void},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundWebhookReference, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, ResponseId, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::report;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as imerchantsolutions, ForeignTryFrom, ImerchantsolutionsCaptureRequestData,
    ImerchantsolutionsCaptureResponseData, ImerchantsolutionsPaymentSyncResponse,
    ImerchantsolutionsPaymentsRequestData,
    ImerchantsolutionsPaymentsRequestData as ImerchantsolutionsRepeatPaymentRequest,
    ImerchantsolutionsPaymentsResponseData,
    ImerchantsolutionsPaymentsResponseData as ImerchantsolutionsRepeatPaymentResponse,
    ImerchantsolutionsRefundRequestData, ImerchantsolutionsRefundResponseData,
    ImerchantsolutionsRefundSyncResponse, ImerchantsolutionsVoidRequestData,
    ImerchantsolutionsVoidResponseData, ImerchantsolutionsWebhookData,
    ImerchantsolutionsWebhookEventType,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

use error_stack::ResultExt;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Imerchantsolutions<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"type": "payment.completed","paymentId": "cmml1234abcd","pspReference": "ABC123DEF456","reference": "order-12345","amount": 5000,"currency": "USD","status": "captured","processor": "Adyen","cardLast4": "1111","cardBrand": "visa","customerEmail": "customer@example.com","partnerId": "your_partner_id","merchantId": "merchant_id","timestamp": "2026-03-30T15:45:00.000Z"}}}"#
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::WebhookError>> {
        let signature = request
            .headers
            .get("x-webhook-signature")
            .ok_or_else(|| report!(errors::WebhookError::WebhookSignatureNotFound))
            .attach_printable(
                "Missing incoming webhook signature for imerchantsolutions connector",
            )?;

        hex::decode(signature).change_context(errors::WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::WebhookError>> {
        let message = std::str::from_utf8(&request.body)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification message parsing failed for imerchantsolutions connector")?;

        Ok(message.to_string().into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        let algorithm = crypto::HmacSha256;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => {
                return Err(error_stack::report!(
                    errors::WebhookError::WebhookVerificationSecretNotFound
                ));
            }
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification failed for imerchantsolutions connector")
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let webhook_body: ImerchantsolutionsWebhookData = request
            .body
            .parse_struct("ImerchantsolutionsWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        EventType::foreign_try_from((webhook_body.event_type, webhook_body.status))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<errors::WebhookError>,
    > {
        let webhook_body: ImerchantsolutionsWebhookData = request
            .body
            .parse_struct("ImerchantsolutionsWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(Box::new(webhook_body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let webhook_body: ImerchantsolutionsWebhookData = request
            .body
            .parse_struct("ImerchantsolutionsWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_resource_reference = match webhook_body.event_type {
            ImerchantsolutionsWebhookEventType::PaymentCompleted
            | ImerchantsolutionsWebhookEventType::PaymentCancelled
            | ImerchantsolutionsWebhookEventType::PaymentFailed => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(webhook_body.psp_reference),
                    merchant_transaction_id: webhook_body.merchant_reference,
                })
            }
            ImerchantsolutionsWebhookEventType::PaymentRefunded => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(webhook_body.psp_reference.clone()),
                    merchant_refund_id: Some(webhook_body.psp_reference),
                    connector_transaction_id: webhook_body.original_reference,
                })
            }
        };

        Ok(Some(webhook_resource_reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let webhook_body: ImerchantsolutionsWebhookData = request
            .body
            .parse_struct("ImerchantsolutionsWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status: AttemptStatus = webhook_body.status.into();

        let (error_code, error_message, error_reason) = if status == AttemptStatus::Failure {
            (None, webhook_body.error, webhook_body.reason)
        } else {
            (None, None, None)
        };

        let minor_amount_captured = match status {
            AttemptStatus::Charged => webhook_body.amount,
            AttemptStatus::PartialCharged => webhook_body.total_captured,
            _ => None,
        };

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                webhook_body.psp_reference,
            )),
            status,
            connector_response_reference_id: Some(webhook_body.payment_id),
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: minor_amount_captured
                .map(|minor_amount| minor_amount.get_amount_as_i64()),
            minor_amount_captured,
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
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let webhook_body: ImerchantsolutionsWebhookData = request
            .body
            .parse_struct("ImerchantsolutionsWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = RefundStatus::try_from(webhook_body.status)?;

        let (error_code, error_message) = if status == RefundStatus::Failure {
            (webhook_body.error.clone(), webhook_body.error)
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(webhook_body.psp_reference.clone()),
            status,
            connector_response_reference_id: Some(webhook_body.psp_reference),
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Imerchantsolutions<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Imerchantsolutions<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Imerchantsolutions,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_API_KEY: &str = "X-Api-Key";
    pub(crate) const X_MERCHANT_ID: &str = "X-Merchant-Id";
}

macros::create_all_prerequisites!(
    connector_name: Imerchantsolutions,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ImerchantsolutionsPaymentsRequestData<T>,
            response_body: ImerchantsolutionsPaymentsResponseData,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: ImerchantsolutionsRepeatPaymentRequest<T>,
            response_body: ImerchantsolutionsRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: ImerchantsolutionsCaptureRequestData,
            response_body: ImerchantsolutionsCaptureResponseData,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: ImerchantsolutionsPaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: ImerchantsolutionsVoidRequestData,
            response_body: ImerchantsolutionsVoidResponseData,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: ImerchantsolutionsRefundRequestData,
            response_body: ImerchantsolutionsRefundResponseData,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: ImerchantsolutionsRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            Ok(header)
        }

        pub fn connector_base_url_payments<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.imerchantsolutions.base_url.to_string()
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.imerchantsolutions.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Imerchantsolutions<T>
{
    fn id(&self) -> &'static str {
        "imerchantsolutions"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.imerchantsolutions.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth =
            imerchantsolutions::ImerchantsolutionsAuthType::try_from(auth_type).map_err(|_| {
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Provide AuthType as HeaderKey".to_string()),
                        doc_url: Some(
                            "https://imerchantsolutions.com/docs#authentication".to_string(),
                        ),
                        additional_context: Some(
                            "Provided AuthType is incorrect. AuthType should be HeaderKey."
                                .to_string(),
                        ),
                    },
                }
            })?;
        let mut auth_header = vec![(headers::X_API_KEY.to_string(), auth.api_key.into_masked())];
        if let Some(merchant_id) = auth.merchant_id {
            auth_header.push((
                headers::X_MERCHANT_ID.to_string(),
                merchant_id.into_masked(),
            ));
        }
        Ok(auth_header)
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: imerchantsolutions::ImerchantsolutionsErrorResponse = res
            .response
            .parse_struct("ImerchantsolutionsErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "imerchantsolutions: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code.unwrap_or(NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason: response.message,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Imerchantsolutions,
    curl_request: Json(ImerchantsolutionsPaymentsRequestData),
    curl_response: ImerchantsolutionsPaymentsResponseData,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/payments"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Imerchantsolutions,
    curl_request: Json(ImerchantsolutionsRepeatPaymentRequest),
    curl_response: ImerchantsolutionsRepeatPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/payments"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Imerchantsolutions,
    curl_response: ImerchantsolutionsPaymentSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let psp_reference = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: Some("https://imerchantsolutions.com/docs/api#get--payments-capture".to_string()),
                        additional_context: Some("connector_transaction_id is missing from the PSync request.".to_string()),
                    },
                })?;
            Ok(format!("{base_url}/payments/capture?pspReference={psp_reference}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Imerchantsolutions,
    curl_request: Json(ImerchantsolutionsVoidRequestData),
    curl_response: ImerchantsolutionsVoidResponseData,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}/payments/cancel",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Imerchantsolutions,
    curl_request: Json(ImerchantsolutionsCaptureRequestData),
    curl_response: ImerchantsolutionsCaptureResponseData,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!(
                "{}/payments/capture",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Imerchantsolutions,
    curl_request: Json(ImerchantsolutionsRefundRequest),
    curl_response: ImerchantsolutionsRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!(
                "{base_url}/refunds",
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Imerchantsolutions,
    curl_response: ImerchantsolutionsRefundSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            let transaction_id = &req.request.connector_transaction_id;
            if transaction_id.is_empty() {
                return Err(errors::IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: Some("https://imerchantsolutions.com/docs/api#get--refunds".to_string()),
                        additional_context: Some("connector_transaction_id is missing from the Rsync request.".to_string()),
                    },
                }
                .into());
            }
            Ok(format!(
                "{base_url}/refunds?pspReference={transaction_id}",
            ))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Imerchantsolutions,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateOrder,
        SetupMandate,
        PaymentMethodToken,
        ServerAuthenticationToken,
        MandateRevoke,
        VoidPC,
    ],
    not_supported: [
        IncrementalAuthorization,
        ClientAuthenticationToken,
        SubmitEvidence,
        DefendDispute,
        Accept,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
    ],
);
