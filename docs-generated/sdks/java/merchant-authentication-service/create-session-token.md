# createSessionToken Method

<!--
---
title: createSessionToken (Java SDK)
description: Create session token using the Java SDK
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

The `createSessionToken` method creates a session token for payment processing.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantSessionId` | String | Yes | Session reference |
| `amount` | Money | Yes | Payment amount |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `sessionToken` | String | Token string |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantSessionId", "session_001");
request.put("amount", Map.of("minorAmount", 10000, "currency", "USD"));

Map<String, Object> response = authClient.createSessionToken(request);
```