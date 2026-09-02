//! GlobalpaymentsHeartland connector — Global Payments **Heartland / Portico** gateway.
//!
//! Portico (`Hps.Exchange.PosGateway`) is a **SOAP 1.1 / XML** gateway with a single RPC
//! endpoint: every flow is a `POST` of the same envelope to the same `.asmx` URL, and the
//! operation is selected by the name of the single child element inside
//! `<Ver1.0><Transaction>`. There are no path segments, no query parameters and no HTTP
//! verb other than `POST`.
//!
//! Implemented scope: Card, one-time payments — Authorize (`CreditAuth` for manual
//! capture, `CreditSale` for auto capture; non-3DS **and** external-3DS pass-through),
//! PSync and RSync (`ReportTxnDetail`), Capture (`CreditAddToBatch`), Void (`CreditVoid`)
//! and Refund (`CreditReturn`).
//!
//! Not to be confused with the separate `globalpay` (GP-API / Realex) connector: they are
//! different gateways.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::XmlExt, types::StringMajorUnit};
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
    GlobalpaymentsHeartlandCaptureRequest, GlobalpaymentsHeartlandCaptureResponse,
    GlobalpaymentsHeartlandErrorResponse, GlobalpaymentsHeartlandPSyncRequest,
    GlobalpaymentsHeartlandPSyncResponse, GlobalpaymentsHeartlandPaymentsRequest,
    GlobalpaymentsHeartlandPaymentsResponse, GlobalpaymentsHeartlandRSyncRequest,
    GlobalpaymentsHeartlandRSyncResponse, GlobalpaymentsHeartlandRefundRequest,
    GlobalpaymentsHeartlandRefundResponse, GlobalpaymentsHeartlandVoidRequest,
    GlobalpaymentsHeartlandVoidResponse,
};

use super::macros::{self, GetSoapXml};
use crate::types::ResponseRouterData;

/// SOAP 1.1 content type. Portico rejects anything else.
const CONTENT_TYPE_XML: &str = "text/xml; charset=utf-8";

/// The `SOAPAction` value — **the surrounding double quotes are part of the value**.
const SOAP_ACTION: &str = "\"http://Hps.Exchange.PosGateway/PosGatewayService/DoTransaction\"";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const SOAP_ACTION: &str = "SOAPAction";
}

// =============================================================================
// AMOUNT CONVERTER
// =============================================================================
// Portico quotes amounts in **major** units as a decimal string with 2 dp ("10.00").
// Sending minor units would authorize a hundred times the intended amount.
macros::create_amount_converter_wrapper!(
    connector_name: GlobalpaymentsHeartland,
    amount_type: StringMajorUnit
);

// =============================================================================
// CONNECTOR STRUCT + PREREQUISITES
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: GlobalpaymentsHeartland,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GlobalpaymentsHeartlandPaymentsRequest,
            response_body: GlobalpaymentsHeartlandPaymentsResponse,
            response_format: xml,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: GlobalpaymentsHeartlandPSyncRequest,
            response_body: GlobalpaymentsHeartlandPSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: GlobalpaymentsHeartlandCaptureRequest,
            response_body: GlobalpaymentsHeartlandCaptureResponse,
            response_format: xml,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: GlobalpaymentsHeartlandVoidRequest,
            response_body: GlobalpaymentsHeartlandVoidResponse,
            response_format: xml,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GlobalpaymentsHeartlandRefundRequest,
            response_body: GlobalpaymentsHeartlandRefundResponse,
            response_format: xml,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: GlobalpaymentsHeartlandRSyncRequest,
            response_body: GlobalpaymentsHeartlandRSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        /// Content-Type plus the quoted `SOAPAction`. There is **no** auth header: the
        /// `SecretAPIKey` lives in the SOAP body at `Ver1.0/Header/SecretAPIKey`.
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    CONTENT_TYPE_XML.to_string().into(),
                ),
                (
                    headers::SOAP_ACTION.to_string(),
                    SOAP_ACTION.to_string().into(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpayments_heartland.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpayments_heartland.base_url
        }
    }
);

// =============================================================================
// RESPONSE PREPROCESSING
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    GlobalpaymentsHeartland<T>
{
    /// quick-xml's deserializer does **not** strip namespace prefixes, so `<soap:Body>`
    /// would never bind to a `Body` field and the response structs would silently fail.
    /// Drop the `soap:` prefix and the four namespace declarations Portico emits before
    /// the body reaches the parser. The structure is otherwise left untouched.
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _req: &RouterDataV2<F, FCD, Req, Res>,
        bytes: bytes::Bytes,
        _status_code: u16,
    ) -> CustomResult<bytes::Bytes, IntegrationError> {
        let response_str = String::from_utf8(bytes.to_vec()).change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )?;

        let xml_response = response_str
            .replace("soap:", "")
            .replace(
                " xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\"",
                "",
            )
            .replace(
                " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
                "",
            )
            .replace(" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"", "")
            .replace(" xmlns=\"http://Hps.Exchange.PosGateway\"", "");

        Ok(bytes::Bytes::from(xml_response))
    }
}

