pub mod constants;
pub mod headers;
pub mod transformers;

use common_enums as enums;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::{ByteSliceExt, BytesExt},
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        ConnectorSpecifications, ConnectorWebhookSecrets, EventType, PaymentFlowData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RequestDetails, ResponseId, WebhookDetailsResponse,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, UpiData},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{ConnectorInfo, Connectors},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as phonepe;

use self::transformers::{
    PhonepeCaptureRequest, PhonepeCaptureResponse, PhonepePaymentsRequest, PhonepePaymentsResponse,
    PhonepeRefundRequest, PhonepeRefundResponse, PhonepeRefundSyncRequest,
    PhonepeRefundSyncResponse, PhonepeSyncRequest, PhonepeSyncResponse, PhonepeVoidRequest,
    PhonepeVoidResponse,
};
use super::macros;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::IntegrationErrorContext;
use domain_types::errors::WebhookError;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Phonepe<T>
{
}
macros::macro_connector_payout_implementation!(
    connector: Phonepe,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Phonepe<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_str = request
            .headers
            .get("x-verify")
            .ok_or(WebhookError::WebhookSignatureNotFound)?;
        Ok(signature_str.as_bytes().to_vec())
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        use hyperswitch_masking::Secret;

        let webhook_request: phonepe::PhonepeWebhookRequest = request
            .body
            .parse_struct("PhonepeWebhookRequest")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let base64_body = &webhook_request.response;
        let api_path = request.uri.as_deref().unwrap_or("");
        let key_index = request
            .headers
            .get("x-verify")
            .and_then(|v| v.split("###").nth(1))
            .unwrap_or("1");

        let salt_key =
            Secret::new(String::from_utf8_lossy(&connector_webhook_secret.secret).to_string());

        let expected_checksum =
            phonepe::compute_phonepe_webhook_checksum(base64_body, api_path, &salt_key, key_index)?;

        Ok(expected_checksum.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        use hyperswitch_masking::Secret;

        let connector_webhook_secret = match connector_webhook_secret {
            Some(secret) => secret,
            None => {
                tracing::warn!("PhonePe webhook: no webhook secret configured");
                return Ok(false);
            }
        };

        // Extract the incoming X-VERIFY header value
        let incoming_verify = match request.headers.get("x-verify") {
            Some(v) => v.clone(),
            None => {
                tracing::warn!("PhonePe webhook: X-VERIFY header missing");
                return Ok(false);
            }
        };

        // Extract key_index from incoming X-VERIFY (format: hash###keyIndex)
        let key_index = incoming_verify.split("###").nth(1).unwrap_or("1");

        // Parse the webhook body to get the base64-encoded payload
        let webhook_request: phonepe::PhonepeWebhookRequest =
            match request.body.parse_struct("PhonepeWebhookRequest") {
                Ok(req) => req,
                Err(e) => {
                    tracing::warn!("PhonePe webhook: failed to parse body: {}", e);
                    return Ok(false);
                }
            };

        let base64_body = &webhook_request.response;
        let api_path = request.uri.as_deref().unwrap_or("");
        let salt_key =
            Secret::new(String::from_utf8_lossy(&connector_webhook_secret.secret).to_string());

        // Compute expected checksum
        let expected_checksum = match phonepe::compute_phonepe_webhook_checksum(
            base64_body,
            api_path,
            &salt_key,
            key_index,
        ) {
            Ok(checksum) => checksum,
            Err(e) => {
                tracing::warn!("PhonePe webhook: failed to compute checksum: {}", e);
                return Ok(false);
            }
        };

        // Constant-time comparison to prevent timing attacks on webhook signature.
        #[allow(deprecated)] // ring 0.17 renamed the module; function is still sound
        Ok(ring::constant_time::verify_slices_are_equal(
            incoming_verify.as_bytes(),
            expected_checksum.as_bytes(),
        )
        .is_ok())
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let payload = phonepe::decode_phonepe_webhook_payload(&request.body)?;
        Ok(phonepe::map_phonepe_webhook_state_to_event_type(
            &payload.state,
        ))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let payload = phonepe::decode_phonepe_webhook_payload(&request.body)?;

        let status = phonepe::map_phonepe_webhook_state_to_attempt_status(&payload.state);

        let (error_code, error_message, error_reason) = if status == enums::AttemptStatus::Failure {
            let code = payload.response_code.clone();
            let message = code.clone();
            (code.clone(), message, code)
        } else {
            (None, None, None)
        };

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                payload.transaction_id.clone(),
            )),
            status,
            connector_response_reference_id: Some(payload.merchant_transaction_id),
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
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
            operation: "process_refund_webhook",
        }))
    }

    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::DisputeWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
            operation: "process_dispute_webhook",
        }))
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let payload = phonepe::decode_phonepe_webhook_payload(&request.body)?;
        Ok(Box::new(payload))
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SourceVerification for Phonepe<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Phonepe<T>
{
}
// Define connector prerequisites
macros::create_all_prerequisites!(
    connector_name: Phonepe,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PhonepePaymentsRequest,
            response_body: PhonepePaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: PhonepeSyncRequest,
            response_body: PhonepeSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PhonepeRefundRequest,
            response_body: PhonepeRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: PhonepeRefundSyncRequest,
            response_body: PhonepeRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PhonepeCaptureRequest,
            response_body: PhonepeCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PhonepeVoidRequest,
            response_body: PhonepeVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.phonepe.base_url.to_string()
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.phonepe.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.phonepe.base_url
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )])
        }
    }
);

