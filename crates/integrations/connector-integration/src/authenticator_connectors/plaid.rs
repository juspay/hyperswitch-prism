pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{ClientAuthenticationToken, GetPaymentMethod, PaymentMethodToken},
    connector_types::{
        ClientAuthenticationTokenRequestData, GetPaymentMethodData, GetPaymentMethodResponseData,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentsResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as plaid, PlaidAuthGetRequest, PlaidAuthGetResponse, PlaidLinkTokenRequest,
    PlaidLinkTokenResponse, PlaidPublicTokenExchangeRequest, PlaidPublicTokenExchangeResponse,
};

use super::super::connectors::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

crate::common_macros::create_amount_converter_wrapper!(connector_name: Plaid, amount_type: FloatMajorUnit);

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

macros::create_all_prerequisites!(
    connector_name: Plaid,
    generic_type: T,
    api: [
        (
            flow: ClientAuthenticationToken,
            request_body: PlaidLinkTokenRequest,
            response_body: PlaidLinkTokenResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: PlaidPublicTokenExchangeRequest,
            response_body: PlaidPublicTokenExchangeResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: GetPaymentMethod,
            request_body: PlaidAuthGetRequest,
            response_body: PlaidAuthGetResponse,
            router_data: RouterDataV2<GetPaymentMethod, PaymentFlowData, GetPaymentMethodData, GetPaymentMethodResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {}
);

// =============================================================================
// CONNECTOR COMMON
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Plaid<T>
{
    fn id(&self) -> &'static str {
        "plaid"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.plaid.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Plaid inlines credentials in the JSON body, not HTTP headers.
        // Auth is handled per-request in transformers; return just Content-Type here.
        let _ = plaid::PlaidAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::CONTENT_TYPE.to_string(),
            self.common_get_content_type().to_string().into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: plaid::PlaidErrorResponse =
            res.response
                .parse_struct("PlaidErrorResponse")
                .change_context(ConnectorError::ResponseDeserializationFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some(
                            "failed to parse Plaid error body as PlaidErrorResponse".to_owned(),
                        ),
                    },
                })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .display_message
                .clone()
                .or(response.error_message.clone())
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.error_message.or(response.display_message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Plaid<T>
{
}

// =============================================================================
// BASE TRAIT IMPLS
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Plaid<T>
{
    fn should_do_payment_method_token(
        &self,
        _payment_method: common_enums::PaymentMethod,
        _payment_method_type: Option<common_enums::PaymentMethodType>,
    ) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Plaid<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Plaid<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Plaid<T>
{
}

// =============================================================================
// AUTHENTICATOR SERVICE TRAIT
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Plaid<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Plaid<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::GetPaymentMethodV2 for Plaid<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AuthenticatorServiceTrait<T> for Plaid<T>
{
}

// =============================================================================
// FLOW 1: ClientAuthenticationToken → POST /link/token/create
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Plaid,
    curl_request: Json(plaid::PlaidLinkTokenRequest),
    curl_response: plaid::PlaidLinkTokenResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/link/token/create",
                req.resource_common_data.connectors.plaid.base_url
            ))
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }
    }
);

// =============================================================================
// FLOW 2: PaymentMethodToken → POST /item/public_token/exchange
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Plaid,
    curl_request: Json(plaid::PlaidPublicTokenExchangeRequest),
    curl_response: plaid::PlaidPublicTokenExchangeResponse,
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
            req: &RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/item/public_token/exchange",
                req.resource_common_data.connectors.plaid.base_url
            ))
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }
    }
);

// =============================================================================
// FLOW 3: GetPaymentMethod → POST /auth/get
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Plaid,
    curl_request: Json(plaid::PlaidAuthGetRequest),
    curl_response: plaid::PlaidAuthGetResponse,
    flow_name: GetPaymentMethod,
    resource_common_data: PaymentFlowData,
    flow_request: GetPaymentMethodData,
    flow_response: GetPaymentMethodResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/auth/get",
                req.resource_common_data.connectors.plaid.base_url
            ))
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Plaid,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Authorize,
        PSync,
        Refund,
        SetupMandate,
        MandateRevoke,
        RepeatPayment,
        CreateConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        PaymentMethodEligibility,
    ],
    not_supported: [
        Capture,
        RSync,
        Void,
        VoidPC,
        VoidPostRefund,
        IncrementalAuthorization,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        CreateOrder,
        Accept,
        DefendDispute,
        SubmitEvidence,
    ]
);
