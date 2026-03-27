# createOrder Method

<!--
---
title: createOrder (PHP SDK)
description: Initialize an order using the PHP SDK
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

The `createOrder` method initializes a payment order at the processor.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your order reference |
| `amount` | Money | Yes | Expected amount |
| `webhookUrl` | string | No | Notification URL |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorOrderId` | string | Connector's order ID |
| `status` | PaymentStatus | STARTED |
| `sessionToken` | array | Wallet session data |

## Example

```php
$request = [
    'merchantOrderId' => 'order_001',
    'amount' => ['minorAmount' => 1000, 'currency' => 'USD'],
    'webhookUrl' => 'https://your-app.com/webhooks'
];
$response = $paymentClient->createOrder($request);
```