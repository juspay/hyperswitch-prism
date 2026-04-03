# revoke Method

<!--
---
title: revoke (PHP SDK)
description: Cancel a recurring payment mandate using the PHP SDK
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

The `revoke` method cancels an existing recurring payment mandate.

**Business Use Case:** A customer cancels their subscription. Stop future billing.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Subscription cancellation | Honor cancellations |
| Compliance | Meet regulatory requirements |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mandateId` | string | Yes | The mandate ID to revoke |
| `reason` | string | No | Reason for revocation |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `mandateId` | string | The revoked mandate ID |
| `status` | MandateStatus | REVOKED |
| `revokedAt` | string | ISO 8601 timestamp |
| `statusCode` | int | HTTP status code |

## Example

### SDK Setup

```php
use HyperswitchPrism\RecurringPaymentClient;

$recurringClient = new RecurringPaymentClient([
    'connector' => 'stripe',
    'apiKey' => 'YOUR_API_KEY',
    'environment' => 'SANDBOX'
]);
```

### Request

```php
$request = [
    'mandateId' => 'mandate_xxx',
    'reason' => 'customer_canceled'
];

$response = $recurringClient->revoke($request);
```

### Response

```php
[
    'mandateId' => 'mandate_xxx',
    'status' => 'REVOKED',
    'revokedAt' => '2024-01-15T10:30:00Z',
    'statusCode' => 200
]
```

## Next Steps

- [charge](./charge.md) - Process payments before revocation