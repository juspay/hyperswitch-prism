pub mod transformers;
use super::macros;
use std::fmt::Debug;

use crate::{types::ResponseRouterData, with_error_response_body};
use common_enums::CurrencyUnit;
use base64::Engine;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow,
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificAuth, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as revolv3, validate_psync, Revolv3AuthReversalRequest, Revolv3AuthReversalResponse,
    Revolv3AuthorizeResponse, Revolv3CaptureRequest, Revolv3PaymentSyncResponse,
    Revolv3PaymentsRequest, Revolv3PaymentsResponse, Revolv3RefundRequest, Revolv3RefundResponse,
    Revolv3RefundSyncResponse, Revolv3RepeatPaymentRequest, Revolv3RepeatPaymentResponse,
    Revolv3SaleResponse, Revolv3SetupMandateRequest, Revolv3InvoiceWebhookBody,  Revolv3WebhookBody, Revolv3WebhookBodyData,
};
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const REVOLV3_TOKEN: &str = "x-revolv3-token";
    pub const REVOLV3_SIGNATURE_KEY: &str = "X-Revolv3-Signature";
    pub const REVOLV3_SIGNATURE_KEY_LOWERCASE: &str = "x-revolv3-signature";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Revolv3<T>
{
    fn id(&self) -> &'static str {
        "revolv3"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.revolv3.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificAuth,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
        let auth = revolv3::Revolv3AuthType::try_from(auth_type)
            .change_context(errors::ConnectorError::FailedToObtainAuthType)?;
        Ok(vec![(
            headers::REVOLV3_TOKEN.to_string(),
            auth.api_key.expose().into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: revolv3::Revolv3ErrorResponse = res
            .response
            .parse_struct("Revolv3ErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: common_utils::consts::NO_ERROR_CODE.to_string(),
            message: response.message,
            reason: response.errors.map(|errors| errors.join(", ")),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Revolv3<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificAuth>,
    ) -> Result<bool, error_stack::Report<errors::ConnectorError>> {
        let webhook_secret = connector_webhook_secret
            .ok_or(errors::ConnectorError::WebhookVerificationSecretNotFound)?
            .secret;

        let signature_header = request
            .headers
            .get(headers::REVOLV3_SIGNATURE_KEY_LOWERCASE)
            .or_else(|| request.headers.get(headers::REVOLV3_SIGNATURE_KEY))
            .ok_or(errors::ConnectorError::WebhookSignatureNotFound)?;

        let url = match request.url {
            Some(url) => url,
            None => {
                tracing::warn!(
                    target: "revolv3_webhook",
                    "Missing URL in webhook request"
                );
                return Ok(false);
            }
        };

        let message = format!("{}${}", url, String::from_utf8_lossy(&request.body));

        use common_utils::crypto::{HmacSha256, SignMessage};
        let crypto_algorithm = HmacSha256;
        let computed_signature = match crypto_algorithm
            .sign_message(&webhook_secret, message.as_bytes())
        {
            Ok(sig) => sig,
            Err(crypto_error) => {
                tracing::error!(
                    target: "revolv3_webhook",
                    "Failed to compute HMAC-Sha256 signature for webhook verification, error: {:?} - verification failed but continuing processing",
                    crypto_error
                );
                return Ok(false);
            }
        };

        let computed_signature_b64 = BASE64_ENGINE.encode(&computed_signature);
        let check_point = computed_signature_b64 == *signature_header;
        Ok(check_point)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificAuth>,
    ) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
        let webhook_body: Revolv3WebhookBody = request
            .body
            .parse_struct("Revolv3Webhook")
            .change_context(errors::ConnectorError::WebhookEventTypeNotFound)
            .attach_printable_lazy(|| {
                "Failed to parse webhook event type from Revolv3 webhook body"
            })?;

        let webhook_body_data: Revolv3WebhookBodyData = serde_json::from_str(&webhook_body.body)
            .change_context(errors::ConnectorError::WebhookEventTypeNotFound)
            .attach_printable_lazy(|| {
                "Failed to parse webhook event type from Revolv3 webhook body"
            })?;

        match webhook_body_data {
            Revolv3WebhookBodyData::InvoiceData(invoice_webhook) => {
                Ok(invoice_webhook.get_invoice_status().to_event_type())
            }
            Revolv3WebhookBodyData::TestData(_) => Ok(EventType::EndpointVerification),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificAuth>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        let request_body_copy = request.body.clone();
        let webhook_data: Revolv3WebhookBody = request
            .body
            .parse_struct("Revolv3WebhookBody")
            .change_context(errors::ConnectorError::WebhookResourceObjectNotFound)
            .attach_printable_lazy(|| "Failed to parse Revolv3 payment webhook body structure")?;

        let webhook_body: Revolv3InvoiceWebhookBody = serde_json::from_str(&webhook_data.body)
            .change_context(errors::ConnectorError::WebhookEventTypeNotFound)
            .attach_printable_lazy(|| "Failed to parse invoice data from Revolv3 webhook body")?;

        let invoice_id = webhook_body.get_invoice_id();
        let status = webhook_body.get_invoice_status().to_attempt_status()?;

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(invoice_id.clone())),
            status,
            status_code: 200,
            mandate_reference: webhook_body.get_mandate_reference().map(Box::new),
            connector_response_reference_id: webhook_body.get_merchant_invoice_ref_id(),
            error_code: webhook_body.get_error_code(),
            error_message: webhook_body.get_error_message(),
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            response_headers: None,
            minor_amount_captured: None,
            amount_captured: None,
            error_reason: webhook_body.get_error_message(),
            network_txn_id: webhook_body.get_network_transaction_id(),
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificAuth>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        let request_body_copy = request.body.clone();
        let webhook_data: Revolv3WebhookBody = request
            .body
            .parse_struct("Revolv3WebhookBody")
            .change_context(errors::ConnectorError::WebhookResourceObjectNotFound)
            .attach_printable_lazy(|| "Failed to parse Revolv3 refund webhook body structure")?;

        let webhook_body: Revolv3InvoiceWebhookBody = serde_json::from_str(&webhook_data.body)
            .change_context(errors::ConnectorError::WebhookEventTypeNotFound)
            .attach_printable_lazy(|| "Failed to parse invoice data from Revolv3 webhook body")?;
        let invoice_id = webhook_body.get_invoice_id();

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(invoice_id.clone()),
            status: webhook_body.get_invoice_status().to_refund_status()?,
            status_code: 200,
            connector_response_reference_id: Some(invoice_id),
            error_code: webhook_body.get_error_code(),
            error_message: webhook_body.get_error_message(),
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            response_headers: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::Accept,
        DisputeFlowData,
        AcceptDisputeData,
        DisputeResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::DefendDispute,
        DisputeFlowData,
        DisputeDefendData,
        DisputeResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::SubmitEvidence,
        DisputeFlowData,
        SubmitEvidenceData,
        DisputeResponseData,
    > for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Revolv3<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Revolv3<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Revolv3,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: Revolv3PaymentsRequest<T>,
            response_body: Revolv3PaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: Revolv3PaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: Revolv3CaptureRequest,
            response_body: Revolv3SaleResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: Revolv3AuthReversalRequest,
            response_body: Revolv3AuthReversalResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: Revolv3RefundRequest,
            response_body: Revolv3RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: Revolv3RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: Revolv3RepeatPaymentRequest<T>,
            response_body: Revolv3RepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: Revolv3SetupMandateRequest<T>,
            response_body: Revolv3AuthorizeResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                )
            ];
            let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.revolv3.base_url.to_string()
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolv3,
    curl_request: Json(Revolv3PaymentsRequest),
    curl_response: Revolv3PaymentsResponse,
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
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
        self.build_headers(req)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    ) -> CustomResult<String, errors::ConnectorError> {
        let base_url = self.connector_base_url(req);
        if req.request.is_auto_capture()? {
            Ok(format!(
                "{base_url}/api/payments/sale"
            ))
        } else {
        Ok(format!(
            "{base_url}/api/payments/authorization"
        ))
    }
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolv3,
    curl_response: Revolv3PaymentSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            validate_psync(&req.request.connector_metadata)?;
            self.build_headers(req)
        }

    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<String, errors::ConnectorError> {
            let invoice_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/Invoices/{invoice_id}"))
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolv3,
    curl_request: Json(Revolv3RefundRequest),
    curl_response: Revolv3RefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
              self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let invoice_id = req.request.connector_transaction_id.clone();
            if invoice_id.is_empty() {Err(errors::ConnectorError::MissingConnectorTransactionID)?};
            let base_url = req.resource_common_data.connectors.revolv3.base_url.to_string();
            Ok(format!("{base_url}/api/Invoices/{invoice_id}/refund"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolv3,
    curl_response: Revolv3RefundSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
              self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let invoice_id = req.request.connector_refund_id.clone();
            let base_url = req.resource_common_data.connectors.revolv3.base_url.to_string();
            Ok(format!("{base_url}/api/Invoices/{invoice_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolv3,
    curl_request: Json(Revolv3CaptureRequest),
    curl_response: Revolv3SaleResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
              self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let payment_method_authorization_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/Payments/capture/{payment_method_authorization_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolv3,
    curl_request: Json(Revolv3AuthReversalRequest),
    curl_response: Revolv3AuthReversalResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
             let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/api/PaymentMethod/reverse-auth"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Revolv3,
    curl_request: Json(Revolv3RepeatPaymentRequest),
    curl_response: Revolv3RepeatPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let base_url = self.connector_base_url(req);
            match req.request.get_mandate_reference() {
                MandateReferenceId::NetworkMandateId(_) => {
                      if req.request.is_auto_capture()? {
                        Ok(format!("{base_url}/api/payments/sale"))
                    } else {
                        Ok(format!("{base_url}/api/payments/authorization"))
                    }
                }
                MandateReferenceId::ConnectorMandateId(connector_mandate_data) => {
                    let payment_method_id = connector_mandate_data.get_connector_mandate_id()
                        .ok_or(errors::ConnectorError::MissingConnectorTransactionID)?;
                    if req.request.is_auto_capture()? {
                        Ok(format!("{base_url}/api/payments/sale/{payment_method_id}"))
                    } else {
                        Ok(format!("{base_url}/api/payments/authorization/{payment_method_id}"))
                    }
                }
                MandateReferenceId::NetworkTokenWithNTI(_) => {
                    Err(errors::ConnectorError::FlowNotSupported {
                        flow: "Network Token with NTI".to_string(),
                        connector: "revolv3".to_string(),
                    })?
                }
            }
        }}
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2, get_content_type],
    connector: Revolv3,
    curl_request: Json(Revolv3SetupMandateRequest),
    curl_response: Revolv3AuthorizeResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let base_url = self.connector_base_url(req);
            Ok(format!(
                "{base_url}/api/payments/authorization"
            ))
        }
    }
);
