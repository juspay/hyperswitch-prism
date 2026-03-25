# Create RPC

<!--
---
title: Create
description: Create customer record in the payment processor system for streamlined future payments
created: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/services.proto
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

The `Create` RPC creates a customer record at the payment processor. This stores customer information (name, email, phone, address) at the connector level, enabling streamlined future transactions without resending personal details.

**Business Use Case:** When a new customer signs up or makes their first purchase, you create their profile at the payment processor. This customer ID can then be used across multiple payment operations, improving authorization rates through customer history and enabling features like saved payment methods and recurring billing.

## Purpose

**Why create customer profiles at the payment processor?**

| Scenario | Benefit |
|----------|---------|
| **Returning customers** | Faster checkout without re-entering personal information |
| **Recurring billing** | Link subscriptions to a consistent customer identity |
| **Multiple payment methods** | Organize stored cards/wallets under one customer |
| **Unified reporting** | Track all customer transactions in one view |

**Key outcomes:**
- Unique `connector_customer_id` for referencing this customer
- Consistent identity across all payment operations
- Foundation for payment method tokenization
- Improved authorization rates for repeat customers

## Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `merchant_customer_id` | string | Yes | Your unique customer reference |
| `customer_name` | string | No | Full name of the customer |
| `email` | SecretString | No | Email address of the customer |
| `phone_number` | string | No | Phone number of the customer |
| `address` | PaymentAddress | No | Billing and shipping address information |
| `metadata` | SecretString | No | Additional metadata for the connector (max 20 keys) |
| `connector_feature_data` | SecretString | No | Connector-specific metadata for the transaction |
| `test_mode` | bool | No | Process as test transaction |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchant_customer_id` | string | Your customer reference (echoed back) |
| `connector_customer_id` | string | Connector's customer ID (e.g., Stripe cus_xxx) |
| `error` | ErrorInfo | Error details if creation failed |
| `status_code` | uint32 | HTTP-style status code (200, 400, etc.) |
| `response_headers` | map<string,string> | Connector-specific response headers |

## Example

### Request (grpcurl)

```bash
grpcurl -H "x-connector: stripe" \
  -H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}" \
  -d '{
    "merchant_customer_id": "cust_user_12345",
    "customer_name": "John Doe",
    "email": "john.doe@example.com",
    "phone_number": "+1-555-123-4567",
    "address": {
      "billing_address": {
        "first_name": "John",
        "last_name": "Doe",
        "line_1": "123 Main Street",
        "city": "San Francisco",
        "state": "CA",
        "zip_code": "94105",
        "country": "US"
      }
    },
    "metadata": "{\"tier\": \"premium\"}",
    "test_mode": true
  }' \
  localhost:8080 \
  types.CustomerService/Create
```

### Response

```json
{
  "merchant_customer_id": "cust_user_12345",
  "connector_customer_id": "cus_9OhXXXXXXXXXXXX",
  "status_code": 200
}
```

## Next Steps

- [Authorize](../payment-service/authorize.md) - Create a payment linked to this customer
- [PaymentMethodService.Tokenize](../payment-method-service/tokenize.md) - Store payment methods for this customer
- [SetupRecurring](../payment-service/setup-recurring.md) - Set up recurring billing with customer reference
