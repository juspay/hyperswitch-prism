pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::StringMinorUnit,
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
        ConnectorCustomerResponse, ConnectorWebhookSecrets, DisputeDefendData, DisputeFlowData,
        DisputeResponseData, DisputeWebhookDetailsResponse, EventType, MandateRevokeRequestData,
        MandateRevokeResponseData, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSdkSessionTokenData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, SessionTokenRequestData, SessionTokenResponseData, SetupMandateRequestData,
        SubmitEvidenceData, WebhookDetailsResponse,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
    utils::{self, ForeignTryFrom},
};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as novalnet, NovalnetCancelRequest, NovalnetCancelResponse, NovalnetCaptureRequest,
    NovalnetCaptureResponse, NovalnetPSyncResponse, NovalnetPaymentsRequest,
    NovalnetPaymentsRequest as NovalnetPaymentsRequestMandate,
    NovalnetPaymentsRequest as NovalnetRepeatPaymentsRequest, NovalnetPaymentsResponse,
    NovalnetPaymentsResponse as NovalnetPaymentsResponseMandate,
    NovalnetPaymentsResponse as NovalnetRepeatPaymentsResponse, NovalnetRefundRequest,
    NovalnetRefundResponse, NovalnetRefundSyncResponse, NovalnetSyncRequest,
    NovalnetSyncRequest as NovalnetRSyncRequest,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use error_stack::ResultExt;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_NN_ACCESS_KEY: &str = "X-NN-Access-Key";
}

// Trait implementations with generic type parameters

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SdkSessionTokenV2 for Novalnet<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Novalnet,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Novalnet<T>
{
}

// Authentication trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Novalnet<T>
{
}
macros::create_all_prerequisites!(
    connector_name: Novalnet,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: NovalnetPaymentsRequest<T>,
            response_body: NovalnetPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: NovalnetSyncRequest,
            response_body: NovalnetPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: NovalnetCaptureRequest,
            response_body: NovalnetCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: NovalnetRefundRequest,
            response_body: NovalnetRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: NovalnetRSyncRequest,
            response_body: NovalnetRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: NovalnetCancelRequest,
            response_body: NovalnetCancelResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: NovalnetPaymentsRequestMandate<T>,
            response_body: NovalnetPaymentsResponseMandate,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: NovalnetRepeatPaymentsRequest<T>,
            response_body: NovalnetRepeatPaymentsResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.novalnet.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.novalnet.base_url
        }
    }
);

// Stub implementation for CreateSessionToken

