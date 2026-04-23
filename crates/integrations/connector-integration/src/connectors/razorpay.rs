pub mod test;
pub mod transformers;
use std::sync::LazyLock;

use super::macros;
use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, EventClass, PaymentMethod, PaymentMethodType,
};
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    pii::SecretSerdeValue,
    request::{Method, RequestContent},
    types::{AmountConvertor, MinorUnit},
};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence,
        Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, EventContext, EventType,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RequestDetails, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData, ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData, SetupMandateRequestData, SubmitEvidenceData,
        SupportedPaymentMethodsExt, WebhookDetailsResponse,
    },
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        CardSpecificFeatures, ConnectorInfo, Connectors, FeatureStatus, PaymentConnectorCategory,
        PaymentMethodDataType, PaymentMethodDetails, PaymentMethodSpecificFeatures,
        SupportedPaymentMethods,
    },
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, is_mandate_supported},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers::{self as razorpay, ForeignTryFrom};

use crate::{
    connectors::razorpayv2::transformers::RazorpayV2SyncResponse, with_error_response_body,
    with_response_body,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const ACCEPT: &str = "Accept";
}

#[derive(Clone)]
pub struct Razorpay<T> {
    #[allow(dead_code)]
    pub(crate) amount_converter: &'static (dyn AmountConvertor<Output = MinorUnit> + Sync),
    #[allow(dead_code)]
    _phantom: std::marker::PhantomData<T>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Razorpay<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }
}

