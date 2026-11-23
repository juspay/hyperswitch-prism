use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::types::MinorUnit;
use domain_types::connector_types::ResponseId;
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// INCOMING WEBHOOKS
// Ported 1:1 from hyperswitch `crates/hyperswitch_connectors/src/connectors/finix.rs`
// (`impl IncomingWebhook for Finix`) and `finix/transformers/response.rs`
// (`FinixWebhookBody::{get_webhook_object_reference_id, get_webhook_event_type,
// get_dispute_details}`). Hyperswitch (Direct gateway) is the source of truth.
// =============================================================================

use crate::connectors::finix::transformers::{FinixNetworkDetails, FinixTags};
use domain_types::connector_types::{
    DisputeWebhookDetailsResponse, DisputeWebhookReference, EventType, PaymentWebhookReference,
    RefundWebhookDetailsResponse, RefundWebhookReference, WebhookDetailsResponse,
    WebhookResourceReference,
};
use domain_types::errors::WebhookError;
use time::PrimitiveDateTime;

/// Webhook transaction state. Mirrors HS `FinixState` (transformers/request.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixState {
    Pending,
    Succeeded,
    Failed,
    Canceled,
    #[serde(other)]
    Unknown,
}

/// Webhook transfer type. Mirrors HS `FinixPaymentType` (transformers/request.rs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixPaymentType {
    Debit,
    Credit,
    Reversal,
    Fee,
    Adjustment,
    Dispute,
    Reserve,
    Settlement,
    #[serde(other)]
    Unknown,
}

/// Mirrors HS `FinixThreeDSecure` (transformers/request.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixThreeDSecure {
    pub cardholder_authentication: Secret<String>,
    pub electronic_commerce_indicator: String,
    pub transaction_id: Option<String>,
}

/// Webhook payment resource (authorization or transfer). Mirrors HS
/// `FinixPaymentsResponse` (transformers/response.rs) field-for-field so that
/// (de)serialization succeeds/fails on exactly the same payloads.
/// Named `FinixWebhookPaymentsResponse` because prism already has a different
/// `FinixPaymentsResponse` for the sync/authorize API responses.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixWebhookPaymentsResponse {
    pub id: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub application: Option<Secret<String>>,
    pub amount: MinorUnit,
    pub captured_amount: Option<MinorUnit>,
    pub currency: Currency,
    pub is_void: Option<bool>,
    pub source: Option<Secret<String>>,
    pub state: FinixState,
    pub failure_code: Option<String>,
    pub messages: Option<Vec<String>>,
    pub failure_message: Option<String>,
    pub transfer: Option<String>,
    pub tags: FinixTags,
    #[serde(rename = "type")]
    pub payment_type: Option<FinixPaymentType>,
    pub three_d_secure: Option<FinixThreeDSecure>,
    pub address_verification: Option<String>,
    pub network_details: Option<FinixNetworkDetails>,
}

/// Mirrors HS `FinixDisputeState` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinixDisputeState {
    Inquiry,
    Pending,
    Lost,
    Won,
}

/// Mirrors HS `FinixDisputes` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixDisputes {
    pub transfer: String,
    pub reason: Option<String>,
    pub amount: MinorUnit,
    pub state: FinixDisputeState,
    pub currency: Currency,
    pub id: String,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub created_at: Option<PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub updated_at: Option<PrimitiveDateTime>,
    #[serde(default, with = "common_utils::custom_serde::iso8601::option")]
    pub respond_by: Option<PrimitiveDateTime>,
}

/// Mirrors HS `SingleEventType` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SingleEventType<T>(Vec<T>);

impl<T: Clone> SingleEventType<T> {
    pub fn get_first_event(&self) -> Result<T, error_stack::Report<WebhookError>> {
        self.0
            .first()
            .cloned()
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookBodyDecodingFailed))
    }
}

/// Mirrors HS `FinixEmbedded` (transformers/response.rs). Untagged: variant order
/// (Authorizations, Transfers, Disputes) matches HS so ambiguous payloads resolve
/// identically.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FinixEmbedded {
    Authorizations {
        authorizations: SingleEventType<FinixWebhookPaymentsResponse>,
    },
    Transfers {
        transfers: SingleEventType<FinixWebhookPaymentsResponse>,
    },
    Disputes {
        disputes: SingleEventType<FinixDisputes>,
    },
}

/// Mirrors HS `FinixWebhookBody` (transformers/response.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinixWebhookBody {
    #[serde(rename = "type")]
    pub webhook_type: String,
    pub entity: String,
    #[serde(rename = "_embedded")]
    pub webhook_embedded: FinixEmbedded,
}

