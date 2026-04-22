mod requests;
mod responses;
pub mod transformers;

use requests::{JpmorganClientAuthRequest, *};
use responses::{JpmorganClientAuthResponse, *};

use std::fmt::Debug;

use base64::Engine;
use bytes::Bytes;
use common_enums::CurrencyUnit;
use common_utils::{consts, errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, ClientAuthenticationToken, CreateOrder, DefendDispute,
        IncrementalAuthorization, MandateRevoke, PSync, RSync, Refund, RepeatPayment,
        ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence,
        Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, DisputeDefendData,
        DisputeFlowData, DisputeResponseData, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
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
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const REQUEST_ID: &str = "Request-Id";
    pub(crate) const MERCHANT_ID: &str = "Merchant-Id";
}

fn jpmorgan_flow_not_supported(flow: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::FlowNotSupported {
        flow: flow.to_string(),
        connector: "Jpmorgan".to_string(),
        context: Default::default(),
    })
}
fn jpmorgan_not_implemented(flow: &str) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::not_implemented(format!(
        "{flow} flow for jpmorgan"
    )))
}

// Trait to abstract over PaymentFlowData and RefundFlowData for header building
pub trait JpmorganResourceData {
    fn access_token(&self) -> Option<&ServerAuthenticationTokenResponseData>;
    fn connector_request_reference_id(&self) -> String;
    fn merchant_id(&self) -> &common_utils::id_type::MerchantId;
    fn connectors(&self) -> &Connectors;
}

impl JpmorganResourceData for PaymentFlowData {
    fn access_token(&self) -> Option<&ServerAuthenticationTokenResponseData> {
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
    fn access_token(&self) -> Option<&ServerAuthenticationTokenResponseData> {
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
    fn get_url(
        &self,
        _req: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("incremental_authorization"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Jpmorgan<T>
{
}
macros::macro_connector_payout_implementation!(
    connector: Jpmorgan,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);
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
    connector_types::ServerSessionAuthentication for Jpmorgan<T>
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
    connector_types::ServerAuthentication for Jpmorgan<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Jpmorgan<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Jpmorgan, amount_type: MinorUnit);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> Jpmorgan<T> {
    /// JPMorgan returns malformed JSON for wallet payments where expiry fields are empty
    /// e.g. `"month": ,` instead of `"month": null`. This sanitizes those cases.
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _req: &RouterDataV2<F, FCD, Req, Res>,
        response_bytes: Bytes,
        _status_code: u16,
    ) -> Result<Bytes, ConnectorError> {
        let raw = String::from_utf8(response_bytes.to_vec()).map_err(|_| {
            ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            }
        })?;
        // Replace patterns like `": ,` and `": \r\n` (empty JSON values) with `": null`
        let sanitized = regex_replace_empty_json_values(&raw);
        Ok(Bytes::from(sanitized))
    }
}

/// Replace bare empty values in JSON (e.g. `"key": ,` or `"key": \n`) with `"key": null`.
fn regex_replace_empty_json_values(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let Some(&c) = chars.get(i) else { break };
        result.push(c);
        // After a colon, check if the next non-whitespace char is `,` or `}`
        if c == ':' {
            let mut j = i + 1;
            // consume whitespace
            while chars
                .get(j)
                .is_some_and(|&ch| matches!(ch, ' ' | '\t' | '\r' | '\n'))
            {
                j += 1;
            }
            if chars.get(j).is_some_and(|&ch| ch == ',' || ch == '}') {
                // empty value — insert null and advance past the whitespace we consumed
                result.push_str(" null");
                i = j; // will push chars[j] on next iteration
                continue;
            }
        }
        i += 1;
    }
    result
}

macros::create_all_prerequisites!(
    connector_name: Jpmorgan,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: JpmorganTokenRequest,
            response_body: JpmorganAuthUpdateResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
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
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: JpmorganClientAuthRequest,
            response_body: JpmorganClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        // Generic header builder that works for both PaymentFlowData and RefundFlowData
        pub fn build_headers<F, ResourceData, Req, Res>(
            &self,
            req: &RouterDataV2<F, ResourceData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
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
                        .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?
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
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: JpmorganErrorResponse = res
            .response
            .parse_struct("JpmorganErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "jpmorgan: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

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
    fn get_url(
        &self,
        _req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("create_order"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SubmitEvidence,
            DisputeFlowData,
            SubmitEvidenceData,
            DisputeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("submit_evidence"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("defend_dispute"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("accept_dispute"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("repeat_payment"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerSessionAuthenticationToken,
            PaymentFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented(
            "create_server_session_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PostAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::PostAuthenticate,
            PaymentFlowData,
            domain_types::connector_types::PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("post_authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::Authenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::Authenticate,
            PaymentFlowData,
            domain_types::connector_types::PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PreAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::PreAuthenticate,
            PaymentFlowData,
            domain_types::connector_types::PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("pre_authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_flow_not_supported("void_post_capture"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PaymentMethodToken,
        PaymentFlowData,
        domain_types::connector_types::PaymentMethodTokenizationData<T>,
        domain_types::connector_types::PaymentMethodTokenResponse,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::PaymentMethodToken,
            PaymentFlowData,
            domain_types::connector_types::PaymentMethodTokenizationData<T>,
            domain_types::connector_types::PaymentMethodTokenResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("payment_method_token"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        domain_types::connector_types::ConnectorCustomerData,
        domain_types::connector_types::ConnectorCustomerResponse,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            domain_types::connector_flow::CreateConnectorCustomer,
            PaymentFlowData,
            domain_types::connector_types::ConnectorCustomerData,
            domain_types::connector_types::ConnectorCustomerResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("create_connector_customer"))
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Jpmorgan,
    curl_request: FormUrlEncoded(JpmorganClientAuthRequest),
    curl_response: JpmorganClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // ClientAuthenticationToken flow uses Basic auth with client_id:client_secret
            // to obtain an OAuth2 access token for client-side SDK initialization
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
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            use domain_types::errors::IntegrationErrorContext;
            Ok(format!(
                "{}/am/oauth2/alpha/access_token",
                req.resource_common_data.connectors.jpmorgan.secondary_base_url.as_ref()
                    .ok_or(IntegrationError::FailedToObtainIntegrationUrl {
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Set the 'secondary_base_url' in the JPMorgan connector \
                                 configuration. This URL points to the OAuth2 token endpoint."
                                    .to_owned(),
                            ),
                            doc_url: Some(
                                "https://developer.payments.jpmorgan.com/docs/commerce-solutions/online-payments/capabilities/authentication/oauth"
                                    .to_owned(),
                            ),
                            additional_context: Some(
                                "JPMorgan uses a separate base URL for the OAuth2 token \
                                 endpoint (secondary_base_url) distinct from the payments API."
                                    .to_owned(),
                            ),
                        },
                    })?
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("mandate_revoke"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Jpmorgan<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(jpmorgan_not_implemented("setup_mandate"))
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Jpmorgan,
    curl_request: FormUrlEncoded(JpmorganTokenRequest),
    curl_response: JpmorganAuthUpdateResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            "application/x-www-form-urlencoded"
        }
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
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
            req: &RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/am/oauth2/alpha/access_token",
                req.resource_common_data.connectors.jpmorgan.secondary_base_url.as_ref()
                    .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?
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
    preprocess_response: true,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
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
