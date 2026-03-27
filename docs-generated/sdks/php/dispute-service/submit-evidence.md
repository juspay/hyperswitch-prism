# submitEvidence Method

<!--
---
title: submitEvidence (PHP SDK)
description: Upload evidence to dispute a chargeback using the PHP SDK
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

The `submitEvidence` method uploads supporting documentation to contest a chargeback.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | string | Yes | The dispute ID |
| `evidenceType` | string | Yes | Type of evidence |
| `files` | array | Yes | File URLs |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `disputeId` | string | The dispute ID |
| `evidenceSubmitted` | bool | Success status |
| `status` | DisputeStatus | Updated status |

## Example

```php
$request = [
    'disputeId' => 'dp_xxx',
    'evidenceType' => 'delivery_proof',
    'files' => ['https://example.com/receipt.pdf']
];
$response = $disputeClient->submitEvidence($request);
```