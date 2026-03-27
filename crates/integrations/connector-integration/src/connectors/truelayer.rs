pub mod transformers;

use std::{self, fmt::Debug};

use base64::Engine;
use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateConnectorCustomer,
        CreateOrder, CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke,
        PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, VerifyWebhookSource, Void,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSdkSessionTokenData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, SessionTokenRequestData, SessionTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData, VerifyWebhookSourceFlowData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{Response, VerifyWebhookSourceResponseData},
    types::Connectors,
    utils::base64_decode,
};
use hyperswitch_masking::ExposeInterface;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use josekit::jws::{JwsHeader, ES512};
use serde::Serialize;
use std::collections::BTreeMap;
use transformers::{
    self as truelayer, TruelayerAccessTokenRequestData, TruelayerAccessTokenResponseData,
    TruelayerPSyncResponseData, TruelayerPaymentsRequestData, TruelayerPaymentsResponseData,
    TruelayerRefundRequest, TruelayerRefundResponse, TruelayerRsyncResponse,
    TruelayerVoidResponseData,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

// Trait for types that can provide access tokens
pub trait AccessTokenProvider {
    fn get_access_token(&self) -> CustomResult<String, IntegrationError>;
}

impl AccessTokenProvider for PaymentFlowData {
    fn get_access_token(&self) -> CustomResult<String, IntegrationError> {
        self.get_access_token()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })
    }
}

impl AccessTokenProvider for RefundFlowData {
    fn get_access_token(&self) -> CustomResult<String, IntegrationError> {
        self.get_access_token()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })
    }
}

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::errors::ConnectorResponseTransformationError;
use domain_types::errors::{IntegrationError, WebhookError};
use error_stack::ResultExt;

const TL_SIGNATURE: &str = "Tl-Signature";

// Trait implementations with generic type parameters

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Truelayer<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::VoidPC,
        PaymentFlowData,
        domain_types::connector_types::PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Truelayer<T>
{
}

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const IDEMPOTENCY_KEY: &str = "Idempotency-Key";
}

