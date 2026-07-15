pub mod transformers;

use std::{self, fmt::Debug};

use base64::{engine::general_purpose::URL_SAFE, Engine};
use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund, RepeatPayment},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, MandateReference, PaymentFlowData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::report;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as givepayments, GivepaymentsIncomingWebhookData, GivepaymentsPaymentResponseData,
    GivepaymentsPaymentResponseData as GivepaymentsRepeatPaymentResponse,
    GivepaymentsPaymentResponseData as GivepaymentsPaymentSyncResponse,
    GivepaymentsPaymentsRequestData,
    GivepaymentsPaymentsRequestData as GivepaymentsRepeatPaymentRequest,
    GivepaymentsRefundRequestData, GivepaymentsRefundResponseData,
    GivepaymentsRefundResponseData as GivepaymentsRefundSyncResponse, GivepaymentsWebhookData,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

use error_stack::ResultExt;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Givepayments<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id": "GS_EV_pmV0LOyQvHYnG1VNZD2QeE","created_at": "1661990400","type": "payment.captured","data": { "type": "payment", "object": { "id": "GS_TXN_cKP1ctmwThYaA5UJrUG67A", "id": "GS_TXN_cKP1ctmwThYaA5UJrUG67A", "created_at": 1661990400, "updated_at": 1661990400, "settled_at": null, "status": "successful", "processing_state": "captured", "total_amount": 500, "net_amount": 500, "fee_amount": 44, "fees_paid_by": "merchant", "description": "", "reversal_status": "not_reversed", "billing_descriptor": "1OFFICESUPPLIESSTORE", "risk": { "quarantine": false, "risk_level": "low", "assessment": "" }, "paymethod": { "type": "card", "card": { "id": "GS_PMC_7cmafx7A532uIiZaGRsE4D", "created_at": 1661990400, "updated_at": 1661990400, "brand": "visa", "name": "Jack Francis", "number_last4": "1111", "exp_year": 2023, "exp_month": 8, "is_debit": false, "user": "GS_USR_5z9QxI1cG1YAAZGV9nos4B", "address": null }}, "customer": "GS_CUS_2rbzrEaeBNwNMafRxKBfSb", "merchant": "GS_MER_OVC3SKymD34SH5NjEhPa8D" }}, "merchant": "GS_MER_OVC3SKymD34SH5NjEhPa8D"}"#
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::WebhookError>> {
        let signature = request
            .headers
            .get("gp-webhook-signature")
            .ok_or_else(|| report!(errors::WebhookError::WebhookSignatureNotFound))
            .attach_printable("Missing incoming webhook signature for givepayments connector")?;

        URL_SAFE
            .decode(signature)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::WebhookError>> {
        let message = std::str::from_utf8(&request.body)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
            .attach_printable(
                "Webhook source verification message parsing failed for givepayments connector",
            )?;

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
            .attach_printable("Webhook source verification failed for givepayments connector")
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let webhook_body: GivepaymentsIncomingWebhookData = request
            .body
            .parse_struct("GivepaymentsIncomingWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(webhook_body.event_type.into())
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<errors::WebhookError>,
    > {
        let webhook_body: GivepaymentsIncomingWebhookData = request
            .body
            .parse_struct("GivepaymentsIncomingWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(Box::new(webhook_body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let webhook_body: GivepaymentsIncomingWebhookData = request
            .body
            .parse_struct("GivepaymentsIncomingWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_resource_reference = match webhook_body.data {
            GivepaymentsWebhookData::Payment(response_data) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(response_data.id),
                    merchant_transaction_id: response_data.external_reference,
                })
            }
            GivepaymentsWebhookData::Refund(response_data) => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(response_data.id),
                    merchant_refund_id: response_data.external_reference,
                    connector_transaction_id: None,
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
        let webhook_body: GivepaymentsIncomingWebhookData = request
            .body
            .parse_struct("GivepaymentsIncomingWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let (resource_id, status, connector_mandate_id): (
            Option<ResponseId>,
            AttemptStatus,
            Option<String>,
        ) = match &webhook_body.data {
            GivepaymentsWebhookData::Payment(payment_data) => Ok((
                Some(ResponseId::ConnectorTransactionId(payment_data.id.clone())),
                payment_data.processing_state.clone().into(),
                payment_data
                    .paymethod_token
                    .as_ref()
                    .map(|token_data| token_data.id.clone()),
            )),

            GivepaymentsWebhookData::Refund(_) => {
                Err(errors::WebhookError::WebhookBodyDecodingFailed)
            }
        }?;

        let mandate_reference = connector_mandate_id.map(|mandate_id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(mandate_id),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });

        Ok(WebhookDetailsResponse {
            resource_id,
            status,
            connector_response_reference_id: None,
            mandate_reference,
            error_code: None,
            error_message: None,
            error_reason: None,
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

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
        let webhook_body: GivepaymentsIncomingWebhookData = request
            .body
            .parse_struct("GivepaymentsIncomingWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let (connector_refund_id, status): (Option<String>, RefundStatus) = match &webhook_body.data
        {
            GivepaymentsWebhookData::Payment(_) => {
                Err(errors::WebhookError::WebhookBodyDecodingFailed)
            }
            GivepaymentsWebhookData::Refund(refund_data) => Ok((
                Some(refund_data.id.clone()),
                refund_data.processing_state.clone().into(),
            )),
        }?;

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id,
            merchant_transaction_id: None,
            status,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Givepayments<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Givepayments<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Givepayments,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const GP_REQUESTOR_IP: &str = "GP-Requestor-IP";
    pub(crate) const GP_REQUESTOR_UA: &str = "GP-Requestor-UA";
}

macros::create_all_prerequisites!(
    connector_name: Givepayments,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GivepaymentsPaymentsRequestData<T>,
            response_body: GivepaymentsPaymentResponseData,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: GivepaymentsRepeatPaymentRequest<T>,
            response_body: GivepaymentsRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: GivepaymentsPaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GivepaymentsRefundRequestData,
            response_body: GivepaymentsRefundResponseData,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: GivepaymentsRefundSyncResponse,
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
            req.resource_common_data.connectors.givepayments.base_url.to_string()
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.givepayments.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Givepayments<T>
{
    fn id(&self) -> &'static str {
        "givepayments"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.givepayments.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = givepayments::GivepaymentsAuthType::try_from(auth_type).map_err(|_| {
            errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some("Provide AuthType as HeaderKey".to_string()),
                    doc_url: Some("https://givepayments.com/docs#authentication".to_string()),
                    additional_context: Some(
                        "Provided AuthType is incorrect. AuthType should be HeaderKey.".to_string(),
                    ),
                },
            }
        })?;
        let auth_header = vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )];
        Ok(auth_header)
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: givepayments::GivepaymentsErrorResponse = res
            .response
            .parse_struct("GivepaymentsErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "givepayments: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let error_reason = response
            .input
            .as_ref()
            .map(|inputs| {
                inputs
                    .iter()
                    .map(|input| {
                        format!(
                            "type={}, code={}, param={}, message={}",
                            input.input_error_type, input.code, input.param, input.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .or_else(|| {
                response
                    .payment
                    .as_ref()
                    .map(|payment| format!("code={}, message={}", payment.code, payment.message))
            });

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code.to_string(),
            message: response.message.clone(),
            reason: error_reason.or_else(|| Some(response.message.clone())),
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
    connector: Givepayments,
    curl_request: Json(GivepaymentsPaymentsRequestData),
    curl_response: GivepaymentsPaymentResponseData,
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
            let browser_info = req
                .request
                .browser_info
                .as_ref()
                .ok_or_else(|| errors::IntegrationError::MissingRequiredFields {
                    field_names: vec!["browser_info.ip_address", "browser_info.user_agent"],
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Include browser_info.ip_address and browser_info.user_agent fields in the request".to_string()),
                        doc_url: None,
                        additional_context: Some("GivePayments require ip_address as well as user_agent in the payments header.".to_string()),
                    }
                })?;

            let ip_address = browser_info
                .ip_address
                .ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                    field_name: "browser_info.ip_address",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Include browser_info.ip_address field in the request".to_string()),
                        doc_url: None,
                        additional_context: Some("GivePayments require ip_address in the payments header.".to_string()),
                    }
                })?;

            let user_agent = browser_info
                .user_agent
                .as_ref()
                .ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                    field_name: "browser_info.user_agent",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Include browser_info.user_agent field in the request".to_string()),
                        doc_url: None,
                        additional_context: Some("GivePayments require user_agent in the payments header.".to_string()),
                    }
                })?;

            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::GP_REQUESTOR_IP.to_string(),
                    ip_address.to_string().into(),
                ),
                (
                    headers::GP_REQUESTOR_UA.to_string(),
                    user_agent.as_str().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            Ok(header)
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
    connector: Givepayments,
    curl_request: Json(GivepaymentsRepeatPaymentRequest),
    curl_response: GivepaymentsRepeatPaymentResponse,
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
            let browser_info = req
                .request
                .browser_info
                .as_ref()
                .ok_or_else(|| errors::IntegrationError::MissingRequiredFields {
                    field_names: vec!["browser_info.ip_address", "browser_info.user_agent"],
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Include browser_info.ip_address and browser_info.user_agent fields in the request".to_string()),
                        doc_url: None,
                        additional_context: Some("GivePayments require ip_address as well as user_agent in the payments header.".to_string()),
                    }
                })?;

            let ip_address = browser_info
                .ip_address
                .ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                    field_name: "browser_info.ip_address",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Include browser_info.ip_address field in the request".to_string()),
                        doc_url: None,
                        additional_context: Some("GivePayments require ip_address in the payments header.".to_string()),
                    }
                })?;

            let user_agent = browser_info
                .user_agent
                .as_ref()
                .ok_or_else(|| errors::IntegrationError::MissingRequiredField {
                    field_name: "browser_info.user_agent",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Include browser_info.user_agent field in the request".to_string()),
                        doc_url: None,
                        additional_context: Some("GivePayments require user_agent in the payments header.".to_string()),
                    }
                })?;

            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::GP_REQUESTOR_IP.to_string(),
                    ip_address.to_string().into(),
                ),
                (
                    headers::GP_REQUESTOR_UA.to_string(),
                    user_agent.as_str().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            Ok(header)
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
    connector: Givepayments,
    curl_response: GivepaymentsPaymentSyncResponse,
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
            let payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some("connector_transaction_id is missing from the PSync request.".to_string()),
                    },
                })?;
            Ok(format!("{base_url}/payments/{payment_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Givepayments,
    curl_request: Json(GivepaymentsRefundRequest),
    curl_response: GivepaymentsRefundResponse,
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
    connector: Givepayments,
    curl_response: GivepaymentsRefundSyncResponse,
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
            let connector_refund_id = &req.request.connector_refund_id;
            if connector_refund_id.is_empty() {
                return Err(errors::IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some("connector_transaction_id is missing from the Rsync request.".to_string()),
                    },
                }
                .into());
            }
            Ok(format!(
                "{base_url}/refunds/{connector_refund_id}",
            ))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Givepayments,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Void,
        CreateOrder,
        SetupMandate,
        PaymentMethodToken,
        ServerAuthenticationToken,
        MandateRevoke,
        VoidPC,
    ],
    not_supported: [
        Capture,
        VoidPostRefund,
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
