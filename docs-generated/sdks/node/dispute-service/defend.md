# defend Method

<!--
---
title: defend (Node.js SDK)
description: Submit formal defense against a chargeback using the Node.js SDK
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

The `defend` method submits your formal argument against the customer's chargeback claim.

**Business Use Case:** After submitting delivery proof, submit your formal defense stating why the chargeback is illegitimate.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | string | Yes | The connector's dispute ID |
| `reasonCode` | string | Yes | Defense reason code (see below) |
| `explanation` | string | Yes | Detailed explanation |
| `submitEvidence` | boolean | No | Whether to submit attached evidence |

## Defense Reason Codes

| Code | Description |
|------|-------------|
| `product_or_service_provided` | Product/service was delivered |
| `customer_withdrew_dispute` | Customer withdrew with their bank |
| `duplicate_charge_doc` | Charge is not a duplicate |
| `cancellation_policy_disclosed` | Customer accepted terms |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `disputeId` | string | Connector's dispute ID |
| `defenseSubmitted` | boolean | Success status |
| `status` | DisputeStatus | UNDER_REVIEW |
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
    disputeId: "dp_xxx",
    reasonCode: "product_or_service_provided",
    explanation: "Customer ordered product on 2024-01-10. Package delivered on 2024-01-15 with signature.",
    submitEvidence: true
};

const response = await disputeClient.defend(request);
```

### Response

```javascript
{
    disputeId: "dp_xxx",
    defenseSubmitted: true,
    status: "UNDER_REVIEW",
    statusCode: 200
}
```

## Next Steps

- [get](./get.md) - Check dispute status after submission
- Wait for bank decision (typically 60-75 days)
