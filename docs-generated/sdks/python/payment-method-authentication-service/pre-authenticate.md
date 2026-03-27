# pre_authenticate Method

<!--
---
title: pre_authenticate (Python SDK)
description: Initiate 3D Secure flow using the Python SDK
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

The `pre_authenticate` method initiates the 3D Secure authentication flow. It determines whether frictionless or challenge-based verification is needed.

**Business Use Case:** Before processing a high-value transaction, initiate 3DS to reduce fraud liability.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Fraud prevention | Shift liability to issuer |
| SCA compliance | Meet EU requirements |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | str | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `payment_method` | PaymentMethod | Yes | Card details |
| `return_url` | str | Yes | URL to redirect after 3DS |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | str | Connector's 3DS transaction ID |
| `status` | str | FRICTIONLESS, AUTHENTICATION_REQUIRED |
| `authentication_data` | dict | Device data for next step |
| `redirection_data` | dict | Challenge URL if required |
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
    },
    "return_url": "https://your-app.com/3ds/return"
}

response = await auth_client.pre_authenticate(request)
```

### Response

```python
{
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "FRICTIONLESS",
    "authentication_data": {
        "eci": "05",
        "cavv": "AAABBIIFmA=="
    },
    "status_code": 200
}
```

## Next Steps

- [authenticate](./authenticate.md) - Complete the 3DS flow