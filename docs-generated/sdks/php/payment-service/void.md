# void Method

<!--
---
title: void (PHP SDK)
description: Cancel an authorized payment using the PHP SDK
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

The `void` method cancels an authorized payment before funds are captured. This releases held funds back to the customer.

**Business Use Case:** A customer cancels their order before it ships. Void the authorization to release funds.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Order cancellation | Release funds |
| Fulfillment failure | Clean up authorization |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | string | Yes | Your unique transaction reference |
| `connectorTransactionId` | string | Yes | The connector's transaction ID |
| `voidReason` | string | No | Reason for voiding |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantTransactionId` | string | Your reference |
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | VOIDED |
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
    'voidReason' => 'Customer cancelled order'
];

$response = $paymentClient->void($request);
```

### Response

```php
[
    'merchantTransactionId' => 'txn_order_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'status' => 'VOIDED',
    'statusCode' => 200
]
```

## Next Steps

- [authorize](./authorize.md) - Create initial authorization
- [capture](./capture.md) - Complete payment instead