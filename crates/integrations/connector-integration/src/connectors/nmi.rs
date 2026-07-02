pub mod transformers;

use domain_types::router_data::ConnectorSpecificConfig;
use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        Void,
    },
    connector_types::{
        ConnectorWebhookSecrets, EventType, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    NmiCaptureRequest, NmiPaymentsRequest, NmiRefundRequest, NmiRefundSyncRequest,
    NmiRepeatPaymentRequest, NmiRepeatPaymentResponse, NmiSetupMandateRequest,
    NmiSetupMandateResponse, NmiSyncRequest, NmiVaultRequest, NmiVaultResponse, NmiVoidRequest,
    StandardResponse, SyncResponse,
};

// Type aliases to avoid duplicate templating in macros
pub type NmiCaptureResponse = StandardResponse;
pub type NmiVoidResponse = StandardResponse;
pub type NmiRefundResponse = StandardResponse;
pub type NmiPSyncResponse = SyncResponse;
pub type NmiRSyncResponse = SyncResponse;
pub type NmiPreAuthenticateResponse = NmiVaultResponse;

use super::macros;
use crate::{
    types::ResponseRouterData, with_error_response_body, ConnectorError, IntegrationError,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

pub(crate) mod endpoints {
    pub(crate) const TRANSACT: &str = "/api/transact.php";
    pub(crate) const QUERY: &str = "/api/query.php";
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====

macros::macro_connector_payout_implementation!(
    connector: Nmi,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Nmi<T>
{
    /// HMAC-SHA256 over `"{nonce}.{raw_body}"` with the merchant webhook secret,
    /// where nonce + signature come from the `webhook-signature` header
    /// (`t=<nonce>,s=<hex signature>`). Ports the hyperswitch default
    /// `verify_webhook_source` combined with NMI's overridden signature/message hooks.
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret.ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
        })?;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        use common_utils::crypto::SignMessage;
        common_utils::crypto::HmacSha256
            .sign_message(&connector_webhook_secrets.secret, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to sign the NMI webhook message with HMAC-SHA256")
            .map(|expected_signature| expected_signature == signature)
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let sig_header = transformers::get_nmi_webhook_signature_header(request)?;

        let (_nonce, signature) = transformers::parse_nmi_webhook_signature_header(sig_header)
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;

        // The header carries the signature hex-encoded; decode before comparing.
        hex::decode(signature).change_context(WebhookError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let sig_header = transformers::get_nmi_webhook_signature_header(request)?;

        let (nonce, _signature) = transformers::parse_nmi_webhook_signature_header(sig_header)
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;

        // Byte-exact hyperswitch message: `format!("{}.{}", nonce, raw_body)`.
        let message = format!("{}.{}", nonce, String::from_utf8_lossy(&request.body));
        Ok(message.into_bytes())
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let event_type_body: transformers::NmiWebhookEventBody =
            serde_json::from_slice(&request.body)
                .change_context(WebhookError::WebhookResourceObjectNotFound)
                .attach_printable("Failed to decode the NMI webhook event body")?;

        Ok(transformers::get_nmi_webhook_event(
            event_type_body.event_type,
        ))
    }

    /// Ports HS `get_webhook_object_reference_id`: NMI echoes back the hyperswitch-side
    /// identifier in `event_body.order_id` — the payment attempt id for payment actions
    /// (`PaymentIdType::PaymentAttemptId`) and the refund id for refunds
    /// (`RefundIdType::RefundId`). Both are caller-assigned (merchant) references, so the
    /// connector-assigned id fields MUST stay `None` for the reference to normalise
    /// identically to the Direct gateway.
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let reference_body: transformers::NmiWebhookObjectReference =
            serde_json::from_slice(&request.body)
                .change_context(WebhookError::WebhookResourceObjectNotFound)
                .attach_printable("Failed to decode the NMI webhook object reference")?;

        match reference_body.event_body.action.action_type {
            transformers::NmiActionType::Sale
            | transformers::NmiActionType::Auth
            | transformers::NmiActionType::Capture
            | transformers::NmiActionType::Void => Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: None,
                    merchant_transaction_id: Some(reference_body.event_body.order_id),
                },
            ))),
            transformers::NmiActionType::Refund => Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: None,
                    merchant_refund_id: Some(reference_body.event_body.order_id),
                    connector_transaction_id: None,
                },
            ))),
            // HS maps `credit` to `WebhooksNotImplemented`.
            transformers::NmiActionType::Credit => Err(error_stack::report!(
                WebhookError::WebhooksNotImplemented {
                    operation: "nmi credit webhooks",
                }
            )),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body: transformers::NmiWebhookBody = serde_json::from_slice(&request.body)
            .change_context(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to decode the NMI webhook body")?;

        // HS reshapes payment webhooks into the PSync `SyncResponse` and maps
        // `condition` -> NmiStatus -> AttemptStatus.
        let status = common_enums::AttemptStatus::from(transformers::NmiStatus::from(
            webhook_body.event_body.condition.clone(),
        ));

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                webhook_body.event_body.transaction_id.clone(),
            )),
            status,
            connector_response_reference_id: None,
            mandate_reference: None,
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
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body: transformers::NmiWebhookBody = serde_json::from_slice(&request.body)
            .change_context(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to decode the NMI webhook body")?;

        // `transaction_id` is the NMI-assigned id of the refund transaction;
        // `condition` -> NmiStatus -> RefundStatus mirrors the HS mapping.
        let status = common_enums::RefundStatus::from(transformers::NmiStatus::from(
            webhook_body.event_body.condition.clone(),
        ));

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(webhook_body.event_body.transaction_id.clone()),
            // NMI refund webhooks carry no reference to the original payment;
            // `order_id` here is the merchant refund id (used for the reference).
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

    /// Ports HS `get_webhook_resource_object`: payment actions (incl. `credit`) are
    /// reshaped into the PSync-style `{"transaction":{...}}` object; refunds return
    /// the webhook body as-is.
    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let webhook_body: transformers::NmiWebhookBody = serde_json::from_slice(&request.body)
            .change_context(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to decode the NMI webhook body")?;

        match webhook_body.event_body.action.action_type {
            transformers::NmiActionType::Sale
            | transformers::NmiActionType::Auth
            | transformers::NmiActionType::Capture
            | transformers::NmiActionType::Void
            | transformers::NmiActionType::Credit => Ok(Box::new(
                transformers::NmiWebhookSyncResponse::from(&webhook_body),
            )),
            transformers::NmiActionType::Refund => Ok(Box::new(webhook_body)),
        }
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"event_type":"transaction.sale.success","event_body":{"transaction_id":"dummy_txn_001","order_id":"dummy_order_001","condition":"pendingsettlement","action":{"action_type":"sale"}}}"#
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Nmi<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Nmi<T>
{
} // ===== CREATE CONNECTOR STRUCT WITH MACROS =====
macros::create_all_prerequisites!(
    connector_name: Nmi,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: NmiPaymentsRequest<T>,
            response_body: StandardResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: NmiCaptureRequest,
            response_body: NmiCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: NmiVoidRequest,
            response_body: NmiVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: NmiRefundRequest,
            response_body: NmiRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: PSync,
            request_body: NmiSyncRequest,
            response_body: NmiPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            request_body: NmiRefundSyncRequest,
            response_body: NmiRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: NmiVaultRequest<T>,
            response_body: NmiPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: NmiSetupMandateRequest<T>,
            response_body: NmiSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: NmiRepeatPaymentRequest,
            response_body: NmiRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            _status_code: u16,
        ) -> CustomResult<bytes::Bytes, IntegrationError> {
            // NMI returns different response formats:
            // - XML for query endpoints (PSync/RSync)
            // - URL-encoded for transact endpoints (Authorize/Capture/Refund/Void)
            let response_str = std::str::from_utf8(&bytes)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })
                .attach_printable("Failed to decode NMI response as UTF-8")?;

            // Check if response is XML (PSync/RSync return XML)
            if response_str.trim().starts_with("<?xml") || response_str.trim().starts_with("<") {
                // Parse XML to struct, then serialize back to JSON
                let xml_response: SyncResponse = quick_xml::de::from_str(response_str)
                    .change_context(IntegrationError::BodySerializationFailed {
                        context: Default::default(),
                    })
                    .attach_printable("Failed to parse XML response from NMI query endpoint")?;

                let json_bytes = serde_json::to_vec(&xml_response)
                    .change_context(IntegrationError::BodySerializationFailed {
                        context: Default::default(),
                    })
                    .attach_printable("Failed to convert XML response to JSON")?;

                Ok(bytes::Bytes::from(json_bytes))
            } else {
                // URL-encoded response - parse and convert to JSON
                let url_encoded_response: StandardResponse = serde_urlencoded::from_bytes(&bytes)
                    .change_context(IntegrationError::BodySerializationFailed {
                        context: Default::default(),
                    })
                    .attach_printable("Failed to parse URL-encoded response from NMI transact endpoint")?;

                let json_bytes = serde_json::to_vec(&url_encoded_response)
                    .change_context(IntegrationError::BodySerializationFailed {
                        context: Default::default(),
                    })
                    .attach_printable("Failed to convert URL-encoded response to JSON")?;

                Ok(bytes::Bytes::from(json_bytes))
            }
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nmi.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nmi.base_url
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Nmi<T>
{
    fn id(&self) -> &'static str {
        "nmi"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // NMI uses base currency units (dollars, not cents)
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.nmi.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Parse URL-encoded error response
        let response: StandardResponse = serde_urlencoded::from_bytes(&res.response)
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "nmi: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.response_code.clone(),
            message: response.responsetext.clone(),
            reason: Some(response.responsetext),
            attempt_status: None,
            connector_transaction_id: Some(response.transactionid),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ===== MAIN CONNECTOR INTEGRATION IMPLEMENTATIONS =====
// Authorize flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiPaymentsRequest),
    curl_response: StandardResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoints::TRANSACT))
        }
    }
);