// Authorize flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Phonepe,
    curl_request: Json(PhonepePaymentsRequest),
    curl_response: PhonepePaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // Get base headers first
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/json".to_string().into(),
                ),
            ];

            // Build the request to get the checksum for X-VERIFY header
            let connector_router_data = PhonepeRouterData {
                connector: self.clone(),
                router_data: req
};
            let connector_req = PhonepePaymentsRequest::try_from(&connector_router_data)?;
            headers.push((headers::X_VERIFY.to_string(), connector_req.checksum.into()));

            let browser_info = req.request.browser_info.as_ref();
            let source_channel = phonepe::get_source_channel(browser_info.and_then(|bi| bi.user_agent.as_ref()));
            let source_platform = req
                .request
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.clone().expose().get("SOURCE_PLATFORM").and_then(|v| v.as_str().map(String::from)));

            // Add common headers
            headers.extend([
                (headers::X_SOURCE.to_string(), "API".to_string().into()),
                (headers::X_SOURCE_CHANNEL.to_string(), source_channel.clone().into()),
            ]);

            if let Some(platform) = source_platform {
                headers.push((headers::X_SOURCE_PLATFORM.to_string(), platform.into()));
            }

            if let Some(ip_address) = browser_info.and_then(|bi| bi.ip_address) {
                headers.push((headers::X_MERCHANT_IP.to_string(), ip_address.to_string().into()));
            }

            match source_channel.as_str() {
                "WEB" => {
                    browser_info.map(|bi| {
                        if let Some(user_agent) = &bi.user_agent {
                            headers.push((headers::USER_AGENT.to_string(), user_agent.clone().into()));
                        }
                        if let Some(referer) = &bi.referer {
                            headers.push((headers::X_MERCHANT_DOMAIN.to_string(), referer.clone().into()));
                        }
                    });
                }
                "ANDROID" | "IOS" => {
                    let is_android = source_channel == "ANDROID";

                    browser_info.and_then(|bi| bi.user_agent.as_ref()).map(|user_agent| {
                        let version = match is_android {
                            true => phonepe::get_android_version_from_ua(user_agent),
                            false => user_agent.clone()
};
                        headers.push((headers::X_SOURCE_CHANNEL_VERSION.to_string(), version.into()));
                    });

                    if let PaymentMethodData::Upi(upi_data) = &req.request.payment_method_data {
                        let app_id_opt = match upi_data {
                            UpiData::UpiCollect(collect_data) => {
                                collect_data.vpa_id.as_ref().map(|vpa_id| {
                                    match is_android {
                                        true => vpa_id.peek().to_string(),
                                        false => phonepe::map_ios_payment_source_to_target_app(Some(vpa_id.peek()))
                                            .unwrap_or_else(|| vpa_id.peek().to_string())
}
                                })
                            }
                            UpiData::UpiIntent(intent_data) => {
                                intent_data.app_name.as_ref().map(|app_name| {
                                    match is_android {
                                        true => app_name.clone(),
                                        false => phonepe::map_ios_payment_source_to_target_app(Some(app_name))
                                            .unwrap_or_else(|| app_name.clone())
}
                                })
                            }
                            _ => None
};

                        if let Some(app_id) = app_id_opt {
                            headers.push((headers::X_MERCHANT_APP_ID.to_string(), app_id.into()));
                        }
                    }

                    if is_android {
                        headers.push((headers::X_MERCHANT_APP_SIGNATURE.to_string(), "".to_string().into()));
                    }
                }
                _ => {}
            }

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            let auth = phonepe::PhonepeAuthType::try_from(&req.connector_config)?;

            // Use merchant-based endpoint if merchant is IRCTC
            let api_endpoint = if phonepe::is_irctc_merchant(auth.merchant_id.peek()) {
                constants::API_IRCTC_PAY_ENDPOINT
            } else {
                constants::API_PAY_ENDPOINT
            };

            Ok(format!("{}{}", base_url, api_endpoint))
        }
    }
);

