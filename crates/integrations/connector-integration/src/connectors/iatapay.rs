pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder,
        DefendDispute, IncrementalAuthorization, MandateRevoke, PSync, PaymentMethodToken,
        PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment, ServerAuthenticationToken,
        ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence, Void, VoidPC,
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
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    IatapayAuthUpdateRequest, IatapayAuthUpdateResponse, IatapayErrorResponse,
    IatapayPaymentsRequest, IatapayPaymentsResponse, IatapayRefundRequest, IatapayRefundResponse,
    IatapayRefundSyncResponse, IatapaySyncResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn iatapay_flow_not_supported(flow: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::FlowNotSupported {
        flow: flow.to_string(),
        connector: "Iatapay".to_string(),
        context: Default::default(),
    })
}
fn iatapay_not_implemented(flow: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::not_implemented(format!(
        "{flow} flow for iatapay"
    )))
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("incremental_authorization"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Iatapay<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Iatapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Iatapay<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Iatapay<T>
{
}

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Iatapay<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Iatapay<T>
{
}

// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Iatapay<T>
{
}

// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Iatapay<T>
{
}

// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Iatapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Iatapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Iatapay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Iatapay<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Iatapay<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Iatapay<T>
{
}

// ===== MACRO-BASED SETUP =====
macros::create_all_prerequisites!(
    connector_name: Iatapay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: IatapayPaymentsRequest,
            response_body: IatapayPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: IatapaySyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: IatapayRefundRequest,
            response_body: IatapayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: IatapayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ServerAuthenticationToken,
            request_body: IatapayAuthUpdateRequest,
            response_body: IatapayAuthUpdateResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers_for_payments(
            &self,
            req: &RouterDataV2<impl Debug, PaymentFlowData, impl Debug, impl Debug>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];

            // Use access_token if available (for OAuth-enabled flows)
            let access_token = req.resource_common_data.access_token
                .as_ref()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let auth_header = (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", access_token.access_token.peek()).into_masked(),
            );
            header.push(auth_header);
            Ok(header)
        }

        pub fn build_headers_for_refunds(
            &self,
            req: &RouterDataV2<impl Debug, RefundFlowData, impl Debug, impl Debug>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];

            // Use access_token if available
            // Note: UCS doesn't automatically acquire OAuth tokens for refund flows yet
            // This is a known limitation that needs to be fixed in the grpc-server layer
            let access_token = req.resource_common_data.access_token
                .as_ref()
                .ok_or_else(|| {
                    tracing::error!(
                        "Access token not available for iatapay refund flow. \
                        This connector requires OAuth tokens for both payments and refunds. \
                        UCS currently only auto-acquires tokens for payment flows."
                    );
                    IntegrationError::FailedToObtainAuthType { context: Default::default() }
                })?;

            let auth_header = (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", access_token.access_token.peek()).into_masked(),
            );
            header.push(auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.iatapay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.iatapay.base_url
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Iatapay<T>
{
    fn id(&self) -> &'static str {
        "iatapay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.iatapay.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::IatapayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.client_id.into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: IatapayErrorResponse = res
            .response
            .parse_struct("IatapayErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "iatapay: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        if let Some(i) = event_builder {
            i.set_connector_response(&response);
        }

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error,
            message: response
                .message
                .unwrap_or_else(|| "Unknown error".to_string()),
            reason: response.reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ===== AUTHORIZE FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Iatapay,
    curl_request: Json(IatapayPaymentsRequest),
    curl_response: IatapayPaymentsResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_payments(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments/", self.connector_base_url_payments(req)))
        }
    }
);

// ===== EMPTY IMPLEMENTATIONS FOR OTHER FLOWS =====

// Payment Sync
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Iatapay,
    curl_response: IatapaySyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_payments(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Extract merchant_id from auth credentials
            let auth = transformers::IatapayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            let merchant_id = auth.merchant_id.peek();

            // Extract connector_request_reference_id from request
            let payment_id = req.resource_common_data.get_reference_id()?;

            Ok(format!(
                "{}/merchants/{}/payments/{}",
                self.connector_base_url_payments(req),
                merchant_id,
                payment_id
            ))
        }
    }
);

// Payment Void
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("void"))
    }
}

// Payment Void Post Capture
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("void_post_capture"))
    }
}

// Payment Capture
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("capture"))
    }
}

// Refund
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Iatapay,
    curl_request: Json(IatapayRefundRequest),
    curl_response: IatapayRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_refunds(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}/payments/{}/refund",
                self.connector_base_url_refunds(req),
                connector_payment_id
            ))
        }
    }
);

// Refund Sync
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Iatapay,
    curl_response: IatapayRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers_for_refunds(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Use connector_refund_id from RefundSyncData
            let refund_id = &req.request.connector_refund_id;

            Ok(format!(
                "{}/refunds/{}",
                self.connector_base_url_refunds(req),
                refund_id
            ))
        }
    }
);

// ===== ACCESS TOKEN FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Iatapay,
    curl_request: FormUrlEncoded(IatapayAuthUpdateRequest),
    curl_response: IatapayAuthUpdateResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // For OAuth, extract client_id and client_secret from IatapayAuthType
            let auth = transformers::IatapayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let client_id = auth.client_id.peek();
            let client_secret = auth.client_secret.peek();

            // Create Basic Auth: base64(client_id:client_secret)
            let credentials = format!("{client_id}:{client_secret}");
            let base64_credentials = BASE64_ENGINE.encode(credentials.as_bytes());

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/x-www-form-urlencoded".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Basic {base64_credentials}").into_masked(),
                ),
            ])
        }

        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }

        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/oauth/token", self.connector_base_url_payments(req)))
        }
    }
);

// Setup Mandate
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_not_implemented("setup_mandate"))
    }
}

// Repeat Payment
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_not_implemented("repeat_payment"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("mandate_revoke"))
    }
}

// Order Create
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_not_implemented("create_order"))
    }
}

// Session Token
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerSessionAuthenticationToken,
            PaymentFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_not_implemented(
            "create_server_session_authentication_token",
        ))
    }
}

// Dispute Accept
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("accept_dispute"))
    }
}

// Dispute Defend
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("defend_dispute"))
    }
}

// Submit Evidence
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SubmitEvidence,
            DisputeFlowData,
            SubmitEvidenceData,
            DisputeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("submit_evidence"))
    }
}

// Payment Token (required by PaymentTokenV2 trait)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<T>,
            PaymentMethodTokenResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_not_implemented("payment_method_token"))
    }
}

// ===== AUTHENTICATION FLOW CONNECTOR INTEGRATIONS =====
// Pre Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("pre_authenticate"))
    }
}

// Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("authenticate"))
    }
}

// Post Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("post_authenticate"))
    }
}

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::CreateConnectorCustomer,
            PaymentFlowData,
            ConnectorCustomerData,
            ConnectorCustomerResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_flow_not_supported("create_connector_customer"))
    }
}

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

// ===== ACCESS TOKEN SOURCE VERIFICATION =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Iatapay<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(iatapay_not_implemented(
            "create_client_authentication_token",
        ))
    }
}
