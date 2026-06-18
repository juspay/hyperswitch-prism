pub mod profile;
pub mod raw_inputs;
pub mod rules;
pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void, VoidPC,
        VoidPostRefund,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundVoidPostRefundData, RefundsData, RefundsResponseData,
        RepeatPaymentData, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
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
    self as tsys_transit, TsysTransitAuthorizeRequest, TsysTransitAuthorizeResponse,
    TsysTransitCaptureRequest, TsysTransitCaptureResponse, TsysTransitCardAuthenticationRequest,
    TsysTransitCardAuthenticationResponse, TsysTransitRSyncRequest, TsysTransitRSyncResponse,
    TsysTransitRepeatPaymentRequest, TsysTransitRepeatPaymentResponse, TsysTransitReturnRequest,
    TsysTransitReturnResponse, TsysTransitTransactionInquiryRequest,
    TsysTransitTransactionInquiryResponse, TsysTransitVoidPCRequest, TsysTransitVoidPCResponse,
    TsysTransitVoidPostRefundRequest, TsysTransitVoidPostRefundResponse, TsysTransitVoidRequest,
    TsysTransitVoidResponse,
};

use super::macros::{self, GetSoapXml};
use crate::{types::ResponseRouterData, utils, with_error_response_body};

const CONTENT_TYPE_XML: &str = "text/xml";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// =============================================================================
// AMOUNT CONVERTER
// =============================================================================
// TransIT expects amounts as a decimal string in major currency units (e.g. "1.25").
macros::create_amount_converter_wrapper!(connector_name: TsysTransit, amount_type: StringMajorUnit);

