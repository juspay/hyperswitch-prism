pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::FloatMajorUnit};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateAccessToken, CreateConnectorCustomer,
        CreateOrder, CreateSessionToken, DefendDispute, IncrementalAuthorization, MandateRevoke,
        PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SdkSessionToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorWebhookSecrets, DisputeDefendData, DisputeFlowData,
        DisputeResponseData, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, SessionTokenRequestData, SessionTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{self as cashtocode, CashtocodePaymentsRequest, CashtocodePaymentsResponse};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorResponseTransformationError;
use domain_types::errors::{IntegrationError, WebhookError};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Trait implementations with generic type parameters

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Cashtocode<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secrets: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let webhook_secret = match connector_webhook_secrets.clone() {
            Some(secrets) => secrets,
            None => return Ok(false),
        };

        let base64_signature = request
            .headers
            .get("authorization")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;

        let signature = base64_signature.as_bytes();

        let secret_auth = String::from_utf8(webhook_secret.secret.to_vec())
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Could not convert secret to UTF-8")?;
        let signature_auth = String::from_utf8(signature.to_vec())
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Could not convert secret to UTF-8")?;
        Ok(signature_auth == secret_auth)
    }

    fn get_event_type(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        Ok(domain_types::connector_types::EventType::PaymentIntentSuccess)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let webhook: transformers::CashtocodePaymentsSyncResponse = request
            .body
            .parse_struct("CashtocodePaymentsSyncResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(domain_types::connector_types::WebhookDetailsResponse {
            resource_id: Some(
                domain_types::connector_types::ResponseId::ConnectorTransactionId(
                    webhook.transaction_id.clone(),
                ),
            ),
            status: common_enums::AttemptStatus::Charged,
            status_code: 200,
            mandate_reference: None,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            response_headers: None,
            minor_amount_captured: None,
            amount_captured: None,
            error_reason: None,
            network_txn_id: None,
            payment_method_update: None,
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Cashtocode<T>
{
}

// Authentication trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Cashtocode,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: CashtocodePaymentsRequest,
            response_body: CashtocodePaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.cashtocode.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.cashtocode.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashtocode,
    curl_request: Json(CashtocodePaymentsRequest),
    curl_response: CashtocodePaymentsResponse,
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
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];

            let auth_type = transformers::CashtocodeAuth::try_from((
                &req.connector_config,
                &req.request.currency,
            ))?;

            let mut api_key = get_b64_auth_cashtocode(req.request.payment_method_type, &auth_type)?;

            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/merchant/paytokens", self.connector_base_url_payments(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Cashtocode<T>
{
    fn id(&self) -> &'static str {
        "cashtocode"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.cashtocode.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Cashtocode uses custom auth in get_headers
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
        let response: cashtocode::CashtocodeErrorResponse = res
            .response
            .parse_struct("CashtocodeErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "cashtocode: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error.to_string(),
            message: response.error_description.clone(),
            reason: Some(response.error_description),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// Stub implementations for unsupported flows
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
    for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for Cashtocode<T>
{
}

// Authentication flow implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Cashtocode<T>
{
}

//marker traits

// Duplicate create_all_prerequisites! call removed - already defined above

// Duplicate macro_connector_implementation! for Authorize flow removed - already defined above

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Cashtocode<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Cashtocode<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Cashtocode<T>
{
}

fn get_b64_auth_cashtocode(
    payment_method_type: Option<common_enums::PaymentMethodType>,
    auth_type: &transformers::CashtocodeAuth,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    fn construct_basic_auth(
        username: Option<Secret<String>>,
        password: Option<Secret<String>>,
    ) -> Result<Maskable<String>, IntegrationError> {
        let username = username.ok_or(IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        })?;
        let password = password.ok_or(IntegrationError::FailedToObtainAuthType {
            context: Default::default(),
        })?;
        Ok(format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!(
                "{}:{}",
                username.peek(),
                password.peek()
            ))
        )
        .into_masked())
    }

    let auth_header = match payment_method_type {
        Some(common_enums::PaymentMethodType::ClassicReward) => construct_basic_auth(
            auth_type.username_classic.to_owned(),
            auth_type.password_classic.to_owned(),
        ),
        Some(common_enums::PaymentMethodType::Evoucher) => construct_basic_auth(
            auth_type.username_evoucher.to_owned(),
            auth_type.password_evoucher.to_owned(),
        ),
        _ => {
            return Err(IntegrationError::MissingPaymentMethodType {
                context: Default::default(),
            })?
        }
    }?;

    Ok(vec![(headers::AUTHORIZATION.to_string(), auth_header)])
}
