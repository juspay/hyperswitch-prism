# Defend RPC

<!--
---
title: Defend
description: Submit formal defense with reason code against customer's chargeback claim
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Defend` RPC submits a formal defense against a chargeback dispute. This presents your argument with a reason code to the issuing bank, along with any previously submitted evidence.

**Business Use Case:** After submitting evidence, you must formally contest the dispute before the deadline. This RPC notifies the bank that you are challenging the chargeback and provides your defense reasoning.

## Purpose

**Why formally defend disputes?**

| Scenario | Defense Strategy | Reason Code |
|----------|-----------------|-------------|
| **Product delivered** | Customer received product, delivery confirmed | GOODS_SERVICES_RECEIVED |
| **Valid transaction** | Customer authorized, 3DS authenticated | TRANSACTION_AUTHORIZED |
| **Refund already issued** | Customer was already refunded | REFUND_ISSUED |
| **Cancellation policy followed** | Customer cancelled outside policy window | CANCELLATION_POLICY_DISCLOSED |

**Key outcomes:**
- Dispute status changes to CHALLENGED
- Defense submitted to issuing bank
- Bank begins review process
- Awaiting final resolution (60-75 days)

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_dispute_id` | string | Yes | Your unique dispute reference |
| `connector_transaction_id` | string | Yes | Original transaction ID |
| `dispute_id` | string | Yes | Connector's dispute identifier |
| `reason_code` | string | No | Defense reason code (connector-specific) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `dispute_id` | string | Connector's dispute identifier |
| `dispute_status` | DisputeStatus | Updated status: CHALLENGED |
| `connector_status_code` | string | Connector-specific status code |
| `error` | ErrorInfo | Error details if defense failed |
| `status_code` | uint32 | HTTP-style status code |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `merchant_dispute_id` | string | Your dispute reference (echoed back) |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_dispute_id": "dispute_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "dispute_id": "dp_1Oxxx...",
    "reason_code": "goods_services_received"
  }' \
  localhost:8080 \
  types.DisputeService/Defend
```

### Response

```json
{
  "dispute_id": "dp_1Oxxx...",
  "dispute_status": "CHALLENGED",
  "status_code": 200
}
```

## Next Steps

- [Get](./get.md) - Check dispute resolution status
- [Accept](./accept.md) - Alternative: concede the dispute
