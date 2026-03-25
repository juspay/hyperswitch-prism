# Get RPC

<!--
---
title: Get
description: Retrieve current payment status from the payment processor - synchronize payment state between systems
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/payment.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Get` RPC retrieves the current payment status from the payment processor. This enables synchronization between your system and payment processors for accurate state tracking, especially important for handling asynchronous webhook delays or recovering from system outages.

**Business Use Case:** When a customer refreshes their order page, or your system needs to verify a payment's current state before proceeding with fulfillment. Payment statuses can change asynchronously through webhooks, and `Get` ensures you have the most up-to-date information directly from the source.

## Purpose

**Why use Get instead of relying solely on webhooks?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Order status page** | Call `Get` when customer views order, display current status from `status` field |
| **Webhook recovery** | If webhook missed, call `Get` with `connector_transaction_id` to sync state |
| **Pre-fulfillment check** | Before shipping, call `Get` to confirm payment is CAPTURED, not just AUTHORIZED |
| **Multi-system sync** | Call `Get` periodically to reconcile payment state across microservices |
| **Dispute handling** | Call `Get` to verify payment details when responding to chargebacks |

**Key outcomes:**
- Accurate payment state for customer-facing displays
- Recovery from missed or delayed webhooks
- Confirmation before critical business actions (shipping, digital delivery)
- Audit trail verification for support inquiries

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `connector_transaction_id` | string | Yes | The connector's transaction ID from the original authorization |
| `encoded_data` | string | No | Encoded data for retrieving payment status |
| `capture_method` | CaptureMethod | No | Method for capturing. Values: MANUAL, AUTOMATIC |
| `handle_response` | bytes | No | Raw response bytes from connector for state reconstruction |
| `amount` | Money | No | Amount information for verification |
| `setup_future_usage` | FutureUsage | No | Future usage intent. Values: ON_SESSION, OFF_SESSION |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `sync_type` | SyncRequestType | No | Type of synchronization request |
| `connector_order_reference_id` | string | No | Connector's order reference ID |
| `test_mode` | bool | No | Process as test transaction |
| `payment_experience` | PaymentExperience | No | Desired payment experience. Values: REDIRECT, EMBEDDED |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's transaction ID |
| `status` | PaymentStatus | Current status. Values: STARTED, AUTHORIZED, CAPTURED, FAILED, VOIDED, CHARGED |
| `error` | ErrorInfo | Error details if status is FAILED |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string, string> | Connector-specific response headers |
| `mandate_reference` | MandateReference | Mandate details if recurring payment |
| `network_transaction_id` | string | Card network transaction reference |
| `amount` | Money | Original authorization amount |
| `captured_amount` | int64 | Total captured amount in minor currency units |
| `payment_method_type` | PaymentMethodType | Type of payment method used |
| `capture_method` | CaptureMethod | How payment will be captured. Values: MANUAL, AUTOMATIC |
| `auth_type` | AuthenticationType | Authentication type used. Values: NO_THREE_DS, THREE_DS |
| `created_at` | int64 | Unix timestamp when payment was created |
| `updated_at` | int64 | Unix timestamp of last update |
| `authorized_at` | int64 | Unix timestamp when payment was authorized |
| `captured_at` | int64 | Unix timestamp when payment was captured |
| `customer_name` | string | Customer name associated with payment |
| `email` | string | Customer email address |
| `connector_customer_id` | string | Customer ID from the connector |
| `merchant_order_id` | string | Your internal order ID |
| `metadata` | SecretString | Additional metadata from the connector |
| `connector_response` | ConnectorResponseData | Raw connector response data |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `raw_connector_response` | SecretString | Raw API response from connector (debugging) |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |
| `redirection_data` | RedirectForm | Redirect URL/form for 3DS or bank authentication |
| `incremental_authorization_allowed` | bool | Whether amount can be increased later |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "connector_transaction_id": "pi_3Oxxx...",
    "test_mode": true
  }' \
  localhost:8080 \
  types.PaymentService/Get
```

### Response

```json
{
  "connector_transaction_id": "pi_3Oxxx...",
  "status": "AUTHORIZED",
  "status_code": 200,
  "amount": {
    "minor_amount": 1000,
    "currency": "USD"
  },
  "captured_amount": 0,
  "capture_method": "MANUAL"
}
```

## Next Steps

- [Authorize](./authorize.md) - Authorize a new payment
- [Capture](./capture.md) - Finalize the payment and transfer funds
- [Void](./void.md) - Release held funds if order cancelled
