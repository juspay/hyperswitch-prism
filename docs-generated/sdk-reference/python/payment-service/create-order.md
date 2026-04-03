# create_order Method

<!--
---
title: create_order (Python SDK)
description: Initialize an order in the payment processor using the Python SDK
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

The `create_order` method initializes a payment order at the processor before collecting payment details. This enables improved fraud detection and session tokens for wallet payments.

**Business Use Case:** Set up the payment infrastructure before checkout, or when integrating wallet payments that require a session token.

## Purpose

| Scenario | Benefit |
|----------|---------|
| Wallet payments | Session token for Apple/Google Pay |
| Pre-checkout | Prepare payment context early |
| Risk assessment | Allow processor fraud checks |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | str | Yes | Your unique order reference |
| `amount` | Money | Yes | Expected payment amount |
| `webhook_url` | str | No | URL for notifications |
| `test_mode` | bool | No | Process as test transaction |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_order_id` | str | Your reference |
| `connector_order_id` | str | Connector's order ID |
| `status` | PaymentStatus | STARTED, FAILED |
| `session_token` | dict | Session data for wallets |
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
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "webhook_url": "https://your-app.com/webhooks/stripe",
    "test_mode": True
}

response = await payment_client.create_order(request)
```

### Response

```python
{
    "merchant_order_id": "order_001",
    "connector_order_id": "pi_3Oxxx...",
    "status": "STARTED",
    "session_token": {
        "client_secret": "pi_3Oxxx..._secret_xxx"
    },
    "status_code": 200
}
```

## Next Steps

- [authorize](./authorize.md) - Create payment with order context