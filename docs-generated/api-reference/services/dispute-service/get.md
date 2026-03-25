# Get RPC

<!--
---
title: Get
description: Retrieve dispute status and evidence submission state from the payment processor
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Get` RPC retrieves the current status and details of a dispute from the payment processor. Use this to track dispute progress, check evidence submission deadlines, and monitor resolution status.

**Business Use Case:** When you receive a dispute notification or need to check the status of an ongoing dispute, this RPC provides real-time information about the dispute state, evidence requirements, and resolution timeline.

## Purpose

**Why retrieve dispute status?**

| Scenario | Information Needed | Action |
|----------|-------------------|--------|
| **New dispute notification** | Dispute reason, amount, deadline | Determine defense strategy |
| **Evidence deadline approaching** | Time remaining, evidence submitted | Submit evidence before cutoff |
| **Awaiting resolution** | Current status, bank review progress | Plan for potential loss/profit impact |
| **Dispute resolved** | Final status (WON/LOST), funds movement | Update accounting records |

**Key outcomes:**
- Current dispute status and stage
- Evidence submission deadline
- Dispute reason and amount
- Evidence already submitted
- Resolution timeline

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_dispute_id` | string | Yes | Your unique dispute reference |
| `dispute_id` | string | No | Connector's dispute identifier |
| `connector_dispute_id` | string | Yes | Connector's dispute ID (alternative) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_dispute_id` | string | Connector's unique dispute identifier |
| `connector_transaction_id` | string | Original transaction ID |
| `dispute_status` | DisputeStatus | Current status: OPENED, EXPIRED, ACCEPTED, CHALLENGED, WON, LOST |
| `dispute_stage` | DisputeStage | Current stage: PRE_DISPUTE, DISPUTE, PRE_ARBITRATION, ARBITRATION |
| `connector_status_code` | string | Connector-specific status code |
| `error` | ErrorInfo | Error details if retrieval failed |
| `status_code` | uint32 | HTTP-style status code |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `dispute_amount` | Money | Amount being disputed |
| `dispute_date` | int64 | Unix timestamp when dispute was filed |
| `service_date` | int64 | Unix timestamp when service was provided |
| `shipping_date` | int64 | Unix timestamp when product was shipped |
| `due_date` | int64 | Unix timestamp for evidence submission deadline |
| `evidence_documents` | EvidenceDocument[] | Evidence documents already submitted |
| `dispute_reason` | string | Reason code for the dispute |
| `dispute_message` | string | Detailed dispute description |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_dispute_id": "dispute_001",
    "connector_dispute_id": "dp_1Oxxx..."
  }' \
  localhost:8080 \
  types.DisputeService/Get
```

### Response

```json
{
  "connector_dispute_id": "dp_1Oxxx...",
  "connector_transaction_id": "pi_3Oxxx...",
  "dispute_status": "OPENED",
  "dispute_stage": "DISPUTE",
  "dispute_amount": {
    "minor_amount": 10000,
    "currency": "USD"
  },
  "dispute_date": 1704067200,
  "due_date": 1706659200,
  "dispute_reason": "fraudulent",
  "dispute_message": "Customer claims they did not authorize this transaction",
  "status_code": 200
}
```

## Next Steps

- [SubmitEvidence](./submit-evidence.md) - Upload supporting documentation
- [Defend](./defend.md) - Submit formal defense
- [Accept](./accept.md) - Concede the dispute
