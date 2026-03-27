# revoke Method

<!--
---
title: revoke (Node.js SDK)
description: Cancel an existing recurring payment mandate using the Node.js SDK
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

The `revoke` method cancels an existing recurring payment mandate. Once revoked, no future automatic charges can be made using this mandate.

**Business Use Case:** A customer cancels their SaaS subscription. You call `revoke` to stop all future billing and comply with their cancellation request.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Subscription cancellation | Honor customer cancellation requests |
| Compliance | Meet regulatory requirements |
| Customer retention | Clean revocation improves re-subscription likelihood |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mandateId` | string | Yes | The mandate ID to revoke |
| `reason` | string | No | Reason for revocation |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `mandateId` | string | The revoked mandate ID |
| `status` | MandateStatus | REVOKED |
| `revokedAt` | string | ISO 8601 timestamp |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { RecurringPaymentClient } = require('hyperswitch-prism');

const recurringClient = new RecurringPaymentClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    mandateId: "mandate_xxx",
    reason: "customer_canceled"
};

const response = await recurringClient.revoke(request);
```

### Response

```javascript
{
    mandateId: "mandate_xxx",
    status: "REVOKED",
    revokedAt: "2024-01-15T10:30:00Z",
    statusCode: 200
}
```

## Important Notes

- **Immediate effect** - Revocation takes effect immediately
- **No refunds** - Revoking doesn't refund past charges
- **Idempotent** - Multiple calls return same result
- **No undo** - Create new mandate if needed

## Next Steps

- [charge](./charge.md) - Process payments before revocation
- [setupRecurring](../payment-service/setup-recurring.md) - Create new mandate if customer resubscribes
