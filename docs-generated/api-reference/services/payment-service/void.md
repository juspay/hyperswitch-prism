# Void RPC

<!--
---
title: Void
description: Cancel an authorized payment before capture - release held funds back to customer
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `Void` RPC cancels an authorized payment before it has been captured. This releases the held funds back to the customer's payment method, effectively canceling the transaction without charging the customer.

**Business Use Case:** When a customer cancels their order before it ships, or an item is out of stock after authorization. Void is the appropriate action when no funds have been transferred yet. Unlike refunds (which return captured funds), voids prevent the charge from ever occurring.

## Purpose

**Why use Void instead of Refund?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Order cancellation** | Customer cancels before shipping - call `Void` with `connector_transaction_id` to release held funds |
| **Out of stock** | Item unavailable after authorization - call `Void` to cancel authorization, no charge appears on customer statement |
| **Fraud detection** | Suspicious order flagged before capture - call `Void` to block the transaction entirely |
| **Duplicate prevention** | Accidental double authorization - call `Void` on the duplicate to release the hold |
| **Price adjustment** | Order total needs to change - `Void` the original, create new authorization with correct amount |

**Key outcomes:**
- No charge appears on customer's statement (authorization disappears within 1-3 business days)
- Funds immediately available to customer (vs. 5-10 days for refunds)
- Transaction moves to VOIDED status
- No settlement fees (vs. refund processing fees)

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_void_id` | string | Yes | Your unique identifier for this void operation |
| `connector_transaction_id` | string | Yes | The connector's transaction ID from the original authorization |
| `cancellation_reason` | string | No | Reason for canceling the authorization |
| `all_keys_required` | bool | No | Whether all key fields must match for void to succeed |
| `browser_info` | BrowserInformation | No | Browser details for 3DS verification |
| `amount` | Money | No | Amount to void (for partial voids) |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `test_mode` | bool | No | Process as test transaction |
| `merchant_order_id` | string | No | Your internal order ID for reference |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status. Values: VOIDED, VOID_INITIATED, VOID_FAILED |
| `error` | ErrorInfo | Error details if status is VOID_FAILED |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string, string> | Connector-specific response headers |
| `merchant_transaction_id` | string | Your transaction reference (echoed back) |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |
| `mandate_reference` | MandateReference | Mandate details if recurring payment |
| `incremental_authorization_allowed` | bool | Whether amount can be increased later |
| `connector_feature_data` | SecretString | Connector-specific metadata for the transaction |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_void_id": "void_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "cancellation_reason": "Customer requested cancellation",
    "merchant_order_id": "order-001",
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentService/Void
```

### Response

```json
{
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "VOIDED",
  "status_code": 200
}
```

## Next Steps

- [Authorize](./authorize.md) - Authorize a new payment
- [Capture](./capture.md) - Finalize the payment and transfer funds
- [Get](./get.md) - Check current payment status