// PSync flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Phonepe,
    curl_request: Json(PhonepeSyncRequest),
    curl_response: PhonepeSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // Get base headers first
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
            ];

            // Build the request to get the checksum for X-VERIFY header
            let connector_router_data = PhonepeRouterData {
                connector: self.clone(),
                router_data: req
};
            let connector_req = PhonepeSyncRequest::try_from(&connector_router_data)?;

            // Get merchant ID for X-MERCHANT-ID header
            let auth = phonepe::PhonepeAuthType::try_from(&req.connector_config)?;

            headers.push((headers::X_VERIFY.to_string(), connector_req.checksum.into()));
            headers.push((headers::X_MERCHANT_ID.to_string(), auth.merchant_id.peek().to_string().into()));

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            let merchant_transaction_id = req.resource_common_data.get_reference_id()?;

            let auth = phonepe::PhonepeAuthType::try_from(&req.connector_config)?;
            let merchant_id = auth.merchant_id.peek();

            // Use merchant-based endpoint if merchant is IRCTC
            let api_endpoint = if phonepe::is_irctc_merchant(merchant_id) {
                constants::API_IRCTC_STATUS_ENDPOINT
            } else {
                constants::API_STATUS_ENDPOINT
            };

            Ok(format!("{base_url}{api_endpoint}/{merchant_id}/{merchant_transaction_id}"))
        }
    }
);

// Capture flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Phonepe,
    curl_request: Json(PhonepeCaptureRequest),
    curl_response: PhonepeCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/json".to_string().into(),
                ),
            ];

            let connector_router_data = PhonepeRouterData {
                connector: self.clone(),
                router_data: req,
            };
            let connector_req = PhonepeCaptureRequest::try_from(&connector_router_data)?;
            headers.push((headers::X_VERIFY.to_string(), connector_req.checksum.into()));

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{}{}", base_url, constants::API_CAPTURE_ENDPOINT))
        }
    }
);

// Void flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Phonepe,
    curl_request: Json(PhonepeVoidRequest),
    curl_response: PhonepeVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/json".to_string().into(),
                ),
            ];

            let connector_router_data = PhonepeRouterData {
                connector: self.clone(),
                router_data: req,
            };
            let connector_req = PhonepeVoidRequest::try_from(&connector_router_data)?;
            headers.push((headers::X_VERIFY.to_string(), connector_req.checksum.into()));

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{}{}", base_url, constants::API_VOID_ENDPOINT))
        }
    }
);

