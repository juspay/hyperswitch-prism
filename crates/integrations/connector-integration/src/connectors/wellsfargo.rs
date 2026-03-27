pub mod transformers;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::Method,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateConnectorCustomer,
        CreateOrder, CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke,
        PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, SessionTokenRequestData, SessionTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use ring::digest;
use serde::Serialize;
use std::fmt::Debug;
use time::OffsetDateTime;
use transformers::{
    self as wellsfargo, WellsfargoCaptureRequest, WellsfargoPaymentsRequest,
    WellsfargoPaymentsResponse, WellsfargoPaymentsResponse as WellsfargoCaptureResponse,
    WellsfargoPaymentsResponse as WellsfargoVoidResponse,
    WellsfargoPaymentsResponse as WellsfargoPSyncResponse,
    WellsfargoPaymentsResponse as WellsfargoRefundResponse,
    WellsfargoPaymentsResponse as WellsfargoSetupMandateResponse,
    WellsfargoRSyncResponse as WellsfargoRefundSyncResponse, WellsfargoRefundRequest,
    WellsfargoVoidRequest, WellsfargoZeroMandateRequest,
};
use url::Url;

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// Trait to unify PaymentFlowData and RefundFlowData for header building
pub trait FlowDataBase {
    fn get_connectors(&self) -> &Connectors;
}

impl FlowDataBase for PaymentFlowData {
    fn get_connectors(&self) -> &Connectors {
        &self.connectors
    }
}

impl FlowDataBase for RefundFlowData {
    fn get_connectors(&self) -> &Connectors {
        &self.connectors
    }
}

use error_stack::{Report, ResultExt};

// Trait implementations with generic type parameters
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Wellsfargo<T>
{
}

// Empty SourceVerification implementations for unimplemented flows

// Empty ConnectorIntegrationV2 implementations for unimplemented flows
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Wellsfargo<T>
{
}

// Additional empty implementations for token, customer, and access token flows

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Wellsfargo<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Wellsfargo<T>
{
}

pub(crate) mod headers {
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

macros::create_all_prerequisites!(
    connector_name: Wellsfargo,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: WellsfargoPaymentsRequest<T>,
            response_body: WellsfargoPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: WellsfargoPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: WellsfargoCaptureRequest,
            response_body: WellsfargoCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: WellsfargoVoidRequest,
            response_body: WellsfargoVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: WellsfargoRefundRequest,
            response_body: WellsfargoRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: WellsfargoRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: WellsfargoZeroMandateRequest<T>,
            response_body: WellsfargoSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn generate_digest(&self, payload: &[u8]) -> String {
            let payload_digest = digest::digest(&digest::SHA256, payload);
            BASE64_ENGINE.encode(payload_digest)
        }

        fn generate_signature(
            &self,
            auth: transformers::WellsfargoAuthType,
            host: String,
            resource: &str,
            payload: &str,
            date: OffsetDateTime,
            http_method: Method,
        ) -> CustomResult<String, errors::ConnectorError> {
            let api_key = auth.api_key.expose();
            let api_secret = auth.api_secret.expose();
            let merchant_id = auth.merchant_account.expose();

            let is_post_method = matches!(http_method, Method::Post);
            let is_patch_method = matches!(http_method, Method::Patch);
            let digest_str = if is_post_method || is_patch_method {
                "digest "
            } else {
                ""
            };

            let headers_str = format!("host date (request-target) {digest_str}v-c-merchant-id");

            let request_target = match http_method {
                Method::Post => format!("(request-target): post {resource}\ndigest: SHA-256={payload}\n"),
                Method::Patch => format!("(request-target): patch {resource}\ndigest: SHA-256={payload}\n"),
                Method::Delete => format!("(request-target): delete {resource}\n"),
                Method::Get => format!("(request-target): get {resource}\n"),
                _ => format!("(request-target): {http_method} {resource}\n"),
            };

            let signature_string = format!(
                "host: {host}\ndate: {date}\n{request_target}v-c-merchant-id: {merchant_id}"
            );

            // Decode the base64-encoded API secret before using it for HMAC
            let key_value = BASE64_ENGINE
                .decode(api_secret.as_bytes())
                .change_context(errors::ConnectorError::InvalidConnectorConfig {
                    config: "connector_account_details.api_secret",
                })
                .attach_printable("Failed to decode base64-encoded API secret")?;

            // Use ring::hmac for HMAC-SHA256
            let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &key_value);
            let signature = ring::hmac::sign(&key, signature_string.as_bytes());
            let signature_value = BASE64_ENGINE.encode(signature.as_ref());

            Ok(format!(
                r#"keyid="{api_key}", algorithm="HmacSHA256", headers="{headers_str}", signature="{signature_value}""#
            ))
        }

