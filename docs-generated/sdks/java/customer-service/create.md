# create Method

<!--
---
title: create (Java SDK)
description: Create a customer record using the Java SDK
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

The `create` method creates a customer record in the payment processor.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantCustomerId` | String | Yes | Your customer reference |
| `email` | String | No | Customer email |
| `name` | String | No | Customer name |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorCustomerId` | String | Customer ID (cus_xxx) |
| `status` | CustomerStatus | ACTIVE |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("merchantCustomerId", "cust_12345");
request.put("email", "john@example.com");

Map<String, Object> response = customerClient.create(request);
```