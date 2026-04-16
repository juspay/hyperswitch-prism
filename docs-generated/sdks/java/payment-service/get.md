# get Method

<!--
---
title: get (Java SDK)
description: Retrieve payment status using the Java SDK
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

The `get` method retrieves payment status from the processor.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | String | Yes | Your reference |
| `connectorTransactionId` | String | Yes | Connector's ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | PaymentStatus | Current status |
| `amount` | Money | Payment amount |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantTransactionId", "txn_001");
request.put("connectorTransactionId", "pi_xxx");

Map<String, Object> response = paymentClient.get(request);
```