# revoke Method

<!--
---
title: revoke (Python SDK)
description: Cancel an existing recurring payment mandate using the Python SDK
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

The `revoke` method cancels an existing recurring payment mandate. Once revoked, no future automatic charges can be made using this mandate. Use this when customers cancel their subscriptions or recurring services.

**Business Use Case:** A customer cancels their SaaS subscription. You call `revoke` to stop all future billing and comply with their cancellation request.

## Purpose

**Why revoke mandates?**

| Scenario | Benefit |
|----------|---------|
| **Subscription cancellation** | Honor customer cancellation requests |
| **Compliance** | Meet regulatory requirements for recurring billing |
| **Customer retention** | Clean revocation improves re-subscription likelihood |
| **Billing hygiene** | Remove inactive mandates to reduce system clutter |

**Key outcomes:**
- Future automatic charges are blocked
- Customer billing stops immediately
- Compliance with cancellation regulations

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mandate_id` | string | Yes | The mandate ID to revoke |
| `reason` | string | No | Reason for revocation (customer_canceled, etc.) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `mandate_id` | string | The revoked mandate ID |
| `status` | MandateStatus | New status: REVOKED |
| `revoked_at` | string | ISO 8601 timestamp of revocation |
| `status_code` | int | HTTP-style status code (200, 404, etc.) |

## Example

### SDK Setup

```python
from hyperswitch_prism import RecurringPaymentClient

recurring_client = RecurringPaymentClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "mandate_id": "mandate_xxx",
    "reason": "customer_canceled"
}

response = await recurring_client.revoke(request)
```

### Response

```python
{
    "mandate_id": "mandate_xxx",
    "status": "REVOKED",
    "revoked_at": "2024-01-15T10:30:00Z",
    "status_code": 200
}
```

## Important Notes

- **Immediate effect** - Revocation takes effect immediately
- **No refunds** - Revoking doesn't refund past charges
- **Idempotent** - Calling revoke multiple times returns same result
- **No undo** - A revoked mandate cannot be reactivated; setup a new one instead

## Error Handling

| Error Code | Meaning | Action |
|------------|---------|--------|
| `404` | Mandate not found | Check mandate_id is correct |
| `410` | Already revoked | Mandate was already revoked |

## Best Practices

- Always revoke when customer cancels subscription
- Keep record of revocation reason for support
- Confirm successful revocation before updating subscription status
- Notify customer that cancellation is complete

## Next Steps

- [charge](./charge.md) - Process recurring payments before revocation
- [setup_recurring](../payment-service/setup-recurring.md) - Create new mandate if customer resubscribes
