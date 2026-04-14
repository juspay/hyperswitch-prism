# Accept RPC

<!--
---
title: Accept
description: Concede dispute and accept chargeback loss when evidence is insufficient
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Accept` RPC concedes a chargeback dispute and accepts the loss. Use this when you lack sufficient evidence to defend, when the customer's claim is valid, or when the dispute amount is less than the cost of defense.

**Business Use Case:** Not all disputes are worth fighting. When you don't have delivery confirmation, the product was genuinely defective, or the dispute amount is small, accepting the chargeback avoids additional fees and administrative overhead.

## Purpose

**When to accept disputes?**

| Scenario | Reason | Outcome |
|----------|--------|---------|
| **No delivery proof** | Cannot prove product was delivered | Accept to avoid losing defense fee |
| **Valid customer complaint** | Product/service issue confirmed | Accept to maintain customer relationship |
| **Dispute amount < defense cost** | $20 dispute vs $15 defense fee | Accept to minimize total loss |
| **Evidence deadline passed** | Missed submission window | Accept as defense is no longer possible |
| **Fraudulent transaction confirmed** | Internal investigation confirmed fraud | Accept and write off loss |

**Key outcomes:**
- Dispute status changes to ACCEPTED
- Chargeback processed immediately
- Funds debited from your account
- Defense process terminated

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_dispute_id` | string | Yes | Your unique dispute reference |
| `connector_transaction_id` | string | Yes | Original transaction ID |
| `dispute_id` | string | Yes | Connector's dispute identifier |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `dispute_id` | string | Connector's dispute identifier |
| `dispute_status` | DisputeStatus | Updated status: ACCEPTED |
| `connector_status_code` | string | Connector-specific status code |
| `error` | ErrorInfo | Error details if acceptance failed |
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
    "dispute_id": "dp_1Oxxx..."
  }' \
  localhost:8080 \
  types.DisputeService/Accept
```

### Response

```json
{
  "dispute_id": "dp_1Oxxx...",
  "dispute_status": "ACCEPTED",
  "status_code": 200
}
```

## Next Steps

- [Get](./get.md) - Verify dispute status after acceptance
- [Payment Service](../payment-service/README.md) - Review original transaction
