# tokenize Method

<!--
---
title: tokenize (Java SDK)
description: Store a payment method using the Java SDK
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

The `tokenize` method stores a payment method securely.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `paymentMethod` | PaymentMethod | Yes | Card details |
| `customerId` | String | No | Customer ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `paymentMethodId` | String | Token ID |
| `status` | String | ACTIVE |

## Example

```java
Map<String, Object> card = new HashMap<>();
card.put("cardNumber", Map.of("value", "4242424242424242"));
card.put("cardExpMonth", Map.of("value", "12"));
card.put("cardExpYear", Map.of("value", "2027"));

Map<String, Object> request = new HashMap<>();
request.put("paymentMethod", Map.of("card", card));

Map<String, Object> response = paymentMethodClient.tokenize(request);
```