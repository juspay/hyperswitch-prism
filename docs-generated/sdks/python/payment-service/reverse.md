# reverse Method

<!--
---
title: reverse (Python SDK)
description: Reverse a captured payment before settlement using the Python SDK
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

The `reverse` method cancels a captured payment before funds have settled. Unlike voids, reverses apply to captured payments that haven't completed bank settlement.

**Business Use Case:** When an error is discovered immediately after capture, such as incorrect amount or duplicate transaction. Recover funds before settlement.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Same-day cancellation | Recover funds before settlement |
| Processing error | Undo incorrect capture |
| Duplicate transaction | Reverse double charge |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_reverse_id` | str | Yes | Your unique reverse operation ID |
| `connector_transaction_id` | str | Yes | The connector's transaction ID |
| `cancellation_reason` | str | No | Reason for reversing |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_reverse_id` | str | Your reference (echoed back) |
| `connector_transaction_id` | str | Connector's transaction ID |
| `status` | PaymentStatus | VOIDED, REVERSED |
| `status_code` | int | HTTP status code |

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
    "merchant_reverse_id": "reverse_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "cancellation_reason": "Duplicate charge detected"
}

response = await payment_client.reverse(request)
```

### Response

```python
{
    "merchant_reverse_id": "reverse_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "VOIDED",
    "status_code": 200
}
```

## Next Steps

- [capture](./capture.md) - Capture a payment
- [void](./void.md) - Cancel before capture
- [refund](./refund.md) - Return funds after settlement