# createSessionToken Method

<!--
---
title: createSessionToken (PHP SDK)
description: Create session token for payment processing using the PHP SDK
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

The `createSessionToken` method creates a session token for payment processing.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantSessionId` | string | Yes | Your session reference |
| `amount` | Money | Yes | Payment amount |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `sessionToken` | string | Token for operations |

## Example

```php
$request = [
    'merchantSessionId' => 'session_001',
    'amount' => ['minorAmount' => 10000, 'currency' => 'USD']
];
$response = $authClient->createSessionToken($request);
```