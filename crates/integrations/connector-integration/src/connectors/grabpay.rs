pub mod transformers;

use std::{fmt::Debug, sync::LazyLock};

use base64::Engine;
use common_enums::{enums, CurrencyUnit, PaymentMethodType};
use common_utils::{
    consts::BASE64_ENGINE_URL_SAFE_NO_PAD,
    crypto,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{Method, Request, RequestBuilder, RequestContent},
};
use domain_types::{
    connector_flow::{Authorize, CreateOrder, PSync, RSync, Refund, ServerAuthenticationToken},
    connector_types::{ConnectorSpecifications, SupportedPaymentMethodsExt},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RedirectDetailsResponse, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RequestDetails,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{self, IntegrationError},
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
    self as grabpay, GrabpayAuthorizeRequest, GrabpayAuthorizeResponse,
    GrabpayChargeCompleteResponse, GrabpayCreateOrderRequest, GrabpayCreateOrderResponse,
    GrabpayRefundRequest, GrabpayRefundResponse, GrabpayRefundSyncResponse,
    GrabpayServerAuthenticationTokenRequest, GrabpayServerAuthenticationTokenResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

const CONTENT_TYPE: &str = "application/json";
const CHARGE_INIT_PATH: &str = "/charge/init";
const CHARGE_COMPLETE_PATH: &str = "/charge/complete";
const CHARGE_STATUS_PREFIX: &str = "/charge";
const REFUND_PATH: &str = "/refund";
const PRODUCTION_HOST: &str = "https://partner-api.grab.com";
const STAGING_HOST: &str = "https://partner-api.stg-myteksi.com";
const OAUTH_TOKEN_PATH: &str = "/grabid/v1/oauth2/token";

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const DATE: &str = "Date";
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
            flow: ServerAuthenticationToken,
            request_body: GrabpayServerAuthenticationTokenRequest,
            response_body: GrabpayServerAuthenticationTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: GrabpayCreateOrderRequest,
            response_body: GrabpayCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
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
            let date = format_rfc7231_date(time::OffsetDateTime::now_utc());
            let authorization = build_hmac_authorization(auth, method, path, body, &date)?;

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
            let date = format_rfc7231_date(now);
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
    let host = if base_url.starts_with(STAGING_HOST) {
        STAGING_HOST
    } else {
        PRODUCTION_HOST
    };
    format!("{host}{path}")
}

fn build_hmac_authorization(
    auth: &grabpay::GrabpayAuthType,
    method: &str,
    path: &str,
    body: &[u8],
    date: &str,
) -> CustomResult<String, IntegrationError> {
    use common_utils::crypto::{GenerateDigest, SignMessage};

    let body_digest = crypto::Sha256.generate_digest(body).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;
    let encoded_body_digest = BASE64_ENGINE.encode(body_digest);
    let signing_string =
        format!("{method}\n{CONTENT_TYPE}\n{date}\n{path}\n{encoded_body_digest}\n");
    let signature = crypto::HmacSha256
        .sign_message(
            auth.partner_secret.peek().as_bytes(),
            signing_string.as_bytes(),
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

    Ok(format!(
        "{}:{}",
        auth.partner_id.peek(),
        BASE64_ENGINE.encode(signature)
    ))
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
            context: Default::default(),
        })?;
    let sig = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(signature);
    let time_since_epoch =
        timestamp
            .parse::<i64>()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
    let payload = serde_json::json!({
        "time_since_epoch": time_since_epoch,
        "sig": sig,
    });
    let payload_json = serde_json::to_string(&payload).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;

    Ok(BASE64_ENGINE_URL_SAFE_NO_PAD.encode(payload_json.as_bytes()))
}

fn format_rfc7231_date(date_time: time::OffsetDateTime) -> String {
    let weekday = match date_time.weekday() {
        time::Weekday::Monday => "Mon",
        time::Weekday::Tuesday => "Tue",
        time::Weekday::Wednesday => "Wed",
        time::Weekday::Thursday => "Thu",
        time::Weekday::Friday => "Fri",
        time::Weekday::Saturday => "Sat",
        time::Weekday::Sunday => "Sun",
    };
    let month = match date_time.month() {
        time::Month::January => "Jan",
        time::Month::February => "Feb",
        time::Month::March => "Mar",
        time::Month::April => "Apr",
        time::Month::May => "May",
        time::Month::June => "Jun",
        time::Month::July => "Jul",
        time::Month::August => "Aug",
        time::Month::September => "Sep",
        time::Month::October => "Oct",
        time::Month::November => "Nov",
        time::Month::December => "Dec",
    };

    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        weekday,
        date_time.day(),
        month,
        date_time.year(),
        date_time.hour(),
        date_time.minute(),
        date_time.second()
    )
}

fn payment_access_token(data: &PaymentFlowData) -> CustomResult<String, IntegrationError> {
    data.get_access_token().or_else(|_| {
        grabpay::access_token_from_connector_feature_data(data.connector_feature_data.as_ref())
    })
}

