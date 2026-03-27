# tokenize Method

<!--
---
title: tokenize (PHP SDK)
description: Store a payment method securely using the PHP SDK
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

The `tokenize` method stores a payment method securely at the processor.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `paymentMethod` | PaymentMethod | Yes | Card details |
| `customerId` | string | No | Customer association |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `paymentMethodId` | string | Token ID |
| `status` | string | ACTIVE |

## Example

```php
$request = [
    'paymentMethod' => [
        'card' => [
            'cardNumber' => ['value' => '4242424242424242'],
            'cardExpMonth' => ['value' => '12'],
            'cardExpYear' => ['value' => '2027']
        ]
    ]
];
$response = $paymentMethodClient->tokenize($request);
```