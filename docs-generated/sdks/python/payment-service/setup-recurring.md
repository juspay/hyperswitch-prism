# setup_recurring Method

<!--
---
title: setup_recurring (Python SDK)
description: Setup a recurring payment mandate using the Python SDK
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

The `setup_recurring` method establishes a payment mandate for future recurring charges. This enables subscription billing without requiring customer presence for each transaction.

**Business Use Case:** A customer signs up for your SaaS monthly plan. Setup a recurring mandate so you can charge their card automatically each month.

## Purpose

| Scenario | Benefit |
|----------|---------|
| SaaS subscriptions | Automate monthly billing |
| Utility bills | Enable automatic payments |
| Membership dues | Automate renewals |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_recurring_payment_id` | str | Yes | Your unique recurring setup ID |
| `amount` | Money | Yes | Initial amount for validation |
| `payment_method` | PaymentMethod | Yes | Card or payment method |
| `address` | PaymentAddress | Yes | Billing address |
| `auth_type` | str | Yes | THREE_DS or NO_THREE_DS |
| `setup_future_usage` | str | No | ON_SESSION or OFF_SESSION |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_recurring_payment_id` | str | Your reference |
| `connector_recurring_payment_id` | str | Connector's mandate ID |
| `status` | PaymentStatus | ACTIVE, FAILED |
| `mandate_reference` | dict | Mandate ID and status |
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
    "merchant_recurring_payment_id": "recurring_001",
    "amount": {
        "minor_amount": 2900,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": {"value": "4242424242424242"},
            "card_exp_month": {"value": "12"},
            "card_exp_year": {"value": "2027"},
            "card_cvc": {"value": "123"}
        }
    },
    "address": {
        "billing": {
            "line1": "123 Main St",
            "city": "San Francisco",
            "state": "CA",
            "zip": "94102",
            "country": "US"
        }
    },
    "auth_type": "NO_THREE_DS",
    "setup_future_usage": "OFF_SESSION"
}

response = await payment_client.setup_recurring(request)
```

### Response

```python
{
    "merchant_recurring_payment_id": "recurring_001",
    "connector_recurring_payment_id": "seti_3Oxxx...",
    "status": "ACTIVE",
    "mandate_reference": {
        "mandate_id": "pm_3Oxxx...",
        "mandate_status": "ACTIVE"
    },
    "status_code": 200
}
```

## Next Steps

- [RecurringPaymentService.charge](../recurring-payment-service/charge.md) - Use mandate for charges
- [RecurringPaymentService.revoke](../recurring-payment-service/revoke.md) - Cancel the mandate