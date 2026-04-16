# revoke Method

<!--
---
title: revoke (Java SDK)
description: Cancel a recurring mandate using the Java SDK
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

The `revoke` method cancels a recurring payment mandate.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mandateId` | String | Yes | Mandate ID to revoke |
| `reason` | String | No | Reason for revocation |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | MandateStatus | REVOKED |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("mandateId", "mandate_xxx");

Map<String, Object> response = recurringClient.revoke(request);
```