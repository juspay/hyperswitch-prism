# reverse Method

<!--
---
title: reverse (Java SDK)
description: Reverse a captured payment using the Java SDK
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

The `reverse` method cancels a captured payment before settlement.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantReverseId` | String | Yes | Reverse operation ID |
| `connectorTransactionId` | String | Yes | Transaction ID |
| `cancellationReason` | String | No | Reason |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | PaymentStatus | VOIDED, REVERSED |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantReverseId", "reverse_001");
request.put("connectorTransactionId", "pi_xxx");
request.put("cancellationReason", "Duplicate charge");

Map<String, Object> response = paymentClient.reverse(request);
```