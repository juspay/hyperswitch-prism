mod requests;
mod responses;
pub mod transformers;

use requests::*;
use responses::*;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{consts, errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, CreateAccessToken, CreateOrder, CreateSessionToken,
        DefendDispute, IncrementalAuthorization, MandateRevoke, PSync, RSync, Refund,
        RepeatPayment, SdkSessionToken, SetupMandate, SubmitEvidence, Void,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, DisputeDefendData,
        DisputeFlowData, DisputeResponseData, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsResponseData, PaymentsSdkSessionTokenData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        SessionTokenRequestData, SessionTokenResponseData, SetupMandateRequestData,
        SubmitEvidenceData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as jpmorgan;

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const REQUEST_ID: &str = "Request-Id";
    pub(crate) const MERCHANT_ID: &str = "Merchant-Id";
}

// Trait to abstract over PaymentFlowData and RefundFlowData for header building
pub trait JpmorganResourceData {
    fn access_token(&self) -> Option<&AccessTokenResponseData>;
    fn connector_request_reference_id(&self) -> String;
    fn merchant_id(&self) -> &common_utils::id_type::MerchantId;
    fn connectors(&self) -> &Connectors;
}

impl JpmorganResourceData for PaymentFlowData {
    fn access_token(&self) -> Option<&AccessTokenResponseData> {
        self.access_token.as_ref()
    }

    fn connector_request_reference_id(&self) -> String {
        self.connector_request_reference_id.clone()
    }

    fn merchant_id(&self) -> &common_utils::id_type::MerchantId {
        &self.merchant_id
    }

    fn connectors(&self) -> &Connectors {
        &self.connectors
    }
}

impl JpmorganResourceData for RefundFlowData {
    fn access_token(&self) -> Option<&AccessTokenResponseData> {
        self.access_token.as_ref()
    }

    fn connector_request_reference_id(&self) -> String {
        self.connector_request_reference_id.clone()
    }

    fn merchant_id(&self) -> &common_utils::id_type::MerchantId {
        &self.merchant_id
    }

