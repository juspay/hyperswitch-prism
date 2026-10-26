pub mod transformers;

// Status instrumentation mirrors the existing response transformers.
// Source is transaction_approved; context is request.is_auto_capture().
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Payconex<T>,
    flow: Authorize,
    source: bool,
    context: bool,
    mapping: |status, ctx|{
        let mapped_status = (|| { transformers::map_payment_status(true, status, ctx) })();
        match mapped_status {
            common_enums::AttemptStatus::Authorized => success!(Authorized),
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::PartialCharged => success!(PartialCharged),
            common_enums::AttemptStatus::PartiallyAuthorized => success!(PartiallyAuthorized),
            common_enums::AttemptStatus::AuthenticationFailed => failure!(AuthenticationFailed),
            common_enums::AttemptStatus::AuthorizationFailed => failure!(AuthorizationFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::IntegrityFailure => failure!(IntegrityFailure),
            common_enums::AttemptStatus::AuthenticationPending => non_terminal!(AuthenticationPending),
            common_enums::AttemptStatus::AuthenticationSuccessful => non_terminal!(AuthenticationSuccessful),
            common_enums::AttemptStatus::Authorizing => non_terminal!(Authorizing),
            common_enums::AttemptStatus::AutoRefunded => non_terminal!(AutoRefunded),
            common_enums::AttemptStatus::ConfirmationAwaited => non_terminal!(ConfirmationAwaited),
            common_enums::AttemptStatus::DeviceDataCollectionPending => non_terminal!(DeviceDataCollectionPending),
            common_enums::AttemptStatus::Expired => non_terminal!(Expired),
            common_enums::AttemptStatus::PartialChargedAndChargeable => non_terminal!(PartialChargedAndChargeable),
            common_enums::AttemptStatus::PaymentMethodAwaited => non_terminal!(PaymentMethodAwaited),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            common_enums::AttemptStatus::Started => non_terminal!(Started),
            common_enums::AttemptStatus::Unknown => non_terminal!(Unknown),
            common_enums::AttemptStatus::Unresolved => non_terminal!(Unresolved),
            common_enums::AttemptStatus::Unspecified => non_terminal!(Unspecified),
            common_enums::AttemptStatus::Voided => non_terminal!(Voided),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentsAuthorizeData<T>,
        response: PayconexPaymentsResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_approved) },
        context: |_resource_common_data, request, _response, _http_status_code| { Ok(request.is_auto_capture()) },
    },
}

// Source is (found, transaction_approved); context is request.is_auto_capture().
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Payconex<T>,
    flow: PSync,
    source: (bool, bool),
    context: bool,
    mapping: |status, ctx|{
        let mapped_status = (|| { transformers::map_payment_status(status.0, status.1, ctx) })();
        match mapped_status {
            common_enums::AttemptStatus::Authorized => success!(Authorized),
            common_enums::AttemptStatus::AutoRefunded => success!(AutoRefunded),
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::PartialCharged => success!(PartialCharged),
            common_enums::AttemptStatus::PartiallyAuthorized => success!(PartiallyAuthorized),
            common_enums::AttemptStatus::Voided => success!(Voided),
            common_enums::AttemptStatus::VoidedPostCapture => success!(VoidedPostCapture),
            common_enums::AttemptStatus::AuthenticationFailed => failure!(AuthenticationFailed),
            common_enums::AttemptStatus::AuthorizationFailed => failure!(AuthorizationFailed),
            common_enums::AttemptStatus::CaptureFailed => failure!(CaptureFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::IntegrityFailure => failure!(IntegrityFailure),
            common_enums::AttemptStatus::VoidFailed => failure!(VoidFailed),
            common_enums::AttemptStatus::AuthenticationPending => non_terminal!(AuthenticationPending),
            common_enums::AttemptStatus::AuthenticationSuccessful => non_terminal!(AuthenticationSuccessful),
            common_enums::AttemptStatus::Authorizing => non_terminal!(Authorizing),
            common_enums::AttemptStatus::CaptureInitiated => non_terminal!(CaptureInitiated),
            common_enums::AttemptStatus::CodInitiated => non_terminal!(CodInitiated),
            common_enums::AttemptStatus::ConfirmationAwaited => non_terminal!(ConfirmationAwaited),
            common_enums::AttemptStatus::DeviceDataCollectionPending => non_terminal!(DeviceDataCollectionPending),
            common_enums::AttemptStatus::Expired => non_terminal!(Expired),
            common_enums::AttemptStatus::PartialChargedAndChargeable => non_terminal!(PartialChargedAndChargeable),
            common_enums::AttemptStatus::PaymentMethodAwaited => non_terminal!(PaymentMethodAwaited),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            common_enums::AttemptStatus::Started => non_terminal!(Started),
            common_enums::AttemptStatus::Unknown => non_terminal!(Unknown),
            common_enums::AttemptStatus::Unresolved => non_terminal!(Unresolved),
            common_enums::AttemptStatus::Unspecified => non_terminal!(Unspecified),
            common_enums::AttemptStatus::VoidInitiated => non_terminal!(VoidInitiated),
            common_enums::AttemptStatus::VoidPostCaptureInitiated => non_terminal!(VoidPostCaptureInitiated),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentsSyncData,
        response: PayconexSyncResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok((response.found, response.transaction_approved)) },
        context: |_resource_common_data, request, _response, _http_status_code| { Ok(request.is_auto_capture()) },
    },
}

