pub mod transformers;

use std::{any::type_name, borrow::Cow, collections::HashMap, fmt::Debug};

use bytes::Bytes;
use common_enums::CurrencyUnit;
use common_utils::{
    crypto::{self, GenerateDigest, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::{ByteSliceExt, BytesExt},
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateConnectorCustomer,
        CreateOrder, CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke,
        PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, EventType, MandateReferenceId,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails,
        SessionTokenRequestData, SessionTokenResponseData, SetupMandateRequestData,
        SubmitEvidenceData, WebhookDetailsResponse,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};
use transformers::{
    self as fiuu, FiuuPaymentCancelRequest, FiuuPaymentCancelResponse, FiuuPaymentRequest,
    FiuuPaymentResponse, FiuuPaymentSyncRequest, FiuuPaymentsRequest as FiuuRepeatPaymentsRequest,
    FiuuPaymentsResponse, FiuuPaymentsResponse as FiuuRepeatPaymentsResponse, FiuuRefundRequest,
    FiuuRefundResponse, FiuuRefundSyncRequest, FiuuRefundSyncResponse, FiuuWebhooksResponse,
    PaymentCaptureRequest, PaymentCaptureResponse,
};

use super::macros;
use crate::{
    types::ResponseRouterData, utils::xml_utils::flatten_json_structure, with_error_response_body,
    with_response_body,
};

// Trait implementations with generic type parameters

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Fiuu<T>
{
}

// Authentication trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Fiuu<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Fiuu,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

macros::create_all_prerequisites!(
    connector_name: Fiuu,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: FiuuPaymentRequest<T>,
            response_body: FiuuPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: FiuuPaymentSyncRequest,
            response_body: FiuuPaymentResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PaymentCaptureRequest,
            response_body: PaymentCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: FiuuPaymentCancelRequest,
            response_body: FiuuPaymentCancelResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: FiuuRefundRequest,
            response_body: FiuuRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: FiuuRefundSyncRequest,
            response_body: FiuuRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: FiuuRepeatPaymentsRequest<T>,
            response_body: FiuuRepeatPaymentsResponse,
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
        ) -> Result<Bytes, errors::ConnectorError> {
                let response_str = String::from_utf8(response_bytes.to_vec()).map_err(|e| {
                error!("Error in Deserializing Response Data: {:?}", e);
                errors::ConnectorError::ResponseDeserializationFailed
            })?;

            let mut json = serde_json::Map::new();
            let mut miscellaneous: HashMap<String, Secret<String>> = HashMap::new();

            for line in response_str.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim().is_empty() {
                        error!("Null or empty key encountered in response.");
                        continue;
                    }

                    if let Some(old_value) = json.insert(key.to_string(), Value::String(value.to_string()))
                    {
                        warn!("Repeated key encountered: {}", key);
                        miscellaneous.insert(key.to_string(), Secret::new(old_value.to_string()));
                    }
                }
            }
            if !miscellaneous.is_empty() {
                let misc_value = serde_json::to_value(miscellaneous).map_err(|e| {
                    error!("Error serializing miscellaneous data: {:?}", e);
                    errors::ConnectorError::ResponseDeserializationFailed
                })?;
                json.insert("miscellaneous".to_string(), misc_value);
            }
                // Extract and flatten the JSON structure
            let flattened_json = flatten_json_structure(Value::Object(json));

            // Convert JSON Value to string and then to bytes
            let json_string = serde_json::to_string(&flattened_json).map_err(|e| {
                tracing::error!(error=?e, "Failed to convert to JSON string");
                errors::ConnectorError::ResponseDeserializationFailed
            })?;

            tracing::info!(json=?json_string, "Flattened JSON structure");

            // Return JSON as bytes
            Ok(Bytes::from(json_string.into_bytes()))
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.fiuu.base_url
        }

        pub fn connector_secondary_base_url_payments<'a, F, Req, Res>(
            &'a self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<&'a str, errors::ConnectorError> {
            let base_url = req
                .resource_common_data
                .connectors
                .fiuu
                .secondary_base_url
                .as_deref()
                .ok_or(errors::ConnectorError::InvalidConnectorConfig { config: "secondary_base_url" })?;

            Ok(base_url)
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> CustomResult<&'a str, errors::ConnectorError> {
            let base_url = req
                .resource_common_data
                .connectors
                .fiuu
                .secondary_base_url
                .as_deref()
                .ok_or(errors::ConnectorError::InvalidConnectorConfig { config: "secondary_base_url" })?;

            Ok(base_url)
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Fiuu<T>
{
    fn id(&self) -> &'static str {
        "fiuu"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "multipart/form-data"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.fiuu.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: fiuu::FiuuErrorResponse = res
            .response
            .parse_struct("fiuu::FiuuErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.clone(),
            message: response.error_desc.clone(),
            reason: Some(response.error_desc.clone()),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiuu,
    curl_request: FormData(FiuuPaymentRequest<T>),
    curl_response: FiuuPaymentsResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}RMS/API/Direct/1.4.0/index.php",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiuu,
    curl_request: FormData(FiuuRepeatPaymentsRequest<T>),
    curl_response: FiuuRepeatPaymentsResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let url = match req.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(_) =>{
                    format!(
                        "{}/RMS/API/Recurring/input_v7.php",
                        self.connector_base_url_payments(req)
                    )
                }
                MandateReferenceId::NetworkMandateId(_)
                | MandateReferenceId::NetworkTokenWithNTI(_) => {
                    format!(
                        "{}RMS/API/Direct/1.4.0/index.php",
                        self.connector_base_url_payments(req)
                    )
                }
            };
            Ok(url)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiuu,
    curl_request: FormData(PaymentCaptureRequest),
    curl_response: PaymentCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}RMS/API/capstxn/index.php",
                self.connector_secondary_base_url_payments(req)?
            ))
        }
    }
);

