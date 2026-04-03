use std::collections::HashMap;
use std::error::Error;

use crate::http_client::{
    merge_http_options, HttpClient, HttpOptions as NativeHttpOptions,
    HttpRequest as ClientHttpRequest, NetworkError,
};
use connector_service_ffi::types::{FfiMetadataPayload, FfiRequestData};
use connector_service_ffi::utils::ffi_headers_to_masked_metadata;
use domain_types::router_data::ConnectorSpecificConfig;
use domain_types::router_response_types::Response;
use domain_types::utils::ForeignTryFrom;
use grpc_api_types::payments::{
    ConnectorConfig, CustomerServiceCreateRequest, CustomerServiceCreateResponse,
    DisputeServiceAcceptRequest, DisputeServiceAcceptResponse, DisputeServiceDefendRequest,
    DisputeServiceDefendResponse, DisputeServiceSubmitEvidenceRequest,
    DisputeServiceSubmitEvidenceResponse, FfiOptions,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServiceAuthenticateResponse,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateResponse,
    PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateResponse, PaymentMethodServiceTokenizeRequest,
    PaymentMethodServiceTokenizeResponse, PaymentServiceAuthorizeRequest,
    PaymentServiceAuthorizeResponse, PaymentServiceCaptureRequest, PaymentServiceCaptureResponse,
    PaymentServiceCreateOrderRequest, PaymentServiceCreateOrderResponse, PaymentServiceGetRequest,
    PaymentServiceGetResponse, PaymentServiceProxyAuthorizeRequest,
    PaymentServiceProxySetupRecurringRequest, PaymentServiceRefundRequest,
    PaymentServiceReverseRequest, PaymentServiceReverseResponse,
    PaymentServiceSetupRecurringRequest, PaymentServiceSetupRecurringResponse,
    PaymentServiceTokenAuthorizeRequest, PaymentServiceTokenSetupRecurringRequest,
    PaymentServiceVoidRequest, PaymentServiceVoidResponse, RecurringPaymentServiceChargeRequest,
    RecurringPaymentServiceChargeResponse, RefundResponse, RequestConfig,
};

/// ConnectorClient — high-level Rust wrapper for the Connector Service.
///
/// Handles the full round-trip for any payment flow:
///   1. Build connector HTTP request via Rust core handlers
///   2. Execute the HTTP request via our standardized HttpClient (reqwest)
///   3. Parse the connector response via Rust core handlers
///
/// This client owns its primary connection pool (http_client).
pub struct ConnectorClient {
    http_client: HttpClient,
    config: ConnectorConfig,
    defaults: RequestConfig,
}

// ── Internal macro: generate a ConnectorClient method for a payment flow ──────
//
// Each generated method follows the same round-trip pattern:
//   1. Build FfiRequestData from caller inputs
//   2. Call the flow-specific req_handler to build the connector HTTP request
//   3. Execute HTTP via the shared HttpClient
//   4. Call the flow-specific res_handler to parse the response
//
// Usage: impl_flow_method!(method_name, ReqType, ResType, req_handler_fn, res_handler_fn);
macro_rules! impl_flow_method {
    ($method:ident, $req_type:ty, $res_type:ty, $req_handler:ident, $res_handler:ident) => {
        pub async fn $method(
            &self,
            request: $req_type,
            metadata: &HashMap<String, String>,
            options: Option<RequestConfig>,
        ) -> Result<$res_type, Box<dyn Error>> {
            use connector_service_ffi::handlers::payments::{$req_handler, $res_handler};

            let ffi_options = self.resolve_ffi_options(&options);
            let override_opts = options
                .as_ref()
                .and_then(|o| o.http.as_ref())
                .map(NativeHttpOptions::from);

            let ffi_request = build_ffi_request(request.clone(), metadata, &ffi_options)?;
            let environment = Some(grpc_api_types::payments::Environment::try_from(
                ffi_options.environment,
            )?);

            let connector_request = $req_handler(ffi_request, environment)
                .map_err(|e| format!("{} req_handler failed: {:?}", stringify!($method), e))?
                .ok_or("No connector request generated")?;

            let (body, boundary) = connector_request
                .body
                .as_ref()
                .map(|b| b.get_body_bytes())
                .transpose()
                .map_err(|e| format!("Body extraction failed: {e}"))?
                .unwrap_or((None, None));
            let mut headers = connector_request.get_headers_map();
            if let Some(boundary) = boundary {
                headers.insert(
                    "content-type".to_string(),
                    format!("multipart/form-data; boundary={}", boundary),
                );
            }
            let http_req = ClientHttpRequest {
                url: connector_request.url.clone(),
                method: connector_request.method,
                headers,
                body,
            };
            let http_response = self.http_client.execute(http_req, override_opts).await?;

            let mut header_map = http::HeaderMap::new();
            for (key, value) in &http_response.headers {
                if let Ok(name) = http::header::HeaderName::from_bytes(key.as_bytes()) {
                    if let Ok(val) = http::header::HeaderValue::from_bytes(value.as_bytes()) {
                        header_map.insert(name, val);
                    }
                }
            }
            let response = Response {
                headers: Some(header_map),
                response: bytes::Bytes::from(http_response.body),
                status_code: http_response.status_code,
            };

            let ffi_request_for_res = build_ffi_request(request, metadata, &ffi_options)?;
            match $res_handler(ffi_request_for_res, response, environment) {
                Ok(resp) => Ok(resp),
                Err(e) => Err(format!("{} failed: {:?}", stringify!($method), e).into()),
            }
        }
    };
}

