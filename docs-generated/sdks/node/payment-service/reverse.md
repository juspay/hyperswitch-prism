# reverse Method

<!--
---
title: reverse (Node.js SDK)
description: Reverse a captured payment before settlement using the Node.js SDK
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

The `reverse` method cancels a captured payment before the funds have been settled. Unlike voids (which apply to authorized but not captured payments), reverses apply to captured payments that haven't completed bank settlement.

**Business Use Case:** When an error is discovered immediately after capture, such as incorrect amount charged or duplicate transaction. Reverse allows you to recover funds before they enter the settlement cycle.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Same-day cancellation | Recover funds before settlement |
| Processing error | Undo incorrect capture |
| Duplicate transaction | Reverse accidental double charge |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantReverseId` | string | Yes | Your unique reverse operation ID |
| `connectorTransactionId` | string | Yes | The connector's transaction ID |
| `cancellationReason` | string | No | Reason for reversing |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantReverseId` | string | Your reference (echoed back) |
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | VOIDED, REVERSED |
| `error` | ErrorInfo | Error details if failed |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { PaymentClient } = require('hyperswitch-prism');

const paymentClient = new PaymentClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    merchantReverseId: "reverse_001",
    connectorTransactionId: "pi_3Oxxx...",
    cancellationReason: "Duplicate charge detected"
};

const response = await paymentClient.reverse(request);
```

### Response

```javascript
{
    merchantReverseId: "reverse_001",
    connectorTransactionId: "pi_3Oxxx...",
    status: "VOIDED",
    statusCode: 200
}
```

## Reverse vs Void vs Refund

| Action | When to Use | Timeline |
|--------|-------------|----------|
| **Void** | Before capture | Same day |
| **Reverse** | After capture, before settlement | Same day |
| **Refund** | After settlement | 5-10 business days |

## Next Steps

- [capture](./capture.md) - Capture a payment after authorization
- [void](./void.md) - Cancel before capture
- [refund](./refund.md) - Return funds after settlement
