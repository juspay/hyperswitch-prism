pub mod transformers;

use std::{fmt::Debug, sync::LazyLock};

use base64::Engine;
use common_enums::{enums, CurrencyUnit, PaymentMethodType};
use common_utils::{
    consts::{BASE64_ENGINE_URL_SAFE_NO_PAD, NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{Method, Request, RequestBuilder, RequestContent},
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, PSync, RSync, Refund, ServerSessionAuthenticationToken,
    },
    connector_types::{ConnectorSpecifications, SupportedPaymentMethodsExt},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RedirectDetailsResponse, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RequestDetails, ResponseId,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{self, IntegrationError, IntegrationErrorContext, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        self, ConnectorInfo, Connectors, FeatureStatus, PaymentMethodDetails,
        SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
};
use serde::Serialize;
use transformers::{
    self as grabpay, GrabpayAuthenticateRequest, GrabpayAuthenticateResponse,
    GrabpayAuthorizeRequest, GrabpayAuthorizeResponse, GrabpayChargeCompleteResponse,
    GrabpayRefundRequest, GrabpayRefundResponse, GrabpayRefundSyncResponse,
    GrabpayServerSessionAuthenticationTokenRequest,
    GrabpayServerSessionAuthenticationTokenResponse, GrabpayWebhookBody,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_error_response_body};

const CONTENT_TYPE: &str = "application/json";
const CHARGE_INIT_PATH: &str = "/charge/init";
const CHARGE_COMPLETE_PATH: &str = "/charge/complete";
const CHARGE_STATUS_PREFIX: &str = "/charge";
const REFUND_PATH: &str = "/refund";
const OAUTH_TOKEN_PATH: &str = "/grabid/v1/oauth2/token";
pub(crate) const GRABPAY_DOC_URL: &str =
    "https://developers.grab.com/docs/grabpay-online-integration";
pub(crate) const GRABPAY_CONFIG_SUGGESTED_ACTION: &str =
    "Verify the GrabPay connector configuration (partner_id, partner_secret, client_id, \
     client_secret, merchant_id, base_url) and the request payload, then retry.";

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const DATE: &str = "Date";
}

fn grabpay_integration_context(additional_context: impl Into<String>) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(GRABPAY_CONFIG_SUGGESTED_ACTION.to_string()),
        doc_url: Some(GRABPAY_DOC_URL.to_string()),
        additional_context: Some(additional_context.into()),
    }
}

macros::create_all_prerequisites!(
    connector_name: Grabpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GrabpayAuthorizeRequest<T>,
            response_body: GrabpayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: GrabpayChargeCompleteResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GrabpayRefundRequest,
            response_body: GrabpayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: GrabpayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ServerSessionAuthenticationToken,
            request_body: GrabpayServerSessionAuthenticationTokenRequest,
            response_body: GrabpayServerSessionAuthenticationTokenResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: GrabpayAuthenticateRequest,
            response_body: GrabpayAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_json_headers(
            &self,
        ) -> Vec<(String, Maskable<String>)> {
            vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )]
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.grabpay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.grabpay.base_url
        }

        pub fn build_hmac_headers(
            &self,
            auth: &grabpay::GrabpayAuthType,
            method: &str,
            path: &str,
            body: &[u8],
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let date = format_rfc7231_date(time::OffsetDateTime::now_utc())?;
            let authorization =
                build_hmac_authorization(auth, method, CONTENT_TYPE, path, body, &date)?;

            let mut headers = self.build_json_headers();
            headers.push((headers::DATE.to_string(), date.into()));
            headers.push((headers::AUTHORIZATION.to_string(), authorization.into_masked()));
            Ok(headers)
        }

        pub fn build_pop_headers(
            &self,
            auth: &grabpay::GrabpayAuthType,
            access_token: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let now = time::OffsetDateTime::now_utc();
            let date = format_rfc7231_date(now)?;
            let timestamp = now.unix_timestamp().to_string();
            let pop = build_pop_signature(auth, access_token, &timestamp)?;

            let mut headers = self.build_json_headers();
            headers.push((headers::DATE.to_string(), date.into()));
            headers.push((
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {access_token}").into_masked(),
            ));
            headers.push(("X-GID-AUX-POP".to_string(), pop.into_masked()));
            Ok(headers)
        }
    }
);