impl ConnectorClient {
    /// Initialize a new ConnectorClient.
    ///
    /// # Arguments
    /// * `config` - The ConnectorConfig (connector_config with typed auth, options with environment).
    /// * `options` - Optional RequestConfig for default http/vault settings.
    pub fn new(
        config: ConnectorConfig,
        options: Option<RequestConfig>,
    ) -> Result<Self, NetworkError> {
        let defaults = options.unwrap_or_default();

        // Map the Protobuf options to native transport options
        let native_opts = match defaults.http.as_ref() {
            Some(http_proto) => NativeHttpOptions::from(http_proto),
            None => NativeHttpOptions::default(),
        };

        let http_client = HttpClient::new(native_opts)?;

        Ok(Self {
            http_client,
            config,
            defaults,
        })
    }

    /// Builds FfiOptions from config. Environment comes from SdkOptions (immutable).
    fn resolve_ffi_options(&self, _options: &Option<RequestConfig>) -> FfiOptions {
        let environment = self
            .config
            .options
            .as_ref()
            .map(|o| o.environment)
            .unwrap_or(0);
        FfiOptions {
            environment,
            connector_config: self.config.connector_config.clone(),
        }
    }

    /// Merges client defaults with per-request HTTP overrides. Per-request wins per field.
    fn resolve_http_options(&self, options: Option<&RequestConfig>) -> NativeHttpOptions {
        let base = self
            .defaults
            .http
            .as_ref()
            .map(NativeHttpOptions::from)
            .unwrap_or_default();
        let override_opts = options
            .and_then(|o| o.http.as_ref())
            .map(NativeHttpOptions::from)
            .unwrap_or_default();
        merge_http_options(&base, &override_opts)
    }

