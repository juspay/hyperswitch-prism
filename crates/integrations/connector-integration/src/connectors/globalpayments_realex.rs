//! Global Payments **Ecommerce XML API** (legacy *Realex Payments* "XML API" / "Remote API").
//!
//! Not to be confused with the [`super::globalpay`] connector, which integrates the modern GP-API
//! JSON product (`apis.sandbox.globalpay.com/ucp`, OAuth bearer). This one is the older CGI
//! endpoint that speaks XML in both directions and authenticates with a two-stage SHA-1 digest.
//! The two products share no types, transformers, status maps or configuration.
//!
//! A single endpoint serves every operation; the operation is chosen by the `type` attribute on
//! the `<request>` root element: `auth` (Authorize), `settle` (Capture), `void` (Void), `rebate`
//! (Refund) and `query` (PSync and RSync). There is no refund-status request type: RSync reuses
//! `query`, addressing the synthetic `_rebate_<orderid>` transaction the gateway stores for a
//! successful rebate (tech spec §12.6). A `query` on the *original* order id only ever echoes the
//! authorization leg, which is why PSync cannot see refund state — but the rebate leg can.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData,
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
    GlobalpaymentsRealexCaptureRequest, GlobalpaymentsRealexCaptureResponse,
    GlobalpaymentsRealexPSyncRequest, GlobalpaymentsRealexPSyncResponse,
    GlobalpaymentsRealexPaymentsRequest, GlobalpaymentsRealexPaymentsResponse,
    GlobalpaymentsRealexRSyncRequest, GlobalpaymentsRealexRSyncResponse,
    GlobalpaymentsRealexRefundRequest, GlobalpaymentsRealexRefundResponse,
    GlobalpaymentsRealexVoidRequest, GlobalpaymentsRealexVoidResponse,
};

use super::macros::{self, GetSoapXml};
use crate::{types::ResponseRouterData, utils, with_error_response_body};

/// The gateway accepts `application/xml` and `text/xml`; the former is what the spec verified.
const CONTENT_TYPE_XML: &str = "application/xml";

/// Every operation posts to this one path under the configured base URL.
const EPAGE_REMOTE_PATH: &str = "epage-remote.cgi";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// RealEx amounts are integers in the smallest unit of the currency. The JPY x100 special case is
// applied in the transformer, not here.
macros::create_amount_converter_wrapper!(
    connector_name: GlobalpaymentsRealex,
    amount_type: MinorUnit
);

macros::create_all_prerequisites!(
    connector_name: GlobalpaymentsRealex,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GlobalpaymentsRealexPaymentsRequest,
            response_body: GlobalpaymentsRealexPaymentsResponse,
            response_format: xml,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: GlobalpaymentsRealexPSyncRequest,
            response_body: GlobalpaymentsRealexPSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: GlobalpaymentsRealexCaptureRequest,
            response_body: GlobalpaymentsRealexCaptureResponse,
            response_format: xml,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: GlobalpaymentsRealexVoidRequest,
            response_body: GlobalpaymentsRealexVoidResponse,
            response_format: xml,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GlobalpaymentsRealexRefundRequest,
            response_body: GlobalpaymentsRealexRefundResponse,
            response_format: xml,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: GlobalpaymentsRealexRSyncRequest,
            response_body: GlobalpaymentsRealexRSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpayments_realex.base_url
        }

        /// The Refund flow hangs off `RefundFlowData` rather than `PaymentFlowData`, so it needs
        /// its own accessor; the URL it resolves to is identical.
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpayments_realex.base_url
        }

        /// The single CGI endpoint shared by `auth`, `settle`, `void`, `rebate` and `query`.
        pub fn build_endpoint_url(&self, base_url: &str) -> String {
            format!("{}{}", base_url.trim_end_matches('/'), format_args!("/{EPAGE_REMOTE_PATH}"))
        }

        /// There is no header-based authentication: the only credential material on the wire is
        /// the `<sha1hash>` element inside the body.
        pub fn build_xml_headers(&self) -> Vec<(String, Maskable<String>)> {
            vec![(
                headers::CONTENT_TYPE.to_string(),
                CONTENT_TYPE_XML.to_string().into(),
            )]
        }
    }
);

