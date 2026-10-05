pub mod test;
pub mod transformers;
use std::sync::LazyLock;

use super::macros;
use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, EventClass, PaymentMethod, PaymentMethodType,
    RefundStatus,
};
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{Method, RequestContent},
    types::{AmountConvertor, MinorUnit},
};
use domain_types::errors::{IntegrationError, WebhookError};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateOrder, PSync, RSync, Refund, ServerSessionAuthenticationToken,
    },
    connector_types::{
        ConnectorSpecifications, ConnectorWebhookSecrets, EventContext, EventType,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RequestDetails, ResponseId, ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData, SupportedPaymentMethodsExt,
        WebhookDetailsResponse,
    },
    errors::ConnectorError,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
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
    connectors::razorpayv2::transformers::RazorpayV2SyncResponse, finalize_connector_response,
    types::ResponseRouterData, with_error_response_body,
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
    connector_types::ValidationTrait for Razorpay<T>
{
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
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
    fn should_do_order_create(&self) -> bool {
        true
    }
}

// Type alias for non-generic trait implementations
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
    connector_types::PaymentSyncV2 for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Razorpay<T>
{
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Razorpay<T>
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
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: razorpay::RazorpayErrorResponse =
            res.response.parse_struct("ErrorResponse").map_err(|_| {
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpay: response body did not match the expected format; confirm API version and connector documentation.")
            })?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
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
            attempt_status: Some(FlowStatus::Payment(attempt_status)),
            connector_transaction_id: None,
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
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
                let typed = events::MaskedSerdeValue::from_masked_optional(
                    &connector_req,
                    "typed_connector_request",
                );
                Ok(Some(common_utils::request::ConnectorRequestData::new(
                    RequestContent::FormUrlEncoded(Box::new(connector_req)),
                    typed,
                )))
            }
            PaymentMethodData::BankRedirect(
                domain_types::payment_method_data::BankRedirectData::Netbanking { .. },
            ) => {
                let connector_req =
                    razorpay::RazorpayNetbankingRequest::try_from(&connector_router_data)?;
                let typed = events::MaskedSerdeValue::from_masked_optional(
                    &connector_req,
                    "typed_connector_request",
                );
                Ok(Some(common_utils::request::ConnectorRequestData::new(
                    RequestContent::Json(Box::new(connector_req)),
                    typed,
                )))
            }
            _ => {
                let connector_req =
                    razorpay::RazorpayPaymentRequest::try_from(&connector_router_data)?;
                let typed = events::MaskedSerdeValue::from_masked_optional(
                    &connector_req,
                    "typed_connector_request",
                );
                Ok(Some(common_utils::request::ConnectorRequestData::new(
                    RequestContent::Json(Box::new(connector_req)),
                    typed,
                )))
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
        use domain_types::connector_types::RawConnectorRequestResponse;
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
                        // Serialize once for both event logging and typed_connector_response
                        let masked = events::MaskedSerdeValue::from_masked_optional(
                            &upi_response,
                            "connector_response",
                        );
                        if let Some(ref msv) = masked {
                            if let Some(evt) = event_builder {
                                evt.response_data = Some(msv.clone());
                            }
                        }

                        // Use the transformer for UPI response handling
                        let mut result = RouterDataV2::foreign_try_from((
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
                        )?;
                        result.resource_common_data.set_typed_connector_response(
                            masked.as_ref().map(|m| m.inner().to_string()),
                        );
                        Ok(result)
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

                        finalize_connector_response!(event_builder, response, data, res.status_code)
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

                finalize_connector_response!(event_builder, response, data, res.status_code)
            }
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
        use domain_types::connector_types::RawConnectorRequestResponse;
        let sync_response: RazorpayV2SyncResponse = res
            .response
            .parse_struct("RazorpayV2SyncResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "razorpay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        // Serialize once for both event logging and typed_connector_response
        let masked =
            events::MaskedSerdeValue::from_masked_optional(&sync_response, "connector_response");
        if let Some(ref msv) = masked {
            if let Some(evt) = event_builder {
                evt.response_data = Some(msv.clone());
            }
        }

        // Use the transformer for PSync response handling
        let mut result = RouterDataV2::foreign_try_from((
            sync_response,
            data.clone(),
            res.status_code,
            res.response.to_vec(),
        ))
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "razorpay",
        ))?;
        result
            .resource_common_data
            .set_typed_connector_response(masked.as_ref().map(|m| m.inner().to_string()));
        Ok(result)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let converted_amount = self
            .amount_converter
            .convert(req.request.amount, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data =
            razorpay::RazorpayRouterData::try_from((converted_amount, req))?;
        let connector_req = razorpay::RazorpayOrderRequest::try_from(&connector_router_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::FormUrlEncoded(Box::new(connector_req)),
            typed,
        )))
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

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Razorpay<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            ServerSessionAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
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
            ServerSessionAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
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
            ServerSessionAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let converted_amount = self
            .amount_converter
            .convert(req.request.amount, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data =
            razorpay::RazorpayRouterData::try_from((converted_amount, req))?;
        let connector_req =
            razorpay::RazorpaySessionTokenRequest::try_from(&connector_router_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::FormUrlEncoded(Box::new(connector_req)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            ServerSessionAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            ServerSessionAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
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

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
            connector_returned_payment_method_details: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(notif.entity.order_id)),
            status: transformers::get_razorpay_payment_webhook_status(
                notif.entity.entity,
                notif.entity.status,
            )?,
            mandate_reference: None,
            connector_response_reference_id: None,
            connector_request_reference_id: None,
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
            sender_payment_instrument_id: None,
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
            merchant_transaction_id: None,
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let converted_amount = self
            .amount_converter
            .convert(req.request.minor_refund_amount, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let refund_router_data = razorpay::RazorpayRouterData::try_from((converted_amount, req))?;
        let connector_req = razorpay::RazorpayRefundRequest::try_from(&refund_router_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::Json(Box::new(connector_req)),
            typed,
        )))
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

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let converted_amount = self
            .amount_converter
            .convert(req.request.minor_amount_to_capture, req.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let connector_router_data =
            razorpay::RazorpayRouterData::try_from((converted_amount, req))?;
        let connector_req = razorpay::RazorpayCaptureRequest::try_from(&connector_router_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::Json(Box::new(connector_req)),
            typed,
        )))
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

        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

