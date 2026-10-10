use common_utils::{
    consts, crypto,
    crypto::VerifySignature,
    errors::CustomResult,
    events,
    ext_traits::BytesExt,
    types::{FloatMajorUnit, StringMajorUnit},
};
use domain_types::router_data::ConnectorSpecificConfig;
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync,
        PreAuthenticate, RSync, Refund, RepeatPayment, ServerSessionAuthenticationToken,
        SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventContext, EventType,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};

use serde::Serialize;
use std::fmt::Debug;
pub mod transformers;

use transformers::{
    NuveiAuthenticateRequest, NuveiAuthenticateResponse, NuveiCaptureRequest, NuveiCaptureResponse,
    NuveiClientAuthRequest, NuveiClientAuthResponse, NuveiErrorResponse, NuveiInitPaymentRequest,
    NuveiInitPaymentResponse, NuveiOpenOrderRequest, NuveiOpenOrderResponse, NuveiPaymentRequest,
    NuveiPaymentResponse, NuveiRefundRequest, NuveiRefundResponse, NuveiRefundSyncRequest,
    NuveiRefundSyncResponse, NuveiRepeatPaymentRequest, NuveiRepeatPaymentResponse,
    NuveiSessionTokenRequest, NuveiSessionTokenResponse, NuveiSetupMandateRequest,
    NuveiSetupMandateResponse, NuveiSyncRequest, NuveiSyncResponse, NuveiVoidRequest,
    NuveiVoidResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};

// Local headers module
mod headers {
    pub const CONTENT_TYPE: &str = "Content-Type";
    /// Carries the checksum of a Control Panel (chargeback) DMN.
    pub const CHECKSUM: &str = "checksum";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Nuvei<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Nuvei,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Nuvei<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Nuvei<T>
{
    fn should_do_session_token(
        &self,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> bool {
        true
    }

    /// A 3DS card payment runs /initPayment.do (PreAuthenticate), then the
    /// first /payment.do with the threeD block (Authenticate). Authenticate
    /// always ends the loop: it returns the ACS redirect, a final frictionless
    /// result or an error. After the challenge the caller comes back with a
    /// redirect state: PostAuthenticate reports the challenge result without
    /// calling Nuvei, then Authorize makes the charging /payment.do call. Both
    /// redirect states go to PostAuthenticate because the cres arrives in the
    /// form body, which the redirect state does not describe.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};

        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,

                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authenticate
                }

                (
                    RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                    None,
                ) => AuthenticationStep::PostAuthenticate,

                (
                    RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                    Some(AuthenticationStep::PostAuthenticate),
                ) => AuthenticationStep::Authorize,

                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Nuvei<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let checksum = match get_webhook_object_from_body(&request.body)? {
            transformers::NuveiWebhook::PaymentDmn(dmn) => dmn
                .advance_response_checksum
                .ok_or(WebhookError::WebhookSignatureNotFound)?
                .expose(),
            transformers::NuveiWebhook::Chargeback(_) => request
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(headers::CHECKSUM))
                .map(|(_, value)| value.clone())
                .ok_or(WebhookError::WebhookSignatureNotFound)?,
        };

        hex::decode(checksum).change_context(WebhookError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let secret = std::str::from_utf8(&connector_webhook_secret.secret)
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;

        match get_webhook_object_from_body(&request.body)? {
            // secret + totalAmount + currency + responseTimeStamp +
            // PPP_TransactionID + Status + productId, over the decoded values
            // (spec: Webhook Authentication & Signature Verification).
            transformers::NuveiWebhook::PaymentDmn(dmn) => {
                let status = dmn
                    .status
                    .and_then(transformers::NuveiDmnStatus::checksum_value)
                    .ok_or(WebhookError::WebhookSourceVerificationFailed)?;
                let product_id = dmn
                    .product_id
                    .unwrap_or_else(|| transformers::NUVEI_DMN_ABSENT_PRODUCT_ID.to_string());

                Ok(format!(
                    "{secret}{}{}{}{}{status}{product_id}",
                    dmn.total_amount.get_amount_as_string(),
                    dmn.currency,
                    dmn.response_time_stamp,
                    dmn.ppp_transaction_id,
                )
                .into_bytes())
            }
            // secret + every JSON value of the payload, in payload order.
            transformers::NuveiWebhook::Chargeback(_) => {
                let payload: serde_json::Value = serde_json::from_slice(&request.body)
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?;
                let mut message = secret.to_string();
                transformers::concat_nuvei_json_values(&payload, &mut message);

                Ok(message.into_bytes())
            }
        }
    }

