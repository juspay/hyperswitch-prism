# Get Method

<!--
---
title: Get Refund
description: Retrieve refund status from the payment processor - track refund progress through processor settlement for accurate customer communication
last_updated: 2026-03-11
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: python
---
-->

## Overview

The `get` method retrieves the current status of a refund from the payment processor. Use this to check refund progress, provide customer updates, and synchronize refund states with your internal systems.

**Business Use Case:** When a customer asks about their refund status or when your system needs to verify the current state of a refund for reconciliation purposes. Refunds can take time to process (minutes to days depending on the processor), so checking status helps you provide accurate information to customers.

## Purpose

**Why use get for refunds?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Customer inquiry** | Customer asks "Where is my refund?" - call `get` to retrieve current status |
| **Reconciliation** | Daily financial sync - call `get` for all pending refunds to update internal records |
| **Status polling** | After initiating refund, periodically call `get` until status is SUCCEEDED or FAILED |
| **Support dashboard** | Build support tools showing real-time refund status from processors |
| **Audit trail** | Verify refund completed before closing support tickets |

**Key outcomes:**
- Current refund status (PENDING, SUCCEEDED, FAILED)
- Refund amount and currency confirmation
- Timestamps for refund lifecycle tracking
- Error details if refund failed

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_refund_id` | string | Yes | Your unique identifier for this refund |
| `connector_transaction_id` | string | Yes | The connector's transaction ID from the original payment |
| `refund_id` | string | Yes | The connector's refund ID (e.g., Stripe re_xxx) |
| `refund_reason` | string | No | Reason for the refund (for context) |
| `browser_info` | BrowserInformation | No | Browser information if relevant |
| `refund_metadata` | SecretString | No | Metadata specific to the refund sync |
| `state` | ConnectorState | No | State data for access token storage |
| `test_mode` | bool | No | Process as test transaction |
| `payment_method_type` | PaymentMethodType | No | Payment method type for context |
| `connector_feature_data` | SecretString | No | Connector-specific metadata |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_refund_id` | string | Your refund reference (echoed back) |
| `connector_refund_id` | string | Connector's ID for the refund |
| `status` | RefundStatus | Current status: PENDING, SUCCEEDED, FAILED |
| `error` | ErrorInfo | Error details if refund failed |
| `status_code` | uint32 | HTTP status code from the connector |
| `response_headers` | map<string, string> | Optional HTTP response headers |
| `refund_amount` | Money | Amount being refunded |
| `payment_amount` | int64 | Original payment amount |
| `refund_reason` | string | Reason for the refund |
| `created_at` | int64 | Unix timestamp when the refund was created |
| `updated_at` | int64 | Unix timestamp when the refund was last updated |
| `processed_at` | int64 | Unix timestamp when the refund was processed |

## Example

### SDK Setup

```python
from hyperswitch_prism import PaymentClient

payment_client = PaymentClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
from google.protobuf.json_format import ParseDict
from hyperswitch_prism.generated import payment_pb2

request = ParseDict(
    {
        "merchant_refund_id": "refund_001",
        "connector_transaction_id": "pi_3Oxxx...",
        "refund_id": "re_3Oxxx...",
        "refund_reason": "Customer returned item",
        "test_mode": True
    },
    payment_pb2.RefundServiceGetRequest()
)

response = await payment_client.get_refund(request)
```

### Response

```python
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
    "updated_at": 1709577600,
    "processed_at": 1709577600
}
```

## Status Values

| Status | Description | Typical Duration |
|--------|-------------|------------------|
| `PENDING` | Refund is being processed by the payment processor | Minutes to 5-10 business days |
| `SUCCEEDED` | Refund has completed successfully | Funds returned to customer |
| `FAILED` | Refund could not be processed | Check error details for reason |

## Next Steps

- [Refund](../payment-service/refund.md) - Initiate a new refund via Payment Service
- [Get Payment](../payment-service/get.md) - Check the original payment status
- [Event Service](../event-service/README.md) - Handle refund webhook notifications
