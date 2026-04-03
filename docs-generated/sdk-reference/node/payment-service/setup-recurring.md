# setupRecurring Method

<!--
---
title: setupRecurring (Node.js SDK)
description: Setup a recurring payment mandate using the Node.js SDK
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

The `setupRecurring` method establishes a payment mandate for future recurring charges. This enables subscription billing, automated bill payments, and installment plans without requiring customer presence for each transaction.

**Business Use Case:** A customer signs up for your SaaS monthly plan. Setup a recurring mandate so you can charge their card automatically each month.

## Purpose

| Scenario | Benefit |
|----------|---------|
| SaaS subscriptions | Automate monthly billing |
| Utility bills | Enable automatic payments |
| Installment plans | Schedule multiple payments |
| Membership dues | Automate renewals |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantRecurringPaymentId` | string | Yes | Your unique recurring setup ID |
| `amount` | Money | Yes | Initial amount for validation |
| `paymentMethod` | PaymentMethod | Yes | Card or payment method |
| `address` | PaymentAddress | Yes | Billing address |
| `authType` | string | Yes | THREE_DS or NO_THREE_DS |
| `setupFutureUsage` | string | No | ON_SESSION or OFF_SESSION |
| `customer` | object | No | Customer information |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantRecurringPaymentId` | string | Your reference (echoed back) |
| `connectorRecurringPaymentId` | string | Connector's mandate ID |
| `status` | PaymentStatus | ACTIVE, FAILED, PENDING |
| `mandateReference` | object | Mandate ID and status |
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
    merchantRecurringPaymentId: "recurring_001",
    amount: {
        minorAmount: 2900,
        currency: "USD"
    },
    paymentMethod: {
        card: {
            cardNumber: { value: "4242424242424242" },
            cardExpMonth: { value: "12" },
            cardExpYear: { value: "2027" },
            cardCvc: { value: "123" },
            cardHolderName: { value: "John Doe" }
        }
    },
    address: {
        billing: {
            line1: "123 Main St",
            city: "San Francisco",
            state: "CA",
            zip: "94102",
            country: "US"
        }
    },
    authType: "NO_THREE_DS",
    setupFutureUsage: "OFF_SESSION"
};

const response = await paymentClient.setupRecurring(request);
```

### Response

```javascript
{
    merchantRecurringPaymentId: "recurring_001",
    connectorRecurringPaymentId: "seti_3Oxxx...",
    status: "ACTIVE",
    mandateReference: {
        mandateId: "pm_3Oxxx...",
        mandateStatus: "ACTIVE"
    },
    statusCode: 200
}
```

## Next Steps

- [RecurringPaymentService.charge](../recurring-payment-service/charge.md) - Use the mandate for future charges
- [RecurringPaymentService.revoke](../recurring-payment-service/revoke.md) - Cancel the mandate
