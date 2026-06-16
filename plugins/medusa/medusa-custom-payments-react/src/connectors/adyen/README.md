# Adyen Connector

React wrapper around **Adyen Web v6** (`@adyen/adyen-web`). Mounts the Adyen Card/Drop-in component using the sessions flow: the card is authorized client-side and the result reaches the Medusa backend **only via webhook** — Adyen has no pollable status API for sessions payments.

## Components

| Piece | File | Role |
|---|---|---|
| `AdyenWrapper` | `AdyenWrapper.tsx` | Loads Adyen Web, mounts the card form, reports authorization via `onAuthorized` |
| Panel dispatch | `../../components/HyperswitchPrismConnectorPanel.tsx` | Maps Medusa `session.data` → wrapper props for provider id `pp_hyperswitch-prism_*adyen` |
| Place Order button | `../../components/payment-buttons/AdyenPaymentButton.tsx` | Disabled until `isAuthorized` is true, then completes the cart |

## Payment flow (storefront ↔ test-project)

```
Storefront              Medusa (test-project)        UCS / Prism            Adyen
    |                          |                          |                    |
    |-- initiate session ----->|-- CreateSessionToken --->|--- /sessions ----->|
    |<- data.sessionData ------|<-------------------------|<-- id+sessionData -|
    |                          |                          |                    |
    |== Drop-in card form: customer pays (client-side authorization) =========>|
    |   onPaymentCompleted("Authorised") → isAuthorized=true                   |
    |                          |                          |                    |
    |                          |<===== webhook /hooks/payment/..._adyen =======|
    |                          |-- HandleEvent (HMAC) --->|  verify + map      |
    |                          |   record outcome + pspReference               |
    |-- Place order ---------->|  authorizePayment reads webhook outcome       |
    |<-- order ----------------|<-- ✅ AUTHORIZED ---------|  (polls up to ~10s)|
    |                          |   funds reserved; Capture or Void available   |
```

> **Result: AUTHORIZED** — Adyen sessions flow reserves funds but does not collect them. An explicit capture call (`POST /admin/payments/:id/capture`) is required to settle the payment.

1. Storefront initiates a payment session (`POST /store/payment-collections/:id/payment-sessions`). The provider plugin asks UCS for an Adyen checkout session; its `id` + `sessionData` blob come back in `payment_session.data`.
2. The panel mounts `AdyenWrapper` with that session — the customer pays directly against Adyen in the drop-in.
3. On `resultCode: Authorised` the wrapper fires `onAuthorized`, enabling the Place Order button. **Nothing is known server-side yet.**
4. Adyen sends the `AUTHORISATION` webhook to Medusa; UCS verifies the HMAC signature and the plugin records the verified outcome (including the `pspReference` needed for capture/refund).
5. Cart completion calls the plugin's `authorizePayment`, which reads that webhook outcome (retrying ~10s to absorb webhook latency and Medusa's 5s webhook processing delay) and authorizes the Medusa session.

## Session data consumed

| Wrapper prop | Source in Medusa `session.data` |
|---|---|
| `sessionData.session.id` | `sessionData.connectorSpecific.adyen.sessionId` |
| `sessionData.session.sessionData` | `sessionData.connectorSpecific.adyen.sessionData.value` |
| `sessionData.clientKey` | Storefront env (`NEXT_PUBLIC_ADYEN_CLIENT_KEY`) |
| `sessionData.minorAmount`, `currency` | `minorAmount`, `currency` |

## Test-project setup

- Provider id: `pp_hyperswitch-prism_hyperswitch-prism-adyen` (config `id: hyperswitch-prism-adyen`, `connector: "adyen"`).
- Webhook URL (Adyen Customer Area → Standard webhook): `https://<host>/hooks/payment/hyperswitch-prism_hyperswitch-prism-adyen` — no query params.
- `webhookSecret` option = the **hex HMAC key** from the Customer Area (`ADYEN_WEBHOOK_SECRET`). Webhooks are mandatory: without them payments can never be authorized.