macros::create_all_prerequisites!(
    connector_name: Truelayer,
    generic_type: T,
    api: [
        (
            flow: CreateAccessToken,
            request_body: TruelayerAccessTokenRequestData,
            response_body: TruelayerAccessTokenResponseData,
            router_data: RouterDataV2<CreateAccessToken, PaymentFlowData, AccessTokenRequestData, AccessTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: TruelayerPaymentsRequestData,
            response_body: TruelayerPaymentsResponseData,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: TruelayerPSyncResponseData,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: TruelayerVoidResponseData,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TruelayerRefundRequest,
            response_body: TruelayerRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: TruelayerRsyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        fn normalize_path(self, path: &str) -> &str {
            path.trim_end_matches('/')
        }

        pub fn build_payload(
            self,
            method: String,
            path: &str,
            headers: &BTreeMap<String, String>,
            body: Option<&str>,
        ) -> String {
            let mut payload = format!("{} {}\n", method.to_uppercase(), self.normalize_path(path));

            for (k, v) in headers {
                payload.push_str(&format!("{}: {}\n", k, v));
            }

            if let Some(body_str) = body {
                payload.push_str(body_str);
            }

            payload
        }

        pub fn generate_tl_signature(
            self,
            method: String,
            path: &str,
            headers: &BTreeMap<String, String>,
            body: Option<&str>,
            private_key: String,
            kid: &str,
        ) -> CustomResult<String, IntegrationError> {

            let payload = self.build_payload(method, path, headers, body);
            let pem = base64_decode(private_key)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            let signer = ES512.signer_from_pem(&pem)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            let tl_headers = headers.keys().cloned().collect::<Vec<_>>().join(",");

            let mut header = JwsHeader::new();
            header.set_algorithm("ES512");
            header.set_key_id(kid);
            header.set_claim("tl_version", Some("2".into()))
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;
            header.set_claim("tl_headers", Some(tl_headers.into()))
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            let jws = josekit::jws::serialize_compact(
                payload.as_bytes(),
                &header,
                &signer,
            )
            .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            let parts: Vec<&str> = jws.split('.').collect();

            match (parts.first(), parts.get(2)) {
                (Some(first), Some(third)) => Ok(format!("{}..{}", first, third)),
                _ => Err(IntegrationError::RequestEncodingFailed { context: Default::default() }.into())
}
        }

        pub fn build_headers<F, FlowData, Req, Res>(
            &self,
            req: &RouterDataV2<F, FlowData, Req, Res>,
            access_token: String,
            private_key: String,
            kid: String,
            path: String,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            FlowData: AccessTokenProvider,
            Self: ConnectorIntegrationV2<F, FlowData, Req, Res>,
        {
            let idempotency_key = uuid::Uuid::new_v4().to_string();
            let truelayer_req = self
                .get_request_body(req)?
                .map(|req| req.get_inner_value().expose().clone());
            let http_method = self.get_http_method();

            let mut headers = BTreeMap::new();
            headers.insert(
                "Idempotency-Key".to_string(),
                idempotency_key.to_string(),
            );

            let body_json_str: Option<&str> = truelayer_req.as_deref();

            let tl_signature = self
                .clone()
                .generate_tl_signature(
                    http_method.to_string(),
                    path.as_str(),
                    &headers,
                    body_json_str,
                    private_key,
                    kid.as_str(),
                )
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            let header = vec![
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", access_token).into_masked(),
                ),
                (TL_SIGNATURE.to_string(), tl_signature.into_masked()),
                (
                    headers::IDEMPOTENCY_KEY.to_string(),
                    idempotency_key.into(),
                ),
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                )
            ];

            Ok(header)
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.truelayer.base_url.to_string()
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Truelayer<T>
{
    fn id(&self) -> &'static str {
        "truelayer"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json; charset=UTF-8"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.truelayer.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
        let response: truelayer::TruelayerErrorResponse = res
            .response
            .parse_struct("TruelayerErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "truelayer: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.title.clone(),
            message: response.title,
            reason: Some(response.detail),
            attempt_status: None,
            connector_transaction_id: Some(response.trace_id),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Truelayer,
    curl_request: Json(TruelayerPaymentsRequestData),
    curl_response: TruelayerPaymentsResponseData,
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
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            let metadata = truelayer::TruelayerMetadata::try_from(&req.connector_config)?;
            let private_key = metadata.private_key.expose().clone();
            let kid = metadata.kid.expose().clone();
            let path = "/v3/payments".to_string();
            self.build_headers(req, access_token.access_token.expose(), private_key, kid, path)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/v3/payments"))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
            self.build_error_response(res, event_builder)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Truelayer,
    curl_request: FormUrlEncoded(TruelayerAccessTokenRequestData),
    curl_response: TruelayerAccessTokenResponseData,
    flow_name: CreateAccessToken,
    resource_common_data: PaymentFlowData,
    flow_request: AccessTokenRequestData,
    flow_response: AccessTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<CreateAccessToken, PaymentFlowData, AccessTokenRequestData, AccessTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = req.resource_common_data.connectors.truelayer.secondary_base_url.as_ref()
                .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?;
            Ok(format!("{base_url}/connect/token"))
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<CreateAccessToken, PaymentFlowData, AccessTokenRequestData, AccessTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
            let response: truelayer::TruelayerAccessTokenErrorResponse = res
                .response
                .parse_struct("TruelayerAccessTokenErrorResponse")
                .change_context(crate::utils::response_deserialization_fail(res.status_code, "truelayer: response body did not match the expected format; confirm API version and connector documentation."))?;

            with_error_response_body!(event_builder, response);

            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.error,
                message: response.error_description.clone().unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                reason: response.error_details.clone().and_then(|details| details.reason),
                attempt_status: Some(AttemptStatus::Failure),
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None
})
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Truelayer,
    curl_response: TruelayerPSyncResponseData,
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
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", access_token.access_token.expose()).into_masked(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            let connector_payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!("{base_url}/v3/payments/{connector_payment_id}"))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
            self.build_error_response(res, event_builder)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Truelayer,
    curl_request: Json(TruelayerRefundRequest),
    curl_response: TruelayerRefundResponse,
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
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            let metadata = truelayer::TruelayerMetadata::try_from(&req.connector_config)?;
            let private_key = metadata.private_key.expose().clone();
            let kid = metadata.kid.expose().clone();
            let connector_payment_id = req.request.connector_transaction_id.clone();
            let path = format!("/v3/payments/{connector_payment_id}/refunds");
            self.build_headers(req, access_token.access_token.expose(), private_key, kid, path)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = req.resource_common_data.connectors.truelayer
                .base_url
                .to_string();
            let connector_payment_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{base_url}/v3/payments/{connector_payment_id}/refunds",
            ))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
            self.build_error_response(res, event_builder)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Truelayer,
    curl_response: TruelayerRsyncResponse,
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
            let access_token = req.resource_common_data
                .access_token
                .clone()
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", access_token.access_token.expose()).into_masked(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let refund_id = &req.request.connector_refund_id;
            let base_url = req.resource_common_data.connectors.truelayer
                .base_url
                .to_string();
            let transaction_id = &req.request.connector_transaction_id;
            if transaction_id.is_empty() {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                context: Default::default()
                }
                .into());
            }
            Ok(format!(
                "{base_url}/v3/payments/{transaction_id}/refunds/{refund_id}",
            ))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
            self.build_error_response(res, event_builder)
        }
    }
);

