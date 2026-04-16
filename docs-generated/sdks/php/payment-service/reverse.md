# reverse Method

<!--
---
title: reverse (PHP SDK)
description: Reverse a captured payment using the PHP SDK
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

The `reverse` method cancels a captured payment before settlement.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantReverseId` | string | Yes | Your reverse operation ID |
| `connectorTransactionId` | string | Yes | The transaction ID |
| `cancellationReason` | string | No | Reason for reversing |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | PaymentStatus | VOIDED, REVERSED |

## Example

```php
$request = [
    'merchantReverseId' => 'reverse_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'cancellationReason' => 'Duplicate charge'
];
$response = $paymentClient->reverse($request);
```