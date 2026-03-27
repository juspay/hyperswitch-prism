# Java SDK

<!--
---
title: Java SDK
description: Java SDK for the Hyperswitch Prism payment orchestration platform
last_updated: 2026-03-21
sdk_language: java
---
-->

## 🎯 What is Prism?

Today, integrating multiple payment processors either makes developers running in circles with AI agents to recreate integrations from specs, or developers spending months of engineering effort. 

Because every payment processor has diverse APIs, error codes, authentication methods, pdf documents to read, and above all - different behaviour in the actual environment when compared to documented specs. All this rests as tribal or undocumented knowledge making it harder AI agents which are very good at implementing clearly documented specification.

**Prism is a stateless, unified connector library for AI agents and Developers to connect with any payment processor**

**Prism offers hardened transformation through testing on payment processor environment & iterative bug fixing**

**Prism can be embedded in you server application with its wide range of multi-language SDKs, or run as a rRPC microservice**


| ❌ Without Prism | ✅ With Prism |
|------------------------------|----------------------------|
| 🗂️ 100+ different API schemas | 📋 Single unified schema |
| ⏳ Never ending agent loops/ months of integration work | ⚡ Hours to integrate, Agent driven |
| 🔗 Brittle, provider-specific code | 🔓 Portable, provider-agnostic code |
| 🚫 Hard to switch providers | 🔄 Change providers in 1 line |

## Installation

### Maven

```xml
<dependency>
    <groupId>io.hyperswitch</groupId>
    <artifactId>prism</artifactId>
    <version>1.0.0</version>
</dependency>
```

### Gradle

```groovy
implementation 'io.hyperswitch:prism:1.0.0'
```

## Quick Start

```java
import com.hyperswitch.prism.PaymentClient;
import java.util.Map;
import java.util.HashMap;

PaymentClient paymentClient = PaymentClient.builder()
    .connector("stripe")
    .apiKey("YOUR_API_KEY")
    .environment("SANDBOX")
    .build();

// Build amount
Map<String, Object> amount = new HashMap<>();
amount.put("minorAmount", 1000);
amount.put("currency", "USD");

// Build card details
Map<String, Object> cardDetails = new HashMap<>();
cardDetails.put("cardNumber", Map.of("value", "4242424242424242"));
cardDetails.put("cardExpMonth", Map.of("value", "12"));
cardDetails.put("cardExpYear", Map.of("value", "2027"));
cardDetails.put("cardCvc", Map.of("value", "123"));
cardDetails.put("cardHolderName", Map.of("value", "John Doe"));

// Build payment method
Map<String, Object> paymentMethod = new HashMap<>();
paymentMethod.put("card", cardDetails);

// Build request
Map<String, Object> request = new HashMap<>();
request.put("merchantTransactionId", "txn_order_001");
request.put("amount", amount);
request.put("paymentMethod", paymentMethod);
request.put("authType", "NO_THREE_DS");
request.put("captureMethod", "MANUAL");

// Authorize payment
Map<String, Object> response = paymentClient.authorize(request);
System.out.println(response.get("status")); // AUTHORIZED
```

## Services

| Service | Description |
|---------|-------------|
| [Payment Service](./payment-service/README.md) | Process payments from authorization to settlement |
| [Recurring Payment Service](./recurring-payment-service/README.md) | Manage subscriptions and recurring billing |
| [Refund Service](./refund-service/README.md) | Retrieve and track refund statuses |
| [Dispute Service](./dispute-service/README.md) | Handle chargebacks and disputes |
| [Event Service](./event-service/README.md) | Process webhook notifications |
| [Payment Method Service](./payment-method-service/README.md) | Store and manage payment methods |
| [Customer Service](./customer-service/README.md) | Manage customer profiles |
| [Merchant Authentication Service](./merchant-authentication-service/README.md) | Generate access tokens |
| [Payment Method Authentication Service](./payment-method-authentication-service/README.md) | 3D Secure authentication |
| [Payout Service](./payout-service/README.md) | Send funds to recipients |

## Configuration

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `connector` | String | Yes | Payment connector name (stripe, adyen, etc.) |
| `apiKey` | String | Yes | Your API key |
| `environment` | String | Yes | SANDBOX or PRODUCTION |
| `timeout` | Duration | No | Request timeout (default: 30s) |

## Error Handling

```java
try {
    Map<String, Object> response = paymentClient.authorize(request);
} catch (PaymentDeclinedException e) {
    // Handle declined payment
    System.err.println("Payment declined: " + e.getMessage());
} catch (ValidationException e) {
    // Handle validation error
    System.err.println("Validation error: " + e.getErrors());
} catch (HyperswitchException e) {
    // Handle other errors
    System.err.println("Error: " + e.getMessage());
}
```

## Support

For support and documentation, visit [https://docs.hyperswitch.io](https://docs.hyperswitch.io)
