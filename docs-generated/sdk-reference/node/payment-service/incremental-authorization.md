# incrementalAuthorization Method

<!--
---
title: incrementalAuthorization (Node.js SDK)
description: Increase authorized amount using the Node.js SDK
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

The `incrementalAuthorization` method increases the authorized amount on an existing authorization that is still in `AUTHORIZED` status. Use this for hospitality, tips, or add-on charges.

**Business Use Case:** A hotel guest adds room service charges to their folio. Increase the authorization hold to cover the additional charges.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Hotel incidentals | Add room service, mini-bar charges |
| Restaurant tips | Add post-dining tip |
| Add-on services | Cover additional costs |
| Metered services | Extend hold for actual usage |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantAuthorizationId` | string | Yes | Your unique incremental auth ID |
| `connectorTransactionId` | string | Yes | Original authorization ID |
| `amount` | Money | Yes | New total amount (not incremental amount) |
| `reason` | string | No | Reason for increase |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorAuthorizationId` | string | Connector's authorization ID |
| `status` | AuthorizationStatus | AUTHORIZED, FAILED |
| `error` | ErrorInfo | Error details if failed |
| `statusCode` | number | HTTP status code |

## Example

### SDK Setup

```javascript
const { PaymentClient } = require('hyperswitch-prism');

const paymentClient = new PaymentClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    merchantAuthorizationId: "incr_auth_001",
    connectorTransactionId: "pi_3Oxxx...",
    amount: {
        minorAmount: 1500,  // New total: $15.00 (was $10.00)
        currency: "USD"
    },
    reason: "Room service charges added"
};

const response = await paymentClient.incrementalAuthorization(request);
```

### Response

```javascript
{
    connectorAuthorizationId: "pi_3Oxxx...",
    status: "AUTHORIZED",
    statusCode: 200
}
```

## Important Notes

- The authorization must have `incrementalAuthorizationAllowed: true`
- Pass the new total amount, not the incremental amount
- Can only increase while in AUTHORIZED status

## Next Steps

- [authorize](./authorize.md) - Create initial authorization (set `requestIncrementalAuthorization: true`)
- [capture](./capture.md) - Finalize with increased amount
