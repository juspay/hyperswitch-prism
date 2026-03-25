# PostAuthenticate RPC

<!--
---
title: PostAuthenticate
description: Validate authentication results with the issuing bank and confirm payment can proceed
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `PostAuthenticate` RPC validates 3D Secure authentication results with the issuing bank. After the customer completes a challenge (if required), this RPC confirms the authentication was successful and returns the data needed to proceed with payment authorization.

**Business Use Case:** After the customer returns from a 3DS challenge or after a frictionless authentication, you need to validate the results before processing the payment. This RPC confirms the authentication and provides the ECI and CAVV values required for liability shift.

## Purpose

**Why post-authenticate?**

| Scenario | Action |
|----------|--------|
| **After challenge completion** | Validate the authentication response |
| **Before payment** | Confirm authentication succeeded |
| **For liability shift** | Obtain ECI/CAVV values |

**Key outcomes:**
- Authentication validated
- ECI and CAVV values returned
- Ready for payment authorization

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_order_id` | string | Yes | Your unique order reference |
| `amount` | Money | Yes | Transaction amount |
| `payment_method` | PaymentMethod | Yes | Card details |
| `customer` | Customer | No | Customer information |
| `address` | PaymentAddress | Yes | Billing address |
| `authentication_data` | AuthenticationData | No | 3DS result data from challenge |
| `connector_order_reference_id` | string | No | Connector's order reference |
| `metadata` | SecretString | No | Additional metadata |
| `connector_feature_data` | SecretString | No | Connector-specific metadata |
| `return_url` | string | No | URL to redirect after authentication |
| `continue_redirection_url` | string | No | URL to continue after redirect |
| `browser_info` | BrowserInformation | No | Browser details |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's authentication transaction ID |
| `status` | PaymentStatus | Current status: AUTHENTICATED, FAILED |
| `error` | ErrorInfo | Error details if validation failed |
| `status_code` | uint32 | HTTP-style status code |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `redirection_data` | RedirectForm | Additional redirect if needed |
| `network_transaction_id` | string | Card network transaction reference |
| `merchant_order_id` | string | Your order reference (echoed back) |
| `authentication_data` | AuthenticationData | Validated 3DS authentication data |
| `incremental_authorization_allowed` | bool | Whether incremental auth is allowed |
| `state` | ConnectorState | State for payment authorization |
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
    "authentication_data": {
      "eci": "05",
      "cavv": "AAABBIIFmAAAAAAAAAAAAAAAAAA="
    }
  }' \
  localhost:8080 \
  types.PaymentMethodAuthenticationService/PostAuthenticate
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
  "incremental_authorization_allowed": false,
  "status_code": 200
}
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Authorize payment with 3DS data
