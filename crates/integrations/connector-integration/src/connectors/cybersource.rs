use std::fmt::Debug;

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::BytesExt,
    types::StringMajorUnit,
    Method,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{Connectors, HasConnectors},
    utils,
};

use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
pub mod transformers;
use base64::Engine;
use error_stack::{Report, ResultExt};
use ring::{digest, hmac};
use time::OffsetDateTime;
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use transformers::{
    self as cybersource, CybersourceAuthEnrollmentRequest, CybersourceAuthSetupRequest,
    CybersourceAuthSetupResponse, CybersourceAuthValidateRequest, CybersourceAuthenticateResponse,
    CybersourceAuthenticateResponse as CybersourcePostAuthenticateResponse,
    CybersourceClientAuthRequest, CybersourceClientAuthResponse, CybersourcePaymentsCaptureRequest,
    CybersourcePaymentsIncrementalAuthorizationRequest,
    CybersourcePaymentsIncrementalAuthorizationResponse, CybersourcePaymentsRequest,
    CybersourcePaymentsResponse, CybersourcePaymentsResponse as CybersourceCaptureResponse,
    CybersourcePaymentsResponse as CybersourceVoidResponse,
    CybersourcePaymentsResponse as CybersourceSetupMandateResponse,
    CybersourcePaymentsResponse as CybersourceRepeatPaymentResponse, CybersourceRefundRequest,
    CybersourceRefundResponse, CybersourceRepeatPaymentRequest, CybersourceRsyncResponse,
    CybersourceTransactionResponse, CybersourceVoidRequest, CybersourceZeroMandateRequest,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const CONNECTOR_UNAUTHORIZED_ERROR: &str = "Authentication Error from the connector";
}

// Trait implementations with generic type parameters

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Cybersource<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Cybersource<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Cybersource,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "void_post_capture".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Cybersource<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Cybersource<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Cybersource<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Cybersource<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Cybersource<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Cybersource,
    generic_type: T,
    api: [
       (
            flow: Authorize,
            request_body: CybersourcePaymentsRequest<T>,
            response_body: CybersourcePaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: CybersourceAuthSetupRequest<T>,
            response_body: CybersourceAuthSetupResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: CybersourceAuthEnrollmentRequest<T>,
            response_body: CybersourceAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: CybersourceAuthValidateRequest<T>,
            response_body: CybersourcePostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: CybersourceTransactionResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: CybersourcePaymentsCaptureRequest,
            response_body: CybersourceCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: CybersourceVoidRequest,
            response_body: CybersourceVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: CybersourceRefundRequest,
            response_body: CybersourceRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: CybersourceZeroMandateRequest<T>,
            response_body: CybersourceSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            response_body: CybersourceRsyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: CybersourceRepeatPaymentRequest,
            response_body: CybersourceRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: CybersourceClientAuthRequest,
            response_body: CybersourceClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: IncrementalAuthorization,
            request_body: CybersourcePaymentsIncrementalAuthorizationRequest,
            response_body: CybersourcePaymentsIncrementalAuthorizationResponse,
            router_data: RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
            FCD: HasConnectors,
        {
        let date = OffsetDateTime::now_utc();
        let cybersource_req = self.get_request_body(req)?;
        let auth = cybersource::CybersourceAuthType::try_from(&req.connector_config)?;
        let merchant_account = auth.merchant_account.clone();
        let base_url = self.base_url(req.resource_common_data.connectors());
        let cybersource_host =
            url::Url::parse(base_url).change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
        let host = cybersource_host
            .host_str()
            .ok_or(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
        let path: String = self
            .get_url(req)?
            .chars()
            .skip(base_url.len() - 1)
            .collect();
        let sha256 = self.generate_digest(
            cybersource_req
                .map(|req| req.get_inner_value().expose())
                .unwrap_or_default()
                .as_bytes()
        );
        let http_method = self.get_http_method();
        let signature = self.generate_signature(
            auth,
            host.to_string(),
            path.as_str(),
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
            (
                "v-c-merchant-id".to_string(),
                merchant_account.into_masked(),
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

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.cybersource.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.cybersource.base_url
        }

        pub fn generate_digest(&self, payload: &[u8]) -> String {
            let payload_digest = digest::digest(&digest::SHA256, payload);
            BASE64_ENGINE.encode(payload_digest)
        }

        pub fn generate_signature(
            &self,
            auth: cybersource::CybersourceAuthType,
            host: String,
            resource: &str,
            payload: &String,
            date: OffsetDateTime,
            http_method: Method,
        ) -> CustomResult<String, IntegrationError> {
            let cybersource::CybersourceAuthType {
            api_key,
            merchant_account,
            api_secret, ..
        } = auth;
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
            merchant_account.peek()
        );
        let key_value = BASE64_ENGINE
            .decode(api_secret.expose())
            .change_context(IntegrationError::InvalidConnectorConfig {
                config: "connector_account_details.api_secret",
                context: Default::default()
            })?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, &key_value);
        let signature_value =
            BASE64_ENGINE.encode(hmac::sign(&key, signature_string.as_bytes()).as_ref());
        let signature_header = format!(
            r#"keyid="{}", algorithm="HmacSHA256", headers="{headers}", signature="{signature_value}""#,
            api_key.peek()
        );

        Ok(signature_header)
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Cybersource<T>
{
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
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: Result<
            cybersource::CybersourceErrorResponse,
            Report<common_utils::errors::ParsingError>,
        > = res.response.parse_struct("Cybersource ErrorResponse");

        let error_message = if res.status_code == 401 {
            headers::CONNECTOR_UNAUTHORIZED_ERROR.to_string()
        } else {
            NO_ERROR_MESSAGE.to_string()
        };
        match response {
            Ok(transformers::CybersourceErrorResponse::StandardError(response)) => {
                with_error_response_body!(event_builder, response);

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
                            transformers::get_error_reason(
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
                            transformers::get_error_reason(
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
                })
            }
            Ok(transformers::CybersourceErrorResponse::AuthenticationError(response)) => {
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
                })
            }
            Ok(transformers::CybersourceErrorResponse::NotAvailableError(response)) => {
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
                })
            }
            Err(error_msg) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}))
                };
                tracing::error!(deserialization_error =? error_msg);
                utils::handle_json_response_deserialization_failure(res, "cybersource")
            }
        }
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourcePaymentsRequest),
    curl_response: CybersourcePaymentsResponse,
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
            Ok(format!("{}pts/v2/payments/", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourceAuthSetupRequest<T>),
    curl_response: CybersourceAuthSetupResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
            "{}risk/v1/authentication-setups",
            self.connector_base_url_payments(req)
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourceAuthEnrollmentRequest<T>),
    curl_response: CybersourceAuthenticateResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
            "{}risk/v1/authentications",
            self.connector_base_url_payments(req)
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourceAuthValidateRequest<T>),
    curl_response: CybersourcePostAuthenticateResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
            "{}risk/v1/authentication-results",
            self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourceZeroMandateRequest<T>),
    curl_response: CybersourceSetupMandateResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}pts/v2/payments/", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_response: CybersourceTransactionResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        let connector_payment_id = req
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
        Ok(format!(
            "{}tss/v2/transactions/{}",
            self.connector_base_url_payments(req),
            connector_payment_id
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourcePaymentsCaptureRequest),
    curl_response: CybersourceCaptureResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {

        let connector_payment_id = req
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
        Ok(format!(
            "{}pts/v2/payments/{}/captures",
            self.connector_base_url_payments(req),
            connector_payment_id,
        ))
        }
    }
);

