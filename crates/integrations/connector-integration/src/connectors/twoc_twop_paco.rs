pub mod transformers;

use std::{self, fmt::Debug};

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void, VoidPC},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as twoc_twop_paco, TwocTwopPacoAuthType, TwocTwopPacoAuthorizeRequest,
    TwocTwopPacoAuthorizeResponse, TwocTwopPacoCaptureRequest, TwocTwopPacoCaptureResponse,
    TwocTwopPacoErrorResponse, TwocTwopPacoNonUiResponse, TwocTwopPacoPSyncInquiryResponse,
    TwocTwopPacoRSyncInquiryResponse, TwocTwopPacoRefundRequest, TwocTwopPacoRefundResponse,
    TwocTwopPacoVoidPcRequest, TwocTwopPacoVoidPcResponse, TwocTwopPacoVoidRequest,
    TwocTwopPacoVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const TOKEN: &str = "token";
    pub(crate) const APIKEY: &str = "apikey";
}

const CONTENT_TYPE_JOSE: &str = "application/jose";
const CONTENT_TYPE_JSON: &str = "application/json";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for TwocTwopPaco<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for TwocTwopPaco<T>
{
}
macros::macro_connector_payout_implementation!(
    connector: TwocTwopPaco,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

macros::macro_connector_flow_status_impls!(
    connector: TwocTwopPaco,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SetupMandate,
        RepeatPayment,
        IncrementalAuthorization,
        MandateRevoke,
    ],
    not_supported: [
        CreateOrder,
        SubmitEvidence,
        DefendDispute,
        Accept,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
        PaymentMethodToken,
        ServerAuthenticationToken,
        ClientAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
    ],
);

