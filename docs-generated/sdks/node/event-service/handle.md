# handle Method

<!--
---
title: handle (Node.js SDK)
description: Process webhook notifications from payment processors using the Node.js SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: node
---
-->

## Overview

The `handle` method processes raw webhook payloads from payment processors. It verifies webhook signatures, parses the event data, and returns a normalized response with the event type and associated payment details.

**Business Use Case:** When Stripe sends a webhook notification that a payment succeeded, you need to verify it's authentic and update your order status. This method handles verification and parsing.

## Purpose

| Challenge | Solution |
|-----------|----------|
| Signature verification | Automatically verifies webhook authenticity |
| Multiple formats | Normalizes Stripe, Adyen into consistent format |
| Security | Validates secrets before processing |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantEventId` | string | Yes | Your unique event reference |
| `payload` | string/object | Yes | Raw webhook body |
| `headers` | object | Yes | HTTP headers including signature |
| `webhookSecret` | string | Yes | Webhook signing secret |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `eventType` | string | Type: payment.captured, refund.succeeded, etc. |
| `eventResponse` | object | Payment/refund/dispute details |
| `sourceVerified` | boolean | Whether signature was verified |
| `eventStatus` | string | COMPLETE, INCOMPLETE |

## Example

### SDK Setup

```javascript
const { EventClient } = require('hyperswitch-prism');

const eventClient = new EventClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY'
});
```

### Express.js Webhook Handler

```javascript
app.post('/webhooks/stripe', express.raw({ type: 'application/json' }), async (req, res) => {
    const request = {
        merchantEventId: `evt_${Date.now()}`,
        payload: req.body,
        headers: req.headers,
        webhookSecret: process.env.STRIPE_WEBHOOK_SECRET
    };

    try {
        const response = await eventClient.handle(request);

        if (response.eventType === 'payment.captured') {
            await fulfillOrder(response.eventResponse.paymentsResponse.merchantTransactionId);
        } else if (response.eventType === 'refund.succeeded') {
            await updateRefundStatus(response.eventResponse.refundsResponse);
        }

        res.json({ received: true });
    } catch (error) {
        res.status(400).json({ error: error.message });
    }
});
```

### Response

```javascript
{
    eventType: "payment.captured",
    eventResponse: {
        paymentsResponse: {
            merchantTransactionId: "txn_order_001",
            connectorTransactionId: "pi_3Oxxx...",
            status: "CAPTURED"
        }
    },
    sourceVerified: true,
    eventStatus: "COMPLETE"
}
```

## Event Types

| Event Type | Description |
|------------|-------------|
| `payment.authorized` | Payment authorized, funds held |
| `payment.captured` | Payment completed |
| `payment.failed` | Payment declined |
| `refund.succeeded` | Refund processed |
| `dispute.created` | New chargeback received |

## Next Steps

- [Payment Service](../payment-service/README.md) - Process payment events
- [Refund Service](../refund-service/README.md) - Process refund events
- [Dispute Service](../dispute-service/README.md) - Process dispute events
