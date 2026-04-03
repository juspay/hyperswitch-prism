# submitEvidence Method

<!--
---
title: submitEvidence (Node.js SDK)
description: Upload evidence to dispute a chargeback using the Node.js SDK
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

The `submitEvidence` method uploads supporting documentation to contest a chargeback dispute.

**Business Use Case:** A customer disputed a charge claiming they never received their order. You have delivery confirmation and submit this evidence.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | string | Yes | The connector's dispute ID |
| `evidenceType` | string | Yes | delivery_proof, customer_communication, receipt, etc. |
| `files` | array | Yes | URLs to evidence files |
| `description` | string | No | Description of evidence |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `disputeId` | string | Connector's dispute ID |
| `evidenceSubmitted` | boolean | Success status |
| `status` | DisputeStatus | Updated dispute status |
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
    evidenceType: "delivery_proof",
    files: [
        "https://storage.example.com/delivery_receipt.pdf",
        "https://storage.example.com/tracking.png"
    ],
    description: "Package delivered on 2024-01-15 with signature"
};

const response = await disputeClient.submitEvidence(request);
```

### Response

```javascript
{
    disputeId: "dp_xxx",
    evidenceSubmitted: true,
    status: "NEEDS_RESPONSE",
    statusCode: 200
}
```

## Evidence Types

| Type | Use When |
|------|----------|
| `delivery_proof` | Physical goods delivered |
| `customer_communication` | Customer confirmed receipt |
| `receipt` | Proof of purchase |
| `cancellation_policy` | Customer agreed to terms |

## Next Steps

- [defend](./defend.md) - Submit formal defense after evidence
- [get](./get.md) - Check dispute status
