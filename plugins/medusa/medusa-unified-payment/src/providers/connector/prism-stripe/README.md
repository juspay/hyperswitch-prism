# Stripe Connector

Server-side provider logic for **Stripe** via the Hyperswitch Prism connector service. Stripe uses **Payment Intents**: a `client_secret` is created server-side and the card is confirmed client-side using Stripe Elements. State is driven by the synchronous `getPaymentStatus` call — no webhooks are required.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` only — Stripe capture/refund/cancel are handled by the generic prism service paths via UCS |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism            Stripe
    |                        |                            |                    |
    |-- initiate session --->|-- CreatePaymentIntent ----->|--- /payment_intents|
    |<- client_secret -------|<---------------------------|<-- client_secret ---|
    |                        |                            |                    |
    |== Stripe Elements: customer enters card and confirms ====================>|
    |   stripe.confirmCardPayment(clientSecret)                                |
    |   → PaymentIntent: requires_capture (manual) / succeeded (auto)         |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|                    |
    |                        |   getPaymentStatus ------->|--- retrieve PI --->|
    |<-- order --------------|<-- ✅ AUTHORIZED -----------|<-- requires_capture|
    |                        |   (manual capture required separately)          |
```

> **Result: AUTHORIZED** — `getPaymentStatus` retrieves the Payment Intent via UCS. The intent is created with manual capture, so the card is reserved but not charged. An explicit capture call settles the payment.

### Step-by-step

1. **initiatePayment** — calls UCS to create a Stripe Payment Intent. Returns `client_secret` in `payment_session.data`.
2. **Stripe Elements (client)** — the React `StripeWrapper` mounts the card form and calls `stripe.confirmCardPayment(clientSecret)` when the customer submits. The card is charged client-side.
3. **authorizePayment** — retrieves the Payment Intent via UCS `getPaymentStatus`. If its status is `succeeded` (or `requires_capture` for manual capture), the session is marked `authorized`.
4. **Capture / Refund / Cancel** — all go through UCS synchronously.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `client_secret` | Stripe Payment Intent client secret for `confirmCardPayment` |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

## Webhooks

Not required. Payment state is retrieved synchronously via `getPaymentStatus` (UCS → Stripe retrieve Payment Intent).

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `apiKey` | `{ value: string }` | Stripe secret key (`sk_test_...` / `sk_live_...`) |
| `publishableKey` | `string \| { value: string }` | Publishable key (`pk_test_...` / `pk_live_...`), optional. Not a secret — surfaced in the payment session as `publishableKey` for the storefront's Elements. |

## Test card

| Number | Expiry | CVV |
|--------|--------|-----|
| `4242 4242 4242 4242` | `03/2030` | `737` |
