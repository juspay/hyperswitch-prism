# IncomingWebhook Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the IncomingWebhook flow in **ANY** payment connector. These patterns are extracted from successful connector implementations (Bluesnap, Trustpay, Fiuu, Noon, Novalnet, etc.) and can be consumed by AI to generate consistent, production-ready webhook handling code for any payment gateway.

## 🚀 Quick Start Guide

To implement IncomingWebhook for a new connector:

1. **Choose Your Pattern**: Determine signature verification method (HMAC-SHA256, SHA256, MD5, or custom)
2. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
3. **Select Components**: Choose event type mapping and webhook processing based on your connector's API
4. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector Webhooks

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{SignatureAlgorithm} → HmacSha256 (or Md5, Sha256 based on connector)
{SignatureHeader} → "x-newpayment-signature" (connector-specific header name)
{WebhookEventEnum} → NewPaymentWebhookEvent
```

**✅ Result**: Complete, production-ready webhook implementation in ~30 minutes

## Table of Contents

1. [Overview](#overview)
2. [Core Webhook Architecture](#core-webhook-architecture)
3. [Signature Verification Patterns](#signature-verification-patterns)
4. [Event Type Mapping Patterns](#event-type-mapping-patterns)
5. [Webhook Processing Patterns](#webhook-processing-patterns)
6. [Implementation Examples](#implementation-examples)
7. [Testing Patterns](#testing-patterns)
8. [Integration Checklist](#integration-checklist)

## Overview

The IncomingWebhook flow handles asynchronous notifications from payment connectors about payment status changes, refunds, disputes, and other events. The flow consists of:

1. **Webhook Reception**: Receiving HTTP POST requests from the connector
2. **Source Verification**: Verifying the webhook authenticity using signatures
3. **Event Type Detection**: Determining the type of event (payment success, refund, dispute, etc.)
4. **Webhook Processing**: Extracting relevant data and transforming it to internal formats
5. **Response**: Returning appropriate HTTP responses to acknowledge receipt

### Key Components:

- **Signature Verification**: Ensures webhooks come from the legitimate connector
- **Event Type Mapping**: Maps connector-specific events to standard UCS events
- **Webhook Processing**: Extracts transaction IDs, statuses, and other relevant data
- **Error Handling**: Handles malformed webhooks, missing fields, and verification failures

## Core Webhook Architecture

### Trait Definition

The `IncomingWebhook` trait is defined in `crates/types-traits/interfaces/src/connector_types.rs`:

```rust
pub trait IncomingWebhook {
    /// Verifies the webhook source authenticity
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<bool, error_stack::Report<ConnectorError>> {
        Ok(false)
    }