// Refund flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Phonepe,
    curl_request: Json(PhonepeRefundRequest),
    curl_response: PhonepeRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/json".to_string().into(),
                ),
            ];

            let connector_router_data = PhonepeRouterData {
                connector: self.clone(),
                router_data: req,
            };
            let connector_req = PhonepeRefundRequest::try_from(&connector_router_data)?;
            headers.push((headers::X_VERIFY.to_string(), connector_req.checksum.into()));

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{}{}", base_url, constants::API_REFUND_ENDPOINT))
        }
    }
);

// RSync flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Phonepe,
    curl_request: Json(PhonepeRefundSyncRequest),
    curl_response: PhonepeRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];

            let connector_router_data = PhonepeRouterData {
                connector: self.clone(),
                router_data: req,
            };
            let connector_req = PhonepeRefundSyncRequest::try_from(&connector_router_data)?;

            let auth = phonepe::PhonepeAuthType::try_from(&req.connector_config)?;

            headers.push((headers::X_VERIFY.to_string(), connector_req.checksum.into()));
            headers.push((headers::X_MERCHANT_ID.to_string(), auth.merchant_id.peek().to_string().into()));

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            let connector_refund_id = &req.request.connector_refund_id;

            let auth = phonepe::PhonepeAuthType::try_from(&req.connector_config)?;
            let merchant_id = auth.merchant_id.peek();

            Ok(format!(
                "{}{}/{}/{}/status",
                base_url,
                constants::API_REFUND_STATUS_ENDPOINT,
                merchant_id,
                connector_refund_id
            ))
        }
    }
);

// Type alias for non-generic trait implementations
// Implement ConnectorServiceTrait by virtue of implementing all required traits

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for Phonepe<T>
{
    fn id(&self) -> &'static str {
        "phonepe"
    }

    fn get_currency_unit(&self) -> enums::CurrencyUnit {
        enums::CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let _auth = phonepe::PhonepeAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass PhonePe credentials via x-connector-config with merchant_id, salt_key, and salt_index"
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developer.phonepe.com/v1/reference/credentials".to_string(),
                    ),
                    additional_context: Some(
                        "Expected ConnectorSpecificConfig::Phonepe with merchant_id, salt_key, and salt_index"
                            .to_string(),
                    ),
                },
            },
        )?;
        Ok(vec![(
            "Content-Type".to_string(),
            "application/json".to_string().into(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.phonepe.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Parse PhonePe error response (unified for both sync and payments)
        let (error_message, error_code, attempt_status) = if let Ok(error_response) =
            res.response
                .parse_struct::<phonepe::PhonepeErrorResponse>("PhonePe ErrorResponse")
        {
            let attempt_status = phonepe::get_phonepe_error_status(&error_response.code);
            (error_response.message, error_response.code, attempt_status)
        } else {
            let raw_response = String::from_utf8_lossy(&res.response);
            (
                "Unknown PhonePe error".to_string(),
                raw_response.to_string(),
                None,
            )
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: error_code,
            message: error_message.clone(),
            reason: Some(error_message),
            attempt_status,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Phonepe<T>
{
    fn get_supported_payment_methods(
        &self,
    ) -> Option<&'static domain_types::types::SupportedPaymentMethods> {
        None // TODO: Add UPI payment methods support
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [enums::EventClass]> {
        None // TODO: Add webhook support
    }

    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        None // TODO: Add connector info
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Phonepe,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateOrder,
        SetupMandate,
        RepeatPayment,
        PaymentMethodToken,
        MandateRevoke,
        ServerAuthenticationToken,
    ],
    not_supported: [
        IncrementalAuthorization,
        VoidPC,
        Accept,
        DefendDispute,
        SubmitEvidence,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
        ClientAuthenticationToken,
    ],
);
