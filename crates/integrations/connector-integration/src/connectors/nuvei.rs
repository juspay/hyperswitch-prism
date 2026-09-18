use common_utils::{errors::CustomResult, events, ext_traits::BytesExt, types::StringMajorUnit};
use domain_types::router_data::ConnectorSpecificConfig;
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync,
        PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        ServerSessionAuthenticationToken, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventContext, EventType,
        MandateReference, PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentVoidData, PaymentWebhookReference, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, ResponseId, ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use std::str::FromStr;

use serde::Serialize;
use std::fmt::Debug;
pub mod transformers;

use transformers::NuveiDmnStatus;
use transformers::{
    NuveiCaptureRequest, NuveiCaptureResponse, NuveiClientAuthRequest, NuveiClientAuthResponse,
    NuveiErrorResponse, NuveiOpenOrderRequest, NuveiOpenOrderResponse, NuveiPaymentRequest,
    NuveiPaymentResponse, NuveiRefundRequest, NuveiRefundResponse, NuveiRefundSyncRequest,
    NuveiRefundSyncResponse, NuveiRepeatPaymentRequest, NuveiRepeatPaymentResponse,
    NuveiSessionTokenRequest, NuveiSessionTokenResponse, NuveiSetupMandateRequest,
    NuveiSetupMandateResponse, NuveiSyncRequest, NuveiSyncResponse,
    NuveiThreeDSAuthenticateRequest, NuveiThreeDSAuthenticateResponse, NuveiThreeDSFinalRequest,
    NuveiThreeDSFinalResponse, NuveiThreeDSInitRequest, NuveiThreeDSInitResponse,
    NuveiTransactionType, NuveiVoidRequest, NuveiVoidResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;

// Local headers module
mod headers {
    pub const CONTENT_TYPE: &str = "Content-Type";
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

    /// P-ThreeDS-04: drive Nuvei's 3DS 2.x sequence.
    ///
    /// Without this override the dispatcher returns the trait default
    /// (`AuthenticationStep::Authorize`) and none of the three legs is ever reached,
    /// however complete their implementations are.
    ///
    /// The three UCS steps map onto Nuvei's **two** REST 1.0 endpoints:
    /// `PreAuthenticate` is `/initPayment.do`, `Authenticate` is the first
    /// `/payment.do` (with the `threeD` challenge block), and `PostAuthenticate` is the
    /// second `/payment.do` (with `threeD` omitted). Every arm terminates: each one
    /// either advances toward `Authorize` or is `Authorize` itself.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};

        // G-ThreeDS-05: Nuvei declares 3DS for Card/Credit and Card/Debit only; every
        // other payment method, and every non-3DS attempt, goes straight to Authorize.
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                // Device data collection: /initPayment.do returns methodUrl +
                // methodPayload for the 3DS method form.
                (RedirectState::InitialRequest, _) => AuthenticationStep::PreAuthenticate,

                // The cardholder came back from the 3DS method form with a payload:
                // run the first /payment.do to obtain the ACS challenge (or a
                // frictionless approval).
                (RedirectState::RedirectWithParams, None) => AuthenticationStep::Authenticate,

                (RedirectState::RedirectWithParams, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::Authorize
                }

                // The cardholder came back from the ACS challenge with nothing but the
                // return URL: run the second /payment.do, with the threeD block omitted.
                (RedirectState::RedirectWithoutParams, None) => {
                    AuthenticationStep::PostAuthenticate
                }

                (
                    RedirectState::RedirectWithoutParams,
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
    /// The merchant secret used to verify a DMN.
    ///
    /// Nuvei has no separate webhook secret: every DMN family signs with the same
    /// `merchantSecretKey` that signs the outbound requests, so it is read from the
    /// connector account config. `connector_webhook_secret` is honoured first for callers
    /// that do provision one (P-IncomingWebhook-03).
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let secret = nuvei_webhook_secret(connector_webhook_secret, connector_account_details)?;
        let dmn = transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())?;

        match dmn {
            transformers::NuveiDmn::Payment(dmn, raw) => {
                // G-IncomingWebhook-01: an unsigned Payment DMN is never processed.
                // `responseChecksum` is the deprecated predecessor and Nuvei publishes no
                // formula for it, so it is never accepted as a substitute.
                let expected = dmn
                    .advance_response_checksum
                    .as_ref()
                    .map(|checksum| checksum.peek().trim().to_string())
                    .filter(|checksum| !checksum.is_empty())
                    .ok_or_else(|| {
                        tracing::warn!(
                            connector = "nuvei",
                            deprecated_param = transformers::NUVEI_DEPRECATED_CHECKSUM_PARAM,
                            "nuvei webhook: payment DMN carried no advanceResponseChecksum"
                        );
                        Report::new(WebhookError::WebhookSignatureNotFound)
                    })?;

                let message = transformers::nuvei_payment_dmn_checksum_message(&raw, secret.peek());

                // G-IncomingWebhook-02 / UD-09: the digest is SHA-256 *or* MD5 per SiteId
                // and Nuvei publishes no way to detect which, so both are compared before
                // the event is refused.
                if nuvei_digest_matches(
                    &common_utils::crypto::Sha256,
                    message.as_bytes(),
                    &expected,
                )? {
                    return Ok(true);
                }
                if nuvei_digest_matches(&common_utils::crypto::Md5, message.as_bytes(), &expected)?
                {
                    tracing::info!(
                        connector = "nuvei",
                        "nuvei webhook: payment DMN verified with the MD5 digest (SiteId configured for MD5)"
                    );
                    return Ok(true);
                }
                tracing::warn!(
                    connector = "nuvei",
                    "nuvei webhook: advanceResponseChecksum matched neither SHA-256 nor MD5"
                );
                Err(Report::new(WebhookError::WebhookSourceVerificationFailed))
            }

            transformers::NuveiDmn::ControlPanelEvent(_) => {
                // G-IncomingWebhook-03 / UD-10: the header name is unpublished, so the
                // lookup is case-insensitive over `checksum` and the spelling that
                // actually arrived is traced for confirmation against a real chargeback.
                let (header_name, expected) =
                    transformers::nuvei_control_panel_checksum_header(&request.headers)
                        .ok_or_else(|| {
                            tracing::warn!(
                                connector = "nuvei",
                                observed_headers = ?request.headers.keys().collect::<Vec<_>>(),
                                "nuvei webhook: control panel event carried no checksum header"
                            );
                            Report::new(WebhookError::WebhookSignatureNotFound)
                        })?;
                tracing::debug!(
                    connector = "nuvei",
                    header_name,
                    "nuvei webhook: control panel checksum header observed (spec gap G-04)"
                );

                let message = transformers::nuvei_control_panel_checksum_message(
                    &request.body,
                    secret.peek(),
                )?;
                if nuvei_digest_matches(
                    &common_utils::crypto::Sha256,
                    message.as_bytes(),
                    expected.trim(),
                )? {
                    return Ok(true);
                }
                tracing::warn!(
                    connector = "nuvei",
                    "nuvei webhook: control panel event checksum did not match"
                );
                Err(Report::new(WebhookError::WebhookSourceVerificationFailed))
            }

            transformers::NuveiDmn::Withdrawal(dmn, raw) => {
                let expected = dmn
                    .checksum
                    .as_ref()
                    .map(|checksum| checksum.peek().trim().to_string())
                    .filter(|checksum| !checksum.is_empty())
                    .ok_or_else(|| Report::new(WebhookError::WebhookSignatureNotFound))?;
                let message =
                    transformers::nuvei_withdrawal_dmn_checksum_message(&raw, secret.peek());
                if nuvei_digest_matches(
                    &common_utils::crypto::Sha256,
                    message.as_bytes(),
                    &expected,
                )? {
                    return Ok(true);
                }
                Err(Report::new(WebhookError::WebhookSourceVerificationFailed))
            }
        }
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let dmn = transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())?;
        let signature = match &dmn {
            transformers::NuveiDmn::Payment(dmn, _) => dmn
                .advance_response_checksum
                .as_ref()
                .map(|checksum| checksum.peek().trim().to_string()),
            transformers::NuveiDmn::ControlPanelEvent(_) => {
                transformers::nuvei_control_panel_checksum_header(&request.headers)
                    .map(|(_, value)| value.trim().to_string())
            }
            transformers::NuveiDmn::Withdrawal(dmn, _) => dmn
                .checksum
                .as_ref()
                .map(|checksum| checksum.peek().trim().to_string()),
        }
        .filter(|signature| !signature.is_empty())
        .ok_or_else(|| Report::new(WebhookError::WebhookSignatureNotFound))?;

        hex::decode(&signature).change_context(WebhookError::WebhookVerificationSecretInvalid)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let secret = String::from_utf8(connector_webhook_secret.secret.clone())
            .change_context(WebhookError::WebhookVerificationSecretInvalid)?;
        let dmn = transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())?;
        let message = match &dmn {
            transformers::NuveiDmn::Payment(_, raw) => {
                transformers::nuvei_payment_dmn_checksum_message(raw, &secret)
            }
            transformers::NuveiDmn::ControlPanelEvent(_) => {
                transformers::nuvei_control_panel_checksum_message(&request.body, &secret)?
            }
            transformers::NuveiDmn::Withdrawal(_, raw) => {
                transformers::nuvei_withdrawal_dmn_checksum_message(raw, &secret)
            }
        };
        Ok(message.into_bytes())
    }

    /// Stateless: it reads only `RequestDetails`, never a secret, because
    /// `EventService.ParseEvent` runs before the merchant's secret has been resolved.
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let dmn = transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())?;
        match dmn {
            transformers::NuveiDmn::Payment(dmn, _) => {
                let (status, transaction_type) = nuvei_dmn_event_key(&dmn)?;
                Ok(transformers::nuvei_dmn_event_type(status, transaction_type))
            }
            transformers::NuveiDmn::ControlPanelEvent(event) => {
                Ok(transformers::nuvei_control_panel_event_type(&event))
            }
            transformers::NuveiDmn::Withdrawal(_, _) => {
                // Payouts are outside this connector's flow set; the DMN is acknowledged
                // rather than mapped to a payment event it is not.
                tracing::info!(
                    connector = "nuvei",
                    "nuvei webhook: withdrawal DMN received, no payout flow is implemented"
                );
                Ok(EventType::IncomingWebhookEventUnspecified)
            }
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let dmn = transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())?;
        match dmn {
            transformers::NuveiDmn::Payment(dmn, _) => {
                let (_, transaction_type) = nuvei_dmn_event_key(&dmn)?;
                let merchant_reference = dmn
                    .client_unique_id
                    .clone()
                    .or_else(|| dmn.merchant_unique_id.clone())
                    .filter(|id| !id.trim().is_empty());

                let reference = match transaction_type {
                    // A refund has no DMN of its own: it arrives as `Credit`, and
                    // `TransactionID` is then the refund's own fiscal id while
                    // `relatedTransactionId` is the payment it belongs to.
                    NuveiTransactionType::Credit | NuveiTransactionType::VoidCredit => {
                        WebhookResourceReference::Refund(RefundWebhookReference {
                            connector_refund_id: nuvei_non_empty(dmn.transaction_id.as_deref()),
                            merchant_refund_id: merchant_reference,
                            connector_transaction_id: nuvei_non_empty(
                                dmn.related_transaction_id.as_deref(),
                            ),
                            merchant_transaction_id: None,
                        })
                    }
                    NuveiTransactionType::Chargeback => {
                        WebhookResourceReference::Dispute(DisputeWebhookReference {
                            connector_dispute_id: nuvei_non_empty(dmn.transaction_id.as_deref()),
                            connector_transaction_id: nuvei_non_empty(
                                dmn.related_transaction_id.as_deref(),
                            ),
                        })
                    }
                    NuveiTransactionType::Auth
                    | NuveiTransactionType::PreAuth
                    | NuveiTransactionType::Sale
                    | NuveiTransactionType::Settle
                    | NuveiTransactionType::Void
                    | NuveiTransactionType::InitAuth3D
                    | NuveiTransactionType::Auth3D
                    | NuveiTransactionType::VerifyAuth3D => {
                        WebhookResourceReference::Payment(PaymentWebhookReference {
                            // `TransactionID`, not `PPP_TransactionID`: the latter is the
                            // payment-page id and only participates in the checksum.
                            connector_transaction_id: nuvei_non_empty(
                                dmn.transaction_id.as_deref(),
                            ),
                            merchant_transaction_id: merchant_reference,
                        })
                    }
                    NuveiTransactionType::Modification | NuveiTransactionType::Unknown => {
                        tracing::info!(
                            connector = "nuvei",
                            "nuvei webhook: DMN transactionType carries no actionable reference"
                        );
                        return Ok(None);
                    }
                };
                Ok(Some(reference))
            }

            transformers::NuveiDmn::ControlPanelEvent(event) => {
                if !event.is_dispute_event() {
                    return Ok(None);
                }
                // The disputed transaction is named by the event's own nested payload,
                // whose members Nuvei never enumerates (spec gap G-04): it is read
                // opportunistically and `EventId` is the dispute id.
                Ok(Some(WebhookResourceReference::Dispute(
                    DisputeWebhookReference {
                        connector_dispute_id: event
                            .extra_str(&["DisputeId", "ChargebackId", "CaseId"])
                            .or_else(|| nuvei_non_empty(event.event_id.as_deref())),
                        connector_transaction_id: event.extra_str(&[
                            "TransactionId",
                            "TransactionID",
                            "relatedTransactionId",
                        ]),
                    },
                )))
            }

            transformers::NuveiDmn::Withdrawal(_, _) => Ok(None),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let dmn = nuvei_payment_dmn(&request)?;
        let (status, transaction_type) = nuvei_dmn_event_key(&dmn)?;
        let (error_code, error_message) = dmn.error_fields();

        let attempt_status = transformers::nuvei_dmn_attempt_status(status, transaction_type);

        // RV-009: `totalAmount` is the *order* amount, which every Payment DMN carries —
        // including a DECLINED one and a bare Auth. Only a DMN that actually moved money
        // reports a captured amount, matching the reference pattern in
        // `imerchantsolutions.rs` (`AttemptStatus::Charged | PartialCharged`); anything
        // else leaves the field unset rather than claiming a capture that never happened.
        let minor_amount_captured = match attempt_status {
            common_enums::AttemptStatus::Charged | common_enums::AttemptStatus::PartialCharged => {
                nuvei_dmn_minor_amount(self.amount_converter_webhooks, &dmn)?
            }
            _ => None,
        };

        Ok(WebhookDetailsResponse {
            resource_id: nuvei_non_empty(dmn.transaction_id.as_deref())
                .map(ResponseId::ConnectorTransactionId),
            status: attempt_status,
            connector_response_reference_id: nuvei_non_empty(dmn.transaction_id.as_deref()),
            connector_request_reference_id: nuvei_non_empty(dmn.client_unique_id.as_deref())
                .or_else(|| nuvei_non_empty(dmn.merchant_unique_id.as_deref())),
            mandate_reference: nuvei_non_empty(dmn.user_payment_option_id.as_deref()).map(
                |connector_mandate_id| {
                    Box::new(MandateReference {
                        connector_mandate_id: Some(connector_mandate_id),
                        payment_method_id: None,
                        mandate_metadata: None,
                        connector_mandate_request_reference_id: None,
                    })
                },
            ),
            error_code,
            error_message: error_message.clone(),
            error_reason: error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured,
            network_txn_id: nuvei_non_empty(dmn.external_scheme_transaction_id.as_deref()),
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    /// A card refund arrives as a Payment DMN with `transactionType = Credit`; Nuvei has
    /// no refund DMN of its own.
    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let dmn = nuvei_payment_dmn(&request)?;
        let (status, _) = nuvei_dmn_event_key(&dmn)?;
        let (error_code, error_message) = dmn.error_fields();

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: nuvei_non_empty(dmn.transaction_id.as_deref()),
            merchant_transaction_id: nuvei_non_empty(dmn.client_unique_id.as_deref())
                .or_else(|| nuvei_non_empty(dmn.merchant_unique_id.as_deref())),
            status: transformers::nuvei_dmn_refund_status(status),
            connector_response_reference_id: nuvei_non_empty(dmn.related_transaction_id.as_deref()),
            error_code,
            error_message,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }

    /// Disputes arrive in **two** families: as a Payment DMN with
    /// `transactionType = Chargeback` (family a) and as a Control Panel Event with a
    /// chargeback `EventType` (family b). Handling only the first would drop every
    /// pre-chargeback alert, RDR alert and dispute resolution.
    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        let dmn = transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())?;
        let raw_connector_response = Some(String::from_utf8_lossy(&request.body).to_string());

        match dmn {
            transformers::NuveiDmn::Payment(dmn, _) => {
                let (status, transaction_type) = nuvei_dmn_event_key(&dmn)?;
                let currency = nuvei_dmn_currency(&dmn)?;
                let minor_amount = nuvei_dmn_minor_amount(self.amount_converter_webhooks, &dmn)?
                    .ok_or_else(|| {
                        Report::new(WebhookError::WebhookMissingRequiredField {
                            field: "totalAmount",
                        })
                    })?;
                let amount = domain_types::utils::convert_amount_for_webhook(
                    &common_utils::types::StringMinorUnitForConnector,
                    minor_amount,
                    currency,
                )?;
                let (error_code, error_message) = dmn.error_fields();

                Ok(DisputeWebhookDetailsResponse {
                    amount,
                    currency,
                    dispute_id: nuvei_non_empty(dmn.transaction_id.as_deref())
                        .or_else(|| nuvei_non_empty(dmn.ppp_transaction_id.as_deref()))
                        .ok_or_else(|| Report::new(WebhookError::WebhookReferenceIdNotFound))?,
                    // Family (a) carries no dispute lifecycle: a Chargeback DMN is the
                    // dispute being opened. `status` still decides won/lost when Nuvei
                    // reverses it.
                    status: nuvei_dmn_dispute_status(status, transaction_type),
                    stage: common_enums::DisputeStage::Dispute,
                    connector_response_reference_id: nuvei_non_empty(
                        dmn.related_transaction_id.as_deref(),
                    ),
                    dispute_message: error_message,
                    raw_connector_response,
                    status_code: 200,
                    response_headers: None,
                    connector_reason_code: error_code,
                })
            }

            transformers::NuveiDmn::ControlPanelEvent(event) => {
                // Nuvei never enumerates the chargeback event's nested members (spec gap
                // G-04), so amount, currency and the disputed transaction are read
                // opportunistically and a missing one is reported, never invented.
                let currency_code = event
                    .extra_str(&["Currency", "currency", "ChargebackCurrency"])
                    .ok_or_else(|| {
                        Report::new(WebhookError::WebhookMissingRequiredField {
                            field: "Control Panel event currency",
                        })
                    })?;
                let currency = common_enums::Currency::from_str(currency_code.trim())
                    .change_context(WebhookError::WebhookProcessingFailed)
                    .attach_printable("nuvei: unrecognised Control Panel event currency")?;
                let amount_major = event
                    .extra_str(&["Amount", "amount", "ChargebackAmount", "totalAmount"])
                    .ok_or_else(|| {
                        Report::new(WebhookError::WebhookMissingRequiredField {
                            field: "Control Panel event amount",
                        })
                    })?;
                let minor_amount =
                    domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
                        self.amount_converter_webhooks,
                        nuvei_string_major_unit(amount_major.trim())?,
                        currency,
                    )?;
                let amount = domain_types::utils::convert_amount_for_webhook(
                    &common_utils::types::StringMinorUnitForConnector,
                    minor_amount,
                    currency,
                )?;

                Ok(DisputeWebhookDetailsResponse {
                    amount,
                    currency,
                    dispute_id: event
                        .extra_str(&["DisputeId", "ChargebackId", "CaseId"])
                        .or_else(|| nuvei_non_empty(event.event_id.as_deref()))
                        .ok_or_else(|| Report::new(WebhookError::WebhookReferenceIdNotFound))?,
                    status: transformers::nuvei_control_panel_dispute_status(&event),
                    stage: transformers::nuvei_control_panel_dispute_stage(&event),
                    connector_response_reference_id: event.extra_str(&[
                        "TransactionId",
                        "TransactionID",
                        "relatedTransactionId",
                    ]),
                    dispute_message: event.event_type.clone(),
                    raw_connector_response,
                    status_code: 200,
                    response_headers: None,
                    connector_reason_code: event.extra_str(&["ReasonCode", "ChargebackReasonCode"]),
                })
            }

            transformers::NuveiDmn::Withdrawal(_, _) => {
                Err(Report::new(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("nuvei: a withdrawal DMN is not a dispute event"))
            }
        }
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let dmn = transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())?;
        match dmn {
            transformers::NuveiDmn::Payment(dmn, _) => Ok(dmn),
            transformers::NuveiDmn::ControlPanelEvent(event) => Ok(event),
            transformers::NuveiDmn::Withdrawal(dmn, _) => Ok(dmn),
        }
    }

    /// Nuvei acknowledges a DMN with HTTP 200 and an empty body. Any other response marks
    /// the DMN *In Retry*: resent every 15 minutes for 24 hours, up to 96 attempts.
    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, Report<WebhookError>> {
        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: None,
        })
    }

    /// The verbatim `Status=APPROVED` Payment DMN from Nuvei's documentation, so the
    /// field probe exercises the real family-(a) body rather than `{}`.
    fn sample_webhook_body(&self) -> &'static [u8] {
        b"ppp_status=OK&PPP_TransactionId=257354778&userid=&merchant_unique_id=5CXS9TWCNFJP&customData=&productId=&first_name=Test&last_name=Test&email=test%40test.com&currency=EUR&clientUniqueId=5CXS9TWCNFJP&Status=APPROVED&transactionType=Sale&TransactionID=2110000000012345678&totalAmount=20.00&responseTimeStamp=2020-03-21.15:42:49&advanceResponseChecksum=2164dc8cd5d93a4529dd6894f20563639ac22b953b06cdd0c5c4f1fb4e2cd3d3"
    }
}

