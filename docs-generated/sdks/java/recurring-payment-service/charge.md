# charge Method

<!--
---
title: charge (Java SDK)
description: Process a recurring payment using the Java SDK
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

The `charge` method processes a recurring payment using an existing mandate.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | String | Yes | Your reference |
| `amount` | Money | Yes | Amount to charge |
| `mandateId` | String | Yes | Mandate ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | PaymentStatus | SUCCEEDED, PENDING |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantTransactionId", "txn_sub_001");
request.put("amount", Map.of("minorAmount", 2900, "currency", "USD"));
request.put("mandateId", "mandate_xxx");

Map<String, Object> response = recurringClient.charge(request);
```