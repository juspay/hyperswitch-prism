# incrementalAuthorization Method

<!--
---
title: incrementalAuthorization (Java SDK)
description: Increase authorized amount using the Java SDK
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

The `incrementalAuthorization` method increases the authorized amount.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantAuthorizationId` | String | Yes | Incremental auth ID |
| `connectorTransactionId` | String | Yes | Original auth ID |
| `amount` | Money | Yes | New total amount |
| `reason` | String | No | Reason |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | AuthorizationStatus | AUTHORIZED |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantAuthorizationId", "incr_auth_001");
request.put("connectorTransactionId", "pi_xxx");
request.put("amount", Map.of("minorAmount", 1500, "currency", "USD"));
request.put("reason", "Room service");

Map<String, Object> response = paymentClient.incrementalAuthorization(request);
```