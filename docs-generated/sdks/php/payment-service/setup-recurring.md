# setupRecurring Method

<!--
---
title: setupRecurring (PHP SDK)
description: Setup a recurring payment mandate using the PHP SDK
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

The `setupRecurring` method establishes a payment mandate for future charges.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantRecurringPaymentId` | string | Yes | Your recurring setup ID |
| `amount` | Money | Yes | Initial amount |
| `paymentMethod` | PaymentMethod | Yes | Card details |
| `address` | PaymentAddress | Yes | Billing address |
| `authType` | string | Yes | THREE_DS or NO_THREE_DS |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | PaymentStatus | ACTIVE, FAILED |
| `mandateReference` | array | Mandate ID and status |

## Example

```php
$request = [
    'merchantRecurringPaymentId' => 'recurring_001',
    'amount' => ['minorAmount' => 2900, 'currency' => 'USD'],
    'paymentMethod' => ['card' => [...]],
    'address' => ['billing' => [...]],
    'authType' => 'NO_THREE_DS'
];
$response = $paymentClient->setupRecurring($request);
```