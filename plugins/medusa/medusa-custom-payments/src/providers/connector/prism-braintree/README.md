# Braintree Connector

Server-side provider logic for **Braintree** via the Hyperswitch Prism connector service. Braintree is a wallet-based connector — it accepts ApplePay, GooglePay, and PayPal payment methods through the Prism service. No client-side SDK wrapper is included in the React package.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` — returns a stub session; authorize/capture/refund/cancel go through generic UCS paths |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism           Braintree
    |                        |                            |                    |
    |-- initiate session --->|-- initiatePayment -------->| (stub session)     |
    |<- session.data --------|                            |                    |
    |                        |                            |                    |
    |== Wallet flow (ApplePay / GooglePay / PayPal) =========================>|
    |   client obtains payment nonce / token                                   |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|--- submit nonce -->|
    |                        |   getPaymentStatus <-------|<-- AUTHORIZED -----|
    |<-- order --------------|<-- ✅ AUTHORIZED -----------|                    |
    |                        |   (manual capture required separately)          |
```

> **Result: AUTHORIZED** — `authorizePayment` delegates to `getPaymentStatus` via UCS, which returns the Braintree transaction status. Braintree authorizes the wallet payment but does not capture immediately. An explicit capture call settles the payment.

### Step-by-step

1. **initiatePayment** — returns a minimal session object (no network call). The actual Braintree client token is expected to be obtained separately by the storefront if needed.
2. **Wallet flow (client)** — the storefront renders the appropriate wallet button (ApplePay, GooglePay, or PayPal Checkout). On approval, a payment nonce or token is returned client-side.
3. **authorizePayment** — forwards the nonce/token to UCS which charges the Braintree vault.
4. **Capture / Refund / Cancel** — all go through UCS synchronously.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `id` | Session identifier (merchant client session ID) |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

## Webhooks

Not configured. Payment state is driven by the synchronous `authorizePayment` response via UCS.

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `publicKey` | `{ value: string }` | Braintree public key |
| `privateKey` | `{ value: string }` | Braintree private key |
| `merchantAccountId` | `{ value: string }` | Braintree merchant account ID (optional) |
