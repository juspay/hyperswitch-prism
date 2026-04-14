# CreateOrder RPC

<!--
---
title: CreateOrder
description: Initialize an order in the payment processor system - sets up payment context before customer enters card details for improved authorization rates
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `CreateOrder` RPC initializes a payment order at the payment processor before collecting payment details from the customer. This pre-establishes the payment context, enabling features like improved fraud detection, session tokens for wallet payments (Apple Pay, Google Pay), and better authorization rates by associating the eventual payment with a previously created order.

**Business Use Case:** When you want to set up the payment infrastructure before the customer reaches the checkout page, or when integrating wallet payments that require a session token. Creating an order first allows the processor to perform risk assessments and prepare the payment session, resulting in smoother checkout experiences and higher conversion rates.

## Purpose

**Why use CreateOrder?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Wallet payments** | Apple Pay or Google Pay integration - call `CreateOrder` to generate session token for wallet SDK |
| **Pre-checkout setup** | Customer adds items to cart - call `CreateOrder` early to prepare payment context |
| **Risk assessment** | High-value transactions - call `CreateOrder` to allow processor fraud checks before payment details |
| **Order tracking** | Complex order flows - call `CreateOrder` to establish order ID for tracking across systems |
| **Session continuity** | Multi-page checkout - call `CreateOrder` to maintain payment context across pages |

**Key outcomes:**
- Order context established at processor
- Session token for wallet payment SDKs
- Improved authorization rates
- Better fraud detection through early context
- Order ID for cross-system tracking

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | string | Yes | Your unique identifier for this order |
| `amount` | Money | Yes | The expected payment amount |
| `webhook_url` | string | No | URL for webhook notifications |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `test_mode` | bool | No | Process as test transaction |
| `payment_method_type` | PaymentMethodType | No | The type of payment method (e.g., apple_pay, google_pay) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_order_id` | string | Identifier for the created order at the connector |
| `status` | PaymentStatus | Status of the order creation attempt |
| `error` | ErrorInfo | Error details if order creation failed |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string, string> | Connector-specific response headers |
| `merchant_order_id` | string | Your order reference (echoed back) |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |
| `raw_connector_response` | SecretString | Raw API response from connector (debugging) |
| `session_token` | SessionToken | JSON serialized session token for wallet payments |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_order_id": "order_001",
    "amount": {
      "minor_amount": 1000,
      "currency": "USD"
    },
    "webhook_url": "https://your-app.com/webhooks/stripe",
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentService/CreateOrder
```

### Response

```json
{
  "connector_order_id": "pi_3Oxxx...",
  "status": "STARTED",
  "status_code": 200,
  "merchant_order_id": "order_001",
  "session_token": {
    "client_secret": "pi_3Oxxx..._secret_xxx"
  }
}
```

## Next Steps

- [Authorize](./authorize.md) - Create payment authorization (pass `order_context` from CreateOrder response)
- [SetupRecurring](./setup-recurring.md) - Set up recurring payments using the order context
- [Get](./get.md) - Check order status
- [MerchantAuthenticationService](../merchant-authentication-service/README.md) - Create session tokens for SDK integration
