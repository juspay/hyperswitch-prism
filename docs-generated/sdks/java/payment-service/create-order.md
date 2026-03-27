# createOrder Method

<!--
---
title: createOrder (Java SDK)
description: Initialize an order using the Java SDK
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

The `createOrder` method initializes a payment order at the processor.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | String | Yes | Order reference |
| `amount` | Money | Yes | Expected amount |
| `webhookUrl` | String | No | Notification URL |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorOrderId` | String | Connector's order ID |
| `status` | PaymentStatus | STARTED |
| `sessionToken` | Map | Session data |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantOrderId", "order_001");
request.put("amount", Map.of("minorAmount", 1000, "currency", "USD"));
request.put("webhookUrl", "https://your-app.com/webhooks");

Map<String, Object> response = paymentClient.createOrder(request);
```