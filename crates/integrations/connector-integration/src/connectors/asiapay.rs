pub mod transformers;

use bytes::Bytes;
use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult,
    events,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundsData,
        RefundsResponseData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use std::fmt::Debug;
use transformers::AsiaPayRedirectResponse;

pub(crate) type AsiaPayPaymentRequest = transformers::AsiaPayPaymentRequest;
pub(crate) type AsiaPayCaptureRequest = transformers::AsiaPayMerchantApiRequest;
pub(crate) type AsiaPayCaptureResponse = transformers::AsiaPayMerchantApiResponse;
pub(crate) type AsiaPayVoidRequest = transformers::AsiaPayMerchantApiRequest;
pub(crate) type AsiaPayVoidResponse = transformers::AsiaPayMerchantApiResponse;
pub(crate) type AsiaPayRefundRequest = transformers::AsiaPayMerchantApiRequest;
pub(crate) type AsiaPayRefundResponse = transformers::AsiaPayMerchantApiResponse;
pub(crate) type AsiaPaySyncRequest = transformers::AsiaPaySyncRequest;
pub(crate) type AsiaPayPSyncResponse = transformers::AsiaPayMerchantApiResponse;

use super::macros;
use crate::{
    types::ResponseRouterData, with_error_response_body,
};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// ============================================================================
// CONNECTOR COMMON
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Asiapay<T>
{
    fn id(&self) -> &'static str {
        "asiapay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.asiapay.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response_str = String::from_utf8_lossy(&res.response);
        let response: transformers::AsiaPayErrorResponse =
            serde_qs::from_str(&response_str).unwrap_or(transformers::AsiaPayErrorResponse {
                error_code: Some(res.status_code.to_string()),
                error_message: Some(response_str.to_string()),
            });

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_else(|| "UNKNOWN".to_string()),
            message: response
                .error_message
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string()),
            reason: response.error_message,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// ============================================================================
// PREREQUISITES
// ============================================================================

macros::create_all_prerequisites!(
    connector_name: Asiapay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AsiaPayPaymentRequest,
            response_body: AsiaPayRedirectResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: AsiaPayCaptureRequest,
            response_body: AsiaPayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AsiaPayVoidRequest,
            response_body: AsiaPayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AsiaPayRefundRequest,
            response_body: AsiaPayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AsiaPaySyncRequest,
            response_body: AsiaPayPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: Bytes,
            status_code: u16,
        ) -> Result<Bytes, ConnectorError> {
            let response_str = String::from_utf8(bytes.to_vec()).map_err(|_| {
                crate::utils::response_deserialization_fail(
                    status_code,
                    "asiapay: response body is not valid UTF-8",
                )
            })?;

            let parsed: std::collections::HashMap<String, String> =
                serde_qs::from_str(&response_str).map_err(|_| {
                    crate::utils::response_deserialization_fail(
                        status_code,
                        "asiapay: response body did not match the expected URL-encoded format",
                    )
                })?;

            let json_bytes = serde_json::to_vec(&parsed).map_err(|_| {
                crate::utils::response_deserialization_fail(
                    status_code,
                    "asiapay: failed to convert response to JSON",
                )
            })?;

            Ok(Bytes::from(json_bytes))
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.asiapay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.asiapay.base_url
        }
    }
);

// ============================================================================
// TRAIT IMPLEMENTATIONS
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Asiapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Asiapay<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Asiapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ============================================================================
// AUTHORIZE FLOW (manual implementation — redirect-based)
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    > for Asiapay<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}/payment/payForm.jsp",
            self.connector_base_url_payments(req)
        ))
    }

    fn get_request_body(
        &self,
        _req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        Ok(None)
    }

    fn build_request_v2(
        &self,
        _req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        Ok(None)
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        _event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        let response = AsiaPayRedirectResponse {
            success_code: None,
            order_ref: None,
            pay_ref: None,
            amt: None,
            cur: None,
            err_msg: None,
            order_status: None,
            prc: None,
            src: None,
            auth_id: None,
            secure_hash: None,
            payer_auth_status: None,
        };

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "asiapay",
        ))
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

// ============================================================================
// CAPTURE FLOW
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiaPayCaptureRequest),
    curl_response: AsiaPayCaptureResponse,
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
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/merchant/api/orderApi.jsp",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ============================================================================
// VOID FLOW
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiaPayVoidRequest),
    curl_response: AsiaPayVoidResponse,
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
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/merchant/api/orderApi.jsp",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ============================================================================
// REFUND FLOW
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiaPayRefundRequest),
    curl_response: AsiaPayRefundResponse,
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
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/merchant/api/orderApi.jsp",
                self.connector_base_url_refunds(req)
            ))
        }
    }
);

// ============================================================================
// PSYNC FLOW
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Asiapay,
    curl_request: FormUrlEncoded(AsiaPaySyncRequest),
    curl_response: AsiaPayPSyncResponse,
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
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/merchant/api/orderApi.jsp",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ============================================================================
// UNSUPPORTED FLOWS
// ============================================================================

macros::macro_connector_flow_status_impls!(
    connector: Asiapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PaymentMethodToken,
        VoidPC,
        MandateRevoke,
        RSync,
    ],
    not_supported: [
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        CreateOrder,
        CreateConnectorCustomer,
        ClientAuthenticationToken,
        IncrementalAuthorization,
        SubmitEvidence,
        DefendDispute,
        Accept,
        RepeatPayment,
        SetupMandate,
    ],
);
