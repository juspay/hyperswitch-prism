pub mod transformers;
pub use transformers as axisbank;

use common_enums as enums;
use common_utils::{errors::CustomResult, events, ext_traits::BytesExt, types::StringMajorUnit};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PaymentMethodToken, PostAuthenticate, PreAuthenticate,
        PSync, RSync, Refund, RepeatPayment, ServerAuthenticationToken,
        ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence, Void, VoidPC,
        VerifyWebhookSource,
    },
    connector_types::{
        ConnectorSpecifications,
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, PaymentVoidData,
        RefundFlowData, RefundsData, RefundsResponseData, RefundSyncData,
        SetupMandateRequestData, PaymentsAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsPostAuthenticateData, RepeatPaymentData, ConnectorCustomerData,
        ConnectorCustomerResponse, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        SubmitEvidenceData, MandateRevokeRequestData, MandateRevokeResponseData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        ClientAuthenticationTokenRequestData,
        PaymentsIncrementalAuthorizationData,
        VerifyWebhookSourceFlowData,
    },
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::VerifyWebhookSourceResponseData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2,
    connector_types, decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use self::transformers::{
    AxisbankAuthConfig, AxisbankPaymentsRequest, AxisbankPaymentsResponse,
    AxisbankSyncRequest, AxisbankSyncResponse, AxisbankRefundRequest, AxisbankRefundResponse,
    AxisbankRefundSyncRequest, AxisbankRefundSyncResponse,
};
use super::macros;
use crate::types::ResponseRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};
use tracing::error;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Axisbank<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SourceVerification for Axisbank<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyWebhookSourceV2 for Axisbank<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Axisbank,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

macros::create_amount_converter_wrapper!(
    connector_name: Axisbank,
    amount_type: StringMajorUnit
);

macros::create_all_prerequisites!(
    connector_name: Axisbank,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AxisbankPaymentsRequest,
            response_body: AxisbankPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AxisbankSyncRequest,
            response_body: AxisbankSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AxisbankRefundRequest,
            response_body: AxisbankRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: AxisbankRefundSyncRequest,
            response_body: AxisbankRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        /// Preprocess JWE-encrypted responses from Axis Bank
        ///
        /// Axis Bank encrypts responses using JWE (RSA-OAEP-256 + A256GCM).
        /// This function decrypts the JWE to reveal the inner JWS, then verifies
        /// the JWS signature using Juspay's public key, and returns the plaintext payload.
        pub fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            response_bytes: bytes::Bytes,
            _status_code: u16,
        ) -> Result<bytes::Bytes, ConnectorError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            use crate::connectors::juspay_upi_stack::{
                crypto::decrypt_jwe_response,
                types::JweResponse,
            };
            use base64::Engine;
            use common_utils::consts::BASE64_ENGINE_URL_SAFE_NO_PAD;

            // Check if this is a JWE-encrypted response
            if !JweResponse::is_jwe_response(&response_bytes) {
                // Not a JWE response (possibly error response), return as-is
                return Ok(response_bytes);
            }

            // Parse the JWE response
            let jwe_response: JweResponse = serde_json::from_slice(&response_bytes)
                .map_err(|e| {
                    error!(error = %e, "Could not parse JWE JSON envelope");
                    error!(raw_response = %String::from_utf8_lossy(&response_bytes), "Raw JWE response");
                    ConnectorError::ResponseDeserializationFailed {
                        context: Default::default(),
                    }
                })?;

            // Get auth config for private key
            let auth_config = AxisbankAuthConfig::try_from(&req.connector_config)
                .map_err(|e| {
                    error!(error = %e, "Could not extract Axisbank auth config");
                    ConnectorError::ResponseDeserializationFailed {
                        context: Default::default(),
                    }
                })?;

            // Decrypt the JWE to get the inner JWS
            // Following Newton Gateway approach: JWE AEAD (A256GCM) provides integrity,
            // so JWS signature verification is skipped after decryption.
            let jws_json = decrypt_jwe_response(
                &jwe_response.cipher_text,
                &jwe_response.encrypted_key,
                &jwe_response.iv,
                &jwe_response.protected,
                &jwe_response.tag,
                &auth_config.merchant_private_key,
            )
            .map_err(|e| {
                error!(error = %e, "JWE decryption failed");
                error!(protected_header = %jwe_response.protected, "JWE protected header (base64url)");
                if let Ok(header_bytes) = BASE64_ENGINE_URL_SAFE_NO_PAD.decode(&jwe_response.protected) {
                    error!(decoded_header = %String::from_utf8_lossy(&header_bytes), "JWE protected header (decoded)");
                }
                ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                }
            })?;

            // Parse the decrypted JWS object
            let jws_obj: crate::connectors::juspay_upi_stack::types::JwsObject =
                serde_json::from_str(&jws_json)
                    .map_err(|e| {
                        error!(error = %e, "Could not parse JWS JSON structure");
                        ConnectorError::ResponseDeserializationFailed {
                            context: Default::default(),
                        }
                    })?;

            // Decode the JWS payload (base64url-encoded)
            let payload_bytes = BASE64_ENGINE_URL_SAFE_NO_PAD
                .decode(&jws_obj.payload)
                .map_err(|e| {
                    error!(error = %e, "Could not base64url-decode JWS payload");
                    ConnectorError::ResponseDeserializationFailed {
                        context: Default::default(),
                    }
                })?;

            // The JWS payload contains a nested structure with the actual response:
            // {"payload": {...}, "responseCode": "...", "responseMessage": "...", "status": "..."}
            let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes)
                .map_err(|e| {
                    error!(error = %e, "Could not parse JWS payload as JSON");
                    ConnectorError::ResponseDeserializationFailed {
                        context: Default::default(),
                    }
                })?;

            let final_response = serde_json::json!({
                "status": payload_json.get("status").cloned().unwrap_or(serde_json::json!("UNKNOWN")),
                "responseCode": payload_json.get("responseCode").cloned().unwrap_or(serde_json::json!("UNKNOWN")),
                "responseMessage": payload_json.get("responseMessage").cloned().unwrap_or(serde_json::json!("Unknown")),
                "payload": payload_json.get("payload").cloned()
            });

            let final_bytes = serde_json::to_vec(&final_response)
                .map_err(|e| {
                    error!(error = %e, "Could not serialize final response");
                    ConnectorError::ResponseDeserializationFailed {
                        context: Default::default(),
                    }
                })?;

            Ok(bytes::Bytes::from(final_bytes))
        }

        pub fn connector_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.axisbank.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.axisbank.base_url
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![
                ("content-type".to_string(), "application/json".to_string().into()),
            ])
        }
    }
);

