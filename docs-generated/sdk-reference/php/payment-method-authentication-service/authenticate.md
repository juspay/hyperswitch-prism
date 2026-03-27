# authenticate Method

<!--
---
title: authenticate (PHP SDK)
description: Execute 3DS challenge or frictionless using the PHP SDK
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

The `authenticate` method executes the 3D Secure authentication step.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your order reference |
| `amount` | Money | Yes | Transaction amount |
| `paymentMethod` | PaymentMethod | Yes | Card details |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | AUTHENTICATED, FAILED |
| `authenticationData` | array | 3DS results |

## Example

```php
$request = [
    'merchantOrderId' => 'order_001',
    'amount' => ['minorAmount' => 10000, 'currency' => 'USD'],
    'paymentMethod' => ['card' => [...]]
];
$response = $authClient->authenticate($request);
```