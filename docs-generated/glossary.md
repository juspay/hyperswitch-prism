<!--
@doc-guidance
────────────────────────────────────────────────────
PAGE INTENT: All a aplhabetcail long list of all the temrinalogy and the descrioption for all the hard to understand temrinalogy used in this repository and docs,

AUDIENCE: Payment developers and architects
TONE: Direct, conversational, opinionated. Write like explaining to a colleague over coffee.
PRIOR READING: [What pages should the reader have seen before this one? Link them.]
LEADS INTO: [What page comes next?]
────────────────────────────────────────────────────
LENGTH: 1.5–2 pages max (~600–800 words prose, tables and code blocks don't count toward this).
         If you need more space, the page is doing too much — split it.

WRITING RULES:
1. FIRST SENTENCE RULE — Open with what the reader gains, not what the thing is.
   Bad:  "The Connector Service SDK provides a unified interface..."
   Good: "You call one method. It works with Stripe, Adyen, or any of 50+ processors."

2. NO HEDGING — Delete: "can be", "may", "it is possible to", "in some cases", "typically".
   Say what IS. If something is conditional, state the condition.

3. VERB OVER NOUN — "lets you configure" not "provides configuration capabilities".
   "transforms the request" not "performs request transformation".

4. ONE IDEA PER SECTION — If a section has more than one takeaway, split it.

5. SHOW THEN TELL — Code example or diagram first, explanation after.
   The reader should see what it looks like before reading why.

6. EARN EVERY SENTENCE — If removing a sentence doesn't lose information, remove it.
   No "In this section, we will discuss..." No "As mentioned earlier..."
   No "It is important to note that..."

7. SPECIFICS OVER CLAIMS — Never say "supports many payment methods".
   Say "supports cards, wallets (Apple Pay, Google Pay), bank transfers, BNPL, and UPI".

8. ERRORS ARE FEATURES — When documenting a flow, show what goes wrong too.
   Include at least one error scenario with the actual error message.

9. NAME THE PRODUCT "Connector Service" — Not "UCS", not "the service", not "our platform".

10. TABLES FOR COMPARISON, PROSE FOR NARRATIVE — Don't put a story in a table.
    Don't write paragraphs when a 3-row table would be clearer.

CODE EXAMPLE RULES:
- Every code block must be runnable or clearly marked as pseudocode
- Use test credentials: Stripe key as $STRIPE_API_KEY, card 4242424242424242
- Show the output, not just the input
- If the example needs setup, show the setup

ANTI-PATTERNS TO REJECT:
- "Comprehensive", "robust", "seamless", "leverage", "utilize", "facilitate"
- Starting paragraphs with "Additionally", "Furthermore", "Moreover"
- Any sentence that describes the documentation itself ("This guide covers...")
- Repeating the heading as the first sentence of a section
────────────────────────────────────────────────────
-->

# Glossary

A-Z reference for Connector Service terminology.

## A

**Authorize** — Hold funds on a customer's payment method without charging. Creates an authorization that can be captured later or voided. See [Authorize API](../../api-reference/services/payment-service/authorize.md).

**Authentication (3DS)** — 3D Secure verification process where the customer verifies ownership of their card through their bank (password, SMS, or biometric). See [Authentication Service](../../api-reference/services/payment-method-authentication-service/).

**Automatic Capture** — Capture funds immediately upon authorization. Used for digital goods and same-day fulfillment.

## B

**BNPL** — Buy Now Pay Later. Payment method type allowing customers to split purchases into installments (Klarna, Afterpay, Affirm).

## C

**Capture** — Complete a payment by transferring funds from the customer's account. Can be full or partial amount. See [Capture API](../../api-reference/services/payment-service/capture.md).

**Capture Method** — `AUTOMATIC` (immediate) or `MANUAL` (merchant-initiated). Determines when funds transfer.

**Connector** — Payment processor integration (Stripe, Adyen, PayPal, etc.). Connector Service supports 50+ connectors.

**Connector Adapter** — Rust module that translates unified requests to connector-specific formats. See [Connectors](../../connectors/).

**Connector Service** — The unified payment abstraction library this documentation describes.

**Customer** — Entity representing a payer. Can have stored payment methods and transaction history.

## D

**Decline** — Rejection of a payment by the issuer or processor. Reasons: insufficient funds, expired card, incorrect CVV, etc.

**Dispute** — Chargeback initiated by a customer through their bank. Requires merchant response (accept or defend).

**DSL** — Domain-Specific Language. The Protocol Buffer schema that defines Connector Service's typed API.

## E

**Environment** — Deployment mode: `development` (mock), `sandbox` (test credentials), `production` (live transactions).

**Error Code** — Unified error identifier (e.g., `PAYMENT_DECLINED`, `NETWORK_TIMEOUT`). Same code across all connectors.

**Event Service** — Handles service specific webhooks (payment/refund/dispute) from payment processors. See [Event Service](../../api-reference/services/event-service/).

## F

**FFI** — Foreign Function Interface. Mechanism allowing SDKs to call Rust core code from Node.js, Python, Java, etc.

**Flow** — Payment operation type (Authorize, Capture, Refund, etc.).

## G

**gRPC** — High-performance RPC framework using Protocol Buffers. Used for microservice mode.

## H

**Handle Event** — Process any incoming webhook from a payment processor. See [handle](../../api-reference/services/event-service/handle.md).

**Hyperswitch** — Open-source Composable Payments Platform built by Juspay with 40K+ Github stars and used by global enterprise companies. The Connector Service is a component of Juspay Hyperswitch.

## I

**Idempotency Key** — Unique identifier preventing duplicate operations. Retry the same request safely.

**Incremental Authorization** — Increase authorized amount after initial authorization. Used by hotels and car rentals.

## M

**Manual Capture** — Merchant-initiated capture (two-step payment flow).

**Merchant** — Business entity processing payments through Connector Service.

**Metadata** — Key-value pairs attached to payments for reconciliation and reporting.

## O

**Override** — Request-level configuration that supersedes connector defaults.

## P

**Partial Capture** — Capture less than authorized amount. Used for multi-shipment orders.

**Payment Intent** — Stripe's term for a payment authorization. Connector Service unifies this concept across all connectors.

**Payment Method** — How customer pays: card, wallet, bank transfer, BNPL.

**Payment Service** — Core service handling authorizations, captures, voids. See [Payment Service](../../api-reference/services/payment-service/).

**Payment Status** — Lifecycle state: `STARTED`, `AUTHORIZED`, `CAPTURED`, `FAILED`, `VOIDED`.

**Protocol Buffers** — Binary serialization format. Defines Connector Service's typed schema.

**PSP** — Payment Service Provider. Synonym for payment processor/connector.

## R

**Recurring Payment** — Subscription billing using stored payment methods.

**Refund** — Return captured funds to customer. Can be partial or full amount.

**Refund Service** — Handles refunds and refund status checks. See [Refund Service](../../api-reference/services/refund-service/).

**Retry** — Re-attempt failed requests for retryable errors (network timeouts, rate limits).

**Return URL** — Where customer returns after 3D Secure or redirect-based payments.

**Reverse** — Refund using connector transaction ID instead of Connector Service payment ID.

## S

**SDK** — Software Development Kit. Language-specific client libraries (Node.js, Python, Java, Rust, Go).

**Service** — API endpoint grouping (PaymentService, RefundService, EventService).

**Status** — Current state of a payment, refund, or other entity.

**Sub-service** — Service that extends another (RefundService is sub-service of PaymentService).

**Sync** — Retrieve latest status from payment processor.

## T

**Token** — Secure reference to stored payment method (PCI-safe alternative to raw card data).

**Transformer** — Function converting unified types to connector-specific formats.

## U

**Unified Error** — Consistent error format regardless of underlying connector.

**Unified Type** — Common data structure used across all connectors (Money, PaymentMethod, Address).

## V

**Validation** — Schema-level checks ensuring request correctness before sending to processors.

**Void** — Cancel an authorization without charging. Releases held funds.

## W

**Wallet** — Digital payment method (Apple Pay, Google Pay, PayPal).

**Webhook** — HTTP callback from payment processor notifying of events (payment captured, refund completed).

**Webhook Secret** — Shared key for verifying webhook authenticity.
