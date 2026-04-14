# verifyRedirectResponse Method

<!--
---
title: verifyRedirectResponse (Node.js SDK)
description: Validate redirect-based payment responses using the Node.js SDK
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

The `verifyRedirectResponse` method validates the authenticity of payment responses from redirect-based authentication flows (3DS, bank redirects, wallet callbacks).

**Business Use Case:** When a customer returns from a 3DS challenge or bank redirect, verify the response is legitimate before fulfilling the order.

## Purpose

| Scenario | Benefit |
|----------|---------|
| 3DS completion | Validate authentication result |
| Bank redirect | Confirm payment success |
| Wallet payment | Verify token authenticity |
| Fraud prevention | Prevent spoofed notifications |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your unique order reference |
| `requestDetails` | object | Yes | Redirect request details |
| `requestDetails.headers` | object | Yes | HTTP headers |
| `requestDetails.queryParams` | object | Yes | URL query parameters |
| `requestDetails.body` | string | Yes | Request body |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantOrderId` | string | Your reference (echoed back) |
| `sourceVerified` | boolean | Whether response is authentic |
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current payment status |
| `responseAmount` | Money | Verified amount |
| `error` | ErrorInfo | Error details if failed |

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
    merchantOrderId: "order_001",
    requestDetails: {
        headers: {
            "Content-Type": "application/x-www-form-urlencoded"
        },
        queryParams: {
            payment_intent: "pi_3Oxxx...",
            payment_intent_client_secret: "pi_3Oxxx..._secret_xxx"
        },
        body: ""
    }
};

const response = await paymentClient.verifyRedirectResponse(request);
```

### Response

```javascript
{
    merchantOrderId: "order_001",
    sourceVerified: true,
    connectorTransactionId: "pi_3Oxxx...",
    status: "AUTHORIZED",
    responseAmount: {
        minorAmount: 1000,
        currency: "USD"
    },
    statusCode: 200
}
```

## Next Steps

- [capture](./capture.md) - Finalize payment after verification
- [get](./get.md) - Check payment status
