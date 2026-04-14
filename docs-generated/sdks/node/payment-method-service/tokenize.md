# tokenize Method

<!--
---
title: tokenize (Node.js SDK)
description: Store a payment method securely using the Node.js SDK
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

The `tokenize` method stores a payment method securely at the payment processor. It replaces sensitive card data with a secure token, enabling one-click payments without PCI compliance exposure.

**Business Use Case:** A customer wants to save their card for faster checkout next time. Tokenize the card details to create a secure token for future use.

## Purpose

| Scenario | Benefit |
|----------|---------|
| One-click checkout | No re-entry of card details |
| Recurring billing | Stored methods for subscriptions |
| PCI compliance | No raw card storage in your system |
| Security | Tokens are useless if compromised |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `paymentMethod` | PaymentMethod | Yes | Card or wallet details to tokenize |
| `customerId` | string | No | Associate with existing customer |
| `metadata` | object | No | Additional data (max 20 keys) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `paymentMethodId` | string | Token ID for future use (e.g., pm_xxx) |
| `paymentMethodType` | string | card, wallet, etc. |
| `status` | string | ACTIVE |
| `fingerprint` | string | Unique identifier for this card |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { PaymentMethodClient } = require('hyperswitch-prism');

const paymentMethodClient = new PaymentMethodClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    paymentMethod: {
        card: {
            cardNumber: { value: "4242424242424242" },
            cardExpMonth: { value: "12" },
            cardExpYear: { value: "2027" },
            cardCvc: { value: "123" },
            cardHolderName: { value: "John Doe" }
        }
    },
    customerId: "cus_xxx"
};

const response = await paymentMethodClient.tokenize(request);
```

### Response

```javascript
{
    paymentMethodId: "pm_3Oxxx...",
    paymentMethodType: "card",
    status: "ACTIVE",
    fingerprint: "abc123...",
    statusCode: 200
}
```

## Using Tokenized Payment Methods

```javascript
// Use the token in authorize
const authResponse = await paymentClient.authorize({
    merchantTransactionId: "txn_001",
    amount: { minorAmount: 1000, currency: "USD" },
    paymentMethod: {
        paymentMethodId: "pm_3Oxxx..."  // Use token instead of raw card
    },
    authType: "NO_THREE_DS"
});
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Use tokenized payment methods
- [Recurring Payment Service](../recurring-payment-service/README.md) - Set up subscriptions with stored cards