/// Parsed `Finix-Signature` header. Mirrors HS `FinixWebhookSignature`
/// (`sig` kept as the hex string; hex-decoded at the verification site).
#[derive(Debug)]
pub struct FinixWebhookSignature {
    pub timestamp: String,
    pub sig: String,
}

/// HS treats an empty body or a body that trims to `{}` as Finix's
/// endpoint-verification probe (finix.rs `get_webhook_event_type`). HS's
/// `SetupWebhook` arm is unreachable there (`is_test_webhook` only matches an
/// exact `"{}"` body, which this check already catches), so it is not ported.
pub(crate) fn is_endpoint_verification_body(body: &[u8]) -> bool {
    let trimmed_body = std::str::from_utf8(body)
        .map(|string| string.trim())
        .unwrap_or("");
    body.is_empty() || trimmed_body == "{}"
}

pub(crate) fn parse_finix_webhook_body(
    body: &[u8],
) -> Result<FinixWebhookBody, error_stack::Report<WebhookError>> {
    serde_json::from_slice::<FinixWebhookBody>(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("failed to deserialize the Finix webhook body (FinixWebhookBody)")
}

/// Parses the comma-separated `key=value` pairs of the `Finix-Signature` header.
/// Ports HS `decode_finix_signature` (finix.rs); header lookup is
/// case-insensitive because the gRPC surface transports headers as a plain map.
pub(crate) fn decode_finix_signature(
    headers: &HashMap<String, String>,
) -> Result<FinixWebhookSignature, error_stack::Report<WebhookError>> {
    let security_header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("finix-signature"))
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;

    let map: HashMap<&str, &str> = security_header
        .split(',')
        .map(|kv| {
            let mut parts = kv.trim().splitn(2, '=');
            let key = parts
                .next()
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;
            let value = parts
                .next()
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?;
            Ok((key, value))
        })
        .collect::<Result<_, error_stack::Report<WebhookError>>>()?;

    let timestamp = map
        .get("timestamp")
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?
        .to_string();

    let sig = map
        .get("sig")
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookSignatureNotFound))?
        .to_string();

    Ok(FinixWebhookSignature { timestamp, sig })
}

/// Ports HS `FinixWebhookBody::get_webhook_event_type` arm-for-arm.
/// HS maps a PENDING reversal to `IncomingWebhookEvent::EventNotSupported`;
/// the equivalent here is `EventType::IncomingWebhookEventUnspecified`
/// (proto `WEBHOOK_EVENT_TYPE_UNSPECIFIED` = 0, which the HS UCS client maps
/// back to `EventNotSupported` in `from_ucs_event_type`).
pub(crate) fn get_finix_webhook_event_type(
    body: &FinixWebhookBody,
) -> Result<EventType, error_stack::Report<WebhookError>> {
    match &body.webhook_embedded {
        FinixEmbedded::Authorizations { authorizations } => {
            let authorization = authorizations.get_first_event()?;

            if authorization.is_void == Some(true) {
                match authorization.state {
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::PaymentIntentCancelFailure)
                    }
                    FinixState::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixState::Succeeded => Ok(EventType::PaymentIntentCancelled),
                }
            } else {
                match authorization.state {
                    FinixState::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixState::Succeeded => Ok(EventType::PaymentIntentAuthorizationSuccess),
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::PaymentIntentAuthorizationFailure)
                    }
                }
            }
        }
        FinixEmbedded::Transfers { transfers } => {
            let transfer = transfers.get_first_event()?;

            if transfer.payment_type == Some(FinixPaymentType::Reversal) {
                match transfer.state {
                    FinixState::Succeeded => Ok(EventType::RefundSuccess),
                    // HS: `EventNotSupported` (see doc comment above).
                    FinixState::Pending => Ok(EventType::IncomingWebhookEventUnspecified),
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::RefundFailure)
                    }
                }
            } else {
                match transfer.state {
                    FinixState::Pending => Ok(EventType::PaymentIntentProcessing),
                    FinixState::Succeeded => Ok(EventType::PaymentIntentSuccess),
                    FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                        Ok(EventType::PaymentIntentFailure)
                    }
                }
            }
        }
        FinixEmbedded::Disputes { disputes } => {
            let dispute = disputes.get_first_event()?;

            match dispute.state {
                FinixDisputeState::Pending => Ok(EventType::DisputeOpened),
                FinixDisputeState::Inquiry => Ok(EventType::DisputeChallenged),
                FinixDisputeState::Lost => Ok(EventType::DisputeLost),
                FinixDisputeState::Won => Ok(EventType::DisputeWon),
            }
        }
    }
}

