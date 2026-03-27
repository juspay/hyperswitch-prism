# get Method

<!--
---
title: get (PHP SDK)
description: Retrieve refund status using the PHP SDK
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

The `get` method retrieves the current status of a refund from the payment processor.

**Business Use Case:** Check refund status for customer inquiries.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `connectorRefundId` | string | Yes | The connector's refund ID |
| `merchantRefundId` | string | No | Your refund reference |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorRefundId` | string | Connector's refund ID |
| `merchantRefundId` | string | Your reference |
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
    'connectorRefundId' => 're_3Oxxx...'
];

$response = $paymentClient->getRefund($request);
```

### Response

```php
[
    'connectorRefundId' => 're_3Oxxx...',
    'merchantRefundId' => 'refund_001',
    'status' => 'SUCCEEDED',
    'refundAmount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'statusCode' => 200
]
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Initiate refunds