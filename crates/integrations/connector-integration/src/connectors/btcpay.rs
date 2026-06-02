pub mod transformers;

use std::fmt::Debug;

use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Authorize, ServerSessionAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
    },
    errors,
    errors::IntegrationError,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
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
use transformers as btcpay;
use transformers::{
    BtcpayInvoiceResponse, BtcpayPaymentsRequest, BtcpaySessionTokenRequest,
    BtcpaySessionTokenResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> Btcpay<T> {
    /// Extracts the BTCPay store id, which is supplied per-payment via the
    /// request metadata (`{"store_id": "..."}`) or, as a fallback, via the
    /// connector auth config.
    fn get_store_id<F, Req, Res>(
        &self,
        req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        request_metadata: Option<&hyperswitch_masking::Secret<serde_json::Value>>,
    ) -> CustomResult<String, IntegrationError> {
        if let Some(metadata) = request_metadata {
            let parsed: btcpay::BtcpayMetadata = serde_json::from_value(metadata.clone().expose())
                .change_context(IntegrationError::InvalidConnectorConfig {
                    config: "metadata.store_id",
                    context: Default::default(),
                })?;
            return Ok(parsed.store_id);
        }

        let auth = btcpay::BtcpayAuthType::try_from(&req.connector_config)?;
        auth.store_id.map(|s| s.expose()).ok_or_else(|| {
            error_stack::report!(IntegrationError::InvalidConnectorConfig {
                config: "store_id",
                context: Default::default(),
            })
        })
    }
}

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Btcpay<T>
{
    fn id(&self) -> &'static str {
        "btcpay"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.btcpay.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = btcpay::BtcpayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        // BTCPay Greenfield uses `Authorization: token <API_KEY>` (NOT Bearer).
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("token {}", auth.api_key.expose()).into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: btcpay::BtcpayErrorResponse = res
            .response
            .parse_struct("BtcpayErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        with_error_response_body!(event_builder, response);

        let first = response.first();
        let code = first
            .and_then(|e| e.code.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
        let message = first
            .and_then(|e| e.message.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Btcpay<T>
{
}

// =============================================================================
// PREREQUISITES — Authorize + ClientSDK session token only
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Btcpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: BtcpayPaymentsRequest,
            response_body: BtcpayInvoiceResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: ServerSessionAuthenticationToken,
            request_body: BtcpaySessionTokenRequest,
            response_body: BtcpaySessionTokenResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
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
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.btcpay.base_url
        }
    }
);

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Btcpay,
    curl_request: Json(BtcpayPaymentsRequest),
    curl_response: BtcpayInvoiceResponse,
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
            let store_id = self.get_store_id(req, req.request.metadata.as_ref())?;
            Ok(format!(
                "{}/api/v1/stores/{}/invoices",
                self.connector_base_url_payments(req),
                store_id
            ))
        }
    }
);

// =============================================================================
// CLIENT SDK SESSION TOKEN FLOW (ServerSessionAuthenticationToken)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Btcpay,
    curl_request: Json(BtcpaySessionTokenRequest),
    curl_response: BtcpaySessionTokenResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let store_id = self.get_store_id(req, None)?;
            Ok(format!(
                "{}/api/v1/stores/{}/invoices",
                self.connector_base_url_payments(req),
                store_id
            ))
        }
    }
);

// =============================================================================
// MARKER TRAIT IMPLEMENTATIONS
// =============================================================================
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Btcpay<T>
{
}

// Authorize flow marker.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Btcpay<T>
{
}

// Client SDK session-token flow marker + enablement.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Btcpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Btcpay<T>
{
    fn should_do_session_token(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Btcpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Btcpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Btcpay<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Btcpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS (unimplemented flows) =====
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Btcpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PSync,
        PaymentMethodToken,
        VoidPC,
        Void,
        RSync,
        Refund,
        RepeatPayment,
        ServerAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);
