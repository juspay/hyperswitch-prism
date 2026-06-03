pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_flow::{Authorize, ClientAuthenticationToken},
    connector_types::{
        ClientAuthenticationTokenRequestData, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, RefundFlowData,
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
    self as przelewy24, Przelewy24ClientAuthRequest, Przelewy24ClientAuthResponse,
    Przelewy24PaymentsRequest, Przelewy24PaymentsResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

macros::create_all_prerequisites!(
    connector_name: Przelewy24,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: Przelewy24PaymentsRequest,
            response_body: Przelewy24PaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: Przelewy24ClientAuthRequest,
            response_body: Przelewy24ClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
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
            &req.resource_common_data.connectors.przelewy24.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.przelewy24.base_url
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Przelewy24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Przelewy24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Przelewy24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Przelewy24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Przelewy24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Przelewy24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Przelewy24<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Przelewy24<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Przelewy24,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== AUTHORIZE (transaction/register) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Przelewy24,
    curl_request: Json(Przelewy24PaymentsRequest),
    curl_response: Przelewy24PaymentsResponse,
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
            Ok(format!(
                "{}/api/v1/transaction/register",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ===== ClientAuthenticationToken (ClientSDKSessionToken) — same /transaction/register endpoint =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Przelewy24,
    curl_request: Json(Przelewy24ClientAuthRequest),
    curl_response: Przelewy24ClientAuthResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/api/v1/transaction/register",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ===== CONNECTOR COMMON =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Przelewy24<T>
{
    fn id(&self) -> &'static str {
        "przelewy24"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.przelewy24.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = przelewy24::Przelewy24AuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        // HTTP Basic over posId(merchantId):apiKey
        let basic = BASE64_ENGINE.encode(format!(
            "{}:{}",
            auth.merchant_id.peek(),
            auth.api_key.peek()
        ));
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {basic}").into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: przelewy24::Przelewy24ErrorResponse = res
            .response
            .parse_struct("Przelewy24ErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "przelewy24: response body did not match the expected format; confirm API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        let message = response
            .error
            .clone()
            .or_else(|| response.message.as_ref().map(|m| m.to_string()))
            .unwrap_or_else(|| "Przelewy24 error".to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .code
                .map(|c| c.to_string())
                .unwrap_or_else(|| res.status_code.to_string()),
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Przelewy24,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PSync,
        Capture,
        Void,
        Refund,
        RSync,
    ],
    not_supported: [
        VoidPC,
        SetupMandate,
        MandateRevoke,
        RepeatPayment,
        CreateOrder,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        CreateConnectorCustomer,
        PaymentMethodToken,
        IncrementalAuthorization,
        Accept,
        DefendDispute,
        SubmitEvidence,
    ],
);