// Add implementation for Void
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiuu,
    curl_request: FormData(FiuuPaymentCancelRequest),
    curl_response: FiuuPaymentCancelResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}RMS/API/refundAPI/refund.php",
                self.connector_secondary_base_url_payments(req)?
            ))
        }
    }
);

// Add implementation for Refund
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiuu,
    curl_request: FormData(FiuuRefundRequest),
    curl_response: FiuuRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}RMS/API/refundAPI/index.php",
                self.connector_base_url_refunds(req)?
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiuu,
    curl_request: FormData(FiuuRefundSyncRequest),
    curl_response: FiuuRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}RMS/API/refundAPI/q_by_txn.php",
                self.connector_base_url_refunds(req)?
            ))
        }
    }
);

// PSync is not implemented using the macro structure because the response is parsed differently according to the header
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for Fiuu<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
        self.build_headers(req)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<String, errors::ConnectorError> {
        Ok(format!(
            "{}RMS/API/gate-query/index.php",
            self.connector_secondary_base_url_payments(req)?
        ))
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Option<macro_types::RequestContent>, macro_types::ConnectorError> {
        let bridge = self.p_sync;
        let input_data = FiuuRouterData {
            connector: self.to_owned(),
            router_data: req.clone(),
        };
        let request = bridge.request_body(input_data)?;
        let form_data = <FiuuPaymentSyncRequest as GetFormData>::get_form_data(&request);
        Ok(Some(macro_types::RequestContent::FormData(form_data)))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        macro_types::ConnectorError,
    > {
        match res.headers {
            Some(headers) => {
                let content_header = utils::get_http_header("Content-type", &headers)
                    .attach_printable("Missing content type in headers")
                    .change_context(errors::ConnectorError::ResponseHandlingFailed)?;
                let response: FiuuPaymentResponse = if content_header
                    .to_lowercase()
                    .replace(' ', "")
                    == "text/plain;charset=utf-8"
                {
                    parse_response(&res.response)
                } else {
                    Err(errors::ConnectorError::ResponseDeserializationFailed)
                        .attach_printable(format!("Expected content type to be text/plain;charset=UTF-8 , but received different content type as {content_header} in response"))?
                }?;
                with_response_body!(event_builder, response);

                RouterDataV2::try_from(ResponseRouterData {
                    response,
                    router_data: data.clone(),
                    http_code: res.status_code,
                })
                .change_context(errors::ConnectorError::ResponseHandlingFailed)
            }
            None => {
                // We don't get headers for payment webhook response handling
                let response: FiuuPaymentResponse = res
                    .response
                    .parse_struct("fiuu::FiuuPaymentResponse")
                    .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;
                with_response_body!(event_builder, response);

                RouterDataV2::try_from(ResponseRouterData {
                    response,
                    router_data: data.clone(),
                    http_code: res.status_code,
                })
                .change_context(errors::ConnectorError::ResponseHandlingFailed)
            }
        }
    }
    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, macro_types::ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Fiuu<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        let header = request
            .headers
            .get("content-type")
            .ok_or(errors::ConnectorError::WebhookSourceVerificationFailed)?;
        let resource: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
            parse_and_log_keys_in_url_encoded_response::<FiuuWebhooksResponse>(&request.body);
            serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
                .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?
        } else {
            request
                .body
                .parse_struct("fiuu::FiuuWebhooksResponse")
                .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?
        };

        let signature = match resource {
            FiuuWebhooksResponse::FiuuWebhookPaymentResponse(webhooks_payment_response) => {
                webhooks_payment_response.skey
            }
            FiuuWebhooksResponse::FiuuWebhookRefundResponse(webhooks_refunds_response) => {
                webhooks_refunds_response.signature
            }
        };
        hex::decode(signature.expose())
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        let header = request
            .headers
            .get("content-type")
            .ok_or(errors::ConnectorError::WebhookSourceVerificationFailed)?;
        let resource: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
            parse_and_log_keys_in_url_encoded_response::<FiuuWebhooksResponse>(&request.body);
            serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
                .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?
        } else {
            request
                .body
                .parse_struct("fiuu::FiuuWebhooksResponse")
                .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?
        };
        let verification_message = match resource {
            FiuuWebhooksResponse::FiuuWebhookPaymentResponse(webhooks_payment_response) => {
                let key0 = format!(
                    "{}{}{}{}{}{}",
                    webhooks_payment_response.tran_id,
                    webhooks_payment_response.order_id,
                    webhooks_payment_response.status,
                    webhooks_payment_response.domain.clone().peek(),
                    webhooks_payment_response.amount.get_amount_as_string(),
                    webhooks_payment_response.currency
                );
                let md5_key0 = hex::encode(
                    crypto::Md5
                        .generate_digest(key0.as_bytes())
                        .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?,
                );
                let key1 = format!(
                    "{}{}{}{}{}",
                    webhooks_payment_response.paydate,
                    webhooks_payment_response.domain.peek(),
                    md5_key0,
                    webhooks_payment_response
                        .appcode
                        .map_or("".to_string(), |appcode| appcode.expose()),
                    String::from_utf8_lossy(&connector_webhook_secrets.secret)
                );
                key1
            }
            FiuuWebhooksResponse::FiuuWebhookRefundResponse(webhooks_refunds_response) => {
                format!(
                    "{}{}{}{}{}{}{}{}",
                    webhooks_refunds_response.refund_type,
                    webhooks_refunds_response.merchant_id.peek(),
                    webhooks_refunds_response.ref_id,
                    webhooks_refunds_response.refund_id,
                    webhooks_refunds_response.txn_id,
                    webhooks_refunds_response.amount.get_amount_as_string(),
                    webhooks_refunds_response.status,
                    String::from_utf8_lossy(&connector_webhook_secrets.secret)
                )
            }
        };
        Ok(verification_message.as_bytes().to_vec())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::ConnectorError>> {
        let algorithm = crypto::Md5;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => Err(errors::ConnectorError::WebhookSourceVerificationFailed)?,
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
        let header = request
            .headers
            .get("content-type")
            .ok_or(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        let resource: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
            parse_and_log_keys_in_url_encoded_response::<FiuuWebhooksResponse>(&request.body);
            serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
                .change_context(errors::ConnectorError::WebhookEventTypeNotFound)?
        } else {
            request
                .body
                .parse_struct("fiuu::FiuuWebhooksResponse")
                .change_context(errors::ConnectorError::WebhookEventTypeNotFound)?
        };

        match resource {
            FiuuWebhooksResponse::FiuuWebhookPaymentResponse(webhooks_payment_response) => {
                Ok(EventType::from(webhooks_payment_response.status))
            }
            FiuuWebhooksResponse::FiuuWebhookRefundResponse(webhooks_refunds_response) => {
                Ok(EventType::from(webhooks_refunds_response.status))
            }
        }
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> CustomResult<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, errors::ConnectorError>
    {
        let header = request
            .headers
            .get("content-type")
            .ok_or(errors::ConnectorError::WebhookBodyDecodingFailed)?;

        let payload: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
            parse_and_log_keys_in_url_encoded_response::<FiuuWebhooksResponse>(&request.body);
            serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
                .change_context(errors::ConnectorError::WebhookResourceObjectNotFound)?
        } else {
            request
                .body
                .parse_struct("fiuu::FiuuWebhooksResponse")
                .change_context(errors::ConnectorError::WebhookResourceObjectNotFound)?
        };

        match payload.clone() {
            FiuuWebhooksResponse::FiuuWebhookPaymentResponse(webhook_payment_response) => {
                Ok(Box::new(FiuuPaymentResponse::FiuuWebhooksPaymentResponse(
                    webhook_payment_response,
                )))
            }
            FiuuWebhooksResponse::FiuuWebhookRefundResponse(webhook_refund_response) => Ok(
                Box::new(FiuuRefundSyncResponse::Webhook(webhook_refund_response)),
            ),
        }
    }

    fn process_payment_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        Ok(WebhookDetailsResponse {
            resource_id: None,
            status: common_enums::AttemptStatus::Unknown,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
            mandate_reference: None,
            minor_amount_captured: None,
            amount_captured: None,
            error_reason: None,
            network_txn_id: None,
            payment_method_update: None,
            transformation_status: common_enums::WebhookTransformationStatus::Incomplete,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        let header = request
            .headers
            .get("content-type")
            .ok_or(errors::ConnectorError::WebhookBodyDecodingFailed)?;

        let payload: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
            parse_and_log_keys_in_url_encoded_response::<FiuuWebhooksResponse>(&request.body);
            serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
                .change_context(errors::ConnectorError::WebhookResourceObjectNotFound)?
        } else {
            request
                .body
                .parse_struct("fiuu::FiuuWebhooksResponse")
                .change_context(errors::ConnectorError::WebhookResourceObjectNotFound)?
        };

        let notif = match payload.clone() {
            FiuuWebhooksResponse::FiuuWebhookPaymentResponse(_) => {
                Err(errors::ConnectorError::WebhookBodyDecodingFailed)
            }
            FiuuWebhooksResponse::FiuuWebhookRefundResponse(webhook_refund_response) => {
                Ok(FiuuRefundSyncResponse::Webhook(webhook_refund_response))
            }
        }?;

        let response = RefundWebhookDetailsResponse::try_from(notif)
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }
}

