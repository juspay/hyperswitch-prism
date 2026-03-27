pub mod transformers;
use super::macros;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::BytesExt,
    request::Method,
    types::StringMajorUnit,
};
use std::fmt::Debug;
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const UNAUTHORIZED_STATUS_CODE: u16 = 401;

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
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{Connectors, HasConnectors},
};

use base64::Engine;
use error_stack::{Report, ResultExt};
use ring::{digest, hmac};
use serde::Serialize;
use time::OffsetDateTime;

use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};

use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};

use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorResponseTransformationError;
use domain_types::errors::IntegrationError;
use transformers::{
    BankOfAmericaAuthType, BankOfAmericaPaymentsResponseForSetupMandate,
    BankOfAmericaPaymentsResponseForVoid, BankOfAmericaRefundRequestForRefund,
    BankOfAmericaRefundResponseForRefund, BankOfAmericaRsyncResponseForRSync,
    BankOfAmericaTransactionResponse, BankofamericaCaptureRequest, BankofamericaErrorResponse,
    BankofamericaPaymentsRequest, BankofamericaPaymentsRequestForSetupMandate,
    BankofamericaPaymentsResponse, BankofamericaPaymentsResponseForCapture,
    BankofamericaVoidRequestForVoid,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const CONNECTOR_UNAUTHORIZED_ERROR: &str = "Authentication Error from the connector";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Bankofamerica<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Bankofamerica<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Bankofamerica<T>
{
    fn id(&self) -> &'static str {
        "bankofamerica"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json;charset=utf-8"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.bankofamerica.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
        let response: Result<
            BankofamericaErrorResponse,
            Report<common_utils::errors::ParsingError>,
        > = res.response.parse_struct("Bankofamerica ErrorResponse");

        let error_message = if res.status_code == UNAUTHORIZED_STATUS_CODE {
            headers::CONNECTOR_UNAUTHORIZED_ERROR.to_string()
        } else {
            NO_ERROR_MESSAGE.to_string()
        };
        match response {
            Ok(BankofamericaErrorResponse::StandardError(response)) => {
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
                            error_info.reason.clone(),
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
                                .reason
                                .map_or(error_message.to_string(), |reason| reason.to_string()),
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
            Ok(BankofamericaErrorResponse::AuthenticationError(response)) => Ok(ErrorResponse {
                status_code: res.status_code,
                code: NO_ERROR_CODE.to_string(),
                message: response.response.rmsg.clone(),
                reason: Some(response.response.rmsg),
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            }),
            Err(error_msg) => {
                tracing::error!(deserialization_error =? error_msg);
                domain_types::utils::handle_json_response_deserialization_failure(
                    res,
                    "bankofamerica",
                )
            }
        }
    }
}

macros::create_all_prerequisites!(
    connector_name: Bankofamerica,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: BankofamericaPaymentsRequest<T>,
            response_body: BankofamericaPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: BankofamericaCaptureRequest,
            response_body: BankofamericaPaymentsResponseForCapture,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: BankOfAmericaTransactionResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: BankofamericaPaymentsRequestForSetupMandate<T>,
            response_body: BankOfAmericaPaymentsResponseForSetupMandate,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: BankofamericaVoidRequestForVoid,
            response_body: BankOfAmericaPaymentsResponseForVoid,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BankOfAmericaRefundRequestForRefund,
            response_body: BankOfAmericaRefundResponseForRefund,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: BankOfAmericaRsyncResponseForRSync,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
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
        let bankofamerica_req = self.get_request_body(req)?;
        let auth = BankOfAmericaAuthType::try_from(&req.connector_config)?;
        let merchant_account = auth.merchant_account.clone();
        let base_url = self.base_url(req.resource_common_data.connectors());
        let bankofamerica_host =
            url::Url::parse(base_url).change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
        let host = bankofamerica_host
            .host_str()
            .ok_or(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
        let url = self.get_url(req)?;
        let skip_len = base_url.len().saturating_sub(1);
        if url.len() <= skip_len {
            return Err(
                IntegrationError::InvalidDataFormat {
                    field_name: "url",
                    context: Default::default()
}
                .into(),
            );
        }
        let path: String = url.chars().skip(skip_len).collect();
        let sha256 = self.generate_digest(
            bankofamerica_req
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
            &req.resource_common_data.connectors.bankofamerica.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.bankofamerica.base_url
        }

        pub fn generate_digest(&self, payload: &[u8]) -> String {
        let payload_digest = digest::digest(&digest::SHA256, payload);
        BASE64_ENGINE.encode(payload_digest)
    }

    pub fn generate_signature(
        &self,
        auth: BankOfAmericaAuthType,
        host: String,
        resource: &str,
        payload: &String,
        date: OffsetDateTime,
        http_method: Method,
    ) -> CustomResult<String, IntegrationError> {
        let BankOfAmericaAuthType {
            api_key,
            merchant_account,
            api_secret
} = auth;
        let is_post_method = matches!(http_method, Method::Post);
        let digest_str = if is_post_method { "digest " } else { "" };
        let headers = format!("host date (request-target) {digest_str}v-c-merchant-id");
        let request_target = if is_post_method {
            format!("(request-target): post {resource}\ndigest: SHA-256={payload}\n")
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

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bankofamerica,
    curl_request: Json(BankofamericaPaymentsRequest<T>),
    curl_response: BankofamericaPaymentsResponse,
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
            "{}pts/v2/payments/",
            self.connector_base_url_payments(req)
        ))
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bankofamerica,
    curl_response: BankOfAmericaTransactionResponse,
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
            "{}tss/v2/transactions/{connector_payment_id}",
            self.connector_base_url_payments(req)
        ))
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bankofamerica,
    curl_request: Json(BankofamericaCaptureRequest),
    curl_response: BankofamericaPaymentsResponseForCapture,
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
            "{}pts/v2/payments/{connector_payment_id}/captures",
            self.connector_base_url_payments(req)
        ))
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bankofamerica,
    curl_request: Json(BankofamericaVoidRequestForVoid),
    curl_response: BankOfAmericaPaymentsResponseForVoid,
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
        let connector_payment_id = &req.request.connector_transaction_id;
        if connector_payment_id.is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID { context: Default::default() }.into());
        }
        Ok(format!(
            "{}pts/v2/payments/{connector_payment_id}/reversals",
            self.connector_base_url_payments(req)
        ))
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bankofamerica,
    curl_request: Json(BankofamericaPaymentsRequestForSetupMandate<T>),
    curl_response: BankOfAmericaPaymentsResponseForSetupMandate,
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
        _req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}pts/v2/payments/",
            self.connector_base_url_payments(_req)
        ))
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bankofamerica,
    curl_request: Json(BankOfAmericaRefundRequestForRefund),
    curl_response: BankOfAmericaRefundResponseForRefund,
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
        let connector_payment_id = &req.request.connector_transaction_id;
        if connector_payment_id.is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID { context: Default::default() }.into());
        }
        Ok(format!(
            "{}pts/v2/payments/{connector_payment_id}/refunds",
            self.connector_base_url_refunds(req)
        ))
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bankofamerica,
    curl_response: BankOfAmericaRsyncResponseForRSync,
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
            "{}tss/v2/transactions/{refund_id}",
            self.connector_base_url_refunds(req)
        ))
    }
    }
);
