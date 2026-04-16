# refund Method

<!--
---
title: refund (PHP SDK)
description: Issue a refund using the PHP SDK
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

The `refund` method returns funds to a customer's payment method after a successful payment.

**Business Use Case:** A customer returns an item. Process a refund to return their money.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Product returns | Refund for returned merchandise |
| Service cancellation | Refund for unrendered services |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantRefundId` | string | Yes | Your unique refund reference |
| `connectorTransactionId` | string | Yes | The connector's transaction ID |
| `refundAmount` | Money | No | Amount to refund (omit for full) |
| `reason` | string | No | Reason for refund |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantRefundId` | string | Your reference |
| `connectorRefundId` | string | Connector's refund ID |
| `status` | RefundStatus | PENDING, SUCCEEDED, FAILED |
| `refundAmount` | Money | Refund amount |
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
    'merchantRefundId' => 'refund_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'refundAmount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'reason' => 'Customer returned item'
];

$response = $paymentClient->refund($request);
```

### Response

```php
[
    'merchantRefundId' => 'refund_001',
    'connectorRefundId' => 're_3Oxxx...',
    'status' => 'PENDING',
    'refundAmount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'statusCode' => 200
]
```

## Next Steps

- [getRefund](../refund-service/get.md) - Check refund status
- [capture](./capture.md) - Ensure payment captured