    /// Authorize a payment flow.
    ///
    /// # Arguments
    /// * `request` - The PaymentServiceAuthorizeRequest protobuf message.
    /// * `metadata` - Metadata map containing x-* headers for MaskedMetadata.
    /// * `options` - Optional RequestConfig for per-call overrides (http, vault).
    pub async fn authorize(
        &self,
        request: PaymentServiceAuthorizeRequest,
        metadata: &HashMap<String, String>,
        options: Option<RequestConfig>,
    ) -> Result<PaymentServiceAuthorizeResponse, Box<dyn Error>> {
        use connector_service_ffi::handlers::payments::{
            authorize_req_handler, authorize_res_handler,
        };

        // 1. Resolve final configuration
        let ffi_options = self.resolve_ffi_options(&options);
        let merged_http = self.resolve_http_options(options.as_ref());

        let ffi_request = build_ffi_request(request.clone(), metadata, &ffi_options)?;
        let environment = Some(grpc_api_types::payments::Environment::try_from(
            ffi_options.environment,
        )?);

        // 2. Build the connector HTTP request via core handler
        let connector_request = authorize_req_handler(ffi_request, environment)
            .map_err(|e| format!("authorize_req_handler failed: {:?}", e))?
            .ok_or("No connector request generated")?;

        // 3. Execute HTTP using the instance-owned client and potential overrides
        let (body, boundary) = connector_request
            .body
            .as_ref()
            .map(|b| b.get_body_bytes())
            .transpose()
            .map_err(|e| format!("Body extraction failed: {e}"))?
            .unwrap_or((None, None));
        let mut headers = connector_request.get_headers_map();

        if let Some(boundary) = boundary {
            headers.insert(
                "content-type".to_string(),
                format!("multipart/form-data; boundary={}", boundary),
            );
        }

        let http_req = ClientHttpRequest {
            url: connector_request.url.clone(),
            method: connector_request.method,
            headers,
            body,
        };

        let http_response = self
            .http_client
            .execute(http_req, Some(merged_http))
            .await?;

        // 4. Convert HTTP response to domain Response type
        let mut header_map = http::HeaderMap::new();
        for (key, value) in &http_response.headers {
            let key_bytes: &[u8] = key.as_bytes();
            let val_bytes: &[u8] = value.as_bytes();
            if let Ok(name) = http::header::HeaderName::from_bytes(key_bytes) {
                if let Ok(val) = http::header::HeaderValue::from_bytes(val_bytes) {
                    header_map.insert(name, val);
                }
            }
        }

        let response = Response {
            headers: Some(header_map),
            response: bytes::Bytes::from(http_response.body),
            status_code: http_response.status_code,
        };

        // 5. Parse response via core handler
        let ffi_request_for_res = build_ffi_request(request, metadata, &ffi_options)?;
        match authorize_res_handler(ffi_request_for_res, response, environment) {
            Ok(auth_response) => Ok(auth_response),
            Err(error_response) => {
                Err(format!("Authorization failed: {:?}", error_response).into())
            }
        }
    }

    // ── Payment flows (generated via impl_flow_method!) ──────────────────────

    impl_flow_method!(
        capture,
        PaymentServiceCaptureRequest,
        PaymentServiceCaptureResponse,
        capture_req_handler,
        capture_res_handler
    );
    impl_flow_method!(
        refund,
        PaymentServiceRefundRequest,
        RefundResponse,
        refund_req_handler,
        refund_res_handler
    );
    impl_flow_method!(
        void,
        PaymentServiceVoidRequest,
        PaymentServiceVoidResponse,
        void_req_handler,
        void_res_handler
    );
    impl_flow_method!(
        get,
        PaymentServiceGetRequest,
        PaymentServiceGetResponse,
        get_req_handler,
        get_res_handler
    );
    impl_flow_method!(
        setup_recurring,
        PaymentServiceSetupRecurringRequest,
        PaymentServiceSetupRecurringResponse,
        setup_recurring_req_handler,
        setup_recurring_res_handler
    );
    impl_flow_method!(
        reverse,
        PaymentServiceReverseRequest,
        PaymentServiceReverseResponse,
        reverse_req_handler,
        reverse_res_handler
    );
    impl_flow_method!(
        create_order,
        PaymentServiceCreateOrderRequest,
        PaymentServiceCreateOrderResponse,
        create_order_req_handler,
        create_order_res_handler
    );

    // ── Recurring / tokenization ──────────────────────────────────────────────
    // Note: the probe data uses "recurring_charge" as the flow key; the FFI handler is "charge".
    impl_flow_method!(
        recurring_charge,
        RecurringPaymentServiceChargeRequest,
        RecurringPaymentServiceChargeResponse,
        charge_req_handler,
        charge_res_handler
    );
    impl_flow_method!(
        tokenize,
        PaymentMethodServiceTokenizeRequest,
        PaymentMethodServiceTokenizeResponse,
        tokenize_req_handler,
        tokenize_res_handler
    );

    // ── Customer management ───────────────────────────────────────────────────
    // Note: the probe data uses "create_customer" as the flow key; the FFI handler is "create".
    impl_flow_method!(
        create_customer,
        CustomerServiceCreateRequest,
        CustomerServiceCreateResponse,
        create_req_handler,
        create_res_handler
    );

    // ── Dispute flows ─────────────────────────────────────────────────────────
    // Note: probe data keys are "dispute_accept", "dispute_defend", "dispute_submit_evidence".
    impl_flow_method!(
        dispute_accept,
        DisputeServiceAcceptRequest,
        DisputeServiceAcceptResponse,
        accept_req_handler,
        accept_res_handler
    );
    impl_flow_method!(
        dispute_defend,
        DisputeServiceDefendRequest,
        DisputeServiceDefendResponse,
        defend_req_handler,
        defend_res_handler
    );
    impl_flow_method!(
        dispute_submit_evidence,
        DisputeServiceSubmitEvidenceRequest,
        DisputeServiceSubmitEvidenceResponse,
        submit_evidence_req_handler,
        submit_evidence_res_handler
    );

