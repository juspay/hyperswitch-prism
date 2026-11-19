use common_enums::{AttemptStatus, RefundStatus};
use domain_types::{
    connector_types::{
        EventType, MandateReference, PaymentWebhookReference, RefundWebhookDetailsResponse,
        RefundWebhookReference, ResponseId, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{self},
};
use error_stack::ResultExt;
use hyperswitch_masking::ExposeInterface;
use serde::{Deserialize, Serialize};

use crate::connectors::ecommpay::transformers::{
    error_from_operation, map_payment_status, map_refund_operation_status, non_empty,
    EcommpayOperation, EcommpayOperationStatus, EcommpayOperationType, EcommpayPaymentStatus,
    EcommpayPaymentType, EcommpayStatusResponse,
};

// =============================================================================
// Callbacks
// =============================================================================
//
// Ecommpay posts three unrelated body shapes to the merchant: payment/operation callbacks
// (signed with a top-level `signature`), card-token callbacks (`general` + `request`, signed
// under `general.signature`) and chargeback callbacks (a separate channel, unsigned by design).
// A callback carries no event name: the event is `operation.type` + `operation.status`.

/// HTTP status recorded on the details of a callback (the callback itself is the response).
const CALLBACK_STATUS_CODE: u16 = 200;

/// `event` of a chargeback callback (techspec "Chargeback callbacks").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EcommpayChargebackEvent {
    NewChargebacksSummary,
    NewPreArbitrationSummary,
    NewArbitrationSummary,
    NewChargebackDetails,
    NewPreArbitrationDetails,
    NewArbitrationDetails,
    ChargebackCancelledByIssuer,
    ChargebackLost,
    ChargebackWon,
    #[serde(other)]
    Unknown,
}

/// Chargeback callback: only the event is read. Its `chargebacks[].operation_id` is an
/// Ecommpay operation id, not a payment id, so it is never used as a reference.
#[derive(Debug, Clone, Deserialize)]
pub struct EcommpayChargebackCallback {
    pub event: EcommpayChargebackEvent,
}

/// Card-token callback (`request.action` `tokenize` / `token_revoke`); nothing in it is read.
#[derive(Debug, Clone, Deserialize)]
pub struct EcommpayTokenCallback {
    pub general: serde::de::IgnoredAny,
    pub request: serde::de::IgnoredAny,
}

/// Every body Ecommpay posts to a merchant callback URL. Order matters: a chargeback is the
/// only shape with `event`, a token callback the only one with `general` + `request`; the
/// payment callback is the status-response shape with a singular `operation`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum EcommpayWebhookBody {
    Chargeback(EcommpayChargebackCallback),
    Token(EcommpayTokenCallback),
    Payment(Box<EcommpayStatusResponse>),
}

pub fn parse_webhook_body(
    body: &[u8],
) -> Result<EcommpayWebhookBody, error_stack::Report<errors::WebhookError>> {
    serde_json::from_slice::<EcommpayWebhookBody>(body)
        .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
        .attach_printable(
            "Ecommpay callback is neither a payment, a token nor a chargeback callback",
        )
}

/// The outcome class of an operation status.
///
/// `ActionRequired` is Ecommpay's "waiting for the customer": `awaiting 3ds result`,
/// `awaiting redirect result`, `awaiting customer action` and `awaiting merchant auth` (the
/// customer must authenticate, or finish a step at the payment system). `Processing` is
/// everything still in flight where nothing is asked of the customer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationOutcome {
    Succeeded,
    Failed,
    ActionRequired,
    Processing,
}

/// Exhaustive on purpose: a new upstream status must be classified, never absorbed.
fn operation_outcome(status: EcommpayOperationStatus) -> OperationOutcome {
    match status {
        EcommpayOperationStatus::Success => OperationOutcome::Succeeded,
        EcommpayOperationStatus::Decline | EcommpayOperationStatus::Error => {
            OperationOutcome::Failed
        }
        EcommpayOperationStatus::Awaiting3dsResult
        | EcommpayOperationStatus::AwaitingRedirectResult
        | EcommpayOperationStatus::AwaitingCustomerAction
        | EcommpayOperationStatus::AwaitingMerchantAuth => OperationOutcome::ActionRequired,
        EcommpayOperationStatus::Processing
        | EcommpayOperationStatus::AwaitingClarification
        | EcommpayOperationStatus::AwaitingCustomer
        | EcommpayOperationStatus::AwaitingCapture => OperationOutcome::Processing,
        // Undocumented: classified as in progress, never as a terminal state.
        EcommpayOperationStatus::Unknown => {
            tracing::warn!(
                "Ecommpay: callback carries an undocumented operation.status; treated as \
                 processing"
            );
            OperationOutcome::Processing
        }
    }
}

