//! Stripe connector integration for Hyperswitch Prism.
//!
//! Production base URL: `https://api.stripe.com/`
//! API reference: <https://stripe.com/docs/api>
//! API version pinned: `2022-11-15` (via `stripe-version` header)
//!
//! # Implemented Flows
//!
//! | Flow                    | Status       | Stripe endpoint                                      |
//! |-------------------------|--------------|------------------------------------------------------|
//! | `Authorize`             | Implemented  | `POST /v1/payment_intents`                           |
//! | `PSync`                 | Implemented  | `GET  /v1/payment_intents/{id}?expand[0]=latest_charge` |
//! | `Capture`               | Implemented  | `POST /v1/payment_intents/{id}/capture`              |
//! | `Void`                  | Implemented  | `POST /v1/payment_intents/{id}/cancel`               |
//! | `Refund`                | Implemented  | `POST /v1/refunds`                                   |
//! | `RSync`                 | Implemented  | `GET  /v1/refunds/{id}`                              |
//! | `IncrementalAuthorization` | Implemented | `POST /v1/payment_intents/{id}/increment_authorization` |
//! | `SetupMandate`          | Implemented  | `POST /v1/setup_intents`                             |
//! | `RepeatPayment`         | Implemented  | `POST /v1/payment_intents` (off-session)             |
//! | `AcceptDispute`         | Implemented  | `POST /v1/disputes/{id}/close`                       |
//! | `SubmitEvidence`        | Implemented  | `POST /v1/disputes/{id}` (evidence update)           |
//! | `MandateRevoke`         | Wired (stub) | `POST /v1/payment_methods/{id}/detach` (no-op stub)  |
//! | `CreateOrder`           | Wired (stub) | Not applicable — Stripe has no order-create endpoint |
//! | `PreAuthenticate`       | Wired (stub) | Not applicable — 3DS handled inline in Authorize     |
//! | `Authenticate`          | Wired (stub) | Not applicable — 3DS redirect via `next_action`      |
//! | `PostAuthenticate`      | Wired (stub) | Not applicable — post-3DS polled via PSync           |
//! | `ServerAuthToken`       | Wired (stub) | Not applicable — Stripe uses API key auth directly   |
//! | `ServerSessionAuthToken`| Wired (stub) | Not applicable — no session token flow               |
//! | `DefendDispute`         | Stub         | No distinct Stripe endpoint; covered by SubmitEvidence |
//!
//! # Authentication
//!
//! Bearer token auth: `Authorization: Bearer {api_key}`.
//! Connect platform payments use the `Stripe-Account: {transfer_account_id}` header for Direct charges.
//!
//! # Webhook Verification
//!
//! HMAC-SHA256 over `{timestamp}.{raw_body}` with a 300-second replay-protection window.
//! Signature carried in the `Stripe-Signature` header as `t={ts},v1={hex_sig}`.

pub mod transformers;
use std::{
    fmt::Debug,
    marker::{Send, Sync},
};

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
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
        ConnectorCustomerResponse, ConnectorWebhookSecrets, DisputeDefendData,
        DisputeFlowData, DisputeResponseData, EventType, MandateRevokeRequestData,
        MandateRevokeResponseData, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData, SetupMandateRequestData,
        SubmitEvidenceData, WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};

