pub mod transformers;

use std::fmt::Debug;

use bytes::Bytes;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment},
    connector_types::{
        ConnectorSpecifications, PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as elavon, ElavonCaptureResponse, ElavonPSyncResponse, ElavonPaymentsResponse,
    ElavonRSyncResponse, ElavonRefundResponse, ElavonRepeatPaymentResponse, XMLCaptureRequest,
    XMLElavonRequest, XMLPSyncRequest, XMLRSyncRequest, XMLRefundRequest, XMLRepeatPaymentRequest,
};

use super::macros;
use crate::{
    types::ResponseRouterData, utils::preprocess_xml_response_bytes, with_error_response_body,
};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Elavon<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Elavon,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Elavon<T>
{
}
// Type alias for non-generic trait implementations

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Elavon<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Elavon<T>
{
    fn id(&self) -> &'static str {
        "elavon"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(Vec::new())
    }

    fn base_url<'a>(&self, _connectors: &'a Connectors) -> &'a str {
        "https://api.demo.convergepay.com/VirtualMerchantDemo/"
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        match res
            .response
            .parse_struct::<ElavonPaymentsResponse>("ElavonPaymentsResponse")
            .map_err(|_| {
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "elavon: response body did not match the expected format; confirm API version and connector documentation.")
            }) {
            Ok(elavon_response) => {
                with_error_response_body!(event_builder, elavon_response);
                let typed = macros::serialize_typed_connector_payload(&elavon_response, "typed_connector_response");
                match elavon_response.result {
                    elavon::ElavonResult::Error(error_payload) => Ok(ErrorResponse {
                        status_code: res.status_code,
                        code: error_payload.error_code.unwrap_or_else(|| "".to_string()),
                        message: error_payload.error_message,
                        reason: error_payload.error_name,
                        attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                        connector_transaction_id: error_payload.ssl_txn_id,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        typed_connector_response: typed,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    }),
                    elavon::ElavonResult::Success(success_payload) => Ok(ErrorResponse {
                        status_code: res.status_code,
                        code: "".to_string(),
                        message: "Received success response in error flow".to_string(),
                        reason: Some(format!(
                            "Unexpected success: {:?}",
                            success_payload.ssl_result_message
                        )),
                        attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                        connector_transaction_id: Some(success_payload.ssl_txn_id),
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
                    }),
                }
            }
            Err(_parsing_error) => {
                let (message, reason) = match res.status_code {
                    500..=599 => (
                        "Elavon server error".to_string(),
                        Some(String::from_utf8_lossy(&res.response).into_owned()),
                    ),
                    _ => (
                        "Elavon error response".to_string(),
                        Some(String::from_utf8_lossy(&res.response).into_owned()),
                    ),
                };
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: "".to_string(),
                    message,
                    reason,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: None,
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
    }
}

