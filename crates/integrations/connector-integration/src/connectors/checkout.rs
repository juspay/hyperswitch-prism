pub mod transformers;

use std::fmt::Debug;

use common_utils::{
    consts, crypto, errors::CustomResult, events, ext_traits::ByteSliceExt,
    types::StringMinorUnitForConnector,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PaymentMethodToken, RSync, Refund, RepeatPayment, SetupMandate,
        Void,
    },
    connector_types::{
        ConnectorWebhookSecrets, DisputeWebhookDetailsResponse, DisputeWebhookReference,
        EventContext, EventType, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::SyncRequestType,
    router_response_types::Response,
    types::Connectors,
    utils::convert_amount_for_webhook,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    is_chargeback_event, is_refund_event, CheckoutDisputeWebhookBody, CheckoutErrorResponse,
    CheckoutTokenRequest, CheckoutTokenResponse, CheckoutWebhookBody, CheckoutWebhookEventTypeBody,
    PaymentCaptureRequest, PaymentCaptureResponse, PaymentVoidRequest, PaymentVoidResponse,
    PaymentsRequest, PaymentsRequest as SetupMandateRequest,
    PaymentsRequest as RepeatPaymentRequest, PaymentsResponse, PaymentsResponse as PSyncResponse,
    PaymentsResponse as SetupMandateResponse, PaymentsResponse as RepeatPaymentResponse,
    RSyncResponse, RefundRequest, RefundResponse,
};

use super::macros;
use crate::{
    types::ResponseRouterData,
    utils::{
        get_error_code_error_message_based_on_priority, ConnectorErrorType,
        ConnectorErrorTypeMapping,
    },
    with_error_response_body,
};
use domain_types::errors::{ConnectorError, IntegrationError};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Type alias for non-generic trait implementations

