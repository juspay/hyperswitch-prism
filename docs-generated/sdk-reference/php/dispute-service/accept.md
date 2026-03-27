# accept Method

<!--
---
title: accept (PHP SDK)
description: Concede a chargeback dispute using the PHP SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: php
---
-->

## Overview

The `accept` method concedes a dispute and accepts the loss.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | string | Yes | The dispute ID |
| `reason` | string | No | Internal reason |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `disputeId` | string | The dispute ID |
| `status` | DisputeStatus | LOST |
| `amountDebited` | Money | Amount charged back |

## Example

```php
$request = [
    'disputeId' => 'dp_xxx',
    'reason' => 'Insufficient evidence'
];
$response = $disputeClient->accept($request);
```