fn refund_access_token(data: &RefundFlowData) -> CustomResult<String, IntegrationError> {
    data.get_access_token().or_else(|_| {
        grabpay::access_token_from_connector_feature_data(data.connector_feature_data.as_ref())
    })
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
        let _auth = grabpay::GrabpayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

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
                code: res.status_code.to_string(),
                message: "GrabPay error response was empty".to_string(),
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
                context: Default::default(),
            })?;

        with_error_response_body!(event_builder, response);

        let message = response
            .message
            .or(response.error_description)
            .or(response.reason.clone())
            .unwrap_or_else(|| "GrabPay error response".to_string());
        let code = response
            .code
            .or(response.error)
            .or(response.reason.clone())
            .unwrap_or_else(|| res.status_code.to_string());

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
    curl_request: Json(GrabpayCreateOrderRequest),
    curl_response: GrabpayCreateOrderResponse,
    flow_name: CreateOrder,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentCreateOrderData,
    flow_response: PaymentCreateOrderResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = grabpay::GrabpayAuthType::try_from(&req.connector_config).change_context(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                },
            )?;
            let connector_request = GrabpayCreateOrderRequest::try_from(req.clone())?;
            let body = serde_json::to_vec(&connector_request).change_context(
                IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                },
            )?;
            let request_path = url::Url::parse(&self.get_url(req)?)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?
                .path()
                .to_string();

            self.build_hmac_headers(&auth, "POST", &request_path, &body)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
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
                    context: Default::default(),
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
                    context: Default::default(),
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
                    context: Default::default(),
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
        match req.resource_common_data.get_access_token() {
            Ok(access_token) => {
                let auth = grabpay::GrabpayAuthType::try_from(&req.connector_config)
                    .change_context(IntegrationError::FailedToObtainAuthType {
                        context: Default::default(),
                    })?;
                self.build_pop_headers(&auth, &access_token)
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
        if req.resource_common_data.get_access_token().is_ok() {
            Ok(format!(
                "{}{CHARGE_COMPLETE_PATH}",
                self.connector_base_url_payments(req)
            ))
        } else if req.resource_common_data.connector_feature_data.is_some() {
            Err(
                error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "access_token",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "GrabPay post-redirect authorize requires a successful OAuth token exchange"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }),
            )
        } else {
            grabpay::build_grabpay_authorize_url(req)
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
        if req.resource_common_data.get_access_token().is_ok() {
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
        let method = if req.resource_common_data.get_access_token().is_ok() {
            Method::Post
        } else {
            Method::Get
        };

        Ok(Some(
            RequestBuilder::new()
                .method(method)
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
        let response = if data.resource_common_data.get_access_token().is_ok() {
            res.response
                .parse_struct("GrabpayAuthorizeResponse")
                .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                })?
        } else {
            GrabpayAuthorizeResponse::redirect(data).change_context(
                errors::ConnectorError::ResponseDeserializationFailed {
                    context: Default::default(),
                },
            )?
        };

        if let Some(event) = event_builder {
            event.set_connector_response(&response)
        }

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(errors::ConnectorError::ResponseDeserializationFailed {
            context: Default::default(),
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Grabpay<T>
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_http_method(&self) -> Method {
        Method::Post
    }

    fn get_headers(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(self.build_json_headers())
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(oauth_endpoint(
            &req.resource_common_data.connectors.grabpay.base_url,
            OAUTH_TOKEN_PATH,
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        match GrabpayServerAuthenticationTokenRequest::try_from(req) {
            Ok(request) => Ok(Some(RequestContent::Json(Box::new(request)))),
            Err(error) if grabpay::is_missing_oauth_code_error(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<Request>, IntegrationError> {
        let Some(body) = self.get_request_body(req)? else {
            return Ok(None);
        };

        Ok(Some(
            RequestBuilder::new()
                .method(Method::Post)
                .url(self.get_url(req)?.as_str())
                .attach_default_headers()
                .headers(self.get_headers(req)?)
                .set_body(body)
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
        errors::ConnectorError,
    > {
        let response: GrabpayServerAuthenticationTokenResponse = res
            .response
            .parse_struct("GrabpayServerAuthenticationTokenResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
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
            context: Default::default(),
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
    fn should_do_order_create(&self) -> bool {
        true
    }

    fn should_do_access_token(&self, payment_method: Option<enums::PaymentMethod>) -> bool {
        match payment_method {
            Some(payment_method) => matches!(payment_method, enums::PaymentMethod::Wallet),
            None => true,
        }
    }

    fn should_do_session_token(&self) -> bool {
        false
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
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Grabpay<T>
{
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
    let raw_connector_response = serde_json::to_string(&serde_json::json!({
        "code": code,
        "state": state,
        "error": error,
    }))
    .change_context(IntegrationError::RequestEncodingFailed {
        context: Default::default(),
    })?;

    Ok(RedirectDetailsResponse {
        resource_id: None,
        status: None,
        connector_response_reference_id: None,
        error_code: error,
        error_message: None,
        error_reason: None,
        response_amount: None,
        raw_connector_response: Some(raw_connector_response),
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
                context: Default::default(),
            })?,
        None => serde_json::Value::Object(serde_json::Map::new()),
    };

    let feature_data_object = feature_data.as_object_mut().ok_or_else(|| {
        error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data",
            context: Default::default(),
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
                context: Default::default(),
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
    connector_types::PaymentOrderCreate for Grabpay<T>
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
    connector_types::ServerAuthentication for Grabpay<T>
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

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::panic_in_result_fn)]
mod tests {
    use base64::Engine;
    use common_enums::{AttemptStatus, RefundStatus};
    use common_utils::consts::BASE64_ENGINE_URL_SAFE_NO_PAD;
    use hyperswitch_masking::Secret;

    use super::{build_hmac_authorization, build_pop_signature, format_rfc7231_date, grabpay};

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
            format_rfc7231_date(timestamp),
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
}

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Grabpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        CreatePaymentMethod,
        DefendDispute,
        GetConnectorCustomer,
        GetPaymentMethod,
        MandateRevoke,
        Authenticate,
        IncrementalAuthorization,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        PaymentMethodEligibility,
        Recharge,
        VoidPC,
        VoidPostRefund,
        RepeatPayment,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
    not_supported: [Capture, Void],
);
