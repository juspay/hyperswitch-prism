# void Method

<!--
---
title: void (Python SDK)
description: Cancel an authorized payment using the Python SDK - release reserved funds
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

The `void` method cancels an authorized payment before funds are captured. This releases the held funds back to the customer's payment method, effectively canceling the transaction.

**Business Use Case:** A customer cancels their order before it ships. The payment was authorized at checkout, but since you're not shipping, you void the authorization to release the hold on their funds.

## Purpose

**Why use void?**

| Scenario | Benefit |
|----------|---------|
| **Order cancellation** | Release funds when customer cancels |
| **Fulfillment failure** | Void if item is out of stock |
| **Authorization timing out** | Clean up old authorizations |
| **Fraud prevention** | Void suspicious transactions before capture |

**Key outcomes:**
- Held funds released immediately
- No charge to customer
- Transaction terminated cleanly

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_transaction_id` | string | Yes | Your unique transaction reference |
| `connector_transaction_id` | string | Yes | The connector's transaction ID from authorize |
| `void_reason` | string | No | Reason for voiding |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_transaction_id` | string | Your transaction reference (echoed back) |
| `connector_transaction_id` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status: VOIDED |
| `voided_amount` | int64 | Amount voided in minor units |
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
    "connector_transaction_id": "pi_3Oxxx...",
    "void_reason": "Customer cancelled order"
}

response = await payment_client.void(request)
```

### Response

```python
{
    "merchant_transaction_id": "txn_order_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "VOIDED",
    "voided_amount": 1000,
    "status_code": 200
}
```

## Void vs Refund

| Action | When to Use | Effect on Customer |
|--------|-------------|-------------------|
| **Void** | Before capture, during authorization hold | Funds released immediately, no charge |
| **Refund** | After capture, funds already transferred | Funds returned, may take 5-10 days |

## Error Handling

| Error Code | Meaning | Action |
|------------|---------|--------|
| `404` | Transaction not found | Verify connector_transaction_id |
| `409` | Already captured | Cannot void, use refund instead |
| `410` | Already voided | Already voided, idempotent result |

## Best Practices

- Void as soon as you know the transaction won't complete
- Void is cheaper than refund (no chargeback risk, no settlement costs)
- Authorizations typically expire in 7-10 days if not captured

## Next Steps

- [authorize](./authorize.md) - Create initial authorization
- [capture](./capture.md) - Complete payment instead of voiding
- [refund](./refund.md) - Return funds after capture
