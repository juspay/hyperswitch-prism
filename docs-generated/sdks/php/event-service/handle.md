# handle Method

<!--
---
title: handle (PHP SDK)
description: Process webhook notifications using the PHP SDK
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

The `handle` method processes webhook payloads from payment processors.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantEventId` | string | Yes | Your event reference |
| `payload` | string | Yes | Raw webhook body |
| `headers` | array | Yes | HTTP headers |
| `webhookSecret` | string | Yes | Signing secret |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `eventType` | string | Event type |
| `sourceVerified` | bool | Signature verified |

## Example

```php
$request = [
    'merchantEventId' => 'evt_001',
    'payload' => file_get_contents('php://input'),
    'headers' => getallheaders(),
    'webhookSecret' => 'whsec_xxx'
];
$response = $eventClient->handle($request);
```