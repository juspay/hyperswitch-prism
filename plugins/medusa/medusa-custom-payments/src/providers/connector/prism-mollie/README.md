# Mollie Connector

Server-side provider logic for **Mollie** via the Hyperswitch Prism connector service. Two payment methods are supported:

- **Card (Mollie Components)** — in-page card tokenization: the card is entered and tokenized client-side into a single-use `cardToken` (card data never touches the server, PCI SAQ-A), the token is persisted to the session, and the server then charges it. One-off card payments complete via a **3DS redirect**, after which the order is finalised.
- **Klarna (Pay later)** — a **redirect** method with **no client-side tokenization**. The storefront collects billing (name + email + postal address) and re-initiates with `paymentMethodType: "klarna"`; `authorizePayment` builds the Klarna payment and Mollie returns a **hosted Klarna checkout URL** to redirect to. Klarna via Mollie is **EUR-only / EU markets**.

Both methods capture automatically (`CaptureMethod.AUTOMATIC`) and surface the next step as `data.redirectUrl` for the storefront to follow; the post-redirect retry PSyncs to the terminal status.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment`, `reInitiatePayment`, `authorizePayment`, `shouldSkipVoid`; `KlarnaBilling` + `toKlarnaBillingAddress` (maps the storefront billing form to the SDK billing address for the Klarna arm) |

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

## Klarna (Pay later) flow

Klarna is a redirect method — there is no in-page tokenization. The storefront collects billing in a form (`MollieKlarnaForm`) and Klarna via Mollie requires a **EUR** session.

```
Storefront              Medusa backend               UCS / Prism            Mollie
    |                        |                            |                    |
    |== create EUR session ==|  initiatePayment (mollie)  |                    |
    |   (Klarna is EU-only)  |  returns profileId         |                    |
    |                        |                            |                    |
    |== Klarna billing form: name + email + address =========================>|
    |                        |                            |                    |
    |-- re-initiate session->|  reInitiatePayment:        |                    |
    |   { paymentMethodType: |  persist billing + method  |                    |
    |     "klarna", billing, |  in session.data (no call) |                    |
    |     returnUrl }        |                            |                    |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|                    |
    |                        |   paymentMethod.klarna {}  |--- POST /payments ->|
    |                        |   + billingAddress         |   (Klarna order)    |
    |                        |   CaptureMethod.AUTOMATIC  |                    |
    |<- requires_more -------|<-- redirectUrl ------------|<- hosted checkout --|
    |                        |                            |                    |
    |== Redirect to Mollie-hosted Klarna checkout ==========================>|
    |   customer completes Klarna                                              |
    |<== Redirect back to returnUrl ========================================|
    |                        |                            |                    |
    |-- place order (retry)->|-- authorizePayment (PSync)>|--- GET /payments ->|
    |<-- order --------------|<-- CAPTURED ---------------|<-- status: paid ----|
```

### Notes

- **EUR-only** — Klarna via Mollie is restricted to EU markets, so the storefront must create the payment session in **EUR**. (Card has no such restriction.)
- **Billing is required** — `authorizePayment` validates that `firstName`, `email`, and `line1` are present and returns `PaymentSessionStatus.ERROR` otherwise. The connector reads billing via `get_payment_method_billing()`, which falls back to the request `billing_address`, so the top-level `billingAddress` built by `toKlarnaBillingAddress` is sufficient. An unknown/unmappable country code is logged and omitted (Klarna's risk check needs a valid billing country).
- **No client token** — the Klarna arm sends `paymentMethod.klarna {}`; the connector builds the order line itself from the amount + description.
- **Idempotent retry** — identical to Card: once a `connectorTransactionId` exists, the retry PSyncs instead of re-authorizing.

## Session data

Produced by `initiatePayment`:

| Field | Description |
|-------|-------------|
| `profileId` | Public Mollie profile id (`pfl_…`) for the Mollie Components form |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

Added after re-initiation (Card):

| Field | Description |
|-------|-------------|
| `cardToken` | Single-use Mollie Components card token |
| `returnUrl` | Storefront URL Mollie redirects to after 3DS |

Added after re-initiation (Klarna):

| Field | Description |
|-------|-------------|
| `paymentMethodType` | `"klarna"` — routes `authorizePayment` to the Klarna arm |
| `billing` | `KlarnaBilling`: `firstName`, `lastName`, `email`, `line1`, `line2?`, `city`, `postalCode`, `country` (ISO 3166-1 alpha-2) |
| `returnUrl` | Storefront URL Mollie redirects to after the Klarna hosted checkout |

Added after `authorizePayment`:

| Field | Description |
|-------|-------------|
| `id`, `connectorTransactionId` | Mollie payment reference (`tr_…`) — drives the idempotent PSync on retry |
| `redirectUrl` | GET URL the storefront must send the customer to — Card: the 3DS page; Klarna: the Mollie-hosted Klarna checkout |
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