    /// A DMN is verified only when its SHA-256 checksum matches. A missing
    /// secret, a missing checksum or a mismatch all report `false`.
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let Some(connector_webhook_secret) =
            connector_webhook_secret.filter(|secrets| !secrets.secret.is_empty())
        else {
            return Ok(false);
        };

        let (Ok(signature), Ok(message)) = (
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secret),
            self.get_webhook_source_verification_message(&request, &connector_webhook_secret),
        ) else {
            return Ok(false);
        };

        crypto::Sha256
            .verify_signature(&connector_webhook_secret.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        b"ppp_status=OK&PPP_TransactionID=547&TransactionID=2110000000004302220&Status=APPROVED&transactionType=Sale&totalAmount=115&currency=USD&responseTimeStamp=2020-03-14.16%3A22%3A34&productId=Your+Product"
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        match get_webhook_object_from_body(&request.body)? {
            transformers::NuveiWebhook::PaymentDmn(dmn) => {
                get_payment_dmn_event(&dmn).map(|event| event.event_type())
            }
            transformers::NuveiWebhook::Chargeback(notification) => {
                get_chargeback_dispute_status(&notification)
                    .map(transformers::get_nuvei_dispute_event_type)
            }
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        match get_webhook_object_from_body(&request.body)? {
            transformers::NuveiWebhook::PaymentDmn(dmn) => {
                if dmn.is_payout() {
                    return Err(report!(WebhookError::WebhookEventTypeNotFound));
                }
                transformers::get_nuvei_dmn_reference(*dmn).map(Some)
            }
            transformers::NuveiWebhook::Chargeback(notification) => Ok(Some(
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: notification.chargeback.dispute_id,
                    connector_transaction_id: Some(
                        notification.transaction_details.transaction_id.to_string(),
                    ),
                }),
            )),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let dmn = get_payment_dmn_from_body(&request.body)?;
        let status = match get_payment_dmn_event(&dmn)? {
            transformers::NuveiDmnEvent::Payment { status, .. } => status,
            transformers::NuveiDmnEvent::Refund { .. } => {
                return Err(report!(WebhookError::WebhookEventTypeNotFound))
            }
        };
        let (error_code, error_message) = get_payment_dmn_error(&dmn);

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(
                dmn.transaction_id
                    .filter(|transaction_id| !transaction_id.is_empty())
                    .ok_or(WebhookError::WebhookReferenceIdNotFound)?,
            )),
            status,
            connector_response_reference_id: None,
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let dmn = get_payment_dmn_from_body(&request.body)?;
        let status = match get_payment_dmn_event(&dmn)? {
            transformers::NuveiDmnEvent::Refund { status, .. } => status,
            transformers::NuveiDmnEvent::Payment { .. } => {
                return Err(report!(WebhookError::WebhookEventTypeNotFound))
            }
        };
        let (error_code, error_message) = get_payment_dmn_error(&dmn);

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(
                dmn.transaction_id
                    .filter(|transaction_id| !transaction_id.is_empty())
                    .ok_or(WebhookError::WebhookReferenceIdNotFound)?,
            ),
            merchant_transaction_id: None,
            status,
            connector_response_reference_id: None,
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        let notification = match get_webhook_object_from_body(&request.body)? {
            transformers::NuveiWebhook::Chargeback(notification) => notification,
            transformers::NuveiWebhook::PaymentDmn(_) => {
                return Err(report!(WebhookError::WebhookEventTypeNotFound))
            }
        };
        let status = get_chargeback_dispute_status(&notification)?;
        let chargeback = notification.chargeback;
        let stage = transformers::get_nuvei_dispute_stage(&chargeback)
            .ok_or(WebhookError::WebhookEventTypeNotFound)?;
        let currency = chargeback.reported_currency;
        // ReportedAmount is a JSON number in major units.
        let amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
            self.amount_converter_float_major_unit,
            chargeback.reported_amount,
            currency,
        )?;

        Ok(DisputeWebhookDetailsResponse {
            amount: domain_types::utils::convert_amount_for_webhook(
                &common_utils::types::StringMinorUnitForConnector,
                amount,
                currency,
            )?,
            currency,
            dispute_id: chargeback
                .dispute_id
                .ok_or(WebhookError::WebhookReferenceIdNotFound)?,
            status,
            stage,
            connector_response_reference_id: None,
            dispute_message: chargeback.chargeback_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            connector_reason_code: chargeback.chargeback_reason_category,
            additional_details: None,
        })
    }
}

