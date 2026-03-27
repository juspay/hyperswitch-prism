# postAuthenticate Method

<!--
---
title: postAuthenticate (PHP SDK)
description: Validate authentication results using the PHP SDK
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

The `postAuthenticate` method validates 3D Secure results with the bank.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your order reference |
| `amount` | Money | Yes | Transaction amount |
| `paymentMethod` | PaymentMethod | Yes | Card details |
| `authenticationData` | array | No | 3DS result data |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | AUTHENTICATED, FAILED |
| `authenticationData` | array | Validated 3DS data |

## Example

```php
$request = [
    'merchantOrderId' => 'order_001',
    'amount' => ['minorAmount' => 10000, 'currency' => 'USD'],
    'authenticationData' => ['eci' => '05', 'cavv' => 'AAAB...']
];
$response = $authClient->postAuthenticate($request);
```