# createSdkSessionToken Method

<!--
---
title: createSdkSessionToken (Node.js SDK)
description: Initialize wallet payment sessions using the Node.js SDK
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

The `createSdkSessionToken` method initializes wallet payment sessions for Apple Pay, Google Pay, and other SDK-based payments. It sets up the secure context needed for tokenized wallet payments.

**Business Use Case:** When offering Apple Pay or Google Pay checkout, initialize a session with merchant configuration and payment details.

## Purpose

| Wallet | Purpose |
|--------|---------|
| Apple Pay | Initialize PKPaymentSession |
| Google Pay | Configure PaymentDataRequest |
| PayPal SDK | Set up checkout context |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantSdkSessionId` | string | Yes | Your unique SDK session reference |
| `amount` | Money | Yes | Payment amount |
| `paymentMethodType` | string | No | APPLE_PAY, GOOGLE_PAY |
| `countryCode` | string | No | ISO country code |
| `customer` | object | No | Customer information |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `sessionToken` | object | Wallet-specific session data |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { MerchantAuthenticationClient } = require('hyperswitch-prism');

const authClient = new MerchantAuthenticationClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    merchantSdkSessionId: "sdk_session_001",
    amount: {
        minorAmount: 10000,
        currency: "USD"
    },
    paymentMethodType: "APPLE_PAY",
    countryCode: "US",
    customer: {
        name: "John Doe",
        email: "john@example.com"
    }
};

const response = await authClient.createSdkSessionToken(request);
```

### Response

```javascript
{
    sessionToken: {
        applePay: {
            sessionData: "eyJtZXJjaGFudElkZW50aWZpZXIiOi...",
            displayMessage: "Example Store"
        }
    },
    statusCode: 200
}
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Process wallet payments
