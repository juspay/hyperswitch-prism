pub mod transformers;

use std::{fmt::Debug, sync::LazyLock};

use common_enums::{self as enums, CurrencyUnit};
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, request::RequestContent,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, PSync, RSync, Refund},
    connector_types::{
        ConnectorSpecifications, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        SupportedPaymentMethodsExt,
    },
    errors::{self, ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        ConnectorInfo, Connectors, FeatureStatus, PaymentConnectorCategory, PaymentMethodDetails,
        SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::ForeignTryFrom;
use transformers::{
    self as easebuzz, EasebuzzCaptureRequest, EasebuzzCaptureResponse, EasebuzzInitiateLinkRequest,
    EasebuzzInitiateLinkResponse, EasebuzzPaymentsRequest, EasebuzzPaymentsResponse,
    EasebuzzRefundRequest, EasebuzzRefundResponse, EasebuzzRefundSyncRequest,
    EasebuzzRefundSyncResponse, EasebuzzSyncRequest, EasebuzzSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body, with_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// ============================================================================
// SUPPORTED PAYMENT METHODS
// ============================================================================

static EASEBUZZ_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> =
    LazyLock::new(|| {
        let mut supported = SupportedPaymentMethods::new();

        // UPI Intent (UPI_PAY)
        supported.add(
            enums::PaymentMethod::Upi,
            enums::PaymentMethodType::UpiIntent,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![enums::CaptureMethod::Automatic],
                specific_features: None,
            },
        );

        // UPI Collect (UPI_COLLECT)
        supported.add(
            enums::PaymentMethod::Upi,
            enums::PaymentMethodType::UpiCollect,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![enums::CaptureMethod::Automatic],
                specific_features: None,
            },
        );

        // UPI QR (UPI_QR)
        supported.add(
            enums::PaymentMethod::Upi,
            enums::PaymentMethodType::UpiQr,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![enums::CaptureMethod::Automatic],
                specific_features: None,
            },
        );

        // Wallet (EaseBuzz Redirect)
        supported.add(
            enums::PaymentMethod::Wallet,
            enums::PaymentMethodType::EaseBuzz,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![enums::CaptureMethod::Automatic],
                specific_features: None,
            },
        );

        // Net Banking
        supported.add(
            enums::PaymentMethod::BankRedirect,
            enums::PaymentMethodType::Netbanking,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![enums::CaptureMethod::Automatic],
                specific_features: None,
            },
        );

        supported
    });

static EASEBUZZ_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Easebuzz",
    description: "Easebuzz is an Indian payment gateway providing UPI, Net Banking, Wallet, and Card payment solutions.",
    connector_type: PaymentConnectorCategory::PaymentGateway,
};

// ============================================================================
// FLOW TRAIT IMPLEMENTATIONS (before create_all_prerequisites! macro)
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Easebuzz<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Easebuzz<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Easebuzz<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Easebuzz,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ============================================================================
// PREREQUISITES MACRO — creates Easebuzz<T> struct + bridges for Authorize
// ============================================================================

macros::create_all_prerequisites!(
    connector_name: Easebuzz,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: EasebuzzPaymentsRequest,
            response_body: EasebuzzPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: EasebuzzSyncRequest,
            response_body: EasebuzzSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: EasebuzzCaptureRequest,
            response_body: EasebuzzCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: EasebuzzRefundRequest,
            response_body: EasebuzzRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: EasebuzzRefundSyncRequest,
            response_body: EasebuzzRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
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
            &req.resource_common_data.connectors.easebuzz.base_url
        }

        pub fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            _status_code: u16,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            // EaseBuzz seamless endpoint may return HTML (UPI redirect pages).
            // If not valid JSON, wrap as a JSON null so serde_json::Value parses.
            match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(_) => Ok(bytes),
                Err(_) => Ok(bytes::Bytes::from(b"null".to_vec())),
            }
        }

        pub fn connector_dashboard_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<&'a str, IntegrationError> {
            req.resource_common_data
                .connectors
                .easebuzz
                .secondary_base_url
                .as_deref()
                .ok_or(IntegrationError::InvalidConnectorConfig {
                    config: "secondary_base_url",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Set secondary_base_url to the Easebuzz dashboard URL in connector config".to_string()
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Easebuzz requires secondary_base_url (dashboard URL) for PSync and Refund flows".to_string()
                        ),
                    },
                })
                .map_err(error_stack::Report::from)
        }
    }
);

// ============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Easebuzz<T>
{
    fn id(&self) -> &'static str {
        "easebuzz"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.easebuzz.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Easebuzz uses hash-based auth in the request body, not headers
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: easebuzz::EasebuzzErrorResponse = res
            .response
            .parse_struct("EasebuzzErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Failed to parse Easebuzz response body as JSON".to_string(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code,
            message: response.message,
            reason: None,
            attempt_status: Some(enums::AttemptStatus::Pending),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ============================================================================
// CONNECTOR SPECIFICATIONS
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Easebuzz<T>
{
    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&EASEBUZZ_SUPPORTED_PAYMENT_METHODS)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [enums::EventClass]> {
        None
    }

    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&EASEBUZZ_CONNECTOR_INFO)
    }
}

// ============================================================================
// AUTHORIZE FLOW IMPLEMENTATION (via macro)
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Easebuzz,
    curl_request: FormUrlEncoded(EasebuzzPaymentsRequest),
    curl_response: EasebuzzPaymentsResponse,
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
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/initiate_seamless_payment/"))
        }

        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            _event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: easebuzz::EasebuzzErrorResponse = res
                .response
                .parse_struct("EasebuzzErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some("Failed to parse Easebuzz response body as JSON".to_string()),
                    },
                })?;

            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.code,
                message: response.message,
                reason: None,
                attempt_status: Some(enums::AttemptStatus::Failure),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        }
    }
);

