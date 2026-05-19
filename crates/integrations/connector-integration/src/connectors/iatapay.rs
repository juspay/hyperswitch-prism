pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
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

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====

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

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Iatapay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Iatapay<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Iatapay<T>
{
}

// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
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
        _connector_config: &ConnectorSpecificConfig,
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

// Payment Void Post Capture

// Payment Capture

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

// Repeat Payment

// Order Create

// Session Token

// Dispute Accept

// Dispute Defend

// Submit Evidence

// Payment Token (required by PaymentTokenV2 trait)

// ===== AUTHENTICATION FLOW CONNECTOR INTEGRATIONS =====
// Pre Authentication

// Authentication

// Post Authentication

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

// ===== ACCESS TOKEN SOURCE VERIFICATION =====

macros::macro_connector_flow_status_impls!(
    connector: Iatapay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SetupMandate,
        RepeatPayment,
        CreateOrder,
        ServerSessionAuthenticationToken,
        PaymentMethodToken,
        ClientAuthenticationToken,
    ],
    not_supported: [
        IncrementalAuthorization,
        Void,
        VoidPC,
        Capture,
        MandateRevoke,
        Accept,
        DefendDispute,
        SubmitEvidence,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        CreateConnectorCustomer,
    ],
);
