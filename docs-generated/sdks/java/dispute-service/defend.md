# defend Method

<!--
---
title: defend (Java SDK)
description: Submit formal defense using the Java SDK
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

The `defend` method submits formal defense against a chargeback.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | String | Yes | Dispute ID |
| `reasonCode` | String | Yes | Defense reason code |
| `explanation` | String | Yes | Explanation |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("disputeId", "dp_xxx");
request.put("reasonCode", "product_or_service_provided");
request.put("explanation", "Product delivered successfully");

Map<String, Object> response = disputeClient.defend(request);
```