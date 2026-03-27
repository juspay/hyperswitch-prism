# createAccessToken Method

<!--
---
title: createAccessToken (PHP SDK)
description: Generate short-lived API access token using the PHP SDK
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

The `createAccessToken` method generates a short-lived authentication token.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `scope` | string | No | Token scope |
| `expiresIn` | int | No | Lifetime in seconds |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `accessToken` | string | Token string |
| `tokenType` | string | Bearer |
| `expiresIn` | int | Seconds until expiry |

## Example

```php
$request = ['scope' => 'payment:write', 'expiresIn' => 3600];
$response = $authClient->createAccessToken($request);
```