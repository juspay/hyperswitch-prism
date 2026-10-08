# EnrollDisburseAccount Method

<!--
---
title: EnrollDisburseAccount (Node SDK)
description: Enroll an account for disbursement of funds. Using the Node.js SDK.
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

The `enrollDisburseAccount` method registers and verifies a destination account (like a bank account) to receive disbursements from your platform.

## Purpose

Use this operation to securely enroll payout destinations.

| Scenario | Developer Implementation |
|----------|--------------------------|
| Link a vendor's bank account | Call `enrollDisburseAccount` with their bank details. |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier for the operation. |
| `address` | PayoutAddress | Yes | Address information associated with the account. |
| `payoutMethodData` | PayoutMethod | No | Specific details of the payout instrument being enrolled. |
| `amount` | Money | Yes | The amount to be paid out (if enrolling inline). |
| `customer` | Customer | No | Details about the customer/account holder. |
| `accessToken` | SecretString | No | Access token for the connector, if required. |

## Response Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantPayoutId` | string | No | Your internal identifier. |
| `payoutStatus` | PayoutEnums.PayoutStatus | No | The status of the enrollment. |
| `connectorPayoutId` | string | No | The unique identifier assigned by the connector. |
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
    amount: {
        minorAmount: 0,
        currency: "USD"
    },
    payoutMethodData: {
        ach: {
            bankAccountNumber: "000123456789",
            bankRoutingNumber: "110000000"
        }
    }
};

const response = await payoutClient.enrollDisburseAccount(request);
```

### Response

```javascript
{
    payoutStatus: "SUCCESS",
    connectorPayoutId: "ba_1Hh1XYZ2eZvKYlo2C",
    statusCode: 200
}
```

## Next Steps

- [Transfer Payout](./transfer.md)
