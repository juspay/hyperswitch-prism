# Python SDK

<!--
---
title: Python SDK
description: Python SDK for the Hyperswitch Prism payment orchestration platform
last_updated: 2026-03-21
sdk_language: python
---
-->

## 🎯 What is Prism?

Today, integrating multiple payment processors either makes developers running in circles with AI agents to recreate integrations from specs, or developers spending months of engineering effort. 

Because every payment processor has diverse APIs, error codes, authentication methods, pdf documents to read, and above all - different behaviour in the actual environment when compared to documented specs. All this rests as tribal or undocumented knowledge making it harder AI agents which are very good at implementing clearly documented specification.

**Prism is a stateless, unified connector library for AI agents and Developers to connect with any payment processor**

**Prism offers hardened transformation through testing on payment processor environment & iterative bug fixing**

**Prism can be embedded in you server application with its wide range of multi-language SDKs, or run as a rRPC microservice**


| ❌ Without Prism | ✅ With Prism |
|------------------------------|----------------------------|
| 🗂️ 100+ different API schemas | 📋 Single unified schema |
| ⏳ Never ending agent loops/ months of integration work | ⚡ Hours to integrate, Agent driven |
| 🔗 Brittle, provider-specific code | 🔓 Portable, provider-agnostic code |
| 🚫 Hard to switch providers | 🔄 Change providers in 1 line |

## Installation

```bash
pip install hyperswitch_prism
```

## Quick Start

```python
from hyperswitch_prism import PaymentClient

payment_client = PaymentClient(
    connector='stripe',
    api_key='YOUR_API_KEY',
    environment='SANDBOX'
)

# Authorize a payment
response = await payment_client.authorize({
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
    "auth_type": "NO_THREE_DS"
})

print(response["status"])  # AUTHORIZED
```

## Services

| Service | Description |
|---------|-------------|
| [Payment Service](./payment-service/README.md) | Process payments from authorization to settlement |
| [Recurring Payment Service](./recurring-payment-service/README.md) | Manage subscriptions and recurring billing |
| [Refund Service](./refund-service/README.md) | Retrieve and track refund statuses |
| [Dispute Service](./dispute-service/README.md) | Handle chargebacks and disputes |
| [Event Service](./event-service/README.md) | Process webhook notifications |
| [Payment Method Service](./payment-method-service/README.md) | Store and manage payment methods |
| [Customer Service](./customer-service/README.md) | Manage customer profiles |
| [Merchant Authentication Service](./merchant-authentication-service/README.md) | Generate access tokens |
| [Payment Method Authentication Service](./payment-method-authentication-service/README.md) | 3D Secure authentication |
| [Payout Service](./payout-service/README.md) | Send funds to recipients |

## Configuration

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `connector` | str | Yes | Payment connector name (stripe, adyen, etc.) |
| `api_key` | str | Yes | Your API key |
| `environment` | str | Yes | SANDBOX or PRODUCTION |
| `timeout` | int | No | Request timeout in seconds (default: 30) |

## Error Handling

```python
from hyperswitch_prism.exceptions import PaymentDeclined, ValidationError

try:
    response = await payment_client.authorize(request)
except PaymentDeclined as e:
    # Handle declined payment
    print(f"Payment declined: {e.message}")
except ValidationError as e:
    # Handle validation error
    print(f"Validation error: {e.errors}")
except HyperswitchError as e:
    # Handle other errors
    print(f"Error: {e.message}")
```

## Support

For support and documentation, visit [https://docs.hyperswitch.io](https://docs.hyperswitch.io)