// Add implementation for Void
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourceVoidRequest),
    curl_response: CybersourceVoidResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        let connector_payment_id = req.request.connector_transaction_id.clone();
        Ok(format!(
            "{}pts/v2/payments/{connector_payment_id}/reversals",
            self.connector_base_url_payments(req),
        ))
        }
    }
);

// Add implementation for Refund
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourceRefundRequest),
    curl_response: CybersourceRefundResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        let connector_payment_id = req.request.connector_transaction_id.clone();
        Ok(format!(
            "{}pts/v2/payments/{}/refunds",
            self.connector_base_url_refunds(req),
            connector_payment_id
        ))
        }
    }
);

// Implement RSync to fix the RefundSyncV2 trait requirement
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_response: CybersourceRsyncResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        let refund_id = req.request.connector_refund_id.clone();
        Ok(format!(
            "{}tss/v2/transactions/{}",
            self.connector_base_url_refunds(req),
            refund_id
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourceRepeatPaymentRequest),
    curl_response: CybersourceRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}pts/v2/payments/",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

// Manual implementation for ClientAuthenticationToken flow.
// Cannot use macro_connector_implementation! because Cybersource Flex v2 sessions API
// returns a raw JWT string (content-type: application/jwt), not JSON.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Cybersource<T>
{
    fn get_http_method(&self) -> Method {
        Method::Post
    }
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }
    fn get_headers(
        &self,
        req: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }
    fn get_url(
        &self,
        req: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}microform/v2/sessions",
            self.connector_base_url_payments(req)
        ))
    }
    fn get_request_body(
        &self,
        req: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let bridge = self.client_authentication_token;
        let input_data = CybersourceRouterData {
            connector: self.to_owned(),
            router_data: req.clone(),
        };
        let request = bridge.request_body(input_data)?;
        Ok(Some(common_utils::request::RequestContent::Json(Box::new(
            request,
        ))))
    }
    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        let response_bytes = res.response.to_vec();
        let response_str = String::from_utf8(response_bytes.clone())
            .map_err(|_| ConnectorError::response_handling_failed(res.status_code))?;

        // Cybersource Microform v2 sessions API returns JSON with captureContext field
        // Flex v2 sessions API returns a raw JWT string (content-type: application/jwt)
        let capture_context_jwt =
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response_str) {
                json.get("captureContext")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or(response_str)
            } else {
                response_str
            };

        // Decode JWT payload to extract client library info
        let parts: Vec<&str> = capture_context_jwt.split('.').collect();
        if parts.len() < 2 {
            return Err(Report::new(ConnectorError::response_handling_failed(
                res.status_code,
            )));
        }

        let payload = parts.get(1).ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed(res.status_code))
        })?;
        let decoded_bytes = Engine::decode(&BASE64_ENGINE, payload)
            .map_err(|_| ConnectorError::response_handling_failed(res.status_code))?;
        let decoded_str = String::from_utf8(decoded_bytes)
            .map_err(|_| ConnectorError::response_handling_failed(res.status_code))?;
        let decoded: serde_json::Value = serde_json::from_str(&decoded_str)
            .map_err(|_| ConnectorError::response_handling_failed(res.status_code))?;

        let client_library = decoded
            .get("ctx")
            .and_then(|ctx| ctx.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("data"))
            .and_then(|data| data.get("clientLibrary"))
            .and_then(|val| val.as_str())
            .unwrap_or_default()
            .to_string();
        let client_library_integrity = decoded
            .get("ctx")
            .and_then(|ctx| ctx.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("data"))
            .and_then(|data| data.get("clientLibraryIntegrity"))
            .and_then(|val| val.as_str())
            .unwrap_or_default()
            .to_string();

        let response_body = CybersourceClientAuthResponse {
            capture_context: capture_context_jwt,
            client_library,
            client_library_integrity,
        };
        event_builder.map(|i| i.set_connector_response(&response_body));

        let response_router_data = ResponseRouterData {
            response: response_body,
            router_data: data.clone(),
            http_code: res.status_code,
        };

        let result = RouterDataV2::<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >::try_from(response_router_data)
        .map_err(|e| e.change_context(ConnectorError::response_handling_failed(res.status_code)))?;

        Ok(result)
    }
    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