pub(crate) fn oauth_endpoint(base_url: &str, path: &str) -> String {
    url::Url::parse(base_url)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?;
            let port = url
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default();
            Some(format!("{}://{host}{port}{path}", url.scheme()))
        })
        .unwrap_or_else(|| format!("{base_url}{path}"))
}

/// Builds GrabPay's canonical signing string:
/// `{method}\n{content_type}\n{date}\n{path}\n{base64(sha256(body))}\n`.
fn grabpay_hmac_signing_string(
    method: &str,
    content_type: &str,
    path: &str,
    body: &[u8],
    date: &str,
) -> CustomResult<String, IntegrationError> {
    use common_utils::crypto::GenerateDigest;

    let body_digest = crypto::Sha256.generate_digest(body).change_context(
        IntegrationError::RequestEncodingFailed {
            context: grabpay_integration_context(
                "GrabPay HMAC signing failed to hash request body",
            ),
        },
    )?;
    let encoded_body_digest = BASE64_ENGINE.encode(body_digest);
    Ok(format!(
        "{method}\n{content_type}\n{date}\n{path}\n{encoded_body_digest}\n"
    ))
}

fn build_hmac_authorization(
    auth: &grabpay::GrabpayAuthType,
    method: &str,
    content_type: &str,
    path: &str,
    body: &[u8],
    date: &str,
) -> CustomResult<String, IntegrationError> {
    use common_utils::crypto::SignMessage;

    let signing_string = grabpay_hmac_signing_string(method, content_type, path, body, date)?;
    let signature = crypto::HmacSha256
        .sign_message(
            auth.partner_secret.peek().as_bytes(),
            signing_string.as_bytes(),
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: grabpay_integration_context(
                "GrabPay HMAC signing failed to sign canonical request",
            ),
        })?;

    Ok(format!(
        "{}:{}",
        auth.partner_id.peek(),
        BASE64_ENGINE.encode(signature)
    ))
}

fn get_webhook_header<'a>(
    headers: &'a std::collections::HashMap<String, String>,
    header_name: &'static str,
) -> Result<&'a str, error_stack::Report<WebhookError>> {
    headers
        .iter()
        .find_map(|(key, value)| {
            key.eq_ignore_ascii_case(header_name)
                .then_some(value.as_str())
        })
        .ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookMissingRequiredField { field: header_name })
        })
}

fn grabpay_webhook_path(uri: Option<&str>) -> Result<String, error_stack::Report<WebhookError>> {
    let uri = uri.ok_or_else(|| {
        error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "uri" })
    })?;

    if let Ok(url) = url::Url::parse(uri) {
        return Ok(url.path().to_string());
    }

    Ok(uri.split('?').next().unwrap_or(uri).to_string())
}

fn build_pop_signature(
    auth: &grabpay::GrabpayAuthType,
    access_token: &str,
    timestamp: &str,
) -> CustomResult<String, IntegrationError> {
    use common_utils::crypto::SignMessage;

    let signing_message = format!("{timestamp}{access_token}");
    let signature = crypto::HmacSha256
        .sign_message(
            auth.client_secret.peek().as_bytes(),
            signing_message.as_bytes(),
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: grabpay_integration_context("GrabPay PoP signing failed to sign token proof"),
        })?;
    let sig = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(signature);
    let time_since_epoch =
        timestamp
            .parse::<i64>()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: grabpay_integration_context(
                    "GrabPay PoP signing failed to parse Unix timestamp",
                ),
            })?;
    let payload = serde_json::json!({
        "time_since_epoch": time_since_epoch,
        "sig": sig,
    });
    let payload_json = serde_json::to_string(&payload).change_context(
        IntegrationError::RequestEncodingFailed {
            context: grabpay_integration_context(
                "GrabPay PoP signing failed to serialize token proof payload",
            ),
        },
    )?;

    Ok(BASE64_ENGINE_URL_SAFE_NO_PAD.encode(payload_json.as_bytes()))
}