    // ── Authentication flows ──────────────────────────────────────────────────
    impl_flow_method!(
        create_server_authentication_token,
        MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
        MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse,
        create_server_authentication_token_req_handler,
        create_server_authentication_token_res_handler
    );
    impl_flow_method!(
        create_server_session_authentication_token,
        MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
        MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse,
        create_server_session_authentication_token_req_handler,
        create_server_session_authentication_token_res_handler
    );
    impl_flow_method!(
        create_client_authentication_token,
        MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
        MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse,
        create_client_authentication_token_req_handler,
        create_client_authentication_token_res_handler
    );
    impl_flow_method!(
        pre_authenticate,
        PaymentMethodAuthenticationServicePreAuthenticateRequest,
        PaymentMethodAuthenticationServicePreAuthenticateResponse,
        pre_authenticate_req_handler,
        pre_authenticate_res_handler
    );
    impl_flow_method!(
        authenticate,
        PaymentMethodAuthenticationServiceAuthenticateRequest,
        PaymentMethodAuthenticationServiceAuthenticateResponse,
        authenticate_req_handler,
        authenticate_res_handler
    );
    impl_flow_method!(
        post_authenticate,
        PaymentMethodAuthenticationServicePostAuthenticateRequest,
        PaymentMethodAuthenticationServicePostAuthenticateResponse,
        post_authenticate_req_handler,
        post_authenticate_res_handler
    );

    // ── Non-PCI: Tokenized payment methods ────────────────────────────────────
    // For merchants holding connector-issued tokens (Stripe pm_xxx, Adyen stored
    // refs, etc.). No raw card data fields are present in the request types.
    impl_flow_method!(
        token_authorize,
        PaymentServiceTokenAuthorizeRequest,
        PaymentServiceAuthorizeResponse,
        token_authorize_req_handler,
        token_authorize_res_handler
    );
    impl_flow_method!(
        token_setup_recurring,
        PaymentServiceTokenSetupRecurringRequest,
        PaymentServiceSetupRecurringResponse,
        token_setup_recurring_req_handler,
        token_setup_recurring_res_handler
    );

    // ── Non-PCI: Proxied payment methods ──────────────────────────────────────
    // For merchants using VGS, Basis Theory, or Spreedly. Card proxy fields
    // hold vault alias tokens; the proxy substitutes real values before the
    // connector sees them. 3DS flows are supported because the proxy substitutes
    // aliases with the real PAN before forwarding to the 3DS server.
    impl_flow_method!(
        proxy_authorize,
        PaymentServiceProxyAuthorizeRequest,
        PaymentServiceAuthorizeResponse,
        proxy_authorize_req_handler,
        proxy_authorize_res_handler
    );
    impl_flow_method!(
        proxy_setup_recurring,
        PaymentServiceProxySetupRecurringRequest,
        PaymentServiceSetupRecurringResponse,
        proxy_setup_recurring_req_handler,
        proxy_setup_recurring_res_handler
    );
}

/// Internal helper to build the context-heavy FfiRequestData from raw inputs.
pub fn build_ffi_request<T>(
    payload: T,
    metadata: &HashMap<String, String>,
    options: &FfiOptions,
) -> Result<FfiRequestData<T>, Box<dyn Error>> {
    let proto_config = options
        .connector_config
        .as_ref()
        .ok_or("Missing connector_config in FfiOptions")?;

    let config_variant = proto_config
        .config
        .as_ref()
        .ok_or("Missing config variant in ConnectorSpecificConfig")?;

    let connector =
        domain_types::connector_types::ConnectorEnum::foreign_try_from(config_variant.clone())
            .map_err(|e| format!("Connector mapping failed: {e}"))?;

    let connector_config = ConnectorSpecificConfig::foreign_try_from(proto_config.clone())
        .map_err(|e| format!("Connector config mapping failed: {e}"))?;

    let masked_metadata = ffi_headers_to_masked_metadata(metadata)
        .map_err(|e| format!("Metadata mapping failed: {:?}", e))?;

    Ok(FfiRequestData {
        payload,
        extracted_metadata: FfiMetadataPayload {
            connector,
            connector_config,
        },
        masked_metadata: Some(masked_metadata),
    })
}