// ============================================================================
// OTHER FLOW STUBS
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Easebuzz<T>
{
    fn get_headers(
        &self,
        _req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/x-www-form-urlencoded".to_string().into(),
        )])
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.connector_base_url_payments(req);
        Ok(format!("{base_url}/payment/initiateLink"))
    }

    fn get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = EasebuzzInitiateLinkRequest::try_from(req)?;
        Ok(Some(RequestContent::FormUrlEncoded(Box::new(
            connector_req,
        ))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
        ConnectorError,
    > {
        let response: EasebuzzInitiateLinkResponse = res
            .response
            .parse_struct("EasebuzzInitiateLinkResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some("Failed to parse Easebuzz initiateLink response; expected JSON with status and data fields".to_string()),
                },
            })?;

        with_response_body!(event_builder, response);

        RouterDataV2::foreign_try_from((response, data.clone(), res.status_code, false))
            .change_context(ConnectorError::ResponseHandlingFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some(
                    "Failed to map Easebuzz initiateLink response to PaymentCreateOrderResponse"
                        .to_string(),
                ),
            },
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: easebuzz::EasebuzzErrorResponse = res
            .response
            .parse_struct("EasebuzzErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some("Failed to parse Easebuzz error response".to_string()),
                },
            })?;

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code,
            message: response.message,
            reason: None,
            attempt_status: Some(enums::AttemptStatus::Failure),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ============================================================================
// PSYNC FLOW IMPLEMENTATION (via macro)
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Easebuzz,
    curl_request: FormUrlEncoded(EasebuzzSyncRequest),
    curl_response: EasebuzzSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_dashboard_base_url(req)?;
            Ok(format!("{base_url}/transaction/v1/retrieve"))
        }

        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            _event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: easebuzz::EasebuzzErrorResponse = res
                .response
                .parse_struct("EasebuzzErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some("Failed to parse Easebuzz response body as JSON".to_string()),
                    },
                })?;

            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.code,
                message: response.message,
                reason: None,
                attempt_status: Some(enums::AttemptStatus::Pending),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        }
    }
);

// ============================================================================
// CAPTURE FLOW IMPLEMENTATION (via macro)
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Easebuzz,
    curl_request: FormUrlEncoded(EasebuzzCaptureRequest),
    curl_response: EasebuzzCaptureResponse,
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
            _req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/payment/v1/capture/direct"))
        }

        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            _event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: easebuzz::EasebuzzErrorResponse = res
                .response
                .parse_struct("EasebuzzErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some("Failed to parse Easebuzz response body as JSON".to_string()),
                    },
                })?;

            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.code,
                message: response.message,
                reason: None,
                attempt_status: Some(enums::AttemptStatus::Failure),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        }
    }
);

// ============================================================================
// RSYNC FLOW IMPLEMENTATION (via macro)
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Easebuzz,
    curl_request: FormUrlEncoded(EasebuzzRefundSyncRequest),
    curl_response: EasebuzzRefundSyncResponse,
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
            _req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let secondary_base_url = req
                .resource_common_data
                .connectors
                .easebuzz
                .secondary_base_url
                .as_deref()
                .ok_or(IntegrationError::InvalidConnectorConfig {
                    config: "secondary_base_url",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Set secondary_base_url to the Easebuzz dashboard URL in connector config".to_string()
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Easebuzz requires secondary_base_url (dashboard URL) for PSync and Refund flows".to_string()
                        ),
                    },
                })
                .map_err(error_stack::Report::from)?;
            Ok(format!("{secondary_base_url}/refund/v1/retrieve"))
        }

        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            _event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: easebuzz::EasebuzzErrorResponse = res
                .response
                .parse_struct("EasebuzzErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some("Failed to parse Easebuzz response body as JSON".to_string()),
                    },
                })?;

            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.code,
                message: response.message,
                reason: None,
                attempt_status: Some(enums::AttemptStatus::Pending),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        }
    }
);

// ============================================================================
// REFUND FLOW IMPLEMENTATION (via macro)
// ============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Easebuzz,
    curl_request: FormUrlEncoded(EasebuzzRefundRequest),
    curl_response: EasebuzzRefundResponse,
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
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let secondary_base_url = req
                .resource_common_data
                .connectors
                .easebuzz
                .secondary_base_url
                .as_deref()
                .ok_or(IntegrationError::InvalidConnectorConfig {
                    config: "secondary_base_url",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Set secondary_base_url to the Easebuzz dashboard URL in connector config".to_string()
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Easebuzz requires secondary_base_url (dashboard URL) for PSync and Refund flows".to_string()
                        ),
                    },
                })
                .map_err(error_stack::Report::from)?;
            Ok(format!("{secondary_base_url}/transaction/v2/refund"))
        }

        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            _event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: easebuzz::EasebuzzErrorResponse = res
                .response
                .parse_struct("EasebuzzErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some("Failed to parse Easebuzz response body as JSON".to_string()),
                    },
                })?;

            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.code,
                message: response.message,
                reason: None,
                attempt_status: Some(enums::AttemptStatus::Failure),
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        }
    }
);

// ============================================================================
// SOURCE VERIFICATION
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Easebuzz<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Easebuzz,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        ServerAuthenticationToken,
        Accept,
        CreateConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        IncrementalAuthorization,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        Void,
        RepeatPayment,
        SetupMandate,
        SubmitEvidence,
    ],
);
