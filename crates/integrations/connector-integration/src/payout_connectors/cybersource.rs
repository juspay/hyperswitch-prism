pub mod transformers;

use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine as _};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::BytesExt,
    request::RequestContent,
    types::{AmountConvertor, StringMajorUnitForConnector},
    Method,
};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEligibility,
        PayoutEnrollDisburseAccount, PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid,
        ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse,
        PayoutEligibilityRequest, PayoutEligibilityResponse, PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse, PayoutFlowData, PayoutGetRequest, PayoutGetResponse,
        PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest, PayoutTransferResponse,
        PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEligibilityV2,
        PayoutEnrollDisburseAccountV2, PayoutGetV2, PayoutServiceTrait, PayoutStageV2,
        PayoutTransferV2, PayoutVoidV2, ServerAuthentication,
    },
};
use ring::{digest, hmac};
use time::OffsetDateTime;

use crate::{
    connectors::cybersource::transformers as cs_payments, set_typed_response,
    types::ResponseRouterData, with_error_response_body,
};
use transformers::{CybersourceAuthType, CybersourceFulfillResponse, CybersourcePayoutContext};

mod headers {
    pub(super) const CONTENT_TYPE: &str = "Content-Type";
    pub(super) const ACCEPT: &str = "Accept";
    pub(super) const CONNECTOR_UNAUTHORIZED_ERROR: &str = "Authentication Error from the connector";
}

pub struct CybersourcePayouts;

