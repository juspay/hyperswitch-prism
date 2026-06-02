pub mod transformers;

use crate::{common_macros, with_error_response_body, with_response_body};
use common_utils::{
    consts::NO_ERROR_MESSAGE,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{Method, RequestBuilder},
};
use domain_types::{
    connector_flow::{SurchargeCalculate, SurchargePaymentSucceeded, SurchargeRefundSucceeded},
    errors::{ConnectorError, IntegrationError, ResponseTransformationErrorContext},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargePaymentSucceededRequest, SurchargePaymentSucceededResponse,
        SurchargeRefundSucceededRequest, SurchargeRefundSucceededResponse,
    },
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        SurchargeCalculateV2, SurchargePaymentSucceededV2, SurchargeRefundSucceededV2,
        SurchargeServiceTrait,
    },
};
use transformers::{
    InterPaymentsSurchargeRequest, InterPaymentsSurchargeResponse, InterpaymentsAuthType,
    InterpaymentsErrorResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}
pub struct InterPayments;

impl InterPayments {
    pub const fn new() -> &'static Self {
        &Self
    }
}

impl ConnectorCommon for InterPayments {
    fn id(&self) -> &'static str {
        "interpayments"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.interpayments.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = InterpaymentsAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to obtain InterPayments authentication credentials".to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: InterpaymentsErrorResponse = res
            .response
            .parse_struct("InterPaymentsSurchargeResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                    "interpayments: response body did not match the expected format; confirm API version and connector documentation.",
                ),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.reason_code.clone(),
            message: response
                .message
                .clone()
                .unwrap_or(NO_ERROR_MESSAGE.to_string()),
            reason: response.reason.clone(),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

common_macros::create_amount_converter_wrapper!(connector_name: InterPayments, amount_type: FloatMajorUnit);

impl SurchargeServiceTrait for InterPayments {}
impl SurchargeCalculateV2 for InterPayments {}
impl SurchargePaymentSucceededV2 for InterPayments {}
impl SurchargeRefundSucceededV2 for InterPayments {}

impl InterPayments {
    pub fn connector_base_url_payments<'a, F, Req, Res>(
        &self,
        req: &'a RouterDataV2<F, SurchargeFlowData, Req, Res>,
    ) -> &'a str {
        &req.resource_common_data.connectors.interpayments.base_url
    }
}

impl
    ConnectorIntegrationV2<
        SurchargeCalculate,
        SurchargeFlowData,
        SurchargeCalculateRequest,
        SurchargeCalculateResponse,
    > for InterPayments
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            ConnectorIntegrationV2::<
                SurchargeCalculate,
                SurchargeFlowData,
                SurchargeCalculateRequest,
                SurchargeCalculateResponse,
            >::get_content_type(self)
            .to_string()
            .into(),
        )];

        let mut auth_header = self.get_auth_header(&req.connector_config)?;
        headers.append(&mut auth_header);
        Ok(headers)
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!("{}/ch", self.connector_base_url_payments(req)))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let request = InterPaymentsSurchargeRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::Json(Box::new(
            request,
        ))))
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        Ok(Some(
            RequestBuilder::new()
                .method(Method::Post)
                .url(self.get_url(req)?.as_str())
                .attach_default_headers()
                .headers(self.get_headers(req)?)
                .set_optional_body(self.get_request_body(req)?)
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            SurchargeCalculate,
            SurchargeFlowData,
            SurchargeCalculateRequest,
            SurchargeCalculateResponse,
        >,
        ConnectorError,
    > {
        let response: InterPaymentsSurchargeResponse = res
            .response
            .parse_struct("InterPaymentsSurchargeResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some("Failed to parse interpayments calculate surcharge response; expected JSON with status and data fields".to_string()),
                },
            })?;

        with_response_body!(event_builder, response);
        RouterDataV2::try_from(crate::types::ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "interpayments",
        ))
    }
    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

impl
    ConnectorIntegrationV2<
        SurchargePaymentSucceeded,
        SurchargeFlowData,
        SurchargePaymentSucceededRequest,
        SurchargePaymentSucceededResponse,
    > for InterPayments
{
    fn get_url(
        &self,
        req: &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!("{}/ch/sale", self.connector_base_url_payments(req)))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            ConnectorIntegrationV2::<
                SurchargePaymentSucceeded,
                SurchargeFlowData,
                SurchargePaymentSucceededRequest,
                SurchargePaymentSucceededResponse,
            >::get_content_type(self)
            .to_string()
            .into(),
        )];

        let mut auth_header = self.get_auth_header(&req.connector_config)?;
        headers.append(&mut auth_header);
        Ok(headers)
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        Ok(Some(
            RequestBuilder::new()
                .method(Method::Post)
                .url(self.get_url(req)?.as_str())
                .attach_default_headers()
                .headers(self.get_headers(req)?)
                .set_optional_body(self.get_request_body(req)?)
                .build(),
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let request = transformers::InterPaymentsPaymentSucceededRequest::from(req);
        Ok(Some(common_utils::request::RequestContent::Json(Box::new(
            request,
        ))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        >,
        ConnectorError,
    > {
        let response: transformers::InterPaymentsPaymentSucceededResponse = res
            .response
            .parse_struct("InterPaymentsPaymentSucceededResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Failed to parse interpayments payment succeeded response".to_string(),
                    ),
                },
            })?;

        with_response_body!(event_builder, response);

        let response_data = SurchargePaymentSucceededResponse {
            status_code: res.status_code,
        };

        let mut data = data.clone();
        data.response = Ok(response_data);
        Ok(data)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

impl
    ConnectorIntegrationV2<
        SurchargeRefundSucceeded,
        SurchargeFlowData,
        SurchargeRefundSucceededRequest,
        SurchargeRefundSucceededResponse,
    > for InterPayments
{
    fn get_url(
        &self,
        req: &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}/ch/refund",
            self.connector_base_url_payments(req)
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            ConnectorIntegrationV2::<
                SurchargeRefundSucceeded,
                SurchargeFlowData,
                SurchargeRefundSucceededRequest,
                SurchargeRefundSucceededResponse,
            >::get_content_type(self)
            .to_string()
            .into(),
        )];

        let mut auth_header = self.get_auth_header(&req.connector_config)?;
        headers.append(&mut auth_header);
        Ok(headers)
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        Ok(Some(
            RequestBuilder::new()
                .method(Method::Post)
                .url(self.get_url(req)?.as_str())
                .attach_default_headers()
                .headers(self.get_headers(req)?)
                .set_optional_body(self.get_request_body(req)?)
                .build(),
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let request = transformers::InterPaymentsRefundSucceededRequest::from(req);
        Ok(Some(common_utils::request::RequestContent::Json(Box::new(
            request,
        ))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        >,
        ConnectorError,
    > {
        let response: transformers::InterPaymentsRefundSucceededResponse = res
            .response
            .parse_struct("InterPaymentsRefundSucceededResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Failed to parse interpayments refund succeeded response".to_string(),
                    ),
                },
            })?;

        with_response_body!(event_builder, response);

        let response_data = SurchargeRefundSucceededResponse {
            status_code: res.status_code,
        };

        let mut data = data.clone();
        data.response = Ok(response_data);
        Ok(data)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}
