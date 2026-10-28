use base64::Engine;
use common_utils::{
    consts,
    crypto::VerifySignature,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::{FloatMajorUnitForConnector, StringMajorUnit, StringMinorUnitForConnector},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateOrder, PSync, RSync, Refund, RepeatPayment,
        ServerAuthenticationToken, SetupMandate, Void,
    },
    connector_types::{
        ConnectorSpecifications, ConnectorWebhookSecrets, EventContext, EventType,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundWebhookReference, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, SetupMandateRequestData, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, Report, ResultExt};
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};

use serde::Serialize;
use std::fmt::Debug;
pub mod transformers;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use transformers::{
    self as trustpay, RefundResponse, RefundResponse as RefundSyncResponse,
    TrustpayAuthUpdateRequest, TrustpayAuthUpdateResponse, TrustpayCaptureRequest,
    TrustpayCaptureResponse, TrustpayCreateIntentRequest, TrustpayCreateIntentResponse,
    TrustpayErrorResponse, TrustpayPaymentsRequest,
    TrustpayPaymentsResponse as TrustpayPaymentsSyncResponse, TrustpayPaymentsResponse,
    TrustpayRefundRequest, TrustpayRepeatPaymentRequest, TrustpayRepeatPaymentResponse,
    TrustpaySetupMandateRequest, TrustpaySetupMandateResponse, TrustpayVoidRequest,
    TrustpayVoidResponse,
};

use super::macros::{self, ContentTypeSelector};
use crate::types::ResponseRouterData;
use crate::utils::{self, ConnectorErrorType, ConnectorErrorTypeMapping};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};

macros::create_amount_converter_wrapper!(connector_name: Trustpay, amount_type: StringMajorUnit);

// Local headers module
mod headers {
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const AUTHORIZATION: &str = "Authorization";
    pub const X_API_KEY: &str = "X-Api-Key";
}