impl connector_types::ConnectorValidation for Razorpay<DefaultPCIHolder> {
    fn validate_mandate_payment(
        &self,
        pm_type: Option<PaymentMethodType>,
        pm_data: PaymentMethodData<DefaultPCIHolder>,
    ) -> CustomResult<(), IntegrationError> {
        let mandate_supported_pmd = std::collections::HashSet::from([PaymentMethodDataType::Card]);
        is_mandate_supported(pm_data, pm_type, mandate_supported_pmd, self.id())
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

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector: Razorpay<T>,
    flow:      Authorize,
    source:    bool,
    context:   Option<common_enums::CaptureMethod>,
    params:    [has_next_action, capture_method],
    success:   _ => [Authorized, Charged],
    failure:   none,
    extractors: {
        request: PaymentsAuthorizeData<T>,
        response: razorpay::RazorpayResponse,
        // Both Authorize transformer paths map a `PaymentResponse` with
        // `has_next_action = true` (→ AuthenticationPending) and a `PsyncResponse`
        // via `get_psync_razorpay_payment_status`; surface `has_next_action` from the
        // response payload for the redirect branch.
        source: |response: &razorpay::RazorpayResponse| match response {
            razorpay::RazorpayResponse::PaymentResponse(payment_response) => {
                payment_response.next.is_some()
            }
            razorpay::RazorpayResponse::PsyncResponse(_) => false,
        },
        context: |request: &PaymentsAuthorizeData<T>, _response| request.capture_method,
    },
    {
        // Mirrors `get_authorization_razorpay_payment_status_from_action`.
        if has_next_action {
            AttemptStatus::AuthenticationPending
        } else if capture_method == Some(common_enums::CaptureMethod::Manual) {
            AttemptStatus::Authorized
        } else {
            AttemptStatus::Charged
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector: Razorpay<T>,
    flow:      PSync,
    source:    razorpay::RazorpayStatus,
    context:   (),
    params:    [status, _ctx],
    success:   _ => [Charged, AutoRefunded],
    failure:   none,
    extractors: {
        request: PaymentsSyncData,
        response: RazorpayV2SyncResponse,
        source: |response: &RazorpayV2SyncResponse| match response {
            RazorpayV2SyncResponse::PaymentResponse(payment) => {
                razorpay::RazorpayStatus::from(payment.status.clone())
            }
            RazorpayV2SyncResponse::OrderPaymentsCollection(collection) => collection
                .items
                .first()
                .map(|payment| razorpay::RazorpayStatus::from(payment.status.clone()))
                .unwrap_or(razorpay::RazorpayStatus::Created),
        },
        // The PSync `ForeignTryFrom` ignores the request's capture_method and hardcodes
        // `is_manual_capture = false`, so no mapping context is threaded through here.
        context: |_request: &PaymentsSyncData, _response| (),
    },
    {
        // Mirrors `get_psync_razorpay_payment_status(false, status)`.
        match status {
            razorpay::RazorpayStatus::Created => AttemptStatus::Pending,
            razorpay::RazorpayStatus::Authorized => AttemptStatus::Charged,
            razorpay::RazorpayStatus::Captured => AttemptStatus::Charged,
            razorpay::RazorpayStatus::Refunded => AttemptStatus::AutoRefunded,
            razorpay::RazorpayStatus::Failed => AttemptStatus::Failure,
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector: Razorpay<T>,
    flow:      Capture,
    source:    razorpay::RazorpayPaymentStatus,
    context:   (),
    params:    [status, _ctx],
    success:   _ => [Charged],
    failure:   none,
    extractors: {
        request: PaymentsCaptureData,
        response: razorpay::RazorpayCaptureResponse,
        source: |response: &razorpay::RazorpayCaptureResponse| response.status.clone(),
        context: |_request: &PaymentsCaptureData, _response| (),
    },
    {
        match status {
            razorpay::RazorpayPaymentStatus::Captured => AttemptStatus::Charged,
            razorpay::RazorpayPaymentStatus::Authorized => AttemptStatus::Authorized,
            razorpay::RazorpayPaymentStatus::Failed => AttemptStatus::Failure,
        }
    }
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector: Razorpay<T>,
    flow:      Refund,
    source:    razorpay::RazorpayRefundStatus,
    context:   (),
    params:    [status, _ctx],
    success:   _ => [Success],
    failure:   none,
    extractors: {
        request: RefundsData,
        response: razorpay::RazorpayRefundResponse,
        source: |response: &razorpay::RazorpayRefundResponse| response.status.clone(),
        context: |_request: &RefundsData, _response| (),
    },
    {
        match status {
            razorpay::RazorpayRefundStatus::Failed => RefundStatus::Failure,
            razorpay::RazorpayRefundStatus::Pending | razorpay::RazorpayRefundStatus::Created => {
                RefundStatus::Pending
            }
            razorpay::RazorpayRefundStatus::Processed => RefundStatus::Success,
        }
    }
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    connector: Razorpay<T>,
    flow:      RSync,
    source:    razorpay::RazorpayRefundStatus,
    context:   (),
    params:    [status, _ctx],
    success:   _ => [Success],
    failure:   none,
    extractors: {
        request: RefundSyncData,
        response: razorpay::RazorpayRefundResponse,
        source: |response: &razorpay::RazorpayRefundResponse| response.status.clone(),
        context: |_request: &RefundSyncData, _response| (),
    },
    {
        match status {
            razorpay::RazorpayRefundStatus::Failed => RefundStatus::Failure,
            razorpay::RazorpayRefundStatus::Pending | razorpay::RazorpayRefundStatus::Created => {
                RefundStatus::Pending
            }
            razorpay::RazorpayRefundStatus::Processed => RefundStatus::Success,
        }
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Razorpay,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SetupMandate,
        Accept,
        SubmitEvidence,
        DefendDispute,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
        ServerAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        RepeatPayment,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        VoidPC,
        Void,
        MandateRevoke,
    ],
);
