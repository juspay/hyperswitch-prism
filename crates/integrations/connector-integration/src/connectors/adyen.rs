mod test;
pub mod transformers;
use std::{
    fmt::Debug,
    marker::{Send, Sync},
    sync::LazyLock,
};

use base64::Engine;
use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, EventClass, PaymentMethod, PaymentMethodType,
};
use common_utils::{
    consts,
    crypto::{self, SignMessage},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    pii::SecretSerdeValue,
    types::StringMinorUnit,
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
        ConnectorCustomerResponse, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, DisputeWebhookReference,
        EventContext, MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData, SupportedPaymentMethodsExt,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        self, CardSpecificFeatures, ConnectorInfo, Connectors, FeatureStatus,
        PaymentMethodDataType, PaymentMethodDetails, PaymentMethodSpecificFeatures,
        SupportedPaymentMethods,
    },
    utils,
};
use error_stack::{report, ResultExt};
use hex;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, is_mandate_supported, ConnectorValidation},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as adyen, AdyenCaptureRequest, AdyenCaptureResponse, AdyenClientAuthRequest,
    AdyenClientAuthResponse, AdyenDefendDisputeRequest, AdyenDefendDisputeResponse,
    AdyenDisputeAcceptRequest, AdyenDisputeAcceptResponse, AdyenDisputeSubmitEvidenceRequest,
    AdyenIncrementalAuthRequest, AdyenIncrementalAuthResponse, AdyenNotificationRequestItemWH,
    AdyenOrderCreateRequest, AdyenOrderCreateResponse, AdyenPSyncResponse, AdyenPaymentRequest,
    AdyenPaymentResponse, AdyenRedirectRequest, AdyenRefundRequest, AdyenRefundResponse,
    AdyenRepeatPaymentRequest, AdyenRepeatPaymentResponse, AdyenSubmitEvidenceResponse,
    AdyenVoidRequest, AdyenVoidResponse, SetupMandateRequest, SetupMandateResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::IntegrationErrorContext;
use domain_types::errors::WebhookError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_API_KEY: &str = "X-Api-Key";
}

// Type alias for non-generic trait implementations

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Adyen<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Adyen<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Adyen,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AdyenPaymentRequest<T>,
            response_body: AdyenPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: AdyenRedirectRequest,
            response_body: AdyenPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: AdyenCaptureRequest,
            response_body: AdyenCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AdyenVoidRequest,
            response_body: AdyenVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AdyenRefundRequest,
            response_body: AdyenRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: SetupMandateRequest<T>,
            response_body: SetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: AdyenRepeatPaymentRequest,
            response_body: AdyenRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: Accept,
            request_body: AdyenDisputeAcceptRequest,
            response_body: AdyenDisputeAcceptResponse,
            router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,

        ),
        (
            flow: SubmitEvidence,
            request_body: AdyenDisputeSubmitEvidenceRequest,
            response_body: AdyenSubmitEvidenceResponse,
            router_data: RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,

        ),
        (
            flow: DefendDispute,
            request_body: AdyenDefendDisputeRequest,
            response_body: AdyenDefendDisputeResponse,
            router_data: RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: AdyenClientAuthRequest,
            response_body: AdyenClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: AdyenOrderCreateRequest,
            response_body: AdyenOrderCreateResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: IncrementalAuthorization,
            request_body: AdyenIncrementalAuthRequest,
            response_body: AdyenIncrementalAuthResponse,
            router_data: RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter_webhooks: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self
                .get_auth_header(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.adyen.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.adyen.base_url
        }

        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> Option<&'a str> {
            req.resource_common_data.connectors.adyen.dispute_base_url.as_deref()
        }
    }
);

