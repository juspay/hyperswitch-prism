# capture Method

<!--
---
title: capture (PHP SDK)
description: Finalize an authorized payment using the PHP SDK
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

The `capture` method finalizes an authorized payment by transferring reserved funds to your merchant account.

**Business Use Case:** An e-commerce order has shipped. Capture the funds to complete the transaction.

## Purpose

| Scenario | Benefit |
|----------|---------|
| E-commerce fulfillment | Charge when orders ship |
| Service completion | Bill after service rendered |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | string | Yes | Your unique transaction reference |
| `connectorTransactionId` | string | Yes | The connector's transaction ID |
| `amount` | Money | No | Amount to capture (can be partial) |
| `description` | string | No | Description for statement |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantTransactionId` | string | Your reference |
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | CAPTURED, PENDING, FAILED |
| `capturedAmount` | int | Amount captured |
| `statusCode` | int | HTTP status code |

## Example

### SDK Setup

```php
use HyperswitchPrism\PaymentClient;

$paymentClient = new PaymentClient([
    'connector' => 'stripe',
    'apiKey' => 'YOUR_API_KEY',
    'environment' => 'SANDBOX'
]);
```

### Request

```php
$request = [
    'merchantTransactionId' => 'txn_order_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'amount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'description' => 'Order shipment #12345'
];

$response = $paymentClient->capture($request);
```

### Response

```php
[
    'merchantTransactionId' => 'txn_order_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'status' => 'CAPTURED',
    'capturedAmount' => 1000,
    'statusCode' => 200
]
```

## Next Steps

- [authorize](./authorize.md) - Create initial authorization
- [void](./void.md) - Cancel instead of capturing