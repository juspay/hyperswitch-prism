# Adyen Connector

Server-side provider logic for **Adyen** via the Hyperswitch Prism connector service. Adyen uses the **sessions flow**: the card is authorised client-side inside the Drop-in and the outcome reaches the backend **only via webhook** — there is no pollable status API for sessions payments.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment`, `authorizePayment`, `getPaymentStatus`, `handleWebhook`, `cancelPayment`, `refundPayment` |
| `../webhook-common.ts` | Shared webhook verification, outcome store, event mapping (used by adyen + paypal) |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism            Adyen
    |                        |                            |                    |
    |-- initiate session --->|-- CreateSessionToken ----->|--- /sessions ----->|
    |<- data.sessionData ----|<---------------------------|<-- id+sessionData -|
    |                        |                            |                    |
    |== Drop-in card form: customer pays (client-side authorization) =========>|
    |   resultCode: Authorised → onPaymentCompleted                            |
    |                        |                            |                    |
    |                        |<===== AUTHORISATION webhook ===================|
    |                        |-- HandleEvent (HMAC) ----->| verify + map       |
    |                        |   recordWebhookOutcome (pspReference)           |
    |-- place order -------->|                            |                    |
    |                        |-- authorizePayment ------->|                    |
    |                        |   polls outcome store up to ~12s               |
    |<-- order --------------|<-- ✅ AUTHORIZED + pspRef -|                    |
    |                        |   (manual capture required separately)          |
```

> **Result: AUTHORIZED** — the Adyen sessions flow authorises the card but does not capture funds. An explicit capture call (`POST /admin/payments/:id/capture`) is required to settle the payment.

### Step-by-step

1. **initiatePayment** — calls `CreateSessionToken` on UCS, which creates an Adyen checkout session. Returns `{ id, sessionData }` blob stored in `payment_session.data`.
2. **Drop-in (client)** — the React `AdyenWrapper` mounts Adyen Web v6 with that session. The customer pays entirely client-side.
3. **Webhook** — Adyen sends `AUTHORISATION` to `/hooks/payment/{provider_id}`. UCS verifies the HMAC signature; `handleWebhook` records the verified outcome (including `pspReference`) in the in-process outcome store.
4. **authorizePayment** — called when the storefront completes the cart. Polls the outcome store with up to 8 retries × 1.5 s to absorb webhook-delivery latency. Returns `authorized` with the `pspReference` when the outcome arrives.
5. **Capture / Refund / Cancel** — all go through UCS synchronously. Refund confirmations arrive as `REFUND` webhooks (acknowledged and logged; no Medusa action).

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `id` | Adyen checkout session ID |
| `sessionData.connectorSpecific.adyen.sessionId` | Adyen session ID (for the Drop-in) |
| `sessionData.connectorSpecific.adyen.sessionData.value` | Adyen session data blob |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

## Webhook

- **URL**: `{backend_url}/hooks/payment/{provider_id}` — Standard webhook in Adyen Customer Area.
- **Verification**: HMAC-SHA256. The hex HMAC key from Customer Area → Webhooks → Additional settings must be set as `webhookSecret` (env `ADYEN_WEBHOOK_SECRET`). Events without a valid signature are rejected.
- **Required**: webhooks are the **only** way Adyen signals authorization. Without them, `authorizePayment` always times out as `pending`.

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `apiKey` | `{ value: string }` | Adyen API key |
| `merchantAccount` | `{ value: string }` | Adyen merchant account name |
| `publishableKey` | `string \| { value: string }` | Adyen client key (`test_...` / `live_...`), optional. Not a secret — surfaced in the payment session as `publishableKey` for the storefront's drop-in. |
| `webhookSecret` | `string` | Hex HMAC key (not wrapped in `{ value }`) |

## Test card

| Number | Expiry | CVV |
|--------|--------|-----|
| `4111 1111 4555 1142` | `03/2030` | `737` |
