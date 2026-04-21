pub mod transformers;

use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self},
    decode::BodyDecoding,
    verification::SourceVerification,
};

use common_enums::CurrencyUnit;
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    PinelabsOnlineAccessTokenErrorResponse, PinelabsOnlineAccessTokenRequest,
    PinelabsOnlineAccessTokenResponse, PinelabsOnlineAuthorizeResponse,
    PinelabsOnlineCaptureRequest, PinelabsOnlineCaptureResponse, PinelabsOnlineCreateOrderResponse,
    PinelabsOnlineErrorResponse, PinelabsOnlineOrderRequest, PinelabsOnlinePSyncResponse,
    PinelabsOnlineRSyncResponse, PinelabsOnlineRefundRequest, PinelabsOnlineRefundResponse,
    PinelabsOnlineTransactionRequest, PinelabsOnlineVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// ========== Marker trait implementations ==========

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for PinelabsOnline<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }

    fn should_do_order_create(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for PinelabsOnline<T>
{
}

// ========== ConnectorCommon ==========

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for PinelabsOnline<T>
{
    fn id(&self) -> &'static str {
        "pinelabs_online"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.pinelabs_online.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        // Auth is handled via access token obtained from ServerAuthenticationToken flow.
        // Individual flow get_headers() methods extract the access token from resource_common_data.
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: PinelabsOnlineErrorResponse = res
            .response
            .parse_struct("PinelabsOnlineErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;
        with_error_response_body!(event_builder, response);
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code.unwrap_or_else(|| "UNKNOWN".to_string()),
            message: response
                .message
                .unwrap_or_else(|| "Unknown error".to_string()),
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// ========== Macro-generated struct and common functions ==========

macros::create_all_prerequisites!(
    connector_name: PinelabsOnline,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: PinelabsOnlineAccessTokenRequest,
            response_body: PinelabsOnlineAccessTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: PinelabsOnlineOrderRequest,
            response_body: PinelabsOnlineCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: PinelabsOnlineTransactionRequest,
            response_body: PinelabsOnlineAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PinelabsOnlinePSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PinelabsOnlineCaptureRequest,
            response_body: PinelabsOnlineCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: PinelabsOnlineVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PinelabsOnlineRefundRequest,
            response_body: PinelabsOnlineRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: PinelabsOnlineRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers(
            &self,
            access_token: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        {
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.pinelabs_online.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.pinelabs_online.base_url
        }
    }
);

// ========== ServerAuthenticationToken Flow ==========

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: PinelabsOnline,
    curl_request: Json(PinelabsOnlineAccessTokenRequest),
    curl_response: PinelabsOnlineAccessTokenResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            // The auth token endpoint shares the same host as the payments API but uses a
            // different path prefix (/api/auth/v1/token vs /api/pay/v1/...).  Since the
            // connector config only exposes a single `base_url` (the payments base), we
            // derive the token URL by replacing the known path segment.
            //
            // Risk: if the configured base_url does not contain "/api/pay/v1" (e.g. a custom
            // sandbox URL), this replacement will be a no-op and the request will be sent to
            // the wrong endpoint.  A dedicated `auth_url` config field would be safer but is
            // not currently available in the connector config schema.
            let base_url = self.connector_base_url_payments(req);
            let token_url = base_url.replace("/api/pay/v1", "/api/auth/v1/token");
            Ok(token_url)
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
            let response: PinelabsOnlineAccessTokenErrorResponse = res
                .response
                .parse_struct("PinelabsOnlineAccessTokenErrorResponse")
                .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

            with_error_response_body!(event_builder, response);

            let error_code = response
                .error
                .clone()
                .or_else(|| response.trace_id.clone())
                .unwrap_or_else(|| "UNKNOWN_ERROR".to_string());
            let error_message = response
                .error
                .or(response.message)
                .unwrap_or_else(|| "Unknown error".to_string());

            Ok(ErrorResponse {
                status_code: res.status_code,
                code: error_code,
                message: error_message,
                reason: response.error_description,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        }
    }
);

// ========== CreateOrder Flow (Phase 1: POST /orders) ==========

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PinelabsOnline,
    curl_request: Json(PinelabsOnlineOrderRequest),
    curl_response: PinelabsOnlineCreateOrderResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req
                .resource_common_data
                .access_token
                .clone()
                .ok_or(errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            self.build_headers(&access_token.access_token.expose())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/orders", self.connector_base_url_payments(req)))
        }
    }
);