// =============================================================================
// CONNECTOR COMMON
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for GlobalpaymentsHeartland<T>
{
    fn id(&self) -> &'static str {
        "globalpayments_heartland"
    }

    /// Major units — see the amount converter above.
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.globalpayments_heartland.base_url.as_ref()
    }

    /// **Empty on purpose.** Portico authenticates from the SOAP body
    /// (`Ver1.0/Header/SecretAPIKey`); there is no `Authorization` header, no Basic auth
    /// and no bearer token.
    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![])
    }

    /// Portico answers **HTTP 200 for everything** on the verified paths — auth failures,
    /// rejects and declines included — so this is effectively only reached on a transport
    /// surprise. It still parses `Ver1.0/Header` when the body happens to carry one.
    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let raw = String::from_utf8_lossy(res.response.as_ref()).to_string();
        let parsed: Option<GlobalpaymentsHeartlandErrorResponse> = raw
            .replace("soap:", "")
            .as_str()
            .parse_xml::<GlobalpaymentsHeartlandErrorResponse>()
            .ok();

        let header = parsed.map(|parsed| parsed.body.pos_response.ver.header);
        let code = header
            .as_ref()
            .and_then(|header| header.gateway_rsp_code.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
        let message = header
            .as_ref()
            .and_then(|header| header.gateway_rsp_msg.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: header.and_then(|header| header.gateway_txn_id),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

// =============================================================================
// MARKER TRAITS
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for GlobalpaymentsHeartland<T>
{
}

/// Portico is polling-based: no signed webhook exists, so reconciliation is via
/// PSync/RSync (`ReportTxnDetail`) and the default no-op webhook impl stands.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for GlobalpaymentsHeartland<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for GlobalpaymentsHeartland<T>
{
}

// =============================================================================
// AUTHORIZE — `CreditAuth` / `CreditSale`
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsHeartland,
    curl_request: SoapXml(GlobalpaymentsHeartlandPaymentsRequest),
    curl_response: GlobalpaymentsHeartlandPaymentsResponse,
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
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // The configured base URL is the full `.asmx` endpoint; every flow posts to it
            // unchanged.
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// PSYNC — `ReportTxnDetail`
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsHeartland,
    curl_request: SoapXml(GlobalpaymentsHeartlandPSyncRequest),
    curl_response: GlobalpaymentsHeartlandPSyncResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// CAPTURE — `CreditAddToBatch`
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsHeartland,
    curl_request: SoapXml(GlobalpaymentsHeartlandCaptureRequest),
    curl_response: GlobalpaymentsHeartlandCaptureResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// VOID — `CreditVoid`
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsHeartland,
    curl_request: SoapXml(GlobalpaymentsHeartlandVoidRequest),
    curl_response: GlobalpaymentsHeartlandVoidResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// =============================================================================
// REFUND — `CreditReturn`
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsHeartland,
    curl_request: SoapXml(GlobalpaymentsHeartlandRefundRequest),
    curl_response: GlobalpaymentsHeartlandRefundResponse,
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
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// =============================================================================
// RSYNC — `ReportTxnDetail` on the refund's own `GatewayTxnId`
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsHeartland,
    curl_request: SoapXml(GlobalpaymentsHeartlandRSyncRequest),
    curl_response: GlobalpaymentsHeartlandRSyncResponse,
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
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// =============================================================================
// PAYOUTS + REMAINING FLOWS
// =============================================================================
macros::macro_connector_payout_implementation!(
    connector: GlobalpaymentsHeartland,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// Everything outside the six core card flows is out of scope for this integration:
// Portico exposes no mandate/tokenization, wallet, dispute or webhook operation that is
// reachable on the verified MID.
macros::macro_connector_flow_status_impls!(
    connector: GlobalpaymentsHeartland,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        Authenticate,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        CreateOrder,
        DefendDispute,
        GetConnectorCustomer,
        IncrementalAuthorization,
        MandateRevoke,
        PaymentMethodToken,
        PostAuthenticate,
        PreAuthenticate,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence,
        VoidPC,
        VoidPostRefund
    ],
);
