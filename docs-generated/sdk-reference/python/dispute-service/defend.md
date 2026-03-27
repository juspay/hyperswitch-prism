# defend Method

<!--
---
title: defend (Python SDK)
description: Submit formal defense against a chargeback using the Python SDK
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

The `defend` method submits your formal argument against the customer's chargeback claim. This presents your case to the bank with a reason code and supporting evidence.

**Business Use Case:** After submitting delivery proof, you now submit your formal defense stating the chargeback is illegitimate because the customer received and signed for the package.

## Purpose

**Why use defend?**

| Reason Code | Use When |
|-------------|----------|
| `product_or_service_provided` | Customer received what they paid for |
| `customer_withdrew_dispute` | Customer contacted bank to withdraw |
| `duplicate_charge_doc` | Charge is legitimate and not duplicated |
| `cancellation_policy_disclosed` | Customer agreed to terms |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `dispute_id` | string | Yes | The connector's dispute ID |
| `reason_code` | string | Yes | Defense reason code |
| `explanation` | string | Yes | Detailed explanation of defense |
| `submit_evidence` | bool | No | Whether to submit attached evidence |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `dispute_id` | string | Connector's dispute ID |
| `defense_submitted` | bool | Whether defense was accepted |
| `status` | DisputeStatus | Updated status: UNDER_REVIEW |
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
    "dispute_id": "dp_xxx",
    "reason_code": "product_or_service_provided",
    "explanation": "Customer ordered product on 2024-01-10. Package was delivered on 2024-01-15 to the customer's verified billing address. Customer signed for delivery. Delivery confirmation and signature captured. Customer never contacted support about issues.",
    "submit_evidence": True
}

response = await dispute_client.defend(request)
```

### Response

```python
{
    "dispute_id": "dp_xxx",
    "defense_submitted": True,
    "status": "UNDER_REVIEW",
    "status_code": 200
}
```

## Defense Reason Codes

| Code | Description |
|------|-------------|
| `product_or_service_provided` | Product/service was delivered as described |
| `customer_withdrew_dispute` | Customer withdrew with their bank |
| `duplicate_charge_doc` | Charge is not a duplicate |
| `cancellation_policy_disclosed` | Customer accepted terms at purchase |
| `merchandise_or_service_not_as_described` | Product matched description |
| `credit_not_processed` | Refund was already provided |

## Next Steps

- [get](./get.md) - Check dispute status after submission
- [accept](./accept.md) - Consider conceding if defense is weak
- Wait for bank decision (typically takes 60-75 days)
