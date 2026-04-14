# Payment Service

<!--
---
title: Payment Service (Node SDK)
description: Complete payment lifecycle management - authorize, capture, refund, and void payments using the Node.js SDK
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

The Payment Service provides comprehensive payment lifecycle management for digital businesses. It enables you to process payments across 100+ connectors through a unified SDK, handling everything from initial authorization to refunds and recurring payments.

**Business Use Cases:**
- **E-commerce checkout** - Authorize funds at purchase, capture when items ship
- **SaaS subscriptions** - Set up recurring payments with mandate management
- **Marketplace platforms** - Hold funds from buyers, release to sellers on fulfillment
- **Hotel/travel bookings** - Pre-authorize for incidentals, capture adjusted amounts
- **Digital goods delivery** - Immediate capture for instant-access products

The service supports both synchronous responses and asynchronous flows (3DS authentication, redirect-based payments), with state management for multi-step operations.

## Operations

| Operation | Description | Use When |
|-----------|-------------|----------|
| [`authorize`](./authorize.md) | Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing. | Two-step payment flow, verify funds before shipping |
| [`capture`](./capture.md) | Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle. | Order shipped/service delivered, ready to charge |
| [`get`](./get.md) | Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking. | Check payment status, webhook recovery, pre-fulfillment verification |
| [`void`](./void.md) | Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned. | Order cancelled before shipping, customer request |
| [`reverse`](./reverse.md) | Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations. | Same-day cancellation, processing error correction |
| [`refund`](./refund.md) | Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment. | Product returns, post-settlement cancellations |
| [`incrementalAuthorization`](./incremental-authorization.md) | Increase authorized amount if still in authorized state. Allows adding charges to existing authorization for hospitality, tips, or incremental services. | Hotel incidentals, restaurant tips, add-on services |
| [`createOrder`](./create-order.md) | Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates. | Pre-checkout setup, session initialization |
| [`verifyRedirectResponse`](./verify-redirect-response.md) | Validate redirect-based payment responses. Confirms authenticity of redirect-based payment completions to prevent fraud and tampering. | 3DS completion, bank redirect verification |
| [`setupRecurring`](./setup-recurring.md) | Setup a recurring payment instruction for future payments/debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases. | Subscription setup, recurring billing |

## Common Patterns

### E-commerce Checkout Flow

Standard two-step payment flow for physical goods. Authorize at checkout, capture when shipped.

```mermaid
sequenceDiagram
    participant App as Your App
    participant CS as Prism
    participant PP as Payment Provider

    App->>CS: 1. createOrder
    CS->>PP: Create order with provider
    PP-->>CS: Return provider order
    CS-->>App: Return order_context
    App->>CS: 2. authorize (with order_context)
    CS->>PP: Reserve funds
    PP-->>CS: Return authorization
    CS-->>App: Return connector_transaction_id (AUTHORIZED)
    Note over App: Order ships to customer
    App->>CS: 3. capture (when order ships)
    CS->>PP: Transfer funds
    PP-->>CS: Return capture confirmation
    CS-->>App: Return status: CAPTURED
```

**Flow Explanation:**

1. **createOrder** - Initialize a payment order at the processor before collecting payment details. This sets up the payment context and returns an `order_context` that improves authorization rates by associating the eventual payment with this initial order.

2. **authorize** - After the customer enters their payment details, call the `authorize` method with the `order_context` from step 1. This reserves the funds on the customer's payment method without transferring them. The response includes a `connector_transaction_id` and status `AUTHORIZED`. The funds are now held but not yet charged.

3. **capture** - Once the order is shipped, call the `capture` method with the `connector_transaction_id` from step 2. This finalizes the transaction and transfers the reserved funds from the customer to your merchant account. The status changes to `CAPTURED`.

**Cancellation Path:**
If the customer cancels before shipping, call the `void` method instead of `capture` to release the held funds back to the customer.

---

### SaaS Subscription Setup

Set up recurring payments for subscription businesses. Authorize initial payment, set up mandate for future charges.

