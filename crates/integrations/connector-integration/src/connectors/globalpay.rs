pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, request::RequestContent,
    types::StringMinorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateOrder,
        CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke, PSync,
        PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, SessionTokenRequestData, SessionTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    errors::{self},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as globalpay;
use transformers::{
    GlobalpayAuthorizeResponse, GlobalpayCaptureRequest, GlobalpayCaptureResponse,
    GlobalpayPSyncResponse, GlobalpayPaymentsRequest, GlobalpayRSyncResponse,
    GlobalpayRefundRequest, GlobalpayRefundResponse, GlobalpayVoidRequest, GlobalpayVoidResponse,
};

use crate::connectors::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use crate::with_response_body;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_GP_VERSION: &str = "X-GP-Version";
}

const API_VERSION: &str = "2021-03-22";

macros::create_amount_converter_wrapper!(connector_name: Globalpay, amount_type: StringMinorUnit);
macros::create_all_prerequisites!(
    connector_name: Globalpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GlobalpayPaymentsRequest<T>,
            response_body: GlobalpayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: GlobalpayPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: GlobalpayVoidRequest,
            response_body: GlobalpayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: GlobalpayCaptureRequest,
            response_body: GlobalpayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GlobalpayRefundRequest,
            response_body: GlobalpayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: GlobalpayRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        /// Build headers for payment flows with OAuth token
        pub fn build_payment_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let access_token = req
                .resource_common_data
                .get_access_token()
                .change_context(errors::ConnectorError::FailedToObtainAuthType)
                .attach_printable("Failed to get OAuth access token for GlobalPay")?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::X_GP_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
            ])
        }


        /// Build headers for sync flows with OAuth token (no content-type, no idempotency)
        pub fn build_payment_sync_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let access_token = req
                .resource_common_data
                .get_access_token()
                .change_context(errors::ConnectorError::FailedToObtainAuthType)
                .attach_printable("Failed to get OAuth access token for GlobalPay")?;

            Ok(vec![
                (
                    headers::X_GP_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
            ])
        }

        /// Build headers for refund flows with OAuth token
        pub fn build_refund_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let access_token = req
                .resource_common_data
                .get_access_token()
                .change_context(errors::ConnectorError::FailedToObtainAuthType)
                .attach_printable("Failed to get OAuth access token for GlobalPay refund flow")?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::X_GP_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
            ])
        }

        /// Build headers for refund sync flows with OAuth token (no content-type, no idempotency)
        pub fn build_refund_sync_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let access_token = req
                .resource_common_data
                .get_access_token()
                .change_context(errors::ConnectorError::FailedToObtainAuthType)
                .attach_printable("Failed to get OAuth access token for GlobalPay refund flow")?;

            Ok(vec![
                (
                    headers::X_GP_VERSION.to_string(),
                    API_VERSION.to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
            ])
        }

        /// Get base URL for payment endpoints
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpay.base_url
        }

        /// Get base URL for refund endpoints
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpay.base_url
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
// Main service trait - aggregates all other traits

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Globalpay<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Globalpay<T>
{
}

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Globalpay<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Globalpay<T>
{
}

// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Globalpay<T>
{
}

// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Globalpay<T>
{
}

// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Globalpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Globalpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Globalpay<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Globalpay<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Globalpay<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Globalpay<T>
{
}

// ===== MAIN CONNECTOR INTEGRATION IMPLEMENTATIONS =====
// Authorize flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayPaymentsRequest<T>),
    curl_response: GlobalpayAuthorizeResponse,
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
            self.build_payment_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!("{}/transactions", self.connector_base_url_payments(req)))
        }
    }
);

// Payment Sync flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_response: GlobalpayPSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_payment_sync_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let transaction_id = req
                .request
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;
            Ok(format!("{}/transactions/{}", self.connector_base_url_payments(req), transaction_id))
        }
    }
);

// Payment Void flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayVoidRequest),
    curl_response: GlobalpayVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_payment_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let transaction_id = &req.request.connector_transaction_id;
            Ok(format!("{}/transactions/{}/reversal", self.connector_base_url_payments(req), transaction_id))
        }
    }
);

// Payment Void Post Capture
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Globalpay<T>
{
}

// Payment Capture flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayCaptureRequest),
    curl_response: GlobalpayCaptureResponse,
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
            self.build_payment_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let transaction_id = req
                .request
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;
            Ok(format!("{}/transactions/{}/capture", self.connector_base_url_payments(req), transaction_id))
        }
    }
);

// Refund flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_request: Json(GlobalpayRefundRequest),
    curl_response: GlobalpayRefundResponse,
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
            self.build_refund_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/transactions/{}/refund", self.connector_base_url_refunds(req), transaction_id))
        }
    }
);

// Refund Sync flow implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Globalpay,
    curl_response: GlobalpayRSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_refund_sync_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/transactions/{}", self.connector_base_url_refunds(req), refund_id))
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
    > for Globalpay<T>
{
}

// Repeat Payment
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Globalpay<T>
{
}

// Order Create
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Globalpay<T>
{
}

// Session Token
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Globalpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Globalpay<T>
{
}

// Dispute Accept
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Globalpay<T>
{
}

// Dispute Defend
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Globalpay<T>
{
}

// Submit Evidence
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Globalpay<T>
{
}

// Payment Token (required by PaymentTokenV2 trait)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Globalpay<T>
{
}

// Access Token (required by PaymentAccessToken trait)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Globalpay<T>
{
    fn get_headers(
        &self,
        _req: &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (
                headers::X_GP_VERSION.to_string(),
                API_VERSION.to_string().into(),
            ),
        ])
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    ) -> CustomResult<String, errors::ConnectorError> {
        let base_url = &req.resource_common_data.connectors.globalpay.base_url;
        Ok(format!("{base_url}/accesstoken"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, errors::ConnectorError> {
        let request = globalpay::GlobalpayAccessTokenRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(request))))
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, errors::ConnectorError> {
        let request_body = self.get_request_body(req)?;
        let mut request_builder = common_utils::request::RequestBuilder::new()
            .method(common_utils::request::Method::Post)
            .url(&self.get_url(req)?)
            .attach_default_headers()
            .headers(self.get_headers(req)?);

        if let Some(body) = request_body {
            request_builder = request_builder.set_body(body);
        }

        Ok(Some(request_builder.build()))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
        errors::ConnectorError,
    > {
        let response: globalpay::GlobalpayAccessTokenResponse = res
            .response
            .parse_struct("GlobalpayAccessTokenResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

        with_response_body!(event_builder, response);

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder)
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
    > for Globalpay<T>
{
}

// Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Globalpay<T>
{
}

// Post Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Globalpay<T>
{
}

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Globalpay<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Globalpay<T>
{
    fn id(&self) -> &'static str {
        "globalpay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.globalpay.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
        // Note: This method should not be used for OAuth-based connectors
        // Use build_payment_headers or build_refund_headers instead
        Err(errors::ConnectorError::FailedToObtainAuthType.into())
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: globalpay::GlobalpayErrorResponse = res
            .response
            .parse_struct("GlobalpayErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code,
            message: response.detailed_error_description,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}