/// Formats a timestamp as an HTTP-date (RFC 7231, e.g. `Wed, 02 Nov 2022 08:00:00 GMT`)
/// for GrabPay's `Date` header, using `time`'s format description instead of a manual match.
fn format_rfc7231_date(date_time: time::OffsetDateTime) -> CustomResult<String, IntegrationError> {
    let format = time::macros::format_description!(
        "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT"
    );
    date_time
        .format(&format)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: grabpay_integration_context(
                "GrabPay failed to format the RFC 7231 Date header",
            ),
        })
}

fn payment_access_token(data: &PaymentFlowData) -> CustomResult<String, IntegrationError> {
    data.get_access_token().or_else(|err| {
        session_token_from_connector_feature_data(data.connector_feature_data.as_ref()).ok_or(err)
    })
}

fn refund_access_token(data: &RefundFlowData) -> CustomResult<String, IntegrationError> {
    data.get_access_token().or_else(|err| {
        session_token_from_connector_feature_data(data.connector_feature_data.as_ref()).ok_or(err)
    })
}

fn session_token_from_connector_feature_data(
    connector_feature_data: Option<&common_utils::pii::SecretSerdeValue>,
) -> Option<String> {
    let metadata =
        utils::to_connector_meta_from_secret::<serde_json::Value>(connector_feature_data.cloned())
            .ok()?;

    metadata
        .get("session_token")
        .or_else(|| metadata.get("access_token"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Grabpay<T>
{
    fn id(&self) -> &'static str {
        "grabpay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.grabpay.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let _auth =
            grabpay::GrabpayAuthType::try_from(auth_type)
                .change_context(IntegrationError::FailedToObtainAuthType {
                context: grabpay_integration_context(
                    "GrabPay connector configuration was not supplied in ConnectorSpecificConfig",
                ),
            })?;

        Ok(Vec::new())
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        if res.response.is_empty() {
            return Ok(ErrorResponse {
                status_code: res.status_code,
                code: NO_ERROR_CODE.to_string(),
                message: NO_ERROR_MESSAGE.to_string(),
                reason: None,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            });
        }

        let response: grabpay::GrabpayErrorResponse = res
            .response
            .parse_struct("GrabpayErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "GrabPay error response did not match the expected schema".to_string(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);

        let message = response
            .message
            .or(response.error_description)
            .or(response.reason.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
        let code = response
            .code
            .or(response.error)
            .or(response.reason.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason: response.reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Grabpay,
    curl_request: Json(GrabpayAuthenticateRequest),
    curl_response: GrabpayAuthenticateResponse,
    flow_name: Authenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = grabpay::GrabpayAuthType::try_from(&req.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: grabpay_integration_context(
                        "GrabPay Authenticate requires GrabPay connector configuration",
                    ),
                },
            )?;
            let connector_request = GrabpayAuthenticateRequest::try_from(req.clone())?;
            let body = serde_json::to_vec(&connector_request).change_context(
                IntegrationError::RequestEncodingFailed {
                    context: grabpay_integration_context(
                        "GrabPay Authenticate failed to serialize HMAC request body",
                    ),
                },
            )?;
            let request_path = url::Url::parse(&self.get_url(req)?)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: grabpay_integration_context(
                        "GrabPay Authenticate failed to parse charge init URL for HMAC path",
                    ),
                })?
                .path()
                .to_string();

            self.build_hmac_headers(&auth, "POST", &request_path, &body)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}{CHARGE_INIT_PATH}"))
        }

    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Grabpay,
    curl_response: GrabpayRefundSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = grabpay::GrabpayAuthType::try_from(&req.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: grabpay_integration_context(
                        "GrabPay RSync requires GrabPay connector configuration",
                    ),
                },
            )?;
            let access_token = refund_access_token(&req.resource_common_data)?;

            self.build_pop_headers(&auth, &access_token)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let partner_tx_id = req.request.connector_refund_id.clone();
            grabpay::validate_partner_tx_id(&partner_tx_id)?;
            let currency = req
                .request
                .refund_money
                .as_ref()
                .map(|money| money.currency)
                .map(Ok)
                .unwrap_or_else(|| {
                    grabpay::currency_from_connector_feature_data(
                        req.resource_common_data.connector_feature_data.as_ref(),
                    )
                })?;

            Ok(format!(
                "{}{REFUND_PATH}/{}/status?currency={}",
                self.connector_base_url_refunds(req),
                partner_tx_id,
                currency,
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Grabpay,
    curl_response: GrabpayChargeCompleteResponse,
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
            let auth = grabpay::GrabpayAuthType::try_from(&req.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: grabpay_integration_context(
                        "GrabPay PSync requires GrabPay connector configuration",
                    ),
                },
            )?;
            let access_token = payment_access_token(&req.resource_common_data)?;

            self.build_pop_headers(&auth, &access_token)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let partner_tx_id = req
                .resource_common_data
                .connector_request_reference_id
                .clone();
            grabpay::validate_partner_tx_id(&partner_tx_id)?;
            Ok(format!(
                "{}{CHARGE_STATUS_PREFIX}/{}/status?currency={}",
                self.connector_base_url_payments(req),
                partner_tx_id,
                req.request.currency,
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Grabpay,
    curl_request: Json(GrabpayRefundRequest),
    curl_response: GrabpayRefundResponse,
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
            let auth = grabpay::GrabpayAuthType::try_from(&req.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: grabpay_integration_context(
                        "GrabPay Refund requires GrabPay connector configuration",
                    ),
                },
            )?;
            let access_token = refund_access_token(&req.resource_common_data)?;

            self.build_pop_headers(&auth, &access_token)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_refunds(req), REFUND_PATH))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    > for Grabpay<T>
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        match req.resource_common_data.get_session_token() {
            Ok(session_token) => {
                let auth = grabpay::GrabpayAuthType::try_from(&req.connector_config)
                    .change_context(IntegrationError::FailedToObtainAuthType {
                        context: grabpay_integration_context(
                            "GrabPay Authorize requires GrabPay connector configuration",
                        ),
                    })?;
                self.build_pop_headers(&auth, &session_token)
            }
            Err(_) => Ok(self.build_json_headers()),
        }
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        if req.resource_common_data.get_session_token().is_ok() {
            Ok(format!(
                "{}{CHARGE_COMPLETE_PATH}",
                self.connector_base_url_payments(req)
            ))
        } else {
            Err(error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: grabpay_integration_context(
                    "GrabPay post-redirect authorize requires a successful OAuth session-token exchange",
                ),
            }))
        }
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        if req.resource_common_data.get_session_token().is_ok() {
            let request = GrabpayAuthorizeRequest::try_from(req.clone())?;
            Ok(Some(RequestContent::Json(Box::new(request))))
        } else {
            Ok(None)
        }
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<Request>, IntegrationError> {
        Ok(Some(
            RequestBuilder::new()
                .method(Method::Post)
                .url(self.get_url(req)?.as_str())
                .attach_default_headers()
                .headers(self.get_headers(req)?)
                .set_optional_body(self.get_request_body(req)?)
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        errors::ConnectorError,
    > {
        let response: GrabpayAuthorizeResponse = res
            .response
            .parse_struct("GrabpayAuthorizeResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "GrabPay charge complete response did not match the expected schema"
                            .to_string(),
                    ),
                },
            })?;

        if let Some(event) = event_builder {
            event.set_connector_response(&response)
        }

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(errors::ConnectorError::ResponseDeserializationFailed {
            context: errors::ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some(
                    "GrabPay authorize response transformation failed".to_string(),
                ),
            },
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Grabpay,
    curl_request: Json(GrabpayServerSessionAuthenticationTokenRequest),
    curl_response: GrabpayServerSessionAuthenticationTokenResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.build_json_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(oauth_endpoint(
                &req.resource_common_data.connectors.grabpay.base_url,
                OAUTH_TOKEN_PATH,
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Grabpay<T>
{
    /// GrabPay's flow: `Authenticate` (POST `/charge/init` → OAuth redirect URL) on the
    /// initial request; `Authorize` (POST `/charge/complete`) after the customer completes
    /// the OAuth consent and the caller redirects back (mirrors Flywire).
    fn next_authentication_step(
        &self,
        _auth_type: common_enums::AuthenticationType,
        _payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        _completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use interfaces::connector_types::{AuthenticationStep, RedirectState};
        match redirect_state {
            RedirectState::InitialRequest => AuthenticationStep::Authenticate,
            RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams => {
                AuthenticationStep::Authorize
            }
        }
    }

    fn should_do_access_token(&self, _payment_method: Option<enums::PaymentMethod>) -> bool {
        false
    }

    fn should_do_session_token(&self, connector_feature_data: Option<&Secret<String>>) -> bool {
        connector_feature_data
            .and_then(|data| serde_json::from_str::<serde_json::Value>(data.peek()).ok())
            .map(|feature_data| {
                feature_data
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
            })
            .unwrap_or(false)
    }

    fn requires_authorize_post_redirect(&self) -> bool {
        true
    }

    fn merchant_order_id_source(&self) -> connector_types::MerchantOrderIdSource {
        connector_types::MerchantOrderIdSource::TransactionId
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Grabpay<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        use common_utils::crypto::VerifySignature;

        let connector_account_details = connector_account_details
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookVerificationSecretNotFound))?;
        let auth = grabpay::GrabpayAuthType::try_from(&connector_account_details)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;

        let incoming_authorization =
            get_webhook_header(&request.headers, headers::AUTHORIZATION)?.trim();
        let content_type = get_webhook_header(&request.headers, headers::CONTENT_TYPE)?.trim();
        let date = get_webhook_header(&request.headers, headers::DATE)?;
        let path = grabpay_webhook_path(request.uri.as_deref())?;
        let method = format!("{:?}", request.method).to_uppercase();

        // GrabPay's Authorization header is `{partner_id}:{base64(HMAC-SHA256(canonical))}`.
        // The partner_id prefix is a public identifier; the signature must be compared in
        // constant time. `HmacSha256::verify_signature` uses ring's constant-time verification.
        let Some((incoming_partner_id, incoming_signature_b64)) =
            incoming_authorization.split_once(':')
        else {
            return Ok(false);
        };
        if incoming_partner_id != auth.partner_id.peek() {
            return Ok(false);
        }
        let incoming_signature = match BASE64_ENGINE.decode(incoming_signature_b64) {
            Ok(signature) => signature,
            Err(_) => return Ok(false),
        };
        let signing_string =
            grabpay_hmac_signing_string(&method, content_type, &path, &request.body, date)
                .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        crypto::HmacSha256
            .verify_signature(
                auth.partner_secret.peek().as_bytes(),
                &incoming_signature,
                signing_string.as_bytes(),
            )
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let webhook_body: GrabpayWebhookBody = request
            .body
            .parse_struct("GrabpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(grabpay::grabpay_webhook_event_type(&webhook_body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let webhook_body: GrabpayWebhookBody = request
            .body
            .parse_struct("GrabpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(Some(webhook_body.webhook_reference()))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body: GrabpayWebhookBody = request
            .body
            .parse_struct("GrabpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        if webhook_body.is_refund_event() {
            return Err(error_stack::report!(
                WebhookError::WebhookBodyDecodingFailed
            ));
        }

        let status = grabpay::grabpay_webhook_attempt_status(&webhook_body);
        let is_failure = status == enums::AttemptStatus::Failure;
        let reason = webhook_body.effective_reason();
        let connector_transaction_id = webhook_body.tx_id.clone().ok_or_else(|| {
            error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "txID" })
        })?;

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(connector_transaction_id)),
            status,
            connector_response_reference_id: None,
            connector_request_reference_id: webhook_body.partner_tx_id,
            mandate_reference: None,
            error_code: if is_failure { reason.clone() } else { None },
            error_message: if is_failure { reason.clone() } else { None },
            error_reason: reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body: GrabpayWebhookBody = request
            .body
            .parse_struct("GrabpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        if !webhook_body.is_refund_event() {
            return Err(error_stack::report!(
                WebhookError::WebhookBodyDecodingFailed
            ));
        }

        let status = grabpay::grabpay_webhook_refund_status(&webhook_body);
        let is_failure = status == enums::RefundStatus::Failure;
        let reason = webhook_body.effective_reason();

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: webhook_body.tx_id,
            merchant_transaction_id: webhook_body
                .payload
                .as_ref()
                .and_then(|payload| payload.partner_group_tx_id.clone()),
            status,
            connector_response_reference_id: webhook_body.partner_tx_id,
            error_code: if is_failure { reason.clone() } else { None },
            error_message: if is_failure { reason } else { None },
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let webhook_body: GrabpayWebhookBody = request
            .body
            .parse_struct("GrabpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(Box::new(webhook_body))
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"txType":"payment","txStatus":"success","partnerID":"partner_123","partnerTxID":"txn_123","txID":"grab_txn_123","amount":100,"currency":"SGD","payload":{"newStatus":"success","paymentMethod":"GRABPAY"}}"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Grabpay<T>
{
    fn decode_redirect_response_body(
        &self,
        request: &RequestDetails,
        _secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<Vec<u8>, IntegrationError> {
        Ok(request.body.clone())
    }

    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, IntegrationError> {
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        request: &RequestDetails,
        connector_feature_data: Option<&Secret<String>>,
    ) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
        process_grabpay_redirect_response(request, connector_feature_data)
    }
}

fn process_grabpay_redirect_response(
    request: &RequestDetails,
    base_connector_feature_data: Option<&Secret<String>>,
) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
    let code = get_query_param(request, "code");
    let state = get_query_param(request, "state");
    let error = get_query_param(request, "error");
    let connector_feature_data =
        build_redirect_connector_feature_data(base_connector_feature_data, &code, &state)?;

    Ok(RedirectDetailsResponse {
        resource_id: None,
        status: None,
        connector_response_reference_id: None,
        error_code: error,
        error_message: None,
        error_reason: None,
        response_amount: None,
        raw_connector_response: None,
        connector_feature_data,
    })
}

fn build_redirect_connector_feature_data(
    base_connector_feature_data: Option<&Secret<String>>,
    code: &Option<String>,
    state: &Option<String>,
) -> CustomResult<Option<String>, IntegrationError> {
    let mut feature_data = match base_connector_feature_data {
        Some(feature_data) => serde_json::from_str::<serde_json::Value>(feature_data.peek())
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data",
                context: grabpay_integration_context(
                    "GrabPay redirect response received malformed connector_feature_data JSON",
                ),
            })?,
        None => serde_json::Value::Object(serde_json::Map::new()),
    };

    let feature_data_object = feature_data.as_object_mut().ok_or_else(|| {
        error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data",
            context: grabpay_integration_context(
                "GrabPay redirect response expected connector_feature_data to be a JSON object",
            ),
        })
    })?;

    if let Some(code) = code {
        feature_data_object.insert("code".to_string(), serde_json::Value::String(code.clone()));
    }

    if let Some(state) = state {
        feature_data_object.insert(
            "callback_state".to_string(),
            serde_json::Value::String(state.clone()),
        );
    }

    if feature_data_object.is_empty() {
        Ok(None)
    } else {
        serde_json::to_string(&feature_data)
            .map(Some)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: grabpay_integration_context(
                    "GrabPay redirect response failed to serialize connector_feature_data",
                ),
            })
    }
}