/// The merchant secret a DMN is verified with.
///
/// Nuvei signs every DMN family with the same `merchantSecretKey` it signs requests with;
/// there is no separate webhook secret, so the connector account config is the source and
/// an explicitly provisioned `connector_webhook_secret` merely overrides it.
fn nuvei_webhook_secret(
    connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    connector_account_details: Option<ConnectorSpecificConfig>,
) -> Result<hyperswitch_masking::Secret<String>, Report<WebhookError>> {
    if let Some(secrets) = connector_webhook_secret {
        if !secrets.secret.is_empty() {
            let secret = String::from_utf8(secrets.secret)
                .change_context(WebhookError::WebhookVerificationSecretInvalid)?;
            return Ok(hyperswitch_masking::Secret::new(secret));
        }
    }

    let connector_account_details = connector_account_details
        .ok_or_else(|| Report::new(WebhookError::WebhookVerificationSecretNotFound))?;
    let auth = transformers::NuveiAuthType::try_from(&connector_account_details)
        .change_context(WebhookError::WebhookVerificationSecretInvalid)?;
    // RV-007: the account-details fallback is guarded exactly like the explicit secret
    // above. An empty `merchant_secret` would make the pre-image of every DMN family
    // consist solely of public wire fields, so any third party could forge a checksum
    // that verifies. Refuse instead of signing with a zero-length key.
    if auth.merchant_secret.peek().trim().is_empty() {
        return Err(Report::new(WebhookError::WebhookVerificationSecretNotFound)
            .attach_printable("nuvei: connector account merchant_secret is empty"));
    }
    Ok(auth.merchant_secret.clone())
}

