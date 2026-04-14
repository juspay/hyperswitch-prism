# Reverse RPC

<!--
---
title: Reverse
description: Reverse a captured payment before settlement - recover funds after capture but before bank settlement
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `Reverse` RPC cancels a captured payment before the funds have been settled to the merchant's account. Unlike voids (which apply to authorized but not captured payments), reverses apply to captured payments that haven't completed bank settlement yet.

**Business Use Case:** When an error is discovered immediately after capture, such as incorrect amount charged, duplicate transaction, or same-day order cancellation. Reverse allows you to recover funds before they enter the settlement cycle, avoiding the longer timeline and additional fees of a refund.

## Purpose

**Why use Reverse instead of Refund?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Same-day cancellation** | Customer cancels immediately after capture - call `Reverse` to recover funds before settlement |
| **Processing error** | Wrong amount captured due to system error - call `Reverse` to undo the incorrect capture |
| **Duplicate transaction** | Accidental double charge captured - call `Reverse` on the duplicate before settlement |
| **Fraud detected** | Fraudulent transaction caught post-capture - call `Reverse` to block settlement |
| **Technical glitch** | System issue caused erroneous capture - call `Reverse` to prevent fund transfer |

**Key outcomes:**
- Funds recovered before settlement (typically same-day)
- Faster than refunds (no 5-10 day wait)
- Lower or no processing fees compared to refunds
- Transaction status moves to REVERSED or VOIDED
- No charge appears on customer's final statement

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_reverse_id` | string | Yes | Your unique identifier for this reverse operation |
| `connector_transaction_id` | string | Yes | The connector's transaction ID from the original authorization |
| `cancellation_reason` | string | No | Reason for reversing the captured payment |
| `all_keys_required` | bool | No | Whether all key fields must match for reverse to succeed |
| `browser_info` | BrowserInformation | No | Browser details for 3DS verification |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status of the payment |
| `error` | ErrorInfo | Error details if reverse failed |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string, string> | Connector-specific response headers |
| `merchant_reverse_id` | string | Your reverse reference (echoed back) |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_reverse_id": "reverse_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "cancellation_reason": "Duplicate charge detected",
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentService/Reverse
```

### Response

```json
{
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "VOIDED",
  "status_code": 200,
  "merchant_reverse_id": "reverse_001"
}
```

## Next Steps

- [Capture](./capture.md) - Capture a payment after authorization
- [Void](./void.md) - Cancel an authorized payment before capture
- [Refund](./refund.md) - Return funds after settlement
- [Get](./get.md) - Check current payment status