// Type alias for non-generic trait implementations
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Razorpay<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Razorpay,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SourceVerification for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Razorpay<T>
{
}
impl<T> Razorpay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            amount_converter: &common_utils::types::MinorUnitForConnector,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for Razorpay<T>
{
    fn id(&self) -> &'static str {
        "razorpay"
    }
    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = razorpay::RazorpayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.generate_authorization_header().into_masked(),
        )])
    }
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.razorpay.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: razorpay::RazorpayErrorResponse =
            res.response.parse_struct("ErrorResponse").map_err(|_| {
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpay: response body did not match the expected format; confirm API version and connector documentation.")
            })?;

        with_error_response_body!(event_builder, response);

        let (code, message, reason, attempt_status) = match response {
            razorpay::RazorpayErrorResponse::StandardError { error } => {
                let attempt_status = match error.code.as_str() {
                    "BAD_REQUEST_ERROR" => AttemptStatus::Failure,
                    "GATEWAY_ERROR" => AttemptStatus::Failure,
                    "AUTHENTICATION_ERROR" => AttemptStatus::AuthenticationFailed,
                    "AUTHORIZATION_ERROR" => AttemptStatus::AuthorizationFailed,
                    "SERVER_ERROR" => AttemptStatus::Pending,
                    _ => AttemptStatus::Pending,
                };
                (error.code, error.description, error.reason, attempt_status)
            }
            razorpay::RazorpayErrorResponse::SimpleError { message } => {
                // For simple error messages like "no Route matched with those values"
                // Default to a generic error code
                (
                    "ROUTE_ERROR".to_string(),
                    message.clone(),
                    Some(message.clone()),
                    AttemptStatus::Failure,
                )
            }
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason,
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
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    > for Razorpay<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
    where
        Self: ConnectorIntegrationV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    {
        let content_type = match &req.request.payment_method_data {
            PaymentMethodData::Upi(_) => "application/x-www-form-urlencoded",
            _ => "application/json",
        };
        let mut header = vec![
            (
                headers::CONTENT_TYPE.to_string(),
                content_type.to_string().into(),
            ),
            (
                headers::ACCEPT.to_string(),
                "application/json".to_string().into(),
            ),
        ];
        let mut api_key = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        header.append(&mut api_key);
        Ok(header)
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
        let base_url = &req.resource_common_data.connectors.razorpay.base_url;

        // For UPI payments, use the specific UPI endpoint
        match &req.request.payment_method_data {
            PaymentMethodData::Upi(_) => Ok(format!("{base_url}v1/payments/create/upi")),
            _ => Ok(format!("{base_url}v1/payments/create/json")),
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
        let converted_amount = self
            .amount_converter
            .convert(req.request.minor_amount, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data =
            razorpay::RazorpayRouterData::try_from((converted_amount, req))?;

        match &req.request.payment_method_data {
            PaymentMethodData::Upi(_) => {
                let connector_req =
                    razorpay::RazorpayWebCollectRequest::try_from(&connector_router_data)?;
                Ok(Some(RequestContent::FormUrlEncoded(Box::new(
                    connector_req,
                ))))
            }
            PaymentMethodData::BankRedirect(
                domain_types::payment_method_data::BankRedirectData::Netbanking { .. },
            ) => {
                let connector_req =
                    razorpay::RazorpayNetbankingRequest::try_from(&connector_router_data)?;
                Ok(Some(RequestContent::Json(Box::new(connector_req))))
            }
            _ => {
                let connector_req =
                    razorpay::RazorpayPaymentRequest::try_from(&connector_router_data)?;
                Ok(Some(RequestContent::Json(Box::new(connector_req))))
            }
        }
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
        // Handle UPI payments differently from regular payments
        match &data.request.payment_method_data {
            PaymentMethodData::Upi(_) => {
                // Try to parse as UPI response first
                let upi_response_result = res
                    .response
                    .parse_struct::<razorpay::RazorpayUpiPaymentsResponse>(
                        "RazorpayUpiPaymentsResponse",
                    );

                match upi_response_result {
                    Ok(upi_response) => {
                        with_response_body!(event_builder, upi_response);

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
                                "razorpay",
                            ),
                        )
                    }
                    Err(_) => {
                        // Fall back to regular payment response
                        let response: razorpay::RazorpayResponse = res
                            .response
                            .parse_struct("RazorpayPaymentResponse")
                            .change_context(
                                crate::utils::response_deserialization_fail(
                                    res.status_code,
                                "razorpay: response body did not match the expected format; confirm API version and connector documentation."),
                            )?;

                        with_response_body!(event_builder, response);
                        RouterDataV2::foreign_try_from((
                            response,
                            data.clone(),
                            res.status_code,
                            data.request.capture_method,
                            false,
                            data.request.payment_method_type,
                        ))
                        .change_context(
                            crate::utils::response_handling_fail_for_connector(
                                res.status_code,
                                "razorpay",
                            ),
                        )
                    }
                }
            }
            _ => {
                // Regular payment response handling
                let response: razorpay::RazorpayResponse = res
                    .response
                    .parse_struct("RazorpayPaymentResponse")
                    .map_err(|_| {
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                        "razorpay: response body did not match the expected format; confirm API version and connector documentation.")
                    })?;

                with_response_body!(event_builder, response);

                RouterDataV2::foreign_try_from((
                    response,
                    data.clone(),
                    res.status_code,
                    data.request.capture_method,
                    false,
                    data.request.payment_method_type,
                ))
                .change_context(
                    crate::utils::response_handling_fail_for_connector(res.status_code, "razorpay"),
                )
            }
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for Razorpay<T>
{
    fn get_http_method(&self) -> Method {
        Method::Get
    }
    fn get_headers(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
    where
        Self: ConnectorIntegrationV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        header.append(&mut api_key);
        Ok(header)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = &req.resource_common_data.connectors.razorpay.base_url;

        // Check if connector_order_id is provided to determine URL pattern
        match &req.resource_common_data.connector_order_id {
            Some(ref_id) => {
                // Use orders endpoint when connector_order_id is provided
                Ok(format!("{base_url}v1/orders/{ref_id}/payments"))
            }
            None => {
                // Extract payment ID from connector_transaction_id for standard payment sync
                let payment_id = req
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: Default::default(),
                    })?;

                Ok(format!("{base_url}v1/payments/{payment_id}"))
            }
        }
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
        let sync_response: RazorpayV2SyncResponse = res
            .response
            .parse_struct("RazorpayV2SyncResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_response_body!(event_builder, sync_response);

        // Use the transformer for PSync response handling
        RouterDataV2::foreign_try_from((
            sync_response,
            data.clone(),
            res.status_code,
            res.response.to_vec(),
        ))
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "razorpay",
        ))
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Razorpay<T>
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
        let mut header = vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            ),
            (
                headers::ACCEPT.to_string(),
                "application/json".to_string().into(),
            ),
        ];
        let mut api_key = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        header.append(&mut api_key);
        Ok(header)
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
        Ok(format!(
            "{}v1/orders",
            req.resource_common_data.connectors.razorpay.base_url
        ))
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
        let converted_amount = self
            .amount_converter
            .convert(req.request.amount, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data =
            razorpay::RazorpayRouterData::try_from((converted_amount, req))?;
        let connector_req = razorpay::RazorpayOrderRequest::try_from(&connector_router_data)?;
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
        let response: razorpay::RazorpayOrderResponse = res
            .response
            .parse_struct("RazorpayOrderResponse")
            .map_err(|_| {
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpay: response body did not match the expected format; confirm API version and connector documentation.")
            })?;

        with_response_body!(event_builder, response);

        RouterDataV2::foreign_try_from((response, data.clone(), res.status_code, false))
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "razorpay",
            ))
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for Razorpay<T>
{
    fn get_http_method(&self) -> Method {
        Method::Get
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
    where
        Self: ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        header.append(&mut api_key);
        Ok(header)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let refund_id = req.request.connector_refund_id.clone();
        Ok(format!(
            "{}v1/refunds/{}",
            req.resource_common_data.connectors.razorpay.base_url, refund_id
        ))
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
        let response: razorpay::RazorpayRefundResponse = res
            .response
            .parse_struct("RazorpayRefundSyncResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_response_body!(event_builder, response);

        RouterDataV2::foreign_try_from((response, data.clone(), res.status_code)).change_context(
            crate::utils::response_handling_fail_for_connector(res.status_code, "razorpay"),
        )
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Razorpay<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"account_id":"probe_acct","contains":["payment"],"entity":"event","event":"payment.captured","payload":{"payment":{"entity":{"id":"pay_probe001","entity":"payment","amount":1000,"currency":"USD","status":"captured","order_id":"order_probe001"}}}}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let payload = transformers::get_webhook_object_from_body(request.body)?;

        if payload.refund.is_some() {
            Ok(EventType::RefundSuccess)
        } else {
            Ok(EventType::PaymentIntentSuccess)
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let payload = transformers::get_webhook_object_from_body(request.body)?;

        let notif = payload
            .payment
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookReferenceIdNotFound))?;

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(notif.entity.order_id)),
            status: transformers::get_razorpay_payment_webhook_status(
                notif.entity.entity,
                notif.entity.status,
            )?,
            mandate_reference: None,
            connector_response_reference_id: None,
            error_code: notif.entity.error_code,
            error_message: notif.entity.error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
            minor_amount_captured: None,
            amount_captured: None,
            error_reason: None,
            network_txn_id: None,
            payment_method_update: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let payload = transformers::get_webhook_object_from_body(request.body)?;

        let notif = payload
            .refund
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookReferenceIdNotFound))?;

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(notif.entity.id),
            status: transformers::get_razorpay_refund_webhook_status(
                notif.entity.entity,
                notif.entity.status,
            )?,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
    for Razorpay<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
    where
        Self: ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        header.append(&mut api_key);
        Ok(header)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let connector_payment_id = req.request.connector_transaction_id.clone();
        Ok(format!(
            "{}v1/payments/{}/refund",
            req.resource_common_data.connectors.razorpay.base_url, connector_payment_id
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
        let refund_router_data = razorpay::RazorpayRouterData::try_from((converted_amount, req))?;
        let connector_req = razorpay::RazorpayRefundRequest::try_from(&refund_router_data)?;

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
        let response: razorpay::RazorpayRefundResponse = res
            .response
            .parse_struct("RazorpayRefundResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_response_body!(event_builder, response);

        RouterDataV2::foreign_try_from((response, data.clone(), res.status_code)).change_context(
            crate::utils::response_handling_fail_for_connector(res.status_code, "razorpay"),
        )
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Razorpay<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
    where
        Self: ConnectorIntegrationV2<
            Capture,
            PaymentFlowData,
            PaymentsCaptureData,
            PaymentsResponseData,
        >,
    {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_config).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        header.append(&mut api_key);
        Ok(header)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let id = match &req.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id,
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into());
            }
        };
        Ok(format!(
            "{}v1/payments/{}/capture",
            req.resource_common_data.connectors.razorpay.base_url, id
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let converted_amount = self
            .amount_converter
            .convert(req.request.minor_amount_to_capture, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data =
            razorpay::RazorpayRouterData::try_from((converted_amount, req))?;
        let connector_req = razorpay::RazorpayCaptureRequest::try_from(&connector_router_data)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ConnectorError,
    > {
        let response: razorpay::RazorpayCaptureResponse = res
            .response
            .parse_struct("RazorpayCaptureResponse")
            .map_err(|err| {
                report!(
                    crate::utils::response_deserialization_fail(
                        res.status_code
                    , "razorpay: response body did not match the expected format; confirm API version and connector documentation.")
                )
                .attach_printable(format!("Failed to parse RazorpayCaptureResponse: {err:?}"))
            })?;

        with_response_body!(event_builder, response);

        RouterDataV2::foreign_try_from((response, data.clone(), res.status_code)).change_context(
            crate::utils::response_handling_fail_for_connector(res.status_code, "razorpay"),
        )
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Razorpay<T>
{
}

// SourceVerification implementations for all flows

impl connector_types::ConnectorValidation for Razorpay<DefaultPCIHolder> {
    fn validate_mandate_payment(
        &self,
        pm_type: Option<PaymentMethodType>,
        pm_data: PaymentMethodData<DefaultPCIHolder>,
    ) -> CustomResult<(), IntegrationError> {
        let mandate_supported_pmd = std::collections::HashSet::from([PaymentMethodDataType::Card]);
        is_mandate_supported(pm_data, pm_type, mandate_supported_pmd, self.id())
    }

    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _is_three_ds: bool,
        _status: AttemptStatus,
        _connector_meta_data: Option<SecretSerdeValue>,
    ) -> CustomResult<(), IntegrationError> {
        if data.encoded_data.is_some() {
            return Ok(());
        }
        Err(IntegrationError::MissingRequiredField {
            field_name: "encoded_data",
            context: Default::default(),
        }
        .into())
    }
    fn is_webhook_source_verification_mandatory(&self) -> bool {
        false
    }
}

static RAZORPAY_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> =
    LazyLock::new(|| {
        let razorpay_supported_capture_methods = vec![
            CaptureMethod::Automatic,
            CaptureMethod::Manual,
            CaptureMethod::ManualMultiple,
            // CaptureMethod::Scheduled,
        ];

        let razorpay_supported_card_network = vec![
            CardNetwork::Visa,
            CardNetwork::Mastercard,
            CardNetwork::AmericanExpress,
            CardNetwork::Maestro,
            CardNetwork::RuPay,
            CardNetwork::DinersClub,
            //have to add bajaj to this list too
            // ref : https://razorpay.com/docs/payments/payment-methods/cards/
        ];

        let mut razorpay_supported_payment_methods = SupportedPaymentMethods::new();

        razorpay_supported_payment_methods.add(
            PaymentMethod::Card,
            PaymentMethodType::Card,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: razorpay_supported_capture_methods.clone(),
                specific_features: Some(PaymentMethodSpecificFeatures::Card(
                    CardSpecificFeatures {
                        three_ds: FeatureStatus::NotSupported,
                        no_three_ds: FeatureStatus::Supported,
                        supported_card_networks: razorpay_supported_card_network.clone(),
                    },
                )),
            },
        );

        for wallet_type in [
            PaymentMethodType::LazyPay,
            PaymentMethodType::PhonePe,
            PaymentMethodType::BillDesk,
            PaymentMethodType::Cashfree,
            PaymentMethodType::PayU,
            PaymentMethodType::EaseBuzz,
        ] {
            razorpay_supported_payment_methods.add(
                PaymentMethod::Wallet,
                wallet_type,
                PaymentMethodDetails {
                    mandates: FeatureStatus::NotSupported,
                    refunds: FeatureStatus::Supported,
                    supported_capture_methods: vec![CaptureMethod::Automatic],
                    specific_features: None,
                },
            );
        }

        for upi_type in [
            PaymentMethodType::UpiCollect,
            PaymentMethodType::UpiIntent,
            PaymentMethodType::UpiQr,
        ] {
            razorpay_supported_payment_methods.add(
                PaymentMethod::Upi,
                upi_type,
                PaymentMethodDetails {
                    mandates: FeatureStatus::NotSupported,
                    refunds: FeatureStatus::NotSupported,
                    supported_capture_methods: vec![CaptureMethod::Automatic],
                    specific_features: None,
                },
            );
        }

        razorpay_supported_payment_methods.add(
            PaymentMethod::BankRedirect,
            PaymentMethodType::Netbanking,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: vec![CaptureMethod::Automatic],
                specific_features: None,
            },
        );

        razorpay_supported_payment_methods
    });

static RAZORPAY_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Razorpay",
    description: "Razorpay is a payment gateway that allows businesses to accept, process, and disburse payments with its product suite.",
    connector_type: PaymentConnectorCategory::PaymentGateway
};

static RAZORPAY_SUPPORTED_WEBHOOK_FLOWS: &[EventClass] =
    &[EventClass::Payments, EventClass::Refunds];

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Razorpay<T>
{
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&RAZORPAY_CONNECTOR_INFO)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [EventClass]> {
        Some(RAZORPAY_SUPPORTED_WEBHOOK_FLOWS)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&RAZORPAY_SUPPORTED_PAYMENT_METHODS)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::RepeatPayment,
        PaymentFlowData,
        domain_types::connector_types::RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Razorpay<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Razorpay<T>
{
}
