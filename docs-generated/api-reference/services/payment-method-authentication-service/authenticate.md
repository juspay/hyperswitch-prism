# Authenticate RPC

<!--
---
title: Authenticate
description: Execute 3DS challenge or frictionless verification to authenticate customer via bank
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Authenticate` RPC executes the 3D Secure authentication step. For frictionless flows, it completes the authentication silently. For challenge flows, it presents the bank's authentication page to the customer for verification.

**Business Use Case:** After initiating 3DS with PreAuthenticate, this RPC handles the actual authentication. If a challenge is required, it manages the customer interaction with the bank's authentication page.

## Purpose

**Why authenticate?**

| Flow Type | What Happens |
|-----------|--------------|
| **Frictionless** | Completes authentication without customer action |
| **Challenge** | Presents bank challenge page for customer verification |

**Key outcomes:**
- Authentication completed
- Authentication data returned
- Ready for payment authorization

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | string | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `payment_method` | PaymentMethod | Yes | Card details |
| `customer` | Customer | No | Customer information |
| `address` | PaymentAddress | Yes | Billing address |
| `authentication_data` | AuthenticationData | No | Existing 3DS data from PreAuthenticate |
| `metadata` | SecretString | No | Additional metadata |
| `connector_feature_data` | SecretString | No | Connector-specific metadata |
| `return_url` | string | No | URL to redirect after authentication |
| `continue_redirection_url` | string | No | URL to continue after redirect |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's authentication transaction ID |
| `status` | PaymentStatus | Current status: AUTHENTICATED, FAILED, PENDING |
| `error` | ErrorInfo | Error details if authentication failed |
| `status_code` | uint32 | HTTP-style status code |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `redirection_data` | RedirectForm | Challenge URL if additional step needed |
| `network_transaction_id` | string | Card network transaction reference |
| `merchant_order_id` | string | Your order reference (echoed back) |
| `authentication_data` | AuthenticationData | 3DS authentication results |
| `state` | ConnectorState | State to pass to next step |
| `raw_connector_response` | SecretString | Raw response for debugging |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_order_id": "order_001",
    "amount": {
      "minor_amount": 10000,
      "currency": "USD"
    },
    "payment_method": {
      "card": {
        "card_number": "4242424242424242",
        "expiry_month": "12",
        "expiry_year": "2027"
      }
    },
    "address": {
      "billing_address": {
        "line_1": "123 Main St",
        "city": "San Francisco",
        "country": "US"
      }
    },
    "return_url": "https://your-app.com/3ds/return"
  }' \
  localhost:8080 \
  types.PaymentMethodAuthenticationService/Authenticate
```

### Response

```json
{
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "AUTHENTICATED",
  "authentication_data": {
    "eci": "05",
    "cavv": "AAABBIIFmAAAAAAAAAAAAAAAAAA=",
    "trans_status": "Y"
  },
  "status_code": 200
}
```

## Next Steps

- [PostAuthenticate](./post-authenticate.md) - Validate authentication results
- [Payment Service](../payment-service/README.md) - Process payment with auth data