/// Whether `digest(message)` renders to `expected`, compared case-insensitively because
/// Nuvei does not document the hex case of `advanceResponseChecksum`.
fn nuvei_digest_matches(
    algorithm: &dyn common_utils::crypto::GenerateDigest,
    message: &[u8],
    expected: &str,
) -> Result<bool, Report<WebhookError>> {
    let digest = algorithm
        .generate_digest(message)
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;
    Ok(hex::encode(digest).eq_ignore_ascii_case(expected))
}

/// A trimmed, non-empty owned copy — never `Some("")`.
fn nuvei_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// The family-(a) body, refusing any other family.
fn nuvei_payment_dmn(
    request: &RequestDetails,
) -> Result<Box<transformers::NuveiPaymentDmn>, Report<WebhookError>> {
    match transformers::nuvei_parse_dmn(&request.body, request.query_params.as_deref())? {
        transformers::NuveiDmn::Payment(dmn, _) => Ok(dmn),
        transformers::NuveiDmn::ControlPanelEvent(_) | transformers::NuveiDmn::Withdrawal(_, _) => {
            Err(Report::new(WebhookError::WebhookBodyDecodingFailed)
                .attach_printable("nuvei: expected a payment DMN, got another DMN family"))
        }
    }
}

/// G-IncomingWebhook-04: the event of a Payment DMN is the pair
/// `(Status × transactionType)` — neither alone determines it, so a DMN missing either is
/// refused rather than mapped to a guessed event.
fn nuvei_dmn_event_key(
    dmn: &transformers::NuveiPaymentDmn,
) -> Result<(NuveiDmnStatus, NuveiTransactionType), Report<WebhookError>> {
    let status = dmn.status.ok_or_else(|| {
        tracing::warn!(
            connector = "nuvei",
            "nuvei webhook: payment DMN carried no Status"
        );
        Report::new(WebhookError::WebhookEventTypeNotFound)
    })?;
    let transaction_type = dmn.transaction_type.ok_or_else(|| {
        tracing::warn!(
            connector = "nuvei",
            "nuvei webhook: payment DMN carried no transactionType"
        );
        Report::new(WebhookError::WebhookEventTypeNotFound)
    })?;
    if matches!(status, NuveiDmnStatus::Unknown) {
        tracing::warn!(
            connector = "nuvei",
            "nuvei webhook: undocumented DMN Status, refusing the event"
        );
        return Err(Report::new(WebhookError::WebhookEventTypeNotFound));
    }
    Ok((status, transaction_type))
}