impl CybersourcePayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn generate_digest(payload: &[u8]) -> String {
        let payload_digest = digest::digest(&digest::SHA256, payload);
        BASE64_ENGINE.encode(payload_digest)
    }

    fn generate_signature(
        auth: &CybersourceAuthType,
        host: String,
        resource: &str,
        payload: &str,
        date: OffsetDateTime,
        http_method: Method,
    ) -> CustomResult<String, IntegrationError> {
        let is_post_method = matches!(http_method, Method::Post);
        let is_patch_method = matches!(http_method, Method::Patch);
        let is_delete_method = matches!(http_method, Method::Delete);
        let digest_str = if is_post_method || is_patch_method {
            "digest "
        } else {
            ""
        };
        let headers = format!("host date (request-target) {digest_str}v-c-merchant-id");
        let request_target = if is_post_method {
            format!("(request-target): post {resource}\ndigest: SHA-256={payload}\n")
        } else if is_patch_method {
            format!("(request-target): patch {resource}\ndigest: SHA-256={payload}\n")
        } else if is_delete_method {
            format!("(request-target): delete {resource}\n")
        } else {
            format!("(request-target): get {resource}\n")
        };
        let signature_string = format!(
            "host: {host}\ndate: {date}\n{request_target}v-c-merchant-id: {}",
            auth.merchant_account.peek()
        );
        let key_value = BASE64_ENGINE
            .decode(auth.api_secret.clone().expose())
            .change_context(IntegrationError::InvalidConnectorConfig {
                config: "connector_account_details.api_secret",
                context: Default::default(),
            })?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, &key_value);
        let signature_value =
            BASE64_ENGINE.encode(hmac::sign(&key, signature_string.as_bytes()).as_ref());
        Ok(format!(
            r#"keyid="{}", algorithm="HmacSHA256", headers="{headers}", signature="{signature_value}""#,
            auth.api_key.peek()
        ))
    }

    fn build_signed_headers(
        &self,
        connector_config: &ConnectorSpecificConfig,
        connectors: &Connectors,
        url: &str,
        http_method: Method,
        body: Option<&RequestContent>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let date = OffsetDateTime::now_utc();
        let auth = CybersourceAuthType::try_from(connector_config)?;
        let base_url = self.base_url(connectors);
        let cybersource_host =
            url::Url::parse(base_url).change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let host = cybersource_host
            .host_str()
            .ok_or(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let path: String = url.chars().skip(base_url.len() - 1).collect();
        let sha256 = Self::generate_digest(
            body.map(|req| req.get_inner_value().expose())
                .unwrap_or_default()
                .as_bytes(),
        );
        let signature = Self::generate_signature(
            &auth,
            host.to_string(),
            path.as_str(),
            &sha256,
            date,
            http_method,
        )?;

        let mut headers = vec![
            (
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (
                headers::ACCEPT.to_string(),
                "application/hal+json;charset=utf-8".to_string().into(),
            ),
            (
                "v-c-merchant-id".to_string(),
                auth.merchant_account.clone().into_masked(),
            ),
            ("Date".to_string(), date.to_string().into()),
            ("Host".to_string(), host.to_string().into()),
            ("Signature".to_string(), signature.into_masked()),
        ];
        if matches!(http_method, Method::Post | Method::Put | Method::Patch) {
            headers.push((
                "Digest".to_string(),
                format!("SHA-256={sha256}").into_masked(),
            ));
        }
        Ok(headers)
    }
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for CybersourcePayouts {
    fn id(&self) -> &'static str {
        "cybersource"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json;charset=utf-8"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.cybersource.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: Result<
            cs_payments::CybersourceErrorResponse,
            Report<common_utils::errors::ParsingError>,
        > = res.response.parse_struct("Cybersource ErrorResponse");

        let error_message = if res.status_code == 401 {
            headers::CONNECTOR_UNAUTHORIZED_ERROR.to_string()
        } else {
            NO_ERROR_MESSAGE.to_string()
        };
        match response {
            Ok(cs_payments::CybersourceErrorResponse::StandardError(response)) => {
                with_error_response_body!(event_builder, response);
                let typed = crate::connectors::macros::serialize_typed_connector_payload(
                    &response,
                    "typed_connector_response",
                );
                let (code, message, reason) = match response.error_information {
                    Some(ref error_info) => {
                        let detailed_error_info = error_info.details.as_ref().map(|details| {
                            details
                                .iter()
                                .map(|det| format!("{} : {}", det.field, det.reason))
                                .collect::<Vec<_>>()
                                .join(", ")
                        });
                        (
                            error_info.reason.clone(),
                            error_info.message.clone(),
                            cs_payments::get_error_reason(
                                Some(error_info.message.clone()),
                                detailed_error_info,
                                None,
                            ),
                        )
                    }
                    None => {
                        let detailed_error_info = response.details.map(|details| {
                            details
                                .iter()
                                .map(|det| format!("{} : {}", det.field, det.reason))
                                .collect::<Vec<_>>()
                                .join(", ")
                        });
                        (
                            response
                                .reason
                                .clone()
                                .map_or(NO_ERROR_CODE.to_string(), |reason| reason.to_string()),
                            response
                                .message
                                .clone()
                                .map_or(error_message.to_string(), |msg| msg.to_string()),
                            cs_payments::get_error_reason(
                                response.message,
                                detailed_error_info,
                                None,
                            ),
                        )
                    }
                };
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code,
                    message,
                    reason,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Ok(cs_payments::CybersourceErrorResponse::AuthenticationError(response)) => {
                with_error_response_body!(event_builder, response);
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: NO_ERROR_CODE.to_string(),
                    message: response.response.rmsg.clone(),
                    reason: Some(response.response.rmsg),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Ok(cs_payments::CybersourceErrorResponse::NotAvailableError(response)) => {
                with_error_response_body!(event_builder, response);
                let error_response = response
                    .errors
                    .iter()
                    .map(|error_info| {
                        format!(
                            "{}: {}",
                            error_info.error_type.clone().unwrap_or("".to_string()),
                            error_info.message.clone().unwrap_or("".to_string())
                        )
                    })
                    .collect::<Vec<String>>()
                    .join(" & ");
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: NO_ERROR_CODE.to_string(),
                    message: error_response.clone(),
                    reason: Some(error_response),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Err(error_msg) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}))
                };
                tracing::error!(deserialization_error =? error_msg);
                crate::utils::handle_json_response_deserialization_failure(res, "cybersource")
            }
        }
    }
}

