# authenticate Method

<!--
---
title: authenticate (Python SDK)
description: Execute 3DS challenge or frictionless verification using the Python SDK
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

The `authenticate` method executes the 3D Secure authentication step.

**Business Use Case:** After initiating 3DS with pre_authenticate, this handles the actual authentication.

## Purpose

| Flow Type | What Happens |
|-----------|--------------|
| Frictionless | Completes without action |
| Challenge | Presents bank challenge page |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | str | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `payment_method` | PaymentMethod | Yes | Card details |
| `return_url` | str | No | URL to redirect after authentication |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | str | Connector's authentication ID |
| `status` | str | AUTHENTICATED, FAILED, PENDING |
| `authentication_data` | dict | 3DS results |
| `status_code` | int | HTTP status code |

## Example

### SDK Setup

```python
from hyperswitch_prism import PaymentMethodAuthenticationClient

auth_client = PaymentMethodAuthenticationClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "merchant_order_id": "order_001",
    "amount": {
        "minor_amount": 10000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": {"value": "4242424242424242"},
            "card_exp_month": {"value": "12"},
            "card_exp_year": {"value": "2027"}
        }
    }
}

response = await auth_client.authenticate(request)
```

### Response

```python
{
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "AUTHENTICATED",
    "authentication_data": {
        "eci": "05",
        "cavv": "AAABBIIFmA=="
    },
    "status_code": 200
}
```

## Next Steps

- [post_authenticate](./post-authenticate.md) - Validate results