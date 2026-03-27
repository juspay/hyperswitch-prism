# get Method

<!--
---
title: get (PHP SDK)
description: Retrieve payment status using the PHP SDK
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

The `get` method retrieves the current status of a payment from the payment processor.

**Business Use Case:** A webhook was missed. Check the actual status with the processor before fulfilling.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Webhook fallback | Poll when webhooks fail |
| Reconciliation | Sync payment states |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | string | Yes | Your unique transaction reference |
| `connectorTransactionId` | string | Yes | Connector's transaction ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantTransactionId` | string | Your reference |
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status |
| `amount` | Money | Payment amount |
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
    'connectorTransactionId' => 'pi_3Oxxx...'
];

$response = $paymentClient->get($request);
```

### Response

```php
[
    'merchantTransactionId' => 'txn_order_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'status' => 'CAPTURED',
    'amount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'statusCode' => 200
]
```

## Next Steps

- [authorize](./authorize.md) - Create new authorization
- [capture](./capture.md) - Complete authorized payment