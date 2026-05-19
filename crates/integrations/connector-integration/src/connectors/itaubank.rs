pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, request::RequestContent,
};
use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors::{self},
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::*,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::ExposeInterface;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;

use crate::types::ResponseRouterData;

use self::transformers::{
    ItaubankAccessTokenRequest, ItaubankAccessTokenResponse, ItaubankAuthType,
    ItaubankErrorResponse, ItaubankPayoutGetResponse, ItaubankTransferRequest,
    ItaubankTransferResponse,
};
pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const USER_AGENT: &str = "User-Agent";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_ITAU_API_KEY: &str = "x-itau-apikey";
}

use std::fmt::Debug;

use super::macros;

// ===== MACRO PREREQUISITES =====
macros::create_all_prerequisites!(
    connector_name: Itaubank,
    generic_type: T,
    api: [
        (
            flow: PayoutGet,
            response_body: ItaubankPayoutGetResponse,
            router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {}
);

macros::macro_connector_payout_implementation!(
    connector: Itaubank,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutCreate,
        PayoutVoid,
        PayoutStage,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);

fn construct_itaubank_error_message(error_res: &ItaubankErrorResponse) -> String {
    let campos_msg = if error_res.campos.is_empty() {
        None
    } else {
        Some(
            error_res
                .campos
                .iter()
                .map(|c| format!("{}: {}", c.campo, c.mensagem))
                .collect::<Vec<String>>()
                .join(", "),
        )
    };

    match (error_res.mensagem.clone(), campos_msg) {
        (Some(msg), Some(campos)) => format!("{} | {}", msg, campos),
        (Some(msg), None) => msg,
        (None, Some(campos)) => campos,
        (None, None) => "Unknown error".to_string(),
    }
}

// ===== CONNECTOR COMMON IMPL =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Itaubank<T>
{
    fn id(&self) -> &'static str {
        "itaubank"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.itaubank.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: Result<ItaubankErrorResponse, _> =
            res.response.parse_struct("ItaubankErrorResponse");

        match response {
            Ok(error_res) => {
                event_builder.map(|i| i.set_connector_response(&error_res));

                let message = construct_itaubank_error_message(&error_res);

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_res.codigo,
                    message,
                    reason: error_res.mensagem,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            Err(_) => {
                tracing::error!(
                    "Failed to parse error response from Itaubank. Status: {}, Raw: {:?}",
                    res.status_code,
                    res.response
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: res.status_code.to_string(),
                    message: "Failed to parse error response from connector".to_string(),
                    reason: Some(format!("Raw response: {:?}", res.response)),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
        }
    }
}

// ===== VALIDATION TRAIT =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Itaubank<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

// ===== CONNECTOR SERVICE TRAIT =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Itaubank<T>
{
}

// ===== ACCESS TOKEN FLOW =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Itaubank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Itaubank<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, errors::IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, errors::IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.private_key)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        // if secondary_base_url is present, use it, else use base_url
        if let Some(secondary_base_url) = req
            .resource_common_data
            .connectors
            .itaubank
            .secondary_base_url
            .as_deref()
        {
            Ok(format!("{}/api/oauth/token", secondary_base_url))
        } else {
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!("{}/api/oauth/jwt", base_url))
        }
    }

    fn get_headers(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            ),
            (headers::ACCEPT.to_string(), "*/*".to_string().into()),
            (
                headers::USER_AGENT.to_string(),
                "Hyperswitch".to_string().into(),
            ),
        ])
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, errors::IntegrationError> {
        let connector_req = ItaubankAccessTokenRequest::try_from(req)?;
        Ok(Some(RequestContent::FormUrlEncoded(Box::new(
            connector_req,
        ))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
        errors::ConnectorError,
    > {
        let response: Result<ItaubankAccessTokenResponse, _> =
            res.response.parse_struct("ItaubankAccessTokenResponse");

        match response {
            Ok(token_res) => {
                event_builder.map(|i| i.set_connector_response(&token_res));
                let access_token_data = ServerAuthenticationTokenResponseData {
                    access_token: token_res.access_token.into(),
                    token_type: token_res.token_type,
                    expires_in: token_res.expires_in,
                };

                Ok(RouterDataV2 {
                    response: Ok(access_token_data),
                    ..data.clone()
                })
            }
            Err(_) => {
                tracing::error!(
                    "Failed to parse access token response from Itaubank. Status: {}, Raw: {:?}",
                    res.status_code,
                    res.response
                );
                Err(errors::ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                }
                .into())
            }
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

// ===== PAYOUT TRANSFER FLOW =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PayoutTransferV2 for Itaubank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for Itaubank<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, errors::IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, errors::IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.private_key)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        let base_url = build_env_specific_endpoint(
            self.base_url(&req.resource_common_data.connectors),
            req.resource_common_data.test_mode,
        );
        Ok(format!("{base_url}/v1/transferencias"))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let access_token = req.resource_common_data.get_access_token().map_err(|_| {
            errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
        })?;
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;

        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ),
            (headers::ACCEPT.to_string(), "*/*".to_string().into()),
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {access_token}").into_masked(),
            ),
            (
                headers::USER_AGENT.to_string(),
                "Hyperswitch".to_string().into(),
            ),
            (
                headers::X_ITAU_API_KEY.to_string(),
                auth.client_id.expose().into_masked(),
            ),
        ])
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, errors::IntegrationError> {
        let connector_req = ItaubankTransferRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        errors::ConnectorError,
    > {
        let response: Result<ItaubankTransferResponse, _> =
            res.response.parse_struct("ItaubankTransferResponse");

        match response {
            Ok(transfer_res) => {
                event_builder.map(|i| i.set_connector_response(&transfer_res));
                Ok(RouterDataV2 {
                    response: Ok(PayoutTransferResponse {
                        merchant_payout_id: None,
                        payout_status: transfer_res.transfer_status.get_payout_status(),
                        connector_payout_id: Some(transfer_res.id),
                        status_code: res.status_code,
                    }),
                    ..data.clone()
                })
            }
            Err(_) => {
                tracing::error!(
                    "Failed to parse transfer response from Itaubank. Status: {}, Raw: {:?}",
                    res.status_code,
                    res.response
                );
                Err(errors::ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                }
                .into())
            }
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

// ===== PAYOUT GET FLOW =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PayoutGetV2 for Itaubank<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Itaubank,
    curl_response: ItaubankPayoutGetResponse,
    flow_name: PayoutGet,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutGetRequest,
    flow_response: PayoutGetResponse,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let access_token = req.resource_common_data.get_access_token().map_err(|_| {
                errors::IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                }
            })?;
            let auth = ItaubankAuthType::try_from(&req.connector_config)?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (headers::ACCEPT.to_string(), "*/*".to_string().into()),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into_masked(),
                ),
                (
                    headers::USER_AGENT.to_string(),
                    "Hyperswitch".to_string().into(),
                ),
                (
                    headers::X_ITAU_API_KEY.to_string(),
                    auth.client_id.expose().into_masked(),
                )
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = build_env_specific_endpoint(
                self.base_url(&req.resource_common_data.connectors),
                req.resource_common_data.test_mode,
            );
            let connector_payout_id = req.request.connector_payout_id.clone().ok_or(errors::IntegrationError::MissingConnectorTransactionID{ context: Default::default() })?;
            Ok(format!("{}v1/pagamentos_sispag/{}", base_url, connector_payout_id))
        }
        fn get_certificate(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, errors::IntegrationError> {
            let auth = ItaubankAuthType::try_from(&req.connector_config)?;
            Ok(auth.certificates)
        }
        fn get_certificate_key(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, errors::IntegrationError> {
            let auth = ItaubankAuthType::try_from(&req.connector_config)?;
            Ok(auth.private_key)
        }
    }
);

fn build_env_specific_endpoint(base_url: &str, test_mode: Option<bool>) -> String {
    if test_mode.unwrap_or(true) {
        format!("{base_url}/itau-ep9-gtw-sispag-ext")
    } else {
        format!("{base_url}/sispag")
    }
}

// ===== NO-OP PAYMENT TRAIT IMPLS =====
// VerifyWebhookSource is provided by default_impl_verify_webhook_source_v2! in default_implementations.rs

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Itaubank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Itaubank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Itaubank<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Itaubank<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Itaubank,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Authorize,
        PSync,
        Refund,
        RSync,
        SetupMandate,
        RepeatPayment,
    ],
    not_supported: [
        Void,
        VoidPC,
        Capture,
        ClientAuthenticationToken,
        MandateRevoke,
        CreateOrder,
        ServerSessionAuthenticationToken,
        IncrementalAuthorization,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        Accept,
        SubmitEvidence,
        DefendDispute,
        CreateConnectorCustomer,
    ],
);