macros::create_all_prerequisites!(
    connector_name: Elavon,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: XMLElavonRequest,
            response_body: ElavonPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: XMLPSyncRequest,
            response_body: ElavonPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: XMLCaptureRequest,
            response_body: ElavonCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: XMLRefundRequest,
            response_body: ElavonRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: XMLRSyncRequest,
            response_body: ElavonRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: XMLRepeatPaymentRequest,
            response_body: ElavonRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            response_bytes: Bytes,
            status_code: u16,
        ) -> Result<Bytes, ConnectorError> {
            // Use the utility function to preprocess XML response bytes
            preprocess_xml_response_bytes(response_bytes, status_code)
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
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Elavon,
    curl_request: FormUrlEncoded(XMLElavonRequest),
    curl_response: ElavonPaymentsResponse,
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
            Ok(format!(
                "{}processxml.do",
                req.resource_common_data.connectors.elavon.base_url
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Elavon,
    curl_request: FormUrlEncoded(XMLPSyncRequest),
    curl_response: ElavonPSyncResponse,
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
                "{}processxml.do",
                req.resource_common_data.connectors.elavon.base_url
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Elavon,
    curl_request: FormUrlEncoded(XMLCaptureRequest),
    curl_response: ElavonCaptureResponse,
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
                "{}processxml.do",
                req.resource_common_data.connectors.elavon.base_url
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Elavon,
    curl_request: FormUrlEncoded(XMLRefundRequest),
    curl_response: ElavonRefundResponse,
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
                "{}processxml.do",
                req.resource_common_data.connectors.elavon.base_url
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Elavon,
    curl_request: FormUrlEncoded(XMLRSyncRequest),
    curl_response: ElavonRSyncResponse,
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
            Ok(format!(
                "{}processxml.do",
                req.resource_common_data.connectors.elavon.base_url
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Elavon,
    curl_request: FormUrlEncoded(XMLRepeatPaymentRequest),
    curl_response: ElavonRepeatPaymentResponse,
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
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}processxml.do",
                req.resource_common_data.connectors.elavon.base_url
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Elavon<T>
{
}

// Flow declarations mirror the production transformer mappings, including
// context-dependent and nonterminal outcomes.
domain_types::impl_flow_status_mapping_ctx! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Elavon<T>,
    flow: domain_types::connector_flow::PSync,
    source: elavon::TransactionSyncStatus,
    context: elavon::SyncTransactionType,
    params: [status, ctx],
    success_status: STL,
    success_targets: [Authorized, Charged],
    failure_status: PST,
    failure_target: Failure,
    {
        use common_enums::AttemptStatus;
        use elavon::{SyncTransactionType, TransactionSyncStatus};
        match status {
            TransactionSyncStatus::STL => match ctx {
                SyncTransactionType::Sale | SyncTransactionType::AuthOnly => AttemptStatus::Charged,
                SyncTransactionType::Return => AttemptStatus::Pending,
            },
            TransactionSyncStatus::OPN => match ctx {
                SyncTransactionType::AuthOnly => AttemptStatus::Authorized,
                SyncTransactionType::Sale | SyncTransactionType::Return => AttemptStatus::Pending,
            },
            TransactionSyncStatus::PEN | TransactionSyncStatus::REV => AttemptStatus::Pending,
            TransactionSyncStatus::PST | TransactionSyncStatus::FPR | TransactionSyncStatus::PRE => {
                if ctx == SyncTransactionType::AuthOnly && status == TransactionSyncStatus::PRE {
                    AttemptStatus::AuthenticationFailed
                } else {
                    AttemptStatus::Failure
                }
            }
        }
    }
}

domain_types::impl_refund_flow_status_mapping_ctx! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Elavon<T>,
    flow: domain_types::connector_flow::RSync,
    source: elavon::TransactionSyncStatus,
    context: elavon::SyncTransactionType,
    params: [status, ctx],
    success_status: STL,
    failure_status: PST,
    {
        use common_enums::RefundStatus;
        use elavon::{SyncTransactionType, TransactionSyncStatus};
        match ctx {
            SyncTransactionType::Return => match status {
                TransactionSyncStatus::STL => RefundStatus::Success,
                TransactionSyncStatus::PEN | TransactionSyncStatus::OPN => RefundStatus::Pending,
                TransactionSyncStatus::REV => RefundStatus::ManualReview,
                TransactionSyncStatus::PST | TransactionSyncStatus::FPR | TransactionSyncStatus::PRE => {
                    RefundStatus::Failure
                }
            },
            _ => RefundStatus::Pending,
        }
    }
}

domain_types::impl_flow_status_mapping_ctx! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Elavon<T>,
    flow: domain_types::connector_flow::Authorize,
    source: elavon::ElavonResult,
    context: u16,
    params: [status, ctx],
    success_sample: Some(elavon::ElavonResult::Success(elavon::PaymentResponse {
        ssl_result: elavon::SslResult::Approved,
        ssl_txn_id: String::new(),
        ssl_result_message: String::new(),
        ssl_token: None,
        ssl_approval_code: None,
        ssl_transaction_type: Some("ccsale".to_string()),
        ssl_cvv2_response: None,
        ssl_avs_response: None,
        ssl_token_response: None,
    })),
    failure_sample: Some(elavon::ElavonResult::Error(elavon::ElavonErrorResponse {
        error_code: None,
        error_message: String::new(),
        error_name: None,
        ssl_txn_id: None,
    })),
    {
        elavon::get_elavon_attempt_status(&status, ctx).0
    }
}

domain_types::impl_flow_status_mapping_ctx! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Elavon<T>,
    flow: domain_types::connector_flow::Capture,
    source: elavon::ElavonResult,
    context: u16,
    params: [status, ctx],
    success_sample: Some(elavon::ElavonResult::Success(elavon::PaymentResponse {
        ssl_result: elavon::SslResult::Approved,
        ssl_txn_id: String::new(),
        ssl_result_message: String::new(),
        ssl_token: None,
        ssl_approval_code: None,
        ssl_transaction_type: Some("cccomplete".to_string()),
        ssl_cvv2_response: None,
        ssl_avs_response: None,
        ssl_token_response: None,
    })),
    failure_sample: Some(elavon::ElavonResult::Error(elavon::ElavonErrorResponse {
        error_code: None,
        error_message: String::new(),
        error_name: None,
        ssl_txn_id: None,
    })),
    {
        let attempt_status = elavon::get_elavon_attempt_status(&status, ctx).0;
        match &status {
            elavon::ElavonResult::Success(payload) => {
                match payload.ssl_transaction_type.as_deref() {
                    Some("cccomplete") | Some("ccsale") => match payload.ssl_result {
                        elavon::SslResult::Approved => common_enums::AttemptStatus::Charged,
                        _ => common_enums::AttemptStatus::Failure,
                    },
                    _ => attempt_status,
                }
            }
            _ => attempt_status,
        }
    }
}

