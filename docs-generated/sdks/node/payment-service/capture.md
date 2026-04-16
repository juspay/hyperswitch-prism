# Capture Method

<!--
---
title: Capture (Node SDK)
description: Finalize an authorized payment using the Node.js SDK - transfer reserved funds to complete the payment
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

The `capture` method finalizes an authorized payment by transferring the reserved funds from the customer's payment method to the merchant's account. This completes the two-step payment flow (authorize + capture), committing the transaction and triggering settlement.

**Business Use Case:** When an e-commerce order ships, a hotel guest checks out, or a marketplace seller fulfills an order, you need to actually charge the customer. Capture converts the held funds into actual revenue. Without capture, authorized funds are automatically released after a hold period (typically 7-10 days), resulting in lost sales and fulfillment costs.

## Purpose

**Why use capture instead of immediate charge?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **E-commerce fulfillment** | Call `authorize` at checkout, listen for shipping event, call `capture` with `connectorTransactionId` when order ships |
| **Hotel checkout** | Call `authorize` at check-in for room + incidentals, adjust amount based on actual charges, call `capture` at checkout |
| **Marketplace release** | Hold funds via `authorize`, release to seller minus commission by calling `capture` with reduced amount when seller ships |
| **Partial shipments** | Call `capture` for each shipped item with partial amounts, keep remaining authorization for future shipments |
| **Tip adjustments** | Authorize base amount, capture higher amount including tip for hospitality transactions |

**Key outcomes:**
- Funds transferred to merchant account (typically 1-2 business days)
- Customer sees final charge on their statement
- Transaction moves to CAPTURED status for settlement
- Ability to capture partial amounts for split shipments

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchantCaptureId` | string | Yes | Your unique identifier for this capture operation |
| `connectorTransactionId` | string | Yes | The connector's transaction ID from the original authorization |
| `amountToCapture` | Money | Yes | The amount to capture (can be less than or equal to authorized amount) |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connectorFeatureData` | SecretString | No | Connector-specific metadata for the transaction |
| `multipleCaptureData` | MultipleCaptureRequestData | No | Data for multiple partial captures |
| `browserInfo` | BrowserInformation | No | Browser details for 3DS verification |
| `captureMethod` | CaptureMethod | No | Method for capturing (MANUAL or AUTOMATIC) |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `testMode` | bool | No | Process as test transaction |
| `merchantOrderId` | string | No | Your internal order ID for reference |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connectorTransactionId` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status of the payment (CAPTURED, PENDING, FAILED, etc.) |
| `error` | ErrorInfo | Error details if status is FAILED |
| `statusCode` | uint32 | HTTP-style status code (200, 402, etc.) |
| `responseHeaders` | map<string, string> | Connector-specific response headers |
| `merchantCaptureId` | string | Your capture reference (echoed back) |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `rawConnectorRequest` | SecretString | Raw API request sent to connector (debugging) |
| `capturedAmount` | int64 | Total captured amount in minor currency units |
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
    merchantCaptureId: "capture_001",
    connectorTransactionId: "pi_3Oxxx...",
    amountToCapture: {
        minorAmount: 1000,
        currency: "USD"
    },
    merchantOrderId: "order-001",
    testMode: true
};

const response = await paymentClient.capture(request);
```

### Response

```javascript
{
    connectorTransactionId: "pi_3Oxxx...",
    status: "CAPTURED",
    statusCode: 200,
    merchantCaptureId: "capture_001",
    capturedAmount: 1000
}
```

## Next Steps

- [Authorize](./authorize.md) - Authorize a new payment (if additional charges needed)
- [Void](./void.md) - Cancel an authorization instead of capturing
- [Refund](./refund.md) - Return captured funds to customer
- [Get](./get.md) - Check current payment status
