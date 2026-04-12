pub mod test;
pub mod transformers;
use common_enums::AttemptStatus;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::BytesExt,
    request::RequestContent,
    types::{AmountConvertor, MinorUnit},
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
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
use transformers as razorpayv2;

use super::macros;
use crate::connectors::razorpay::transformers::ForeignTryFrom;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

#[derive(Clone)]
pub struct RazorpayV2<T> {
    #[allow(dead_code)]
    pub(crate) amount_converter: &'static (dyn AmountConvertor<Output = MinorUnit> + Sync),
    #[allow(dead_code)]
    _phantom: std::marker::PhantomData<T>,
}

impl<T> RazorpayV2<T> {
    pub const fn new() -> &'static Self {
        &Self {
            amount_converter: &common_utils::types::MinorUnitForConnector,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for RazorpayV2<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for RazorpayV2<T>
{
    fn id(&self) -> &'static str {
        "razorpayv2"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn get_auth_header(
        &self,
        auth_type: &domain_types::router_data::ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = razorpayv2::RazorpayV2AuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.generate_authorization_header().into(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.razorpayv2.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        let response: razorpayv2::RazorpayV2ErrorResponse = res
            .response
            .parse_struct("RazorpayV2ErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpayv2: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }

        let (code, message, attempt_status) = match response {
            razorpayv2::RazorpayV2ErrorResponse::StandardError { error } => {
                let attempt_status = match error.code.as_str() {
                    "BAD_REQUEST_ERROR" => AttemptStatus::Failure,
                    "GATEWAY_ERROR" => AttemptStatus::Failure,
                    "AUTHENTICATION_ERROR" => AttemptStatus::AuthenticationFailed,
                    "AUTHORIZATION_ERROR" => AttemptStatus::AuthorizationFailed,
                    "SERVER_ERROR" => AttemptStatus::Pending,
                    _ => AttemptStatus::Pending,
                };
                (error.code, error.description.clone(), attempt_status)
            }
            razorpayv2::RazorpayV2ErrorResponse::SimpleError { message } => {
                // For simple error messages like "no Route matched with those values"
                // Default to failure status and use a generic error code
                (
                    "ROUTE_ERROR".to_string(),
                    message.clone(),
                    AttemptStatus::Failure,
                )
            }
        };

        Ok(domain_types::router_data::ErrorResponse {
            code,
            message: message.clone(),
            reason: Some(message),
            status_code: res.status_code,
            attempt_status: Some(attempt_status),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for RazorpayV2<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut auth_headers = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        headers.append(&mut auth_headers);
        Ok(headers)
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
        let base_url = &req.resource_common_data.connectors.razorpayv2.base_url;
        Ok(format!("{base_url}v1/orders"))
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
        let connector_router_data: razorpayv2::RazorpayV2RouterData<&PaymentCreateOrderData, T> =
            razorpayv2::RazorpayV2RouterData::try_from((
                req.request.amount,
                &req.request,
                Some(
                    req.resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
            ))?;
        let connector_req =
            razorpayv2::RazorpayV2CreateOrderRequest::try_from(&connector_router_data)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
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
        let response: razorpayv2::RazorpayV2CreateOrderResponse = res
            .response
            .parse_struct("RazorpayV2CreateOrderResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpayv2: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }

        let order_response = PaymentCreateOrderResponse {
            merchant_order_id: data.request.merchant_order_id.clone(),
            connector_order_id: response.id.clone(),
            session_data: None,
        };

        Ok(RouterDataV2 {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                connector_order_id: Some(response.id),
                ..data.resource_common_data.clone()
            },
            ..data.clone()
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        let response: razorpayv2::RazorpayV2ErrorResponse = res
            .response
            .parse_struct("RazorpayV2ErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpayv2: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }

        let (code, message) = match response {
            razorpayv2::RazorpayV2ErrorResponse::StandardError { error } => {
                (error.code, error.description.clone())
            }
            razorpayv2::RazorpayV2ErrorResponse::SimpleError { message } => {
                ("ROUTE_ERROR".to_string(), message.clone())
            }
        };

        Ok(domain_types::router_data::ErrorResponse {
            code,
            message: message.clone(),
            reason: Some(message),
            status_code: res.status_code,
            attempt_status: Some(AttemptStatus::Pending),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    > for RazorpayV2<T>
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
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut auth_headers = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        headers.append(&mut auth_headers);
        Ok(headers)
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
        let base_url = &req.resource_common_data.connectors.razorpayv2.base_url;

        // For UPI payments, use the specific UPI endpoint
        match &req.request.payment_method_data {
            PaymentMethodData::Upi(_) => Ok(format!("{base_url}v1/payments/create/upi")),
            _ => Ok(format!("{base_url}v1/payments")),
        }
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let order_id = req
            .resource_common_data
            .connector_order_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: Default::default(),
            })?
            .clone();
        let converted_amount = self
            .amount_converter
            .convert(req.request.minor_amount, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data = razorpayv2::RazorpayV2RouterData::try_from((
            converted_amount,
            req,
            Some(order_id),
            req.resource_common_data
                .address
                .get_payment_method_billing()
                .cloned(),
        ))?;
        // Always use v2 request format
        let connector_req =
            razorpayv2::RazorpayV2PaymentsRequest::try_from(&connector_router_data)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ConnectorError,
    > {
        // Try to parse as UPI response first
        let upi_response_result = res
            .response
            .parse_struct::<razorpayv2::RazorpayV2UpiPaymentsResponse>(
                "RazorpayV2UpiPaymentsResponse",
            );

        match upi_response_result {
            Ok(upi_response) => {
                if let Some(i) = event_builder {
                    i.set_connector_response(&upi_response)
                }

                // Use the transformer for UPI response handling
                RouterDataV2::foreign_try_from((
                    upi_response,
                    data.clone(),
                    res.status_code,
                    res.response.to_vec(),
                ))
                .change_context(
                    crate::utils::response_handling_fail_for_connector(
                        res.status_code,
                        "razorpayv2",
                    ),
                )
            }
            Err(_) => {
                // Fall back to regular payment response
                let response: razorpayv2::RazorpayV2PaymentsResponse = res
                    .response
                    .parse_struct("RazorpayV2PaymentsResponse")
                    .change_context(
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                        "razorpayv2: response body did not match the expected format; confirm API version and connector documentation."),
                    )?;

                if let Some(i) = event_builder {
                    i.set_connector_response(&response)
                }

                // Use the transformer for regular response handling
                RouterDataV2::foreign_try_from((
                    response,
                    data.clone(),
                    res.status_code,
                    res.response.to_vec(),
                ))
                .change_context(
                    crate::utils::response_handling_fail_for_connector(
                        res.status_code,
                        "razorpayv2",
                    ),
                )
            }
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

// Implement required traits for ConnectorServiceTrait
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for RazorpayV2<T>
{
}
// Type alias for non-generic trait implementations
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for RazorpayV2<T>
{
}
macros::macro_connector_payout_implementation!(
    connector: RazorpayV2,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for RazorpayV2<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SourceVerification for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for RazorpayV2<T>
{
}

// Stub implementations for flows not yet implemented
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for RazorpayV2<T>
{
    fn get_http_method(&self) -> common_utils::Method {
        common_utils::Method::Get
    }
    fn get_headers(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut auth_headers = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        headers.append(&mut auth_headers);
        Ok(headers)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = &req.resource_common_data.connectors.razorpayv2.base_url;

        // Check if connector_order_id is provided to determine URL pattern
        match &req.resource_common_data.connector_order_id {
            Some(ref_id) => {
                // Use orders endpoint when connector_order_id is provided
                Ok(format!("{base_url}v1/orders/{ref_id}/payments"))
            }
            None => {
                // Extract payment ID from connector_transaction_id for standard payment sync
                let payment_id = match &req.request.connector_transaction_id {
                    ResponseId::ConnectorTransactionId(id) => id,
                    ResponseId::EncodedData(data) => data,
                    ResponseId::NoResponseId => {
                        return Err(IntegrationError::MissingRequiredField {
                            field_name: "connector_transaction_id",
                            context: Default::default(),
                        }
                        .into());
                    }
                };

                Ok(format!("{base_url}v1/payments/{payment_id}"))
            }
        }
    }

    fn get_request_body(
        &self,
        _req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        // GET request doesn't need a body
        Ok(None)
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ConnectorError,
    > {
        // Parse the response using the enum that handles both collection and direct payment responses
        let sync_response: razorpayv2::RazorpayV2SyncResponse = res
            .response
            .parse_struct("RazorpayV2SyncResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpayv2: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        if let Some(i) = event_builder {
            i.set_connector_response(&sync_response)
        }

        // Use the transformer for PSync response handling
        RouterDataV2::foreign_try_from((
            sync_response,
            data.clone(),
            res.status_code,
            res.response.to_vec(),
        ))
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "razorpayv2",
        ))
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for RazorpayV2<T>
{
    fn get_http_method(&self) -> common_utils::Method {
        common_utils::Method::Get
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut auth_headers = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        headers.append(&mut auth_headers);
        Ok(headers)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = &req.resource_common_data.connectors.razorpayv2.base_url;

        // Extract refund ID from connector_refund_id
        let refund_id = &req.request.connector_refund_id;

        Ok(format!("{base_url}v1/refunds/{refund_id}"))
    }

    fn get_request_body(
        &self,
        _req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        // GET request doesn't need a body
        Ok(None)
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ConnectorError,
    > {
        let response: razorpayv2::RazorpayV2RefundResponse = res
            .response
            .parse_struct("RazorpayV2RefundResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpayv2: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }

        RouterDataV2::foreign_try_from((
            response,
            data.clone(),
            res.status_code,
            res.response.to_vec(),
        ))
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "razorpayv2",
        ))
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
    for RazorpayV2<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut auth_headers = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        headers.append(&mut auth_headers);
        Ok(headers)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = &req.resource_common_data.connectors.razorpayv2.base_url;
        let connector_payment_id = &req.request.connector_transaction_id;
        Ok(format!(
            "{base_url}v1/payments/{connector_payment_id}/refund"
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let converted_amount = self
            .amount_converter
            .convert(req.request.minor_refund_amount, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data = razorpayv2::RazorpayV2RouterData::<
            &RefundsData,
            DefaultPCIHolder,
        >::try_from((converted_amount, &req.request, None))?;
        let connector_req = razorpayv2::RazorpayV2RefundRequest::try_from(&connector_router_data)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ConnectorError,
    > {
        let response: razorpayv2::RazorpayV2RefundResponse = res
            .response
            .parse_struct("RazorpayV2RefundResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpayv2: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }

        RouterDataV2::foreign_try_from((
            response,
            data.clone(),
            res.status_code,
            res.response.to_vec(),
        ))
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "razorpayv2",
        ))
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<domain_types::router_data::ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for RazorpayV2<T>
{
}
// SourceVerification implementations for all flows

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    domain_types::connector_types::ConnectorSpecifications for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for RazorpayV2<T>
{
}
