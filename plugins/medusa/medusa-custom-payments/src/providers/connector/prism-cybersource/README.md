# Cybersource Connector

Server-side provider logic for **Cybersource** via the Hyperswitch Prism connector service. Cybersource uses a **Capture Context** (JWT) for Unified Checkout — the capture context is fetched server-side and used client-side to initialize the payment form. Card authorization is synchronous.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` — fetches the capture context and stores it in `session.data`; authorize/capture/refund/cancel go through generic UCS paths |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism           Cybersource
    |                        |                            |                    |
    |-- initiate session --->|-- GenerateCaptureContext -->|--- capture context>|
    |<- captureContext JWT --|<---------------------------|<-- JWT ------------|
    |                        |                            |                    |
    |== Unified Checkout: customer enters card ================================>|
    |   flex.createToken(captureContext) → transientToken                      |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|--- authorize ----->|
    |                        |   getPaymentStatus <-------|<-- AUTHORIZED -----|
    |<-- order --------------|<-- ✅ AUTHORIZED -----------|                    |
    |                        |   (manual capture required separately)          |
```

> **Result: AUTHORIZED** — `authorizePayment` delegates to `getPaymentStatus` via UCS. Cybersource is a two-step processor: the transient token authorizes the card (reserves funds) but does not capture. An explicit capture call settles the payment.

### Step-by-step

1. **initiatePayment** — calls UCS to generate a Cybersource capture context JWT. Returns `captureContext` in `payment_session.data`.
2. **Unified Checkout (client)** — the storefront loads the Cybersource Flex Microform JS SDK, initializes it with the `captureContext`, and renders the hosted card fields. On submit, `flex.createToken()` produces a transient token.
3. **authorizePayment** — forwards the transient token through UCS to authorize the payment. Returns `authorized` on success.
4. **Capture / Refund / Cancel** — all go through UCS synchronously.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `captureContext` | Cybersource capture context JWT (for Flex Microform SDK initialization) |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

## Webhooks

Not required. Payment state is retrieved synchronously via `authorizePayment` / `getPaymentStatus`.

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `apiKey` | `{ value: string }` | Cybersource API key |
| `apiSecret` | `{ value: string }` | Cybersource API secret |
| `merchantAccount` | `{ value: string }` | Cybersource merchant ID |