```mermaid
sequenceDiagram
    participant App as Your App
    participant CS as Prism
    participant PP as Payment Provider

    App->>CS: 1. setupRecurring
    CS->>PP: Create mandate
    PP-->>CS: Return mandate_reference
    CS-->>App: Return mandate_reference
    App->>CS: 2. authorize (with mandate_reference)
    CS->>PP: Reserve funds
    PP-->>CS: Return authorization
    CS-->>App: Return connector_transaction_id (AUTHORIZED)
    App->>CS: 3. capture (immediate for first charge)
    CS->>PP: Transfer funds
    PP-->>CS: Return capture confirmation
    CS-->>App: Return status: CAPTURED
    Note over App: Next billing cycle (e.g., 30 days later)
    App->>CS: 4. charge (with stored mandate_reference)
    CS->>PP: Create recurring charge using mandate
    PP-->>CS: Return charge confirmation
    CS-->>App: Return status: CHARGED
```

**Flow Explanation:**

1. **setupRecurring** - Before the first charge, call the `setupRecurring` method to create a payment mandate at the processor. A mandate is the customer's authorization for future recurring charges. The response includes a `mandate_reference` that represents this stored consent.

2. **authorize** - For the initial subscription charge, call the `authorize` method with the `mandate_reference` from step 1. This links the payment to the mandate and reserves the funds on the customer's payment method. The response includes a `connector_transaction_id` with status `AUTHORIZED`.

3. **capture** - Since this is an immediate charge (not a delayed shipment), call the `capture` method right after authorization. This transfers the reserved funds and completes the initial subscription payment. The status changes to `CAPTURED`.

4. **charge (subsequent billing)** - For future billing cycles (e.g., monthly renewal), call the Recurring Payment Service's `charge` method with the stored `mandate_reference`. This creates a new charge using the saved mandate without requiring the customer to re-enter payment details or be present. The processor returns a new `connector_transaction_id` with status `CHARGED` for the recurring payment.

---

### Hotel/Travel Booking with Incremental Charges

Pre-authorize for room plus incidentals, add charges during stay, capture final amount.

```mermaid
sequenceDiagram
    participant App as Your App
    participant CS as Prism
    participant PP as Payment Provider

    App->>CS: 1. authorize (room rate + $200 incidentals)
    CS->>PP: Reserve funds
    PP-->>CS: Return authorization
    CS-->>App: Return connector_transaction_id (AUTHORIZED)
    Note over App: Guest checks in
    App->>CS: 2. get (verify authorization active)
    CS->>PP: Check status
    PP-->>CS: Return status: AUTHORIZED
    CS-->>App: Return status: AUTHORIZED
    Note over App: Guest charges room service ($50)
    App->>CS: 3. incrementalAuthorization (add $50)
    CS->>PP: Increase authorization
    PP-->>CS: Return new authorized amount
    CS-->>App: Return new authorized amount
    Note over App: Guest checks out (total: room + $50)
    App->>CS: 4. capture (final amount only)
    CS->>PP: Transfer funds
    PP-->>CS: Return capture confirmation
    CS-->>App: Return status: CAPTURED
    App->>CS: 5. void (remaining incidental hold)
    CS->>PP: Release unused hold
    PP-->>CS: Return void confirmation
    CS-->>App: Return status: VOIDED
```

**Flow Explanation:**

1. **authorize (initial hold)** - At booking or check-in, call the `authorize` method with the room rate plus an additional amount for incidentals (e.g., $200). This reserves the total amount on the customer's card. The response includes a `connector_transaction_id` with status `AUTHORIZED`.

2. **get (verify status)** - Before adding charges, call the `get` method to verify the authorization is still active and hasn't expired or been cancelled. This returns the current status of the authorization.

3. **incrementalAuthorization** - When the guest adds charges (e.g., room service for $50), call the `incrementalAuthorization` method to increase the authorized amount. This ensures the final capture won't be declined for exceeding the original authorization.

4. **capture (final amount)** - At checkout, call the `capture` method with the actual final amount (room rate + room service charges). Only this amount is transferred from the customer. The status changes to `CAPTURED`.

5. **void (unused hold)** - After capturing the final amount, call the `void` method to release the remaining incidental hold that was authorized but not charged (the $200 incidental buffer minus any incidental charges). This returns the unused funds to the customer's available balance.

---

## SDK Setup

```javascript
const { PaymentClient } = require('hyperswitch-prism');

const paymentClient = new PaymentClient({
    connector: 'stripe',
    apiKey: 'YOUR_API_KEY',
    environment: 'SANDBOX' // or 'PRODUCTION'
});
```

## Next Steps

- [Refund Service](../refund-service/README.md) - Process refunds and returns
- [Dispute Service](../dispute-service/README.md) - Handle chargebacks and disputes
- [Customer Service](../customer-service/README.md) - Manage customer payment methods
