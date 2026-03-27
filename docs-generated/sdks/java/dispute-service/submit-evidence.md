# submitEvidence Method

<!--
---
title: submitEvidence (Java SDK)
description: Upload dispute evidence using the Java SDK
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

The `submitEvidence` method uploads evidence to contest a chargeback.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | String | Yes | Dispute ID |
| `evidenceType` | String | Yes | Type of evidence |
| `files` | List | Yes | File URLs |

## Example

```java
Map<String, Object> request = new HashMap<>();
request.put("disputeId", "dp_xxx");
request.put("evidenceType", "delivery_proof");
request.put("files", List.of("https://example.com/receipt.pdf"));

Map<String, Object> response = disputeClient.submitEvidence(request);
```