    /// Extracts the signature from the webhook request
    fn get_webhook_source_verification_signature(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<ConnectorError>> {
        Ok(Vec::new())
    }

    /// Constructs the message used for signature verification
    fn get_webhook_source_verification_message(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<ConnectorError>> {
        Ok(Vec::new())
    }

    /// Determines the event type from the webhook payload
    fn get_event_type(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<EventType, error_stack::Report<ConnectorError>>;

    /// Processes payment webhooks
    fn process_payment_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<ConnectorError>>;

    /// Processes refund webhooks
    fn process_refund_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<ConnectorError>>;

    /// Processes dispute/chargeback webhooks
    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<ConnectorError>>;

    /// Returns the webhook resource object for logging/debugging
    fn get_webhook_resource_object(
        &self,
        _request: RequestDetails,
    ) -> Result<Box<dyn ErasedMaskSerialize>, error_stack::Report<ConnectorError>>;
}
```

### RequestDetails Structure

```rust
pub struct RequestDetails {
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
    pub method: String,
    pub url: String,
}
```

### ConnectorWebhookSecrets Structure

```rust
pub struct ConnectorWebhookSecrets {
    pub secret: Vec<u8>,
    pub additional_secret: Option<Vec<u8>>,
}
```

## Signature Verification Patterns

### Pattern 1: HMAC-SHA256 (Most Common)

Used by: Bluesnap, Trustpay, Revolut, Noon, Novalnet

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for {ConnectorName}<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<bool, error_stack::Report<errors::ConnectorError>> {
        let connector_webhook_secret = connector_webhook_secret
            .ok_or(errors::ConnectorError::WebhookSourceVerificationFailed)
            .attach_printable("Connector webhook secret not configured")?;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secret)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secret)?;

        use common_utils::crypto::{HmacSha256, SignMessage};
        let expected_signature = HmacSha256
            .sign_message(&connector_webhook_secret.secret, &message)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to sign webhook message with HMAC-SHA256")?;

        Ok(expected_signature.eq(&signature))
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        let signature_str = request
            .headers
            .get("{signature_header}")  // e.g., "bls-signature", "x-revolut-signature"
            .ok_or(errors::ConnectorError::WebhookSignatureNotFound)?;

        hex::decode(signature_str)
            .change_context(errors::ConnectorError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        // Pattern A: Timestamp + Body (Bluesnap style)
        let timestamp = request
            .headers
            .get("{timestamp_header}")  // e.g., "bls-ipn-timestamp"
            .ok_or(errors::ConnectorError::WebhookSourceVerificationFailed)?;
        let body_str = String::from_utf8_lossy(&request.body);
        Ok(format!("{timestamp}{body_str}").into_bytes())

        // Pattern B: Sorted Payload Values (Trustpay style)
        let response: serde_json::Value = request
            .body
            .parse_struct("Webhook Value")
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;
        let values = utils::collect_and_sort_values_by_removing_signature(&response, &signature);
        let payload = values.join("/");
        Ok(payload.into_bytes())
    }
}
```

### Pattern 2: MD5 Verification

Used by: Fiuu

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for {ConnectorName}<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<bool, error_stack::Report<errors::ConnectorError>> {
        let algorithm = crypto::Md5;

        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => Err(errors::ConnectorError::WebhookSourceVerificationFailed)?,
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        algorithm
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        // MD5-specific message construction
        let resource: {ConnectorName}WebhookBody = request
            .body
            .parse_struct("WebhookBody")
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)?;

        let verification_message = format!(
            "{}{}{}{}{}{}",
            resource.transaction_id,
            resource.order_id,
            resource.status,
            resource.merchant_id,
            resource.amount,
            String::from_utf8_lossy(&connector_webhook_secrets.secret)
        );

        Ok(verification_message.as_bytes().to_vec())
    }
}
```

### Pattern 3: Body-Extracted Signature

Used by: Trustpay, Novalnet

```rust
fn get_webhook_source_verification_signature(
    &self,
    request: &RequestDetails,
    _connector_webhook_secret: &ConnectorWebhookSecrets,
) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
    // Parse webhook body to extract signature field
    let webhook_response: {ConnectorName}WebhookResponse = request
        .body
        .parse_struct("WebhookResponse")
        .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

    hex::decode(webhook_response.signature)
        .change_context(errors::ConnectorError::WebhookSignatureNotFound)
}
```

### Pattern 4: Graceful Failure (Fail Open)

Used by: Novalnet (logs warnings but continues processing)

```rust
fn verify_webhook_source(
    &self,
    request: RequestDetails,
    connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<bool, error_stack::Report<errors::ConnectorError>> {
    let connector_webhook_secrets = match connector_webhook_secret {
        Some(secrets) => secrets,
        None => {
            tracing::warn!("No webhook secret configured");
            return Ok(false);
        }
    };

    let signature = match self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets) {
        Ok(sig) => sig,
        Err(error) => {
            tracing::warn!("Failed to get signature: {} - continuing processing", error);
            return Ok(false);
        }
    };

    let message = match self.get_webhook_source_verification_message(&request, &connector_webhook_secrets) {
        Ok(msg) => msg,
        Err(error) => {
            tracing::warn!("Failed to get message: {} - continuing processing", error);
            return Ok(false);
        }
    };

    match algorithm.verify_signature(&connector_webhook_secrets.secret, &signature, &message) {
        Ok(is_verified) => Ok(is_verified),
        Err(error) => {
            tracing::warn!("Verification failed: {} - continuing processing", error);
            Ok(false)
        }
    }
}
```

## Event Type Mapping Patterns

### Pattern 1: Direct Enum Mapping

```rust
// In transformers.rs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}WebhookEvent {
    PaymentSuccess,
    PaymentFailed,
    PaymentPending,
    RefundSuccess,
    RefundFailed,
    ChargebackOpened,
    ChargebackWon,
    ChargebackLost,
}

