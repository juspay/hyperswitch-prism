# handle Method

<!--
---
title: handle (Python SDK)
description: Process webhook notifications from payment processors using the Python SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: python
---
-->

## Overview

The `handle` method processes raw webhook payloads from payment processors. It verifies webhook signatures and returns a normalized response.

**Business Use Case:** When Stripe sends a webhook that a payment succeeded, verify it's authentic and update your order status.

## Purpose

| Challenge | Solution |
|-----------|----------|
| Signature verification | Automatic verification |
| Multiple formats | Normalized responses |
| Security | Validates secrets |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_event_id` | str | Yes | Your unique event reference |
| `payload` | str/dict | Yes | Raw webhook body |
| `headers` | dict | Yes | HTTP headers |
| `webhook_secret` | str | Yes | Webhook signing secret |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `event_type` | str | Type: payment.captured, refund.succeeded, etc. |
| `event_response` | dict | Payment/refund/dispute details |
| `source_verified` | bool | Whether signature verified |
| `event_status` | str | COMPLETE, INCOMPLETE |

## Example

### SDK Setup

```python
from hyperswitch_prism import EventClient

event_client = EventClient(
    connector='stripe',
    api_key='YOUR_API_KEY'
)
```

### Flask Webhook Handler

```python
from flask import Flask, request

@app.route('/webhooks/stripe', methods=['POST'])
async def handle_webhook():
    request_data = {
        "merchant_event_id": f"evt_{int(time.time())}",
        "payload": request.get_data().decode(),
        "headers": dict(request.headers),
        "webhook_secret": os.environ['STRIPE_WEBHOOK_SECRET']
    }

    try:
        response = await event_client.handle(request_data)

        if response["event_type"] == "payment.captured":
            await fulfill_order(
                response["event_response"]["payments_response"]["merchant_transaction_id"]
            )

        return "OK", 200
    except Exception as e:
        return str(e), 400
```

### Response

```python
{
    "event_type": "payment.captured",
    "event_response": {
        "payments_response": {
            "merchant_transaction_id": "txn_order_001",
            "connector_transaction_id": "pi_3Oxxx...",
            "status": "CAPTURED"
        }
    },
    "source_verified": True,
    "event_status": "COMPLETE"
}
```

## Event Types

| Event Type | Description |
|------------|-------------|
| `payment.authorized` | Payment authorized |
| `payment.captured` | Payment completed |
| `payment.failed` | Payment declined |
| `refund.succeeded` | Refund processed |
| `dispute.created` | New chargeback |

## Next Steps

- [Payment Service](../payment-service/README.md) - Process payment events
- [Refund Service](../refund-service/README.md) - Process refund events