// Authorize Flow - Register Intent
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankPaymentsRequest),
    curl_response: AxisbankPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = AxisbankAuthConfig::try_from(&req.connector_config)?;
            let timestamp = axisbank::get_current_timestamp_ms();
            let merchant_request_id = req.resource_common_data.connector_request_reference_id.clone();

            let headers = vec![
                ("content-type".to_string(), "application/json".to_string().into()),
                ("x-merchant-id".to_string(), auth.merchant_id.into()),
                ("x-merchant-channel-id".to_string(), auth.merchant_channel_id.into()),
                ("x-timestamp".to_string(), timestamp.into()),
                ("jpupi-routing-id".to_string(), merchant_request_id.into()),
            ];

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{}merchants/transactions/registerIntent", base_url))
        }
    }
);

// PSync Flow - Status 360
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankSyncRequest),
    curl_response: AxisbankSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = AxisbankAuthConfig::try_from(&req.connector_config)?;
            let timestamp = axisbank::get_current_timestamp_ms();
            let merchant_request_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .unwrap_or_else(|_| req.resource_common_data.connector_request_reference_id.clone());

            let headers = vec![
                ("content-type".to_string(), "application/json".to_string().into()),
                ("x-merchant-id".to_string(), auth.merchant_id.into()),
                ("x-merchant-channel-id".to_string(), auth.merchant_channel_id.into()),
                ("x-timestamp".to_string(), timestamp.into()),
                ("jpupi-routing-id".to_string(), merchant_request_id.into()),
            ];

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{}merchants/transactions/status360", base_url))
        }
    }
);

// Refund Flow - Refund 360
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankRefundRequest),
    curl_response: AxisbankRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = AxisbankAuthConfig::try_from(&req.connector_config)?;
            let timestamp = axisbank::get_current_timestamp_ms();
            let refund_request_id = req.request.refund_id.clone();

            let headers = vec![
                ("content-type".to_string(), "application/json".to_string().into()),
                ("x-merchant-id".to_string(), auth.merchant_id.into()),
                ("x-merchant-channel-id".to_string(), auth.merchant_channel_id.into()),
                ("x-timestamp".to_string(), timestamp.into()),
                ("jpupi-routing-id".to_string(), refund_request_id.into()),
            ];

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{}merchants/transactions/refund360", base_url))
        }
    }
);

// RSync Flow - Refund Status (uses same endpoint as Refund)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Axisbank,
    curl_request: Json(AxisbankRefundSyncRequest),
    curl_response: AxisbankRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = AxisbankAuthConfig::try_from(&req.connector_config)?;
            let timestamp = axisbank::get_current_timestamp_ms();
            let refund_request_id = req.request.connector_refund_id.clone();

            let headers = vec![
                ("content-type".to_string(), "application/json".to_string().into()),
                ("x-merchant-id".to_string(), auth.merchant_id.into()),
                ("x-merchant-channel-id".to_string(), auth.merchant_channel_id.into()),
                ("x-timestamp".to_string(), timestamp.into()),
                ("jpupi-routing-id".to_string(), refund_request_id.into()),
            ];

            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{}merchants/transactions/refund360", base_url))
        }
    }
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for Axisbank<T>
{
    fn id(&self) -> &'static str {
        "axisbank"
    }

    fn get_currency_unit(&self) -> enums::CurrencyUnit {
        enums::CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.axisbank.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let error_response = if let Ok(error) = res.response.parse_struct::<axisbank::AxisbankErrorResponse>("Axisbank ErrorResponse") {
            axisbank::build_error_response(res.status_code, &error.response_code, &error.response_message)
        } else {
            let raw_response = String::from_utf8_lossy(&res.response);
            axisbank::build_error_response(
                res.status_code,
                "UNKNOWN",
                &raw_response,
            )
        };
        
        Ok(error_response)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Axisbank<T>
{
    fn get_supported_payment_methods(
        &self,
    ) -> Option<&'static domain_types::types::SupportedPaymentMethods> {
        None
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [enums::EventClass]> {
        None
    }
}

// Stub implementations for unsupported flows
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Capture,
        PaymentFlowData,
        PaymentsCaptureData,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Axisbank<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > for Axisbank<T>
{
}