fn get_query_param(request: &RequestDetails, param_name: &str) -> Option<String> {
    request.query_params.as_ref().and_then(|query_params| {
        serde_json::from_str::<serde_json::Value>(query_params)
            .ok()
            .and_then(|value| {
                value
                    .as_object()
                    .and_then(|object| object.get(param_name))
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .or_else(|| {
                url::form_urlencoded::parse(query_params.as_bytes())
                    .find(|(key, _)| key == param_name)
                    .map(|(_, value)| value.into_owned())
            })
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Grabpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Grabpay<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Grabpay, amount_type: MinorUnit);

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Grabpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

static GRABPAY_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> = LazyLock::new(|| {
    let supported_capture_methods = vec![enums::CaptureMethod::Automatic];

    let mut grabpay_supported_payment_methods = SupportedPaymentMethods::new();

    grabpay_supported_payment_methods.add(
        enums::PaymentMethod::Wallet,
        PaymentMethodType::Grabpay,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods,
            specific_features: None,
        },
    );

    grabpay_supported_payment_methods
});

static GRABPAY_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "GrabPay",
    description: "GrabPay One-Time Charge wallet payments.",
    connector_type: types::PaymentConnectorCategory::AlternativePaymentMethod,
};

impl ConnectorSpecifications for Grabpay<domain_types::payment_method_data::DefaultPCIHolder> {
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&GRABPAY_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&*GRABPAY_SUPPORTED_PAYMENT_METHODS)
    }
}

