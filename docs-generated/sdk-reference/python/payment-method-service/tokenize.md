# tokenize Method

<!--
---
title: tokenize (Python SDK)
description: Store a payment method securely using the Python SDK
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

The `tokenize` method stores a payment method securely at the payment processor. It replaces sensitive card data with a secure token.

**Business Use Case:** A customer wants to save their card for faster checkout. Tokenize the card details to create a secure token.

## Purpose

| Scenario | Benefit |
|----------|---------|
| One-click checkout | No re-entry of card details |
| Recurring billing | Stored methods for subscriptions |
| PCI compliance | No raw card storage |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `payment_method` | PaymentMethod | Yes | Card or wallet details |
| `customer_id` | str | No | Associate with existing customer |
| `metadata` | dict | No | Additional data |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `payment_method_id` | str | Token ID (e.g., pm_xxx) |
| `payment_method_type` | str | card, wallet, etc. |
| `status` | str | ACTIVE |
| `status_code` | int | HTTP status code |

## Example

### SDK Setup

```python
from hyperswitch_prism import PaymentMethodClient

payment_method_client = PaymentMethodClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "payment_method": {
        "card": {
            "card_number": {"value": "4242424242424242"},
            "card_exp_month": {"value": "12"},
            "card_exp_year": {"value": "2027"},
            "card_cvc": {"value": "123"},
            "card_holder_name": {"value": "John Doe"}
        }
    },
    "customer_id": "cus_xxx"
}

response = await payment_method_client.tokenize(request)
```

### Response

```python
{
    "payment_method_id": "pm_3Oxxx...",
    "payment_method_type": "card",
    "status": "ACTIVE",
    "status_code": 200
}
```

## Using Tokenized Payment Methods

```python
# Use the token in authorize
auth_response = await payment_client.authorize({
    "merchant_transaction_id": "txn_001",
    "amount": {"minor_amount": 1000, "currency": "USD"},
    "payment_method": {
        "payment_method_id": "pm_3Oxxx..."
    },
    "auth_type": "NO_THREE_DS"
})
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Use tokenized methods
- [Recurring Payment Service](../recurring-payment-service/README.md) - Set up subscriptions