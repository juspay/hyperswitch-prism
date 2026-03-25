# Tokenize RPC

<!--
---
title: Tokenize
description: Store payment method securely at the processor - replace raw card details with token for one-click payments and recurring billing
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Tokenize` RPC securely stores payment method details at the payment processor and returns a token that can be used for future payments. This enables one-click checkout experiences and recurring billing without your systems handling sensitive card data.

**Business Use Case:** When a customer wants to save their payment details for faster future purchases or subscription billing. Instead of storing card numbers in your database (which requires PCI compliance), you send the details to the payment processor who returns a secure token. You store only the token, and the processor handles the sensitive data.

## Purpose

**Why use Tokenize?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **One-click checkout** | Store token after first purchase, use it for future payments without customer re-entering card details |
| **Subscription billing** | Tokenize payment method during signup, use token for recurring charges |
| **Mobile wallet storage** | Convert Apple Pay/Google Pay tokens to processor tokens for repeated use |
| **Reduced PCI scope** | Tokenization shifts PCI compliance burden to the payment processor |
| **Improved authorization rates** | Tokenized payments often have higher approval rates due to stored credential protocols |

**Key outcomes:**
- Secure storage of payment credentials at the processor
- Token reference that can be used for future payments
- Reduced PCI compliance requirements for your systems
- Support for stored credential protocols (MIT/CIT transactions)
- Consistent customer experience across payment sessions

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_payment_method_id` | string | Yes | Your unique identifier for this payment method |
| `amount` | Money | Yes | Amount for initial validation (some processors require this) |
| `payment_method` | PaymentMethod | Yes | Payment method details to tokenize (card, wallet, etc.) |
| `customer` | Customer | No | Customer information to associate with payment method |
| `address` | PaymentAddress | No | Billing address for the payment method |
| `metadata` | SecretString | No | Additional metadata for the connector (max 20 keys) |
| `connector_feature_data` | SecretString | No | Connector-specific feature data for the tokenization |
| `return_url` | string | No | URL to redirect customer after any required authentication |
| `test_mode` | bool | No | Process as test transaction |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `payment_method_token` | string | Token to use for future payments (e.g., Stripe pm_xxx) |
| `error` | ErrorInfo | Error details if tokenization failed |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `merchant_payment_method_id` | string | Your payment method reference (echoed back) |
| `state` | ConnectorState | State to pass to next request in multi-step flow |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_payment_method_id": "pm_user_001",
    "amount": {
      "minor_amount": 1000,
      "currency": "USD"
    },
    "payment_method": {
      "card": {
        "card_number": "4242424242424242",
        "expiry_month": "12",
        "expiry_year": "2027",
        "card_holder_name": "John Doe",
        "cvc": "123"
      }
    },
    "customer": {
      "customer_id": "cust_001",
      "name": "John Doe",
      "email": "john@example.com"
    },
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentMethodService/Tokenize
```

### Response

```json
{
  "payment_method_token": "pm_1Oxxx...",
  "merchant_payment_method_id": "pm_user_001",
  "status_code": 200
}
```

## Next Steps

- [Authorize](../payment-service/authorize.md) - Use the tokenized payment method to authorize a payment
- [SetupRecurring](../payment-service/setup-recurring.md) - Set up recurring billing with the stored payment method
- [Customer Service](../customer-service/README.md) - Associate payment methods with customer profiles
