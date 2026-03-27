# verifyRedirectResponse Method

<!--
---
title: verifyRedirectResponse (Java SDK)
description: Validate redirect responses using the Java SDK
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

The `verifyRedirectResponse` method validates payment responses from redirects.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | String | Yes | Order reference |
| `requestDetails` | Map | Yes | Redirect details |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `sourceVerified` | Boolean | Response authentic |
| `status` | PaymentStatus | Payment status |

## Example

```java
Map<String, Object> requestDetails = new HashMap<>();
requestDetails.put("headers", headers);
requestDetails.put("queryParams", queryParams);

Map<String, Object> request = new HashMap<>();
request.put("merchantOrderId", "order_001");
request.put("requestDetails", requestDetails);

Map<String, Object> response = paymentClient.verifyRedirectResponse(request);
```