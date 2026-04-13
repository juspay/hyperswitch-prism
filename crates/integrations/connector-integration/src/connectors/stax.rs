pub mod transformers;

use std::fmt::Debug;

use common_enums::{CurrencyUnit, PaymentMethod, PaymentMethodType};
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateConnectorCustomer,
        IncrementalAuthorization, MandateRevoke, PSync, PaymentMethodToken, RSync, Refund, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorCustomerData, ConnectorCustomerResponse,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;

use self::transformers::{
    StaxAuthType, StaxAuthorizeRequest, StaxAuthorizeResponse, StaxCaptureRequest,
    StaxCaptureResponse, StaxCustomerRequest, StaxCustomerResponse, StaxErrorResponse,
    StaxPSyncRequest, StaxPSyncResponse, StaxRSyncRequest, StaxRSyncResponse, StaxRefundRequest,
    StaxRefundResponse, StaxTokenRequest, StaxTokenResponse, StaxVoidRequest, StaxVoidResponse,
};
use crate::{connectors::macros, types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// ===== CONNECTOR COMMON IMPLEMENTATION - Must be defined before macros =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Stax<T>
{
    fn id(&self) -> &'static str {
        "stax"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base // Stax API expects amounts in major units (dollars)
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.stax.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = StaxAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.expose()).into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: StaxErrorResponse = res
            .response
            .parse_struct("StaxErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "stax: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .code
                .clone()
                .unwrap_or_else(|| res.status_code.to_string()),
            message: response.get_error_message(),
            reason: Some(
                std::str::from_utf8(&res.response)
                    .change_context(
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                        "stax: response body did not match the expected format; confirm API version and connector documentation."),
                    )?
                    .to_owned(),
            ),
            attempt_status: None,
            connector_transaction_id: response.get_connector_transaction_id(),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ===== AUTHENTICATION FLOW IMPLEMENTATIONS =====
// Stax doesn't support 3DS authentication flows, but we need these empty implementations
// for ConnectorServiceTrait. These implementations satisfy the trait bounds.

use domain_types::connector_flow::{Authenticate, PostAuthenticate, PreAuthenticate};
use domain_types::connector_types::{
    PaymentsAuthenticateData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Stax<T>
{
}

// These trait implementations are derived from the above ConnectorIntegrationV2 implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Stax<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Stax,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Stax<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Stax<T>
{
    /// Enable automatic connector customer creation before payment
    /// When enabled, UCS will automatically call CreateConnectorCustomer before Authorize
    fn should_create_connector_customer(&self) -> bool {
        true
    }

    /// Enable automatic payment method tokenization before payment
    /// When enabled, UCS will automatically call PaymentMethodToken before Authorize
    fn should_do_payment_method_token(
        &self,
        payment_method: PaymentMethod,
        _payment_method_type: Option<PaymentMethodType>,
    ) -> bool {
        matches!(
            payment_method,
            PaymentMethod::Card | PaymentMethod::BankDebit
        )
    }
}

// ===== MACRO-BASED CONNECTOR IMPLEMENTATION =====
// Define connector struct and bridges for all flows
macros::create_all_prerequisites!(
    connector_name: Stax,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: StaxAuthorizeRequest,
            response_body: StaxAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: StaxPSyncRequest,
            response_body: StaxPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: StaxCaptureRequest,
            response_body: StaxCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: StaxVoidRequest,
            response_body: StaxVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: StaxRefundRequest,
            response_body: StaxRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: StaxRSyncRequest,
            response_body: StaxRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: StaxTokenRequest<T>,
            response_body: StaxTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: CreateConnectorCustomer,
            request_body: StaxCustomerRequest,
            response_body: StaxCustomerResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        )
    ],
    amount_converters: [],  // Stax uses MinorUnit (default)
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stax.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stax.base_url
        }
    }
);

// ===== FLOW IMPLEMENTATIONS USING MACROS =====

// Authorize Flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxAuthorizeRequest),
    curl_response: StaxAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/charge"))
        }
    }
);

// PSync Flow (GET, no request body)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxPSyncRequest),
    curl_response: StaxPSyncResponse,
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
            // GET requests don't need Content-Type header, only auth
            self.get_auth_header(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/transaction/{transaction_id}"))
        }
    }
);

// Capture Flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxCaptureRequest),
    curl_response: StaxCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/transaction/{transaction_id}/capture"))
        }
    }
);

// Void Flow (POST, no request body)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxVoidRequest),
    curl_response: StaxVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.connector_transaction_id.clone();
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/transaction/{transaction_id}/void"))
        }
    }
);

// Refund Flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxRefundRequest),
    curl_response: StaxRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.connector_transaction_id.clone();
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/transaction/{transaction_id}/refund"))
        }
    }
);

// RSync Flow (GET, no request body)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxRSyncRequest),
    curl_response: StaxRSyncResponse,
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
            // GET requests don't need Content-Type header, only auth
            self.get_auth_header(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let refund_id = req.request.connector_refund_id.clone();
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/transaction/{refund_id}"))
        }
    }
);

// PaymentMethodToken Flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxTokenRequest),
    curl_response: StaxTokenResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/payment-method/"))
        }
    }
);

// CreateConnectorCustomer Flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Stax,
    curl_request: Json(StaxCustomerRequest),
    curl_response: StaxCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/customer"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Stax<T>
{
}

// ===== TRAIT IMPLEMENTATIONS FOR SUPPORTED FLOWS =====
// These traits are auto-implemented based on ConnectorIntegrationV2, but we need explicit impls
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Stax<T>
{
}

// ===== EMPTY IMPLEMENTATIONS FOR UNSUPPORTED FLOWS =====
// These are required by ConnectorServiceTrait but not supported by Stax

use domain_types::connector_flow::{
    Accept, CreateOrder, DefendDispute, RepeatPayment, ServerAuthenticationToken,
    ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence, VoidPC,
};
use domain_types::connector_types::{
    AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
    PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentsCancelPostCaptureData,
    RepeatPaymentData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
    SetupMandateRequestData, SubmitEvidenceData,
};

// Order Create
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Stax<T>
{
}

// Session Token
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Stax<T>
{
}

// Access Token
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Stax<T>
{
}

// Void Post Capture
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Stax<T>
{
}

// Incoming Webhook
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Stax<T>
{
}

// Setup Mandate
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Stax<T>
{
}

// Repeat Payment
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Stax<T>
{
}

// Accept Dispute
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Stax<T>
{
}

// Defend Dispute
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Stax<T>
{
}

// Submit Evidence
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Stax<T>
{
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Now that all required traits are implemented, we can implement ConnectorServiceTrait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Stax<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Stax<T>
{
}
