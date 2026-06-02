pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, ClientAuthenticationToken},
    connector_types::{
        ClientAuthenticationTokenRequestData, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData,
    },
    errors,
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
use transformers as tpay;
use transformers::{TpayClientAuthRequest, TpayOAuthResponse, TpayPaymentsRequest, TpayPaymentsResponse};

use crate::connectors::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

macros::create_amount_converter_wrapper!(connector_name: Tpay, amount_type: FloatMajorUnit);
macros::create_all_prerequisites!(
    connector_name: Tpay,
    generic_type: T,
    api: [
        (
            flow: ClientAuthenticationToken,
            request_body: TpayClientAuthRequest,
            response_body: TpayOAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: Authorize,
            request_body: TpayPaymentsRequest<T>,
            response_body: TpayPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        /// Build headers for the OAuth token-acquisition call (no Bearer token yet).
        pub fn build_oauth_headers<F, Req, Res>(
            &self,
            _req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        /// Build headers for payment flows using the OAuth Bearer access token.
        pub fn build_payment_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req
                .resource_common_data
                .get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })
                .attach_printable("Failed to get OAuth access token for TPay")?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
            ])
        }

        /// Base URL for payment endpoints.
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.tpay.base_url
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Tpay<T>
{
    fn id(&self) -> &'static str {
        "tpay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, _connectors: &'a Connectors) -> &'a str {
        "https://api.tpay.com"
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: tpay::TpayErrorResponse = res
            .response
            .parse_struct("TpayErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        with_error_response_body!(event_builder, response);

        let code = response
            .code
            .or_else(|| response.error.clone())
            .unwrap_or_else(|| "NO_ERROR_CODE".to_string());
        let message = response
            .message
            .or(response.error_description)
            .or(response.error)
            .or(response.result)
            .unwrap_or_else(|| "NO_ERROR_MESSAGE".to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// =============================================================================
// SERVICE / FLOW TRAIT MARKERS
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Tpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Tpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Tpay<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Tpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Tpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Tpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Tpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Tpay<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Tpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// =============================================================================
// FLOW IMPLEMENTATIONS
// =============================================================================

// ----- ClientAuthenticationToken (Client SDK Session Token / OAuth) -----
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tpay,
    curl_request: FormUrlEncoded(TpayClientAuthRequest),
    curl_response: TpayOAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_oauth_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/oauth/auth", self.connector_base_url_payments(req)))
        }
    }
);

// ----- Authorize (create payment / charge) -----
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tpay,
    curl_request: Json(TpayPaymentsRequest<T>),
    curl_response: TpayPaymentsResponse,
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
            self.build_payment_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/transactions", self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// REMAINING (OUT-OF-SCOPE) FLOWS — stubbed as not implemented.
// =============================================================================
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Tpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        CreateConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        Capture,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PSync,
        PaymentMethodToken,
        VoidPC,
        Void,
        RSync,
        Refund,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
);