/// Ports HS `FinixWebhookBody::get_webhook_object_reference_id`:
/// - Authorizations -> `PaymentId(ConnectorTransactionId(authorization.id))`
/// - Transfers (REVERSAL) -> `RefundId(ConnectorRefundId(transfer.id))`
/// - Transfers (FEE) -> error (HS: `WebhookEventTypeNotFound`, platform fee ignored)
/// - Transfers (other) -> `PaymentId(ConnectorTransactionId(transfer.id))`
/// - Disputes -> HS emits `PaymentId(ConnectorTransactionId(dispute.transfer))`; the
///   HS reference normaliser maps a prism Dispute reference via
///   `connector_dispute_id.or(connector_transaction_id)` into exactly that, so
///   `connector_dispute_id` MUST stay `None` for byte-parity with the Direct gateway.
pub(crate) fn get_finix_webhook_reference(
    body: &FinixWebhookBody,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    match &body.webhook_embedded {
        FinixEmbedded::Authorizations { authorizations } => {
            let authorization = authorizations.get_first_event()?;

            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(authorization.id),
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                },
            )))
        }
        FinixEmbedded::Transfers { transfers } => {
            let transfer = transfers.get_first_event()?;

            match transfer.payment_type {
                Some(FinixPaymentType::Reversal) => Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(transfer.id),
                        merchant_refund_id: None,
                        connector_transaction_id: None,
                        merchant_transaction_id: None,
                    },
                ))),
                // finix platform fee ignored (HS: Err(WebhookEventTypeNotFound))
                Some(FinixPaymentType::Fee) => Err(error_stack::report!(
                    WebhookError::WebhookReferenceIdNotFound
                )),
                _ => Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(transfer.id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    },
                ))),
            }
        }
        FinixEmbedded::Disputes { disputes } => {
            let dispute = disputes.get_first_event()?;

            Ok(Some(WebhookResourceReference::Dispute(
                DisputeWebhookReference {
                    connector_dispute_id: None,
                    connector_transaction_id: Some(dispute.transfer),
                },
            )))
        }
    }
}

/// Which HS flow a webhook payment resource belongs to (`FinixFlow` in HS).
/// Webhooks only ever carry authorizations or transfers, so HS's `Capture` arm
/// is unreachable from the webhook path and is not ported.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FinixWebhookFlow {
    Auth,
    Transfer,
}

/// Ports HS `get_attempt_status` (finix/transformers.rs) for the webhook path.
fn get_finix_webhook_attempt_status(
    state: &FinixState,
    flow: FinixWebhookFlow,
    is_void: Option<bool>,
) -> AttemptStatus {
    if is_void == Some(true) {
        return match state {
            FinixState::Failed | FinixState::Canceled | FinixState::Unknown => {
                AttemptStatus::VoidFailed
            }
            FinixState::Pending => AttemptStatus::Voided,
            FinixState::Succeeded => AttemptStatus::Voided,
        };
    }
    match (flow, state) {
        (FinixWebhookFlow::Auth, FinixState::Pending) => AttemptStatus::AuthenticationPending,
        (FinixWebhookFlow::Auth, FinixState::Succeeded) => AttemptStatus::Authorized,
        (
            FinixWebhookFlow::Auth,
            FinixState::Failed | FinixState::Canceled | FinixState::Unknown,
        ) => AttemptStatus::AuthorizationFailed,
        (FinixWebhookFlow::Transfer, FinixState::Pending) => AttemptStatus::Pending,
        (FinixWebhookFlow::Transfer, FinixState::Succeeded) => AttemptStatus::Charged,
        (
            FinixWebhookFlow::Transfer,
            FinixState::Failed | FinixState::Canceled | FinixState::Unknown,
        ) => AttemptStatus::Failure,
    }
}

/// Ports HS `impl From<FinixState> for RefundStatus` (finix/transformers.rs).
fn get_finix_webhook_refund_status(state: &FinixState) -> RefundStatus {
    match state {
        FinixState::Pending => RefundStatus::Pending,
        FinixState::Succeeded => RefundStatus::Success,
        FinixState::Failed | FinixState::Canceled | FinixState::Unknown => RefundStatus::Failure,
    }
}

