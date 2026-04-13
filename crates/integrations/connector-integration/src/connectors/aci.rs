pub mod aci_result_codes;
pub mod transformers;

use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, StringMajorUnit};
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
        MandateReferenceId, MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
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

use common_enums::CurrencyUnit;
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    self as aci, AciCancelRequest, AciCaptureRequest, AciCaptureResponse, AciMandateRequest,
    AciMandateResponse, AciPaymentsRequest, AciPaymentsResponse,
    AciPaymentsResponse as AciPaymentsSyncResponse,
    AciPaymentsResponse as AciRepeatPaymentResponse, AciRefundRequest, AciRefundResponse,
    AciRepeatPaymentRequest, AciVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Trait implementations with generic type parameters
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Aci<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Aci,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Aci<T>
{
    fn id(&self) -> &'static str {
        "aci"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.aci.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = aci::AciAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: aci::AciErrorResponse = res
            .response
            .parse_struct("AciErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "aci: response body did not match the expected format; confirm API version and connector documentation."),
            )?;
        with_error_response_body!(event_builder, response);
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.result.code,
            message: response.result.description,
            reason: response.result.parameter_errors.map(|errors| {
                errors
                    .into_iter()
                    .map(|error_description| {
                        format!(
                            "Field is {} and the message is {}",
                            error_description.name, error_description.message
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("; ")
            }),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

macros::create_all_prerequisites!(
    connector_name: Aci,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AciPaymentsRequest<T>,
            response_body: AciPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: AciMandateRequest<T>,
            response_body: AciMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: AciPaymentsSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: AciCaptureRequest,
            response_body: AciCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AciCancelRequest,
            response_body: AciVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AciRefundRequest,
            response_body: AciRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: AciRepeatPaymentRequest<T>,
            response_body: AciRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
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
        {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            self.common_get_content_type().to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_config)?;
        header.append(&mut api_key);
        Ok(header)
        }
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.aci.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.aci.base_url
        }

        pub fn extract_mandate_id(
            &self,
            mandate_reference: &MandateReferenceId,
        ) -> CustomResult<String, IntegrationError> {
            match mandate_reference {
                MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
                    .get_connector_mandate_id()
                    .ok_or_else(|| {
                        error_stack::report!(IntegrationError::MissingRequiredField {
                            field_name: "connector_mandate_id",
                context: Default::default()
                        })
                    }),
                MandateReferenceId::NetworkMandateId(_) => {
                    Err(error_stack::report!(IntegrationError::not_implemented(
                        "Network mandate ID not supported for repeat payments in aci"
                            .to_string(),
                    )))
                }
                MandateReferenceId::NetworkTokenWithNTI(_) => {
                    Err(error_stack::report!(IntegrationError::not_implemented(
                        "Network token with NTI not supported for aci".to_string(),
                    )))
                }
            }
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Aci,
    curl_request: FormUrlEncoded(AciPaymentsRequest),
    curl_response: AciPaymentsResponse,
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
             Ok(format!("{}{}", self.connector_base_url_payments(req), "v1/payments"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Aci,
    curl_request: FormUrlEncoded(AciMandateRequest<T>),
    curl_response: AciMandateResponse,
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
            Ok(format!("{}v1/registrations", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Aci,
    curl_response: AciPaymentsSyncResponse,
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
        let auth = aci::AciAuthType::try_from(&req.connector_config)?;
        Ok(format!(
            "{}{}{}{}{}",
            self.connector_base_url_payments(req),
            "v1/payments/",
            req.request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?,
            "?entityId=",
            auth.entity_id.peek()
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Aci,
    curl_request: FormUrlEncoded(AciCaptureRequest),
    curl_response: AciCaptureResponse,
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
            Ok(format!(
            "{}{}{}",
            self.connector_base_url_payments(req),
            "v1/payments/",
            req.request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?,
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Aci,
    curl_request: FormUrlEncoded(AciCancelRequest),
    curl_response: AciVoidResponse,
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
            let id = &req.request.connector_transaction_id;
            Ok(format!("{}v1/payments/{}", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Aci,
    curl_request: FormUrlEncoded(AciRefundRequest),
    curl_response: AciRefundResponse,
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
            "{}v1/payments/{}",
            self.connector_base_url_refunds(req),
            connector_payment_id,
        ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Aci,
    curl_request: FormUrlEncoded(AciRepeatPaymentRequest<T>),
    curl_response: AciRepeatPaymentResponse,
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
            let mandate_id = self.extract_mandate_id(&req.request.mandate_reference)?;
             Ok(format!("{}v1/registrations/{}/payments",self.connector_base_url_payments(req),mandate_id))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData> for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Aci<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Aci<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Aci<T>
{
}
