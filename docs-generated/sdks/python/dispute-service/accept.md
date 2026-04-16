# accept Method

<!--
---
title: accept (Python SDK)
description: Concede a chargeback dispute using the Python SDK - accept the loss
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: python
---
-->

## Overview

The `accept` method concedes a dispute and accepts the chargeback loss. Use this when evidence is insufficient, the cost of defense outweighs the dispute amount, or the customer's claim is valid.

**Business Use Case:** A customer disputed a charge claiming the product was never delivered. Your tracking shows it was lost in transit. Since you cannot prove delivery, you accept the dispute and issue a refund.

## Purpose

**Why accept disputes?**

| Scenario | Reason |
|----------|--------|
| **Insufficient evidence** | No proof of delivery or service |
| **Cost-benefit** | Defense cost exceeds dispute amount |
| **Valid claim** | Customer's dispute is legitimate |
| **Fraudulent order** | Order was fraud, not worth defending |

**Key outcomes:**
- Dispute closed immediately
- Funds debited from your account
- No additional fees or penalties
- Clean dispute resolution

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `dispute_id` | string | Yes | The connector's dispute ID |
| `reason` | string | No | Internal reason for accepting |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `dispute_id` | string | Connector's dispute ID |
| `status` | DisputeStatus | New status: LOST |
| `amount_debited` | Money | Amount charged back |
| `status_code` | int | HTTP-style status code (200, 404, etc.) |

## Example

### SDK Setup

```python
from hyperswitch_prism import DisputeClient

dispute_client = DisputeClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "dispute_id": "dp_xxx",
    "reason": "Tracking shows package lost in transit - no delivery confirmation available"
}

response = await dispute_client.accept(request)
```

### Response

```python
{
    "dispute_id": "dp_xxx",
    "status": "LOST",
    "amount_debited": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "status_code": 200
}
```

## Decision Framework

```
┌─────────────────────────────────────────────────────────┐
│          Should I Accept This Dispute?                 │
└─────────────────────────────────────────────────────────┘
                          │
           ┌──────────────┼──────────────┐
           ▼              ▼              ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ Evidence │   │ Cost >   │   │ Valid    │
    │ Exists?  │   │ Dispute? │   │ Claim?   │
    └────┬─────┘   └────┬─────┘   └────┬─────┘
         │              │              │
        No            Yes            Yes
         │              │              │
         └──────────────┴──────────────┘
                        │
                        ▼
              ┌─────────────────┐
              │ ACCEPT DISPUTE  │
              └─────────────────┘
```

## After Acceptance

- Dispute is immediately closed
- Chargeback amount is debited
- Consider reaching out to customer for resolution
- Update internal records

## Next Steps

- [get](./get.md) - Review dispute details before accepting
- Consider customer outreach for retention
- Review processes to prevent similar disputes