/// Payment DMNs arrive form-encoded, chargeback DMNs as JSON (spec: Webhook
/// Payload Structure), so the body is tried as a form first.
fn get_webhook_object_from_body(
    body: &[u8],
) -> Result<transformers::NuveiWebhook, Report<WebhookError>> {
    serde_urlencoded::from_bytes::<transformers::NuveiWebhook>(body).or_else(|_| {
        serde_json::from_slice::<transformers::NuveiWebhook>(body)
            .change_context(WebhookError::WebhookBodyDecodingFailed)
    })
}

fn get_payment_dmn_from_body(
    body: &[u8],
) -> Result<Box<transformers::NuveiPaymentDmn>, Report<WebhookError>> {
    match get_webhook_object_from_body(body)? {
        transformers::NuveiWebhook::PaymentDmn(dmn) => Ok(dmn),
        transformers::NuveiWebhook::Chargeback(_) => {
            Err(report!(WebhookError::WebhookEventTypeNotFound))
        }
    }
}

/// Event of a payment DMN. Payout DMNs and (Status, transactionType) pairs
/// outside the mapping are "event type not found", never a state change.
fn get_payment_dmn_event(
    dmn: &transformers::NuveiPaymentDmn,
) -> Result<transformers::NuveiDmnEvent, Report<WebhookError>> {
    if dmn.is_payout() {
        return Err(report!(WebhookError::WebhookEventTypeNotFound));
    }
    dmn.status
        .zip(dmn.transaction_type)
        .and_then(|(status, transaction_type)| {
            transformers::map_nuvei_dmn_to_event(status, transaction_type)
        })
        .ok_or_else(|| report!(WebhookError::WebhookEventTypeNotFound))
}

fn get_chargeback_dispute_status(
    notification: &transformers::NuveiChargebackDmn,
) -> Result<common_enums::DisputeStatus, Report<WebhookError>> {
    match notification.event_type {
        transformers::NuveiControlPanelEventType::Chargeback => {
            transformers::map_nuvei_dispute_to_event(&notification.chargeback)
                .ok_or_else(|| report!(WebhookError::WebhookEventTypeNotFound))
        }
        transformers::NuveiControlPanelEventType::Unknown => {
            Err(report!(WebhookError::WebhookEventTypeNotFound))
        }
    }
}

/// Code and message of a DECLINED or ERROR DMN: the gateway ReasonCode, else
/// ErrCode, and Reason. Any other DMN carries no error.
fn get_payment_dmn_error(dmn: &transformers::NuveiPaymentDmn) -> (Option<String>, Option<String>) {
    match dmn.status {
        Some(transformers::NuveiDmnStatus::Declined | transformers::NuveiDmnStatus::Error) => {
            let non_empty =
                |value: &Option<String>| value.clone().filter(|value| !value.is_empty());
            (
                Some(
                    non_empty(&dmn.reason_code)
                        .or_else(|| non_empty(&dmn.err_code))
                        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                ),
                Some(
                    non_empty(&dmn.reason).unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                ),
            )
        }
        Some(
            transformers::NuveiDmnStatus::Approved
            | transformers::NuveiDmnStatus::Success
            | transformers::NuveiDmnStatus::Pending
            | transformers::NuveiDmnStatus::Update
            | transformers::NuveiDmnStatus::Unknown,
        )
        | None => (None, None),
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Nuvei<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Nuvei<T>
{
}
// Create all prerequisites using macros
macros::create_all_prerequisites!(
    connector_name: Nuvei,
    generic_type: T,
    api: [
        (
            flow: CreateOrder,
            request_body: NuveiOpenOrderRequest,
            response_body: NuveiOpenOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: ServerSessionAuthenticationToken,
            request_body: NuveiSessionTokenRequest,
            response_body: NuveiSessionTokenResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: NuveiPaymentRequest<T>,
            response_body: NuveiPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: NuveiSyncRequest,
            response_body: NuveiSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: NuveiCaptureRequest,
            response_body: NuveiCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: NuveiRefundRequest,
            response_body: NuveiRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: NuveiRefundSyncRequest,
            response_body: NuveiRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: NuveiVoidRequest,
            response_body: NuveiVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: NuveiClientAuthRequest,
            response_body: NuveiClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: NuveiSetupMandateRequest<T>,
            response_body: NuveiSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: NuveiRepeatPaymentRequest<T>,
            response_body: NuveiRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: NuveiInitPaymentRequest<T>,
            response_body: NuveiInitPaymentResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: NuveiAuthenticateRequest<T>,
            response_body: NuveiAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter_webhooks: StringMajorUnit,
        amount_converter_float_major_unit: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nuvei.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nuvei.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.nuvei.base_url
        }
    }
);

// Implement ServerSessionAuthenticationToken flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiSessionTokenRequest),
    curl_response: NuveiSessionTokenResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/getSessionToken.do", self.connector_base_url_merchant_auth(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Nuvei<T>
{
    fn id(&self) -> &'static str {
        "nuvei"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.nuvei.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: Result<NuveiErrorResponse, Report<common_utils::errors::ParsingError>> =
            res.response.parse_struct("nuvei ErrorResponse");

        match response {
            Ok(response_data) => {
                if let Some(i) = event_builder {
                    i.set_connector_response(&response_data);
                }
                let typed = macros::serialize_typed_connector_payload(
                    &response_data,
                    "typed_connector_response",
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: response_data
                        .err_code
                        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                    message: response_data
                        .reason
                        .filter(|reason| !reason.is_empty())
                        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: response_data.gw_error_code,
                    network_error_message: response_data
                        .gw_error_reason
                        .filter(|reason| !reason.is_empty()),
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Err(error_msg) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}))
                };
                tracing::error!(deserialization_error =? error_msg);
                domain_types::utils::handle_json_response_deserialization_failure(res, "nuvei")
            }
        }
    }
}