macros::macro_connector_payout_implementation!(
    connector: Adyen,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

fn build_env_specific_endpoint(
    base_url: &str,
    test_mode: Option<bool>,
    connector_config: &ConnectorSpecificConfig,
) -> CustomResult<String, IntegrationError> {
    if test_mode.unwrap_or(true) {
        Ok(base_url.to_string())
    } else {
        let endpoint_prefix = match connector_config {
            ConnectorSpecificConfig::Adyen {
                endpoint_prefix, ..
            } => endpoint_prefix.as_deref(),
            _ => None,
        }
        .ok_or(IntegrationError::InvalidConnectorConfig {
            config: "endpoint_prefix",
            context: Default::default(),
        })?;
        Ok(base_url.replace("{{merchant_endpoint_prefix}}", endpoint_prefix))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Adyen<T>
{
    fn id(&self) -> &'static str {
        "adyen"
    }
    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = adyen::AdyenAuthType::try_from(auth_type).map_err(|_| {
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
        })?;
        Ok(vec![(
            headers::X_API_KEY.to_string(),
            auth.api_key.into_masked(),
        )])
    }
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.adyen.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: adyen::AdyenErrorResponse =
            res.response.parse_struct("ErrorResponse").map_err(|_| {
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "adyen: response body did not match the expected format; confirm API version and connector documentation.")
            })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code,
            message: response.message.to_owned(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: response.psp_reference,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

const ADYEN_API_VERSION: &str = "v68";
const ADYEN_AMOUNT_UPDATES_DOC_URL: &str =
    "https://docs.adyen.com/api-explorer/Checkout/68/post/payments/-paymentPspReference-/amountUpdates";

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenPaymentRequest),
    curl_response: AdyenPaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize ],
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
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments"))
        }
        fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenRedirectRequest),
    curl_response: AdyenPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize ],
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
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/details"))
        }
        fn build_request_v2(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
            // For wallet redirects, encoded_data may be None
            // In such cases, gracefully skip the psync request
            if req.request.encoded_data.clone().is_some() {
                // Build the request normally if encoded_data is present
                let url = self.get_url(req)?;
                let headers = self.get_headers(req)?;
                let body = ConnectorIntegrationV2::<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>::get_request_body(self, req)?;

                Ok(Some(
                    common_utils::request::RequestBuilder::new()
                        .method(common_utils::request::Method::Post)
                        .url(&url)
                        .attach_default_headers()
                        .headers(headers)
                        .set_optional_body(body)
                        .build(),
                ))
            } else {
                // For wallet redirects without encoded_data, return None
                // This allows the system to rely on webhooks for payment status
                Ok(None)
            }
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenCaptureRequest),
    curl_response: AdyenCaptureResponse,
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
            let id = match &req.request.connector_transaction_id {
                ResponseId::ConnectorTransactionId(id) => id,
                _ => {
                    return Err(
                        IntegrationError::MissingConnectorTransactionID { context: Default::default() }.into(),
                    )
                }
            };
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/{id}/captures"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenIncrementalAuthRequest),
    curl_response: AdyenIncrementalAuthResponse,
    flow_name: IncrementalAuthorization,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsIncrementalAuthorizationData,
    flow_response: PaymentsResponseData,
    http_method: Post,
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
            let id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Propagate the original authorization's connector_transaction_id \
                             (Adyen pspReference) to the IncrementalAuthorization request."
                                .to_string(),
                        ),
                        doc_url: Some(ADYEN_AMOUNT_UPDATES_DOC_URL.to_string()),
                        additional_context: Some(
                            "connector_transaction_id is required as the paymentPspReference path \
                             segment for the /amountUpdates endpoint."
                                .to_string(),
                        ),
                    },
                })?;
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/{id}/amountUpdates"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Adyen<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenOrderCreateRequest),
    curl_response: AdyenOrderCreateResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/orders"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenVoidRequest),
    curl_response: AdyenVoidResponse,
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
            let id = req.request.connector_transaction_id.clone();
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments/{id}/cancels"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDefendDisputeRequest),
    curl_response: AdyenDefendDisputeResponse,
    flow_name: DefendDispute,
    resource_common_data: DisputeFlowData,
    flow_request: DisputeDefendData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TODO: Add build_env_specific_endpoint when DisputeFlowData has test_mode and connector_feature_data fields
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?;
            Ok(format!("{dispute_url}ca/services/DisputeService/v30/defendDispute"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Adyen<T>
{
}

impl<
        T: PaymentMethodDataTypes
            + Debug
            + std::marker::Sync
            + std::marker::Send
            + 'static
            + Serialize,
    >
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Adyen<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Adyen<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body.clone()).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let hmac_signature = notif
            .additional_data
            .hmac_signature
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Missing hmacSignature in Adyen webhook additional_data")?;

        Ok(hmac_signature.as_bytes().to_vec())
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body.clone()).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        // Adyen HMAC message format: pspReference:originalReference:merchantAccountCode:merchantReference:amount.value:amount.currency:eventCode:success
        let message = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            notif.psp_reference,
            notif.original_reference.as_deref().unwrap_or(""),
            notif.merchant_account_code,
            notif.merchant_reference,
            notif.amount.value,
            notif.amount.currency,
            notif.event_code,
            notif.success
        );

        Ok(message.into_bytes())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Adyen uses HMAC-SHA256
        let algorithm = crypto::HmacSha256;

        let connector_webhook_secrets = connector_webhook_secret
            .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))
            .attach_printable("Missing webhook secret for Adyen verification")?;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;

        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        // Adyen webhook secret is hex-encoded, need to decode it
        let raw_key = hex::decode(&connector_webhook_secrets.secret)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to decode hex webhook secret for Adyen")?;

        // Compute HMAC-SHA256 signature
        let computed_signature = algorithm
            .sign_message(&raw_key, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to compute HMAC signature for Adyen")?;

        // Base64 encode the computed signature
        let computed_signature_b64 = consts::BASE64_ENGINE.encode(&computed_signature);

        // Adyen sends base64-encoded signature as string, compare base64 strings
        let received_signature_str = std::str::from_utf8(&signature)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to parse received signature as UTF-8")?;

        Ok(computed_signature_b64 == received_signature_str)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"notificationItems":[{"NotificationRequestItem":{"pspReference":"probe_ref_001","merchantReference":"probe_order_001","merchantAccountCode":"ProbeAccount","eventCode":"AUTHORISATION","success":"true","amount":{"currency":"USD","value":1000},"additionalData":{}}}]}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;
        transformers::get_adyen_webhook_event_type(notif.event_code).map_err(|e| report!(e))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        use transformers::WebhookEventCode;

        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let reference = match notif.event_code {
            // Capture/cancellation/adjustment events: psp_reference is the event's own PSP ref;
            // original_reference is the parent authorisation's PSP ref — that's the lookup key.
            WebhookEventCode::Capture
            | WebhookEventCode::CaptureFailed
            | WebhookEventCode::Cancellation
            | WebhookEventCode::AuthorisationAdjustment => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: notif.original_reference,
                    merchant_transaction_id: Some(notif.merchant_reference),
                })
            }
            // Authorisation and OfferClosed: psp_reference is the payment PSP ref.
            WebhookEventCode::Authorisation
            | WebhookEventCode::OfferClosed
            | WebhookEventCode::RecurringContract => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(notif.psp_reference),
                    merchant_transaction_id: Some(notif.merchant_reference),
                })
            }
            // Refund events: psp_reference is the refund's own PSP ref;
            // original_reference is the parent payment's PSP ref.
            WebhookEventCode::Refund
            | WebhookEventCode::CancelOrRefund
            | WebhookEventCode::RefundFailed
            | WebhookEventCode::RefundReversed => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(notif.psp_reference),
                    merchant_refund_id: Some(notif.merchant_reference),
                    connector_transaction_id: notif.original_reference,
                })
            }
            // Dispute events: psp_reference is the dispute ID; original_reference is the parent payment.
            WebhookEventCode::NotificationOfChargeback
            | WebhookEventCode::Chargeback
            | WebhookEventCode::ChargebackReversed
            | WebhookEventCode::PrearbitrationWon
            | WebhookEventCode::SecondChargeback
            | WebhookEventCode::PrearbitrationLost => {
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(notif.psp_reference),
                    connector_transaction_id: notif.original_reference,
                })
            }
            // Unknown: no actionable reference.
            WebhookEventCode::Unknown => return Ok(None),
        };

        Ok(Some(reference))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let (error_code, error_message, error_reason) =
            if transformers::get_adyen_payment_webhook_event(
                notif.event_code.clone(),
                notif.success.clone(),
            )? == AttemptStatus::Failure
            {
                (
                    notif.reason.clone(),
                    notif.reason.clone(),
                    notif.reason.clone(),
                )
            } else {
                (None, None, None)
            };

        let mandate_reference = transformers::get_adyen_mandate_reference_from_webhook(&notif);
        let network_txn_id = transformers::get_adyen_network_txn_id_from_webhook(&notif);
        let payment_method_update =
            transformers::get_adyen_payment_method_update_from_webhook(&notif);

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                notif.psp_reference.clone(),
            )),
            status: transformers::get_adyen_payment_webhook_event(notif.event_code, notif.success)?,
            connector_response_reference_id: Some(notif.psp_reference),
            error_code,
            mandate_reference,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
            minor_amount_captured: None,
            amount_captured: None,
            error_reason,
            network_txn_id,
            payment_method_update,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let (error_code, error_message) = if transformers::get_adyen_refund_webhook_event(
            notif.event_code.clone(),
            notif.success.clone(),
        )? == common_enums::RefundStatus::Failure
        {
            (notif.reason.clone(), notif.reason.clone())
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(notif.psp_reference.clone()),
            status: transformers::get_adyen_refund_webhook_event(notif.event_code, notif.success)?,
            connector_response_reference_id: Some(notif.psp_reference.clone()),
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request_body_copy).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::DisputeWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let request_body_copy = request.body.clone();
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;
        let (stage, status) = transformers::get_dispute_stage_and_status(
            notif.event_code,
            notif.additional_data.dispute_status,
        );

        let amount = utils::convert_amount_for_webhook(
            self.amount_converter_webhooks,
            notif.amount.value,
            notif.amount.currency,
        )?;

        Ok(
            domain_types::connector_types::DisputeWebhookDetailsResponse {
                amount,
                currency: notif.amount.currency,
                dispute_id: notif.psp_reference.clone(),
                stage,
                status,
                connector_response_reference_id: Some(notif.psp_reference.clone()),
                dispute_message: notif.reason,
                connector_reason_code: notif.additional_data.chargeback_reason_code,
                raw_connector_response: Some(
                    String::from_utf8_lossy(&request_body_copy).to_string(),
                ),
                status_code: 200,
                response_headers: None,
            },
        )
    }

    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
    ) -> Result<
        interfaces::api::ApplicationResponse<serde_json::Value>,
        error_stack::Report<WebhookError>,
    > {
        Ok(interfaces::api::ApplicationResponse::TextPlain(
            "[accepted]".to_string(),
        ))
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenRefundRequest),
    curl_response: AdyenRefundResponse,
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

            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_refunds(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!(
                "{endpoint}{ADYEN_API_VERSION}/payments/{connector_payment_id}/refunds",
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(SetupMandateRequest),
    curl_response: SetupMandateResponse,
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
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDisputeAcceptRequest),
    curl_response: AdyenDisputeAcceptResponse,
    flow_name: Accept,
    resource_common_data: DisputeFlowData,
    flow_request: AcceptDisputeData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TODO: Add build_env_specific_endpoint when DisputeFlowData has test_mode and connector_feature_data fields
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?;
            Ok(format!("{dispute_url}ca/services/DisputeService/v30/acceptDispute"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDisputeSubmitEvidenceRequest),
    curl_response: AdyenSubmitEvidenceResponse,
    flow_name: SubmitEvidence,
    resource_common_data: DisputeFlowData,
    flow_request: SubmitEvidenceData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // TODO: Add build_env_specific_endpoint when DisputeFlowData has test_mode and connector_feature_data fields
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(IntegrationError::FailedToObtainIntegrationUrl { context: Default::default() })?;
            Ok(format!("{dispute_url}ca/services/DisputeService/v30/supplyDefenseDocument"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenRepeatPaymentRequest),
    curl_response: AdyenRepeatPaymentResponse,
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
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/payments"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenClientAuthRequest),
    curl_response: AdyenClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = build_env_specific_endpoint(
                self.connector_base_url_payments(req),
                req.resource_common_data.test_mode,
                &req.connector_config,
            )?;
            Ok(format!("{endpoint}{ADYEN_API_VERSION}/sessions"))
        }
    }
);

