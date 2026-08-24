pub mod transformers;

use std::fmt::Debug;

use crate::connectors::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{BASE64_ENGINE, NO_ERROR_CODE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    FloatMajorUnit,
};
use domain_types::{
    connector_flow::{PayoutCreate, PayoutGet, PayoutStage, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
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
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateV2, PayoutGetV2, PayoutServiceTrait, PayoutStageV2, PayoutTransferV2,
    },
};
use serde::Serialize;
use transformers::{
    self as gigadat, GigadatPayoutCreateResponse, GigadatPayoutGetResponse, GigadatPayoutMeta,
    GigadatPayoutStageRequest, GigadatPayoutStageResponse, GigadatPayoutTransferResponse,
};

// ===== PAYOUT UTILITY FUNCTIONS =====
fn get_connector_payout_id(
    connector_payout_id: &Option<String>,
) -> CustomResult<String, IntegrationError> {
    connector_payout_id.as_ref().cloned().ok_or_else(|| {
        IntegrationError::MissingRequiredField {
            field_name: "connector_payout_id",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Gigadat payout sync needs the transaction id returned by the transfer call"
                        .to_string(),
                ),
                suggested_action: Some(
                    "Run PayoutTransfer first so the connector payout id is available".to_string(),
                ),
                doc_url: None,
            },
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
                context: IntegrationErrorContext {
                additional_context: Some(
                    "Gigadat needs either the transaction id or the staged quote id to build the url".to_string(),
                ),
                suggested_action: Some(
                    "Run PayoutStage or PayoutTransfer first".to_string(),
                ),
                doc_url: None,
            },
            }
            .into()
        })
}

fn get_psp_token_from_payout_method_data(
    payout_method_data: &Option<domain_types::payouts::payout_method_data::PayoutMethodData>,
) -> CustomResult<Secret<String>, IntegrationError> {
    payout_method_data
        .as_ref()
        .and_then(|pmd| match pmd {
            domain_types::payouts::payout_method_data::PayoutMethodData::Passthrough(pt) => {
                Some(pt.psp_token.clone())
            }
            _ => None,
        })
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "psp_token (from payout_method_data.passthrough)",
                context: IntegrationErrorContext {
                additional_context: Some(
                    "Gigadat payouts are completed with the passthrough psp_token issued at stage time".to_string(),
                ),
                suggested_action: Some(
                    "Send the payout method as passthrough carrying the staged psp_token".to_string(),
                ),
                doc_url: None,
            },
            }
            .into()
        })
}

fn get_psp_token_from_payout_metadata(
    payout_connector_metadata: &Option<Secret<String>>,
) -> CustomResult<Secret<String>, IntegrationError> {
    match payout_connector_metadata {
        Some(raw) => serde_json::from_str::<GigadatPayoutMeta>(raw.peek())
            .map(|meta| meta.token)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "payout_connector_metadata (expected staged-payout token json)",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "The staged payout response body did not contain the expected token json"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Check that the preceding PayoutStage call succeeded".to_string(),
                    ),
                    doc_url: None,
                },
            }),
        None => Err(IntegrationError::MissingRequiredField {
            field_name: "psp_token (from payout_connector_metadata)",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "No staged payout response was carried over, so the token cannot be recovered"
                        .to_string(),
                ),
                suggested_action: Some("Run PayoutStage before PayoutTransfer".to_string()),
                doc_url: None,
            },
        }
        .into()),
    }
}

fn mask_webflow_token(url: &str) -> String {
    url.split_once("&token=")
        .map(|(base, _token)| format!("{base}&token=***"))
        .unwrap_or_else(|| url.to_owned())
}

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

const MAX_ERROR_BODY_LENGTH: usize = 1024;

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
            response_body: GigadatPayoutGetResponse,
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
macros::macro_connector_flow_status_impls!(
    connector: GigadatPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ServerAuthenticationToken]
);

// ===== PAYOUT STUB FLOWS =====
macros::macro_connector_payout_implementation!(
    connector: GigadatPayouts,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [
        PayoutVoid,
        PayoutCreateLink,
        PayoutCreateRecipient,
        PayoutEnrollDisburseAccount,
        PayoutEligibility
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
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Gigadat payouts requires a Gigadat connector auth type"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Configure the merchant connector account with Gigadat credentials"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
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
    curl_response: GigadatPayoutGetResponse,
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
                    get_psp_token_from_payout_metadata(
                        &req.resource_common_data.payout_connector_metadata,
                    )
                })?;

            Ok(format!(
                "{}webflow/deposit?transaction={}&token={}",
                self.connector_base_url_payouts(req),
                transfer_id,
                token.peek()
            ))
        }
        fn get_masked_url(&self, url: &str) -> String {
            mask_webflow_token(url)
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
                    get_psp_token_from_payout_metadata(
                        &req.resource_common_data.payout_connector_metadata,
                    )
                })?;

            Ok(format!(
                "{}webflow?transaction={}&token={}",
                self.connector_base_url_payouts(req),
                transfer_id,
                token.peek()
            ))
        }
        fn get_masked_url(&self, url: &str) -> String {
            mask_webflow_token(url)
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
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Gigadat payouts requires a Gigadat connector auth type".to_string(),
                    ),
                    suggested_action: Some(
                        "Configure the merchant connector account with Gigadat credentials"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            },
        )?;

        let auth_key = format!(
            "{}:{}",
            auth.access_token.peek(),
            auth.security_token.peek()
        );
        let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth_header.into_masked(),
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
                // Fall back to treating response as bounded plain text
                String::from_utf8_lossy(&res.response)
                    .chars()
                    .take(MAX_ERROR_BODY_LENGTH)
                    .collect()
            });

        let response = gigadat::GigadatErrorResponse {
            err: error_message.clone(),
        };

        with_error_response_body!(event_builder, response);

        let code = match error_message
            .eq_ignore_ascii_case("Transaction already in progress or completed")
        {
            true => "ALREADY_EXISTS".to_string(),
            false => NO_ERROR_CODE.to_string(),
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
            typed_connector_response: macros::serialize_typed_connector_payload(
                &response,
                "typed_connector_response",
            ),
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}
