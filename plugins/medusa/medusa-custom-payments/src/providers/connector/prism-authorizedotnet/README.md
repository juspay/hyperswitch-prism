# Authorize.Net Connector

Server-side provider logic for **Authorize.Net** via the Hyperswitch Prism connector service. Authorize.Net is a **raw-card** connector here: the storefront collects the card in a plain in-page form (no tokenization, no Accept.js opaque token, no 3DS) and the server authorizes the PAN directly via UCS (`PaymentMethodData::Card`). Authorization is synchronous and funds are captured immediately, so the payment lands **CAPTURED** with no redirect.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` (synthetic pending session, no network call), `reInitiatePayment` (persists the entered card), `authorizePayment` (charges the raw card, `CaptureMethod.AUTOMATIC`), `refundPayment` (credits the card, with void fallback for unsettled txns), `shouldSkipVoid` |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism         Authorize.Net
    |                        |                            |                    |
    |-- initiate session --->|  initiatePayment           |                    |
    |<- pending session -----|  (synthetic, no network call)                   |
    |                        |                            |                    |
    |== raw card form: customer enters PAN / exp / CVV (in-page) ==            |
    |                        |                            |                    |
    |-- re-initiate session->|  reInitiatePayment:        |                    |
    |   { cardNumber, exp,   |  persist card in session.data (no network call) |
    |     cvc }              |                            |                    |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|--- authorize ----->|
    |                        |   paymentMethod.card       |   (raw PAN)        |
    |                        |   CaptureMethod.AUTOMATIC  |                    |
    |<-- order --------------|<-- ✅ CAPTURED -------------|<-- charged --------|
    |                        |                            |                    |
    |== navigate to /order/:id (no redirect) ==                               |
```

> **Result: CAPTURED** — `authorizePayment` charges with `CaptureMethod.AUTOMATIC`, so Authorize.Net collects funds immediately. The connector status maps `Charged` → `PaymentStatus.CHARGED` → Medusa `PaymentSessionStatus.CAPTURED`. Only refunds are available afterward (no separate capture step, no redirect).

### Step-by-step

1. **initiatePayment** — returns a synthetic `PENDING` session so the card form can render. No network call: Authorize.Net's session-token flow (`createClientAuthenticationToken`) is not used, so there is no SDK/tokenization step.
2. **Raw card form (client)** — the React `AuthorizedotnetWrapper` renders plain card fields. On "Pay" it hands the raw `{ cardNumber, cardExpMonth, cardExpYear, cardCvc }` to the host.
3. **Re-initiation** — the storefront calls `initiatePayment` again with the card in `data`. The provider detects `data.cardNumber` and routes to `reInitiatePayment`, which stores the card on `session.data` without a network call.
4. **authorizePayment** — charges the card via UCS with `paymentMethod.card` and `CaptureMethod.AUTOMATIC`. The connector requires a billing address, so the session billing is forwarded (falling back to test defaults). Adopts Authorize.Net's reference as `connectorTransactionId`. Returns `CAPTURED`.
5. **Refund / Cancel** — go through UCS synchronously. `refundPayment` credits the card; if the transaction is still unsettled (Authorize.Net errorCode 54), it **falls back to a void**. `shouldSkipVoid` skips the void when no real transaction exists yet (`data.connectorTransactionId` absent).

> **Sandbox note:** Authorize.Net can decline with errorCode 27 ("AVS mismatch") if the sandbox account's AVS reject settings are strict. Disable AVS reject in the sandbox account if approvals fail despite a matching billing address.

## Session data

Produced by `initiatePayment`:

| Field | Description |
|-------|-------------|
| `id` | Merchant session reference (also the merchant transaction id until a real txn exists) |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |
| `connector` | `"authorizedotnet"` |

Added after re-initiation:

| Field | Description |
|-------|-------------|
| `cardNumber`, `cardExpMonth`, `cardExpYear`, `cardCvc` | Raw card the buyer entered (forwarded to UCS at authorize time) |

Added after `authorizePayment`:

| Field | Description |
|-------|-------------|
| `connectorTransactionId` | Authorize.Net transaction reference — required for refund/void |
| `prismStatus` | Raw connector status (diagnostics) |

## Webhooks

Not supported by UCS for Authorize.Net. Payment state is driven by the synchronous `authorizePayment` response.

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `name` | `{ value: string }` | Authorize.Net **API Login ID** |
| `transactionKey` | `{ value: string }` | Authorize.Net **Transaction Key** |
| `baseUrl` | `string` | Optional endpoint override (e.g. `https://apitest.authorize.net/xml/v1/request.api` for sandbox) |

## Test card

| Number | Expiry | CVV |
|--------|--------|-----|
| `4111 1111 1111 1111` | `03 / 2030` | `123` |

> **Note**: Authorize.Net sandbox approves the Visa test card above for a matching billing address. If it declines with an AVS error, relax the AVS reject settings in the sandbox merchant account (see the sandbox note above).
