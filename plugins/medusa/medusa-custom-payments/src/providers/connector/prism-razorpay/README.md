# Razorpay Connector

Server-side provider logic for **Razorpay** via the Hyperswitch Prism connector service. Razorpay is an India-first processor: payments here are **UPI** based (UPI Collect / UPI Intent), so authorization completes **out-of-band** — the shopper approves in their UPI app and is redirected back.

> **Enablement note.** Razorpay required a `RazorpayConfig` message in the connector-service proto (`payment.proto`) and gRPC connector-config wiring — added in the same change that introduced this module (the Rust connector `razorpay.rs` already existed and is registered). The published `hyperswitch-prism` SDK must be regenerated from the updated proto for live calls; the plugin passes `connectorConfig` through an `as IConnectorConfig` cast, so this module type-checks against the current SDK regardless.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` — surfaces the Razorpay SDK session token / order id into `session.data`; `reInitiatePayment` — stores the chosen UPI VPA / method + billing; `authorizePayment` — builds the UPI payment and returns the next step as `data.redirectUrl`; `shouldSkipVoid` |

## Payment flow

```
Storefront                 Medusa backend              UCS / Prism         Razorpay
    |                          |                           |                    |
    |-- initiate session ----->|-- SDKSessionToken ------->|-- token/order ---->|
    |<- sessionToken/orderId --|<--------------------------|<-------------------|
    |                          |                           |                    |
    |== shopper picks UPI, enters VPA ====================>|                    |
    |-- reinitiate {vpa, billing} -->| store on session    |                    |
    |-- place order ---------->|-- authorizePayment ------>|--- UPI Collect --->|
    |                          |   redirectionData <-------|<-- intent/none ----|
    |<- redirectUrl -----------|                           |                    |
    |== shopper approves in UPI app =======================================>    |
    |-- return → cart.complete ->| authorizePayment(retry) → PSync ----------->|
    |<-- order ----------------|<-- status ----------------|                    |
```

> **Result: out-of-band.** UPI Collect pushes a request to the shopper's UPI app; UPI Intent returns a `redirectUrl` (deep-link) the storefront follows. The post-redirect `cart.complete` retry status-syncs (PSync) the existing payment rather than re-authorizing (idempotent on `data.connectorTransactionId`).

### Step-by-step

1. **initiatePayment** — UCS runs Razorpay's SDKSessionToken flow and returns `sessionData.connectorSpecific.razorpay` (session token, and an `order_…` id for hosted Checkout), surfaced as `data.sessionToken` / `data.orderId`.
2. **reInitiatePayment** — the storefront persists the chosen `vpa` / `paymentMethodType` + `billing` on the session (no network call).
3. **authorizePayment** — builds the SDK `authorize` request: `upi_collect` → `paymentMethod.upiCollect.vpaId` (VPA pushed to the shopper's UPI app), `upi_intent` → `paymentMethod.upiIntent` (deep-link). Razorpay requires `firstName` + `email` billing, so a missing billing returns `ERROR`. The next step is surfaced as `data.redirectUrl` and the real Razorpay reference is adopted as `data.id`.
4. **Capture / Refund / Cancel / PSync** — go through UCS synchronously.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `sessionToken` | Razorpay SDK session token (when the SDKSessionToken flow returns one) |
| `orderId` | Razorpay order id (`order_…`) for hosted Checkout, when present |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

## Webhooks

Not wired in the plugin. Payment state is driven by the synchronous flows (`authorizePayment` → PSync).

## Credentials (`connectorConfig`)

Razorpay uses **BodyKey** auth.

| Field | Type | Description |
|-------|------|-------------|
| `apiKey` | `{ value: string }` | Razorpay key id (`rzp_test_…` / `rzp_live_…`) |
| `apiSecret` | `{ value: string }` (optional) | Razorpay key secret |
| `baseUrl` | `string` (optional) | Override the Razorpay base URL |
| `returnUrl` | `string` (optional) | URL the shopper returns to after the redirect |

In `creds.json` (snake_case): `{ "razorpay": { "api_key": "rzp_test_...", "api_secret": "KEY_SECRET", "return_url": "http://localhost:3000/return" } }`.

## Testing note

Razorpay's UPI authorization cannot be completed headlessly (it needs a real UPI-app approval). The e2e coverage is an API-level lifecycle check (`app/e2e/api/lifecycle.spec.ts`) asserting the initiate session shape, plus mocked unit tests (`__tests__/unit/razorpay.test.ts`) covering request-building and response-mapping. The SDK's native FFI library requires GLIBC ≥ 2.38, so live calls run only in the CI/e2e environment (with the regenerated SDK).
