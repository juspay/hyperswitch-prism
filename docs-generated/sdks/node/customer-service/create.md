# create Method

<!--
---
title: create (Node.js SDK)
description: Create a customer record using the Node.js SDK
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

The `create` method creates a customer record in the payment processor system. Storing customer details streamlines future transactions and can improve authorization rates.

**Business Use Case:** A new user signs up for your e-commerce platform. Create their customer profile to enable faster checkout on future purchases.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Faster checkout | Returning customers skip entering details |
| Payment history | Track all payments by customer |
| Fraud scoring | Established customers have better risk profiles |
| Subscriptions | Required for recurring billing setup |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantCustomerId` | string | Yes | Your unique customer reference |
| `email` | string | No | Customer email address |
| `name` | string | No | Customer full name |
| `phone` | string | No | Customer phone number |
| `description` | string | No | Internal description |
| `metadata` | object | No | Additional data (max 20 keys) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantCustomerId` | string | Your customer reference |
| `connectorCustomerId` | string | Connector's customer ID (e.g., Stripe's cus_xxx) |
| `status` | CustomerStatus | ACTIVE |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { CustomerClient } = require('hyperswitch-prism');

const customerClient = new CustomerClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    merchantCustomerId: "cust_user_12345",
    email: "john.doe@example.com",
    name: "John Doe",
    phone: "+1-555-123-4567",
    description: "Premium plan subscriber"
};

const response = await customerClient.create(request);
```

### Response

```javascript
{
    merchantCustomerId: "cust_user_12345",
    connectorCustomerId: "cus_xxx",
    status: "ACTIVE",
    statusCode: 200
}
```

## Best Practices

- Create customers at account signup, not first purchase
- Use consistent `merchantCustomerId` format
- Store `connectorCustomerId` for future reference
- Include email for processor communications

## Next Steps

- [Payment Method Service](../payment-method-service/README.md) - Store payment methods for customer
- [Payment Service](../payment-service/README.md) - Process payments with customer ID
- [Recurring Payment Service](../recurring-payment-service/README.md) - Set up subscriptions
