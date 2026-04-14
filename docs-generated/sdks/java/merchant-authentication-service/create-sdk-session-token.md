# createSdkSessionToken Method

<!--
---
title: createSdkSessionToken (Java SDK)
description: Initialize wallet sessions using the Java SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: java
---
-->

## Overview

The `createSdkSessionToken` method initializes wallet payment sessions.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantSdkSessionId` | String | Yes | SDK session reference |
| `amount` | Money | Yes | Payment amount |
| `paymentMethodType` | String | No | APPLE_PAY, GOOGLE_PAY |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `sessionToken` | Map | Wallet session data |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantSdkSessionId", "sdk_session_001");
request.put("amount", Map.of("minorAmount", 10000, "currency", "USD"));
request.put("paymentMethodType", "APPLE_PAY");

Map<String, Object> response = authClient.createSdkSessionToken(request);
```