/// Dispute status per the HS webhook event mapping (PENDING -> DisputeOpened,
/// INQUIRY -> DisputeChallenged, LOST -> DisputeLost, WON -> DisputeWon).
fn get_finix_webhook_dispute_status(state: &FinixDisputeState) -> common_enums::DisputeStatus {
    match state {
        FinixDisputeState::Pending => common_enums::DisputeStatus::DisputeOpened,
        FinixDisputeState::Inquiry => common_enums::DisputeStatus::DisputeChallenged,
        FinixDisputeState::Lost => common_enums::DisputeStatus::DisputeLost,
        FinixDisputeState::Won => common_enums::DisputeStatus::DisputeWon,
    }
}

/// Builds the payment webhook content for authorizations and (non-reversal)
/// transfers. Status mapping ports HS `get_attempt_status` with
/// `FinixFlow::Auth` / `FinixFlow::Transfer` respectively.
pub(crate) fn build_finix_payment_webhook_response(
    connector: &dyn common_utils::types::AmountConvertor<Output = MinorUnit>,
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let (resource, flow) = match &body.webhook_embedded {
        FinixEmbedded::Authorizations { authorizations } => {
            (authorizations.get_first_event()?, FinixWebhookFlow::Auth)
        }
        FinixEmbedded::Transfers { transfers } => {
            (transfers.get_first_event()?, FinixWebhookFlow::Transfer)
        }
        FinixEmbedded::Disputes { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a payment webhook, but found a dispute webhook");
        }
    };

    let status = get_finix_webhook_attempt_status(&resource.state, flow, resource.is_void);

    Ok(WebhookDetailsResponse {
        raw_connector_status: None,
        amount: None,
        connector_returned_payment_method_details: None,
        resource_id: Some(ResponseId::ConnectorTransactionId(resource.id.clone())),
        status,
        connector_response_reference_id: Some(resource.id.clone()),
        connector_request_reference_id: Some(resource.id),
        mandate_reference: None,
        error_code: resource.failure_code,
        error_message: resource.failure_message.clone(),
        error_reason: resource.failure_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        captured_money: resource
            .captured_amount
            .map(|amount| {
                crate::utils::webhook_amount_to_money(
                    connector.convert_back(amount, resource.currency),
                    resource.currency,
                )
            })
            .transpose()?,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}

/// Builds the refund webhook content for REVERSAL transfers. Status mapping
/// ports HS `impl From<FinixState> for RefundStatus`.
pub(crate) fn build_finix_refund_webhook_response(
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let transfer = match &body.webhook_embedded {
        FinixEmbedded::Transfers { transfers } => transfers.get_first_event()?,
        FinixEmbedded::Authorizations { .. } | FinixEmbedded::Disputes { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a refund (reversal transfer) webhook");
        }
    };

    Ok(RefundWebhookDetailsResponse {
        raw_connector_status: None,
        connector_refund_id: Some(transfer.id),
        // Finix REVERSAL webhooks only carry connector-side ids — no merchant
        // reference for the original payment.
        merchant_transaction_id: None,
        status: get_finix_webhook_refund_status(&transfer.state),
        connector_response_reference_id: None,
        error_code: transfer.failure_code,
        error_message: transfer.failure_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

/// Builds the dispute webhook content. Ports HS
/// `FinixWebhookBody::get_dispute_details`: amount converted with the HS
/// webhook amount converter (`StringMinorUnitForConnector`), stage is always
/// `Dispute`, reason surfaced as the dispute message.
pub(crate) fn build_finix_dispute_webhook_response(
    body: &FinixWebhookBody,
    raw_body: &[u8],
) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let dispute = match &body.webhook_embedded {
        FinixEmbedded::Disputes { disputes } => disputes.get_first_event()?,
        FinixEmbedded::Authorizations { .. } | FinixEmbedded::Transfers { .. } => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("expected a dispute webhook, but found other webhooks");
        }
    };

    Ok(DisputeWebhookDetailsResponse {
        amount: domain_types::utils::convert_amount_for_webhook(
            &common_utils::types::StringMinorUnitForConnector,
            &domain_types::utils::convert_back_amount_to_money_for_webhook(
                &common_utils::types::MinorUnitForConnector,
                dispute.amount,
                dispute.currency,
            )?,
        )?,
        currency: dispute.currency,
        dispute_id: dispute.id,
        status: get_finix_webhook_dispute_status(&dispute.state),
        stage: common_enums::DisputeStage::Dispute,
        connector_response_reference_id: None,
        dispute_message: dispute.reason,
        connector_reason_code: None,
        additional_details: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