// After adding the ConnectorIntegrationV2 implementation, we can now implement PaymentSessionToken
// Type alias for non-generic trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Novalnet<T>
{
    fn id(&self) -> &'static str {
        "novalnet"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.novalnet.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
        let auth = novalnet::NovalnetAuthType::try_from(auth_type)
            .change_context(errors::ConnectorError::FailedToObtainAuthType)?;
        let api_key: String = auth.payment_access_key.expose();
        let encoded_api_key = BASE64_ENGINE.encode(api_key);
        Ok(vec![(
            headers::X_NN_ACCESS_KEY.to_string(),
            encoded_api_key.into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: novalnet::NovalnetErrorResponse = res
            .response
            .parse_struct("NovalnetErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code,
            message: response.message,
            reason: response.reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetPaymentsRequest),
    curl_response: NovalnetPaymentsResponse,
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
            let url = if req.request.is_auto_capture()? {
                format!("{}/payment",self.connector_base_url_payments(req))
            }
            else {
                format!("{}/authorize",self.connector_base_url_payments(req))
            };

            Ok(url)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetSyncRequest),
    curl_response: NovalnetPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
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
            Ok(format!(
                "{}/transaction/details",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetCaptureRequest),
    curl_response: NovalnetCaptureResponse,
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
            Ok(format!(
                "{}/transaction/capture",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetRefundRequest),
    curl_response: NovalnetRefundResponse,
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
            Ok(format!(
                "{}/transaction/refund",
                self.connector_base_url_refunds(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetSyncRequest),
    curl_response: NovalnetRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
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
            Ok(format!(
                "{}/transaction/details",
                self.connector_base_url_refunds(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetCancelRequest),
    curl_response: NovalnetCancelResponse,
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
            Ok(format!("{}/transaction/cancel", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetPaymentsRequest),
    curl_response: NovalnetPaymentsResponse,
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
            Ok(format!("{}/payment", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Novalnet,
    curl_request: Json(NovalnetRepeatPaymentsRequest),
    curl_response: NovalnetRepeatPaymentsResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let url = if req.request.is_auto_capture()? {
                format!("{}/payment",self.connector_base_url_payments(req))
            }
            else {
                format!("{}/authorize",self.connector_base_url_payments(req))
            };

            Ok(url)
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Novalnet<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        let notif_item = get_webhook_object_from_body(&request.body)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        hex::decode(notif_item.event.checksum)
            .change_context(errors::ConnectorError::WebhookVerificationSecretInvalid)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        let notif = get_webhook_object_from_body(&request.body)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        let (amount, currency) = match notif.transaction {
            novalnet::NovalnetWebhookTransactionData::CaptureTransactionData(data) => {
                (data.amount, data.currency)
            }
            novalnet::NovalnetWebhookTransactionData::CancelTransactionData(data) => {
                (data.amount, data.currency)
            }

            novalnet::NovalnetWebhookTransactionData::RefundsTransactionData(data) => {
                (data.amount, data.currency)
            }

            novalnet::NovalnetWebhookTransactionData::SyncTransactionData(data) => {
                (data.amount, data.currency)
            }
        };
        let amount = amount
            .map(|amount| amount.to_string())
            .unwrap_or("".to_string());
        let currency = currency
            .map(|amount| amount.to_string())
            .unwrap_or("".to_string());

        let secret_auth = String::from_utf8(connector_webhook_secrets.secret.to_vec())
            .change_context(errors::ConnectorError::WebhookVerificationSecretInvalid)
            .attach_printable("Could not convert webhook secret auth to UTF-8")?;
        let reversed_secret_auth = novalnet::reverse_string(&secret_auth);

        let message = format!(
            "{}{}{}{}{}{}",
            notif.event.tid,
            notif.event.event_type,
            notif.result.status,
            amount,
            currency,
            reversed_secret_auth
        );

        Ok(message.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::ConnectorError>> {
        let algorithm = crypto::Sha256;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => {
                tracing::warn!(
                    target: "novalnet_webhook",
                    "Missing webhook secret for Novalnet webhook verification - verification failed but continuing processing"
                );
                return Ok(false);
            }
        };

        let signature = match self
            .get_webhook_source_verification_signature(&request, &connector_webhook_secrets)
        {
            Ok(sig) => sig,
            Err(error) => {
                tracing::warn!(
                    target: "novalnet_webhook",
                    "Failed to get webhook source verification signature for Novalnet: {} - verification failed but continuing processing",
                    error
                );
                return Ok(false);
            }
        };

        let message = match self
            .get_webhook_source_verification_message(&request, &connector_webhook_secrets)
        {
            Ok(msg) => msg,
            Err(error) => {
                tracing::warn!(
                    target: "novalnet_webhook",
                    "Failed to get webhook source verification message for Novalnet: {} - verification failed but continuing processing",
                    error
                );
                return Ok(false);
            }
        };

        match algorithm.verify_signature(&connector_webhook_secrets.secret, &signature, &message) {
            Ok(is_verified) => Ok(is_verified),
            Err(error) => {
                tracing::warn!(
                    target: "novalnet_webhook",
                    "Failed to verify webhook signature for Novalnet: {} - verification failed but continuing processing",
                    error
                );
                Ok(false)
            }
        }
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
        let notif = get_webhook_object_from_body(&request.body)
            .change_context(errors::ConnectorError::WebhookEventTypeNotFound)?;

        let optional_transaction_status = match notif.transaction {
            novalnet::NovalnetWebhookTransactionData::CaptureTransactionData(data) => {
                Some(data.status)
            }
            novalnet::NovalnetWebhookTransactionData::CancelTransactionData(data) => data.status,
            novalnet::NovalnetWebhookTransactionData::RefundsTransactionData(data) => {
                Some(data.status)
            }
            novalnet::NovalnetWebhookTransactionData::SyncTransactionData(data) => {
                Some(data.status)
            }
        };

        let transaction_status =
            optional_transaction_status.ok_or(errors::ConnectorError::MissingRequiredField {
                field_name: "transaction_status",
            })?;
        // NOTE: transaction_status will always be present for Webhooks
        // But we are handling optional type here, since we are reusing TransactionData Struct from NovalnetPaymentsResponseTransactionData for Webhooks response too
        // In NovalnetPaymentsResponseTransactionData, transaction_status is optional

        let incoming_webhook_event =
            novalnet::get_incoming_webhook_event(notif.event.event_type, transaction_status);
        Ok(incoming_webhook_event)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        let notif = get_webhook_object_from_body(&request.body)
            .change_context(errors::ConnectorError::WebhookReferenceIdNotFound)?;

        let response = WebhookDetailsResponse::try_from(notif)
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        let notif: novalnet::NovalnetWebhookNotificationResponseRefunds = request
            .body
            .parse_struct("NovalnetWebhookNotificationResponse")
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

        let response = RefundWebhookDetailsResponse::try_from(notif)
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        let notif: transformers::NovalnetWebhookNotificationResponse =
            get_webhook_object_from_body(&request.body)
                .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;
        let (amount, currency, reason, reason_code) = match notif.transaction {
            novalnet::NovalnetWebhookTransactionData::CaptureTransactionData(data) => {
                (data.amount, data.currency, None, None)
            }
            novalnet::NovalnetWebhookTransactionData::CancelTransactionData(data) => {
                (data.amount, data.currency, None, None)
            }

            novalnet::NovalnetWebhookTransactionData::RefundsTransactionData(data) => {
                (data.amount, data.currency, None, None)
            }

            novalnet::NovalnetWebhookTransactionData::SyncTransactionData(data) => {
                (data.amount, data.currency, data.reason, data.reason_code)
            }
        };

        let dispute_status = novalnet::get_novalnet_dispute_status(notif.event.event_type);

        Ok(DisputeWebhookDetailsResponse {
            amount: utils::convert_amount(
                self.amount_converter,
                amount.ok_or(errors::ConnectorError::AmountConversionFailed)?,
                novalnet::option_to_result(currency)?,
            )?,
            currency: novalnet::option_to_result(currency)?,
            stage: common_enums::DisputeStage::Dispute,
            dispute_id: notif.event.tid.to_string(),
            connector_reason_code: reason_code,
            status: common_enums::DisputeStatus::foreign_try_from(dispute_status)?,
            connector_response_reference_id: None,
            dispute_message: reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }
}

fn get_webhook_object_from_body(
    body: &[u8],
) -> CustomResult<novalnet::NovalnetWebhookNotificationResponse, errors::ConnectorError> {
    let novalnet_webhook_notification_response = body
        .parse_struct("NovalnetWebhookNotificationResponse")
        .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

    Ok(novalnet_webhook_notification_response)
}

// Stub implementations for unsupported flows
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateSessionToken,
        PaymentFlowData,
        SessionTokenRequestData,
        SessionTokenResponseData,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateAccessToken,
        PaymentFlowData,
        AccessTokenRequestData,
        AccessTokenResponseData,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for Novalnet<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Novalnet<T>
{
}

// Authentication flow implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SdkSessionToken,
        PaymentFlowData,
        PaymentsSdkSessionTokenData,
        PaymentsResponseData,
    > for Novalnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Novalnet<T>
{
}