// =============================================================================
// CONNECTOR STRUCT + PREREQUISITES
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: TsysTransit,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: TsysTransitAuthorizeRequest,
            response_body: TsysTransitAuthorizeResponse,
            response_format: xml,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: TsysTransitTransactionInquiryRequest,
            response_body: TsysTransitTransactionInquiryResponse,
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: TsysTransitCaptureRequest,
            response_body: TsysTransitCaptureResponse,
            response_format: xml,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TsysTransitReturnRequest,
            response_body: TsysTransitReturnResponse,
            response_format: xml,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: TsysTransitRSyncRequest,
            response_body: TsysTransitRSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: VoidPostRefund,
            request_body: TsysTransitVoidPostRefundRequest,
            response_body: TsysTransitVoidPostRefundResponse,
            response_format: xml,
            router_data: RouterDataV2<VoidPostRefund, RefundFlowData, RefundVoidPostRefundData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: TsysTransitVoidRequest,
            response_body: TsysTransitVoidResponse,
            response_format: xml,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: TsysTransitVoidPCRequest,
            response_body: TsysTransitVoidPCResponse,
            response_format: xml,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: TsysTransitCardAuthenticationRequest,
            response_body: TsysTransitCardAuthenticationResponse,
            response_format: xml,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: TsysTransitRepeatPaymentRequest,
            response_body: TsysTransitRepeatPaymentResponse,
            response_format: xml,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.tsys_transit.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.tsys_transit.base_url
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for TsysTransit<T>
{
    fn id(&self) -> &'static str {
        "tsysTransit"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // TransIT expects amounts in major units (decimal string, e.g. "1.25").
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.tsys_transit.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // TransIT uses body-based authentication (deviceID / transactionKey /
        // developerID are flattened into the XML payload). No HTTP auth headers.
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: tsys_transit::TsysTransitErrorResponse = res
            .response
            .parse_struct("TsysTransitErrorResponse")
            .change_context(utils::response_deserialization_fail(
                res.status_code,
                "tsysTransit: response body did not match the expected format; confirm API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .response_code
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            message: response
                .response_message
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: response.response_message,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// =============================================================================
// REQUIRED MARKER TRAITS
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> TsysTransit<T> {
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _req: &RouterDataV2<F, FCD, Req, Res>,
        bytes: bytes::Bytes,
        status_code: u16,
    ) -> CustomResult<bytes::Bytes, IntegrationError> {
        let raw_response_xml = String::from_utf8_lossy(bytes.as_ref());
        tracing::info!(
            connector = "tsysTransit",
            http_status = status_code,
            raw_response_xml = %raw_response_xml,
            "tsysTransit raw connector response"
        );
        Ok(bytes)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for TsysTransit<T>
{
}

// XML connectors mirror worldpayxml: keep `BodyDecoding` at the default
// (NoAlgorithm) for now — the response body is parsed via the XML response
// pattern in the macro layer, not via this trait.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for TsysTransit<T>
{
}

// Authorize is the only payments-flow currently wired; the remaining
// trait-marker impls stay in the `macro_connector_flow_status_impls!`
// `not_implemented` block below.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundVoidPostRefundV2 for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for TsysTransit<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for TsysTransit<T>
{
}

// RepeatPayment (MIT replay) reuses the Authorize XML shape — Path A / Path B
// dispatch is handled inside the request transformer via `decode_mandate_dispatch`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for TsysTransit<T>
{
}

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitAuthorizeRequest),
    curl_response: TsysTransitAuthorizeResponse,
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
            // TransIT auth lives in the request body; only Content-Type is required.
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TransIT exposes a single POST `/` endpoint that dispatches on the
            // XML root element (tech spec § Sequence Diagrams).
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// PSYNC FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitTransactionInquiryRequest),
    curl_response: TsysTransitTransactionInquiryResponse,
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
            // TransIT auth lives in the request body; only Content-Type is required.
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TransIT exposes a single POST `/` endpoint that dispatches on the
            // XML root element (tech spec § Sequence Diagrams).
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// CAPTURE FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitCaptureRequest),
    curl_response: TsysTransitCaptureResponse,
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
            // TransIT auth lives in the request body; only Content-Type is required.
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TransIT exposes a single POST `/` endpoint that dispatches on the
            // XML root element (tech spec § Sequence Diagrams).
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// REFUND FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitReturnRequest),
    curl_response: TsysTransitReturnResponse,
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
            // TransIT auth lives in the request body; only Content-Type is required.
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TransIT exposes a single POST `/` endpoint that dispatches on the
            // XML root element (tech spec § Sequence Diagrams).
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// =============================================================================
// RSYNC FLOW
// =============================================================================
//
// TransIT refunds are sync-final on the `<ReturnResponse>` (no separate
// refund-status-poll endpoint). However, HS still dispatches RSync to verify
// terminal status, so we reuse the PSync request/response shape
// (`<TransactionInquiry>`) and map the response to `RefundStatus` instead of
// `AttemptStatus`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitRSyncRequest),
    curl_response: TsysTransitRSyncResponse,
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
            // TransIT auth lives in the request body; only Content-Type is required.
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TransIT exposes a single POST `/` endpoint that dispatches on the
            // XML root element (tech spec § Sequence Diagrams).
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// =============================================================================
// REFUND REVERSE FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitVoidPostRefundRequest),
    curl_response: TsysTransitVoidPostRefundResponse,
    flow_name: VoidPostRefund,
    resource_common_data: RefundFlowData,
    flow_request: RefundVoidPostRefundData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<VoidPostRefund, RefundFlowData, RefundVoidPostRefundData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<VoidPostRefund, RefundFlowData, RefundVoidPostRefundData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// =============================================================================
// VOID FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitVoidRequest),
    curl_response: TsysTransitVoidResponse,
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
            // TransIT auth lives in the request body; only Content-Type is required.
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TransIT exposes a single POST `/` endpoint that dispatches on the
            // XML root element (tech spec § Sequence Diagrams).
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// POST-CAPTURE VOID FLOW
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitVoidPCRequest),
    curl_response: TsysTransitVoidPCResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// CREATE CONNECTOR CUSTOMER FLOW — TransIT `<AddCustomer>` (vault setup, Path B)
// =============================================================================
// =============================================================================
// SETUP MANDATE FLOW — TransIT `<CardAuthentication>` (zero-dollar CIT verify)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitCardAuthenticationRequest),
    curl_response: TsysTransitCardAuthenticationResponse,
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
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// REPEAT PAYMENT FLOW — TransIT `<Sale>` / `<Auth>` replay (MIT)
// =============================================================================
//
// TransIT does not expose a distinct "RecurringCharge" endpoint — MIT replays
// fire the same `<Sale>` (auto-capture) / `<Auth>` (manual capture) XML against
// the same POST `/` endpoint. The request transformer (`TryFrom<&TsysTransitRouterData
// <RouterDataV2<RepeatPayment, ..., RepeatPaymentData<T>, ...>>>`) converts the
// upstream `RepeatPaymentData` into the same `TsysTransitAuthorizeRequest` body that
// the Authorize flow emits, with the mandate id propagated so the existing
// `decode_mandate_dispatch()` logic picks Path A (NTID) or Path B (vault).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: TsysTransit,
    curl_request: SoapXml(TsysTransitRepeatPaymentRequest),
    curl_response: TsysTransitRepeatPaymentResponse,
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
            Ok(vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// FLOW STATUS IMPLEMENTATIONS — remaining flows are scaffolded as `not_implemented`.
// =============================================================================
macros::macro_connector_payout_implementation!(
    connector: TsysTransit,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

macros::macro_connector_flow_status_impls!(
    connector: TsysTransit,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateConnectorCustomer,
        CreateOrder,
        IncrementalAuthorization,
        PaymentMethodToken,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        MandateRevoke,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        SubmitEvidence,
        DefendDispute,
    ],
    not_supported: [
        Accept,
    ],
);
