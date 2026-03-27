# Get Method

<!--
---
title: Get (Node SDK)
description: Retrieve payment status using the Node.js SDK - synchronize payment state between systems
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

The `get` method retrieves the current payment status from the payment processor. This enables synchronization between your system and payment processors for accurate state tracking, especially important for handling asynchronous webhook delays or recovering from system outages.

**Business Use Case:** When a customer refreshes their order page, or your system needs to verify a payment's current state before proceeding with fulfillment. Payment statuses can change asynchronously through webhooks, and `get` ensures you have the most up-to-date information directly from the source.

## Purpose

**Why use get instead of relying solely on webhooks?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Order status page** | Call `get` when customer views order, display current status from `status` field |
| **Webhook recovery** | If webhook missed, call `get` with `connectorTransactionId` to sync state |
| **Pre-fulfillment check** | Before shipping, call `get` to confirm payment is CAPTURED, not just AUTHORIZED |
| **Multi-system sync** | Call `get` periodically to reconcile payment state across microservices |
| **Dispute handling** | Call `get` to verify payment details when responding to chargebacks |

**Key outcomes:**
- Accurate payment state for customer-facing displays
- Recovery from missed or delayed webhooks
- Confirmation before critical business actions (shipping, digital delivery)
- Audit trail verification for support inquiries

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `connectorTransactionId` | string | Yes | The connector's transaction ID from the original authorization |
| `encodedData` | string | No | Encoded data for retrieving payment status |
| `captureMethod` | CaptureMethod | No | Method for capturing. Values: MANUAL, AUTOMATIC |
| `handleResponse` | bytes | No | Raw response bytes from connector for state reconstruction |
| `amount` | Money | No | Amount information for verification |
| `setupFutureUsage` | FutureUsage | No | Future usage intent. Values: ON_SESSION, OFF_SESSION |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connectorFeatureData` | SecretString | No | Connector-specific metadata for the transaction |
| `syncType` | SyncRequestType | No | Type of synchronization request |
| `connectorOrderReferenceId` | string | No | Connector's order reference ID |
| `testMode` | bool | No | Process as test transaction |
| `paymentExperience` | PaymentExperience | No | Desired payment experience. Values: REDIRECT, EMBEDDED |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status. Values: STARTED, AUTHORIZED, CAPTURED, FAILED, VOIDED, CHARGED |
| `error` | ErrorInfo | Error details if status is FAILED |
| `statusCode` | uint32 | HTTP-style status code (200, 402, etc.) |
| `responseHeaders` | map<string, string> | Connector-specific response headers |
| `mandateReference` | MandateReference | Mandate details if recurring payment |
| `networkTransactionId` | string | Card network transaction reference |
| `amount` | Money | Original authorization amount |
| `capturedAmount` | int64 | Total captured amount in minor currency units |
| `paymentMethodType` | PaymentMethodType | Type of payment method used |
| `captureMethod` | CaptureMethod | How payment will be captured. Values: MANUAL, AUTOMATIC |
| `authType` | AuthenticationType | Authentication type used. Values: NO_THREE_DS, THREE_DS |
| `createdAt` | int64 | Unix timestamp when payment was created |
| `updatedAt` | int64 | Unix timestamp of last update |
| `authorizedAt` | int64 | Unix timestamp when payment was authorized |
| `capturedAt` | int64 | Unix timestamp when payment was captured |
| `customerName` | string | Customer name associated with payment |
| `email` | string | Customer email address |
| `connectorCustomerId` | string | Customer ID from the connector |
| `merchantOrderId` | string | Your internal order ID |
| `metadata` | SecretString | Additional metadata from the connector |
| `connectorResponse` | ConnectorResponseData | Raw connector response data |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `rawConnectorResponse` | SecretString | Raw API response from connector (debugging) |
| `rawConnectorRequest` | SecretString | Raw API request sent to connector (debugging) |
| `redirectionData` | RedirectForm | Redirect URL/form for 3DS or bank authentication |
| `incrementalAuthorizationAllowed` | bool | Whether amount can be increased later |

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
    connectorTransactionId: "pi_3Oxxx...",
    testMode: true
};

const response = await paymentClient.get(request);
```

### Response

```javascript
{
    connectorTransactionId: "pi_3Oxxx...",
    status: "AUTHORIZED",
    statusCode: 200,
    amount: {
        minorAmount: 1000,
        currency: "USD"
    },
    capturedAmount: 0,
    captureMethod: "MANUAL"
}
```

## Next Steps

- [Authorize](./authorize.md) - Authorize a new payment
- [Capture](./capture.md) - Finalize the payment and transfer funds
- [Void](./void.md) - Release held funds if order cancelled
