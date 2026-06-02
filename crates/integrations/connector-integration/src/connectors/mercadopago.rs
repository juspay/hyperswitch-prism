pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts, errors::CustomResult, events, ext_traits::BytesExt, types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, ServerSessionAuthenticationToken},
    connector_types::{
        ConnectorSpecifications, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    MercadopagoErrorResponse, MercadopagoPaymentsRequest, MercadopagoPaymentsResponse,
    MercadopagoSessionTokenRequest, MercadopagoSessionTokenResponse,
};

use super::macros;
use crate::types::ResponseRouterData;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_IDEMPOTENCY_KEY: &str = "X-Idempotency-Key";
}

// =============================================================================
// BASE / MARKER TRAIT IMPLEMENTATIONS
// =============================================================================
macros::macro_connector_payout_implementation!(
    connector: Mercadopago,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Mercadopago<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Mercadopago<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Mercadopago<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Mercadopago<T>
{
    fn should_do_session_token(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Mercadopago<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Mercadopago<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Mercadopago<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Mercadopago<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Mercadopago<T>
{
}

// =============================================================================
// PREREQUISITES (struct, amount converter, member functions)
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Mercadopago,
    generic_type: T,
    api: [
        (
            flow: ServerSessionAuthenticationToken,
            request_body: MercadopagoSessionTokenRequest,
            response_body: MercadopagoSessionTokenResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: MercadopagoPaymentsRequest,
            response_body: MercadopagoPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = transformers::MercadopagoAuthType::try_from(&req.connector_config)?;
            let header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", auth.api_key.expose()).into(),
                ),
            ];
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.mercadopago.base_url
        }
    }
);

// =============================================================================
// CONNECTOR COMMON
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Mercadopago<T>
{
    fn id(&self) -> &'static str {
        "mercadopago"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.mercadopago.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::MercadopagoAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.expose()).into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: Result<
            MercadopagoErrorResponse,
            error_stack::Report<common_utils::errors::ParsingError>,
        > = res.response.parse_struct("MercadopagoErrorResponse");

        match response {
            Ok(response_data) => {
                if let Some(i) = event_builder {
                    i.set_connector_response(&response_data);
                }
                let code = response_data
                    .cause
                    .first()
                    .and_then(|c| c.code.as_ref().map(|v| v.to_string()))
                    .or_else(|| response_data.error.clone())
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
                let reason = response_data
                    .cause
                    .first()
                    .and_then(|c| c.description.clone());
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code,
                    message: if response_data.message.is_empty() {
                        consts::NO_ERROR_MESSAGE.to_string()
                    } else {
                        response_data.message.clone()
                    },
                    reason,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                })
            }
            Err(error_msg) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({
                        "error": "Error response parsing failed",
                        "status_code": res.status_code
                    }))
                };
                tracing::error!(deserialization_error =? error_msg);
                domain_types::utils::handle_json_response_deserialization_failure(
                    res,
                    "mercadopago",
                )
            }
        }
    }
}

// =============================================================================
// SERVER SESSION AUTHENTICATION TOKEN (ClientSDK session token)
// =============================================================================
// No connector REST endpoint creates the SDK session for card checkout. We call
// the authenticated `GET /v1/payment_methods` endpoint to validate credentials,
// then return the merchant `public_key` (from auth) as the session token used to
// initialize the MercadoPago.js client SDK.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Mercadopago,
    curl_response: MercadopagoSessionTokenResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payment_methods", self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// AUTHORIZE (Create Payment)
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Mercadopago,
    curl_request: Json(MercadopagoPaymentsRequest),
    curl_response: MercadopagoPaymentsResponse,
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
            let mut header = self.build_headers(req)?;
            header.push((
                headers::X_IDEMPOTENCY_KEY.to_string(),
                req.resource_common_data
                    .connector_request_reference_id
                    .clone()
                    .into(),
            ));
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// REMAINING FLOWS — stubbed as not implemented
// =============================================================================
macros::macro_connector_flow_status_impls!(
    connector: Mercadopago,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
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
        SetupMandate,
        SubmitEvidence
    ],
);
