# Capture RPC

<!--
---
title: Capture
description: Finalize an authorized payment transaction - transfers reserved funds to complete the payment lifecycle
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `Capture` RPC finalizes an authorized payment by transferring the reserved funds from the customer's payment method to the merchant's account. This completes the two-step payment flow (authorize + capture), committing the transaction and triggering settlement.

**Business Use Case:** When an e-commerce order ships, a hotel guest checks out, or a marketplace seller fulfills an order, you need to actually charge the customer. Capture converts the held funds into actual revenue. Without capture, authorized funds are automatically released after a hold period (typically 7-10 days), resulting in lost sales and fulfillment costs.

## Purpose

**Why use capture instead of immediate charge?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **E-commerce fulfillment** | Call `Authorize` at checkout, listen for shipping event, call `Capture` with `connector_transaction_id` when order ships |
| **Hotel checkout** | Call `Authorize` at check-in for room + incidentals, adjust amount based on actual charges, call `Capture` at checkout |
| **Marketplace release** | Hold funds via `Authorize`, release to seller minus commission by calling `Capture` with reduced amount when seller ships |
| **Partial shipments** | Call `Capture` for each shipped item with partial amounts, keep remaining authorization for future shipments |
| **Tip adjustments** | Authorize base amount, capture higher amount including tip for hospitality transactions |

**Key outcomes:**
- Funds transferred to merchant account (typically 1-2 business days)
- Customer sees final charge on their statement
- Transaction moves to CAPTURED status for settlement
- Ability to capture partial amounts for split shipments

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_capture_id` | string | Yes | Your unique identifier for this capture operation |
| `connector_transaction_id` | string | Yes | The connector's transaction ID from the original authorization |
| `amount_to_capture` | Money | Yes | The amount to capture (can be less than or equal to authorized amount) |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `multiple_capture_data` | MultipleCaptureRequestData | No | Data for multiple partial captures |
| `browser_info` | BrowserInformation | No | Browser details for 3DS verification |
| `capture_method` | CaptureMethod | No | Method for capturing (MANUAL or AUTOMATIC) |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `test_mode` | bool | No | Process as test transaction |
| `merchant_order_id` | string | No | Your internal order ID for reference |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status of the payment (CAPTURED, PENDING, FAILED, etc.) |
| `error` | ErrorInfo | Error details if status is FAILED |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string, string> | Connector-specific response headers |
| `merchant_capture_id` | string | Your capture reference (echoed back) |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |
| `captured_amount` | int64 | Total captured amount in minor currency units |
| `mandate_reference` | MandateReference | Mandate details if recurring payment |
| `incremental_authorization_allowed` | bool | Whether amount can be increased later |
| `connector_feature_data` | SecretString | Connector-specific metadata for the transaction |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_capture_id": "capture_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "amount_to_capture": {
      "minor_amount": 1000,
      "currency": "USD"
    },
    "merchant_order_id": "order-001",
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentService/Capture

```

### Response

```json
{
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "CAPTURED",
  "status_code": 200,
  "merchant_capture_id": "capture_001",
  "captured_amount": 1000
}
```

## Next Steps

- [Authorize](./authorize.md) - Authorize a new payment (if additional charges needed)
- [Void](./void.md) - Cancel an authorization instead of capturing
- [Refund](./refund.md) - Return captured funds to customer
- [Get](./get.md) - Check current payment status