static ADYEN_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> = LazyLock::new(|| {
    let adyen_supported_capture_methods = vec![
        CaptureMethod::Automatic,
        CaptureMethod::Manual,
        CaptureMethod::ManualMultiple,
        // CaptureMethod::Scheduled,
    ];

    let adyen_supported_card_network = vec![
        CardNetwork::AmericanExpress,
        CardNetwork::CartesBancaires,
        CardNetwork::UnionPay,
        CardNetwork::DinersClub,
        CardNetwork::Discover,
        CardNetwork::Interac,
        CardNetwork::JCB,
        CardNetwork::Maestro,
        CardNetwork::Mastercard,
        CardNetwork::Visa,
    ];

    let mut adyen_supported_payment_methods = SupportedPaymentMethods::new();

    adyen_supported_payment_methods.add(
        PaymentMethod::Card,
        PaymentMethodType::Card,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: Some(PaymentMethodSpecificFeatures::Card(CardSpecificFeatures {
                three_ds: FeatureStatus::Supported,
                no_three_ds: FeatureStatus::Supported,
                supported_card_networks: adyen_supported_card_network.clone(),
            })),
        },
    );

    // Bank Debit - ACH
    adyen_supported_payment_methods.add(
        PaymentMethod::BankDebit,
        PaymentMethodType::Ach,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Bank Debit - SEPA
    adyen_supported_payment_methods.add(
        PaymentMethod::BankDebit,
        PaymentMethodType::Sepa,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    // Bank Debit - BACS
    adyen_supported_payment_methods.add(
        PaymentMethod::BankDebit,
        PaymentMethodType::Bacs,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );

    adyen_supported_payment_methods
});

static ADYEN_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Adyen", 
    description: "Adyen is a Dutch payment company with the status of an acquiring bank that allows businesses to accept e-commerce, mobile, and point-of-sale payments. It is listed on the stock exchange Euronext Amsterdam.",
    connector_type: types::PaymentConnectorCategory::PaymentGateway
};

