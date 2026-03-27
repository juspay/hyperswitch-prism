# authorize Method

<!--
---
title: authorize (Java SDK)
description: Authorize a payment using the Java SDK - reserve funds without capturing
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

The `authorize` method reserves funds on a customer's payment method without transferring them. This is the first step in a two-step payment flow (authorize + capture), commonly used in e-commerce, marketplaces, and subscription businesses.

**Business Use Case:** When a customer places an order, you want to verify their payment method has sufficient funds and lock those funds for fulfillment. The actual charge (capture) happens later when the order ships or service is delivered.

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
| `merchantTransactionId` | string | Yes | Your unique transaction reference |
| `amount` | Money | Yes | The amount for the payment in minor units (e.g., 1000 = $10.00) |
| `orderTaxAmount` | int64 | No | Tax amount for the order in minor units |
| `shippingCost` | int64 | No | Cost of shipping for the order in minor units |
| `paymentMethod` | PaymentMethod | Yes | Payment method to be used (card, wallet, bank) |
| `captureMethod` | CaptureMethod | No | Method for capturing. Values: MANUAL, AUTOMATIC |
| `customer` | Customer | No | Customer information for fraud scoring |
| `address` | PaymentAddress | No | Billing and shipping address |
| `authType` | AuthenticationType | Yes | Authentication flow type (e.g., THREE_DS, NO_THREE_DS) |
| `enrolledFor3ds` | bool | No | Whether 3DS enrollment check passed |
| `authenticationData` | AuthenticationData | No | 3DS authentication results |
| `metadata` | SecretString | No | Additional metadata for the connector (max 20 keys) |
| `returnUrl` | string | No | URL to redirect customer after 3DS/redirect flow |
| `webhookUrl` | string | No | URL for async webhook notifications |
| `testMode` | bool | No | Process as test transaction |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantTransactionId` | string | Your transaction reference (echoed back) |
| `connectorTransactionId` | string | Connector's transaction ID (e.g., Stripe pi_xxx) |
| `status` | PaymentStatus | Current status: AUTHORIZED, PENDING, FAILED, etc. |
| `error` | ErrorInfo | Error details if status is FAILED |
| `statusCode` | uint32 | HTTP-style status code (200, 402, etc.) |
| `incrementalAuthorizationAllowed` | bool | Whether amount can be increased later |

## Example

### SDK Setup

```java
import com.hyperswitch.prism.PaymentClient;
import java.util.Map;
import java.util.HashMap;

PaymentClient paymentClient = PaymentClient.builder()
    .connector("stripe")
    .apiKey("YOUR_API_KEY")
    .environment("SANDBOX")
    .build();
```

### Request

```java
Map<String, Object> amount = new HashMap<>();
amount.put("minorAmount", 1000);
amount.put("currency", "USD");

Map<String, Object> cardDetails = new HashMap<>();
cardDetails.put("cardNumber", Map.of("value", "4242424242424242"));
cardDetails.put("cardExpMonth", Map.of("value", "12"));
cardDetails.put("cardExpYear", Map.of("value", "2027"));
cardDetails.put("cardCvc", Map.of("value", "123"));
cardDetails.put("cardHolderName", Map.of("value", "John Doe"));

Map<String, Object> paymentMethod = new HashMap<>();
paymentMethod.put("card", cardDetails);

Map<String, Object> request = new HashMap<>();
request.put("merchantTransactionId", "txn_order_001");
request.put("amount", amount);
request.put("paymentMethod", paymentMethod);
request.put("authType", "NO_THREE_DS");
request.put("captureMethod", "MANUAL");
request.put("testMode", true);

Response response = paymentClient.authorize(request);
```

### Response

```java
{
    "merchantTransactionId": "txn_order_001",
    "connectorTransactionId": "pi_3Oxxx...",
    "status": "AUTHORIZED",
    "statusCode": 200,
    "incrementalAuthorizationAllowed": false
}
```

## Next Steps

- [capture](./capture.md) - Finalize the payment and transfer funds
- [void](./void.md) - Release held funds if order cancelled
- [get](./get.md) - Check current authorization status