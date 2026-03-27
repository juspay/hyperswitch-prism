# verify_redirect_response Method

<!--
---
title: verify_redirect_response (Python SDK)
description: Validate redirect-based payment responses using the Python SDK
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

The `verify_redirect_response` method validates payment responses from redirect-based flows (3DS, bank redirects, wallet callbacks).

**Business Use Case:** When a customer returns from a 3DS challenge, verify the response is legitimate before fulfilling the order.

## Purpose

| Scenario | Benefit |
|----------|---------|
| 3DS completion | Validate authentication result |
| Bank redirect | Confirm payment success |
| Fraud prevention | Prevent spoofed notifications |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | str | Yes | Your unique order reference |
| `request_details` | dict | Yes | Redirect request details |
| `request_details.headers` | dict | Yes | HTTP headers |
| `request_details.query_params` | dict | Yes | URL query parameters |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_order_id` | str | Your reference |
| `source_verified` | bool | Whether response is authentic |
| `connector_transaction_id` | str | Connector's transaction ID |
| `status` | PaymentStatus | Current payment status |
| `status_code` | int | HTTP status code |

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
request = {
    "merchant_order_id": "order_001",
    "request_details": {
        "headers": {"Content-Type": "application/x-www-form-urlencoded"},
        "query_params": {
            "payment_intent": "pi_3Oxxx...",
            "payment_intent_client_secret": "pi_3Oxxx..._secret_xxx"
        },
        "body": ""
    }
}

response = await payment_client.verify_redirect_response(request)
```

### Response

```python
{
    "merchant_order_id": "order_001",
    "source_verified": True,
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "AUTHORIZED",
    "response_amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "status_code": 200
}
```

## Next Steps

- [capture](./capture.md) - Finalize payment after verification