# accept Method

<!--
---
title: accept (Node.js SDK)
description: Concede a chargeback dispute using the Node.js SDK
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

The `accept` method concedes a dispute and accepts the chargeback loss. Use when evidence is insufficient or cost of defense exceeds dispute amount.

**Business Use Case:** Tracking shows the package was lost in transit. You cannot prove delivery, so you accept the dispute.

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `disputeId` | string | Yes | The connector's dispute ID |
| `reason` | string | No | Internal reason for accepting |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `disputeId` | string | Connector's dispute ID |
| `status` | DisputeStatus | LOST |
| `amountDebited` | Money | Amount charged back |
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
    reason: "Package lost in transit - no delivery confirmation"
};

const response = await disputeClient.accept(request);
```

### Response

```javascript
{
    disputeId: "dp_xxx",
    status: "LOST",
    amountDebited: {
        minorAmount: 1000,
        currency: "USD"
    },
    statusCode: 200
}
```

## When to Accept

Accept disputes when:
- No proof of delivery exists
- Defense cost exceeds dispute amount
- Customer's claim is valid
- The order was fraudulent

## Next Steps

- Reach out to customer for resolution
- Review processes to prevent similar disputes
