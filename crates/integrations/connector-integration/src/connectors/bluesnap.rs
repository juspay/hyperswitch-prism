mod requests;
mod responses;
pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder,
        DefendDispute, IncrementalAuthorization, MandateRevoke, PSync, PaymentMethodToken,
        PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment, ServerAuthenticationToken,
        ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence, Void, VoidPC,
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
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as bluesnap, BluesnapAuthorizeRequest, BluesnapAuthorizeResponse, BluesnapCaptureRequest,
    BluesnapCaptureResponse, BluesnapClientAuthRequest, BluesnapClientAuthResponse,
    BluesnapPSyncResponse, BluesnapRefundRequest, BluesnapRefundResponse,
    BluesnapRefundSyncResponse, BluesnapVoidRequest, BluesnapVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

macros::create_amount_converter_wrapper!(connector_name: Bluesnap, amount_type: StringMajorUnit);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
// Main service trait - aggregates all other traits

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Bluesnap<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Bluesnap<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Bluesnap,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Bluesnap<T>
{
}

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Bluesnap<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Bluesnap<T>
{
}

// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Bluesnap<T>
{
}

// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Bluesnap<T>
{
}

// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Bluesnap<T>
{
    fn verify_webhook_source(
        &self,
        request: domain_types::connector_types::RequestDetails,
        connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> CustomResult<bool, WebhookError> {
        let connector_webhook_secret = connector_webhook_secret
            .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))
            .attach_printable("Connector webhook secret not configured")?;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secret)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secret)?;

        use common_utils::crypto::{HmacSha256, SignMessage};
        HmacSha256
            .sign_message(&connector_webhook_secret.secret, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to sign webhook message with HMAC-SHA256")
            .map(|expected_signature| expected_signature.eq(&signature))
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: &domain_types::connector_types::ConnectorWebhookSecrets,
    ) -> CustomResult<Vec<u8>, WebhookError> {
        let signature_str = request
            .headers
            .get("bls-signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;

        hex::decode(signature_str).change_context(WebhookError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: &domain_types::connector_types::ConnectorWebhookSecrets,
    ) -> CustomResult<Vec<u8>, WebhookError> {
        let timestamp = request
            .headers
            .get("bls-ipn-timestamp")
            .ok_or_else(|| report!(WebhookError::WebhookSourceVerificationFailed))?;

        let body_str = String::from_utf8_lossy(&request.body);

        Ok(format!("{timestamp}{body_str}").into_bytes())
    }

    fn get_event_type(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> CustomResult<domain_types::connector_types::EventType, WebhookError> {
        match serde_urlencoded::from_bytes::<transformers::BluesnapWebhookBody>(&request.body) {
            Ok(webhook_body) => match webhook_body.transaction_type {
                transformers::BluesnapWebhookEvent::Chargeback
                | transformers::BluesnapWebhookEvent::ChargebackStatusChanged => {
                    let dispute_body: transformers::BluesnapDisputeWebhookBody =
                        serde_urlencoded::from_bytes(&request.body)
                            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

                    transformers::map_chargeback_status_to_event_type(&dispute_body.cb_status)
                }
                _ => Ok(transformers::map_webhook_event_to_incoming_webhook_event(
                    &webhook_body.transaction_type,
                )),
            },
            Err(_) => {
                let dispute_body: transformers::BluesnapDisputeWebhookBody =
                    serde_urlencoded::from_bytes(&request.body)
                        .change_context(WebhookError::WebhookBodyDecodingFailed)?;

                transformers::map_chargeback_status_to_event_type(&dispute_body.cb_status)
            }
        }
    }

    fn process_payment_webhook(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> CustomResult<domain_types::connector_types::WebhookDetailsResponse, WebhookError> {
        let webhook_body: transformers::BluesnapWebhookBody =
            serde_urlencoded::from_bytes(&request.body)
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let status = match webhook_body.transaction_type {
            transformers::BluesnapWebhookEvent::Decline
            | transformers::BluesnapWebhookEvent::CcChargeFailed => {
                common_enums::AttemptStatus::Failure
            }
            transformers::BluesnapWebhookEvent::Charge => common_enums::AttemptStatus::Charged,
            _ => common_enums::AttemptStatus::Pending,
        };

        let resource_id = if !webhook_body.merchant_transaction_id.is_empty() {
            Some(domain_types::connector_types::ResponseId::EncodedData(
                webhook_body.merchant_transaction_id,
            ))
        } else if !webhook_body.reference_number.is_empty() {
            Some(
                domain_types::connector_types::ResponseId::ConnectorTransactionId(
                    webhook_body.reference_number,
                ),
            )
        } else {
            None
        };

        Ok(domain_types::connector_types::WebhookDetailsResponse {
            resource_id,
            status,
            connector_response_reference_id: None,
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: None,
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
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> CustomResult<domain_types::connector_types::RefundWebhookDetailsResponse, WebhookError>
    {
        let webhook_body: transformers::BluesnapWebhookBody =
            serde_urlencoded::from_bytes(&request.body)
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let connector_refund_id = webhook_body
            .reversal_ref_num
            .ok_or_else(|| report!(WebhookError::WebhookReferenceIdNotFound))?;

        Ok(
            domain_types::connector_types::RefundWebhookDetailsResponse {
                connector_refund_id: Some(connector_refund_id),
                status: common_enums::RefundStatus::Success,
                connector_response_reference_id: None,
                error_code: None,
                error_message: None,
                raw_connector_response: None,
                status_code: 200,
                response_headers: None,
            },
        )
    }
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Bluesnap<T>
{
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Bluesnap<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Bluesnap,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: BluesnapAuthorizeRequest,
            response_body: BluesnapAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: BluesnapCaptureRequest,
            response_body: BluesnapCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: BluesnapPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BluesnapRefundRequest,
            response_body: BluesnapRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: BluesnapRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: BluesnapVoidRequest,
            response_body: BluesnapVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: BluesnapClientAuthRequest,
            response_body: BluesnapClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                "Content-Type".to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.bluesnap.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.bluesnap.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bluesnap,
    curl_request: Json(BluesnapAuthorizeRequest),
    curl_response: BluesnapAuthorizeResponse,
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
            use domain_types::payment_method_data::PaymentMethodData;

            let base_url = self.connector_base_url_payments(req);

            match &req.request.payment_method_data {
                PaymentMethodData::BankDebit(_) => {
                    // ACH uses alt-transactions endpoint
                    Ok(format!("{}/services/2/alt-transactions", base_url))
                },
                _ => {
                    // Cards and wallets use standard transactions endpoint
                    Ok(format!("{}/services/2/transactions", base_url))
                }
}
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bluesnap,
    curl_response: BluesnapPSyncResponse,
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
            let connector_tx_id = match &req.request.connector_transaction_id {
                domain_types::connector_types::ResponseId::ConnectorTransactionId(id) => id.clone(),
                _ => return Err(IntegrationError::MissingConnectorTransactionID { context: Default::default() }.into()),
            };
            let base_url = self.connector_base_url_payments(req);
            // Bank debit (ACH/SEPA) uses alt-transactions endpoint for retrieval.
            // The Authorize response stores {"is_alt_transaction": true} in connector_metadata,
            // which flows to PSync as connector_feature_data.
            let is_alt = req
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .and_then(|data| data.peek().get("is_alt_transaction"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let endpoint = if is_alt { "alt-transactions" } else { "transactions" };
            Ok(format!("{}/services/2/{}/{}", base_url, endpoint, connector_tx_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bluesnap,
    curl_request: Json(BluesnapCaptureRequest),
    curl_response: BluesnapCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Put,
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
            Ok(format!("{}/services/2/transactions", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bluesnap,
    curl_request: Json(BluesnapRefundRequest),
    curl_response: BluesnapRefundResponse,
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
            let connector_transaction_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}/services/2/transactions/refund/{}",
                self.connector_base_url_refunds(req),
                connector_transaction_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bluesnap,
    curl_response: BluesnapRefundSyncResponse,
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
            let refund_id = &req.request.connector_refund_id;
            Ok(format!(
                "{}/services/2/transactions/{}",
                self.connector_base_url_refunds(req),
                refund_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Bluesnap,
    curl_request: Json(BluesnapVoidRequest),
    curl_response: BluesnapVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Put,
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
            _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/services/2/transactions", self.connector_base_url_payments(_req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Bluesnap<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
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
        let base_url = self.connector_base_url_payments(req);
        Ok(format!("{}/services/2/payment-fields-tokens", base_url))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
        _event_builder: Option<&mut events::Event>,
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
        // Bluesnap returns the pfToken in the Location header, not in the body.
        // Location header format: https://sandbox.bluesnap.com/services/2/payment-fields-tokens/<pfToken>
        let location = res
            .headers
            .as_ref()
            .and_then(|h| h.get("location"))
            .and_then(|v| v.to_str().ok())
            .ok_or(ConnectorError::ResponseDeserializationFailed {
                context: domain_types::errors::ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "Bluesnap POST /services/2/payment-fields-tokens did not return a \
                         Location header containing the pfToken URL."
                            .to_owned(),
                    ),
                },
            })?;

        let pf_token =
            location
                .rsplit('/')
                .next()
                .ok_or(ConnectorError::ResponseDeserializationFailed {
                    context: domain_types::errors::ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some(format!(
                            "Failed to extract pfToken from Bluesnap Location header: \
                             expected URL ending with '/<pfToken>', got '{location}'."
                        )),
                    },
                })?;

        let response = BluesnapClientAuthResponse {
            pf_token: Some(hyperswitch_masking::Secret::new(pf_token.to_string())),
        };

        let response_router_data = ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        };

        RouterDataV2::<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >::try_from(response_router_data)
        .change_context(ConnectorError::ResponseHandlingFailed {
            context: domain_types::errors::ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some(
                    "Failed to convert Bluesnap ClientAuthenticationToken response \
                     into RouterDataV2."
                        .to_owned(),
                ),
            },
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

// ===== STUB IMPLEMENTATIONS FOR UNSUPPORTED FLOWS =====

// Payment Void Post Capture
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Bluesnap<T>
{
}

// Setup Mandate
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Bluesnap<T>
{
}

// Repeat Payment
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Bluesnap<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Bluesnap<T>
{
}

// Order Create
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Bluesnap<T>
{
}

// Session Token
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Bluesnap<T>
{
}

// Dispute Accept
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Bluesnap<T>
{
}

// Dispute Defend
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Bluesnap<T>
{
}

// Submit Evidence
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Bluesnap<T>
{
}

// Payment Token (required by PaymentTokenV2 trait)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Bluesnap<T>
{
}

// Access Token (required by ServerAuthentication trait)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Bluesnap<T>
{
}

// ===== AUTHENTICATION FLOW CONNECTOR INTEGRATIONS =====
// Pre Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Bluesnap<T>
{
}

// Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Bluesnap<T>
{
}

// Post Authentication
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Bluesnap<T>
{
}

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Bluesnap<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Bluesnap<T>
{
    fn id(&self) -> &'static str {
        "bluesnap"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.bluesnap.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = bluesnap::BluesnapAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.generate_basic_auth().into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: bluesnap::BluesnapErrorResponse = res
            .response
            .parse_struct("BluesnapErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "bluesnap: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code(),
            message: response.message(),
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}
