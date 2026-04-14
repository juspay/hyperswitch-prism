# createSdkSessionToken Method

<!--
---
title: createSdkSessionToken (PHP SDK)
description: Initialize wallet payment sessions using the PHP SDK
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

The `createSdkSessionToken` method initializes wallet payment sessions for Apple Pay, Google Pay.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantSdkSessionId` | string | Yes | Your SDK session reference |
| `amount` | Money | Yes | Payment amount |
| `paymentMethodType` | string | No | APPLE_PAY, GOOGLE_PAY |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `sessionToken` | array | Wallet-specific data |

## Example

```php
$request = [
    'merchantSdkSessionId' => 'sdk_session_001',
    'amount' => ['minorAmount' => 10000, 'currency' => 'USD'],
    'paymentMethodType' => 'APPLE_PAY'
];
$response = $authClient->createSdkSessionToken($request);
```