        pub fn build_headers<F, FlowData, Req, Res>(
            &self,
            req: &RouterDataV2<F, FlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError>
        where
            Self: ConnectorIntegrationV2<F, FlowData, Req, Res>,
            FlowData: FlowDataBase,
        {
            let date = OffsetDateTime::now_utc();
            let auth = transformers::WellsfargoAuthType::try_from(&req.connector_config)?;
            let merchant_account = auth.merchant_account.clone().expose();

            let base_url = &req.resource_common_data.get_connectors().wellsfargo.base_url;
            let wellsfargo_host = Url::parse(base_url)
                .change_context(errors::ConnectorError::RequestEncodingFailed)
                .attach_printable("Failed to parse Wells Fargo base URL")?;
            let host = wellsfargo_host
                .host_str()
                .ok_or(errors::ConnectorError::RequestEncodingFailed)?;

            // Get the request body for digest calculation
            let request_body = self.get_request_body(req)?;
            let sha256 = if let Some(body) = request_body {
                let body_string = body.get_inner_value();
                self.generate_digest(body_string.expose().as_bytes())
            } else {
                String::new()
            };

            // Get URL path
            let url = self.get_url(req)?;
            let path: String = url.chars().skip(base_url.len() - 1).collect();

            let http_method = self.get_http_method();
            let signature = self.generate_signature(
                auth,
                host.to_string(),
                &path,
                &sha256,
                date,
                http_method,
            )?;

            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/hal+json;charset=utf-8".to_string().into(),
                ),
                ("v-c-merchant-id".to_string(), merchant_account.into_masked()),
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

        pub fn connector_base_url<'a, F, FlowData, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, FlowData, Req, Res>,
        ) -> &'a str
        where
            FlowData: FlowDataBase,
        {
            &req.resource_common_data.get_connectors().wellsfargo.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Wellsfargo<T>
{
    fn id(&self) -> &'static str {
        "wellsfargo"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json;charset=utf-8"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.wellsfargo.base_url.as_ref()
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: Result<
            wellsfargo::WellsfargoErrorResponse,
            Report<common_utils::errors::ParsingError>,
        > = res.response.parse_struct("Wellsfargo ErrorResponse");

        let error_message = if res.status_code == 401 {
            "Authentication failed"
        } else {
            NO_ERROR_MESSAGE
        };
        match response {
            Ok(transformers::WellsfargoErrorResponse::StandardError(response)) => {
                with_error_response_body!(event_builder, response);

                let (code, message, reason) = match response.error_information {
                    Some(ref error_info) => {
                        let detailed_error_info = error_info.details.as_ref().map(|details| {
                            details
                                .iter()
                                .map(|det| {
                                    format!(
                                        "{} : {}",
                                        det.field.as_deref().unwrap_or("unknown"),
                                        det.reason.as_deref().unwrap_or("unknown")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        });
                        (
                            error_info.reason.clone(),
                            error_info.message.clone(),
                            transformers::get_error_reason(
                                error_info.message.clone(),
                                detailed_error_info,
                                None, // AVS/risk info support can be added later
                            ),
                        )
                    }
                    None => {
                        let detailed_error_info = response.details.as_ref().map(|details| {
                            details
                                .iter()
                                .map(|det| {
                                    let field = det.field.as_deref().unwrap_or("unknown");
                                    let reason = det.reason.as_deref().unwrap_or("unknown");
                                    format!("{} : {}", field, reason)
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        });
                        (
                            response.reason.clone(),
                            response.message.clone(),
                            transformers::get_error_reason(
                                response.message.clone(),
                                detailed_error_info,
                                None, // AVS/risk info support can be added later
                            ),
                        )
                    }
                };

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: message.unwrap_or_else(|| error_message.to_string()),
                    reason: reason.or_else(|| Some(error_message.to_string())),
                    attempt_status: None,
                    connector_transaction_id: response.id.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                })
            }
            Ok(transformers::WellsfargoErrorResponse::AuthenticationError(response)) => {
                event_builder.map(|i| i.set_connector_response(&response));
                tracing::info!(connector_response=?response);
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
                })
            }
            Ok(transformers::WellsfargoErrorResponse::NotAvailableError(response)) => {
                event_builder.map(|i| i.set_connector_response(&response));
                tracing::info!(connector_response=?response);
                let error_response = response
                    .errors
                    .iter()
                    .filter_map(|error_info| error_info.message.clone())
                    .collect::<Vec<String>>()
                    .join(" & ");
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: NO_ERROR_CODE.to_string(),
                    message: error_response.clone(),
                    reason: Some(error_response),
                    attempt_status: None,
                    connector_transaction_id: response.id.clone(),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                })
            }
            Err(error_msg) => {
                event_builder.map(|event| event.set_connector_response(&serde_json::json!({"error": res.response.escape_ascii().to_string(), "status_code": res.status_code})));
                tracing::error!(deserialization_error =? error_msg);
                domain_types::utils::handle_json_response_deserialization_failure(res, "wellsfargo")
            }
        }
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Wellsfargo,
    curl_request: Json(WellsfargoPaymentsRequest),
    curl_response: WellsfargoPaymentsResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}pts/v2/payments/",
                self.connector_base_url(req)
            ))
        }
    }
);