use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
};
use serde::Serialize;
use transformers::{
    self as stripe, AuthenticateStubResponse, CancelRequest, CaptureRequest,
    CreateConnectorCustomerRequest, CreateConnectorCustomerResponse, CreateOrderStubResponse,
    DisputeObj, DisputeObj as SubmitEvidenceResponse, MandateRevokeStubResponse,
    PaymentIncrementalAuthRequest, PaymentIntentRequest,
    PaymentIntentRequest as RepeatPaymentRequest,
    PaymentIntentResponse as PaymentIncrementalAuthResponse, PaymentSyncResponse,
    PaymentsAuthorizeResponse, PaymentsAuthorizeResponse as RepeatPaymentResponse,
    PaymentsCaptureResponse, PaymentsVoidResponse, PostAuthenticateStubResponse,
    PreAuthenticateStubResponse, RefundResponse, RefundResponse as RefundSyncResponse,
    ServerAuthTokenStubResponse, ServerSessionAuthTokenStubResponse, SetupMandateRequest,
    SetupMandateResponse, StripeClientAuthRequest, StripeClientAuthResponse, StripeRefundRequest,
    StripeSubmitEvidenceRequest, StripeTokenResponse, TokenRequest, WebhookEvent,
    WebhookEventObjectResource, WebhookEventStatus, WebhookEventType, WebhookEventTypeBody,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const STRIPE_COMPATIBLE_CONNECT_ACCOUNT: &str = "Stripe-Account";
}
use stripe::auth_headers;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Stripe<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Stripe,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Stripe<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret
            .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))?;

        let signature_header = request
            .headers
            .get("stripe-signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?
            .clone();

        let mut timestamp = None;
        let mut v1_signatures: Vec<String> = Vec::new();

        for part in signature_header.split(',') {
            let mut kv = part.splitn(2, '=');
            match (kv.next(), kv.next()) {
                (Some("t"), Some(val)) => timestamp = Some(val.to_string()),
                (Some("v1"), Some(val)) => v1_signatures.push(val.to_string()),
                _ => {}
            }
        }

        let timestamp_str = timestamp
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound)
                .attach_printable("Missing timestamp in Stripe-Signature header"))?;

        if v1_signatures.is_empty() {
            return Err(report!(WebhookError::WebhookSignatureNotFound)
                .attach_printable("No v1 signatures found in Stripe-Signature header"));
        }

        // Replay protection: reject if timestamp is older than 300 seconds
        let ts: i64 = timestamp_str
            .parse()
            .map_err(|_| report!(WebhookError::WebhookSourceVerificationFailed)
                .attach_printable("Invalid timestamp in Stripe-Signature header"))?;

        let now = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| report!(WebhookError::WebhookSourceVerificationFailed))?
                .as_secs(),
        )
        .map_err(|_| report!(WebhookError::WebhookSourceVerificationFailed))?;

        if (now - ts).abs() > 300 {
            return Err(report!(WebhookError::WebhookSourceVerificationFailed)
                .attach_printable("Stripe webhook timestamp outside 300s tolerance window"));
        }

        // Build signed payload: "{timestamp}.{raw_body}"
        let signed_payload = format!(
            "{}.{}",
            timestamp_str,
            String::from_utf8_lossy(&request.body)
        );

        // Use HmacSha256 verify_signature for constant-time comparison
        let matched = v1_signatures.iter().any(|sig| {
            hex::decode(sig)
                .ok()
                .and_then(|decoded_sig| {
                    crypto::HmacSha256
                        .verify_signature(
                            &connector_webhook_secrets.secret,
                            &decoded_sig,
                            signed_payload.as_bytes(),
                        )
                        .ok()
                })
                .unwrap_or(false)
        });

        Ok(matched)
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_header = request
            .headers
            .get("stripe-signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;

        for part in signature_header.split(',') {
            let mut kv = part.splitn(2, '=');
            if let (Some("v1"), Some(val)) = (kv.next(), kv.next()) {
                let decoded = hex::decode(val)
                    .change_context(WebhookError::WebhookSourceVerificationFailed)?;
                return Ok(decoded);
            }
        }

        Err(report!(WebhookError::WebhookSignatureNotFound)
            .attach_printable("No v1 signature found in Stripe-Signature header"))
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_header = request
            .headers
            .get("stripe-signature")
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;

        let mut timestamp = None;
        for part in signature_header.split(',') {
            let mut kv = part.splitn(2, '=');
            if let (Some("t"), Some(val)) = (kv.next(), kv.next()) {
                timestamp = Some(val.to_string());
                break;
            }
        }

        let ts = timestamp.ok_or_else(|| {
            report!(WebhookError::WebhookSignatureNotFound)
                .attach_printable("Missing timestamp in Stripe-Signature header")
        })?;

        let signed_payload = format!("{}.{}", ts, String::from_utf8_lossy(&request.body));
        Ok(signed_payload.into_bytes())
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let event: WebhookEventTypeBody = request
            .body
            .parse_struct("WebhookEventTypeBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        match event.event_type {
            WebhookEventType::PaymentIntentSucceed => Ok(EventType::PaymentIntentSuccess),
            WebhookEventType::PaymentIntentFailed => Ok(EventType::PaymentIntentFailure),
            WebhookEventType::PaymentIntentCanceled => Ok(EventType::PaymentIntentCancelled),
            WebhookEventType::PaymentIntentProcessing => Ok(EventType::PaymentIntentProcessing),
            WebhookEventType::PaymentIntentRequiresAction => Ok(EventType::PaymentActionRequired),
            WebhookEventType::PaymentIntentAmountCapturableUpdated => {
                Ok(EventType::PaymentIntentAuthorizationSuccess)
            }
            WebhookEventType::PaymentIntentPartiallyFunded => {
                Ok(EventType::PaymentIntentPartiallyFunded)
            }
            WebhookEventType::ChargeSucceeded
            | WebhookEventType::ChargeCaptured => Ok(EventType::PaymentIntentCaptureSuccess),
            WebhookEventType::ChargeFailed => Ok(EventType::PaymentIntentFailure),
            WebhookEventType::ChargePending => Ok(EventType::PaymentIntentProcessing),
            WebhookEventType::ChargeExpired => Ok(EventType::PaymentIntentExpired),
            WebhookEventType::ChargeRefunded
            | WebhookEventType::ChargeRefundUpdated => Ok(EventType::RefundSuccess),
            WebhookEventType::DisputeCreated => Ok(EventType::DisputeOpened),
            WebhookEventType::DisputeUpdated => Ok(EventType::DisputeChallenged),
            WebhookEventType::DisputeClosed => {
                match event.event_data.event_object.status {
                    Some(WebhookEventStatus::Won) => Ok(EventType::DisputeWon),
                    Some(WebhookEventStatus::Lost) => Ok(EventType::DisputeLost),
                    Some(WebhookEventStatus::WarningClosed) => Ok(EventType::DisputeWon),
                    Some(WebhookEventStatus::ChargeRefunded) => Ok(EventType::DisputeLost),
                    _ => Ok(EventType::DisputeWon),
                }
            }
            WebhookEventType::ChargeDisputeFundsWithdrawn => Ok(EventType::DisputeLost),
            WebhookEventType::ChargeDisputeFundsReinstated => Ok(EventType::DisputeWon),
            WebhookEventType::SourceChargeable => Ok(EventType::SourceChargeable),
            WebhookEventType::SourceTransactionCreated => Ok(EventType::SourceTransactionCreated),
            WebhookEventType::PaymentIntentCreated
            | WebhookEventType::ChargeUpdated
            | WebhookEventType::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let request_body_copy = request.body.clone();
        let event: WebhookEventTypeBody = request
            .body
            .parse_struct("WebhookEventTypeBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let resource: WebhookEventObjectResource = serde_json::from_slice(&request_body_copy)
            .map_err(|_| report!(WebhookError::WebhookBodyDecodingFailed))?;

        let resource_id = resource
            .data
            .object
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| id.to_string());

        let status = match event.event_type {
            WebhookEventType::PaymentIntentSucceed => common_enums::AttemptStatus::Charged,
            WebhookEventType::PaymentIntentFailed
            | WebhookEventType::ChargeFailed => common_enums::AttemptStatus::Failure,
            WebhookEventType::PaymentIntentCanceled => common_enums::AttemptStatus::Voided,
            WebhookEventType::PaymentIntentProcessing
            | WebhookEventType::ChargePending => common_enums::AttemptStatus::Pending,
            WebhookEventType::PaymentIntentRequiresAction => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            WebhookEventType::PaymentIntentAmountCapturableUpdated => {
                common_enums::AttemptStatus::Authorized
            }
            WebhookEventType::ChargeSucceeded
            | WebhookEventType::ChargeCaptured => common_enums::AttemptStatus::Charged,
            WebhookEventType::ChargeExpired => common_enums::AttemptStatus::Failure,
            _ => common_enums::AttemptStatus::Pending,
        };

        let (error_code, error_message) = if status == common_enums::AttemptStatus::Failure {
            let last_error = resource
                .data
                .object
                .get("last_payment_error")
                .and_then(|e| e.get("code"))
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
            let last_msg = resource
                .data
                .object
                .get("last_payment_error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            (last_error, last_msg)
        } else {
            (None, None)
        };

        Ok(WebhookDetailsResponse {
            resource_id: resource_id
                .map(ResponseId::ConnectorTransactionId),
            status,
            connector_response_reference_id: resource
                .data
                .object
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            error_code: error_code.clone(),
            error_message,
            error_reason: error_code,
            raw_connector_response: Some(
                String::from_utf8_lossy(&request_body_copy).to_string(),
            ),
            status_code: 200,
            response_headers: None,
            mandate_reference: None,
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
            minor_amount_captured: None,
            amount_captured: None,
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
        let request_body_copy = request.body.clone();
        let event: WebhookEventTypeBody = request
            .body
            .parse_struct("WebhookEventTypeBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let resource: WebhookEventObjectResource = serde_json::from_slice(&request_body_copy)
            .map_err(|_| report!(WebhookError::WebhookBodyDecodingFailed))?;

        let refund_id = resource
            .data
            .object
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let refund_status = match event.event_data.event_object.status {
            Some(WebhookEventStatus::Succeeded) => common_enums::RefundStatus::Success,
            Some(WebhookEventStatus::Failed) => common_enums::RefundStatus::Failure,
            Some(WebhookEventStatus::Canceled) => common_enums::RefundStatus::Failure,
            _ => common_enums::RefundStatus::Pending,
        };

        let (error_code, error_message) = if refund_status == common_enums::RefundStatus::Failure {
            let reason = resource
                .data
                .object
                .get("failure_reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (reason.clone(), reason)
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: refund_id.clone(),
            status: refund_status,
            connector_response_reference_id: refund_id,
            error_code,
            error_message,
            raw_connector_response: Some(
                String::from_utf8_lossy(&request_body_copy).to_string(),
            ),
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
        let event: WebhookEvent = request
            .body
            .parse_struct("WebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let obj = &event.event_data.event_object;

        let (stage, status) = match event.event_type {
            WebhookEventType::DisputeCreated => (
                common_enums::DisputeStage::Dispute,
                common_enums::DisputeStatus::DisputeOpened,
            ),
            WebhookEventType::DisputeUpdated => {
                match &obj.status {
                    Some(WebhookEventStatus::WarningNeedsResponse)
                    | Some(WebhookEventStatus::NeedsResponse) => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeOpened,
                    ),
                    Some(WebhookEventStatus::WarningUnderReview)
                    | Some(WebhookEventStatus::UnderReview) => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeChallenged,
                    ),
                    Some(WebhookEventStatus::Won) => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeWon,
                    ),
                    Some(WebhookEventStatus::Lost)
                    | Some(WebhookEventStatus::ChargeRefunded) => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeLost,
                    ),
                    Some(WebhookEventStatus::WarningClosed) => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeWon,
                    ),
                    _ => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeOpened,
                    ),
                }
            }
            WebhookEventType::DisputeClosed => {
                match &obj.status {
                    Some(WebhookEventStatus::Won)
                    | Some(WebhookEventStatus::WarningClosed) => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeWon,
                    ),
                    Some(WebhookEventStatus::Lost)
                    | Some(WebhookEventStatus::ChargeRefunded) => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeLost,
                    ),
                    _ => (
                        common_enums::DisputeStage::Dispute,
                        common_enums::DisputeStatus::DisputeWon,
                    ),
                }
            }
            WebhookEventType::ChargeDisputeFundsWithdrawn => (
                common_enums::DisputeStage::Dispute,
                common_enums::DisputeStatus::DisputeLost,
            ),
            WebhookEventType::ChargeDisputeFundsReinstated => (
                common_enums::DisputeStage::Dispute,
                common_enums::DisputeStatus::DisputeWon,
            ),
            _ => (
                common_enums::DisputeStage::Dispute,
                common_enums::DisputeStatus::DisputeOpened,
            ),
        };

        let minor_amount = obj
            .amount
            .unwrap_or(common_utils::types::MinorUnit::new(0));
        let amount = domain_types::utils::convert_amount_for_webhook(
            &common_utils::types::StringMinorUnitForConnector,
            minor_amount,
            obj.currency,
        )?;

        Ok(
            domain_types::connector_types::DisputeWebhookDetailsResponse {
                amount,
                currency: obj.currency,
                dispute_id: obj.id.clone(),
                stage,
                status,
                connector_response_reference_id: Some(obj.id.clone()),
                dispute_message: obj.reason.clone(),
                connector_reason_code: None,
                raw_connector_response: Some(
                    String::from_utf8_lossy(&request_body_copy).to_string(),
                ),
                status_code: 200,
                response_headers: None,
            },
        )
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<WebhookError>,
    > {
        let resource: WebhookEventObjectResource = serde_json::from_slice(&request.body)
            .map_err(|_| report!(WebhookError::WebhookBodyDecodingFailed))?;
        Ok(Box::new(resource.data.object))
    }

    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
    ) -> Result<
        interfaces::api::ApplicationResponse<serde_json::Value>,
        error_stack::Report<WebhookError>,
    > {
        Ok(interfaces::api::ApplicationResponse::StatusOk)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Stripe<T>
{
    fn get_algorithm(
        &self,
    ) -> CustomResult<
        Box<dyn VerifySignature + Send>,
        IntegrationError,
    > {
        Ok(Box::new(crypto::HmacSha256))
    }

    fn get_secrets(
        &self,
        secrets: ConnectorSourceVerificationSecrets,
    ) -> CustomResult<Vec<u8>, IntegrationError> {
        match secrets {
            ConnectorSourceVerificationSecrets::WebhookSecret(webhook_secrets) => {
                Ok(webhook_secrets.secret)
            }
            ConnectorSourceVerificationSecrets::AuthWithWebHookSecret {
                webhook_secret, ..
            } => Ok(webhook_secret.secret),
            _ => Ok(Vec::new()),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Stripe<T>
{
    fn should_create_connector_customer(&self) -> bool {
        true
    }
    fn should_do_payment_method_token(
        &self,
        payment_method: common_enums::PaymentMethod,
        payment_method_type: Option<common_enums::PaymentMethodType>,
    ) -> bool {
        matches!(payment_method, common_enums::PaymentMethod::Wallet)
            && !matches!(
                payment_method_type,
                Some(common_enums::PaymentMethodType::GooglePay)
            )
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Stripe<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Stripe, amount_type: MinorUnit);
macros::create_all_prerequisites!(
    connector_name: Stripe,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PaymentIntentRequest<T>,
            response_body: PaymentsAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: RepeatPaymentRequest<T>,
            response_body: RepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: CaptureRequest,
            response_body: PaymentsCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: CancelRequest,
            response_body: PaymentsVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: StripeRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: TokenRequest<T>,
            response_body: StripeTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: SetupMandate,
            request_body: SetupMandateRequest<T>,
            response_body: SetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: CreateConnectorCustomer,
            request_body: CreateConnectorCustomerRequest,
            response_body: CreateConnectorCustomerResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: IncrementalAuthorization,
            request_body: PaymentIncrementalAuthRequest,
            response_body: PaymentIncrementalAuthResponse,
            router_data: RouterDataV2<IncrementalAuthorization, PaymentFlowData, PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: StripeClientAuthRequest,
            response_body: StripeClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, PaymentFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: Accept,
            response_body: DisputeObj,
            router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ),
        (
            flow: SubmitEvidence,
            request_body: StripeSubmitEvidenceRequest,
            response_body: SubmitEvidenceResponse,
            router_data: RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ),
        (
            flow: MandateRevoke,
            response_body: MandateRevokeStubResponse,
            router_data: RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ),
        (
            flow: CreateOrder,
            response_body: CreateOrderStubResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: PreAuthenticate,
            response_body: PreAuthenticateStubResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            response_body: AuthenticateStubResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            response_body: PostAuthenticateStubResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: ServerAuthenticationToken,
            response_body: ServerAuthTokenStubResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: ServerSessionAuthenticationToken,
            response_body: ServerSessionAuthTokenStubResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, PaymentFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
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
            &req.resource_common_data.connectors.stripe.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stripe.base_url
        }

        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stripe.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Stripe<T>
{
    fn id(&self) -> &'static str {
        "stripe"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        // &self.base_url
        connectors.stripe.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = stripe::StripeAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", auth.api_key.peek()).into_masked(),
            ),
            (
                auth_headers::STRIPE_API_VERSION.to_string(),
                auth_headers::STRIPE_VERSION.to_string().into_masked(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: stripe::ErrorResponse =
            res.response.parse_struct("ErrorResponse").change_context(
                crate::utils::response_handling_fail_for_connector(res.status_code, "stripe"),
            )?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error
                .code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .error
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.error.message.map(|message| {
                response
                    .error
                    .decline_code
                    .clone()
                    .map(|decline_code| {
                        format!("message - {message}, decline_code - {decline_code}")
                    })
                    .unwrap_or(message)
            }),
            attempt_status: None,
            connector_transaction_id: response.error.payment_intent.map(|pi| pi.id),
            network_advice_code: response.error.network_advice_code,
            network_decline_code: response.error.network_decline_code,
            network_error_message: response.error.decline_code.or(response.error.advice_code),
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(PaymentIntentRequest),
    curl_response: PaymentsAuthorizeResponse,
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
                self.common_get_content_type()
                    .to_string()
                    .into(),
            )];

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            let stripe_split_payment_metadata = stripe::StripeSplitPaymentRequest::try_from(req)?;

            // if the request has split payment object, then append the transfer account id in headers in charge_type is Direct
            if let Some(domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(
                stripe_split_payment,
            )) = &req.request.split_payments
            {
                if stripe_split_payment.charge_type
                    ==common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct)
                {
                    let mut customer_account_header = vec![(
                        headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                        stripe_split_payment
                            .transfer_account_id
                            .clone()
                            .into_masked(),
                    )];
                    header.append(&mut customer_account_header);
                }
            }
            // if request doesn't have transfer_account_id, but stripe_split_payment_metadata has it, append it
            else if let Some(transfer_account_id) =
                stripe_split_payment_metadata.transfer_account_id.clone()
            {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.into_masked(),
                )];
                header.append(&mut customer_account_header);
            }
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_intents"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(RepeatPaymentRequest),
    curl_response: RepeatPaymentResponse,
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
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type()
                    .to_string()
                    .into(),
            )];

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            let stripe_split_payment_metadata = stripe::StripeSplitPaymentRequest::try_from(req)?;

            let transfer_account_id = req
                .request
                .split_payments
                .as_ref()
                .map(|split_payments| {
                    let domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(stripe_split_payment) =
                        split_payments;
                    stripe_split_payment
                })
                .filter(|stripe_split_payment| {
                    matches!(stripe_split_payment.charge_type, common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct))
                })
                .map(|stripe_split_payment| stripe_split_payment.transfer_account_id.clone())
                .or_else(|| stripe_split_payment_metadata.transfer_account_id.clone().map(|s| s.expose()));

            if let Some(transfer_account_id) = transfer_account_id {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.clone().into_masked(),
                )];
                header.append(&mut customer_account_header);
            };
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_intents"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(TokenRequest),
    curl_response: StripeTokenResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let transfer_account_id = req
                .request
                .split_payments
                .as_ref()
                .map(|split_payments| {
                    let domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(stripe_split_payment) =
                        split_payments;
                    stripe_split_payment
                })
                .filter(|stripe_split_payment| {
                    matches!(stripe_split_payment.charge_type, common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct))
                })
                .map(|stripe_split_payment| stripe_split_payment.transfer_account_id.clone());

            if let Some(transfer_account_id) = transfer_account_id {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.clone().into_masked(),
                )];
                header.append(&mut customer_account_header);
            };

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_methods"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(SetupMandateRequest),
    curl_response: SetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/setup_intents"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(CreateConnectorCustomerRequest),
    curl_response: CreateConnectorCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type()
                    .to_string()
                    .into(),
            )];
            let transfer_account_id = req
                .request
                .split_payments
                .as_ref()
                .map(|split_payments| {
                    let domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(stripe_split_payment) =
                        split_payments;
                    stripe_split_payment
                })
                .filter(|stripe_split_payment| {
                    matches!(stripe_split_payment.charge_type, common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct))
                })
                .map(|stripe_split_payment| stripe_split_payment.transfer_account_id.clone());

            if let Some(transfer_account_id) = transfer_account_id {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.clone().into_masked(),
                )];
                header.append(&mut customer_account_header);
            };

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), "v1/customers"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_response: PaymentSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            if let Some(domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(
                stripe_split_payment,
            )) = &req.request.split_payments
            {
                transformers::transform_headers_for_connect_platform(
                    stripe_split_payment.charge_type.clone(),
                    Secret::new(stripe_split_payment.transfer_account_id.clone()),
                    &mut header,
                );
            }
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_transaction_id.clone();

            match id.get_connector_transaction_id() {
                Ok(x) if x.starts_with("set") => Ok(format!(
                    "{}{}/{}?expand[0]=latest_attempt", // expand latest attempt to extract payment checks and three_d_secure data
                    self.connector_base_url_payments(req),
                    "v1/setup_intents",
                    x,
                )),
                Ok(x) => Ok(format!(
                    "{}{}/{}{}",
                    self.connector_base_url_payments(req),
                    "v1/payment_intents",
                    x,
                    "?expand[0]=latest_charge" //updated payment_id(if present) reside inside latest_charge field
                )),
                x => x.change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })
}
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(CaptureRequest),
    curl_response: PaymentsCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                Self::common_get_content_type(self).to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_transaction_id.get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!(
                "{}{}/{}/capture",
                self.connector_base_url_payments(req),
                "v1/payment_intents",
                id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(CancelRequest),
    curl_response: PaymentsVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let payment_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}v1/payment_intents/{}/cancel",
                self.connector_base_url_payments(req),
                payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(PaymentIncrementalAuthRequest),
    curl_response: PaymentIncrementalAuthResponse,
    flow_name: IncrementalAuthorization,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsIncrementalAuthorizationData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
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
            let payment_id = &req.request.connector_transaction_id.get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!(
                "{}v1/payment_intents/{}/increment_authorization",
                self.connector_base_url_payments(req),
                payment_id
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(StripeRefundRequest),
    curl_response: RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            if let Some(domain_types::connector_types::SplitRefundsRequest::StripeSplitRefund(ref stripe_split_refund)) =
                req.request.split_refunds.as_ref()
            {
                match &stripe_split_refund.charge_type {
                    common_enums::PaymentChargeType::Stripe(stripe_charge) => {
                        if stripe_charge == &common_enums::StripeChargeType::Direct {
                            let mut customer_account_header = vec![(
                                headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                                stripe_split_refund
                                    .transfer_account_id
                                    .clone()
                                    .into_masked(),
                            )];
                            header.append(&mut customer_account_header);
                        }
                    }
                }
            }
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_refunds(req), "v1/refunds"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_response: RefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);

            if let Some(domain_types::connector_types::SplitRefundsRequest::StripeSplitRefund(ref stripe_refund)) =
                req.request.split_refunds.as_ref()
            {
                transformers::transform_headers_for_connect_platform(
                    stripe_refund.charge_type.clone(),
                    Secret::new(stripe_refund.transfer_account_id.clone()),
                    &mut header,
                );
            }
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.connector_refund_id.clone();
            Ok(format!("{}v1/refunds/{}", self.connector_base_url_refunds(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(StripeClientAuthRequest),
    curl_response: StripeClientAuthResponse,
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                "v1/payment_intents"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_response: DisputeObj,
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
            let dispute_id = &req.request.connector_dispute_id;
            Ok(format!(
                "{}v1/disputes/{}/close",
                self.connector_base_url_disputes(req),
                dispute_id
            ))
        }
    }
);
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(StripeSubmitEvidenceRequest),
    curl_response: SubmitEvidenceResponse,
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
            let dispute_id = &req.request.connector_dispute_id;
            Ok(format!(
                "{}v1/disputes/{}",
                self.connector_base_url_disputes(req),
                dispute_id
            ))
        }
    }
);
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Stripe<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Stripe<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Stripe<T>
{
}

// FlowNotSupported: payment_method_eligibility
// Stripe has no explicit payment method eligibility endpoint.

// FlowNotSupported: reverse
// Not a standard Stripe flow. Stripe uses void/cancel pre-capture and refund post-capture.

// SourceVerification implementations for all flows
