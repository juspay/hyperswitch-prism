# Payment Method Service

<!--
---
title: Payment Method Service (Java SDK)
description: Tokenize and retrieve payment methods using the Java SDK
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

The Payment Method Service enables you to securely store payment methods at payment processors using the Java SDK. Tokenization replaces sensitive card data with secure tokens, enabling one-click payments and recurring billing without PCI compliance exposure.

**Business Use Cases:**
- **One-click checkout** - Returning customers pay without re-entering card details
- **Subscription billing** - Stored payment methods for recurring charges
- **Vault migration** - Move existing tokens between processors
- **PCI compliance** - Reduce compliance scope by avoiding raw card storage

## Operations

| Operation | Description | Use When |
|-----------|-------------|----------|
| [`tokenize`](./tokenize.md) | Store payment method for future use. Replaces raw card details with secure token. | Customer wants to save card for future purchases |

## SDK Setup

```java
import com.hyperswitch.prism.PaymentMethodClient;

PaymentMethodClient paymentMethodClient = PaymentMethodClient.builder()
    .connector("stripe")
    .apiKey("YOUR_API_KEY")
    .environment("SANDBOX")
    .build();
```

## Next Steps

- [Payment Service](../payment-service/README.md) - Use tokenized payment methods for charges
- [Customer Service](../customer-service/README.md) - Associate payment methods with customers
- [Recurring Payment Service](../recurring-payment-service/README.md) - Set up recurring billing with stored methods
