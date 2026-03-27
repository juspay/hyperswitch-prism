# get Method

<!--
---
title: get (Python SDK)
description: Retrieve payment status using the Python SDK - synchronize payment state
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

The `get` method retrieves the current status of a payment from the payment processor. Use this to synchronize your system's payment state with the processor's actual state, especially useful for handling asynchronous status updates.

**Business Use Case:** A webhook notification was missed or delayed. Your system shows a payment as PENDING, but you need to verify its actual status with the processor before fulfilling the order.

## Purpose

**Why use get?**

| Scenario | Benefit |
|----------|---------|
| **Webhook fallback** | Poll when webhooks fail or are delayed |
| **Reconciliation** | Sync payment states with internal systems |
| **Customer service** | Verify payment status for support inquiries |
| **Before fulfillment** | Confirm payment succeeded before shipping |

**Key outcomes:**
- Current payment status from processor
- Synchronized state between systems
- Accurate payment information

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_transaction_id` | string | Yes | Your unique transaction reference |
| `connector_transaction_id` | string | Yes* | Connector's transaction ID (*one of merchant or connector ID required) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_transaction_id` | string | Your transaction reference |
| `connector_transaction_id` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status: AUTHORIZED, CAPTURED, FAILED, etc. |
| `amount` | Money | Payment amount |
| `error` | ErrorInfo | Error details if status is FAILED |
| `status_code` | int | HTTP-style status code (200, 404, etc.) |

## Example

### SDK Setup

```python
from hyperswitch_prism import PaymentClient

payment_client = PaymentClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "merchant_transaction_id": "txn_order_001",
    "connector_transaction_id": "pi_3Oxxx..."
}

response = await payment_client.get(request)
```

### Response

```python
{
    "merchant_transaction_id": "txn_order_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "CAPTURED",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "status_code": 200
}
```

## Common Patterns

### Polling for Status Updates

```python
import asyncio

async def poll_payment_status(merchant_id, connector_id, max_attempts=10):
    for attempt in range(max_attempts):
        response = await payment_client.get({
            "merchant_transaction_id": merchant_id,
            "connector_transaction_id": connector_id
        })

        if response["status"] in ["CAPTURED", "FAILED"]:
            return response

        # Exponential backoff
        await asyncio.sleep(2 ** attempt)

    return response
```

## Status Values

| Status | Description |
|--------|-------------|
| `AUTHORIZED` | Funds reserved, awaiting capture |
| `CAPTURED` | Payment completed, funds transferred |
| `FAILED` | Payment failed, check error details |
| `VOIDED` | Authorization was cancelled |
| `REFUNDED` | Payment was refunded |

## Error Handling

| Error Code | Meaning | Action |
|------------|---------|--------|
| `404` | Payment not found | Verify transaction ID |
| `400` | Invalid request | Check required fields |

## Next Steps

- [authorize](./authorize.md) - Create new authorization
- [capture](./capture.md) - Complete authorized payment
- [void](./void.md) - Cancel authorization