#[allow(
    clippy::indexing_slicing,
    clippy::panic_in_result_fn,
    dead_code,
    unused_imports
)]
mod tests {
    use base64::Engine;
    use common_enums::{AttemptStatus, RefundStatus};
    use common_utils::consts::BASE64_ENGINE_URL_SAFE_NO_PAD;
    use domain_types::connector_types::EventType;
    use hyperswitch_masking::Secret;

    use super::{
        build_hmac_authorization, build_pop_signature, format_rfc7231_date, grabpay, CONTENT_TYPE,
    };

    fn test_auth() -> grabpay::GrabpayAuthType {
        grabpay::GrabpayAuthType {
            partner_id: Secret::new("partner_id_test".to_string()),
            partner_secret: Secret::new("partner_secret_test".to_string()),
            client_id: Secret::new("client_id_test".to_string()),
            client_secret: Secret::new("client_secret_test".to_string()),
            merchant_id: Secret::new("merchant_id_test".to_string()),
        }
    }

    #[test]
    fn test_hmac_authorization_signature() -> Result<(), Box<dyn std::error::Error>> {
        let authorization = build_hmac_authorization(
            &test_auth(),
            "POST",
            CONTENT_TYPE,
            "/charge/init",
            br#"{"amount":1000,"currency":"SGD"}"#,
            "Wed, 02 Nov 2022 08:00:00 GMT",
        )?;

        assert_eq!(
            authorization,
            "partner_id_test:ZGa243Gm1Z3nTa+VoE7nFl7GuJ+j6bgTS2wvwiE/k50="
        );
        Ok(())
    }

