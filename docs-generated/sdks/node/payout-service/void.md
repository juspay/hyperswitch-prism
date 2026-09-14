# Void Method

<!--
---
title: Void (Node SDK)
description: Cancel a pending payout. Using the Node.js SDK.
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

The `void` method cancels a payout that has been initiated but not yet completed or transferred. This is used when a payout needs to be stopped before the funds leave the merchant's account.

## Purpose

Use this operation to halt a payout process.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Cancel a scheduled payout | Call `void` with the `merchantPayoutId` and required details. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the payout. |
| `address` | PayoutAddress | Yes | Address information associated with the payout. |
| `connectorFeatureData` | SecretString | No | Connector-specific metadata or feature configurations. |
| `connectorPayoutId` | string | No | The unique payout ID assigned by the connector. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The current status of the payout (e.g., CANCELLED). |
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
    connectorPayoutId: "po_1Hh1XYZ2eZvKYlo2C"
};

const response = await payoutClient.void(request);
```

### Response

```javascript
{
    payoutStatus: "CANCELLED",
    connectorPayoutId: "po_1Hh1XYZ2eZvKYlo2C",
    statusCode: 200
}
```

## Next Steps

- [Get Payout Status](./get.md)