// Source is transaction_approved; required transaction IDs remain TryFrom validation.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Payconex<T>,
    flow: Capture,
    source: bool,
    mapping: |status|{ if status {
            success!(Charged)
        } else {
            failure!(CaptureFailed)
        } },
    runtime: {
        request: PaymentsCaptureData,
        response: PayconexCaptureResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_approved) },
    },
}

// Source is transaction_approved; required transaction IDs remain TryFrom validation.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Payconex<T>,
    flow: Void,
    source: bool,
    mapping: |status|{ if status {
            success!(Voided)
        } else {
            failure!(VoidFailed)
        } },
    runtime: {
        request: PaymentVoidData,
        response: PayconexVoidResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_approved) },
    },
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Payconex<T>,
    flow: Refund,
    source: bool,
    mapping: |status|{
        let mapped_status = (|| { transformers::map_refund_status(true, status) })();
        match mapped_status {
            common_enums::RefundStatus::Success => success!(Success),
            common_enums::RefundStatus::Failure => failure!(Failure),
            common_enums::RefundStatus::TransactionFailure => non_terminal!(TransactionFailure),
            common_enums::RefundStatus::Pending => non_terminal!(Pending),
            common_enums::RefundStatus::ManualReview => non_terminal!(ManualReview),
            common_enums::RefundStatus::Unknown => non_terminal!(Pending),
        }
    },
    runtime: {
        request: RefundsData,
        response: PayconexRefundResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(response.transaction_approved) },
    },
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Payconex<T>,
    flow: RSync,
    source: (bool, bool),
    mapping: |status|{
        let mapped_status = (|| { transformers::map_refund_status(status.0, status.1) })();
        match mapped_status {
            common_enums::RefundStatus::Success => success!(Success),
            common_enums::RefundStatus::Failure => failure!(Failure),
            common_enums::RefundStatus::TransactionFailure => non_terminal!(TransactionFailure),
            common_enums::RefundStatus::Pending => non_terminal!(Pending),
            common_enums::RefundStatus::ManualReview => non_terminal!(ManualReview),
            common_enums::RefundStatus::Unknown => non_terminal!(Pending),
        }
    },
    runtime: {
        request: RefundSyncData,
        response: PayconexRefundSyncResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok((response.found, response.transaction_approved)) },
    },
}

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    types::StringMajorUnit,
};
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
    PayconexCaptureRequest, PayconexCaptureResponse, PayconexErrorResponse,
    PayconexPaymentsRequest, PayconexPaymentsResponse, PayconexRefundRequest,
    PayconexRefundResponse, PayconexRefundSyncRequest, PayconexRefundSyncResponse,
    PayconexSyncRequest, PayconexSyncResponse, PayconexVoidRequest, PayconexVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

const QSAPI_PATH: &str = "/api/qsapi/3.8";
const TSAPI_PATH: &str = "/api/tsapi/3.8";

