# post_authenticate Method

<!--
---
title: post_authenticate (Python SDK)
description: Validate authentication results using the Python SDK
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

The `post_authenticate` method validates 3D Secure authentication results with the issuing bank.

**Business Use Case:** After customer completes a 3DS challenge, validate results before processing payment.

## Purpose

| Scenario | Action |
|----------|--------|
| After challenge | Validate response |
| Before payment | Confirm success |
| Liability shift | Obtain ECI/CAVV |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | str | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `payment_method` | PaymentMethod | Yes | Card details |
| `authentication_data` | dict | No | 3DS result data |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | str | Connector's authentication ID |
| `status` | str | AUTHENTICATED, FAILED |
| `authentication_data` | dict | Validated 3DS data |
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
    "authentication_data": {
        "eci": "05",
        "cavv": "AAABBIIFmA=="
    }
}

response = await auth_client.post_authenticate(request)
```

### Response

```python
{
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "AUTHENTICATED",
    "authentication_data": {
        "eci": "05",
        "cavv": "AAABBIIFmA==",
        "trans_status": "Y"
    },
    "status_code": 200
}
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Authorize with 3DS data