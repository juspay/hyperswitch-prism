pub mod transformers;

use std::fmt::Debug;

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, FloatMajorUnit};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::{
    connector_flow::{Authorize, PSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as gigadat, GigadatPaymentsRequest, GigadatPaymentsResponse, GigadatRefundRequest,
    GigadatRefundResponse, GigadatSyncResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

macros::create_all_prerequisites!(
    connector_name: Gigadat,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GigadatPaymentsRequest,
            response_body: GigadatPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: GigadatSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GigadatRefundRequest,
            response_body: GigadatRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
        ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            self.get_content_type().to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_config)?;
        header.append(&mut api_key);
        Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.gigadat.base_url
        }
         pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.gigadat.base_url
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Gigadat<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Gigadat<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Gigadat<T>
{
}

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Gigadat<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
macros::macro_connector_payout_implementation!(
    connector: Gigadat,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Gigadat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Gigadat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Gigadat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Gigadat<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Gigadat<T>
{
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
// ===== AUTHORIZE FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Gigadat,
    curl_request: Json(GigadatPaymentsRequest),
    curl_response: GigadatPaymentsResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let auth = gigadat::GigadatAuthType::try_from(&req.connector_config)
            .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
        Ok(format!(
            "{}api/payment-token/{}",
            self.connector_base_url_payments(req),
            auth.campaign_id.peek()
        ))
        }
    }
);

// ===== PSYNC FLOW IMPLEMENTATION =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Gigadat,
    curl_response: GigadatSyncResponse,
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
            self.build_headers(req)
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
        Ok(format!(
            "{}api/transactions/{}",
            self.connector_base_url_payments(req),
            transaction_id
        ))
        }
    }
);

// ===== REFUND FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Gigadat,
    curl_request: Json(GigadatRefundRequest),
    curl_response: GigadatRefundResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
            "{}refunds",
            self.connector_base_url_refunds(req)
        ))
        }
        fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Refund has different error format
        let response: gigadat::GigadatRefundErrorResponse = res
            .response
            .parse_struct("GigadatRefundErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(res.status_code, "gigadat: response body did not match the expected format; confirm API version and connector documentation."))?;

        with_error_response_body!(event_builder, response);

        let code = response
            .error
            .first()
            .and_then(|e| e.code.clone())
            .unwrap_or_else(|| "REFUND_ERROR".to_string());
        let message = response
            .error
            .first()
            .map(|e| e.detail.clone())
            .unwrap_or_else(|| "Refund error".to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None
})
    }
    }
);

// Payment Void - Not supported

// Payment Void Post Capture - Not supported

// Payment Capture - Not supported

// Refund Sync - Not supported by Gigadat

// Setup Mandate

// Repeat Payment

// Order Create

// Session Token

// Dispute Accept

// Dispute Defend

// Submit Evidence

// Payment Token (required by PaymentTokenV2 trait)

// Access Token (required by ServerAuthentication trait)

// ===== AUTHENTICATION FLOW CONNECTOR INTEGRATIONS =====
// Pre Authentication

// Authentication

// Post Authentication

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Gigadat<T>
{
    fn id(&self) -> &'static str {
        "gigadat"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base // Gigadat uses FloatMajorUnit
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.gigadat.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = gigadat::GigadatAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        // Build Basic Auth: base64(access_token:security_token)
        let auth_key = format!(
            "{}:{}",
            auth.access_token.peek(),
            auth.security_token.peek()
        );
        let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth_header.into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: gigadat::GigadatErrorResponse = res
            .response
            .parse_struct("GigadatErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "gigadat: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.err.clone(),
            message: response.err.clone(),
            reason: Some(response.err),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Gigadat,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        RepeatPayment,
    ],
    not_supported: [
        Void,
        VoidPC,
        Capture,
        RSync,
        SetupMandate,
        CreateOrder,
        ServerSessionAuthenticationToken,
        Accept,
        DefendDispute,
        SubmitEvidence,
        PaymentMethodToken,
        ServerAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        IncrementalAuthorization,
        CreateConnectorCustomer,
        ClientAuthenticationToken,
        MandateRevoke,
    ],
);
