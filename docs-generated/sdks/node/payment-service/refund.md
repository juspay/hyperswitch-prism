# refund Method

<!--
---
title: refund (Node.js SDK)
description: Issue a refund using the Node.js SDK - return funds to customer
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

The `refund` method returns funds to a customer's payment method after a successful payment. Use this for returns, cancellations after fulfillment, or goodwill adjustments.

**Business Use Case:** A customer returns an item they purchased. The original payment was already captured. You process a refund to return their money.

## Purpose

**Why use refund?**

| Scenario | Benefit |
|----------|---------|
| **Product returns** - Refund customers for returned merchandise |
| **Service cancellation** - Refund for unrendered services |
| **Goodwill gestures** - Partial refunds for service issues |
| **Duplicate charges** - Correct accidental charges |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantRefundId` | string | Yes | Your unique refund reference |
| `connectorTransactionId` | string | Yes | The connector's transaction ID from the payment |
| `refundAmount` | Money | Yes | Amount to refund (can be partial) |
| `reason` | string | No | Reason for refund |
| `metadata` | object | No | Additional data (max 20 keys) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantRefundId` | string | Your refund reference (echoed back) |
| `connectorRefundId` | string | Connector's refund ID (e.g., re_xxx) |
| `status` | RefundStatus | Current status: PENDING, SUCCEEDED, FAILED |
| `refundAmount` | Money | Refund amount |
| `error` | ErrorInfo | Error details if status is FAILED |
| `statusCode` | number | HTTP-style status code (200, 422, etc.) |

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
    merchantRefundId: "refund_001",
    connectorTransactionId: "pi_3Oxxx...",
    refundAmount: {
        minorAmount: 1000,
        currency: "USD"
    },
    reason: "Customer returned item"
};

const response = await paymentClient.refund(request);
```

### Response

```javascript
{
    merchantRefundId: "refund_001",
    connectorRefundId: "re_3Oxxx...",
    status: "PENDING",
    refundAmount: {
        minorAmount: 1000,
        currency: "USD"
    },
    statusCode: 200
}
```

## Refund vs Void vs Reverse

| Action | When to Use | Timeline |
|--------|-------------|----------|
| **Void** | Before capture, during authorization hold | Same day |
| **Reverse** | After capture, before settlement | Same day |
| **Refund** | After settlement | 5-10 business days |

## Next Steps

- [getRefund](../refund-service/get.md) - Check refund status
- [capture](./capture.md) - Ensure payment is captured before refunding
- [void](./void.md) - Cancel if still in authorized state