// =============================================================================
// MARKER TRAITS
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for GlobalpaymentsRealex<T>
{
}

// The XML card flows have no webhooks (tech spec §1).
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for GlobalpaymentsRealex<T>
{
}

// Like the other XML connectors, the response body is parsed through the macro layer's XML
// pattern rather than through `BodyDecoding`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    GlobalpaymentsRealex<T>
{
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _req: &RouterDataV2<F, FCD, Req, Res>,
        bytes: bytes::Bytes,
        _status_code: u16,
    ) -> CustomResult<bytes::Bytes, IntegrationError> {
        // The XML is passed through untouched; the macro layer deserializes it.
        Ok(bytes)
    }
}

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexPaymentsRequest),
    curl_response: GlobalpaymentsRealexPaymentsResponse,
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
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// PSYNC FLOW (`type="query"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexPSyncRequest),
    curl_response: GlobalpaymentsRealexPSyncResponse,
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
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // A status enquiry is a POST like everything else on this API: the same single CGI
            // endpoint, with `type="query"` on the request root.
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// CAPTURE FLOW (`type="settle"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexCaptureRequest),
    curl_response: GlobalpaymentsRealexCaptureResponse,
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
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same single CGI endpoint as Authorize; only the `type` attribute differs.
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// VOID FLOW (`type="void"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexVoidRequest),
    curl_response: GlobalpaymentsRealexVoidResponse,
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
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same single CGI endpoint as Authorize and Capture; only the `type` attribute differs.
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// REFUND FLOW (`type="rebate"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexRefundRequest),
    curl_response: GlobalpaymentsRealexRefundResponse,
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
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same single CGI endpoint as every other flow; only the `type` attribute differs.
            Ok(self.build_endpoint_url(self.connector_base_url_refunds(req)))
        }
    }
);

// =============================================================================
// RSYNC FLOW (`type="query"` on the `_rebate_` leg)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexRSyncRequest),
    curl_response: GlobalpaymentsRealexRSyncResponse,
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
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // A refund status enquiry is the same `type="query"` operation PSync uses, against the
            // same single CGI endpoint; only the `<orderid>` differs.
            Ok(self.build_endpoint_url(self.connector_base_url_refunds(req)))
        }
    }
);

// =============================================================================
// CONNECTOR COMMON
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for GlobalpaymentsRealex<T>
{
    fn id(&self) -> &'static str {
        "globalpayments_realex"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.globalpayments_realex.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Authentication is the `<sha1hash>` element in the body; there are no auth headers.
        Ok(vec![])
    }

    /// In practice this is never reached for a business failure: the gateway answers **every**
    /// outcome — declines, bad digests, malformed XML — with HTTP 200, so failures are classified
    /// from `<result>` in the response transformer. This path only covers a genuine transport-level
    /// non-200 that still carried a RealEx document.
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: GlobalpaymentsRealexPaymentsResponse = res
            .response
            .parse_struct("GlobalpaymentsRealexPaymentsResponse")
            .change_context(utils::response_deserialization_fail(
                res.status_code,
                "globalpayments_realex: response body did not match the expected format; confirm \
                 API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.result.clone(),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: response.message,
            attempt_status: None,
            connector_transaction_id: response.pasref,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

// All six core flows are wired: Authorize (`auth`), PSync (`query`), Capture (`settle`), Void
// (`void`), Refund (`rebate`) and RSync (`query` on the `_rebate_` leg). A `query` on the original
// order id does only ever echo the authorization leg, so refund state is not readable *there*
// (tech spec §12.4.5) — but a successful rebate is stored as its own transaction under the
// synthetic order id `_rebate_<orderid>`, and querying that leg is exactly what RSync does
// (tech spec §12.6).
macros::macro_connector_flow_status_impls!(
    connector: GlobalpaymentsRealex,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
        SetupMandate,
        RepeatPayment,
        PaymentMethodToken,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        CreateOrder,
        SubmitEvidence,
        DefendDispute,
        Accept,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        IncrementalAuthorization,
    ],
);