domain_types::impl_flow_status_mapping_ctx! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Elavon<T>,
    flow: domain_types::connector_flow::RepeatPayment,
    source: elavon::ElavonResult,
    context: u16,
    params: [status, ctx],
    success_sample: Some(elavon::ElavonResult::Success(elavon::PaymentResponse {
        ssl_result: elavon::SslResult::Approved,
        ssl_txn_id: String::new(),
        ssl_result_message: String::new(),
        ssl_token: None,
        ssl_approval_code: None,
        ssl_transaction_type: Some("ccsale".to_string()),
        ssl_cvv2_response: None,
        ssl_avs_response: None,
        ssl_token_response: None,
    })),
    failure_sample: Some(elavon::ElavonResult::Error(elavon::ElavonErrorResponse {
        error_code: None,
        error_message: String::new(),
        error_name: None,
        ssl_txn_id: None,
    })),
    {
        elavon::get_elavon_attempt_status(&status, ctx).0
    }
}

domain_types::impl_refund_flow_status_mapping_ctx! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Elavon<T>,
    flow: domain_types::connector_flow::Refund,
    source: elavon::ElavonResult,
    context: (),
    params: [status, ctx],
    success_sample: Some(elavon::ElavonResult::Success(elavon::PaymentResponse {
        ssl_result: elavon::SslResult::Approved,
        ssl_txn_id: String::new(),
        ssl_result_message: String::new(),
        ssl_token: None,
        ssl_approval_code: None,
        ssl_transaction_type: Some("RETURN".to_string()),
        ssl_cvv2_response: None,
        ssl_avs_response: None,
        ssl_token_response: None,
    })),
    failure_sample: Some(elavon::ElavonResult::Error(elavon::ElavonErrorResponse {
        error_code: None,
        error_message: String::new(),
        error_name: None,
        ssl_txn_id: None,
    })),
    {
        let _ = ctx;
        match status {
            elavon::ElavonResult::Success(payload) => {
                match payload.ssl_transaction_type.as_deref() {
                    Some("RETURN") => match payload.ssl_result {
                        elavon::SslResult::Approved => common_enums::RefundStatus::Success,
                        elavon::SslResult::Declined => common_enums::RefundStatus::Failure,
                        elavon::SslResult::Other(_) => common_enums::RefundStatus::Pending,
                    },
                    _ => common_enums::RefundStatus::Pending,
                }
            }
            elavon::ElavonResult::Error(_) => common_enums::RefundStatus::Failure,
        }
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Elavon,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Void,
        SetupMandate,
        PaymentMethodToken,
        MandateRevoke,
        VoidPC,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        CreateOrder,
        Accept,
        SubmitEvidence,
        DefendDispute,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
);
