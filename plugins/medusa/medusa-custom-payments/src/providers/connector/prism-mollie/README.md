# Mollie Connector

Server-side provider logic for **Mollie** via the Hyperswitch Prism connector service. Mollie uses **Mollie Components** (in-page card tokenization): the card is entered and tokenized client-side into a single-use `cardToken` (card data never touches the server, PCI SAQ-A), the token is persisted to the session, and the server then charges it. One-off card payments complete via a **3DS redirect**, after which the order is finalised.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment`, `reInitiatePayment`, `authorizePayment`, `shouldSkipVoid` |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism            Mollie
    |                        |                            |                    |
    |-- initiate session --->|  initiatePayment           |                    |
    |<- profileId -----------|  (returns public profileId, no network call)    |
    |                        |                            |                    |
    |== Mollie Components: tokenize card (client-side) =======================>|
    |   createToken() -> cardToken                                             |
    |                        |                            |                    |
    |-- re-initiate session->|  reInitiatePayment:        |                    |
    |   { cardToken,         |  persist token + returnUrl in session.data      |
    |     returnUrl }        |                            |                    |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|                    |
    |                        |   CaptureMethod.AUTOMATIC  |--- POST /payments ->|
    |                        |   paymentMethod.token      |   (cardToken)       |
    |<- requires_more -------|<-- redirectUrl ------------|<-- status: open ----|
    |                        |                            |                    |
    |== Redirect to Mollie 3DS (GET checkout URL) ===========================>|
    |   customer authenticates                                                 |
    |<== Redirect back to returnUrl ========================================|
    |                        |                            |                    |
    |-- place order (retry)->|-- authorizePayment ------->|                    |
    |                        |   (connectorTransactionId  |                    |
    |                        |    present -> PSync) ------>|--- GET /payments ->|
    |<-- order --------------|<-- CAPTURED ---------------|<-- status: paid ----|
```

> **Result: CAPTURED** — `authorizePayment` charges with `CaptureMethod.AUTOMATIC`, so Mollie collects funds immediately. Mollie's terminal status `paid` → connector `Charged` → `PaymentStatus.CHARGED` → Medusa `PaymentSessionStatus.CAPTURED`. Funds are collected when the order completes; only refunds are available afterward (no separate capture step).

### Step-by-step

1. **initiatePayment** — returns the public `profileId` (from `connectorConfig.profileToken`) in `payment_session.data` for the client-side Mollie Components form. No network call: the Components short-circuit skips the hosted client-auth that would otherwise create an orphan redirect payment.
2. **Components (client)** — the React `MollieWrapper` loads `mollie.js`, mounts the in-page card fields, and on "Pay" calls `mollie.createToken()` to obtain a single-use `cardToken`.
3. **Re-initiation** — the storefront calls `initiatePayment` again with `{ cardToken, returnUrl }` in `data`. The provider detects `data.cardToken` and routes to `reInitiatePayment`, which stores the token and the storefront return URL in `session.data` without another network call.
4. **authorizePayment** — charges the card via UCS with `paymentMethod.token` and `CaptureMethod.AUTOMATIC`. One-off cards return `requires_more` plus a 3DS `redirectUrl` (a GET URL reconstructed from Mollie's checkout link); the provider adopts Mollie's `tr_…` reference as both `id` and `connectorTransactionId`.
5. **3DS redirect** — the storefront sends the customer to `redirectUrl`. Mollie authenticates and redirects back to the `returnUrl`, which re-runs order placement.
6. **Idempotent retry (PSync)** — on the retry, `authorizePayment` sees an existing `connectorTransactionId` and **syncs status via `getPaymentStatus`** instead of re-authorizing (the `cardToken` is single-use). Mollie `paid` → `CAPTURED` and the order is created.
7. **Refund / Cancel** — go through UCS synchronously. `shouldSkipVoid` skips the void when no real Mollie payment exists yet (`data.id` still equals the Medusa session id).

## Session data

Produced by `initiatePayment`:

| Field | Description |
|-------|-------------|
| `profileId` | Public Mollie profile id (`pfl_…`) for the Mollie Components form |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

Added after re-initiation:

| Field | Description |
|-------|-------------|
| `cardToken` | Single-use Mollie Components card token |
| `returnUrl` | Storefront URL Mollie redirects to after 3DS |

Added after `authorizePayment`:

| Field | Description |
|-------|-------------|
| `id`, `connectorTransactionId` | Mollie payment reference (`tr_…`) — drives the idempotent PSync on retry |
| `redirectUrl` | 3DS GET URL the storefront must send the customer to |
| `prismStatus` | Raw connector status (diagnostics) |

## Webhooks

Not supported by UCS for Mollie. Payment state is driven by the synchronous `authorizePayment` response (with `getPaymentStatus` PSync on the post-3DS retry).

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `apiKey` | `{ value: string }` | Mollie API key (`test_...` / `live_...`) |
| `profileToken` | `{ value: string }` | Public Mollie profile id (`pfl_…`); surfaced to the storefront as `profileId` so Mollie Components can initialise |

## Test card

| Number | Expiry | CVV |
|--------|--------|-----|
| `4111 1111 1111 1111` | `12/2030` | `123` |

> **Note**: Mollie **test mode** always replaces the live flow with a hosted test-mode screen where you select the final status (Paid / Failed / Open / Expired) — there is no no-3DS test card. Selecting **Paid** drives the payment to `paid` → `CAPTURED`.
