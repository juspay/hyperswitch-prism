# GlobalPay Connector

Server-side provider logic for **GlobalPay (Heartland)** via the Hyperswitch Prism connector service. GlobalPay uses a **two-phase initiation**: a client access token is issued server-side, the hosted card form tokenizes the card client-side, and the resulting `paymentReference` is persisted back to the session before the server-side charge.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment`, `reInitiatePayment`, `authorizePayment`, `refundPayment`, `cancelPayment` |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism          GlobalPay
    |                        |                            |                    |
    |-- initiate session --->|-- createClientAuthToken -->|--- /authentications|
    |<- accessToken ---------|<---------------------------|<-- access_token ----|
    |                        |                            |                    |
    |== Hosted card form: tokenize card (client-side) ========================>|
    |   token-success → paymentReference                                       |
    |                        |                            |                    |
    |-- re-initiate session->|  reInitiatePayment:        |                    |
    |   { paymentReference } |  persist token in session.data                 |
    |   ("Card saved")       |                            |                    |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|                    |
    |                        |   CaptureMethod.AUTOMATIC  |--- /authentications|
    |                        |   charge with reference -->|--- /transactions ->|
    |<-- order --------------|<-- ✅ CAPTURED ------------|<-- CHARGED --------|
```

> **Result: CAPTURED** — `CaptureMethod.AUTOMATIC` is passed to UCS, which charges the card immediately using the stored `paymentReference`. Funds are collected at the time the cart is completed. Only refunds are available after this point (no separate capture step).

### Step-by-step

1. **initiatePayment** — calls `createClientAuthenticationToken` on UCS (PMT_POST_Create permission required). Returns `accessToken` in `payment_session.data`.
2. **Hosted form (client)** — the React `GlobalPayWrapper` configures the GlobalPayments SDK with the `accessToken` and mounts the hosted card form. On submission the card is **tokenized only** — nothing is charged.
3. **Re-initiation** — on `token-success` the storefront calls `initiatePayment` again with `{ paymentReference, id }` in `data`. The provider detects `data.paymentReference` and routes to `reInitiatePayment`, which stores the token in `session.data` without another network call.
4. **authorizePayment** — reads `paymentReference` from `session.data` (or `cart.metadata` as a fallback). Fetches a fresh **server** access token (authorization-permission scope), then calls UCS `authorizePayment` with the token reference. The server token is required — the client token from step 1 lacks authorization scope.
5. **Capture / Refund / Cancel** — all go through UCS synchronously. GlobalPay webhooks are not supported by UCS.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `accessToken` | Client access token for the hosted card form (tokenization scope only) |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

After re-initiation, `session.data` additionally contains:

| Field | Description |
|-------|-------------|
| `paymentReference` | Card token from GlobalPay tokenization |

## Webhooks

Not supported by UCS for GlobalPay. Payment state is driven by the synchronous `authorizePayment` response.

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `appId` | `{ value: string }` | GlobalPay app ID |
| `appKey` | `{ value: string }` | GlobalPay app key |
| `returnUrl` | `string` | URL GlobalPay redirects to after 3DS (if applicable) |

## Test card

| Number | Expiry | CVV |
|--------|--------|-----|
| `4263 9700 0000 5262` | `03/2030` | `737` |

> **Note**: the `accessToken` must carry the `PMT_POST_Create` permission. If tokenization fails with `ACTION_NOT_AUTHORIZED` (error code `40022`), contact GlobalPay support to enable this permission on your sandbox app.
