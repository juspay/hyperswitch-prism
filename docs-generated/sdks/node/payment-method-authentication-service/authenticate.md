# authenticate Method

<!--
---
title: authenticate (Node.js SDK)
description: Execute 3DS challenge or frictionless verification using the Node.js SDK
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

The `authenticate` method executes the 3D Secure authentication step. For frictionless flows, it completes silently. For challenge flows, it presents the bank's authentication page.

**Business Use Case:** After initiating 3DS with preAuthenticate, this handles the actual authentication, managing customer interaction with the bank's page.

## Purpose

| Flow Type | What Happens |
|-----------|--------------|
| Frictionless | Completes without customer action |
| Challenge | Presents bank challenge page |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `paymentMethod` | PaymentMethod | Yes | Card details |
| `authenticationData` | object | No | Existing 3DS data from preAuthenticate |
| `returnUrl` | string | No | URL to redirect after authentication |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorTransactionId` | string | Connector's authentication ID |
| `status` | string | AUTHENTICATED, FAILED, PENDING |
| `authenticationData` | object | 3DS results (ECI, CAVV) |
| `redirectionData` | object | Challenge URL if needed |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { PaymentMethodAuthenticationClient } = require('hyperswitch-prism');

const authClient = new PaymentMethodAuthenticationClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    merchantOrderId: "order_001",
    amount: {
        minorAmount: 10000,
        currency: "USD"
    },
    paymentMethod: {
        card: {
            cardNumber: { value: "4242424242424242" },
            cardExpMonth: { value: "12" },
            cardExpYear: { value: "2027" }
        }
    },
    returnUrl: "https://your-app.com/3ds/return"
};

const response = await authClient.authenticate(request);
```

### Response - Frictionless

```javascript
{
    connectorTransactionId: "pi_3Oxxx...",
    status: "AUTHENTICATED",
    authenticationData: {
        eci: "05",
        cavv: "AAABBIIFmA=="
    },
    statusCode: 200
}
```

### Response - Challenge Required

```javascript
{
    connectorTransactionId: "pi_3Oxxx...",
    status: "PENDING",
    redirectionData: {
        url: "https://acs.bank.com/3ds/challenge",
        method: "POST"
    },
    statusCode: 200
}
```

## Next Steps

- [postAuthenticate](./post-authenticate.md) - Validate authentication results
- [Payment Service](../payment-service/README.md) - Process payment with auth data
