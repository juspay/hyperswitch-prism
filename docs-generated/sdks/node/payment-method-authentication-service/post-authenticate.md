# postAuthenticate Method

<!--
---
title: postAuthenticate (Node.js SDK)
description: Validate authentication results with the issuing bank using the Node.js SDK
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

The `postAuthenticate` method validates 3D Secure authentication results with the issuing bank. After the customer completes a challenge, this confirms the authentication was successful.

**Business Use Case:** After the customer returns from a 3DS challenge, validate the results before processing the payment to obtain liability shift.

## Purpose

| Scenario | Action |
|----------|--------|
| After challenge | Validate the authentication response |
| Before payment | Confirm authentication succeeded |
| Liability shift | Obtain ECI/CAVV values |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `paymentMethod` | PaymentMethod | Yes | Card details |
| `authenticationData` | object | No | 3DS result data from challenge |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorTransactionId` | string | Connector's authentication ID |
| `status` | string | AUTHENTICATED, FAILED |
| `authenticationData` | object | Validated 3DS data (ECI, CAVV) |
| `state` | object | State for payment authorization |
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
    authenticationData: {
        eci: "05",
        cavv: "AAABBIIFmAAAAAAAAAAAAAAAAAA="
    }
};

const response = await authClient.postAuthenticate(request);
```

### Response

```javascript
{
    connectorTransactionId: "pi_3Oxxx...",
    status: "AUTHENTICATED",
    authenticationData: {
        eci: "05",
        cavv: "AAABBIIFmAAAAAAAAAAAAAAAAAA=",
        transStatus: "Y"
    },
    statusCode: 200
}
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Authorize payment with 3DS data