/// `refund` and `reversal` are the two refund operations (a reversal is a same-day refund).
pub fn is_refund_operation(operation: &EcommpayOperation) -> bool {
    matches!(
        operation.operation_type,
        EcommpayOperationType::Refund | EcommpayOperationType::Reversal
    )
}

/// Whether the callback is about an individual COF debit rather than the payment that
/// registered the series.
pub fn is_recurring_operation(operation: &EcommpayOperation) -> bool {
    matches!(operation.operation_type, EcommpayOperationType::Recurring)
}

/// Event of a payment/operation callback, by `operation.type` + `operation.status` (techspec
/// "Webhook Events"). Exhaustive on purpose: no catch-all arm.
fn operation_event_type(operation: &EcommpayOperation) -> EventType {
    let outcome = operation_outcome(operation.status);
    match operation.operation_type {
        EcommpayOperationType::Sale
        | EcommpayOperationType::Recurring
        | EcommpayOperationType::AccountVerification => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentSuccess,
            OperationOutcome::Failed => EventType::PaymentIntentFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Auth => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentAuthorizationSuccess,
            OperationOutcome::Failed => EventType::PaymentIntentAuthorizationFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Capture => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentCaptureSuccess,
            OperationOutcome::Failed => EventType::PaymentIntentCaptureFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Cancel => match outcome {
            OperationOutcome::Succeeded => EventType::PaymentIntentCancelled,
            OperationOutcome::Failed => EventType::PaymentIntentCancelFailure,
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        // Review rule 1: a refund callback is mapped by its operation status, never by the
        // operation name alone.
        EcommpayOperationType::Refund | EcommpayOperationType::Reversal => match outcome {
            OperationOutcome::Succeeded => EventType::RefundSuccess,
            OperationOutcome::Failed => EventType::RefundFailure,
            OperationOutcome::ActionRequired | OperationOutcome::Processing => {
                EventType::RefundProcessing
            }
        },
        // No event row in the techspec for these operations: they never move the payment to a
        // terminal state from a callback.
        EcommpayOperationType::Incremental
        | EcommpayOperationType::PaymentConfirmation
        | EcommpayOperationType::RecurringCancel => match outcome {
            OperationOutcome::ActionRequired => EventType::PaymentActionRequired,
            OperationOutcome::Succeeded
            | OperationOutcome::Failed
            | OperationOutcome::Processing => EventType::PaymentIntentProcessing,
        },
        EcommpayOperationType::Unknown => {
            tracing::warn!(
                "Ecommpay: callback carries an undocumented operation.type; treated as \
                 processing"
            );
            EventType::PaymentIntentProcessing
        }
    }
}

/// Chargeback callbacks become dispute events; they are unsigned and carry no reference.
fn chargeback_event_type(event: EcommpayChargebackEvent) -> EventType {
    match event {
        EcommpayChargebackEvent::NewChargebacksSummary
        | EcommpayChargebackEvent::NewPreArbitrationSummary
        | EcommpayChargebackEvent::NewArbitrationSummary
        | EcommpayChargebackEvent::NewChargebackDetails
        | EcommpayChargebackEvent::NewPreArbitrationDetails
        | EcommpayChargebackEvent::NewArbitrationDetails => EventType::DisputeOpened,
        EcommpayChargebackEvent::ChargebackWon => EventType::DisputeWon,
        EcommpayChargebackEvent::ChargebackLost => EventType::DisputeLost,
        EcommpayChargebackEvent::ChargebackCancelledByIssuer => EventType::DisputeCancelled,
        EcommpayChargebackEvent::Unknown => {
            tracing::warn!(
                "Ecommpay: chargeback callback carries an undocumented event; left unspecified"
            );
            EventType::IncomingWebhookEventUnspecified
        }
    }
}

/// Event type of any callback body (ParseEvent phase, stateless).
pub fn webhook_event_type(body: &EcommpayWebhookBody) -> EventType {
    match body {
        EcommpayWebhookBody::Chargeback(callback) => chargeback_event_type(callback.event),
        EcommpayWebhookBody::Token(_) => EventType::IncomingWebhookEventUnspecified,
        EcommpayWebhookBody::Payment(response) => match response.operation.as_ref() {
            Some(operation) => operation_event_type(operation),
            None => {
                tracing::warn!(
                    "Ecommpay: payment callback carries no operation object; treated as \
                     processing"
                );
                EventType::PaymentIntentProcessing
            }
        },
    }
}

fn callback_payment_id(
    response: &EcommpayStatusResponse,
) -> Result<String, error_stack::Report<errors::WebhookError>> {
    non_empty(response.payment.id.clone()).ok_or_else(|| {
        error_stack::report!(errors::WebhookError::WebhookReferenceIdNotFound)
            .attach_printable("Ecommpay payment callback carries no payment.id")
    })
}

/// The refund operation of a callback and its `request_id` (the connector refund id that the
/// refund acceptance returned).
fn callback_refund_operation(
    response: &EcommpayStatusResponse,
) -> Result<(&EcommpayOperation, String), error_stack::Report<errors::WebhookError>> {
    let operation = response
        .operation
        .as_ref()
        .filter(|operation| is_refund_operation(operation))
        .ok_or_else(|| {
            error_stack::report!(errors::WebhookError::WebhookResourceObjectNotFound)
                .attach_printable("Ecommpay callback carries no refund or reversal operation")
        })?;
    let connector_refund_id = non_empty(operation.request_id.clone()).ok_or_else(|| {
        error_stack::report!(errors::WebhookError::WebhookReferenceIdNotFound)
            .attach_printable("Ecommpay refund callback carries no operation.request_id")
    })?;
    Ok((operation, connector_refund_id))
}

/// Reference of a callback (ParseEvent phase). Payment callbacks are keyed by `payment.id`
/// (the merchant `general.payment_id`, which UCS returns as connector_transaction_id); refund
/// callbacks by `operation.request_id`, with `payment.id` kept as the payment it belongs to.
/// Chargeback and token callbacks have no reference.
pub fn webhook_reference(
    body: &EcommpayWebhookBody,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
    match body {
        EcommpayWebhookBody::Chargeback(_) | EcommpayWebhookBody::Token(_) => Ok(None),
        EcommpayWebhookBody::Payment(response) => {
            let payment_id = callback_payment_id(response)?;
            let is_refund = response.operation.as_ref().is_some_and(is_refund_operation);
            if is_refund {
                let (_, connector_refund_id) = callback_refund_operation(response)?;
                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(connector_refund_id),
                        merchant_refund_id: None,
                        connector_transaction_id: Some(payment_id),
                        merchant_transaction_id: None,
                    },
                )))
            } else {
                Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(payment_id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    },
                )))
            }
        }
    }
}

