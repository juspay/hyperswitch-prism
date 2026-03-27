# verifyRedirectResponse Method

<!--
---
title: verifyRedirectResponse (PHP SDK)
description: Validate redirect responses using the PHP SDK
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

The `verifyRedirectResponse` method validates payment responses from redirects.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantOrderId` | string | Yes | Your order reference |
| `requestDetails` | array | Yes | Redirect details |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `sourceVerified` | bool | Response authentic |
| `status` | PaymentStatus | Payment status |

## Example

```php
$request = [
    'merchantOrderId' => 'order_001',
    'requestDetails' => [
        'headers' => [...],
        'queryParams' => ['payment_intent' => 'pi_xxx']
    ]
];
$response = $paymentClient->verifyRedirectResponse($request);
```