# get Method

<!--
---
title: get (Java SDK)
description: Retrieve dispute status using the Java SDK
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

The `get` method retrieves dispute status.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | String | Yes | Dispute ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | DisputeStatus | NEEDS_RESPONSE, WON, LOST |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("disputeId", "dp_xxx");

Map<String, Object> response = disputeClient.get(request);
```