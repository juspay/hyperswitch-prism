# Node.js SDK

<!--
---
title: Node.js SDK
description: Node.js SDK for the Hyperswitch Prism payment orchestration platform
last_updated: 2026-03-21
sdk_language: node
---
-->

## 🎯 What is Prism?

Today, integrating multiple payment processors either makes developers running in circles with AI agents to recreate integrations from specs, or developers spending months of engineering effort. 

Because every payment processor has diverse APIs, error codes, authentication methods, pdf documents to read, and above all - different behaviour in the actual environment when compared to documented specs. All this rests as tribal or undocumented knowledge making it harder AI agents which are very good at implementing clearly documented specification.

**Prism is a stateless, unified connector library for AI agents and Developers to connect with any payment processor**

**Prism offers hardened transformation through testing on payment processor environment & iterative bug fixing**

**Prism can be embedded in you server application with its wide range of multi-language SDKs, or run as a rRPC microservice**


| ❌ Without Prism | ✅ With Prism |
|------------------------------|----------------------------|
| 🗂️ 100+ different API schemas | 📋 Single unified schema |
| ⏳ Never ending agent loops/ months of integration work | ⚡ Hours to integrate, Agent driven |
| 🔗 Brittle, provider-specific code | 🔓 Portable, provider-agnostic code |
| 🚫 Hard to switch providers | 🔄 Change providers in 1 line |


## Installation

```bash
npm install hyperswitch-prism
```

## Quick Start

```javascript
const { PaymentClient } = require('hyperswitch-prism');

const paymentClient = new PaymentClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});

// Authorize a payment
const response = await paymentClient.authorize({
    merchantTransactionId: 'txn_order_001',
    amount: {
        minorAmount: 1000,
        currency: 'USD'
    },
    paymentMethod: {
        card: {
            cardNumber: { value: '4242424242424242' },
            cardExpMonth: { value: '12' },
            cardExpYear: { value: '2027' },
            cardCvc: { value: '123' },
            cardHolderName: { value: 'John Doe' }
        }
    },
    authType: 'NO_THREE_DS'
});

console.log(response.status); // AUTHORIZED
```

## Services

| Service | Description |
|---------|-------------|
| [Payment Service](./payment-service/README.md) | Process payments from authorization to settlement |
| [Recurring Payment Service](./recurring-payment-service/README.md) | Manage subscriptions and recurring billing |
| [Refund Service](./refund-service/README.md) | Retrieve and track refund statuses |
| [Dispute Service](./dispute-service/README.md) | Handle chargebacks and disputes |
| [Event Service](./event-service/README.md) | Process webhook notifications |
| [Payment Method Service](./payment-method-service/README.md) | Store and manage payment methods |
| [Customer Service](./customer-service/README.md) | Manage customer profiles |
| [Merchant Authentication Service](./merchant-authentication-service/README.md) | Generate access tokens |
| [Payment Method Authentication Service](./payment-method-authentication-service/README.md) | 3D Secure authentication |
| [Payout Service](./payout-service/README.md) | Send funds to recipients |

## Configuration

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `connector` | string | Yes | Payment connector name (stripe, adyen, etc.) |
| `apiKey` | string | Yes | Your API key |
| `environment` | string | Yes | SANDBOX or PRODUCTION |
| `timeout` | number | No | Request timeout in ms (default: 30000) |

## Error Handling

```javascript
try {
    const response = await paymentClient.authorize(request);
} catch (error) {
    if (error.code === 'PAYMENT_DECLINED') {
        // Handle declined payment
    } else if (error.code === 'VALIDATION_ERROR') {
        // Handle validation error
    } else {
        // Handle other errors
    }
}
```

## Support

For support and documentation, visit [https://docs.hyperswitch.io](https://docs.hyperswitch.io)
