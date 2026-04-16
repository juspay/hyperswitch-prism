# CreateSdkSessionToken RPC

<!--
---
title: CreateSdkSessionToken
description: Initialize wallet payment sessions for Apple Pay, Google Pay, and other SDK-based payments
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `CreateSdkSessionToken` RPC initializes wallet payment sessions for Apple Pay, Google Pay, and other SDK-based payment methods. It sets up the secure context needed for tokenized wallet payments with device verification.

**Business Use Case:** When offering Apple Pay or Google Pay checkout options, you need to initialize a session with the payment processor that includes your merchant configuration, payment details, and supported payment methods. This RPC handles that initialization and returns the session data needed to present the native payment sheet.

## Purpose

**Why use SDK session tokens?**

| Wallet | Purpose |
|--------|---------|
| **Apple Pay** | Initialize PKPaymentSession with merchant validation |
| **Google Pay** | Configure PaymentDataRequest with merchant info |
| **PayPal SDK** | Set up checkout context |

**Key outcomes:**
- Wallet-specific session configuration
- Merchant validation data
- Supported payment methods list
- Ready for native SDK presentation

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_sdk_session_id` | string | Yes | Your unique SDK session reference |
| `amount` | Money | Yes | Payment amount |
| `order_tax_amount` | int64 | No | Tax amount in minor units |
| `shipping_cost` | int64 | No | Shipping cost in minor units |
| `payment_method_type` | PaymentMethodType | No | Target wallet type (APPLE_PAY, GOOGLE_PAY) |
| `country_alpha2_code` | CountryAlpha2 | No | ISO country code for localization |
| `customer` | Customer | No | Customer information |
| `metadata` | SecretString | No | Additional metadata |
| `connector_feature_data` | SecretString | No | Connector-specific metadata |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `session_token` | SessionToken | Wallet-specific session data |
| `error` | ErrorInfo | Error details if creation failed |
| `status_code` | uint32 | HTTP-style status code |
| `raw_connector_response` | SecretString | Raw response for debugging |
| `raw_connector_request` | SecretString | Raw request for debugging |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_sdk_session_id": "sdk_session_001",
    "amount": {
      "minor_amount": 10000,
      "currency": "USD"
    },
    "payment_method_type": "APPLE_PAY",
    "country_alpha2_code": "US",
    "customer": {
      "name": "John Doe",
      "email": "john@example.com"
    }
  }' \
  localhost:8080 \
  types.MerchantAuthenticationService/CreateSdkSessionToken
```

### Response

```json
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

- [CreateAccessToken](./create-access-token.md) - Generate API access tokens
- [CreateSessionToken](./create-session-token.md) - Create standard session tokens
- [Payment Service](../payment-service/README.md) - Process wallet payments