fn build_5xx_error_response(
    res: Response,
    event_builder: Option<&mut events::Event>,
) -> CustomResult<ErrorResponse, ConnectorError> {
    let parsed: Result<
        cs_payments::CybersourceServerErrorResponse,
        Report<common_utils::errors::ParsingError>,
    > = res.response.parse_struct("CybersourceServerErrorResponse");
    if let Ok(response) = parsed {
        with_error_response_body!(event_builder, response);
        let code = response
            .reason
            .as_ref()
            .map(|r| format!("{:?}", r).to_uppercase())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        let message = response
            .message
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
        return Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        });
    }
    let error_message = match res.status_code {
        500 => "internal_server_error",
        501 => "not_implemented",
        502 => "bad_gateway",
        503 => "service_unavailable",
        504 => "gateway_timeout",
        _ => "unknown_error",
    };
    Ok(ErrorResponse {
        status_code: res.status_code,
        code: res.status_code.to_string(),
        message: error_message.to_string(),
        reason: String::from_utf8(res.response.to_vec()).ok(),
        attempt_status: None,
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    })
}

// ===== SERVER AUTHENTICATION (not implemented) =====

impl ServerAuthentication for CybersourcePayouts {}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for CybersourcePayouts
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

impl PayoutServiceTrait for CybersourcePayouts {}

// ===== PAYOUT TRANSFER (REAL) =====

impl PayoutTransferV2 for CybersourcePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for CybersourcePayouts
{
    fn get_http_method(&self) -> Method {
        Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}pts/v2/payouts",
            req.resource_common_data.connectors.cybersource.base_url
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let converter = StringMajorUnitForConnector;
        let total_amount = converter
            .convert(req.request.amount, req.request.destination_currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let connector_req = transformers::CybersourcePayoutFulfillRequest::try_from((
            req,
            CybersourcePayoutContext { total_amount },
        ))?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let url = <Self as ConnectorIntegrationV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >>::get_url(self, req)?;
        let body = <Self as ConnectorIntegrationV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >>::get_request_body(self, req)?;
        self.build_signed_headers(
            &req.connector_config,
            &req.resource_common_data.connectors,
            &url,
            Method::Post,
            body.as_ref(),
        )
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
        ConnectorError,
    > {
        let response: CybersourceFulfillResponse = res
            .response
            .parse_struct("CybersourceFulfillResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;
        set_typed_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        if res.status_code >= 500 {
            build_5xx_error_response(res, event_builder)
        } else {
            self.build_error_response(res, event_builder, connector_config)
        }
    }
}

// ===== PAYOUT STUB FLOWS =====

impl PayoutCreateV2 for CybersourcePayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create",
            Default::default(),
        )
        .into())
    }
}

impl PayoutGetV2 for CybersourcePayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_get",
            Default::default(),
        )
        .into())
    }
}

impl PayoutVoidV2 for CybersourcePayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_void",
            Default::default(),
        )
        .into())
    }
}

impl PayoutStageV2 for CybersourcePayouts {}

impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_stage",
            Default::default(),
        )
        .into())
    }
}

impl PayoutCreateLinkV2 for CybersourcePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateLink,
            PayoutFlowData,
            PayoutCreateLinkRequest,
            PayoutCreateLinkResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create_link",
            Default::default(),
        )
        .into())
    }
}

impl PayoutCreateRecipientV2 for CybersourcePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create_recipient",
            Default::default(),
        )
        .into())
    }
}

impl PayoutEnrollDisburseAccountV2 for CybersourcePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_enroll_disburse_account",
            Default::default(),
        )
        .into())
    }
}

impl PayoutEligibilityV2 for CybersourcePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    > for CybersourcePayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_eligibility",
            Default::default(),
        )
        .into())
    }
}
