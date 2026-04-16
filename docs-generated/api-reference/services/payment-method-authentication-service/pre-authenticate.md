# PreAuthenticate RPC

<!--
---
title: PreAuthenticate
description: Initiate 3DS flow before payment authorization to collect device data and prepare authentication context
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `PreAuthenticate` RPC initiates the 3D Secure authentication flow. It collects device data and prepares the authentication context, determining whether the transaction qualifies for frictionless authentication or requires a customer challenge.

**Business Use Case:** Before processing a card payment, you need to check if 3DS authentication is required. This RPC communicates with the issuing bank to initiate the authentication session and determine the authentication path (frictionless or challenge).

## Purpose

**Why pre-authenticate?**

| Scenario | Outcome |
|----------|---------|
| **Low risk transaction** | Bank approves frictionlessly (no customer action) |
| **High risk transaction** | Bank requires challenge (OTP, password) |
| **SCA compliance** | Meet EU Strong Customer Authentication requirements |
| **Liability shift** | Enable fraud liability protection |

**Key outcomes:**
- Authentication session initialized
- Risk assessment performed
- Challenge URL (if required)
- Authentication data for payment

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | string | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `payment_method` | PaymentMethod | Yes | Card details for authentication |
| `customer` | Customer | No | Customer information |
| `address` | PaymentAddress | Yes | Billing address |
| `enrolled_for_3ds` | bool | Yes | Whether 3DS enrollment check passed |
| `metadata` | SecretString | No | Additional metadata |
| `connector_feature_data` | SecretString | No | Connector-specific metadata |
| `return_url` | string | No | URL to redirect after authentication |
| `continue_redirection_url` | string | No | URL to continue after redirect |
| `browser_info` | BrowserInformation | No | Browser details for fraud detection |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's authentication transaction ID |
| `status` | PaymentStatus | Current status: PENDING, AUTHENTICATED, FAILED |
| `error` | ErrorInfo | Error details if authentication failed |
| `status_code` | uint32 | HTTP-style status code |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `redirection_data` | RedirectForm | Challenge URL/form (if challenge required) |
| `network_transaction_id` | string | Card network transaction reference |
| `merchant_order_id` | string | Your order reference (echoed back) |
| `state` | ConnectorState | State to pass to next authentication step |
| `raw_connector_response` | SecretString | Raw response for debugging |
| `authentication_data` | AuthenticationData | 3DS authentication results |

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
        "expiry_year": "2027",
        "cvc": "123"
      }
    },
    "address": {
      "billing_address": {
        "line_1": "123 Main St",
        "city": "San Francisco",
        "state": "CA",
        "zip_code": "94105",
        "country": "US"
      }
    },
    "enrolled_for_3ds": true,
    "return_url": "https://your-app.com/3ds/return",
    "browser_info": {
      "accept_header": "text/html",
      "user_agent": "Mozilla/5.0..."
    }
  }' \
  localhost:8080 \
  types.PaymentMethodAuthenticationService/PreAuthenticate
```

### Response (Frictionless)

```json
{
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "AUTHENTICATED",
  "authentication_data": {
    "eci": "05",
    "cavv": "AAABBIIFmAAAAAAAAAAAAAAAAAA="
  },
  "status_code": 200
}
```

### Response (Challenge Required)

```json
{
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "PENDING",
  "redirection_data": {
    "form": {
      "form_method": "POST",
      "form_url": "https://bank.com/3ds/challenge",
      "form_fields": {
        "creq": "..."
      }
    }
  },
  "state": {
    "connector_state": "..."
  },
  "status_code": 200
}
```

## Next Steps

- [Authenticate](./authenticate.md) - Execute challenge if required
- [PostAuthenticate](./post-authenticate.md) - Validate authentication results