// Payment Sync
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiSyncRequest),
    curl_response: NmiPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoints::QUERY))
        }
    }
);

// Payment Capture
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiCaptureRequest),
    curl_response: StandardResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoints::TRANSACT))
        }
    }
);

// Payment Void
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiVoidRequest),
    curl_response: NmiVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoints::TRANSACT))
        }
    }
);

// Refund
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiRefundRequest),
    curl_response: NmiRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_refunds(req), endpoints::TRANSACT))
        }
    }
);

// Refund Sync
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiRefundSyncRequest),
    curl_response: NmiRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_refunds(req), endpoints::QUERY))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiVaultRequest),
    curl_response: NmiPreAuthenticateResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoints::TRANSACT))
        }
    }
);

// SetupMandate (SetupRecurring) - adds payment method to Customer Vault
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiSetupMandateRequest),
    curl_response: NmiSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoints::TRANSACT))
        }
    }
);

// RepeatPayment (RecurringPaymentService/Charge) - sale using stored customer_vault_id
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nmi,
    curl_request: FormUrlEncoded(NmiRepeatPaymentRequest),
    curl_response: NmiRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoints::TRANSACT))
        }
    }
);

// ===== EMPTY CONNECTOR INTEGRATIONS =====

macros::macro_connector_flow_status_impls!(
    connector: Nmi,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PaymentMethodToken,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
        CreateConnectorCustomer,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        VoidPC,
        CreateOrder,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        Accept,
        DefendDispute,
        SubmitEvidence,
        ClientAuthenticationToken,
    ],
);
