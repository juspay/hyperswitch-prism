# incrementalAuthorization Method

<!--
---
title: incrementalAuthorization (PHP SDK)
description: Increase authorized amount using the PHP SDK
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

The `incrementalAuthorization` method increases the authorized amount.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantAuthorizationId` | string | Yes | Your incremental auth ID |
| `connectorTransactionId` | string | Yes | Original authorization ID |
| `amount` | Money | Yes | New total amount |
| `reason` | string | No | Reason for increase |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | AuthorizationStatus | AUTHORIZED |

## Example

```php
$request = [
    'merchantAuthorizationId' => 'incr_auth_001',
    'connectorTransactionId' => 'pi_3Oxxx...',
    'amount' => ['minorAmount' => 1500, 'currency' => 'USD'],
    'reason' => 'Room service charges'
];
$response = $paymentClient->incrementalAuthorization($request);
```