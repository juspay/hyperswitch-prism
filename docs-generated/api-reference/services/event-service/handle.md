# Handle RPC

<!--
---
title: Handle
description: Process webhook notifications from connectors and translate into standardized responses
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Handle` RPC processes raw webhook payloads from payment processors. It verifies webhook signatures, parses the event data, and returns a normalized response with the event type and associated payment/refund/dispute details.

**Business Use Case:** When payment processors send webhook notifications to your endpoint, you need to verify they're authentic, understand what happened, and update your system accordingly. This RPC handles the verification and parsing so you can focus on business logic.

## Purpose

**Why use the Event Service for webhooks?**

| Challenge | How Handle RPC Helps |
|-----------|---------------------|
| **Signature verification** | Automatically verifies webhook authenticity using connector secrets |
| **Multiple connector formats** | Normalizes Stripe, Adyen, PayPal events into consistent format |
| **Event parsing** | Extracts payment/refund/dispute IDs and status from complex payloads |
| **Security** | Validates webhook secrets before processing |

**Key outcomes:**
- Verified webhook authenticity
- Normalized event structure
- Extracted entity details (payment, refund, dispute)
- Ready-to-process event data

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_event_id` | string | Yes | Your unique event reference for idempotency |
| `request_details` | RequestDetails | Yes | HTTP request details including headers, body, URL |
| `webhook_secrets` | WebhookSecrets | No | Secrets for verifying webhook signatures |
| `state` | ConnectorState | No | State from previous webhook processing |

### RequestDetails Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `method` | string | Yes | HTTP method (e.g., "POST") |
| `url` | string | Yes | Full webhook URL |
| `headers` | map<string,string> | Yes | HTTP headers including signature headers |
| `body` | bytes | Yes | Raw webhook payload |

### WebhookSecrets Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `secret` | SecretString | No | Webhook signing secret from connector dashboard |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `event_type` | WebhookEventType | Type of event: PAYMENT_INTENT_SUCCESS, CHARGE_REFUNDED, DISPUTE_CREATED, etc. |
| `event_response` | EventResponse | Event content with payment/refund/dispute details |
| `source_verified` | bool | Whether webhook signature was verified |
| `merchant_event_id` | string | Your event reference (echoed back) |
| `event_status` | WebhookEventStatus | Processing status: COMPLETE, INCOMPLETE |

### EventResponse Content

The `event_response` contains one of the following based on `event_type`:

| Field | Type | Present When |
|-------|------|--------------|
| `payments_response` | PaymentServiceGetResponse | Payment-related events |
| `refunds_response` | RefundResponse | Refund-related events |
| `disputes_response` | DisputeResponse | Dispute-related events |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_event_id": "evt_webhook_001",
    "request_details": {
      "method": "POST",
      "url": "https://your-app.com/webhooks/stripe",
      "headers": {
        "stripe-signature": "t=1234567890,v1=abc123...",
        "content-type": "application/json"
      },
      "body": "ewogICJpZCI6ICJldF8xT..."
    },
    "webhook_secrets": {
      "secret": "whsec_your_webhook_secret"
    }
  }' \
  localhost:8080 \
  types.EventService/Handle
```

### Response

```json
{
  "event_type": "PAYMENT_INTENT_SUCCESS",
  "event_response": {
    "payments_response": {
      "connector_transaction_id": "pi_3Oxxx...",
      "status": "CAPTURED",
      "status_code": 200
    }
  },
  "source_verified": true,
  "event_status": "WEBHOOK_EVENT_STATUS_COMPLETE"
}
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Process payment events
- [Refund Service](../refund-service/README.md) - Process refund events
- [Dispute Service](../dispute-service/README.md) - Process dispute events
