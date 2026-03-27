# charge Method

<!--
---
title: charge (PHP SDK)
description: Process a recurring payment using the PHP SDK
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

The `charge` method processes a recurring payment using an existing mandate.

**Business Use Case:** Your SaaS subscription renews. Charge the customer's stored payment method.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Subscription billing | Automate monthly charges |
| Membership dues | Process recurring fees |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | string | Yes | Your unique transaction reference |
| `amount` | Money | Yes | Amount to charge |
| `mandateId` | string | Yes | The mandate ID |
| `description` | string | No | Statement description |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantTransactionId` | string | Your reference |
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | SUCCEEDED, PENDING, FAILED |
| `statusCode` | int | HTTP status code |

## Example

### SDK Setup

```php
use HyperswitchPrism\RecurringPaymentClient;

$recurringClient = new RecurringPaymentClient([
    'connector' => 'stripe',
    'apiKey' => 'YOUR_API_KEY',
    'environment' => 'SANDBOX'
]);
```

### Request

```php
$request = [
    'merchantTransactionId' => 'txn_sub_monthly_001',
    'amount' => [
        'minorAmount' => 2900,
        'currency' => 'USD'
    ],
    'mandateId' => 'mandate_xxx',
    'description' => 'Monthly Pro Plan Subscription'
];

$response = $recurringClient->charge($request);
```

### Response

```php
[
    'merchantTransactionId' => 'txn_sub_monthly_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'status' => 'SUCCEEDED',
    'statusCode' => 200
]
```

## Next Steps

- [setupRecurring](../payment-service/setup-recurring.md) - Create mandate
- [revoke](./revoke.md) - Cancel recurring