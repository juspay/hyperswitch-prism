# create Method

<!--
---
title: create (PHP SDK)
description: Create a customer record using the PHP SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: php
---
-->

## Overview

The `create` method creates a customer record in the payment processor system.

**Business Use Case:** A new user signs up. Create their profile for faster checkout.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Faster checkout | Skip entering details |
| Payment history | Track payments |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantCustomerId` | string | Yes | Your unique customer reference |
| `email` | string | No | Customer email |
| `name` | string | No | Customer name |
| `phone` | string | No | Customer phone |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantCustomerId` | string | Your reference |
| `connectorCustomerId` | string | Connector's customer ID |
| `status` | CustomerStatus | ACTIVE |
| `statusCode` | int | HTTP status code |

## Example

### SDK Setup

```php
use HyperswitchPrism\CustomerClient;

$customerClient = new CustomerClient([
    'connector' => 'stripe',
    'apiKey' => 'YOUR_API_KEY',
    'environment' => 'SANDBOX'
]);
```

### Request

```php
$request = [
    'merchantCustomerId' => 'cust_user_12345',
    'email' => 'john.doe@example.com',
    'name' => 'John Doe',
    'phone' => '+1-555-123-4567'
];

$response = $customerClient->create($request);
```

### Response

```php
[
    'merchantCustomerId' => 'cust_user_12345',
    'connectorCustomerId' => 'cus_xxx',
    'status' => 'ACTIVE',
    'statusCode' => 200
]
```

## Next Steps

- [Payment Method Service](../payment-method-service/README.md) - Store payment methods
- [Payment Service](../payment-service/README.md) - Process payments