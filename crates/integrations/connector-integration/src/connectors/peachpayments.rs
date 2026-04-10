pub mod requests;
pub mod responses;
pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::{ByteSliceExt, StringExt},
};
use domain_types::{
    connector_flow::{self, Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::*,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{Response, VerifyWebhookSourceResponseData},
    types::Connectors,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use requests::{
    PeachpaymentsAuthorizeRequest, PeachpaymentsCaptureRequest, PeachpaymentsRefundRequest,
    PeachpaymentsVoidRequest,
};
use responses::{
    PeachpaymentsCaptureResponse, PeachpaymentsPaymentsResponse, PeachpaymentsRefundResponse,
    PeachpaymentsRefundSyncResponse, PeachpaymentsSyncResponse, PeachpaymentsVoidResponse,
};
use serde::Serialize;

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

const REFUND: &str = "Refund";
const TRANSACTION: &str = "transaction";

macros::create_all_prerequisites!(
    connector_name: Peachpayments,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PeachpaymentsAuthorizeRequest<T>,
            response_body: PeachpaymentsPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PeachpaymentsCaptureRequest,
            response_body: PeachpaymentsCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PeachpaymentsVoidRequest,
            response_body: PeachpaymentsVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PeachpaymentsRefundRequest,
            response_body: PeachpaymentsRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: PeachpaymentsRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PeachpaymentsSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                Self::common_get_content_type(self).to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.peachpayments.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.peachpayments.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Peachpayments,
    curl_request: Json(PeachpaymentsAuthorizeRequest),
    curl_response: PeachpaymentsPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>) -> CustomResult<String, IntegrationError> {
            match req.request.capture_method {
                Some(common_enums::CaptureMethod::Automatic) => Ok(format!(
                    "{}/transactions/create-and-confirm",
                    self.connector_base_url_payments(req)
                )),
                Some(common_enums::CaptureMethod::Manual) => Ok(format!(
                    "{}/transactions/authorization",
                    self.connector_base_url_payments(req)
                )),
                _ => Err(IntegrationError::CaptureMethodNotSupported { context: Default::default() }.into())
}
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Peachpayments,
    curl_request: Json(PeachpaymentsCaptureRequest),
    curl_response: PeachpaymentsCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req.request.connector_transaction_id.get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!("{}/transactions/authorization/{}/capture", self.connector_base_url_payments(req), connector_payment_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Peachpayments,
    curl_request: Json(PeachpaymentsVoidRequest),
    curl_response: PeachpaymentsVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/transactions/authorization/{}/reverse", self.connector_base_url_payments(req), connector_payment_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Peachpayments,
    curl_request: Json(PeachpaymentsRefundRequest),
    curl_response: PeachpaymentsRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<String, IntegrationError> {
            let connector_transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/transactions/{}/refund", self.connector_base_url_refunds(req), connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Peachpayments,
    curl_response: PeachpaymentsSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>) -> CustomResult<String, IntegrationError> {
            let reference_id = req.resource_common_data.connector_request_reference_id.clone();
            Ok(format!("{}/transactions/by-reference/{}", self.connector_base_url_payments(req), reference_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Peachpayments,
    curl_response: PeachpaymentsRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(&self, req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>) -> CustomResult<String, IntegrationError> {
            let reference_id = req.resource_common_data.connector_request_reference_id.clone();
            Ok(format!("{}/transactions/by-reference/{}", self.connector_base_url_refunds(req), reference_id))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Peachpayments<T>
{
    fn id(&self) -> &'static str {
        "peachpayments"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.peachpayments.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::PeachpaymentsAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![
            ("x-api-key".to_string(), auth.api_key.expose().into()),
            ("x-tenant-id".to_string(), auth.tenant_id.expose().into()),
            ("x-exi-auth-ver".to_string(), "v1".to_string().into()),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: responses::PeachpaymentsErrorResponse = res
            .response
            .parse_struct("PeachpaymentsErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "peachpayments: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_ref.clone(),
            message: response.message,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// BODY DECODING IMPLEMENTATION
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Peachpayments<T>
{
}

// Main service trait - aggregates all other traits
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Peachpayments<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Peachpayments,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyWebhookSourceV2 for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::Accept,
        DisputeFlowData,
        AcceptDisputeData,
        DisputeResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::DefendDispute,
        DisputeFlowData,
        DisputeDefendData,
        DisputeResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::SubmitEvidence,
        DisputeFlowData,
        SubmitEvidenceData,
        DisputeResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        connector_flow::VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Peachpayments<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Peachpayments<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body = String::from_utf8(request.body.clone())
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_body: responses::PeachpaymentsIncomingWebhook = body
            .parse_struct("PeachpaymentsIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let description = webhook_body
            .transaction
            .as_ref()
            .map(|transaction| transaction.transaction_type.description.clone());

        match webhook_body.webhook_type.as_str() {
            TRANSACTION => {
                if let Some(transaction) = webhook_body.transaction {
                    match transaction.transaction_result {
                        responses::PeachpaymentsPaymentStatus::Successful => {
                            Ok(EventType::PaymentIntentSuccess)
                        }
                        responses::PeachpaymentsPaymentStatus::ApprovedConfirmed => {
                            if description == Some(REFUND.to_string()) {
                                Ok(EventType::RefundSuccess)
                            } else {
                                Ok(EventType::PaymentIntentSuccess)
                            }
                        }
                        responses::PeachpaymentsPaymentStatus::Authorized
                        | responses::PeachpaymentsPaymentStatus::Approved => {
                            Ok(EventType::PaymentIntentAuthorizationSuccess)
                        }
                        responses::PeachpaymentsPaymentStatus::Pending => {
                            Ok(EventType::PaymentIntentProcessing)
                        }
                        responses::PeachpaymentsPaymentStatus::Declined
                        | responses::PeachpaymentsPaymentStatus::Failed => {
                            if description == Some(REFUND.to_string()) {
                                Ok(EventType::RefundFailure)
                            } else {
                                Ok(EventType::PaymentIntentFailure)
                            }
                        }
                        responses::PeachpaymentsPaymentStatus::Voided
                        | responses::PeachpaymentsPaymentStatus::Reversed => {
                            Ok(EventType::PaymentIntentCancelled)
                        }
                        responses::PeachpaymentsPaymentStatus::ThreedsRequired => {
                            Ok(EventType::PaymentActionRequired)
                        }
                    }
                } else {
                    Err(report!(WebhookError::WebhookEventTypeNotFound))
                }
            }
            _ => Err(report!(WebhookError::WebhookEventTypeNotFound)),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = String::from_utf8(request.body.clone())
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_body: responses::PeachpaymentsIncomingWebhook = body
            .parse_struct("PeachpaymentsIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let transaction = webhook_body
            .transaction
            .ok_or_else(|| report!(WebhookError::WebhookResourceObjectNotFound))?;

        let status: common_enums::AttemptStatus = transaction.transaction_result.clone().into();

        let (error_code, error_message, error_reason) =
            if status == common_enums::AttemptStatus::Failure {
                (
                    Some(transformers::get_error_code(
                        transaction.response_code.as_ref(),
                    )),
                    Some(transformers::get_error_message(
                        transaction.response_code.as_ref(),
                    )),
                    transaction.error_message.clone(),
                )
            } else {
                (None, None, None)
            };

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                transaction.transaction_id.clone(),
            )),
            status,
            connector_response_reference_id: Some(transaction.reference_id),
            mandate_reference: None,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = String::from_utf8(request.body.clone())
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_body: responses::PeachpaymentsIncomingWebhook = body
            .parse_struct("PeachpaymentsIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let transaction = webhook_body
            .transaction
            .ok_or_else(|| report!(WebhookError::WebhookResourceObjectNotFound))?;

        let refund_status: common_enums::RefundStatus = match transaction.transaction_result {
            responses::PeachpaymentsPaymentStatus::ApprovedConfirmed
            | responses::PeachpaymentsPaymentStatus::Successful => {
                common_enums::RefundStatus::Success
            }
            _ => common_enums::RefundStatus::Failure,
        };

        let (error_code, error_message) = if refund_status == common_enums::RefundStatus::Failure {
            (
                Some(transformers::get_error_code(
                    transaction.response_code.as_ref(),
                )),
                Some(transformers::get_error_message(
                    transaction.response_code.as_ref(),
                )),
            )
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(transaction.transaction_id),
            status: refund_status,
            connector_response_reference_id: Some(transaction.reference_id),
            error_code,
            error_message,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let body = String::from_utf8(request.body.clone())
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_body: responses::PeachpaymentsIncomingWebhook = body
            .parse_struct("PeachpaymentsIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(Box::new(webhook_body))
    }
}