    fn connectors(&self) -> &Connectors {
        &self.connectors
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Jpmorgan<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Jpmorgan<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Jpmorgan, amount_type: MinorUnit);
macros::create_all_prerequisites!(
    connector_name: Jpmorgan,
    generic_type: T,
    api: [
        (
            flow: CreateAccessToken,
            request_body: JpmorganTokenRequest,
            response_body: JpmorganAuthUpdateResponse,
            router_data: RouterDataV2<CreateAccessToken, PaymentFlowData, AccessTokenRequestData, AccessTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: JpmorganPaymentsRequest<T>,
            response_body: JpmorganPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: JpmorganPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: JpmorganCaptureRequest,
            response_body: JpmorganCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: JpmorganVoidRequest,
            response_body: JpmorganVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: JpmorganRefundRequest,
            response_body: JpmorganRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: JpmorganRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        // Generic header builder that works for both PaymentFlowData and RefundFlowData
        pub fn build_headers<F, ResourceData, Req, Res>(
            &self,
            req: &RouterDataV2<F, ResourceData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError>
        where
            Self: ConnectorIntegrationV2<F, ResourceData, Req, Res>,
            ResourceData: JpmorganResourceData,
        {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                Self::common_get_content_type(self).to_string().into(),
            )];

            // OAuth 2.0 Bearer token from access_token
            let auth_header = (
                headers::AUTHORIZATION.to_string(),
                format!(
                    "Bearer {}",
                    &req.resource_common_data
                        .access_token()
                        .ok_or(errors::ConnectorError::FailedToObtainAuthType)?
                        .access_token.peek()
                )
                .into_masked(),
            );

            // Request-Id header
            let request_id_header = (
                headers::REQUEST_ID.to_string(),
                req.resource_common_data.connector_request_reference_id().into_masked(),
            );

            // Merchant-Id header
            let merchant_id_header = (
                headers::MERCHANT_ID.to_string(),
                req.resource_common_data.merchant_id().get_string_repr().to_string().into_masked(),
            );

            headers.push(auth_header);
            headers.push(request_id_header);
            headers.push(merchant_id_header);

            Ok(headers)
        }

        // Generic base URL getter that works for both PaymentFlowData and RefundFlowData
        pub fn connector_base_url<'a, F, ResourceData, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, ResourceData, Req, Res>,
        ) -> &'a str
        where
            ResourceData: JpmorganResourceData,
        {
            &req.resource_common_data.connectors().jpmorgan.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Jpmorgan<T>
{
    fn id(&self) -> &'static str {
        "jpmorgan"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.jpmorgan.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: JpmorganErrorResponse = res
            .response
            .parse_struct("JpmorganErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

        with_error_response_body!(event_builder, response);

        let response_message = response
            .response_message
            .as_ref()
            .map_or_else(|| consts::NO_ERROR_MESSAGE.to_string(), ToString::to_string);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.response_code,
            message: response_message.clone(),
            reason: Some(response_message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PostAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::Authenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PreAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::VoidPC,
        PaymentFlowData,
        domain_types::connector_types::PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PaymentMethodToken,
        PaymentFlowData,
        domain_types::connector_types::PaymentMethodTokenizationData<T>,
        domain_types::connector_types::PaymentMethodTokenResponse,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        domain_types::connector_types::ConnectorCustomerData,
        domain_types::connector_types::ConnectorCustomerResponse,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Jpmorgan<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Jpmorgan,
    curl_request: FormUrlEncoded(JpmorganTokenRequest),
    curl_response: JpmorganAuthUpdateResponse,
    flow_name: CreateAccessToken,
    resource_common_data: PaymentFlowData,
    flow_request: AccessTokenRequestData,
    flow_response: AccessTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateAccessToken, PaymentFlowData, AccessTokenRequestData, AccessTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            let auth = jpmorgan::JpmorganAuthType::try_from(&req.connector_config)?;
            let creds = format!("{}:{}", auth.client_id.peek(), auth.client_secret.peek());
            let encoded_creds = BASE64_ENGINE.encode(creds);
            let auth_string = format!("Basic {}", encoded_creds);

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/x-www-form-urlencoded".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    auth_string.into_masked(),
                ),
            ])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateAccessToken, PaymentFlowData, AccessTokenRequestData, AccessTokenResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}/am/oauth2/alpha/access_token",
                req.resource_common_data.connectors.jpmorgan.secondary_base_url.as_ref()
                    .ok_or(errors::ConnectorError::FailedToObtainIntegrationUrl)?
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Jpmorgan,
    curl_request: Json(JpmorganPaymentsRequest<T>),
    curl_response: JpmorganPaymentsResponse,
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
            Ok(format!("{}/payments", self.connector_base_url(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Jpmorgan,
    curl_response: JpmorganPSyncResponse,
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
            let transaction_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;
            Ok(format!("{}/payments/{}", self.connector_base_url(req), transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Jpmorgan,
    curl_request: Json(JpmorganCaptureRequest),
    curl_response: JpmorganCaptureResponse,
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
            let transaction_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(errors::ConnectorError::MissingConnectorTransactionID)?;
            Ok(format!("{}/payments/{}/captures", self.connector_base_url(req), transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Jpmorgan,
    curl_request: Json(JpmorganVoidRequest),
    curl_response: JpmorganVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Patch,
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
            Ok(format!("{}/payments/{}", self.connector_base_url(req), req.request.connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Jpmorgan,
    curl_request: Json(JpmorganRefundRequest),
    curl_response: JpmorganRefundResponse,
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
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!("{}/refunds", self.connector_base_url(_req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Jpmorgan,
    curl_response: JpmorganRSyncResponse,
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
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/refunds/{}", self.connector_base_url(req), refund_id))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Jpmorgan<T>
{
}
