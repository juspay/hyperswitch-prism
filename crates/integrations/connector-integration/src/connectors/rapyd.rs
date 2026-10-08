pub mod transformers;

use base64::Engine;
use common_utils::{
    crypto::VerifySignature,
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::{AmountConvertor, StringMinorUnitForConnector},
    StringMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventContext, EventType,
        MandateReference, PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentVoidData, PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use ring::hmac;
use serde::Serialize;
use std::fmt::Debug;
use transformers::{
    CaptureRequest, RapydAuthType, RapydClientAuthRequest, RapydClientAuthResponse,
    RapydCreateOrderRequest, RapydCreateOrderResponse, RapydErrorResponse, RapydPaymentsRequest,
    RapydPaymentsResponse as RapydCaptureResponse, RapydPaymentsResponse as RapydPSyncResponse,
    RapydPaymentsResponse as RapydVoidResponse, RapydPaymentsResponse as RapydAuthorizeResponse,
    RapydRefundRequest, RapydRepeatPaymentRequest, RapydRepeatPaymentResponse,
    RapydSetupMandateRequest, RapydSetupMandateResponse, RefundResponse,
    RefundResponse as RapydRSyncResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

pub const BASE64_ENGINE_URL_SAFE: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Rapyd<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Rapyd,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Rapyd<T>
{
    /// Verifies the `signature` header against the HMAC-SHA256 of the request.
    ///
    /// Fails closed: without a webhook secret, with a secret that is not the
    /// `{access_key, secret_key}` object, with a missing `salt` / `timestamp` /
    /// `signature` header, an undecodable signature or no usable URL, this
    /// returns an error and the event is reported as unverified. The
    /// connector account credentials are deliberately not a fallback: a
    /// webhook is trusted only on the secret configured for webhooks.
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret
            .ok_or_else(|| Report::new(WebhookError::WebhookVerificationSecretNotFound))?;
        let secret_key = parse_webhook_secret(&connector_webhook_secrets)?.secret_key;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        common_utils::crypto::HmacSha256
            .verify_signature(secret_key.peek().as_bytes(), &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Rapyd webhook signature verification failed")
    }

    /// The raw HMAC digest carried by the `signature` header. Rapyd sends
    /// BASE64(hex(HMAC)) — the same encoding as the request signature built
    /// by `generate_signature` — so the header is URL-safe Base64-decoded and
    /// then hex-decoded.
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let base64_signature = get_webhook_header(request, "signature")
            .ok_or_else(|| Report::new(WebhookError::WebhookSignatureNotFound))?;
        let hex_digest = BASE64_ENGINE_URL_SAFE
            .decode(base64_signature.as_bytes())
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Rapyd webhook signature is not URL-safe Base64")?;
        hex::decode(hex_digest)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Rapyd webhook signature is not a hex-encoded digest")
    }

    /// The signed message: `url + salt + timestamp + access_key + secret_key +
    /// body`. Unlike a request signature it has no HTTP-method component.
    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, Report<WebhookError>> {
        let auth = parse_webhook_secret(connector_webhook_secret)?;
        let url = get_webhook_url(request)?;
        let salt = get_webhook_header(request, "salt").ok_or_else(|| {
            Report::new(WebhookError::WebhookMissingRequiredField { field: "salt" })
        })?;
        let timestamp = get_webhook_header(request, "timestamp").ok_or_else(|| {
            Report::new(WebhookError::WebhookMissingRequiredField { field: "timestamp" })
        })?;
        let body = std::str::from_utf8(&request.body)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Rapyd webhook body is not UTF-8")?;

        Ok(format!(
            "{url}{salt}{timestamp}{}{}{body}",
            auth.access_key.peek(),
            auth.secret_key.peek()
        )
        .into_bytes())
    }

    /// Classifies the event from the body `type`, one arm per row of the
    /// techspec event table ("Receive Webhook (inbound)").
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let webhook = parse_webhook_body(&request)?;
        match webhook.webhook_type {
            // `PAYMENT_COMPLETED`, `PAYMENT_CAPTURED` | payment success
            transformers::RapydWebhookEventType::PaymentCompleted
            | transformers::RapydWebhookEventType::PaymentCaptured => {
                Ok(EventType::PaymentIntentSuccess)
            }
            // `PAYMENT_FAILED` | payment failure
            transformers::RapydWebhookEventType::PaymentFailed => {
                Ok(EventType::PaymentIntentFailure)
            }
            // `REFUND_COMPLETED` | refund success
            transformers::RapydWebhookEventType::RefundCompleted => Ok(EventType::RefundSuccess),
            // `PAYMENT_REFUND_FAILED`, `PAYMENT_REFUND_REJECTED` | refund failure
            transformers::RapydWebhookEventType::PaymentRefundFailed
            | transformers::RapydWebhookEventType::PaymentRefundRejected => {
                Ok(EventType::RefundFailure)
            }
            // `PAYMENT_DISPUTE_CREATED` | dispute opened
            transformers::RapydWebhookEventType::PaymentDisputeCreated => {
                Ok(EventType::DisputeOpened)
            }
            // `PAYMENT_DISPUTE_UPDATED` | by `data.status`
            transformers::RapydWebhookEventType::PaymentDisputeUpdated => {
                match typed_webhook_payload(&webhook)? {
                    transformers::RapydWebhookPayload::Dispute(dispute) => {
                        Ok(match dispute_status(dispute.status) {
                            Some(common_enums::DisputeStatus::DisputeOpened) => {
                                EventType::DisputeOpened
                            }
                            Some(common_enums::DisputeStatus::DisputeChallenged) => {
                                EventType::DisputeChallenged
                            }
                            Some(common_enums::DisputeStatus::DisputeLost) => {
                                EventType::DisputeLost
                            }
                            Some(common_enums::DisputeStatus::DisputeWon) => EventType::DisputeWon,
                            // `dispute_status` yields only the four above;
                            // the rest are listed so a new mapping there
                            // cannot be silently classified here.
                            Some(
                                common_enums::DisputeStatus::DisputeExpired
                                | common_enums::DisputeStatus::DisputeAccepted
                                | common_enums::DisputeStatus::DisputeCancelled,
                            )
                            | None => EventType::IncomingWebhookEventUnspecified,
                        })
                    }
                    transformers::RapydWebhookPayload::Payment(_)
                    | transformers::RapydWebhookPayload::Refund(_)
                    | transformers::RapydWebhookPayload::Unsupported => {
                        Err(Report::new(WebhookError::WebhookBodyDecodingFailed))
                    }
                }
            }
            // Documented events with no mapped outcome, and any unknown
            // `type`: not an error, an unsupported event.
            transformers::RapydWebhookEventType::PaymentSucceeded
            | transformers::RapydWebhookEventType::PaymentCanceled
            | transformers::RapydWebhookEventType::PaymentExpired
            | transformers::RapydWebhookEventType::PaymentUpdated
            | transformers::RapydWebhookEventType::PaymentRefundUpdated
            | transformers::RapydWebhookEventType::CardAddedSuccessfully
            | transformers::RapydWebhookEventType::Unknown => {
                Ok(EventType::IncomingWebhookEventUnspecified)
            }
        }
    }

    /// The object the event is about, read from the body only so that
    /// ParseEvent works without secrets.
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let webhook = parse_webhook_body(&request)?;
        Ok(match typed_webhook_payload(&webhook)? {
            // payment events: `data.id` is the `payment_...` id
            transformers::RapydWebhookPayload::Payment(payment) => {
                Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(payment.id),
                    merchant_transaction_id: None,
                }))
            }
            // refund events: `data.id` is the `refund_...` id, `data.payment`
            // the refunded payment
            transformers::RapydWebhookPayload::Refund(refund) => {
                Some(WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(refund.id),
                    merchant_refund_id: None,
                    connector_transaction_id: refund.payment,
                    merchant_transaction_id: None,
                }))
            }
            // dispute events: `data.token` is the `dispute_...` id,
            // `data.original_transaction_id` the disputed payment
            transformers::RapydWebhookPayload::Dispute(dispute) => {
                Some(WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(dispute.token),
                    connector_transaction_id: Some(dispute.original_transaction_id),
                }))
            }
            transformers::RapydWebhookPayload::Unsupported => None,
        })
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let webhook = parse_webhook_body(&request)?;
        let payment = match typed_webhook_payload(&webhook)? {
            transformers::RapydWebhookPayload::Payment(payment) => payment,
            transformers::RapydWebhookPayload::Refund(_)
            | transformers::RapydWebhookPayload::Dispute(_)
            | transformers::RapydWebhookPayload::Unsupported => {
                return Err(Report::new(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Rapyd webhook does not carry a payment object"));
            }
        };

        let status = payment.attempt_status();
        // Rapyd sends `failure_code` / `failure_message` as empty strings on
        // events that did not fail, so they are reported only on a failure.
        let (error_code, error_message, error_reason) =
            if status == common_enums::AttemptStatus::Failure {
                let failure_message = non_empty(payment.failure_message);
                (
                    Some(
                        non_empty(payment.failure_code)
                            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    ),
                    Some(
                        failure_message
                            .clone()
                            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    ),
                    failure_message,
                )
            } else {
                (None, None, None)
            };
        // The saved card token (`card_...`) is the mandate reference, as on
        // the Authorize and SetupMandate responses.
        let mandate_reference = payment.payment_method.map(|card| {
            Box::new(MandateReference {
                connector_mandate_id: Some(card),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(payment.id)),
            status,
            connector_response_reference_id: payment.merchant_reference_id,
            connector_request_reference_id: None,
            mandate_reference,
            error_code,
            error_message,
            error_reason,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
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
        let webhook = parse_webhook_body(&request)?;
        let refund = match typed_webhook_payload(&webhook)? {
            transformers::RapydWebhookPayload::Refund(refund) => refund,
            transformers::RapydWebhookPayload::Payment(_)
            | transformers::RapydWebhookPayload::Dispute(_)
            | transformers::RapydWebhookPayload::Unsupported => {
                return Err(Report::new(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Rapyd webhook does not carry a refund object"));
            }
        };

        let status = common_enums::RefundStatus::from(refund.status);
        // As for payments: the failure fields are empty strings unless the
        // refund failed.
        let (error_code, error_message) = if status == common_enums::RefundStatus::Failure {
            (
                Some(
                    non_empty(refund.failure_code)
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                ),
                Some(
                    non_empty(refund.failure_reason)
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                ),
            )
        } else {
            (None, None)
        };

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: Some(refund.id),
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
        let webhook = parse_webhook_body(&request)?;
        let dispute = match typed_webhook_payload(&webhook)? {
            transformers::RapydWebhookPayload::Dispute(dispute) => dispute,
            transformers::RapydWebhookPayload::Payment(_)
            | transformers::RapydWebhookPayload::Refund(_)
            | transformers::RapydWebhookPayload::Unsupported => {
                return Err(Report::new(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Rapyd webhook does not carry a dispute object"));
            }
        };

        // The status follows the event type: a created dispute is open; an
        // updated one is read from `data.status`. A status with no dispute
        // outcome (`PRA`, `ARB`, `REV`, unknown) is an unsupported event and
        // is never defaulted to one.
        let status = match webhook.webhook_type {
            transformers::RapydWebhookEventType::PaymentDisputeCreated => {
                common_enums::DisputeStatus::DisputeOpened
            }
            transformers::RapydWebhookEventType::PaymentDisputeUpdated => {
                dispute_status(dispute.status)
                    .ok_or_else(|| Report::new(WebhookError::WebhookEventTypeNotFound))
                    .attach_printable("Rapyd dispute status has no mapped dispute outcome")?
            }
            transformers::RapydWebhookEventType::PaymentCompleted
            | transformers::RapydWebhookEventType::PaymentCaptured
            | transformers::RapydWebhookEventType::PaymentFailed
            | transformers::RapydWebhookEventType::PaymentSucceeded
            | transformers::RapydWebhookEventType::PaymentCanceled
            | transformers::RapydWebhookEventType::PaymentExpired
            | transformers::RapydWebhookEventType::PaymentUpdated
            | transformers::RapydWebhookEventType::RefundCompleted
            | transformers::RapydWebhookEventType::PaymentRefundFailed
            | transformers::RapydWebhookEventType::PaymentRefundRejected
            | transformers::RapydWebhookEventType::PaymentRefundUpdated
            | transformers::RapydWebhookEventType::CardAddedSuccessfully
            | transformers::RapydWebhookEventType::Unknown => {
                return Err(Report::new(WebhookError::WebhookEventTypeNotFound));
            }
        };

        // `data.amount` is read in minor units and passed through the shared
        // converter, as the reference implementation does.
        let amount = StringMinorUnitForConnector
            .convert(dispute.amount, dispute.currency)
            .map_err(|error| {
                Report::new(WebhookError::WebhookAmountConversionFailed {
                    reason: error.to_string(),
                })
            })?;

        Ok(DisputeWebhookDetailsResponse {
            amount,
            currency: dispute.currency,
            dispute_id: dispute.token,
            status,
            stage: common_enums::DisputeStage::Dispute,
            connector_response_reference_id: None,
            dispute_message: dispute.dispute_reason_description.clone(),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            connector_reason_code: dispute.dispute_reason_description,
            additional_details: None,
        })
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let webhook = parse_webhook_body(&request)?;
        let resource: Box<dyn hyperswitch_masking::ErasedMaskSerialize> =
            match typed_webhook_payload(&webhook)? {
                transformers::RapydWebhookPayload::Payment(payment) => Box::new(payment),
                transformers::RapydWebhookPayload::Refund(refund) => Box::new(refund),
                transformers::RapydWebhookPayload::Dispute(dispute) => Box::new(dispute),
                transformers::RapydWebhookPayload::Unsupported => {
                    return Err(Report::new(WebhookError::WebhookResourceObjectNotFound));
                }
            };
        Ok(resource)
    }
}

/// Parses the inbound webhook body.
fn parse_webhook_body(
    request: &RequestDetails,
) -> Result<transformers::RapydIncomingWebhook, Report<WebhookError>> {
    request
        .body
        .parse_struct("RapydIncomingWebhook")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("Failed to parse the Rapyd webhook body")
}

/// Types `data` by the event `type`; a `data` that does not have the shape
/// its `type` requires is a decoding failure, never a defaulted value.
fn typed_webhook_payload(
    webhook: &transformers::RapydIncomingWebhook,
) -> Result<transformers::RapydWebhookPayload, Report<WebhookError>> {
    webhook
        .payload()
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("Rapyd webhook `data` does not match its `type`")
}

/// The webhook secret is the JSON object `{access_key, secret_key}`. Anything
/// else cannot verify a Rapyd signature.
fn parse_webhook_secret(
    connector_webhook_secret: &ConnectorWebhookSecrets,
) -> Result<transformers::RapydWebhookSecret, Report<WebhookError>> {
    connector_webhook_secret
        .secret
        .parse_struct("RapydWebhookSecret")
        .change_context(WebhookError::WebhookVerificationSecretNotFound)
        .attach_printable(
            "Rapyd webhook secret must be a JSON object with `access_key` and `secret_key`",
        )
}

/// HTTP header names are case-insensitive and the caller forwards them as
/// received.
fn get_webhook_header<'a>(request: &'a RequestDetails, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// The URL Rapyd signed: "the entire URL that was configured ... to receive
/// webhooks". The caller forwards either that absolute URL or only its path;
/// a path is completed with the `host` header, as the reference does.
fn get_webhook_url(request: &RequestDetails) -> Result<String, Report<WebhookError>> {
    let uri = request
        .uri
        .as_deref()
        .filter(|uri| !uri.is_empty())
        .ok_or_else(|| Report::new(WebhookError::WebhookMissingRequiredField { field: "uri" }))?;
    if uri.starts_with("https://") || uri.starts_with("http://") {
        Ok(uri.to_string())
    } else {
        let host = get_webhook_header(request, "host")
            .filter(|host| !host.is_empty())
            .ok_or_else(|| {
                Report::new(WebhookError::WebhookMissingRequiredField { field: "host" })
            })?;
        Ok(format!("https://{host}{uri}"))
    }
}

/// Dispute `data.status` to dispute status, one arm per documented value.
fn dispute_status(
    status: transformers::RapydWebhookDisputeStatus,
) -> Option<common_enums::DisputeStatus> {
    match status {
        // `ACT` | dispute opened
        transformers::RapydWebhookDisputeStatus::Active => {
            Some(common_enums::DisputeStatus::DisputeOpened)
        }
        // `RVW` | dispute challenged
        transformers::RapydWebhookDisputeStatus::Review => {
            Some(common_enums::DisputeStatus::DisputeChallenged)
        }
        // `LOS` | dispute lost
        transformers::RapydWebhookDisputeStatus::Lose => {
            Some(common_enums::DisputeStatus::DisputeLost)
        }
        // `WIN` | dispute won
        transformers::RapydWebhookDisputeStatus::Win => {
            Some(common_enums::DisputeStatus::DisputeWon)
        }
        // `PRA`, `ARB`, `REV` and any other value | event not supported
        transformers::RapydWebhookDisputeStatus::PreArbitration
        | transformers::RapydWebhookDisputeStatus::Arbitration
        | transformers::RapydWebhookDisputeStatus::Reversed
        | transformers::RapydWebhookDisputeStatus::Unknown => None,
    }
}

/// Rapyd sends absent text fields as empty strings.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Rapyd<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Rapyd<T>
{
    fn id(&self) -> &'static str {
        "rapyd"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = RapydAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;

        // Return basic auth headers - signature will be added in get_headers method
        Ok(vec![(
            "access_key".to_string(),
            auth.access_key.into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.rapyd.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: Result<RapydErrorResponse, Report<common_utils::errors::ParsingError>> =
            res.response.parse_struct("rapyd ErrorResponse");

        match response {
            Ok(response_data) => {
                with_error_response_body!(event_builder, response_data);
                let typed = macros::serialize_typed_connector_payload(
                    &response_data,
                    "typed_connector_response",
                );
                let RapydErrorResponse { status, data } = response_data;
                // Rapyd sends empty strings where a value is absent; an empty
                // value is never reported as the error code, message or id.
                let non_empty = |value: Option<String>| value.filter(|v| !v.is_empty());
                let (failure_code, failure_message, payment_id) = data
                    .map(|d| (d.failure_code, d.failure_message, d.id))
                    .unwrap_or((None, None, None));
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: non_empty(status.error_code)
                        .or_else(|| non_empty(failure_code))
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: non_empty(status.status)
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: non_empty(status.message).or_else(|| non_empty(failure_message)),
                    // Flow-neutral: each create call decides through
                    // `build_error_response_with_status`.
                    attempt_status: None,
                    connector_transaction_id: non_empty(payment_id),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
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
                domain_types::utils::handle_json_response_deserialization_failure(res, "rapyd")
            }
        }
    }
}

macros::create_all_prerequisites!(
    connector_name: Rapyd,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: RapydPaymentsRequest<T>,
            response_body: RapydAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: RapydPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: CaptureRequest,
            response_body: RapydCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: RapydVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: RapydRefundRequest,
            response_body: RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: RapydRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: RapydClientAuthRequest,
            response_body: RapydClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: CreateOrder,
            request_body: RapydCreateOrderRequest,
            response_body: RapydCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: SetupMandate,
            request_body: RapydSetupMandateRequest<T>,
            response_body: RapydSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: RapydRepeatPaymentRequest<T>,
            response_body: RapydRepeatPaymentResponse,
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
            http_method: &str,
            url_path: &str,
            body: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let auth = RapydAuthType::try_from(&req.connector_config)?;
            let timestamp = common_utils::date_time::now_unix_timestamp();
            let salt = common_utils::crypto::generate_cryptographically_secure_random_string(12);

            let signature = self.generate_signature(
                &auth,
                http_method,
                url_path,
                body,
                timestamp,
                &salt,
            )?;

            let headers = vec![
                (headers::CONTENT_TYPE.to_string(), "application/json".to_string().into()),
                ("access_key".to_string(), auth.access_key.into_masked()),
                ("salt".to_string(), salt.into()),
                ("timestamp".to_string(), timestamp.to_string().into()),
                ("signature".to_string(), signature.into()),
            ];
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.rapyd.base_url
        }

        pub fn generate_signature(
            &self,
            auth: &RapydAuthType,
            http_method: &str,
            url_path: &str,
            body: &str,
            timestamp: i64,
            salt: &str,
        ) -> CustomResult<String, IntegrationError> {
            let RapydAuthType {
            access_key,
            secret_key
} = auth;
        let to_sign = format!(
            "{http_method}{url_path}{salt}{timestamp}{}{}{body}",
            access_key.peek(),
            secret_key.peek()
        );
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.peek().as_bytes());
        let tag = hmac::sign(&key, to_sign.as_bytes());
        let hmac_sign = hex::encode(tag);
        let signature_value = BASE64_ENGINE_URL_SAFE.encode(hmac_sign);
        Ok(signature_value)
        }

        /// Error response of a call that creates an object (payment, refund).
        /// A Rapyd 4xx carrying an error code is a terminal answer about that
        /// object, so the flow's failure status is attached. 401 (signature or
        /// timestamp), 429, 5xx and bodies without a Rapyd error code say
        /// nothing about the object and keep `attempt_status: None`.
        pub fn build_error_response_with_status(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
            status: FlowStatus,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let http_status = res.status_code;
            let error_response = self.build_error_response(res, event_builder, connector_config)?;
            let is_terminal_client_error =
                (400..500).contains(&http_status) && http_status != 401 && http_status != 429;
            if is_terminal_client_error && error_response.code != common_utils::consts::NO_ERROR_CODE {
                Ok(ErrorResponse {
                    attempt_status: Some(status),
                    ..error_response
                })
            } else {
                Ok(error_response)
            }
        }

        /// A caller-supplied id that goes into a URL path or a request body:
        /// returned unchanged when it is not blank, refused otherwise so a blank
        /// id never reaches Rapyd as a different route.
        pub fn require_non_blank_id(
            &self,
            value: &str,
            field_name: &'static str,
        ) -> CustomResult<String, IntegrationError> {
            if value.trim().is_empty() {
                Err(Report::new(IntegrationError::MissingRequiredField {
                    field_name,
                    context: crate::utils::integration_ctx(
                        format!("rapyd: `{field_name}` is empty or whitespace-only"),
                        format!("Provide the non-empty `{field_name}` returned by Rapyd for this object."),
                    ),
                }))
            } else {
                Ok(value.to_string())
            }
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydPaymentsRequest),
    curl_response: RapydAuthorizeResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            // Get the exact request body that will be sent
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // A Rapyd 4xx on Create Payment is a decline of the payment being created.
            self.build_error_response_with_status(
                res,
                event_builder,
                connector_config,
                FlowStatus::Payment(common_enums::AttemptStatus::Failure),
            )
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_response: RapydPSyncResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = "";
            self.build_headers(req, "get", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let id = req.request.get_connector_transaction_id()?;
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(CaptureRequest),
    curl_response: RapydCaptureResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // A `ResponseId` that is not a connector transaction id is the same
            // missing field as a blank one.
            let id = req.request.get_connector_transaction_id().change_context(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                    context: crate::utils::integration_ctx(
                        "rapyd capture: the request carries no connector transaction id",
                        "Provide the `connector_transaction_id` returned by Rapyd when the payment was authorized.",
                    ),
                },
            )?;
            let id = self.require_non_blank_id(&id, "connector_transaction_id")?;
            Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_response: RapydVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Delete,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = "";
            self.build_headers(req, "delete", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // The id is the URL path segment: a blank one would address
            // `DELETE /v1/payments/`, so it is refused before any request is built.
            let id = self.require_non_blank_id(
                &req.request.connector_transaction_id,
                "connector_transaction_id",
            )?;
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydRefundRequest),
    curl_response: RefundResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_refunds(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/refunds", self.connector_base_url_refunds(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // A Rapyd 4xx on Create Refund is a refusal of the refund being created.
            self.build_error_response_with_status(
                res,
                event_builder,
                connector_config,
                FlowStatus::Refund(common_enums::RefundStatus::Failure),
            )
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_response: RapydRSyncResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_refunds(req))
                .unwrap_or(&url);
            let body = "";
            self.build_headers(req, "get", url_path, body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // The id is the URL path segment: a blank one would address
            // `GET /v1/refunds/` (List Refunds), so it is refused before any
            // request is built.
            let id = self.require_non_blank_id(
                &req.request.connector_refund_id,
                "connector_refund_id",
            )?;
            Ok(format!("{}/v1/refunds/{}", self.connector_base_url_refunds(req), id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(RapydCreateOrderRequest),
    curl_response: RapydCreateOrderResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_payments(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/checkout", self.connector_base_url_payments(req)))
        }
    }
);

// SetupMandate flow – reuses the standard `/v1/payments` endpoint for
// card-on-file verification. The returned payment id is surfaced as the
// connector_mandate_id for subsequent RepeatPayment (MIT) calls.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydSetupMandateRequest),
    curl_response: RapydSetupMandateResponse,
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
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd SetupMandate: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd SetupMandate: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Reuse /v1/payments — `save_payment_method: true` + inline customer
            // object in the body yields a reusable `card_*` token without
            // requiring the complete_payment_url whitelist that the
            // /v1/customers endpoint enforces on sandbox accounts.
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // A Rapyd 4xx on Create Payment is a decline of the payment being created.
            self.build_error_response_with_status(
                res,
                event_builder,
                connector_config,
                FlowStatus::Payment(common_enums::AttemptStatus::Failure),
            )
        }
    }
);

// RepeatPayment (MIT) – Rapyd has no dedicated recurring endpoint. It reuses
// `/v1/payments` but substitutes the card object with a stored
// `payment_method` token (the card_* id returned by SetupMandate) paired
// with `initiation_type: unscheduled`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Rapyd,
    curl_request: Json(RapydRepeatPaymentRequest),
    curl_response: RapydRepeatPaymentResponse,
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
            let url = self.get_url(req)?;
            // HMAC SHA-256 signs only the path component (everything after
            // the base URL). Falling back to the full URL here would produce
            // a signature Rapyd cannot verify, so treat a missing prefix as
            // a hard error instead of silently signing the wrong input.
            let url_path = url
                .strip_prefix(self.connector_base_url_payments(req))
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd RepeatPayment: computed URL did not start with the configured base URL; HMAC signature requires the exact path component"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?;
            let body = self
                .get_request_body(req)?
                .ok_or(IntegrationError::RequestEncodingFailed {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "rapyd RepeatPayment: request body is required for HMAC signing"
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                })?
                .content
                .get_inner_value()
                .expose();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // A Rapyd 4xx on Create Payment is a decline of the payment being created.
            self.build_error_response_with_status(
                res,
                event_builder,
                connector_config,
                FlowStatus::Payment(common_enums::AttemptStatus::Failure),
            )
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Rapyd,
    curl_request: Json(RapydClientAuthRequest),
    curl_response: RapydClientAuthResponse,
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
            let url = self.get_url(req)?;
            let url_path = url.strip_prefix(self.connector_base_url_merchant_auth(req))
                .unwrap_or(&url);
            let body = self.get_request_body(req)?
                .map(|content| content.content.get_inner_value().expose())
                .unwrap_or_default();
            self.build_headers(req, "post", url_path, &body)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/checkout", self.connector_base_url_merchant_auth(req)))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Rapyd,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        PaymentMethodToken,
        SubmitEvidence,
        DefendDispute,
        Accept,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
    ],
);