macros::macro_connector_payout_implementation!(
    connector: Trustpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Trustpay<T>
{
    fn should_do_access_token(&self, payment_method: Option<common_enums::PaymentMethod>) -> bool {
        matches!(
            payment_method,
            Some(
                common_enums::PaymentMethod::BankRedirect
                    | common_enums::PaymentMethod::BankTransfer
            )
        )
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Trustpay<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let webhook_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        hex::decode(webhook_response.signature)
            .change_context(WebhookError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let trustpay_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let response: serde_json::Value = request
            .body
            .parse_struct("Webhook Value")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let values = utils::collect_and_sort_values_by_removing_signature(
            &response,
            &trustpay_response.signature,
        );
        let payload = values.join("/");
        Ok(payload.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => return Ok(false),
        };

        let algorithm = common_utils::crypto::HmacSha256;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"PaymentInformation":{"CreditDebitIndicator":"CRDT","References":{"MerchantReference":"probe_txn_001","PaymentRequestId":"72750"},"Status":"Paid","Amount":{"Amount":10.00,"Currency":"EUR"}},"Signature":"probe_sig"}"#
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let webhook_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(trustpay::get_event_type_from_webhook(
            &webhook_response.payment_information.credit_debit_indicator,
            &webhook_response.payment_information.status,
        ))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let webhook_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let payment_information = webhook_response.payment_information;
        let merchant_reference = payment_information
            .references
            .merchant_reference
            .ok_or_else(|| report!(WebhookError::WebhookReferenceIdNotFound))?;

        let reference = match (
            payment_information.credit_debit_indicator,
            payment_information.status,
        ) {
            (trustpay::CreditDebitIndicator::Crdt, _)
            | (trustpay::CreditDebitIndicator::Dbit, trustpay::WebhookStatus::Chargebacked) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: None,
                    merchant_transaction_id: Some(merchant_reference),
                    connector_preprocessing_id: None,
                })
            }
            (trustpay::CreditDebitIndicator::Dbit, _) => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: payment_information.references.payment_id,
                    merchant_refund_id: Some(merchant_reference),
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                })
            }
        };
        Ok(Some(reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let webhook_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let (status, error, _payment_response_data) =
            trustpay::handle_webhook_response_incoming_webhook(
                webhook_response.payment_information.clone(),
                200,
            )?;

        let (error_code, error_message, error_reason): (
            Option<String>,
            Option<String>,
            Option<String>,
        ) = if status == common_enums::AttemptStatus::Failure {
            (
                error.as_ref().map(|e| e.code.clone()),
                error.as_ref().map(|e| e.message.clone()),
                error.as_ref().and_then(|e| e.reason.clone()),
            )
        } else {
            (None, None, None)
        };

        Ok(WebhookDetailsResponse {
            connector_returned_payment_method_details: None,
            resource_id: webhook_response
                .payment_information
                .references
                .merchant_reference
                .clone()
                .map(ResponseId::ConnectorTransactionId),
            status,
            connector_response_reference_id: webhook_response
                .payment_information
                .references
                .payment_request_id,
            connector_request_reference_id: webhook_response
                .payment_information
                .references
                .merchant_reference,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
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
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let webhook_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let (error, refund_response_data) =
            trustpay::handle_webhooks_refund_response_incoming_webhook(
                webhook_response.payment_information,
                200,
            )?;

        let (error_code, error_message): (Option<String>, Option<String>) =
            if refund_response_data.refund_status == common_enums::RefundStatus::Failure {
                (
                    error.as_ref().map(|e| e.code.clone()),
                    error.as_ref().map(|e| e.message.clone()),
                )
            } else {
                (None, None)
            };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(refund_response_data.connector_refund_id.clone()),
            merchant_transaction_id: None,
            status: refund_response_data.refund_status,
            connector_response_reference_id: Some(refund_response_data.connector_refund_id),
            error_code,
            error_message,
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
    ) -> Result<domain_types::connector_types::DisputeWebhookDetailsResponse, Report<WebhookError>>
    {
        let webhook_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let payment_info = webhook_response.payment_information;
        let reason_info = payment_info.status_reason_information.unwrap_or_default();

        let connector_dispute_id = payment_info
            .references
            .payment_id
            .ok_or_else(|| report!(WebhookError::WebhookReferenceIdNotFound))?;

        let minor_units = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
            &FloatMajorUnitForConnector,
            payment_info.amount.amount,
            payment_info.amount.currency,
        )?;
        let amount = domain_types::utils::convert_amount_for_webhook(
            &StringMinorUnitForConnector,
            minor_units,
            payment_info.amount.currency,
        )?;

        Ok(
            domain_types::connector_types::DisputeWebhookDetailsResponse {
                amount,
                currency: payment_info.amount.currency,
                dispute_id: connector_dispute_id.clone(),
                status: common_enums::enums::DisputeStatus::DisputeLost,
                stage: common_enums::enums::DisputeStage::Dispute,
                connector_response_reference_id: Some(connector_dispute_id),
                dispute_message: reason_info.reason.reject_reason,
                raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
                status_code: 200,
                response_headers: None,
                connector_reason_code: reason_info.reason.code,
                additional_details: None,
            },
        )
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let webhook_response: trustpay::TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let resource: Box<dyn hyperswitch_masking::ErasedMaskSerialize> =
            Box::new(webhook_response.payment_information);
        Ok(resource)
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Trustpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Trustpay<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Trustpay,
    generic_type: T,
    api: [
        (
            flow: PSync,
            response_body: TrustpayPaymentsSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: ServerAuthenticationToken,
            request_body: TrustpayAuthUpdateRequest,
            response_body: TrustpayAuthUpdateResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: TrustpayCreateIntentRequest,
            response_body: TrustpayCreateIntentResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: TrustpayPaymentsRequest<T>,
            response_body: TrustpayPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: TrustpayCaptureRequest,
            response_body: TrustpayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: TrustpayVoidRequest,
            response_body: TrustpayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TrustpayRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: TrustpaySetupMandateRequest<T>,
            response_body: TrustpaySetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: TrustpayRepeatPaymentRequest<T>,
            response_body: TrustpayRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {

        pub fn build_headers_for_payments<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
            self.build_headers_for_flavour(
                req,
                matches!(
                    req.resource_common_data.payment_method,
                    common_enums::PaymentMethod::BankRedirect
                        | common_enums::PaymentMethod::BankTransfer
                ),
            )
        }

        pub fn build_headers_for_flavour<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
            is_bank: bool,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
        match is_bank {
            true => {
                let token = req
                    .resource_common_data
                    .get_access_token()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "access_token",
                context: Default::default()
                    })?;
                Ok(vec![
                    (
                        headers::CONTENT_TYPE.to_string(),
                        "application/json".to_owned().into(),
                    ),
                    (
                        headers::AUTHORIZATION.to_string(),
                        format!("Bearer {token}").into_masked(),
                    ),
                ])
            }
            false => {
                let mut header = vec![(
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                )];
                let mut api_key = self.get_auth_header(&req.connector_config)?;
                header.append(&mut api_key);
                Ok(header)
            }
        }
        }

        pub fn build_headers_for_refunds<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, RefundFlowData, Req, Res>,
        {
            match req.resource_common_data.payment_method {
            Some(common_enums::PaymentMethod::BankRedirect) | Some(common_enums::PaymentMethod::BankTransfer) => {
                let token = req
                    .resource_common_data
                    .get_access_token()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "access_token",
                context: Default::default()
                    })?;
                Ok(vec![
                    (
                        headers::CONTENT_TYPE.to_string(),
                        "application/json".to_owned().into(),
                    ),
                    (
                        headers::AUTHORIZATION.to_string(),
                        format!("Bearer {token}").into_masked(),
                    ),
                ])
            }
            _ => {
                let mut header = vec![(
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                )];
                let mut api_key = self.get_auth_header(&req.connector_config)?;
                header.append(&mut api_key);
                Ok(header)
            }
        }
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.trustpay.base_url
        }

        pub fn connector_base_url_bank_redirects_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.trustpay.base_url_bank_redirects
        }

        pub fn connector_base_url_bank_redirects_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.trustpay.base_url_bank_redirects
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.trustpay.base_url
        }

        pub fn get_auth_header(
            &self,
            auth_type: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = trustpay::TrustpayAuthType::try_from(auth_type)
            .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
        Ok(vec![(
            headers::X_API_KEY.to_string(),
            auth.api_key.into_masked(),
        )])
        }

        /// Card gateway error body (`errors[]`, `paymentStatus`, `paymentStatusDetails`) with
        /// the flow-specific attempt status chosen by the caller.
        pub fn build_trustpay_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            attempt_status: Option<FlowStatus>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: Result<TrustpayErrorResponse, Report<common_utils::errors::ParsingError>> =
                res.response.parse_struct("trustpay ErrorResponse");

            match response {
                Ok(response_data) => {
                    if let Some(i) = event_builder {
                        i.set_connector_response(&response_data);
                    }
                    let typed = macros::serialize_typed_connector_payload(
                        &response_data,
                        "typed_connector_response",
                    );
                    let error_list = response_data.errors.clone().unwrap_or_default();
                    let option_error_code_message =
                        utils::get_error_code_error_message_based_on_priority(
                            self.clone(),
                            error_list.into_iter().map(|errors| errors.into()).collect(),
                        );
                    let reason = response_data.errors.map(|errors| {
                        errors
                            .iter()
                            .map(|error| error.description.clone())
                            .collect::<Vec<String>>()
                            .join(" & ")
                    });
                    let details = response_data.payment_status_details.unwrap_or_default();
                    let (code, message) = match option_error_code_message {
                        Some(error_code_message) => (
                            error_code_message.error_code,
                            error_code_message.error_message,
                        ),
                        None => (
                            response_data
                                .payment_status
                                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                            response_data
                                .payment_description
                                .clone()
                                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        ),
                    };
                    Ok(ErrorResponse {
                        status_code: res.status_code,
                        code,
                        message,
                        reason: reason
                            .or(response_data.description)
                            .or(response_data.payment_description),
                        attempt_status,
                        connector_transaction_id: response_data.instance_id,
                        network_advice_code: details.merchant_advice_code,
                        network_decline_code: details.scheme_response_code,
                        network_error_message: details.extended_description,
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
                    domain_types::utils::handle_json_response_deserialization_failure(res, "trustpay")
                }
            }
        }

        /// `attempt_status` for a 4xx on a flow that moves money; a 5xx stays inconclusive.
        pub fn client_error_status(
            &self,
            status_code: u16,
            flow_status: FlowStatus,
        ) -> Option<FlowStatus> {
            (400..500).contains(&status_code).then_some(flow_status)
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.trustpay.base_url_bank_redirects
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorErrorTypeMapping for Trustpay<T>
{
    fn get_connector_error_type(
        &self,
        error_code: String,
        error_message: String,
    ) -> ConnectorErrorType {
        match (error_code.as_str(), error_message.as_str()) {
            // 2xx card api error codes and messages mapping
            ("100.100.600", "Empty CVV for VISA, MASTER not allowed") => ConnectorErrorType::UserError,
            ("100.350.100", "Referenced session is rejected (no action possible)") => ConnectorErrorType::TechnicalError,
            ("100.380.401", "User authentication failed") => ConnectorErrorType::UserError,
            ("100.380.501", "Risk management transaction timeout") => ConnectorErrorType::TechnicalError,
            ("100.390.103", "PARes validation failed - problem with signature") => ConnectorErrorType::TechnicalError,
            ("100.390.111", "Communication error to VISA/Mastercard Directory Server") => ConnectorErrorType::TechnicalError,
            ("100.390.112", "Technical error in 3D system") => ConnectorErrorType::TechnicalError,
            ("100.390.115", "Authentication failed due to invalid message format") => ConnectorErrorType::TechnicalError,
            ("100.390.118", "Authentication failed due to suspected fraud") => ConnectorErrorType::UserError,
            ("100.400.304", "Invalid input data") => ConnectorErrorType::UserError,
            ("200.300.404", "Invalid or missing parameter") => ConnectorErrorType::UserError,
            ("300.100.100", "Transaction declined (additional customer authentication required)") => ConnectorErrorType::UserError,
            ("400.001.301", "Card not enrolled in 3DS") => ConnectorErrorType::UserError,
            ("400.001.600", "Authentication error") => ConnectorErrorType::UserError,
            ("400.001.601", "Transaction declined (auth. declined)") => ConnectorErrorType::UserError,
            ("400.001.602", "Invalid transaction") => ConnectorErrorType::UserError,
            ("400.001.603", "Invalid transaction") => ConnectorErrorType::UserError,
            ("700.400.200", "Cannot refund (refund volume exceeded or tx reversed or invalid workflow)") => ConnectorErrorType::BusinessError,
            ("700.500.001", "Referenced session contains too many transactions") => ConnectorErrorType::TechnicalError,
            ("700.500.003", "Test accounts not allowed in production") => ConnectorErrorType::UserError,
            ("800.100.151", "Transaction declined (invalid card)") => ConnectorErrorType::UserError,
            ("800.100.152", "Transaction declined by authorization system") => ConnectorErrorType::UserError,
            ("800.100.153", "Transaction declined (invalid CVV)") => ConnectorErrorType::UserError,
            ("800.100.155", "Transaction declined (amount exceeds credit)") => ConnectorErrorType::UserError,
            ("800.100.157", "Transaction declined (wrong expiry date)") => ConnectorErrorType::UserError,
            ("800.100.162", "Transaction declined (limit exceeded)") => ConnectorErrorType::BusinessError,
            ("800.100.163", "Transaction declined (maximum transaction frequency exceeded)") => ConnectorErrorType::BusinessError,
            ("800.100.168", "Transaction declined (restricted card)") => ConnectorErrorType::UserError,
            ("800.100.170", "Transaction declined (transaction not permitted)") => ConnectorErrorType::UserError,
            ("800.100.172", "Transaction declined (account blocked)") => ConnectorErrorType::BusinessError,
            ("800.100.190", "Transaction declined (invalid configuration data)") => ConnectorErrorType::BusinessError,
            ("800.120.100", "Rejected by throttling") => ConnectorErrorType::TechnicalError,
            ("800.300.401", "Bin blacklisted") => ConnectorErrorType::BusinessError,
            ("800.700.100", "Transaction for the same session is currently being processed, please try again later") => ConnectorErrorType::TechnicalError,
            ("900.100.300", "Timeout, uncertain result") => ConnectorErrorType::TechnicalError,
            // 4xx error codes for cards api are unique and messages vary, so we are relying only on error code to decide an error type
            ("4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" | "12" | "13" | "14" | "15" | "16" | "17" | "18" | "19" | "26" | "34" | "39" | "48" | "52" | "85" | "86", _) => ConnectorErrorType::UserError,
            ("21" | "22" | "23" | "30" | "31" | "32" | "35" | "37" | "40" | "41" | "45" | "46" | "49" | "50" | "56" | "60" | "67" | "81" | "82" | "83" | "84" | "87", _) => ConnectorErrorType::BusinessError,
            ("59", _) => ConnectorErrorType::TechnicalError,
            ("1", _) => ConnectorErrorType::UnknownError,
            // Error codes for bank redirects api are unique and messages vary, so we are relying only on error code to decide an error type
            ("1112008" | "1132000" | "1152000", _) => ConnectorErrorType::UserError,
            ("1112009" | "1122006" | "1132001" | "1132002" | "1132003" | "1132004" | "1132005" | "1132006" | "1132008" | "1132009" | "1132010" | "1132011" | "1132012" | "1132013" | "1133000" | "1133001" | "1133002" | "1133003" | "1133004", _) => ConnectorErrorType::BusinessError,
            ("1132014", _) => ConnectorErrorType::TechnicalError,
            ("1132007", _) => ConnectorErrorType::UnknownError,
            _ => ConnectorErrorType::UnknownError
}
    }
}

/// Authorize flavour (aapi JSON + Bearer vs tpgw form + X-Api-Key), shared by Authorize get_url,
/// get_headers and ContentTypeSelector<Authorize>.
// ponytail: keyed on payment_method_data because the framework sets PaymentFlowData.payment_method
// = Card on Authorize (domain_types types.rs:6720); switch back once that is populated.
fn is_bank_flavour_authorize<T: PaymentMethodDataTypes>(
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> bool {
    matches!(
        req.request.payment_method_data,
        PaymentMethodData::BankRedirect(_) | PaymentMethodData::BankTransfer(_)
    )
}

// Implement ContentTypeSelector for dynamic content type selection
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ContentTypeSelector<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
    for Trustpay<T>
{
    fn get_dynamic_content_type(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<common_enums::DynamicContentType, IntegrationError> {
        Ok(if is_bank_flavour_authorize(req) {
            common_enums::DynamicContentType::Json
        } else {
            common_enums::DynamicContentType::FormUrlEncoded
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Trustpay<T>
{
    fn id(&self) -> &'static str {
        "trustpay"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.trustpay.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_trustpay_error_response(res, event_builder, None)
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpay,
    curl_response: TrustpayPaymentsSyncResponse,
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
            self.build_headers_for_payments(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass the instanceId (card) or PaymentRequestId (bank redirect) returned by Authorize as connector_transaction_id."
                                .to_string(),
                        ),
                        doc_url: Some("https://docs.trustpay.eu/".to_string()),
                        additional_context: Some(
                            "Trustpay payment status lookup is keyed on the connector transaction id; none was present on the sync request."
                                .to_string(),
                        ),
                    },
                })?;
        match req.resource_common_data.payment_method {
            common_enums::PaymentMethod::BankRedirect | common_enums::PaymentMethod::BankTransfer => Ok(format!(
                "{}{}/{}",
                self.connector_base_url_bank_redirects_payments(req),
                "api/Payments/Payment",
                transaction_id,
            )),
            _ => Ok(format!(
                "{}{}/{}",
                self.connector_base_url_payments(req),
                "api/v1/instance",
                transaction_id,
            ))
}
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpay,
    curl_request: FormUrlEncoded(TrustpayAuthUpdateRequest),
    curl_response: TrustpayAuthUpdateResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = trustpay::TrustpayAuthType::try_from(&req.connector_config)
            .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            let auth_value = auth
                .project_id
                .zip(auth.secret_key)
                .map(|(project_id, secret_key)| {
                    format!(
                        "Basic {}",
                        BASE64_ENGINE
                            .encode(format!("{project_id}:{secret_key}"))
                    )
                });
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (headers::AUTHORIZATION.to_string(), auth_value.into_masked()),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
            "{}{}",
            self.connector_base_url_merchant_auth(req), "api/oauth2/token"
        ))
        }
    }
);

// Macro implementation for CreateOrder flow (used for wallet initialization)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpay,
    curl_request: FormUrlEncoded(TrustpayCreateIntentRequest),
    curl_response: TrustpayCreateIntentResponse,
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
            self.build_headers_for_payments(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "api/v1/intent"
            ))
        }
    }
);

