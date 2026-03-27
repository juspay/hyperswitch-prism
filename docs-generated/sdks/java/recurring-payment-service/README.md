# Recurring Payment Service

<!--
---
title: Recurring Payment Service (Java SDK)
description: Process subscription billing and manage recurring payment mandates using the Java SDK
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: java
---
-->

## Overview

The Recurring Payment Service enables you to process subscription billing and manage recurring payment mandates using the Java SDK. Once a customer has set up a mandate, this service handles subsequent charges without requiring customer interaction.

**Business Use Cases:**
- **SaaS subscriptions** - Charge customers monthly/yearly for software subscriptions
- **Membership fees** - Process recurring membership dues for clubs and organizations
- **Utility billing** - Automate monthly utility and service bill payments
- **Installment payments** - Collect scheduled payments for large purchases over time

## Operations

| Operation | Description | Use When |
|-----------|-------------|----------|
| [`charge`](./charge.md) | Process a recurring payment using an existing mandate. Charges customer's stored payment method for subscription renewal. | Subscription renewal, recurring billing cycle |
| [`revoke`](./revoke.md) | Cancel an existing recurring payment mandate. Stops future automatic charges. | Subscription cancellation, customer churn |

## SDK Setup

```java
import com.hyperswitch.prism.RecurringPaymentClient;

RecurringPaymentClient recurringClient = RecurringPaymentClient.builder()
    .connector("stripe")
    .apiKey("YOUR_API_KEY")
    .environment("SANDBOX")
    .build();
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Set up initial mandates with setupRecurring
- [Payment Method Service](../payment-method-service/README.md) - Store payment methods for recurring use
- [Customer Service](../customer-service/README.md) - Manage customer profiles for subscriptions
