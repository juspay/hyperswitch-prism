# Authorize RPC

<!--
---
title: Authorize
description: Authorize a payment amount on a payment method - reserves funds without capturing
last_updated: 2026-03-03
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-03
approved: true
---
-->

## Overview

The `Authorize` RPC reserves funds on a customer's payment method without transferring them. This is the first step in a two-step payment flow (authorize + capture), commonly used in e-commerce, marketplaces, and subscription businesses.

**Business Use Case:** When a customer places an order, you want to verify their payment method has sufficient funds and lock those funds for fulfillment. The actual charge (capture) happens later when the order ships or service is delivered. This reduces chargebacks and improves cash flow management.

## Purpose

**Why use authorization instead of immediate charge?**

| Scenario | Benefit |
|----------|---------|
| **E-commerce fulfillment** | Verify funds at checkout, capture when order ships |
| **Hotel reservations** | Pre-authorize for incidentals, capture final amount at checkout |
| **Marketplace holds** | Secure funds from buyer before releasing to seller |
| **Subscription trials** | Validate card at signup, first charge after trial ends |

**Key outcomes:**
- Guaranteed funds availability (typically 7-10 days hold)
- Reduced fraud exposure through pre-verification
- Better customer experience (no double charges for partial shipments)
- Compliance with card network rules for delayed delivery

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_transaction_id` | string | Yes | Your unique transaction reference |
| `amount` | Money | Yes | The amount for the payment in minor units (e.g., 1000 = $10.00) |
| `order_tax_amount` | int64 | No | Tax amount for the order in minor units |
| `shipping_cost` | int64 | No | Cost of shipping for the order in minor units |
| `payment_method` | PaymentMethod | Yes | Payment method to be used (card, wallet, bank) |
| `capture_method` | CaptureMethod | No | Method for capturing. Values: MANUAL, AUTOMATIC |
| `customer` | Customer | No | Customer information for fraud scoring |
| `address` | PaymentAddress | No | Billing and shipping address |
| `auth_type` | AuthenticationType | Yes | Authentication flow type (e.g., THREE_DS, NO_THREE_DS) |
| `enrolled_for_3ds` | bool | No | Whether 3DS enrollment check passed |
| `authentication_data` | AuthenticationData | No | 3DS authentication results |
| `metadata` | SecretString | No | Additional metadata for the connector (max 20 keys) |
| `connector_feature_data` | SecretString | No | Connector-specific feature data for the transaction |
| `return_url` | string | No | URL to redirect customer after 3DS/redirect flow |
| `webhook_url` | string | No | URL for async webhook notifications |
| `complete_authorize_url` | string | No | URL to complete authorization after redirect |
| `session_token` | string | No | Session token for wallet payments (Apple Pay, Google Pay) |
| `order_category` | string | No | Category of goods/services being purchased |
| `merchant_order_id` | string | No | Your internal order ID |
| `setup_future_usage` | FutureUsage | No | ON_SESSION or OFF_SESSION for tokenization |
| `off_session` | bool | No | Whether customer is present (false = customer present) |
| `request_incremental_authorization` | bool | No | Allow increasing authorized amount later |
| `request_extended_authorization` | bool | No | Request extended hold period |
| `enable_partial_authorization` | bool | No | Allow partial approval (e.g., $80 of $100) |
| `customer_acceptance` | CustomerAcceptance | No | Customer consent for recurring payments |
| `browser_info` | BrowserInformation | No | Browser details for 3DS fingerprinting |
| `payment_experience` | PaymentExperience | No | Desired UX (e.g., REDIRECT, EMBEDDED) |
| `description` | string | No | Payment description shown on statements |
| `payment_channel` | PaymentChannel | No | E-commerce, MOTO, or recurring indicator |
| `test_mode` | bool | No | Process as test transaction |
| `setup_mandate_details` | SetupMandateDetails | No | Mandate setup for recurring SEPA/bank debits |
| `statement_descriptor_name` | string | No | Your business name on customer statement |
| `statement_descriptor_suffix` | string | No | Additional descriptor suffix |
| `billing_descriptor` | BillingDescriptor | No | Complete billing descriptor configuration |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `order_details` | OrderDetailsWithAmount[] | No | Line item details with amounts |
| `locale` | string | No | Locale for localized responses (e.g., "en-US") |
| `tokenization_strategy` | Tokenization | No | Tokenization configuration |
| `threeds_completion_indicator` | ThreeDsCompletionIndicator | No | 3DS method completion status |
| `redirection_response` | RedirectionResponse | No | Response data from redirect-based auth |
| `continue_redirection_url` | string | No | URL to continue after redirect |
| `payment_method_token` | SecretString | No | Token for previously saved payment method |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_transaction_id` | string | Your transaction reference (echoed back) |
| `connector_transaction_id` | string | Connector's transaction ID (e.g., Stripe pi_xxx) |
| `status` | PaymentStatus | Current status: AUTHORIZED, PENDING, FAILED, etc. |
| `error` | ErrorInfo | Error details if status is FAILED |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `redirection_data` | RedirectForm | Redirect URL/form for 3DS or bank authentication |
| `network_transaction_id` | string | Card network transaction reference |
| `incremental_authorization_allowed` | bool | Whether amount can be increased later |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `raw_connector_response` | SecretString | Raw API response from connector (debugging) |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |
| `captured_amount` | int64 | Amount already captured (0 for fresh auth) |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_transaction_id": "txn_order_001",
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
    "auth_type": "NO_THREE_DS",
    "capture_method": "MANUAL",
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentService/Authorize
```

### Response

```json
{
  "merchant_transaction_id": "txn_order_001",
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "AUTHORIZED",
  "status_code": 200,
  "incremental_authorization_allowed": false
}
```

## Next Steps

- [Capture](./capture.md) - Finalize the payment and transfer funds
- [Void](./void.md) - Release held funds if order cancelled
- [Get](./get.md) - Check current authorization status