    #[test]
    fn test_pop_signature() -> Result<(), Box<dyn std::error::Error>> {
        let pop = build_pop_signature(&test_auth(), "access_token_test", "1667376000")?;
        let payload_bytes = BASE64_ENGINE_URL_SAFE_NO_PAD.decode(pop)?;
        let payload = serde_json::from_slice::<serde_json::Value>(&payload_bytes)?;

        assert_eq!(payload["time_since_epoch"], 1_667_376_000);
        assert!(payload["sig"].as_str().is_some_and(|sig| !sig.is_empty()));
        Ok(())
    }

    #[test]
    fn test_rfc7231_date_format() -> Result<(), Box<dyn std::error::Error>> {
        let timestamp = time::OffsetDateTime::from_unix_timestamp(1_667_376_000)?;

        assert_eq!(
            format_rfc7231_date(timestamp)?,
            "Wed, 02 Nov 2022 08:00:00 GMT"
        );
        Ok(())
    }

    #[test]
    fn test_grabpay_payment_status_mapping() {
        assert_eq!(
            AttemptStatus::from(grabpay::GrabpayPaymentStatus::Success),
            AttemptStatus::Charged
        );
        assert_eq!(
            AttemptStatus::from(grabpay::GrabpayPaymentStatus::Authorised),
            AttemptStatus::Authorized
        );
        assert_eq!(
            AttemptStatus::from(grabpay::GrabpayPaymentStatus::Processing),
            AttemptStatus::Pending
        );
        assert_eq!(
            AttemptStatus::from(grabpay::GrabpayPaymentStatus::AuthorisationDeclined),
            AttemptStatus::Failure
        );
    }

