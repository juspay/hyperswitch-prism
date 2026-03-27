# get Method

<!--
---
title: get (Java SDK)
description: Retrieve refund status using the Java SDK
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

The `get` method retrieves refund status from the processor.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `connectorRefundId` | String | Yes | Refund ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | RefundStatus | PENDING, SUCCEEDED |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("connectorRefundId", "re_xxx");

Map<String, Object> response = paymentClient.getRefund(request);
```