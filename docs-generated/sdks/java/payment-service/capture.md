# capture Method

<!--
---
title: capture (Java SDK)
description: Finalize an authorized payment using the Java SDK
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

The `capture` method finalizes an authorized payment by transferring reserved funds.

## Purpose

| Scenario | Benefit |
|----------|---------|
| E-commerce fulfillment | Charge when orders ship |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantTransactionId` | String | Yes | Your transaction reference |
| `connectorTransactionId` | String | Yes | Connector's transaction ID |
| `amount` | Money | No | Amount to capture |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | PaymentStatus | CAPTURED, FAILED |
| `capturedAmount` | Long | Amount captured |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantTransactionId", "txn_001");
request.put("connectorTransactionId", "pi_xxx");
request.put("amount", Map.of("minorAmount", 1000, "currency", "USD"));

Map<String, Object> response = paymentClient.capture(request);
```

## Next Steps

- [authorize](./authorize.md) - Create authorization
- [void](./void.md) - Cancel authorization