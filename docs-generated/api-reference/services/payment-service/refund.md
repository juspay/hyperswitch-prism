# Refund RPC

<!--
---
title: Refund
description: Initiate a refund to customer's payment method - return funds for returns, cancellations, or service adjustments after original payment
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `Refund` RPC returns funds to a customer's payment method after the original payment has been captured and settled. This is the standard way to handle returns, cancellations after fulfillment, or service adjustments in e-commerce and retail businesses.

**Business Use Case:** When a customer returns a product, cancels a service after delivery, or disputes a charge that you choose to honor. Refunds reverse the money flow, returning captured funds to the customer's payment method. This is different from voids or reverses which prevent or undo charges before settlement.

## Purpose

**Why use Refund?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Product returns** | Customer returns merchandise - call `Refund` with the original `connector_transaction_id` to return funds |
| **Post-settlement cancellation** | Service cancelled after delivery - call `Refund` to compensate customer for unused service |
| **Customer satisfaction** | Partial refund for service issues - call `Refund` with partial amount to maintain customer relationship |
| **Duplicate charges** | Settled duplicate transaction - call `Refund` to return extra charges after settlement |
| **Fraud goodwill** | Goodwill refund for unauthorized use - call `Refund` to resolve dispute amicably |

**Key outcomes:**
- Funds returned to customer's payment method (typically 5-10 business days)
- Refund appears as separate transaction on customer's statement
- Original payment remains captured and settled
- Refund status tracked separately from original payment
- Can be partial (less than original amount) or full

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_refund_id` | string | Yes | Your unique identifier for this refund operation |
| `connector_transaction_id` | string | Yes | The connector's transaction ID from the original authorization |
| `payment_amount` | int64 | Yes | Original payment amount in minor units |
| `refund_amount` | Money | Yes | Amount to refund (can be partial or full) |
| `reason` | string | No | Reason for the refund |
| `webhook_url` | string | No | URL for webhook notifications |
| `merchant_account_id` | string | No | Merchant account ID for the refund |
| `capture_method` | CaptureMethod | No | Capture method related to the original payment. Values: MANUAL, AUTOMATIC |
| `metadata` | SecretString | No | Metadata specific to the connector |
| `refund_metadata` | SecretString | No | Metadata specific to the refund |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `browser_info` | BrowserInformation | No | Browser information, if relevant |
| `state` | ConnectorState | No | State data for access token storage and other connector-specific state |
| `test_mode` | bool | No | Process as test transaction |
| `payment_method_type` | PaymentMethodType | No | Indicates the sub type of payment method |
| `customer_id` | string | No | Merchant's customer ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_refund_id` | string | Your refund reference (echoed back) |
| `connector_refund_id` | string | Connector's ID for the refund |
| `status` | RefundStatus | Current status of the refund. Values: PENDING, SUCCEEDED, FAILED |
| `error` | ErrorInfo | Error details if refund failed |
| `status_code` | uint32 | HTTP status code from the connector |
| `response_headers` | map<string, string> | Optional HTTP response headers from the connector |
| `refund_amount` | Money | Amount being refunded |
| `payment_amount` | int64 | Original payment amount |
| `refund_reason` | string | Reason for the refund |
| `created_at` | int64 | Unix timestamp when the refund was created |
| `updated_at` | int64 | Unix timestamp when the refund was last updated |
| `processed_at` | int64 | Unix timestamp when the refund was processed |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_refund_id": "refund_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "payment_amount": 1000,
    "refund_amount": {
      "minor_amount": 1000,
      "currency": "USD"
    },
    "reason": "Customer returned item",
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentService/Refund
```

### Response

```json
{
  "merchant_refund_id": "refund_001",
  "connector_refund_id": "re_3Oxxx...",
  "status": "SUCCEEDED",
  "status_code": 200,
  "refund_amount": {
    "minor_amount": 1000,
    "currency": "USD"
  },
  "payment_amount": 1000,
  "refund_reason": "Customer returned item",
  "created_at": 1709577600,
  "updated_at": 1709577600
}
```

## Next Steps

- [Capture](./capture.md) - Finalize a payment before refunding
- [Get](./get.md) - Check original payment status before refunding
- [Void](./void.md) - Cancel before capture (faster than refund)
- [Refund Service](../refund-service/README.md) - Additional refund operations
