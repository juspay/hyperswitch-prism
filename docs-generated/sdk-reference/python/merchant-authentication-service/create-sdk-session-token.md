# create_sdk_session_token Method

<!--
---
title: create_sdk_session_token (Python SDK)
description: Initialize wallet payment sessions using the Python SDK
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

The `create_sdk_session_token` method initializes wallet payment sessions for Apple Pay, Google Pay.

**Business Use Case:** When offering Apple Pay or Google Pay checkout, initialize a session with merchant configuration.

## Purpose

| Wallet | Purpose |
|--------|---------|
| Apple Pay | Initialize PKPaymentSession |
| Google Pay | Configure PaymentDataRequest |

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_sdk_session_id` | str | Yes | Your unique SDK session reference |
| `amount` | Money | Yes | Payment amount |
| `payment_method_type` | str | No | APPLE_PAY, GOOGLE_PAY |
| `country_code` | str | No | ISO country code |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `session_token` | dict | Wallet-specific session data |
| `status_code` | int | HTTP status code |

## Example

### SDK Setup

```python
from hyperswitch_prism import MerchantAuthenticationClient

auth_client = MerchantAuthenticationClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)
```

### Request

```python
request = {
    "merchant_sdk_session_id": "sdk_session_001",
    "amount": {
        "minor_amount": 10000,
        "currency": "USD"
    },
    "payment_method_type": "APPLE_PAY",
    "country_code": "US"
}

response = await auth_client.create_sdk_session_token(request)
```

### Response

```python
{
    "session_token": {
        "apple_pay": {
            "session_data": "eyJtZXJjaGFudElkZW50aWZpZXIiOi...",
            "display_message": "Example Store"
        }
    },
    "status_code": 200
}
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Process wallet payments