/// Payment callback details (HandleEvent phase). The status comes from the shared
/// `payment.status` table, exactly as PSync reads it; a declined or failed operation keeps
/// its own code, message and provider result.
pub fn payment_webhook_details(
    response: &EcommpayStatusResponse,
) -> Result<WebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
    let payment_id = callback_payment_id(response)?;
    // `payment.status` describes the payment as a whole, and a COF series sits at
    // `scheduled recurring processing` while individual debits come and go -- so for a
    // recurring-operation callback it cannot say whether *this* debit was taken. Ecommpay
    // documents a declined `recurring` operation under that payment status, which the
    // payment-level table would have reported as `Charged` while the event derived from the
    // same operation is `PaymentIntentFailure`. The operation is this attempt, so it decides.
    // The registration payment (sale / auth / account verification) keeps the payment status,
    // which is also what PSync of that payment returns.
    let recurring_operation = response
        .operation
        .as_ref()
        .filter(|operation| is_recurring_operation(operation));
    let status = match recurring_operation {
        Some(operation) => match operation_outcome(operation.status) {
            OperationOutcome::Succeeded => AttemptStatus::Charged,
            OperationOutcome::Failed => AttemptStatus::Failure,
            OperationOutcome::ActionRequired => AttemptStatus::AuthenticationPending,
            OperationOutcome::Processing => AttemptStatus::Pending,
        },
        None => map_payment_status(response.payment.status),
    };
    let error = response
        .operation
        .as_ref()
        .filter(|operation| operation_outcome(operation.status) == OperationOutcome::Failed)
        .map(|operation| {
            error_from_operation(
                operation,
                &payment_id,
                CALLBACK_STATUS_CODE,
                Some(AttemptStatus::Failure),
            )
        });
    // Same rule as PSync: the platform-stored credential of a verification or of a purchase
    // that registered a COF series.
    let registers_credential = response.payment.payment_type
        == Some(EcommpayPaymentType::AccountVerification)
        || response.payment.status == EcommpayPaymentStatus::Success;
    let mandate_reference = response
        .recurring
        .as_ref()
        .filter(|_| registers_credential)
        .map(|recurring| {
            Box::new(MandateReference {
                connector_mandate_id: Some(recurring.id.to_string()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });
    let network_txn_id = non_empty(
        response
            .payment
            .scheme_id
            .clone()
            .map(ExposeInterface::expose),
    );
    let (error_code, error_message, error_reason) = match error {
        Some(error) => (Some(error.code), Some(error.message), error.reason),
        None => (None, None, None),
    };
    // `payment.sum` is the amount currently standing on the payment, so on a charged payment it
    // is what was actually captured -- a partial capture reports the captured part rather than
    // the full authorised amount. It is NOT the captured amount once money has been sent back:
    // a refund reduces `payment.sum` while `payment.status` stays `success`, so a refund
    // operation is excluded instead of under-reporting the capture.
    // `payment.sum` is the parent payment's standing amount, so it is not an individual COF
    // debit's captured amount either: a recurring-operation callback reports none.
    let settles_capture = status == AttemptStatus::Charged
        && recurring_operation.is_none()
        && !response.operation.as_ref().is_some_and(is_refund_operation);
    let amount_captured = response
        .payment
        .sum
        .as_ref()
        .filter(|_| settles_capture)
        .map(|sum| {
            domain_types::utils::convert_back_amount_to_money_for_webhook(
                &common_utils::types::MinorUnitForConnector,
                sum.amount,
                sum.currency,
            )
        })
        .transpose()?;
    Ok(WebhookDetailsResponse {
        raw_connector_status: None,
        amount: None,
        resource_id: Some(ResponseId::ConnectorTransactionId(payment_id.clone())),
        status,
        connector_response_reference_id: Some(payment_id),
        connector_request_reference_id: None,
        mandate_reference,
        error_code,
        error_message,
        error_reason,
        // The callback body carries customer and account data; it is not echoed back.
        raw_connector_response: None,
        status_code: CALLBACK_STATUS_CODE,
        response_headers: None,
        captured_money: amount_captured,
        network_txn_id,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// Refund callback details (HandleEvent phase), mapped with the RSync operation table.
pub fn refund_webhook_details(
    response: &EcommpayStatusResponse,
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::WebhookError>> {
    let (operation, connector_refund_id) = callback_refund_operation(response)?;
    let status = map_refund_operation_status(operation.status);
    let (error_code, error_message) = if status == RefundStatus::Failure {
        let error =
            error_from_operation(operation, &connector_refund_id, CALLBACK_STATUS_CODE, None);
        (Some(error.code), Some(error.message))
    } else {
        (None, None)
    };
    Ok(RefundWebhookDetailsResponse {
        raw_connector_status: None,
        connector_refund_id: Some(connector_refund_id),
        merchant_transaction_id: None,
        status,
        connector_response_reference_id: non_empty(response.payment.merchant_refund_id.clone()),
        error_code,
        error_message,
        raw_connector_response: None,
        status_code: CALLBACK_STATUS_CODE,
        response_headers: None,
    })
}