// Stub implementations for unsupported flows (required by macro system)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Truelayer<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyWebhookSourceV2 for Truelayer<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > for Truelayer<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let tl_signature_header = req.request.webhook_headers.get("tl-signature").ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "tl-signature",
                context: Default::default(),
            },
        )?;

        let tl_signature = tl_signature_header.as_str();
        let parts: Vec<&str> = tl_signature.splitn(3, '.').collect();
        let header_b64 = parts.first().ok_or(IntegrationError::InvalidDataFormat {
            field_name: "tl-signature",
            context: Default::default(),
        })?;
        let header_json = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(header_b64)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "tl-signature",
                context: Default::default(),
            })?;
        let jws_header: truelayer::JwsHeaderWebhooks = serde_json::from_slice(&header_json)
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "tl-signature",
                context: Default::default(),
            })?;

        let jku = jws_header
            .jku
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "jku",
                context: Default::default(),
            })?;

        if truelayer::ALLOWED_JKUS.contains(&jku.as_str()) {
            Ok(jku)
        } else {
            Err(IntegrationError::InvalidDataFormat {
                field_name: "jku",
                context: Default::default(),
            }
            .into())
        }
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            VerifyWebhookSource,
            VerifyWebhookSourceFlowData,
            VerifyWebhookSourceRequestData,
            VerifyWebhookSourceResponseData,
        >,
        ConnectorResponseTransformationError,
    > {
        let response: truelayer::Jwks =
            res.response.parse_struct("truelayer Jwks").change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "truelayer: response body did not match the expected format; confirm API version and connector documentation."),
            )?;
        if let Some(event) = event_builder {
            event.set_connector_response(&response)
        }

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(crate::utils::response_handling_fail_for_connector(
            res.status_code,
            "truelayer",
        ))
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
        self.build_error_response(res, event_builder)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Truelayer<T>
{
    fn get_event_type(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<domain_types::router_data::ConnectorSpecificConfig>,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        let webhook_body: truelayer::TruelayerWebhookEventTypeBody = request
            .body
            .parse_struct("TruelayerPayoutsWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(truelayer::get_webhook_event(webhook_body._type))
    }

    fn process_payment_webhook(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<domain_types::router_data::ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let request_body_copy = request.body.clone();
        let details: truelayer::TruelayerWebhookBody = request
            .body
            .parse_struct("TruelayerWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let status = truelayer::get_truelayer_payment_webhook_status(details._type)?;

        let (error_code, error_message, error_reason) = if status == AttemptStatus::Failure {
            (
                details.failure_reason.clone(),
                details.failure_reason.clone(),
                details.failure_reason.clone(),
            )
        } else {
            (None, None, None)
        };

        Ok(domain_types::connector_types::WebhookDetailsResponse {
            resource_id: Some(
                domain_types::connector_types::ResponseId::ConnectorTransactionId(
                    details.payment_id.clone(),
                ),
            ),
            status,
            connector_response_reference_id: None,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<domain_types::router_data::ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::RefundWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let request_body_copy = request.body.clone();
        let details: truelayer::TruelayerWebhookBody = request
            .body
            .parse_struct("TruelayerWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let status = truelayer::get_truelayer_refund_webhook_status(details._type)?;

        let (error_code, error_message) = if status == RefundStatus::Failure {
            (
                details.failure_reason.clone(),
                details.failure_reason.clone(),
            )
        } else {
            (None, None)
        };

        Ok(
            domain_types::connector_types::RefundWebhookDetailsResponse {
                connector_refund_id: details.refund_id.clone(),
                status,
                connector_response_reference_id: details.refund_id.clone(),
                error_code,
                error_message,
                raw_connector_response: Some(
                    String::from_utf8_lossy(&request_body_copy).to_string(),
                ),
                status_code: 200,
                response_headers: None,
            },
        )
    }

    fn get_webhook_resource_object(
        &self,
        request: domain_types::connector_types::RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let details: truelayer::TruelayerWebhookBody = request
            .body
            .parse_struct("TruelayerWebhooksBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(Box::new(details))
    }
}