// IncrementalAuthorization implementation — PATCH to
// /pts/v2/payments/{connector_transaction_id}.
// CyberSource's native incremental authorization uses PATCH on the original
// payment resource (not a sub-resource) with an `additionalAmount` delta.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cybersource,
    curl_request: Json(CybersourcePaymentsIncrementalAuthorizationRequest),
    curl_response: CybersourcePaymentsIncrementalAuthorizationResponse,
    flow_name: IncrementalAuthorization,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsIncrementalAuthorizationData,
    flow_response: PaymentsResponseData,
    http_method: Patch,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })
                .attach_printable("Missing connector_transaction_id for incremental authorization")?;
            Ok(format!(
                "{}pts/v2/payments/{}",
                self.connector_base_url_payments(req),
                connector_payment_id
            ))
        }
    }
);

// Manual implementation for MandateRevoke with correct event builder
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Cybersource<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }
    fn get_http_method(&self) -> Method {
        Method::Delete
    }
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }
    fn get_url(
        &self,
        req: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let connector_mandate_id = req
            .request
            .connector_mandate_id
            .clone()
            .ok_or_else(utils::missing_field_err("connector_mandate_id"))?;
        Ok(format!(
            "{}tms/v1/paymentinstruments/{}",
            self.connector_base_url_payments(req),
            connector_mandate_id.expose(),
        ))
    }
    fn get_request_body(
        &self,
        _req: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        Ok(None)
    }
    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
        ConnectorError,
    > {
        if matches!(res.status_code, 204) {
            event_builder.map(|i| i.set_connector_response(&serde_json::json!({"mandate_status": common_enums::MandateStatus::Revoked.to_string()})));
            Ok(RouterDataV2 {
                response: Ok(MandateRevokeResponseData {
                    mandate_status: common_enums::MandateStatus::Revoked,
                    status_code: res.status_code,
                }),
                ..data.clone()
            })
        } else {
            // If http_code != 204 || http_code != 4xx, we dont know any other response scenario yet.
            let response_value: serde_json::Value = serde_json::from_slice(&res.response)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    res.status_code,
                    "cybersource",
                ))?;
            let response_string = response_value.to_string();

            event_builder.map(|i| {
                i.set_connector_response(
                    &serde_json::json!({"response_string": response_string.clone()}),
                )
            });
            tracing::info!(connector_response=?response_string);

            Ok(RouterDataV2 {
                response: Err(ErrorResponse {
                    code: NO_ERROR_CODE.to_string(),
                    message: response_string.clone(),
                    reason: Some(response_string),
                    status_code: res.status_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                ..data.clone()
            })
        }
    }
    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

// FlowNotSupported markers — CyberSource does not expose endpoints for these flows

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "create_order".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "submit_evidence".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "defend_dispute".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "accept_dispute".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "server_session_authentication_token".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "tokenize_payment_method".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "server_authentication_token".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Cybersource<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(Report::new(IntegrationError::FlowNotSupported {
            flow: "create_customer".to_string(),
            connector: "Cybersource".to_string(),
            context: Default::default(),
        }))
    }
}

// SourceVerification implementations for all flows

// Authentication flow SourceVerification implementations
