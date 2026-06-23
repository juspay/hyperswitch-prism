# Braintree Connector

Server-side provider logic for **Braintree** via the Hyperswitch Prism connector
service. Braintree here is **wallet-only** — it accepts **PayPal, Google Pay, and
Apple Pay** through the braintree-web SDK. Cards are not supported and the Rust
connector crates are not modified; the published SDK already exposes the wallet
arms. The React checkout UI is `<BraintreeWrapper>` in
`@juspay-tech/medusa-custom-payments-react`.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` — surfaces the Braintree `client_token` + per-wallet config; `authorizePayment` — charges the chosen wallet nonce (auto-capture); `shouldSkipVoid` |

## Payment flow (PayPal / Google Pay / Apple Pay)

```
Storefront                Medusa backend             UCS / Prism            Braintree
    |                          |                          |                     |
    |-- initiate session ----->|-- initiatePayment ------>|-- createClientAuth->|
    |                          |   (PAY_PAL arm)          |<- client_token -----|
    |<- {clientToken, ...} ----|                          |                     |
    |                          |                          |                     |
    |== braintree-web: client.create(clientToken) -> wallet button ===========>|
    |   buyer approves -> single-use Braintree NONCE                            |
    |                          |                          |                     |
    |-- persist nonce -------->|  (reinitiate short-circuit stores the nonce)   |
    |-- complete ------------->|-- authorizePayment ----->|-- CHARGE_* (sale) ->|
    |                          |                          |<- CHARGED ----------|
    |<-- ✅ CAPTURED ----------|<-- CAPTURED -------------|                     |
```

> **Result: CAPTURED** — `captureMethod: AUTOMATIC` runs a Braintree sale
> (`CHARGE_*` mutation), so a successful authorize settles immediately. No
> separate capture call is required.

### Step-by-step

1. **initiatePayment** — `PrismService` calls `createClientAuthenticationToken`
   (PAY_PAL arm) before dispatching here; this module reads the method-agnostic
   Braintree `client_token` from `sessionData.paypal.clientToken` and returns it
   plus the per-wallet config (`googlePay`, `applePay`) and `paypalClientId`.
2. **Wallet flow (client)** — `<BraintreeWrapper>` initialises braintree-web with
   the `client_token` and renders the eligible wallet buttons. On approval the
   wallet tokenizes to a single Braintree nonce.
3. **Persist** — the storefront persists `{ braintreeNonce, braintreeWalletType }`
   on the session via the reinitiate route (a `prism.ts` short-circuit stores it
   without re-running the hosted client-auth, which would drop the nonce).
4. **authorizePayment** — branches on `braintreeWalletType` to the correct arm
   (`paypalSdk` / `googlePayThirdPartySdk` / `applePayThirdPartySdk`) and charges
   with `captureMethod: AUTOMATIC` → CAPTURED.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `clientToken` | Braintree client token (~2.4kb base64) — drives all three wallets client-side |
| `paypalClientId` | PayPal client id echoed from the session arm (braintree-web reads PayPal config from the client token itself, so this is informational) |
| `googlePay`, `applePay` | Per-wallet config echoed from `connectorConfig` |
| `currency`, `minorAmount`, `environment` | Amount + environment |

## Webhooks

Not configured. With auto-capture the synchronous authorize response drives state.

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `publicKey` | `{ value: string }` | Braintree public key |
| `privateKey` | `{ value: string }` | Braintree private key |
| `merchantAccountId` | `{ value: string }` | Braintree **merchant account** ID (the settlement sub-account, e.g. `juspay`) — **not** the merchant ID. Required for client-token and charge. |
| `merchantConfigCurrency` | `string` | Settlement currency (e.g. `USD`) — required for authorize; must match the payment currency. |
| `paypalClientId` | `string` | Required (non-empty) for the PayPal session arm. |
| `gpay*`, `applePay*` | — | Per-wallet config for Google Pay / Apple Pay (see `creds.example.json`). |

> **Sandbox note:** completing a real PayPal charge requires the Braintree
> sandbox to have a **linked PayPal sandbox account** (not the default "offline"
> mock, which creates `FAILED` transactions). Google Pay / Apple Pay must be
> enabled in the Braintree control panel (Apple Pay additionally requires a
> validated merchant domain and Safari).