    #[test]
    fn test_grabpay_refund_status_mapping() {
        assert_eq!(
            RefundStatus::from(grabpay::GrabpayRefundStatus::Success),
            RefundStatus::Success
        );
        assert_eq!(
            RefundStatus::from(grabpay::GrabpayRefundStatus::Processing),
            RefundStatus::Pending
        );
        assert_eq!(
            RefundStatus::from(grabpay::GrabpayRefundStatus::TransactionAlreadyExist),
            RefundStatus::Pending
        );
    }

    #[test]
    fn test_grabpay_auth_webhook_with_orig_tx_id_is_payment(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let webhook_body = serde_json::from_slice::<grabpay::GrabpayWebhookBody>(
            br#"{"txType":"Auth","txCategory":"Charge","partnerTxID":"merchant_txn_123","txID":"grab_txn_123","origTxID":"grab_txn_123","amount":1,"currency":"PHP","status":"success","payload":{"partnerGroupTxID":"merchant_txn_123","newStatus":"authorised","reason":"pending_capture","paymentMethod":"GPWALLET"},"txStatus":"authorised"}"#,
        )?;

        assert!(!webhook_body.is_refund_event());
        assert_eq!(
            grabpay::grabpay_webhook_event_type(&webhook_body),
            EventType::PaymentIntentAuthorizationSuccess
        );
        assert_eq!(
            grabpay::grabpay_webhook_attempt_status(&webhook_body),
            AttemptStatus::Authorized
        );
        Ok(())
    }