// Capture implementation - POST request to capture authorized payment
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Wellsfargo,
    curl_request: Json(WellsfargoCaptureRequest),
    curl_response: WellsfargoCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let connector_payment_id = req.request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)
                .attach_printable("Missing connector transaction ID for capture")?;

            Ok(format!(
                "{}pts/v2/payments/{}/captures",
                self.connector_base_url(req),
                connector_payment_id
            ))
        }
    }
);

// Void implementation - POST request to reverse/cancel authorized payment
// Uses /reversals endpoint (matching Hyperswitch implementation) which is semantically correct
// for authorization reversals. This endpoint should only work for authorized-but-not-captured payments.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Wellsfargo,
    curl_request: Json(WellsfargoVoidRequest),
    curl_response: WellsfargoVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let connector_payment_id = &req.request.connector_transaction_id;

            Ok(format!(
                "{}pts/v2/payments/{}/reversals",
                self.connector_base_url(req),
                connector_payment_id
            ))
        }
    }
);

// Refund implementation - POST request to refund captured payment
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Wellsfargo,
    curl_request: Json(WellsfargoRefundRequest),
    curl_response: WellsfargoRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let connector_transaction_id = &req.request.connector_transaction_id;

            Ok(format!(
                "{}pts/v2/payments/{}/refunds",
                self.connector_base_url(req),
                connector_transaction_id
            ))
        }
    }
);

// PSync (Payment Sync) implementation - GET request, no request body
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Wellsfargo,
    curl_response: WellsfargoPSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let connector_payment_id = req.request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)
                .attach_printable("Missing connector transaction ID for payment sync")?;

            Ok(format!(
                "{}pts/v2/payments/{}",
                self.connector_base_url(req),
                connector_payment_id
            ))
        }
    }
);

// RSync (Refund Sync) implementation - GET request to check refund status
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Wellsfargo,
    curl_response: WellsfargoRefundSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let connector_refund_id = &req.request.connector_transaction_id;

            Ok(format!(
                "{}tss/v2/transactions/{}",
                self.connector_base_url(req),
                connector_refund_id
            ))
        }
    }
);

// SetupMandate implementation - POST request to setup mandate (zero-dollar auth)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Wellsfargo,
    curl_request: Json(WellsfargoZeroMandateRequest<T>),
    curl_response: WellsfargoSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}pts/v2/payments",
                self.connector_base_url(req)
            ))
        }
    }
);

// Stub implementations for unsupported flows

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Wellsfargo<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Wellsfargo<T>
{
}

// SourceVerification implementations for all flows

// SourceVerification implementations for flows converted to macros