macros::macro_connector_payout_implementation!(
    connector: Checkout,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Checkout<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Checkout<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"evt_dj6tpkmbhew3bnl4n7sxzrfmsi","type":"payment_captured","version":"1.0.6","created_on":"2020-08-17T14:12:59Z","data":{"id":"pay_y3oqhf46pyzuxjbcn2giaqnb44","action_id":"act_y3oqhf46pyzuxjbcn2giaqnb44","amount":999,"currency":"USD","approved":true,"status":"Captured","auth_code":"956084","response_code":"10000","response_summary":"Approved","reference":"ORD-5023-4E89","payment_id":"pay_y3oqhf46pyzuxjbcn2giaqnb44","action_type":"Capture","processed_on":"2020-08-17T14:12:59Z","processing":{"acquirer_transaction_id":"866461431360025","acquirer_reference_number":"00260971375618446482438"}},"_links":{"self":{"href":"https://api.sandbox.checkout.com/workflows/events/evt_dj6tpkmbhew3bnl4n7sxzrfmsi"}}}"#
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<domain_types::errors::WebhookError>> {
        let signature = request
            .headers
            .get("cko-signature")
            .ok_or_else(|| {
                error_stack::report!(domain_types::errors::WebhookError::WebhookSignatureNotFound)
            })
            .attach_printable("Missing cko-signature header in Checkout webhook")?;

        hex::decode(signature)
            .change_context(domain_types::errors::WebhookError::WebhookSignatureNotFound)
            .attach_printable("Failed to hex-decode cko-signature header")
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<domain_types::errors::WebhookError>> {
        Ok(request.body.clone())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<domain_types::errors::WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret.ok_or_else(|| {
            error_stack::report!(
                domain_types::errors::WebhookError::WebhookVerificationSecretNotFound
            )
        })?;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        use common_utils::crypto::VerifySignature;
        crypto::HmacSha256
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(domain_types::errors::WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<domain_types::errors::WebhookError>> {
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }

        let body: CheckoutWebhookEventTypeBody = request
            .body
            .parse_struct("CheckoutWebhookEventTypeBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(EventType::from(body.transaction_type))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<
        Option<WebhookResourceReference>,
        error_stack::Report<domain_types::errors::WebhookError>,
    > {
        if request.body.is_empty() {
            return Ok(None);
        }

        // Peek at only the event type first to branch on the body shape.
        let type_body: CheckoutWebhookEventTypeBody = request
            .body
            .parse_struct("CheckoutWebhookEventTypeBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        if is_chargeback_event(&type_body.transaction_type) {
            let body: CheckoutDisputeWebhookBody = request
                .body
                .parse_struct("CheckoutDisputeWebhookBody")
                .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

            return Ok(Some(WebhookResourceReference::Dispute(
                DisputeWebhookReference {
                    connector_dispute_id: Some(body.data.id),
                    connector_transaction_id: body.data.payment_id,
                },
            )));
        }

        let body: CheckoutWebhookBody = request
            .body
            .parse_struct("CheckoutWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        if is_refund_event(&type_body.transaction_type) {
            return Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    // Checkout uses action_id as the refund identifier
                    connector_refund_id: body.data.action_id,
                    // The `reference` field echoes the merchant-assigned refund ID
                    merchant_refund_id: body.data.reference,
                    connector_transaction_id: body.data.payment_id,
                    merchant_transaction_id: None,
                },
            )));
        }

        // Payment event — the canonical payment ID is `payment_id`; `id` may be an action ID
        let connector_transaction_id = body.data.payment_id.or(Some(body.data.id));
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id,
                merchant_transaction_id: body.data.reference,
            },
        )))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<domain_types::errors::WebhookError>>
    {
        let body: CheckoutWebhookBody = request
            .body
            .parse_struct("CheckoutWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = common_enums::AttemptStatus::try_from(body.transaction_type)
            .change_context(domain_types::errors::WebhookError::WebhookEventTypeNotFound)
            .attach_printable(
                "Checkout payment webhook: unrecognized event type for payment status mapping",
            )?;

        let resource_id = Some(ResponseId::ConnectorTransactionId(
            body.data.payment_id.unwrap_or(body.data.id),
        ));

        let (error_code, error_message, error_reason) = if status
            == common_enums::AttemptStatus::Failure
            || status == common_enums::AttemptStatus::AuthenticationFailed
        {
            (
                body.data.response_code,
                body.data.response_summary.clone(),
                body.data.response_summary,
            )
        } else {
            (None, None, None)
        };

        Ok(WebhookDetailsResponse {
            connector_returned_payment_method_details: None,
            resource_id,
            status,
            connector_response_reference_id: body.data.reference,
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
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
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<domain_types::errors::WebhookError>>
    {
        let body: CheckoutWebhookBody = request
            .body
            .parse_struct("CheckoutWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        let status = common_enums::RefundStatus::try_from(body.transaction_type)
            .change_context(domain_types::errors::WebhookError::WebhookEventTypeNotFound)
            .attach_printable(
                "Checkout refund webhook: unrecognized event type for refund status mapping",
            )?;

        Ok(RefundWebhookDetailsResponse {
            // action_id is the refund's PSP identifier
            connector_refund_id: body.data.action_id,
            merchant_transaction_id: None,
            status,
            connector_response_reference_id: body.data.reference,
            error_code: body.data.response_code,
            error_message: body.data.response_summary,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        DisputeWebhookDetailsResponse,
        error_stack::Report<domain_types::errors::WebhookError>,
    > {
        let body: CheckoutDisputeWebhookBody = request
            .body
            .parse_struct("CheckoutDisputeWebhookBody")
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

        let (status, stage) =
            <(common_enums::DisputeStatus, common_enums::DisputeStage)>::try_from(
                body.transaction_type,
            )?;

        let currency = body.data.currency.ok_or_else(|| {
            error_stack::report!(
                domain_types::errors::WebhookError::WebhookMissingRequiredField {
                    field: "data.currency",
                }
            )
        })?;

        let amount = convert_amount_for_webhook(
            &StringMinorUnitForConnector,
            body.data.amount.unwrap_or_default(),
            currency,
        )?;

        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency,
            dispute_id: body.data.id,
            status,
            stage,
            connector_response_reference_id: body.data.payment_id,
            dispute_message: None,
            connector_reason_code: body.data.reason_code,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Checkout<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Checkout<T>
{
}
macros::create_all_prerequisites!(
    connector_name: Checkout,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PaymentsRequest<T>,
            response_body: PaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PaymentCaptureRequest,
            response_body: PaymentCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PaymentVoidRequest,
            response_body: PaymentVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: SetupMandateRequest<T>,
            response_body: SetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: RepeatPaymentRequest<T>,
            response_body: RepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: CheckoutTokenRequest,
            response_body: CheckoutTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        /// Headers for Checkout's `POST /tokens`.
        ///
        /// Tokenization is the one Checkout endpoint that is authenticated with the account's
        /// **public** key, which — counter-intuitively — lives in `api_key`, while the secret
        /// key every other endpoint uses lives in `api_secret`. See the note on
        /// [`transformers::CheckoutAuthType`]. Verified against the sandbox: `Bearer sk_...`
        /// on `/tokens` returns 403, and `Bearer pk_...` on `/payments` returns 401. This is
        /// why the flow cannot reuse [`Self::build_headers`].
        pub fn build_tokenization_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = transformers::CheckoutAuthType::try_from(&req.connector_config)?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", auth.api_key.peek()).into_masked(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.checkout.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.checkout.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Checkout<T>
{
    fn id(&self) -> &'static str {
        "checkout"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::CheckoutAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        // `api_secret` holds the **secret** key (`sk_...`) — used by every endpoint except
        // `POST /tokens`, which takes the public key from `api_key`. See the note on
        // [`transformers::CheckoutAuthType`]; the naming is inherited, not descriptive.
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_secret.peek()).into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.checkout.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        if res.response.is_empty() {
            let (error_codes, error_type) = if res.status_code == 401 {
                (
                    Some(vec!["Invalid api key".to_string()]),
                    Some("invalid_api_key".to_string()),
                )
            } else {
                (None, None)
            };
            let response = CheckoutErrorResponse {
                request_id: None,
                error_codes: error_codes.clone(),
                error_type: error_type.clone(),
            };
            with_error_response_body!(event_builder, response);
            return Ok(ErrorResponse {
                status_code: res.status_code,
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: error_codes.map(|errors| errors.join(" & ")).or(error_type),
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            });
        }

        let response: Result<
            CheckoutErrorResponse,
            error_stack::Report<common_utils::errors::ParsingError>,
        > = res.response.parse_struct("ErrorResponse");

        match response {
            Ok(response) => {
                with_error_response_body!(event_builder, response);

                let errors_list = response.error_codes.clone().unwrap_or_default();
                let option_error_code_message = get_error_code_error_message_based_on_priority(
                    self.clone(),
                    errors_list
                        .into_iter()
                        .map(|errors| errors.into())
                        .collect(),
                );
                let typed = macros::serialize_typed_connector_payload(
                    &response,
                    "typed_connector_response",
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: option_error_code_message
                        .clone()
                        .map(|error_code_message| error_code_message.error_code)
                        .unwrap_or(consts::NO_ERROR_CODE.to_string()),
                    message: option_error_code_message
                        .map(|error_code_message| error_code_message.error_message)
                        .unwrap_or(consts::NO_ERROR_MESSAGE.to_string()),
                    reason: response
                        .error_codes
                        .map(|errors| errors.join(" & "))
                        .or(response.error_type),
                    attempt_status: None,
                    connector_transaction_id: response.request_id,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Err(error_msg) => {
                tracing::error!(deserialization_error =? error_msg);
                crate::utils::handle_json_response_deserialization_failure(res, "checkout")
            }
        }
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_request: Json(PaymentsRequest),
    curl_response: PaymentsResponse,
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
            Ok(format!("{}payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_request: Json(RepeatPaymentRequest),
    curl_response: PaymentsResponse,
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
    connector: Checkout,
    curl_response: PSyncResponse,
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
            let suffix = match req.request.sync_type {
                SyncRequestType::MultipleCaptureSync => "/actions",
                SyncRequestType::SinglePaymentSync => ""
};
            Ok(format!(
                "{}{}{}{}",
                self.connector_base_url_payments(req),
                "payments/",
                req.request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?,
                suffix
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_request: Json(PaymentCaptureRequest),
    curl_response: PaymentCaptureResponse,
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
            let connector_tx_id = match &req.request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) => id.clone(),
                _ => return Err(IntegrationError::MissingConnectorTransactionID { context: Default::default() }.into())
};
            Ok(format!("{}payments/{}/captures", self.connector_base_url_payments(req), connector_tx_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_request: Json(RefundRequest),
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
            let connector_tx_id = &req.request.connector_transaction_id;
            Ok(format!("{}payments/{}/refunds", self.connector_base_url_refunds(req), connector_tx_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_response: RSyncResponse,
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
            let connector_tx_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}payments/{}/actions",
                self.connector_base_url_refunds(req),
                connector_tx_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_request: Json(PaymentVoidRequest),
    curl_response: PaymentVoidResponse,
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
            let connector_tx_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}payments/{}/voids", self.connector_base_url_payments(req), connector_tx_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_request: Json(SetupMandateRequest),
    curl_response: SetupMandateResponse,
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
            Ok(format!("{}payments", self.connector_base_url_payments(req)))
        }
    }
);

// Connector-decryption head: hand Checkout the raw Apple Pay / Google Pay payload and get a
// single-use `tok_...` back. The Authorize / SetupMandate tail then spends that token as
// `source.type = "token"` (see the `WalletSource` `From` impl in transformers.rs).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Checkout,
    curl_request: Json(CheckoutTokenRequest),
    curl_response: CheckoutTokenResponse,
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
            self.build_tokenization_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}tokens", self.connector_base_url_payments(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorErrorTypeMapping for Checkout<T>
{
    fn get_connector_error_type(
        &self,
        error_code: String,
        _error_message: String,
    ) -> ConnectorErrorType {
        match error_code.as_str() {
            "action_failure_limit_exceeded" => ConnectorErrorType::BusinessError,
            "address_invalid" => ConnectorErrorType::UserError,
            "amount_exceeds_balance" => ConnectorErrorType::BusinessError,
            "amount_invalid" => ConnectorErrorType::UserError,
            "api_calls_quota_exceeded" => ConnectorErrorType::TechnicalError,
            "billing_descriptor_city_invalid" => ConnectorErrorType::UserError,
            "billing_descriptor_city_required" => ConnectorErrorType::UserError,
            "billing_descriptor_name_invalid" => ConnectorErrorType::UserError,
            "billing_descriptor_name_required" => ConnectorErrorType::UserError,
            "business_invalid" => ConnectorErrorType::BusinessError,
            "business_settings_missing" => ConnectorErrorType::BusinessError,
            "capture_value_greater_than_authorized" => ConnectorErrorType::BusinessError,
            "capture_value_greater_than_remaining_authorized" => ConnectorErrorType::BusinessError,
            "card_authorization_failed" => ConnectorErrorType::UserError,
            "card_disabled" => ConnectorErrorType::UserError,
            "card_expired" => ConnectorErrorType::UserError,
            "card_expiry_month_invalid" => ConnectorErrorType::UserError,
            "card_expiry_month_required" => ConnectorErrorType::UserError,
            "card_expiry_year_invalid" => ConnectorErrorType::UserError,
            "card_expiry_year_required" => ConnectorErrorType::UserError,
            "card_holder_invalid" => ConnectorErrorType::UserError,
            "card_not_found" => ConnectorErrorType::UserError,
            "card_number_invalid" => ConnectorErrorType::UserError,
            "card_number_required" => ConnectorErrorType::UserError,
            "channel_details_invalid" => ConnectorErrorType::BusinessError,
            "channel_url_missing" => ConnectorErrorType::BusinessError,
            "charge_details_invalid" => ConnectorErrorType::BusinessError,
            "city_invalid" => ConnectorErrorType::BusinessError,
            "country_address_invalid" => ConnectorErrorType::UserError,
            "country_invalid" => ConnectorErrorType::UserError,
            "country_phone_code_invalid" => ConnectorErrorType::UserError,
            "country_phone_code_length_invalid" => ConnectorErrorType::UserError,
            "currency_invalid" => ConnectorErrorType::UserError,
            "currency_required" => ConnectorErrorType::UserError,
            "customer_already_exists" => ConnectorErrorType::BusinessError,
            "customer_email_invalid" => ConnectorErrorType::UserError,
            "customer_id_invalid" => ConnectorErrorType::BusinessError,
            "customer_not_found" => ConnectorErrorType::BusinessError,
            "customer_number_invalid" => ConnectorErrorType::UserError,
            "customer_plan_edit_failed" => ConnectorErrorType::BusinessError,
            "customer_plan_id_invalid" => ConnectorErrorType::BusinessError,
            "cvv_invalid" => ConnectorErrorType::UserError,
            "email_in_use" => ConnectorErrorType::BusinessError,
            "email_invalid" => ConnectorErrorType::UserError,
            "email_required" => ConnectorErrorType::UserError,
            "endpoint_invalid" => ConnectorErrorType::TechnicalError,
            "expiry_date_format_invalid" => ConnectorErrorType::UserError,
            "fail_url_invalid" => ConnectorErrorType::TechnicalError,
            "first_name_required" => ConnectorErrorType::UserError,
            "last_name_required" => ConnectorErrorType::UserError,
            "ip_address_invalid" => ConnectorErrorType::UserError,
            "issuer_network_unavailable" => ConnectorErrorType::TechnicalError,
            "metadata_key_invalid" => ConnectorErrorType::BusinessError,
            "parameter_invalid" => ConnectorErrorType::UserError,
            "password_invalid" => ConnectorErrorType::UserError,
            "payment_expired" => ConnectorErrorType::BusinessError,
            "payment_invalid" => ConnectorErrorType::BusinessError,
            "payment_method_invalid" => ConnectorErrorType::UserError,
            "payment_source_required" => ConnectorErrorType::UserError,
            "payment_type_invalid" => ConnectorErrorType::UserError,
            "phone_number_invalid" => ConnectorErrorType::UserError,
            "phone_number_length_invalid" => ConnectorErrorType::UserError,
            "previous_payment_id_invalid" => ConnectorErrorType::BusinessError,
            "recipient_account_number_invalid" => ConnectorErrorType::BusinessError,
            "recipient_account_number_required" => ConnectorErrorType::UserError,
            "recipient_dob_required" => ConnectorErrorType::UserError,
            "recipient_last_name_required" => ConnectorErrorType::UserError,
            "recipient_zip_invalid" => ConnectorErrorType::UserError,
            "recipient_zip_required" => ConnectorErrorType::UserError,
            "recurring_plan_exists" => ConnectorErrorType::BusinessError,
            "recurring_plan_not_exist" => ConnectorErrorType::BusinessError,
            "recurring_plan_removal_failed" => ConnectorErrorType::BusinessError,
            "request_invalid" => ConnectorErrorType::UserError,
            "request_json_invalid" => ConnectorErrorType::UserError,
            "risk_enabled_required" => ConnectorErrorType::BusinessError,
            "server_api_not_allowed" => ConnectorErrorType::TechnicalError,
            "source_email_invalid" => ConnectorErrorType::UserError,
            "source_email_required" => ConnectorErrorType::UserError,
            "source_id_invalid" => ConnectorErrorType::BusinessError,
            "source_id_or_email_required" => ConnectorErrorType::UserError,
            "source_id_required" => ConnectorErrorType::UserError,
            "source_id_unknown" => ConnectorErrorType::BusinessError,
            "source_invalid" => ConnectorErrorType::BusinessError,
            "source_or_destination_required" => ConnectorErrorType::BusinessError,
            "source_token_invalid" => ConnectorErrorType::BusinessError,
            "source_token_required" => ConnectorErrorType::UserError,
            "source_token_type_required" => ConnectorErrorType::UserError,
            "source_token_type_invalid" => ConnectorErrorType::BusinessError,
            "source_type_required" => ConnectorErrorType::UserError,
            "sub_entities_count_invalid" => ConnectorErrorType::BusinessError,
            "success_url_invalid" => ConnectorErrorType::BusinessError,
            "3ds_malfunction" => ConnectorErrorType::TechnicalError,
            "3ds_not_configured" => ConnectorErrorType::BusinessError,
            "3ds_not_enabled_for_card" => ConnectorErrorType::BusinessError,
            "3ds_not_supported" => ConnectorErrorType::BusinessError,
            "3ds_payment_required" => ConnectorErrorType::BusinessError,
            "token_expired" => ConnectorErrorType::BusinessError,
            "token_in_use" => ConnectorErrorType::BusinessError,
            "token_invalid" => ConnectorErrorType::BusinessError,
            "token_required" => ConnectorErrorType::UserError,
            "token_type_required" => ConnectorErrorType::UserError,
            "token_used" => ConnectorErrorType::BusinessError,
            "void_amount_invalid" => ConnectorErrorType::BusinessError,
            "wallet_id_invalid" => ConnectorErrorType::BusinessError,
            "zip_invalid" => ConnectorErrorType::UserError,
            "processing_key_required" => ConnectorErrorType::BusinessError,
            "processing_value_required" => ConnectorErrorType::BusinessError,
            "3ds_version_invalid" => ConnectorErrorType::BusinessError,
            "3ds_version_not_supported" => ConnectorErrorType::BusinessError,
            "processing_error" => ConnectorErrorType::TechnicalError,
            "service_unavailable" => ConnectorErrorType::TechnicalError,
            "token_type_invalid" => ConnectorErrorType::UserError,
            "token_data_invalid" => ConnectorErrorType::UserError,
            _ => ConnectorErrorType::UnknownError,
        }
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Checkout,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        Accept,
        SubmitEvidence,
        DefendDispute,
        CreateOrder,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
    ],
    not_supported: [
        VoidPostRefund,
        MandateRevoke,
        VoidPC,
    ],
);