// Implement Authorize flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiPaymentRequest),
    curl_response: NuveiPaymentResponse,
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
            Ok(format!("{}/payment.do", self.connector_base_url_payments(req)))
        }
    }
);

// Implement PSync flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiSyncRequest),
    curl_response: NuveiSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/getTransactionDetails.do", self.connector_base_url_payments(req)))
        }
    }
);

// Implement Capture flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiCaptureRequest),
    curl_response: NuveiCaptureResponse,
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
            Ok(format!("{}/settleTransaction.do", self.connector_base_url_payments(req)))
        }
    }
);

// Implement Refund flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiRefundRequest),
    curl_response: NuveiRefundResponse,
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
            Ok(format!("{}/refundTransaction.do", self.connector_base_url_refunds(req)))
        }
    }
);

// Implement RSync flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiRefundSyncRequest),
    curl_response: NuveiRefundSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/getTransactionDetails.do", self.connector_base_url_refunds(req)))
        }
    }
);

// Implement Void flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiVoidRequest),
    curl_response: NuveiVoidResponse,
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
            Ok(format!("{}/voidTransaction.do", self.connector_base_url_payments(req)))
        }
    }
);

// Implement CreateOrder flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiOpenOrderRequest),
    curl_response: NuveiOpenOrderResponse,
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
            Ok(format!("{}/openOrder.do", self.connector_base_url_payments(req)))
        }
    }
);

// SetupMandate (SetupRecurring) - stores card credentials for recurring payments.
// Uses the same /payment.do endpoint as Authorize, but with isRebilling="0" so
// Nuvei treats the call as the initial CIT transaction of a recurring series
// and returns a userPaymentOptionId we can use for future MIT charges.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiSetupMandateRequest<T>),
    curl_response: NuveiSetupMandateResponse,
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
            Ok(format!("{}/payment.do", self.connector_base_url_payments(req)))
        }
    }
);
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiRepeatPaymentRequest<T>),
    curl_response: NuveiRepeatPaymentResponse,
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
            Ok(format!("{}/payment.do", self.connector_base_url_payments(req)))
        }
    }
);

// PreAuthenticate - 3DS enrolment lookup. /initPayment.do moves no money and
// shows nothing to the cardholder; its transactionId links the next call.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiInitPaymentRequest<T>),
    curl_response: NuveiInitPaymentResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/initPayment.do", self.connector_base_url_payments(req)))
        }
    }
);

// Authenticate - the first /payment.do of a 3DS payment, the one that carries
// the threeD block. It answers with the ACS challenge redirect or, when the
// issuer needs no challenge, with the final result. It shares the endpoint
// with Authorize, which is the last /payment.do and sends no threeD block.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiAuthenticateRequest<T>),
    curl_response: NuveiAuthenticateResponse,
    flow_name: Authenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payment.do", self.connector_base_url_payments(req)))
        }
    }
);

// PostAuthenticate - the result of the ACS challenge. Nuvei has no call for
// it: the cres the browser brought back is decoded here, so the leg makes no
// outbound request and moves no money.
macros::macro_connector_local_flow_implementation!(
    connector: Nuvei,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    handle_response: transformers::handle_post_authenticate_response,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
);

// Implement ClientAuthenticationToken flow using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiClientAuthRequest),
    curl_response: NuveiClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/getSessionToken.do", self.connector_base_url_merchant_auth(req)))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Nuvei<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Nuvei,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        Accept,
        SubmitEvidence,
        DefendDispute,
        VoidPC,
        ServerAuthenticationToken,
        MandateRevoke,
    ],
);
