# get Method

<!--
---
title: get (Python SDK)
description: Retrieve dispute status using the Python SDK - track dispute progress
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

The `get` method retrieves the current status of a dispute, including evidence deadlines, submitted evidence, and the final decision if resolved.

**Business Use Case:** You need to check which disputes need evidence submission this week and which are approaching their deadlines.

## Purpose

**Why use get for disputes?**

| Scenario | Benefit |
|----------|---------|
| **Track deadlines** | Monitor evidence submission due dates |
| **Check status** | Know if dispute is won, lost, or pending |
| **Review evidence** | See what evidence has been submitted |
| **Reporting** | Build dispute analytics and trends |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `dispute_id` | string | Yes | The connector's dispute ID |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `dispute_id` | string | Connector's dispute ID |
| `payment_intent_id` | string | Related payment transaction ID |
| `status` | DisputeStatus | Current status: NEEDS_RESPONSE, UNDER_REVIEW, WON, LOST |
| `amount` | Money | Disputed amount |
| `reason` | string | Customer's dispute reason code |
| `evidence_due_by` | string | ISO 8601 deadline for evidence submission |
| `evidence_submitted` | bool | Whether evidence has been submitted |
| `status_code` | int | HTTP-style status code |

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
    "dispute_id": "dp_xxx"
}

response = await dispute_client.get(request)
```

### Response

```python
{
    "dispute_id": "dp_xxx",
    "payment_intent_id": "pi_3Oxxx...",
    "status": "NEEDS_RESPONSE",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "reason": "fraudulent",
    "evidence_due_by": "2024-02-15T23:59:59Z",
    "evidence_submitted": False,
    "status_code": 200
}
```

## Dispute Status Values

| Status | Description | Action Needed |
|--------|-------------|---------------|
| `NEEDS_RESPONSE` | Dispute opened, awaiting merchant response | Submit evidence or accept |
| `UNDER_REVIEW` | Evidence submitted, bank reviewing | Wait for decision |
| `WON` | Dispute resolved in merchant favor | None - retain funds |
| `LOST` | Dispute resolved against merchant | Funds debited, consider appeal |

## Next Steps

- [submit_evidence](./submit-evidence.md) - Upload evidence if NEEDS_RESPONSE
- [defend](./defend.md) - Formal defense submission
- [accept](./accept.md) - Concede if evidence is insufficient