// ========== Authorize Flow (Phase 2: POST /orders/{order_id}/payments) ==========

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PinelabsOnline,
    curl_request: Json(PinelabsOnlineTransactionRequest),
    curl_response: PinelabsOnlineAuthorizeResponse,
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
            let access_token = req
                .resource_common_data
                .access_token
                .clone()
                .ok_or(errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            self.build_headers(&access_token.access_token.expose())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            // Phase 2: read order_id from reference_id (set by CreateOrder response via caller)
            let order_id = req
                .resource_common_data
                .reference_id
                .as_ref()
                .ok_or(errors::IntegrationError::MissingRequiredField {
                    field_name: "reference_id (order_id from CreateOrder)",
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}/orders/{}/payments",
                self.connector_base_url_payments(req),
                order_id
            ))
        }
    }
);

// ========== PSync Flow ==========

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PinelabsOnline,
    curl_response: PinelabsOnlinePSyncResponse,
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
            let access_token = req
                .resource_common_data
                .access_token
                .clone()
                .ok_or(errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            self.build_headers(&access_token.access_token.expose())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let connector_transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}/orders/{}",
                self.connector_base_url_payments(req),
                connector_transaction_id
            ))
        }
    }
);

// ========== Capture Flow ==========

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PinelabsOnline,
    curl_request: Json(PinelabsOnlineCaptureRequest),
    curl_response: PinelabsOnlineCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Put,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req
                .resource_common_data
                .access_token
                .clone()
                .ok_or(errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            self.build_headers(&access_token.access_token.expose())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let connector_transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}/orders/{}/capture",
                self.connector_base_url_payments(req),
                connector_transaction_id
            ))
        }
    }
);

// ========== Void Flow ==========
// Void sends PUT /orders/{order_id}/cancel with no request body

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PinelabsOnline,
    curl_response: PinelabsOnlineVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Put,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req
                .resource_common_data
                .access_token
                .clone()
                .ok_or(errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            self.build_headers(&access_token.access_token.expose())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            // `PaymentVoidData.connector_transaction_id` is a plain String (unlike
            // `PaymentsCaptureData` which wraps it in `ResponseId`), so no unwrapping
            // is needed.  We clone it for consistency with the Capture and PSync patterns.
            let connector_transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}/orders/{}/cancel",
                self.connector_base_url_payments(req),
                connector_transaction_id
            ))
        }
    }
);

// ========== Refund Flow ==========

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PinelabsOnline,
    curl_request: Json(PinelabsOnlineRefundRequest),
    curl_response: PinelabsOnlineRefundResponse,
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
            let access_token = req
                .resource_common_data
                .access_token
                .clone()
                .ok_or(errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            self.build_headers(&access_token.access_token.expose())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let connector_payment_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}/refunds/{}",
                self.connector_base_url_refunds(req),
                connector_payment_id,
            ))
        }
    }
);

// ========== RSync (Refund Sync) Flow ==========
// GET /api/pay/v1/orders/reference/{merchant_order_reference}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: PinelabsOnline,
    curl_response: PinelabsOnlineRSyncResponse,
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
            let access_token = req
                .resource_common_data
                .access_token
                .clone()
                .ok_or(errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            self.build_headers(&access_token.access_token.expose())
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!(
                "{}/orders/reference/{}",
                self.connector_base_url_refunds(req),
                refund_id,
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for PinelabsOnline<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: PinelabsOnline,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutTransfer,
        PayoutGet,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);
