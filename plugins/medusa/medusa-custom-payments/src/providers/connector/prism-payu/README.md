# PayU Connector

Server-side provider logic for **PayU** via the Hyperswitch Prism connector service. PayU is an India-first processor: payments are **UPI / hosted-redirect** based (UPI Collect, UPI Intent, wallets, netbanking), so authorization completes **out-of-band** — the shopper approves in their UPI app or on the hosted PayU page and is redirected back.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` — surfaces the PayU SDK session token (OAuth2 access token) into `session.data`; `reInitiatePayment` — stores the chosen UPI VPA / method + billing; `authorizePayment` — builds the UPI/redirect payment and returns the next step as `data.redirectUrl`; `shouldSkipVoid` |

## Payment flow

```
Storefront                 Medusa backend              UCS / Prism            PayU
    |                          |                           |                    |
    |-- initiate session ----->|-- SDKSessionToken ------->|-- OAuth token ---->|
    |<- sessionToken ----------|<--------------------------|<-- access_token ---|
    |                          |                           |                    |
    |== shopper picks UPI, enters VPA ====================>|                    |
    |-- reinitiate {vpa, billing} -->| store on session    |                    |
    |-- place order ---------->|-- authorizePayment ------>|--- UPI Collect --->|
    |                          |   redirectionData <-------|<-- intent/redirect-|
    |<- redirectUrl -----------|                           |                    |
    |== shopper approves in UPI app / hosted page =========================>    |
    |-- return → cart.complete ->| authorizePayment(retry) → PSync ----------->|
    |<-- order ----------------|<-- status ----------------|                    |
```

> **Result: out-of-band.** UPI Collect pushes a request to the shopper's UPI app; UPI Intent / hosted methods return a `redirectUrl` the storefront follows. The post-redirect `cart.complete` retry status-syncs (PSync) the existing payment rather than re-authorizing (idempotent on `data.connectorTransactionId`).

### Step-by-step

1. **initiatePayment** — UCS runs PayU's SDKSessionToken (OAuth2 `client_credentials`) flow and returns `sessionData.connectorSpecific.payu.sessionToken`, surfaced as `data.sessionToken`.
2. **reInitiatePayment** — the storefront persists the chosen `vpa` / `paymentMethodType` + `billing` on the session (no network call).
3. **authorizePayment** — builds the SDK `authorize` request:
   - `upi_collect` → `paymentMethod.upiCollect.vpaId` (VPA pushed to the shopper's UPI app),
   - `upi_intent` → `paymentMethod.upiIntent` (returns a deep-link),
   - default → `paymentMethod.payuRedirect` (hosted wallet/netbanking page).
   PayU requires `firstName` + `email` billing, so a missing billing returns `ERROR`. The next step is surfaced as `data.redirectUrl` (from `redirectionData.uri` or a rebuilt form GET URL) and the real PayU reference is adopted as `data.id`.
4. **Capture / Refund / Cancel / PSync** — go through UCS synchronously.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `sessionToken` | PayU OAuth2 access token (when the SDKSessionToken flow returns one) |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

## Webhooks

Not wired in the plugin. Payment state is driven by the synchronous flows (`authorizePayment` → PSync).

## Credentials (`connectorConfig`)

PayU uses **BodyKey** auth.

| Field | Type | Description |
|-------|------|-------------|
| `apiKey` | `{ value: string }` | PayU merchant key |
| `apiSecret` | `{ value: string }` | PayU merchant salt |
| `baseUrl` | `string` (optional) | Override the PayU base URL |
| `returnUrl` | `string` (optional) | URL the shopper returns to after the redirect |

In `creds.json` (snake_case): `{ "payu": { "api_key": "MERCHANT_KEY", "api_secret": "MERCHANT_SALT", "return_url": "http://localhost:3000/return" } }`.

## Testing note

PayU's UPI/redirect authorization cannot be completed headlessly (it needs a real UPI-app approval or hosted-page interaction). The e2e coverage is therefore an API-level lifecycle check (`app/e2e/api/lifecycle.spec.ts`) asserting the initiate session shape, plus mocked unit tests (`__tests__/unit/payu.test.ts`) covering request-building and response-mapping. The SDK's native FFI library requires GLIBC ≥ 2.38, so live calls run only in the CI/e2e environment.
