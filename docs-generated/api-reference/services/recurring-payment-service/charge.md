# Charge RPC

<!--
---
title: Charge
description: Process a recurring payment using an existing mandate - charge customer's stored payment method for subscription renewal without requiring their presence
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Charge` RPC processes a recurring payment using a previously established mandate. This enables subscription billing and automated payment collection without requiring customer interaction or re-entering payment details.

**Business Use Case:** When a subscription billing cycle triggers (e.g., monthly SaaS renewal), you need to charge the customer's stored payment method. The Charge RPC uses the mandate reference from the initial `SetupRecurring` to process the payment without customer presence, following stored credential protocols for secure recurring billing.

## Purpose

**Why use Charge for recurring payments?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **Monthly subscription renewal** | Call `Charge` with stored `mandate_reference` to process monthly SaaS billing |
| **Annual plan billing** | Call `Charge` yearly with the mandate for annual subscription renewals |
| **Installment collection** | Schedule `Charge` calls for each installment of a payment plan |
| **Membership dues** | Automate monthly/quarterly membership fee collection |
| **Prorated upgrades** | Call `Charge` immediately when customers upgrade plans mid-cycle |

**Key outcomes:**
- Payment processed using stored mandate (no customer interaction required)
- Compliance with card network stored credential requirements
- New transaction ID for each charge (distinct from original mandate)
- Support for MIT (Merchant Initiated Transaction) protocols

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_charge_id` | string | Yes | Your unique identifier for this charge transaction |
| `mandate_reference_id` | MandateReference | Yes | Reference to the existing mandate from SetupRecurring |
| `amount` | Money | Yes | Amount to charge (may differ from original mandate amount) |
| `payment_method` | PaymentMethod | No | Optional payment method for network transaction flows |
| `merchant_order_id` | string | No | Your internal order ID for this charge |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific feature data |
| `webhook_url` | string | No | URL for async webhook notifications |
| `return_url` | string | No | URL to redirect customer if authentication required |
| `description` | string | No | Description shown on customer's statement |
| `address` | PaymentAddress | No | Billing address for this charge |
| `capture_method` | CaptureMethod | No | Capture method (AUTOMATIC recommended for recurring) |
| `email` | SecretString | No | Customer email for notifications |
| `connector_customer_id` | string | No | Connector's customer ID for this charge |
| `browser_info` | BrowserInformation | No | Browser details if customer present |
| `test_mode` | bool | No | Process as test transaction |
| `payment_method_type` | PaymentMethodType | No | Payment method type indicator |
| `merchant_account_id` | SecretString | No | Merchant account ID for the charge |
| `merchant_configured_currency` | Currency | No | Currency for merchant account |
| `off_session` | bool | No | Set to true for off-session MIT charges |
| `enable_partial_authorization` | bool | No | Allow partial approval |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `original_payment_authorized_amount` | Money | No | Original amount for reference |
| `shipping_cost` | int64 | No | Shipping cost in minor units |
| `billing_descriptor` | BillingDescriptor | No | Statement descriptor configuration |
| `mit_category` | MitCategory | No | MIT category (RECURRING, UNSCHEDULED, etc.) |
| `authentication_data` | AuthenticationData | No | Authentication data for 3DS |
| `locale` | string | No | Locale for responses (e.g., "en-US") |
| `connector_testing_data` | SecretString | No | Testing data for connectors |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_transaction_id` | string | Connector's transaction ID for this charge |
| `status` | PaymentStatus | Current status: CHARGED, AUTHORIZED, PENDING, FAILED, etc. |
| `error` | ErrorInfo | Error details if charge failed |
| `status_code` | uint32 | HTTP-style status code (200, 402, etc.) |
| `response_headers` | map<string,string> | Connector-specific response headers |
| `connector_feature_data` | SecretString | Connector-specific metadata |
| `network_transaction_id` | string | Card network transaction reference |
| `merchant_charge_id` | string | Your charge reference (echoed back) |
| `mandate_reference` | MandateReference | Mandate reference used for this charge |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `raw_connector_response` | SecretString | Raw API response from connector (debugging) |
| `raw_connector_request` | SecretString | Raw API request sent to connector (debugging) |
| `captured_amount` | int64 | Amount captured (if different from authorized) |
| `connector_response` | ConnectorResponseData | Structured connector response data |
| `incremental_authorization_allowed` | bool | Whether amount can be increased later |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_charge_id": "charge_sub_001",
    "mandate_reference_id": {
      "connector_mandate_id": "seti_3Oxxx...",
      "payment_method_id": "pm_1Oxxx..."
    },
    "amount": {
      "minor_amount": 2900,
      "currency": "USD"
    },
    "description": "Monthly Subscription - Pro Plan",
    "off_session": true,
    "mit_category": "RECURRING",
    "test_mode": true
  }' \
  localhost:8080 \
  types.RecurringPaymentService/Charge
```

### Response (Success)

```json
{
  "connector_transaction_id": "pi_3Pxxx...",
  "status": "CHARGED",
  "status_code": 200,
  "merchant_charge_id": "charge_sub_001",
  "mandate_reference": {
    "connector_mandate_id": "seti_3Oxxx...",
    "payment_method_id": "pm_1Oxxx..."
  },
  "network_transaction_id": "txn_xxx...",
  "captured_amount": 2900
}
```

### Response (Failure - Insufficient Funds)

```json
{
  "connector_transaction_id": "pi_3Pxxx...",
  "status": "FAILED",
  "status_code": 402,
  "merchant_charge_id": "charge_sub_001",
  "error": {
    "code": "card_declined",
    "message": "Your card was declined.",
    "decline_code": "insufficient_funds"
  }
}
```

## Status Values

| Status | Description | Next Action |
|--------|-------------|-------------|
| `CHARGED` | Payment completed successfully | Confirm subscription renewal |
| `AUTHORIZED` | Funds reserved but not yet captured | Call Capture if using manual capture |
| `PENDING` | Charge is being processed | Wait for webhook or poll Get |
| `FAILED` | Charge could not be completed | Retry with backoff or notify customer |
| `REQUIRES_ACTION` | Additional authentication needed | Handle 3DS or redirect flow |

## MIT Categories

| Category | Use When |
|----------|----------|
| `RECURRING` | Regular subscription billing at fixed intervals |
| `UNSCHEDULED` | Variable amount or timing charges (usage-based) |
| `INSTALLMENT` | Fixed payment plan with defined schedule |
| `DELAYED` | Charge agreed to previously but delayed |
| `RESUBMISSION` | Retry of previously failed transaction |
| `NO_SHOW` | Charge for no-show fees |
| `REAUTHORIZATION` | Re-authorizing a previously authorized amount |

## Best Practices

1. **Set `off_session: true`** - Always indicate that the customer is not present for recurring charges
2. **Include `mit_category`** - Properly categorize the charge type for compliance
3. **Store `connector_transaction_id`** - Save this for refund and dispute handling
4. **Handle failures gracefully** - Implement retry logic with customer notification
5. **Use webhooks** - Set up webhooks for asynchronous status updates

## Next Steps

- [Revoke](./revoke.md) - Cancel the mandate when customer unsubscribes
- [SetupRecurring](../payment-service/setup-recurring.md) - Create initial mandate for new subscriptions
- [Get](../payment-service/get.md) - Check status of pending charges
