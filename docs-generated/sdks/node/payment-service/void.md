# Void Method

<!--
---
title: Void (Node SDK)
description: Cancel an authorized payment using the Node.js SDK - release held funds back to customer
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

The `void` method cancels an authorized payment before it has been captured. This releases the held funds back to the customer's payment method, effectively canceling the transaction without charging the customer.

**Business Use Case:** When a customer cancels their order before it ships, or an item is out of stock after authorization. Void is the appropriate action when no funds have been transferred yet. Unlike refunds (which return captured funds), voids prevent the charge from ever occurring.

## Purpose

**Why use Void instead of Refund?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Order cancellation** | Customer cancels before shipping - call `void` with `connectorTransactionId` to release held funds |
| **Out of stock** | Item unavailable after authorization - call `void` to cancel authorization, no charge appears on customer statement |
| **Fraud detection** | Suspicious order flagged before capture - call `void` to block the transaction entirely |
| **Duplicate prevention** | Accidental double authorization - call `void` on the duplicate to release the hold |
| **Price adjustment** | Order total needs to change - `void` the original, create new authorization with correct amount |

**Key outcomes:**
- No charge appears on customer's statement (authorization disappears within 1-3 business days)
- Funds immediately available to customer (vs. 5-10 days for refunds)
- Transaction moves to VOIDED status
- No settlement fees (vs. refund processing fees)

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantVoidId` | string | Yes | Your unique identifier for this void operation |
| `connectorTransactionId` | string | Yes | The connector's transaction ID from the original authorization |
| `cancellationReason` | string | No | Reason for canceling the authorization |
| `allKeysRequired` | bool | No | Whether all key fields must match for void to succeed |
| `browserInfo` | BrowserInformation | No | Browser details for 3DS verification |
| `amount` | Money | No | Amount to void (for partial voids) |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connectorFeatureData` | SecretString | No | Connector-specific metadata for the transaction |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `testMode` | bool | No | Process as test transaction |
| `merchantOrderId` | string | No | Your internal order ID for reference |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status. Values: VOIDED, VOID_INITIATED, VOID_FAILED |
| `error` | ErrorInfo | Error details if status is VOID_FAILED |
| `statusCode` | uint32 | HTTP-style status code (200, 402, etc.) |
| `responseHeaders` | map<string, string> | Connector-specific response headers |
| `merchantTransactionId` | string | Your transaction reference (echoed back) |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `rawConnectorRequest` | SecretString | Raw API request sent to connector (debugging) |
| `mandateReference` | MandateReference | Mandate details if recurring payment |
| `incrementalAuthorizationAllowed` | bool | Whether amount can be increased later |
| `connectorFeatureData` | SecretString | Connector-specific metadata for the transaction |

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
    merchantVoidId: "void_001",
    connectorTransactionId: "pi_3Oxxx...",
    cancellationReason: "Customer requested cancellation",
    merchantOrderId: "order-001",
    testMode: true
};

const response = await paymentClient.void(request);
```

### Response

```javascript
{
    connectorTransactionId: "pi_3Oxxx...",
    status: "VOIDED",
    statusCode: 200
}
```

## Next Steps

- [Authorize](./authorize.md) - Authorize a new payment
- [Capture](./capture.md) - Finalize the payment and transfer funds
- [Get](./get.md) - Check current payment status