// Macro implementation for Authorize flow using dynamic content types
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Trustpay,
    curl_request: Dynamic(TrustpayPaymentsRequest<T>),
    curl_response: TrustpayPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = self.client_error_status(res.status_code, FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            self.build_trustpay_error_response(res, event_builder, attempt_status)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_flavour(req, is_bank_flavour_authorize(req))
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            match is_bank_flavour_authorize(req) {
                true => Ok(format!(
                    "{}{}",
                    self.connector_base_url_bank_redirects_payments(req),
                    "api/Payments/Payment"
                )),
                false => Ok(format!(
                    "{}{}",
                    self.connector_base_url_payments(req),
                    "api/v1/purchase"
                ))
}
        }
    }
);

// Capture a Preauthorization instance (card gateway only).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Trustpay,
    curl_request: FormUrlEncoded(TrustpayCaptureRequest),
    curl_response: TrustpayCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = self.client_error_status(res.status_code, FlowStatus::Payment(common_enums::AttemptStatus::CaptureFailed));
            self.build_trustpay_error_response(res, event_builder, attempt_status)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_payments(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "api/v1/Capture"
            ))
        }
    }
);

// Reverse (void) an authorized Preauthorization instance (card gateway only).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Trustpay,
    curl_request: FormUrlEncoded(TrustpayVoidRequest),
    curl_response: TrustpayVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = self.client_error_status(res.status_code, FlowStatus::Payment(common_enums::AttemptStatus::VoidFailed));
            self.build_trustpay_error_response(res, event_builder, attempt_status)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_payments(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "api/v1/Reverse"
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ContentTypeSelector<Refund, RefundFlowData, RefundsData, RefundsResponseData> for Trustpay<T>
{
    fn get_dynamic_content_type(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<common_enums::DynamicContentType, IntegrationError> {
        match req.resource_common_data.payment_method {
            Some(common_enums::PaymentMethod::BankRedirect)
            | Some(common_enums::PaymentMethod::BankTransfer) => {
                Ok(common_enums::DynamicContentType::Json)
            }
            _ => Ok(common_enums::DynamicContentType::FormUrlEncoded),
        }
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Trustpay,
    curl_request: Dynamic(TrustpayRefundRequest),
    curl_response: RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = self.client_error_status(res.status_code, FlowStatus::Refund(common_enums::RefundStatus::Failure));
            self.build_trustpay_error_response(res, event_builder, attempt_status)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_refunds(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
          match req.resource_common_data.payment_method {
            Some(common_enums::PaymentMethod::BankRedirect) | Some(common_enums::PaymentMethod::BankTransfer) => Ok(format!(
                "{}{}{}{}",
                self.connector_base_url_bank_redirects_refunds(req),
                "api/Payments/Payment/",
                req.request.connector_transaction_id,
                "/Refund"
            )),
            _ => Ok(format!("{}{}", self.connector_base_url_refunds(req), "api/v1/Refund"))
}
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Trustpay,
    curl_response: RefundSyncResponse,
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
            self.build_headers_for_refunds(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        let id = req.request.connector_refund_id.clone();
        // G-RSync-01: an empty refund id would build a URL for the collection, not the refund.
        if id.is_empty() {
            return Err(IntegrationError::MissingConnectorRefundID {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the refund instanceId (card) or refund PaymentRequestId (bank redirect) returned by Refund as connector_refund_id."
                            .to_string(),
                    ),
                    doc_url: Some("https://docs.trustpay.eu/".to_string()),
                    additional_context: Some(
                        "Trustpay refund status lookup is keyed on the connector refund id; an empty one was present on the refund sync request."
                            .to_string(),
                    ),
                },
            }
            .into());
        }
        match req.resource_common_data.payment_method {
            Some(common_enums::PaymentMethod::BankRedirect) | Some(common_enums::PaymentMethod::BankTransfer) => Ok(format!(
                "{}{}/{}",
                self.connector_base_url_bank_redirects_refunds(req), "api/Payments/Payment", id
            )),
            _ => Ok(format!(
                "{}{}/{}",
                self.connector_base_url_refunds(req),
                "api/v1/instance",
                id
            ))
}
        }
    }
);

// SetupMandate (SetupRecurring) - stores card credentials for recurring payments
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Trustpay,
    curl_request: FormUrlEncoded(TrustpaySetupMandateRequest<T>),
    curl_response: TrustpaySetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = self.client_error_status(res.status_code, FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            self.build_trustpay_error_response(res, event_builder, attempt_status)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_payments(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TrustPay uses the same card API endpoint for mandate setup (zero-auth validation)
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "api/v1/purchase"
            ))
        }
    }
);

// RepeatPayment (recurring subsequent / MIT) - charges a stored InstanceId, or an NTID with raw card
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Trustpay,
    curl_request: FormUrlEncoded(TrustpayRepeatPaymentRequest<T>),
    curl_response: TrustpayRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = self.client_error_status(res.status_code, FlowStatus::Payment(common_enums::AttemptStatus::Failure));
            self.build_trustpay_error_response(res, event_builder, attempt_status)
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_payments(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same card API endpoint as Authorize/SetupMandate, request body carries InstanceId + PaymentType=Recurring
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "api/v1/purchase"
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Trustpay<T>
{
}

// We already have an implementation for ValidationTrait above

macros::macro_connector_flow_status_impls!(
    connector: Trustpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        Accept,
        SubmitEvidence,
        DefendDispute,
        ServerSessionAuthenticationToken,
        VoidPC,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
        MandateRevoke,
    ],
);