// Implementation for empty stubs - these will need to be properly implemented later
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Fiuu<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Fiuu<T>
{
}

// SourceVerification implementations for all flows

// Authentication flow ConnectorIntegrationV2 implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiuu<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Fiuu<T>
{
}

// Authentication flow SourceVerification implementations

fn parse_response<T>(data: &[u8]) -> Result<T, errors::ConnectorError>
where
    T: for<'de> Deserialize<'de>,
{
    let response_str = String::from_utf8(data.to_vec()).map_err(|e| {
        error!("Error in Deserializing Response Data: {:?}", e);
        errors::ConnectorError::ResponseDeserializationFailed
    })?;

    let mut json = serde_json::Map::new();
    let mut miscellaneous: HashMap<String, Secret<String>> = HashMap::new();

    for line in response_str.lines() {
        if let Some((key, value)) = line.split_once('=') {
            if key.trim().is_empty() {
                error!("Null or empty key encountered in response.");
                continue;
            }

            if let Some(old_value) = json.insert(key.to_string(), Value::String(value.to_string()))
            {
                warn!("Repeated key encountered: {}", key);
                miscellaneous.insert(key.to_string(), Secret::new(old_value.to_string()));
            }
        }
    }
    if !miscellaneous.is_empty() {
        let misc_value = serde_json::to_value(miscellaneous).map_err(|e| {
            error!("Error serializing miscellaneous data: {:?}", e);
            errors::ConnectorError::ResponseDeserializationFailed
        })?;
        json.insert("miscellaneous".to_string(), misc_value);
    }

    let response: T = serde_json::from_value(Value::Object(json)).map_err(|e| {
        error!("Error in Deserializing Response Data: {:?}", e);
        errors::ConnectorError::ResponseDeserializationFailed
    })?;

    Ok(response)
}

pub fn parse_and_log_keys_in_url_encoded_response<T>(data: &[u8]) {
    match std::str::from_utf8(data) {
        Ok(query_str) => {
            let loggable_keys = [
                "status",
                "orderid",
                "tranID",
                "nbcb",
                "amount",
                "currency",
                "paydate",
                "channel",
                "error_desc",
                "error_code",
                "extraP",
            ];
            let keys: Vec<(Cow<'_, str>, String)> =
                url::form_urlencoded::parse(query_str.as_bytes())
                    .map(|(key, value)| {
                        if loggable_keys.contains(&key.to_string().as_str()) {
                            (key, value.to_string())
                        } else {
                            (key, "SECRET".to_string())
                        }
                    })
                    .collect();
            info!("Keys in {} response\n{:?}", type_name::<T>(), keys);
        }
        Err(err) => {
            error!("Failed to convert bytes to string: {:?}", err);
        }
    }
}
