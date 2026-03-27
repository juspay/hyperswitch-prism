# preAuthenticate Method

<!--
---
title: preAuthenticate (PHP SDK)
description: Initiate 3D Secure flow using the PHP SDK
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

The `preAuthenticate` method initiates the 3D Secure authentication flow.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your order reference |
| `amount` | Money | Yes | Transaction amount |
| `paymentMethod` | PaymentMethod | Yes | Card details |
| `returnUrl` | string | Yes | URL to redirect |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | FRICTIONLESS, AUTHENTICATION_REQUIRED |
| `authenticationData` | array | 3DS data |

## Example

```php
$request = [
    'merchantOrderId' => 'order_001',
    'amount' => ['minorAmount' => 10000, 'currency' => 'USD'],
    'paymentMethod' => ['card' => [...]],
    'returnUrl' => 'https://your-app.com/3ds/return'
];
$response = $authClient->preAuthenticate($request);
```