# Stripe Connector

React wrapper around **Stripe.js**, built on the official `@stripe/react-stripe-js` `<Elements>` + `<CardElement>`. It confirms the PaymentIntent `client_secret` from the Medusa payment session with `confirmCardPayment` — mirroring Medusa's official storefront integration. This is deliberate: the Payment Element's `confirmPayment` is gated behind hCaptcha (which blocks automation and differs from Medusa's UX), whereas the Card Element + `confirmCardPayment` path is not. Stripe is the only fully **synchronous** connector: the backend can poll the payment status directly, so no webhooks are involved.

## Components

| Piece | File | Role |
|---|---|---|
| `StripeWrapper` | `StripeWrapper.tsx` | Memoizes `loadStripe(publishableKey)`, renders `<Elements>` + `<CardElement>`, calls `confirmCardPayment` on "Pay now" |
| Panel dispatch | `../../components/HyperswitchPrismConnectorPanel.tsx` | Returns `null` for Stripe — host storefronts usually mount their own Stripe Elements; this wrapper is the standalone alternative |
| Place Order button | `../../components/payment-buttons/StripePaymentButton.tsx` | Completes the cart after confirmation |

## Payment flow (storefront ↔ test-project)

```
Storefront              Medusa (test-project)        UCS / Prism           Stripe
    |                          |                          |                    |
    |-- initiate session ----->|-- create payment ------->|-- PaymentIntent -->|
    |<- client_secret ---------|<-------------------------|<-- pi_..._secret --|
    |                          |                          |                    |
    |== Card Element: confirmCardPayment(client_secret) =======================>|
    |   onSubmit(paymentIntent)|                          |                    |
    |                          |                          |                    |
    |-- Place order ---------->|-- PaymentService.get --->|--- GET intent ---->|
    |<-- order ----------------|<-- ✅ AUTHORIZED ---------| requires_capture  |
    |                          |   funds reserved; Capture or Void available   |
```

> **Result: AUTHORIZED** — Stripe PaymentIntent is created with manual capture mode. The card is confirmed client-side (funds reserved) but not charged. An explicit capture call (`POST /admin/payments/:id/capture`) is required to settle the payment.

1. Initiate: the provider plugin creates a payment through UCS, which creates a Stripe PaymentIntent; its `client_secret` lands in `payment_session.data`.
2. The wrapper mounts the Card Element with that secret; "Pay now" runs `stripe.confirmCardPayment(clientSecret, { payment_method: { card } })` — confirmation completes in the browser without an hCaptcha challenge.
3. Place Order completes the cart; the plugin's `authorizePayment` syncs the live status via UCS `PaymentService.get` (PSync) and authorizes the session.
4. Capture/refund flow through UCS to the Stripe API. Stripe webhooks are not supported by UCS — incoming events are acknowledged and ignored.

## Session data consumed

| Wrapper prop | Source in Medusa `session.data` |
|---|---|
| `clientSecret` | `client_secret` |
| `publishableKey` | Storefront env (`NEXT_PUBLIC_STRIPE_KEY`) or `publishableKey` |

## Test-project setup

- Provider id: `pp_hyperswitch-prism_hyperswitch-prism-stripe` (config `id: hyperswitch-prism-stripe`, `connector: "stripe"`, `apiKey` in `connectorConfig`).
- No webhook configuration needed.