macros::macro_connector_payout_implementation!(
    connector: Payconex,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Payconex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Payconex<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Payconex, amount_type: StringMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Payconex,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PayconexPaymentsRequest<T>,
            response_body: PayconexPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: PayconexSyncRequest,
            response_body: PayconexSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PayconexCaptureRequest,
            response_body: PayconexCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PayconexVoidRequest,
            response_body: PayconexVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PayconexRefundRequest,
            response_body: PayconexRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: PayconexRefundSyncRequest,
            response_body: PayconexRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            status_code: u16,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            // PayConex (QSAPI/TSAPI) returns JSON when response_format=JSON is
            // honored, but can fall back to application/x-www-form-urlencoded.
            // Detect the actual shape and only normalize url-encoded payloads
            // to JSON, so the typed response structs parse in both cases.
            let is_json = bytes
                .iter()
                .copied()
                .find(|b| !b.is_ascii_whitespace())
                .map(|b| b == b'{' || b == b'[')
                .unwrap_or(false);

            if is_json {
                return Ok(bytes);
            }

            let url_encoded_response: serde_json::Value = serde_urlencoded::from_bytes(&bytes)
                .change_context(crate::utils::response_deserialization_fail(
                    status_code,
                    "payconex: response body did not match the expected format; confirm API version and connector documentation.",
                ))
                .attach_printable("Failed to parse URL-encoded response from Payconex")?;

            let json_bytes = serde_json::to_vec(&url_encoded_response)
                .change_context(crate::utils::response_deserialization_fail(
                    status_code,
                    "payconex: response body did not match the expected format; confirm API version and connector documentation.",
                ))
                .attach_printable("Failed to convert URL-encoded response to JSON")?;

            Ok(bytes::Bytes::from(json_bytes))
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.payconex.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.payconex.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Payconex<T>
{
    fn id(&self) -> &'static str {
        "payconex"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.payconex.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // PayConex carries credentials in the request body, not headers.
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: PayconexErrorResponse = serde_urlencoded::from_bytes(&res.response)
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "payconex: response body did not match the expected format; confirm API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .error_message
                .clone()
                .or_else(|| response.authorization_message.clone())
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.error_message.or(response.authorization_message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payconex,
    curl_request: FormUrlEncoded(PayconexPaymentsRequest),
    curl_response: PayconexPaymentsResponse,
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
            Ok(format!("{}{QSAPI_PATH}", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payconex,
    curl_request: FormUrlEncoded(PayconexSyncRequest),
    curl_response: PayconexSyncResponse,
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
            Ok(format!("{}{TSAPI_PATH}", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payconex,
    curl_request: FormUrlEncoded(PayconexCaptureRequest),
    curl_response: PayconexCaptureResponse,
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
            Ok(format!("{}{QSAPI_PATH}", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payconex,
    curl_request: FormUrlEncoded(PayconexVoidRequest),
    curl_response: PayconexVoidResponse,
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
            Ok(format!("{}{QSAPI_PATH}", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payconex,
    curl_request: FormUrlEncoded(PayconexRefundRequest),
    curl_response: PayconexRefundResponse,
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
            Ok(format!("{}{QSAPI_PATH}", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payconex,
    curl_request: FormUrlEncoded(PayconexRefundSyncRequest),
    curl_response: PayconexRefundSyncResponse,
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
            Ok(format!("{}{TSAPI_PATH}", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Payconex,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    // Flows that could be built against PayConex later but are not implemented yet.
    not_implemented: [
        CreateConnectorCustomer,
        GetConnectorCustomer,
        CreateOrder,
        MandateRevoke,
        PaymentMethodToken,
        RepeatPayment,
        SetupMandate
    ],
    // Flows PayConex fundamentally cannot do: 3DS authentication (this is a
    // no-3DS card connector), client/server SDK session tokens (direct server-to-server
    // only), disputes, and partial-capture void.
    not_supported: [
        VoidPostRefund,
        Accept,
        Authenticate,
        ClientAuthenticationToken,
        DefendDispute,
        IncrementalAuthorization,
        PostAuthenticate,
        PreAuthenticate,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SubmitEvidence,
        VoidPC
    ],
);