/// The DMN `currency`, parsed into the typed enum rather than carried as a string.
fn nuvei_dmn_currency(
    dmn: &transformers::NuveiPaymentDmn,
) -> Result<common_enums::Currency, Report<WebhookError>> {
    let currency = dmn
        .currency
        .as_deref()
        .map(str::trim)
        .filter(|currency| !currency.is_empty())
        .ok_or_else(|| {
            Report::new(WebhookError::WebhookMissingRequiredField { field: "currency" })
        })?;
    common_enums::Currency::from_str(currency)
        .change_context(WebhookError::WebhookProcessingFailed)
        .attach_printable("nuvei: unrecognised DMN currency")
}

/// `totalAmount` converted back into minor units through the declared
/// `StringMajorUnit` webhook converter. Absent amount and absent currency both yield
/// `None` rather than a fabricated zero.
fn nuvei_dmn_minor_amount(
    amount_converter: &(dyn common_utils::types::AmountConvertor<Output = StringMajorUnit> + Sync),
    dmn: &transformers::NuveiPaymentDmn,
) -> Result<Option<common_utils::types::MinorUnit>, Report<WebhookError>> {
    let Some(total_amount) = dmn.total_amount.clone() else {
        tracing::info!(
            connector = "nuvei",
            "nuvei webhook: DMN carried no totalAmount, no captured amount reported"
        );
        return Ok(None);
    };
    let currency = nuvei_dmn_currency(dmn)?;
    domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
        amount_converter,
        total_amount,
        currency,
    )
    .map(Some)
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
            flow: PreAuthenticate,
            request_body: NuveiThreeDSInitRequest<T>,
            response_body: NuveiThreeDSInitResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: NuveiThreeDSAuthenticateRequest<T>,
            response_body: NuveiThreeDSAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: NuveiThreeDSFinalRequest<T>,
            response_body: NuveiThreeDSFinalResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
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
        )
    ],
    amount_converters: [
        amount_converter_webhooks: StringMajorUnit
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
                // The three-stage error model: `errCode`/`reason` for a stage-1
                // rejection, `gwErrorCode`/`gwErrorReason` for a gateway decline, with
                // `merchantAdviceCode` and `issuerDeclineCode`/`issuerDeclineReason`
                // surfaced as the network advice / decline pair the GSM smart-retry
                // error-code update reads.
                let error_fields = response_data.resolve_error_fields();
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_fields.code,
                    message: error_fields.message,
                    reason: error_fields.reason,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: error_fields.network_advice_code,
                    network_decline_code: error_fields.network_decline_code,
                    network_error_message: error_fields.network_error_message,
                    typed_connector_response: typed,
                    raw_connector_response: Some(hyperswitch_masking::Secret::new(
                        String::from_utf8_lossy(&res.response).to_string(),
                    )),
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

// ThreeDS leg 1 — PreAuthenticate: POST /initPayment.do.
// The 3DS capability probe and device-fingerprinting leg. It returns
// threeD.{v2supported, version, methodUrl, methodPayload, serverTransId}, the
// transactionId the next leg sends as relatedTransactionId, and a refreshed
// sessionToken. UD-01: no checksum is sent — the sessionToken authenticates the call.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiThreeDSInitRequest<T>),
    curl_response: NuveiThreeDSInitResponse,
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

// ThreeDS leg 2 — Authenticate: the FIRST POST /payment.do.
// Same endpoint as Authorize; what makes it 3DS step 1 is the payload —
// relatedTransactionId plus the full paymentOption.card.threeD challenge block.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiThreeDSAuthenticateRequest<T>),
    curl_response: NuveiThreeDSAuthenticateResponse,
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

// ThreeDS leg 3 — PostAuthenticate: the SECOND POST /payment.do.
// Still /payment.do, not /verify3d.do: the /initPayment -> /authorize3d -> /verify3d
// split is the MPI-only variant. This leg carries relatedTransactionId and omits the
// threeD block entirely — sending one is filter code 1155.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Nuvei,
    curl_request: Json(NuveiThreeDSFinalRequest<T>),
    curl_response: NuveiThreeDSFinalResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
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

/// `StringMajorUnit` has no public constructor, so a major-unit amount that arrived as a
/// bare string (only the undocumented Control Panel event payload does — spec gap G-04)
/// is built through its `Deserialize` impl rather than by parsing the money by hand.
fn nuvei_string_major_unit(value: &str) -> Result<StringMajorUnit, Report<WebhookError>> {
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .change_context(WebhookError::WebhookProcessingFailed)
        .attach_printable("nuvei: control panel event amount is not a major-unit string")
}

/// The dispute status a family-(a) `Chargeback` DMN carries.
///
/// Family (a) has no dispute lifecycle of its own: a `Chargeback` DMN *is* the dispute
/// being opened, and only a declined or errored chargeback - Nuvei reversing it -
/// resolves in the merchant's favour. Exhaustive on both axes, no `_ =>` arm.
fn nuvei_dmn_dispute_status(
    status: NuveiDmnStatus,
    transaction_type: NuveiTransactionType,
) -> common_enums::DisputeStatus {
    use common_enums::DisputeStatus;

    match transaction_type {
        NuveiTransactionType::Chargeback => match status {
            NuveiDmnStatus::Declined | NuveiDmnStatus::Error => DisputeStatus::DisputeWon,
            NuveiDmnStatus::Approved
            | NuveiDmnStatus::Success
            | NuveiDmnStatus::Pending
            | NuveiDmnStatus::Update
            | NuveiDmnStatus::Unknown => DisputeStatus::DisputeOpened,
        },
        // No other `transactionType` reaches `process_dispute_webhook`: the fan-out is
        // driven by `get_event_type`, which returns a dispute event only for a
        // chargeback. A body that arrives here anyway is reported as an open dispute
        // rather than silently reclassified.
        NuveiTransactionType::Sale
        | NuveiTransactionType::Auth
        | NuveiTransactionType::PreAuth
        | NuveiTransactionType::Settle
        | NuveiTransactionType::Credit
        | NuveiTransactionType::Void
        | NuveiTransactionType::VoidCredit
        | NuveiTransactionType::InitAuth3D
        | NuveiTransactionType::Auth3D
        | NuveiTransactionType::VerifyAuth3D
        | NuveiTransactionType::Modification
        | NuveiTransactionType::Unknown => DisputeStatus::DisputeOpened,
    }
}
