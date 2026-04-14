# Authorize Method

<!--
---
title: Authorize (Node SDK)
description: Authorize a payment using the Node.js SDK - reserve funds without capturing
last_updated: 2026-03-21
generated_from: backend/grpc-api-types/proto/services.proto
auto_generated: true
reviewed_by: ''
reviewed_at: ''
approved: false
sdk_language: node
---
-->

## Overview

The `authorize` method reserves funds on a customer's payment method without transferring them. This is the first step in a two-step payment flow (authorize + capture), commonly used in e-commerce, marketplaces, and subscription businesses.

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
| `connectorFeatureData` | SecretString | No | Connector-specific feature data for the transaction |
| `returnUrl` | string | No | URL to redirect customer after 3DS/redirect flow |
| `webhookUrl` | string | No | URL for async webhook notifications |
| `completeAuthorizeUrl` | string | No | URL to complete authorization after redirect |
| `sessionToken` | string | No | Session token for wallet payments (Apple Pay, Google Pay) |
| `orderCategory` | string | No | Category of goods/services being purchased |
| `merchantOrderId` | string | No | Your internal order ID |
| `setupFutureUsage` | FutureUsage | No | ON_SESSION or OFF_SESSION for tokenization |
| `offSession` | bool | No | Whether customer is present (false = customer present) |
| `requestIncrementalAuthorization` | bool | No | Allow increasing authorized amount later |
| `requestExtendedAuthorization` | bool | No | Request extended hold period |
| `enablePartialAuthorization` | bool | No | Allow partial approval (e.g., $80 of $100) |
| `customerAcceptance` | CustomerAcceptance | No | Customer consent for recurring payments |
| `browserInfo` | BrowserInformation | No | Browser details for 3DS fingerprinting |
| `paymentExperience` | PaymentExperience | No | Desired UX (e.g., REDIRECT, EMBEDDED) |
| `description` | string | No | Payment description shown on statements |
| `paymentChannel` | PaymentChannel | No | E-commerce, MOTO, or recurring indicator |
| `testMode` | bool | No | Process as test transaction |
| `setupMandateDetails` | SetupMandateDetails | No | Mandate setup for recurring SEPA/bank debits |
| `statementDescriptorName` | string | No | Your business name on customer statement |
| `statementDescriptorSuffix` | string | No | Additional descriptor suffix |
| `billingDescriptor` | BillingDescriptor | No | Complete billing descriptor configuration |
| `state` | ConnectorState | No | State from previous multi-step flow |
| `orderDetails` | OrderDetailsWithAmount[] | No | Line item details with amounts |
| `locale` | string | No | Locale for localized responses (e.g., "en-US") |
| `tokenizationStrategy` | Tokenization | No | Tokenization configuration |
| `threedsCompletionIndicator` | ThreeDsCompletionIndicator | No | 3DS method completion status |
| `redirectionResponse` | RedirectionResponse | No | Response data from redirect-based auth |
| `continueRedirectionUrl` | string | No | URL to continue after redirect |
| `paymentMethodToken` | SecretString | No | Token for previously saved payment method |

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `merchantTransactionId` | string | Your transaction reference (echoed back) |
| `connectorTransactionId` | string | Connector's transaction ID (e.g., Stripe pi_xxx) |
| `status` | PaymentStatus | Current status: AUTHORIZED, PENDING, FAILED, etc. |
| `error` | ErrorInfo | Error details if status is FAILED |
| `statusCode` | uint32 | HTTP-style status code (200, 402, etc.) |
| `responseHeaders` | map<string,string> | Connector-specific response headers |
| `redirectionData` | RedirectForm | Redirect URL/form for 3DS or bank authentication |
| `networkTransactionId` | string | Card network transaction reference |
| `incrementalAuthorizationAllowed` | bool | Whether amount can be increased later |
| `state` | ConnectorState | State to pass to next request in multi-step flow |
| `rawConnectorResponse` | SecretString | Raw API response from connector (debugging) |
| `rawConnectorRequest` | SecretString | Raw API request sent to connector (debugging) |
| `capturedAmount` | int64 | Amount already captured (0 for fresh auth) |

## Example

### SDK Setup

```javascript
const { PaymentClient } = require('hyperswitch-prism');

const paymentClient = new PaymentClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX'
});
```

### Request

```javascript
const request = {
    merchantTransactionId: "txn_order_001",
    amount: {
        minorAmount: 1000,
        currency: "USD"
    },
    paymentMethod: {
        card: {
            cardNumber: { value: "4242424242424242" },
            cardExpMonth: { value: "12" },
            cardExpYear: { value: "2027" },
            cardCvc: { value: "123" },
            cardHolderName: { value: "John Doe" }
        }
    },
    authType: "NO_THREE_DS",
    captureMethod: "MANUAL",
    testMode: true
};

const response = await paymentClient.authorize(request);
```

### Response

```javascript
{
    merchantTransactionId: "txn_order_001",
    connectorTransactionId: "pi_3Oxxx...",
    status: "AUTHORIZED",
    statusCode: 200,
    incrementalAuthorizationAllowed: false
}
```

## Next Steps

- [Capture](./capture.md) - Finalize the payment and transfer funds
- [Void](./void.md) - Release held funds if order cancelled
- [Get](./get.md) - Check current authorization status
