# accept Method

<!--
---
title: accept (Java SDK)
description: Concede a dispute using the Java SDK
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

The `accept` method concedes a dispute and accepts the loss.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | String | Yes | Dispute ID |
| `reason` | String | No | Internal reason |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | DisputeStatus | LOST |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("disputeId", "dp_xxx");

Map<String, Object> response = disputeClient.accept(request);
```