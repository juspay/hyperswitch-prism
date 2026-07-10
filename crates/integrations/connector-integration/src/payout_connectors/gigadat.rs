pub mod transformers;

use std::fmt::Debug;

use crate::connectors::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, FloatMajorUnit};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutGet, PayoutStage, PayoutTransfer, ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutCreateRequest, PayoutCreateResponse, PayoutFlowData, PayoutGetRequest,
        PayoutGetResponse, PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest,
        PayoutTransferResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateV2, PayoutGetV2, PayoutServiceTrait, PayoutStageV2, PayoutTransferV2,
        ServerAuthentication,
    },
};
use serde::Serialize;
use transformers::{
    self as gigadat, GigadatPayoutCreateResponse, GigadatPayoutMeta, GigadatPayoutStageRequest,
    GigadatPayoutStageResponse, GigadatPayoutSyncResponse, GigadatPayoutTransferResponse,
};

// ===== PAYOUT UTILITY FUNCTIONS =====
fn get_connector_payout_id(
    connector_payout_id: &Option<String>,
) -> CustomResult<String, IntegrationError> {
    connector_payout_id.as_ref().cloned().ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "connector_payout_id",
            context: Default::default(),
        }
        .into()
    })
}

fn get_connector_payout_or_quote_id(
    connector_payout_id: &Option<String>,
    connector_quote_id: &Option<String>,
) -> CustomResult<String, IntegrationError> {
    connector_payout_id
        .as_ref()
        .or(connector_quote_id.as_ref())
        .cloned()
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id or connector_quote_id",
                context: Default::default(),
            }
            .into()
        })
}

fn get_psp_token_from_payout_method_data(
    payout_method_data: &Option<domain_types::payouts::payout_method_data::PayoutMethodData>,
) -> CustomResult<Secret<String>, IntegrationError> {
    payout_method_data
        .as_ref()
        .and_then(|pmd| {
            if let domain_types::payouts::payout_method_data::PayoutMethodData::Passthrough(pt) =
                pmd
            {
                Some(pt.psp_token.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "psp_token (from payout_method_data.passthrough)",
                context: Default::default(),
            }
            .into()
        })
}

fn get_psp_token_from_raw_response(
    raw_connector_response: &Option<Secret<String>>,
) -> CustomResult<Secret<String>, IntegrationError> {
    raw_connector_response
        .as_ref()
        .map(|s| s.peek().clone())
        .and_then(|s| serde_json::from_str::<GigadatPayoutMeta>(&s).ok())
        .map(|meta| meta.token)
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "psp_token (from raw_connector_response)",
                context: Default::default(),
            }
            .into()
        })
}

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

macros::create_all_prerequisites!(
    connector_name: GigadatPayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutStage,
            request_body: GigadatPayoutStageRequest,
            response_body: GigadatPayoutStageResponse,
            router_data: RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
        ),
        (
            flow: PayoutGet,
            response_body: GigadatPayoutSyncResponse,
            router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ),
        (
            flow: PayoutTransfer,
            response_body: GigadatPayoutTransferResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ),
        (
            flow: PayoutCreate,
            response_body: GigadatPayoutCreateResponse,
            router_data: RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PayoutFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PayoutFlowData, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payouts<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PayoutFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.gigadat.base_url
        }
    }
);

// ===== PAYOUT SERVICE TRAIT =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait
    for GigadatPayouts<T>
{
}

// ===== PAYOUT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutStageV2
    for GigadatPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutGetV2
    for GigadatPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2
    for GigadatPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutCreateV2
    for GigadatPayouts<T>
{
}

// ===== SERVER AUTHENTICATION (not implemented) =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ServerAuthentication
    for GigadatPayouts<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for GigadatPayouts<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "server_authentication_token",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}

// ===== PAYOUT STUB FLOWS =====
macros::macro_connector_payout_implementation!(
    connector: GigadatPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutVoid,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount
    ]
);

// ===== PAYOUT STAGE FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GigadatPayouts,
    curl_request: Json(GigadatPayoutStageRequest),
    curl_response: GigadatPayoutStageResponse,
    flow_name: PayoutStage,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutStageRequest,
    flow_response: PayoutStageResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let auth = gigadat::GigadatAuthType::try_from(&req.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;
            Ok(format!(
                "{}api/payment-token/{}",
                self.connector_base_url_payouts(req),
                auth.campaign_id.peek()
            ))
        }
    }
);

// ===== PAYOUT GET (SYNC) FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GigadatPayouts,
    curl_response: GigadatPayoutSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let transfer_id = get_connector_payout_id(&req.request.connector_payout_id)?;
            Ok(format!(
                "{}api/transactions/{}",
                self.connector_base_url_payouts(req),
                transfer_id
            ))
        }
    }
);

// ===== PAYOUT TRANSFER FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GigadatPayouts,
    curl_response: GigadatPayoutTransferResponse,
    flow_name: PayoutTransfer,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutTransferRequest,
    flow_response: PayoutTransferResponse,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let transfer_id = get_connector_payout_id(&req.request.connector_payout_id)?;

            let token = get_psp_token_from_payout_method_data(&req.request.payout_method_data)
                .or_else(|_| {
                    get_psp_token_from_raw_response(
                        &req.resource_common_data.raw_connector_response,
                    )
                })?;

            Ok(format!(
                "{}webflow/deposit?transaction={}&token={}",
                self.connector_base_url_payouts(req),
                transfer_id,
                token.peek()
            ))
        }
    }
);

// ===== PAYOUT CREATE FLOW =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GigadatPayouts,
    curl_response: GigadatPayoutCreateResponse,
    flow_name: PayoutCreate,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutCreateRequest,
    flow_response: PayoutCreateResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let transfer_id = get_connector_payout_or_quote_id(
                &req.request.connector_payout_id,
                &req.request.connector_quote_id,
            )?;

            let token = get_psp_token_from_payout_method_data(&req.request.payout_method_data)
                .or_else(|_| {
                    get_psp_token_from_raw_response(
                        &req.resource_common_data.raw_connector_response,
                    )
                })?;

            Ok(format!(
                "{}webflow?transaction={}&token={}",
                self.connector_base_url_payouts(req),
                transfer_id,
                token.peek()
            ))
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for GigadatPayouts<T>
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
        // Try to parse as JSON first, fall back to plain text if that fails
        let error_message = res
            .response
            .parse_struct::<gigadat::GigadatErrorResponse>("GigadatErrorResponse")
            .map(|parsed| parsed.err)
            .unwrap_or_else(|_| {
                // Fall back to treating response as plain text
                String::from_utf8_lossy(&res.response).to_string()
            });

        let response = gigadat::GigadatErrorResponse {
            err: error_message.clone(),
        };

        with_error_response_body!(event_builder, response);

        // Check for specific Gigadat error message
        let is_duplicate_error =
            error_message.eq_ignore_ascii_case("Transaction already in progress or completed");

        // Transaction exists and is either in progress or completed; the caller
        // should initiate a sync to get the actual status.
        let code = if is_duplicate_error {
            "ALREADY_EXISTS".to_string()
        } else {
            error_message.clone()
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: error_message.clone(),
            reason: Some(error_message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}
