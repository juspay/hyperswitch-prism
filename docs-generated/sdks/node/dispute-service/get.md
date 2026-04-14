# get Method

<!--
---
title: get (Node.js SDK)
description: Retrieve dispute status using the Node.js SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: node
---
-->

## Overview

The `get` method retrieves the current status of a dispute, including evidence deadlines, submitted evidence, and the final decision.

**Business Use Case:** Check which disputes need evidence submission and which are approaching their deadlines.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | string | Yes | The connector's dispute ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `disputeId` | string | Connector's dispute ID |
| `paymentIntentId` | string | Related payment transaction ID |
| `status` | DisputeStatus | NEEDS_RESPONSE, UNDER_REVIEW, WON, LOST |
| `amount` | Money | Disputed amount |
| `reason` | string | Customer's dispute reason code |
| `evidenceDueBy` | string | ISO 8601 deadline |
| `evidenceSubmitted` | boolean | Whether evidence submitted |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { DisputeClient } = require('hyperswitch-prism');

const disputeClient = new DisputeClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    disputeId: "dp_xxx"
};

const response = await disputeClient.get(request);
```

### Response

```javascript
{
    disputeId: "dp_xxx",
    paymentIntentId: "pi_3Oxxx...",
    status: "NEEDS_RESPONSE",
    amount: {
        minorAmount: 1000,
        currency: "USD"
    },
    reason: "fraudulent",
    evidenceDueBy: "2024-02-15T23:59:59Z",
    evidenceSubmitted: false,
    statusCode: 200
}
```

## Status Values

| Status | Description | Action |
|--------|-------------|--------|
| `NEEDS_RESPONSE` | Dispute opened, awaiting response | Submit evidence or accept |
| `UNDER_REVIEW` | Evidence submitted, bank reviewing | Wait for decision |
| `WON` | Resolved in merchant favor | None |
| `LOST` | Resolved against merchant | Funds debited |

## Next Steps

- [submitEvidence](./submit-evidence.md) - Upload evidence if NEEDS_RESPONSE
- [defend](./defend.md) - Submit formal defense
- [accept](./accept.md) - Concede if evidence insufficient
