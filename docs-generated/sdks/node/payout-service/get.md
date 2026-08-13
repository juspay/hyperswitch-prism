# Get Method

<!--
---
title: Get (Node SDK)
description: Retrieve the current status and details of a payout. Using the Node.js SDK.
last_updated: 2026-08-13
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: node
---
-->

## Overview

The `get` method allows you to check the current status of a payout. This is essential for syncing your internal systems with the payment processor's state, especially for asynchronous payout methods like bank transfers.

## Purpose

Use this operation to poll for the status of a payout or to verify details before taking further action.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Check if a payout succeeded | Call `get` with the `merchantPayoutId` or `connectorPayoutId`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the payout. |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the payout. |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector. |
| `error` | ErrorInfo | No | Details of any error that occurred. |
| `statusCode` | uint32 | Yes | The HTTP status code returned from the connector. |

## Example

### SDK Setup

```javascript
const { PayoutClient } = require('hyperswitch-prism');

const payoutClient = new PayoutClient({
    connector: 'stripe',
    apiKey: '[REDACTED_ENV_SECRET]',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    merchantPayoutId: "po_internal_12345"
};

const response = await payoutClient.get(request);
```

### Response

```javascript
{
    merchantPayoutId: "po_internal_12345",
    payoutStatus: "SUCCESS",
    connectorPayoutId: "po_1Hh1XYZ2eZvKYlo2C",
    statusCode: 200
}
```

## Next Steps

- [Void Payout](./void.md)
