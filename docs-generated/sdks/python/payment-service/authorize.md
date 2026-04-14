# authorize Method

<!--
---
title: authorize (Python SDK)
description: Authorize a payment using the Python SDK - reserve funds without capturing
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

The `authorize` method reserves funds on a customer's payment method without transferring them. This is the first step in a two-step payment flow (authorize + capture), commonly used in e-commerce, marketplaces, and subscription businesses.

**Business Use Case:** When a customer places an order, you want to verify their payment method has sufficient funds and lock those funds for fulfillment. The actual charge (capture) happens later when the order ships or service is delivered.

## Purpose

**Why use authorization instead of immediate charge?**

| Scenario | Benefit |
|----------|---------|
| **E-commerce fulfillment** | Verify funds at checkout, capture when order ships |
| **Hotel reservations** | Pre-authorize for incidentals, capture final amount at checkout |
| **Marketplace holds** | Secure funds from buyer before releasing to seller |
| **Subscription trials** | Validate card at signup, first charge after trial ends |

**Key outcomes:**
- Guaranteed funds availability (typically 7-10 days hold)
- Reduced fraud exposure through pre-verification
- Better customer experience (no double charges for partial shipments)
- Compliance with card network rules for delayed delivery

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_transaction_id` | string | Yes | Your unique transaction reference |
| `amount` | Money | Yes | The amount for the payment in minor units (e.g., 1000 = $10.00) |
| `order_tax_amount` | int64 | No | Tax amount for the order in minor units |
| `shipping_cost` | int64 | No | Cost of shipping for the order in minor units |
| `payment_method` | PaymentMethod | Yes | Payment method to be used (card, wallet, bank) |
| `capture_method` | CaptureMethod | No | Method for capturing. Values: MANUAL, AUTOMATIC |
| `customer` | Customer | No | Customer information for fraud scoring |
| `address` | PaymentAddress | No | Billing and shipping address |
| `auth_type` | AuthenticationType | Yes | Authentication flow type (e.g., THREE_DS, NO_THREE_DS) |
| `enrolled_for_3ds` | bool | No | Whether 3DS enrollment check passed |
| `authentication_data` | AuthenticationData | No | 3DS authentication results |
| `metadata` | SecretString | No | Additional metadata for the connector (max 20 keys) |
| `return_url` | string | No | URL to redirect customer after 3DS/redirect flow |
| `webhook_url` | string | No | URL for async webhook notifications |
| `test_mode` | bool | No | Process as test transaction |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_transaction_id` | string | Your transaction reference (echoed back) |
| `connector_transaction_id` | string | Connector's transaction ID (e.g., Stripe pi_xxx) |
| `status` | PaymentStatus | Current status: AUTHORIZED, PENDING, FAILED, etc. |
| `error` | ErrorInfo | Error details if status is FAILED |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `incremental_authorization_allowed` | bool | Whether amount can be increased later |

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
    "merchant_transaction_id": "txn_order_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": {"value": "4242424242424242"},
            "card_exp_month": {"value": "12"},
            "card_exp_year": {"value": "2027"},
            "card_cvc": {"value": "123"},
            "card_holder_name": {"value": "John Doe"}
        }
    },
    "auth_type": "NO_THREE_DS",
    "capture_method": "MANUAL",
    "test_mode": True
}

response = await payment_client.authorize(request)
```

### Response

```python
{
    "merchant_transaction_id": "txn_order_001",
    "connector_transaction_id": "pi_3Oxxx...",
    "status": "AUTHORIZED",
    "status_code": 200,
    "incremental_authorization_allowed": False
}
```

## Next Steps

- [capture](./capture.md) - Finalize the payment and transfer funds
- [void](./void.md) - Release held funds if order cancelled
- [get](./get.md) - Check current authorization status