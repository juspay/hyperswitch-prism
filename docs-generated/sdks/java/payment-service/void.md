# void Method

<!--
---
title: void (Java SDK)
description: Cancel an authorized payment using the Java SDK
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

The `void` method cancels an authorized payment before capture.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | String | Yes | Your reference |
| `connectorTransactionId` | String | Yes | Connector's ID |
| `voidReason` | String | No | Reason for voiding |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | PaymentStatus | VOIDED |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantTransactionId", "txn_001");
request.put("connectorTransactionId", "pi_xxx");

Map<String, Object> response = paymentClient.void(request);
```

## Next Steps

- [authorize](./authorize.md) - Create authorization