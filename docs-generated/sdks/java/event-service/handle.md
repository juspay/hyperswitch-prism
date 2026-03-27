# handle Method

<!--
---
title: handle (Java SDK)
description: Process webhook notifications using the Java SDK
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

The `handle` method processes webhook payloads.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantEventId` | String | Yes | Your event reference |
| `payload` | String | Yes | Raw webhook body |
| `headers` | Map | Yes | HTTP headers |
| `webhookSecret` | String | Yes | Signing secret |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `eventType` | String | Event type |
| `sourceVerified` | Boolean | Signature verified |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantEventId", "evt_001");
request.put("payload", payload);
request.put("headers", headers);
request.put("webhookSecret", "whsec_xxx");

Map<String, Object> response = eventClient.handle(request);
```