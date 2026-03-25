# SetupRecurring RPC

<!--
---
title: SetupRecurring
description: Setup a recurring payment instruction for future payments/debits - for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
---
-->

## Overview

The `SetupRecurring` RPC establishes a payment mandate (recurring payment instruction) at the payment processor. This enables future charges without requiring the customer to re-enter payment details or be present for each transaction. It's the foundation of subscription billing, recurring donations, and automated bill payment systems.

**Business Use Case:** When setting up a SaaS subscription, monthly utility bill, insurance premium, or any service that bills customers on a recurring basis. The mandate represents the customer's consent for future charges and is stored securely at the processor, reducing PCI compliance scope and improving authorization rates.

## Purpose

**Why use SetupRecurring?**

| Scenario | Developer Implementation |
|----------|-------------------------|
| **SaaS subscriptions** | Customer signs up for monthly plan - call `SetupRecurring` to create mandate, then use mandate for recurring billing |
| **Utility bills** | Customer enrolls in auto-pay - call `SetupRecurring` to enable automatic monthly charges |
| **Membership renewals** | Gym or club membership - call `SetupRecurring` to set up annual renewal charges |
| **Installment payments** | Buy-now-pay-later setup - call `SetupRecurring` to create scheduled payment plan |
| **Donation programs** | Monthly charity donations - call `SetupRecurring` to enable recurring contributions |

**Key outcomes:**
- Mandate reference created for future charges
- Customer consent stored at processor level
- No PCI exposure for stored payment methods
- Higher authorization rates for repeat charges
- Compliance with SEPA and other mandate regulations

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_recurring_payment_id` | string | Yes | Your unique identifier for this recurring setup |
| `amount` | Money | Yes | Initial amount (for validation) |
| `payment_method` | PaymentMethod | Yes | Payment method to be used for recurring charges |
| `customer` | Customer | No | Customer information |
| `address` | PaymentAddress | Yes | Billing and shipping address |
| `auth_type` | AuthenticationType | Yes | Type of authentication to be used (e.g., THREE_DS, NO_THREE_DS) |
| `enrolled_for_3ds` | bool | Yes | Indicates if the customer is enrolled for 3D Secure |
| `authentication_data` | AuthenticationData | No | Additional authentication data |
| `metadata` | SecretString | No | Additional metadata for the connector |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `return_url` | string | No | URL to redirect after setup |
| `webhook_url` | string | No | URL for webhook notifications |
| `complete_authorize_url` | string | No | URL to complete authorization |
| `session_token` | string | No | Session token, if applicable |
| `order_tax_amount` | int64 | No | Tax amount, if an initial payment is part of setup |
| `order_category` | string | No | Category of the order/service related to the mandate |
| `merchant_order_id` | string | No | Merchant's internal reference ID |
| `shipping_cost` | int64 | No | Shipping cost, if an initial payment is part of setup |
| `setup_future_usage` | FutureUsage | No | Indicates future usage intention. Values: ON_SESSION, OFF_SESSION |
| `off_session` | bool | No | Indicates if off-session process |
| `request_incremental_authorization` | bool | Yes | Indicates if incremental authorization is requested |
| `request_extended_authorization` | bool | No | Indicates if extended authorization is requested |
| `enable_partial_authorization` | bool | No | Indicates if partial authorization is enabled |
| `customer_acceptance` | CustomerAcceptance | No | Details of customer acceptance for mandate |
| `browser_info` | BrowserInformation | No | Information about the customer's browser |
| `payment_experience` | PaymentExperience | No | Preferred payment experience |
| `payment_channel` | PaymentChannel | No | Describes the channel through which the payment was initiated |
| `billing_descriptor` | BillingDescriptor | No | Billing Descriptor information to be sent to the payment gateway |
| `state` | ConnectorState | No | State data for access token storage and other connector-specific state |
| `payment_method_token` | SecretString | No | Token for previously saved payment method |
| `order_id` | string | No | Connector order identifier if an order was created before authorize |
| `locale` | string | No | Locale/language preference for the shopper (e.g., "en-US") |
| `connector_testing_data` | SecretString | No | Connector-specific testing data (JSON stringified) |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `connector_recurring_payment_id` | string | Identifier for the mandate registration |
| `status` | PaymentStatus | Status of the mandate setup attempt |
| `error` | ErrorInfo | Error details if setup failed |
| `status_code` | uint32 | HTTP status code from the connector |
| `response_headers` | map<string, string> | Optional HTTP response headers from the connector |
| `mandate_reference` | MandateReference | Reference to the created mandate for future charges |
| `redirection_data` | RedirectForm | Data for redirecting the customer's browser (if additional auth required) |
| `network_transaction_id` | string | Card network transaction reference |
| `merchant_recurring_payment_id` | string | Your recurring payment reference (echoed back) |
| `connector_response` | ConnectorResponseData | Various data regarding the response from connector |
| `incremental_authorization_allowed` | bool | Indicates if incremental authorization is allowed |
| `captured_amount` | int64 | Captured amount in minor currency units if initial charge included |
| `state` | ConnectorState | State data for access token storage and other connector-specific state |
| `raw_connector_request` | SecretString | Raw request to the connector for debugging |
| `connector_feature_data` | SecretString | Connector-specific metadata for the transaction |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_recurring_payment_id": "recurring_001",
    "amount": {
      "minor_amount": 2900,
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
    "address": {
      "billing": {
        "line1": "123 Main St",
        "city": "San Francisco",
        "state": "CA",
        "zip": "94102",
        "country": "US"
      }
    },
    "auth_type": "NO_THREE_DS",
    "enrolled_for_3ds": false,
    "request_incremental_authorization": false,
    "setup_future_usage": "OFF_SESSION"
  }' \
  localhost:8080 \
  types.PaymentService/SetupRecurring
```

### Response

```json
{
  "connector_recurring_payment_id": "seti_3Oxxx...",
  "status": "AUTHORIZED",
  "status_code": 200,
  "mandate_reference": {
    "mandate_id": "pm_3Oxxx...",
    "mandate_status": "ACTIVE"
  },
  "merchant_recurring_payment_id": "recurring_001",
  "incremental_authorization_allowed": false
}
```

## Next Steps

- [RecurringPaymentService.Charge](../recurring-payment-service/README.md) - Use the mandate for future recurring charges
- [Authorize](./authorize.md) - Create initial charge with mandate
- [CustomerService.Create](../customer-service/README.md) - Create customer for associated recurring payments
- [Capture](./capture.md) - Capture initial payment if part of setup