static ADYEN_SUPPORTED_WEBHOOK_FLOWS: &[EventClass] = &[EventClass::Payments, EventClass::Refunds];

impl ConnectorSpecifications for Adyen<DefaultPCIHolder> {
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&ADYEN_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&ADYEN_SUPPORTED_PAYMENT_METHODS)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [EventClass]> {
        Some(ADYEN_SUPPORTED_WEBHOOK_FLOWS)
    }
}

impl ConnectorValidation for Adyen<DefaultPCIHolder> {
    fn validate_mandate_payment(
        &self,
        pm_type: Option<PaymentMethodType>,
        pm_data: PaymentMethodData<DefaultPCIHolder>,
    ) -> CustomResult<(), IntegrationError> {
        let mandate_supported_pmd = std::collections::HashSet::from([
            PaymentMethodDataType::Card,
            PaymentMethodDataType::AchBankDebit,
            PaymentMethodDataType::SepaBankDebit,
            PaymentMethodDataType::BecsBankDebit,
        ]);
        is_mandate_supported(pm_data, pm_type, mandate_supported_pmd, self.id())
    }

    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _is_three_ds: bool,
        _status: AttemptStatus,
        _connector_feature_data: Option<SecretSerdeValue>,
    ) -> CustomResult<(), IntegrationError> {
        if data.encoded_data.is_some() {
            return Ok(());
        }
        Err(IntegrationError::MissingRequiredField {
            field_name: "encoded_data",
            context: Default::default(),
        }
        .into())
    }
    fn is_webhook_source_verification_mandatory(&self) -> bool {
        false
    }
}

impl<
        T: PaymentMethodDataTypes
            + Debug
            + std::marker::Sync
            + std::marker::Send
            + 'static
            + Serialize,
    >
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Adyen<T>
{
}