    #[test]
    fn test_grabpay_refund_webhook_classification() -> Result<(), Box<dyn std::error::Error>> {
        let webhook_body = serde_json::from_slice::<grabpay::GrabpayWebhookBody>(
            br#"{"txType":"Refund","txCategory":"Refund","partnerTxID":"merchant_refund_123","txID":"grab_refund_123","origTxID":"grab_txn_123","amount":1,"currency":"PHP","payload":{"partnerGroupTxID":"merchant_txn_123","newStatus":"success"}}"#,
        )?;

        assert!(webhook_body.is_refund_event());
        assert_eq!(
            grabpay::grabpay_webhook_event_type(&webhook_body),
            EventType::RefundSuccess
        );
        assert_eq!(
            grabpay::grabpay_webhook_refund_status(&webhook_body),
            RefundStatus::Success
        );
        Ok(())
    }
}

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Grabpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        CreateOrder,
        CreatePaymentMethod,
        DefendDispute,
        GetConnectorCustomer,
        GetPaymentMethod,
        IncrementalAuthorization,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        PaymentMethodEligibility,
        Recharge,
        ServerAuthenticationToken,
        SubmitEvidence,
        VoidPC,
        VoidPostRefund,
        RepeatPayment
    ],
    not_supported: [Capture, MandateRevoke, SetupMandate, Void],
);
