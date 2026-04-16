# createOrder Method

<!--
---
title: createOrder (Node.js SDK)
description: Initialize an order in the payment processor using the Node.js SDK
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

The `createOrder` method initializes a payment order at the processor before collecting payment details. This enables improved fraud detection, session tokens for wallet payments, and better authorization rates.

**Business Use Case:** Set up the payment infrastructure before the customer reaches checkout, or when integrating wallet payments that require a session token.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Wallet payments | Generate session token for Apple/Google Pay |
| Pre-checkout | Prepare payment context early |
| Risk assessment | Allow processor fraud checks |
| Session continuity | Maintain context across pages |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your unique order reference |
| `amount` | Money | Yes | Expected payment amount |
| `webhookUrl` | string | No | URL for notifications |
| `metadata` | object | No | Additional data (max 20 keys) |
| `testMode` | boolean | No | Process as test transaction |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantOrderId` | string | Your reference (echoed back) |
| `connectorOrderId` | string | Connector's order ID |
| `status` | PaymentStatus | STARTED, FAILED |
| `sessionToken` | object | Session data for wallets |
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
    merchantOrderId: "order_001",
    amount: {
        minorAmount: 1000,
        currency: "USD"
    },
    webhookUrl: "https://your-app.com/webhooks/stripe",
    testMode: true
};

const response = await paymentClient.createOrder(request);
```

### Response

```javascript
{
    merchantOrderId: "order_001",
    connectorOrderId: "pi_3Oxxx...",
    status: "STARTED",
    sessionToken: {
        clientSecret: "pi_3Oxxx..._secret_xxx"
    },
    statusCode: 200
}
```

## Next Steps

- [authorize](./authorize.md) - Create payment (pass order context)
- [setupRecurring](./setup-recurring.md) - Set up recurring using order