impl From<{ConnectorName}WebhookEvent> for EventType {
    fn from(event: {ConnectorName}WebhookEvent) -> Self {
        match event {
            {ConnectorName}WebhookEvent::PaymentSuccess => EventType::PaymentIntentSuccess,
            {ConnectorName}WebhookEvent::PaymentFailed => EventType::PaymentIntentFailure,
            {ConnectorName}WebhookEvent::PaymentPending => EventType::PaymentIntentProcessing,
            {ConnectorName}WebhookEvent::RefundSuccess => EventType::RefundSuccess,
            {ConnectorName}WebhookEvent::RefundFailed => EventType::RefundFailure,
            {ConnectorName}WebhookEvent::ChargebackOpened => EventType::DisputeOpened,
            {ConnectorName}WebhookEvent::ChargebackWon => EventType::DisputeWon,
            {ConnectorName}WebhookEvent::ChargebackLost => EventType::DisputeLost,
        }
    }
}

// In connector.rs
fn get_event_type(
    &self,
    request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
    let webhook_body: transformers::{ConnectorName}WebhookBody = request
        .body
        .parse_struct("WebhookBody")
        .change_context(errors::ConnectorError::WebhookEventTypeNotFound)?;

    Ok(EventType::from(webhook_body.event_type))
}
```

### Pattern 2: Conditional Event Type Detection

```rust
fn get_event_type(
    &self,
    request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
    // Try parsing as payment webhook first
    match serde_urlencoded::from_bytes::<transformers::PaymentWebhookBody>(&request.body) {
        Ok(webhook_body) => {
            match webhook_body.transaction_type {
                transformers::WebhookEvent::Chargeback
                | transformers::WebhookEvent::ChargebackStatusChanged => {
                    // Parse as dispute webhook for chargeback events
                    let dispute_body: transformers::DisputeWebhookBody =
                        serde_urlencoded::from_bytes(&request.body)
                            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;
                    transformers::map_chargeback_status_to_event_type(&dispute_body.cb_status)
                }
                _ => Ok(transformers::map_webhook_event_to_incoming_webhook_event(
                    &webhook_body.transaction_type,
                )),
            }
        }
        Err(_) => {
            // Fallback to dispute parsing
            let dispute_body: transformers::DisputeWebhookBody =
                serde_urlencoded::from_bytes(&request.body)
                    .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;
            transformers::map_chargeback_status_to_event_type(&dispute_body.cb_status)
        }
    }
}
```

### Pattern 3: Transaction-Based Event Detection

```rust
fn get_event_type(
    &self,
    request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
    let notif: transformers::WebhookNotification = request
        .body
        .parse_struct("WebhookNotification")
        .change_context(errors::ConnectorError::WebhookEventTypeNotFound)?;

    let transaction_status = match notif.transaction {
        transformers::WebhookTransactionData::CaptureTransactionData(data) => data.status,
        transformers::WebhookTransactionData::CancelTransactionData(data) => data.status,
        transformers::WebhookTransactionData::RefundsTransactionData(data) => data.status,
    };

    Ok(transformers::get_incoming_webhook_event(
        notif.event.event_type,
        transaction_status,
    ))
}
```

## Webhook Processing Patterns

### Payment Webhook Processing

```rust
fn process_payment_webhook(
    &self,
    request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<WebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
    let webhook_body: transformers::WebhookBody = request
        .body
        .parse_struct("WebhookBody")
        .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

    // Map webhook status to UCS AttemptStatus
    let status = match webhook_body.status {
        transformers::WebhookStatus::Success => common_enums::AttemptStatus::Charged,
        transformers::WebhookStatus::Failed => common_enums::AttemptStatus::Failure,
        transformers::WebhookStatus::Pending => common_enums::AttemptStatus::Pending,
        transformers::WebhookStatus::Declined => common_enums::AttemptStatus::Failure,
    };

    // Extract resource ID
    let resource_id = if !webhook_body.merchant_transaction_id.is_empty() {
        Some(ResponseId::EncodedData(webhook_body.merchant_transaction_id))
    } else if !webhook_body.connector_transaction_id.is_empty() {
        Some(ResponseId::ConnectorTransactionId(webhook_body.connector_transaction_id))
    } else {
        None
    };

    Ok(WebhookDetailsResponse {
        resource_id,
        status,
        connector_response_reference_id: webhook_body.reference_number.ok_or_empty(),
        mandate_reference: None,
        error_code: webhook_body.error_code,
        error_message: webhook_body.error_message,
        error_reason: webhook_body.error_reason,
        raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
        status_code: 200,
        response_headers: None,
        transformation_status: common_enums::WebhookTransformationStatus::Complete,
        amount_captured: webhook_body.amount_captured,
        minor_amount_captured: webhook_body.minor_amount_captured,
        network_txn_id: webhook_body.network_txn_id,
    })
}
```

### Refund Webhook Processing

```rust
fn process_refund_webhook(
    &self,
    request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
    let webhook_body: transformers::RefundWebhookBody = request
        .body
        .parse_struct("RefundWebhookBody")
        .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

    let connector_refund_id = webhook_body
        .refund_id
        .ok_or(errors::ConnectorError::WebhookReferenceIdNotFound)?;

    let status = match webhook_body.status {
        transformers::RefundStatus::Success => common_enums::RefundStatus::Success,
        transformers::RefundStatus::Failed => common_enums::RefundStatus::Failure,
        transformers::RefundStatus::Pending => common_enums::RefundStatus::Pending,
    };

    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(connector_refund_id),
        status,
        connector_response_reference_id: webhook_body.reference_number,
        error_code: webhook_body.error_code,
        error_message: webhook_body.error_message,
        raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
```

### Dispute Webhook Processing

```rust
fn process_dispute_webhook(
    &self,
    request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
    let notif: transformers::DisputeWebhookBody = request
        .body
        .parse_struct("DisputeWebhookBody")
        .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

    let (amount, currency, reason, reason_code) = match notif.transaction {
        transformers::DisputeTransactionData::CaptureTransactionData(data) => {
            (data.amount, data.currency, None, None)
        }
        transformers::DisputeTransactionData::ChargebackData(data) => {
            (data.amount, data.currency, data.reason, data.reason_code)
        }
    };

    let dispute_status = transformers::get_dispute_status(notif.event.event_type);

    Ok(DisputeWebhookDetailsResponse {
        amount: utils::convert_amount(
            self.amount_converter,
            amount.ok_or(errors::ConnectorError::AmountConversionFailed)?,
            transformers::option_to_result(currency)?,
        )?,
        currency: transformers::option_to_result(currency)?,
        stage: common_enums::DisputeStage::Dispute,
        dispute_id: notif.event.tid.to_string(),
        connector_reason_code: reason_code,
        status: common_enums::DisputeStatus::foreign_try_from(dispute_status)?,
        connector_response_reference_id: None,
        dispute_message: reason,
        raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
```

### Resource Object Extraction

```rust
fn get_webhook_resource_object(
    &self,
    request: RequestDetails,
) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<errors::ConnectorError>> {
    let resource: transformers::WebhookObject = request
        .body
        .parse_struct("WebhookObject")
        .change_context(errors::ConnectorError::WebhookResourceObjectNotFound)
        .attach_printable("Failed to parse webhook resource object")?;

    Ok(Box::new(transformers::PaymentResponse::from(resource)))
}
```

## Implementation Examples

### Example 1: Bluesnap-Style Implementation (HMAC-SHA256)

```rust
// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Bluesnap<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<bool, error_stack::Report<errors::ConnectorError>> {
        let connector_webhook_secret = connector_webhook_secret
            .ok_or(errors::ConnectorError::WebhookSourceVerificationFailed)
            .attach_printable("Connector webhook secret not configured")?;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secret)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secret)?;

        use common_utils::crypto::{HmacSha256, SignMessage};
        let expected_signature = HmacSha256
            .sign_message(&connector_webhook_secret.secret, &message)
            .change_context(errors::ConnectorError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to sign webhook message with HMAC-SHA256")?;

        Ok(expected_signature.eq(&signature))
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        let signature_str = request
            .headers
            .get("bls-signature")
            .ok_or(errors::ConnectorError::WebhookSignatureNotFound)?;

        hex::decode(signature_str)
            .change_context(errors::ConnectorError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::ConnectorError>> {
        let timestamp = request
            .headers
            .get("bls-ipn-timestamp")
            .ok_or(errors::ConnectorError::WebhookSourceVerificationFailed)?;
        let body_str = String::from_utf8_lossy(&request.body);
        Ok(format!("{timestamp}{body_str}").into_bytes())
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
        // Implementation with conditional parsing for chargebacks
        // ... see Event Type Mapping Pattern 2
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        // Implementation
        // ... see Payment Webhook Processing
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        // Implementation
        // ... see Refund Webhook Processing
    }
}
```

### Example 2: Simple Implementation (No Signature Verification)

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for {ConnectorName}<T>
{
    // Uses default verify_webhook_source (returns false - falls back to psync)

    fn get_event_type(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<EventType, error_stack::Report<errors::ConnectorError>> {
        let webhook_body: {ConnectorName}WebhookBody = request
            .body
            .parse_struct("WebhookBody")
            .change_context(errors::ConnectorError::WebhookEventTypeNotFound)?;

        Ok(EventType::from(webhook_body.event_type))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorAuthType>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<errors::ConnectorError>> {
        let webhook_body: {ConnectorName}WebhookBody = request
            .body
            .parse_struct("WebhookBody")
            .change_context(errors::ConnectorError::WebhookBodyDecodingFailed)?;

        Ok(WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(webhook_body.transaction_id)),
            status: common_enums::AttemptStatus::from(webhook_body.status),
            connector_response_reference_id: webhook_body.reference,
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            transformation_status: common_enums::WebhookTransformationStatus::Complete,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
        })
    }
}
```

## Testing Patterns

### Unit Test Structure

```rust
#[cfg(test)]
mod webhook_tests {
    use super::*;

    #[test]
    fn test_webhook_signature_verification() {
        let connector = {ConnectorName}::new();

        // Create test request
        let request = RequestDetails {
            headers: {
                let mut headers = std::collections::HashMap::new();
                headers.insert("x-signature".to_string(), "expected_signature".to_string());
                headers
            },
            body: b"test webhook body".to_vec(),
            method: "POST".to_string(),
            url: "/webhooks".to_string(),
        };

        let webhook_secret = ConnectorWebhookSecrets {
            secret: b"test_secret".to_vec(),
            additional_secret: None,
        };

        // Test signature extraction
        let signature = connector
            .get_webhook_source_verification_signature(&request, &webhook_secret)
            .unwrap();
        assert!(!signature.is_empty());

        // Test message construction
        let message = connector
            .get_webhook_source_verification_message(&request, &webhook_secret)
            .unwrap();
        assert!(!message.is_empty());
    }

    #[test]
    fn test_event_type_parsing() {
        let connector = {ConnectorName}::new();

        let request = RequestDetails {
            headers: std::collections::HashMap::new(),
            body: r#"{"event_type": "payment.success"}"#.as_bytes().to_vec(),
            method: "POST".to_string(),
            url: "/webhooks".to_string(),
        };

        let event_type = connector
            .get_event_type(request, None, None)
            .unwrap();

        assert_eq!(event_type, EventType::PaymentIntentSuccess);
    }

    #[test]
    fn test_payment_webhook_processing() {
        let connector = {ConnectorName}::new();

        let request = RequestDetails {
            headers: std::collections::HashMap::new(),
            body: r#"{
                "transaction_id": "txn_123",
                "status": "success",
                "amount": 1000,
                "currency": "USD"
            }"#.as_bytes().to_vec(),
            method: "POST".to_string(),
            url: "/webhooks".to_string(),
        };

        let response = connector
            .process_payment_webhook(request, None, None)
            .unwrap();

        assert_eq!(response.status, common_enums::AttemptStatus::Charged);
        assert!(response.resource_id.is_some());
    }
}
```

### Integration Test Pattern

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_full_webhook_flow() {
        let connector = {ConnectorName}::new();

        // Test webhook with valid signature
        let valid_webhook = create_test_webhook("payment.success", true);
        let is_valid = connector
            .verify_webhook_source(valid_webhook.request, valid_webhook.secret, None)
            .unwrap();
        assert!(is_valid);

        // Test webhook with invalid signature
        let invalid_webhook = create_test_webhook("payment.success", false);
        let is_valid = connector
            .verify_webhook_source(invalid_webhook.request, invalid_webhook.secret, None)
            .unwrap();
        assert!(!is_valid);
    }

    fn create_test_webhook(event_type: &str, valid_signature: bool) -> TestWebhook {
        // Helper function to create test webhooks
        // ... implementation
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **Webhook Documentation Review**
  - [ ] Understand connector's webhook format (JSON, Form URL Encoded, XML)
  - [ ] Identify signature verification method (HMAC-SHA256, SHA256, MD5, custom)
  - [ ] Identify signature location (header, body field)
  - [ ] Understand message construction for verification
  - [ ] Review event types and their meanings
  - [ ] Identify required webhook secret configuration

- [ ] **Webhook Types Supported**
  - [ ] Payment status webhooks
  - [ ] Refund webhooks
  - [ ] Dispute/Chargeback webhooks
  - [ ] Mandate webhooks
  - [ ] Other connector-specific events

### Implementation Checklist

- [ ] **Webhook Types Definition**
  - [ ] Define `{ConnectorName}WebhookEvent` enum in transformers.rs
  - [ ] Define `{ConnectorName}WebhookBody` struct for parsing
  - [ ] Define status enums if needed
  - [ ] Implement `From` trait for EventType conversion

- [ ] **Signature Verification**
  - [ ] Implement `get_webhook_source_verification_signature`
  - [ ] Implement `get_webhook_source_verification_message`
  - [ ] Implement `verify_webhook_source` (if custom logic needed)
  - [ ] Handle signature extraction from headers or body
  - [ ] Handle hex/base64 decoding of signatures

- [ ] **Event Type Detection**
  - [ ] Implement `get_event_type`
  - [ ] Map all connector event types to UCS EventTypes
  - [ ] Handle conditional parsing for different webhook types
  - [ ] Handle unknown event types gracefully

- [ ] **Webhook Processing**
  - [ ] Implement `process_payment_webhook`
  - [ ] Implement `process_refund_webhook` (if supported)
  - [ ] Implement `process_dispute_webhook` (if supported)
  - [ ] Implement `get_webhook_resource_object` (optional)
  - [ ] Map connector statuses to UCS AttemptStatus/RefundStatus/DisputeStatus
  - [ ] Extract transaction IDs and reference IDs
  - [ ] Handle error cases and missing fields

- [ ] **Error Handling**
  - [ ] Handle signature verification failures
  - [ ] Handle webhook parsing errors
  - [ ] Handle missing required fields
  - [ ] Log warnings for graceful failures
  - [ ] Return appropriate error responses

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test signature verification with valid signatures
  - [ ] Test signature verification with invalid signatures
  - [ ] Test message construction
  - [ ] Test event type parsing for all event types
  - [ ] Test webhook processing for success cases
  - [ ] Test webhook processing for failure cases
  - [ ] Test error handling for malformed webhooks

- [ ] **Integration Tests**
  - [ ] Test complete webhook flow with valid webhooks
  - [ ] Test webhook flow with invalid signatures
  - [ ] Test with real connector webhook payloads

### Documentation Checklist

- [ ] **Code Documentation**
  - [ ] Document webhook event types
  - [ ] Document signature verification approach
  - [ ] Document any connector-specific handling
  - [ ] Add examples in doc comments

- [ ] **Configuration Documentation**
  - [ ] Document webhook secret configuration
  - [ ] Document webhook URL setup with connector
  - [ ] Document any required webhook settings in connector dashboard

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `NewPayment` | **Always required** - Used in struct/trait names |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `new_payment` | **Always required** - Used in file names, config keys |
| `{SignatureAlgorithm}` | Crypto algorithm for verification | `HmacSha256`, `Md5`, `Sha256` | **Based on connector docs** |
| `{SignatureHeader}` | HTTP header containing signature | `"x-signature"`, `"bls-signature"` | **From connector docs** |
| `{TimestampHeader}` | HTTP header containing timestamp | `"bls-ipn-timestamp"` | **For timestamp-based verification** |
| `{WebhookEventEnum}` | Webhook event enum name | `BluesnapWebhookEvent` | **Always required** |
| `{WebhookBodyStruct}` | Webhook body struct name | `BluesnapWebhookBody` | **Always required** |

### Real-World Examples

**Example 1: HMAC-SHA256 with Header Signature (Bluesnap-style)**
```bash
{ConnectorName} → Bluesnap
{connector_name} → bluesnap
{SignatureAlgorithm} → HmacSha256
{SignatureHeader} → "bls-signature"
{TimestampHeader} → "bls-ipn-timestamp"
{WebhookEventEnum} → BluesnapWebhookEvent
{WebhookBodyStruct} → BluesnapWebhookBody
```

**Example 2: MD5 with Body-Extracted Signature (Fiuu-style)**
```bash
{ConnectorName} → Fiuu
{connector_name} → fiuu
{SignatureAlgorithm} → Md5
{WebhookEventEnum} → FiuuWebhookEvent
{WebhookBodyStruct} → FiuuWebhooksResponse
Signature extracted from body field, not header
```

**Example 3: Simple Implementation (No Verification)**
```bash
{ConnectorName} → SimplePay
{connector_name} → simple_pay
{SignatureAlgorithm} → None (uses default trait methods)
Verification disabled - falls back to psync flow
```

## Best Practices

### Security

1. **Always verify webhooks when possible**
   - Implement signature verification for all connectors that support it
   - Never trust webhook payloads without verification
   - Use constant-time comparison for signature verification (built into crypto utilities)

2. **Handle secrets securely**
   - Never log webhook secrets
   - Use `Secret` types for sensitive data
   - Store secrets in secure configuration

3. **Graceful failure handling**
   - Log verification failures but don't block processing if connector requires it
   - Return appropriate error responses to connector
   - Consider falling back to psync flow on verification failure

### Code Quality

4. **Comprehensive event type mapping**
   - Map ALL connector event types to UCS events
   - Handle unknown events gracefully (log and ignore)
   - Document event type mappings

5. **Consistent error handling**
   - Use descriptive error messages
   - Attach printable context for debugging
   - Handle missing fields gracefully with proper error types

6. **Raw response preservation**
   - Always include `raw_connector_response` in webhook responses
   - Use `String::from_utf8_lossy(&request.body).to_string()` for raw response
   - Helps with debugging and audit trails

### Testing

7. **Test with real payloads**
   - Use actual webhook payloads from connector documentation
   - Test all event types
   - Test edge cases (missing fields, malformed data)

8. **Test signature verification**
   - Test with valid signatures
   - Test with invalid signatures
   - Test with missing signatures
   - Test message construction

## Common Issues and Solutions

### Issue 1: Signature Verification Fails

**Symptoms**: Webhooks fail verification even with correct secret

**Solutions**:
- Verify signature encoding (hex vs base64)
- Check message construction matches connector specification
- Ensure correct header names are used
- Verify webhook secret is correctly configured

### Issue 2: Event Type Not Found

**Symptoms**: `WebhookEventTypeNotFound` errors

**Solutions**:
- Ensure all event types are mapped in `get_event_type`
- Handle unknown events gracefully
- Add logging to debug event type parsing

### Issue 3: Missing Transaction ID

**Symptoms**: `WebhookReferenceIdNotFound` errors

**Solutions**:
- Check field names in webhook body
- Handle multiple ID fields (merchant_transaction_id vs connector_transaction_id)
- Use fallback logic for ID extraction

### Issue 4: Body Parsing Fails

**Symptoms**: `WebhookBodyDecodingFailed` errors

**Solutions**:
- Verify content type handling (JSON vs Form URL Encoded)
- Check struct definitions match connector payload
- Use `serde_urlencoded` for form-encoded webhooks
- Use `serde_json` for JSON webhooks

---

**💡 Pro Tip**: Always test webhook implementations with real connector webhook payloads in a sandbox environment before deploying to production.