macros::create_all_prerequisites!(
    connector_name: TwocTwopPaco,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: TwocTwopPacoAuthorizeRequest,
            response_body: TwocTwopPacoAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: TwocTwopPacoPSyncInquiryResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: TwocTwopPacoCaptureRequest,
            response_body: TwocTwopPacoCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: TwocTwopPacoVoidRequest,
            response_body: TwocTwopPacoVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: TwocTwopPacoVoidPcRequest,
            response_body: TwocTwopPacoVoidPcResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TwocTwopPacoRefundRequest,
            response_body: TwocTwopPacoRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: TwocTwopPacoRSyncInquiryResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_jose_headers(
            &self,
            auth: &TwocTwopPacoAuthType,
        ) -> Vec<(String, Maskable<String>)> {
            vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    CONTENT_TYPE_JOSE.to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    CONTENT_TYPE_JOSE.to_string().into(),
                ),
                (
                    headers::TOKEN.to_string(),
                    auth.access_token.clone().expose().into(),
                ),
            ]
        }

        pub fn build_inquiry_headers(
            &self,
            auth: &TwocTwopPacoAuthType,
        ) -> Vec<(String, Maskable<String>)> {
            vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    CONTENT_TYPE_JSON.to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    CONTENT_TYPE_JSON.to_string().into(),
                ),
                (
                    headers::APIKEY.to_string(),
                    auth.access_token.clone().expose().into(),
                ),
            ]
        }

        pub fn connector_base_url_payments<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data
                .connectors
                .twoc_twop_paco
                .base_url
                .to_string()
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.twoc_twop_paco.base_url
        }

        pub fn preprocess_request_bytes<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: Vec<u8>,
        ) -> CustomResult<Vec<u8>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            let inner: serde_json::Value = serde_json::from_slice(&bytes).map_err(|err| {
                error_stack::report!(errors::IntegrationError::InvalidDataFormat {
                    field_name: "twoc_twop_paco request body",
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(format!(
                            "Inner PACO body was not valid JSON before JOSE wrap: {err}"
                        )),
                    },
                })
            })?;
            let claims = transformers::PacoJoseClaims::new(auth.access_token.peek(), inner);
            let jwe = common_utils::crypto::jose::sign_then_encrypt(&claims, &auth.jose_cfg)
                .map_err(|err| {
                    error_stack::report!(errors::IntegrationError::FailedToObtainAuthType {
                        context: errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "Confirm the PACO PEMs and `kid` resolve a valid JOSE pair."
                                    .to_string(),
                            ),
                            doc_url: Some(
                                "https://developer.2c2p.com/docs/getting-started-with-payment-air-controller-paco"
                                    .to_string(),
                            ),
                            additional_context: Some(format!(
                                "JOSE sign+encrypt failed: {err}"
                            )),
                        },
                    })
                })?;
            Ok(jwe.into_bytes())
        }

        pub fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            status_code: u16,
        ) -> CustomResult<bytes::Bytes, errors::ConnectorError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config).change_context(
                errors::ConnectorError::response_deserialization_failed_with_context(
                    status_code,
                    Some(
                        "twoc_twop_paco: failed to read auth config for response decoding"
                            .to_string(),
                    ),
                ),
            )?;
            let compact = std::str::from_utf8(&bytes).map_err(|_| {
                error_stack::report!(
                    errors::ConnectorError::response_deserialization_failed_with_context(
                        status_code,
                        Some("twoc_twop_paco: response was not valid UTF-8".to_string()),
                    )
                )
            })?;
            let trimmed = compact.trim().trim_matches('"');
            let validation = common_utils::crypto::jose::JoseClaimValidation {
                expected_audience: Some(auth.response_audience.peek().clone()),
                clock_skew_seconds: 60,
            };
            let claims = common_utils::crypto::jose::decrypt_then_verify_with_claims(
                trimmed,
                &auth.jose_cfg,
                Some(&validation),
            )
            .map_err(|err| {
                error_stack::report!(
                    errors::ConnectorError::response_deserialization_failed_with_context(
                        status_code,
                        Some(format!("JOSE decrypt+verify failed: {err}")),
                    )
                )
            })?;
            let inner = match claims.get("response") {
                Some(value) => value.clone(),
                None => claims,
            };
            let inner_bytes = serde_json::to_vec(&inner).map_err(|err| {
                error_stack::report!(
                    errors::ConnectorError::response_deserialization_failed_with_context(
                        status_code,
                        Some(format!(
                            "twoc_twop_paco: failed to re-serialise inner response: {err}"
                        )),
                    )
                )
            })?;
            Ok(bytes::Bytes::from(inner_bytes))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for TwocTwopPaco<T>
{
    fn id(&self) -> &'static str {
        "twoc_twop_paco"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_JOSE
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.twoc_twop_paco.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = TwocTwopPacoAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::TOKEN.to_string(),
            auth.access_token.expose().into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let body = res.response.to_vec();
        let body_str = String::from_utf8_lossy(&body).to_string();
        let trimmed = body_str.trim().trim_matches('"');
        let looks_like_jose = trimmed.split('.').count() == 5
            && trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');

        let body_prefix_hex = || {
            let take = body.len().min(128);
            let hex: String = body
                .iter()
                .take(take)
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join("");
            if body.len() > take {
                format!("body[0..{take}]=0x{hex}…")
            } else {
                format!("body=0x{hex}")
            }
        };
        let fallback = |context: &str| {
            (
                NO_ERROR_CODE.to_string(),
                format!("{NO_ERROR_MESSAGE} ({context}; {})", body_prefix_hex()),
            )
        };

        let (code, message) = if looks_like_jose {
            match TwocTwopPacoAuthType::try_from(connector_config) {
                Ok(auth) => {
                    match common_utils::crypto::jose::decrypt_then_verify(trimmed, &auth.jose_cfg) {
                        Ok(value) => {
                            let inner = value
                                .get("response")
                                .cloned()
                                .unwrap_or_else(|| value.clone());
                            match serde_json::from_value::<TwocTwopPacoNonUiResponse>(inner) {
                                Ok(parsed) => {
                                    let prior = parsed
                                        .merged_result()
                                        .and_then(|b| b.prior_payment_response_details.clone());
                                    let api = parsed.api_response.clone();
                                    with_error_response_body!(event_builder, parsed);
                                    twoc_twop_paco::error_code_message(&api, &prior)
                                }
                                Err(err) => {
                                    tracing::warn!(
                                        error = %err,
                                        "twoc_twop_paco: failed to parse decrypted error envelope"
                                    );
                                    fallback("envelope parse failed")
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                error = %err,
                                "twoc_twop_paco: JOSE decrypt failed for error response"
                            );
                            fallback("JOSE decrypt failed")
                        }
                    }
                }
                Err(_) => fallback("connector auth config missing"),
            }
        } else {
            match serde_json::from_slice::<TwocTwopPacoErrorResponse>(&body) {
                Ok(parsed) => {
                    with_error_response_body!(event_builder, parsed);
                    parsed.flatten()
                }
                Err(_) => fallback("response is neither JOSE nor recognised JSON"),
            }
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_response: TwocTwopPacoPSyncInquiryResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            Ok(self.build_inquiry_headers(&auth))
        }

        fn get_content_type(&self) -> &'static str {
            CONTENT_TYPE_JSON
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let order_no = req
                .resource_common_data
                .connector_request_reference_id
                .clone();
            Ok(format!(
                "{base_url}/api/2.0/Inquiry/transactionStatus?orderNo={}",
                urlencoding::encode(&order_no),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_response: TwocTwopPacoRSyncInquiryResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            Ok(self.build_inquiry_headers(&auth))
        }

        fn get_content_type(&self) -> &'static str {
            CONTENT_TYPE_JSON
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            let order_no = req.request.connector_refund_id.clone();
            Ok(format!(
                "{base_url}/api/2.0/Inquiry/transactionStatus?orderNo={}",
                urlencoding::encode(&order_no),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_request: Json(TwocTwopPacoAuthorizeRequest),
    curl_response: TwocTwopPacoAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_request: true,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            Ok(self.build_jose_headers(&auth))
        }

        fn get_content_type(&self) -> &'static str {
            CONTENT_TYPE_JOSE
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/api/2.0/Payment/nonUi"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_request: Json(TwocTwopPacoCaptureRequest),
    curl_response: TwocTwopPacoCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Put,
    preprocess_request: true,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            Ok(self.build_jose_headers(&auth))
        }

        fn get_content_type(&self) -> &'static str {
            CONTENT_TYPE_JOSE
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/api/2.0/Settlement"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_request: Json(TwocTwopPacoVoidRequest),
    curl_response: TwocTwopPacoVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_request: true,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            Ok(self.build_jose_headers(&auth))
        }

        fn get_content_type(&self) -> &'static str {
            CONTENT_TYPE_JOSE
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/api/2.0/Void"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_request: Json(TwocTwopPacoVoidPcRequest),
    curl_response: TwocTwopPacoVoidPcResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_request: true,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            Ok(self.build_jose_headers(&auth))
        }

        fn get_content_type(&self) -> &'static str {
            CONTENT_TYPE_JOSE
        }

        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/api/2.0/Void"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_request: Json(TwocTwopPacoRefundRequest),
    curl_response: TwocTwopPacoRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_request: true,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth = TwocTwopPacoAuthType::try_from(&req.connector_config)?;
            Ok(self.build_jose_headers(&auth))
        }

        fn get_content_type(&self) -> &'static str {
            CONTENT_TYPE_JOSE
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/api/2.0/Refund/refund"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for TwocTwopPaco<T>
{
}
