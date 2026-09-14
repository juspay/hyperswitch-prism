# CreateRecipient Method

<!--
---
title: CreateRecipient (Node SDK)
description: Create a recipient entity for payouts. Using the Node.js SDK.
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

The `createRecipient` method registers a new recipient entity (individual or business) with the payment processor. This is often required before funds can be transferred to them.

## Purpose

Use this operation to set up a vendor, contractor, or user in the processor's system.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Onboard a new seller | Call `createRecipient` with their details and `recipientType`. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the payout/recipient operation. |
| `address` | PayoutAddress | Yes | Address information associated with the recipient. |
| `payoutMethodData` | PayoutMethod | No | Specific details of the payout instrument for the recipient. |
| `amount` | Money | Yes | The amount to be paid out (if creating recipient inline with a payout). |
| `recipientType` | PayoutEnums.PayoutRecipientType | Yes | Type of entity (e.g., INDIVIDUAL, COMPANY). |
| `customer` | Customer | No | Details about the customer/recipient. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier passed to the payout processor. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The status of the recipient creation. |
| `connectorPayoutId` | string | No | The unique identifier assigned to the recipient or payout. |
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
    recipientType: "INDIVIDUAL",
    amount: {
        minorAmount: 0,
        currency: "USD"
    }
};

const response = await payoutClient.createRecipient(request);
```

### Response

```javascript
{
    payoutStatus: "SUCCESS",
    connectorPayoutId: "acct_1Hh1XYZ2eZvKYlo2C",
    statusCode: 200
}
```

## Next Steps

- [Transfer Payout](./transfer.md)
