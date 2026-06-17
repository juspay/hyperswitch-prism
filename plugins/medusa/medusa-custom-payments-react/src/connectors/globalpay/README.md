# GlobalPay Connector

React wrapper around the **GlobalPayments JS SDK** (`loadGlobalPayScript` in `utils.ts`). Renders GlobalPay's hosted credit-card form, which only **tokenizes** the card client-side — the resulting `paymentReference` must be persisted into the Medusa session before the charge happens server-side at order placement.

## Components

| Piece | File | Role |
|---|---|---|
| `GlobalPayWrapper` | `GlobalPayWrapper.tsx` | Configures the SDK with the session's `accessToken`, mounts the hosted form, emits `paymentReference` on `token-success` |
| Panel dispatch | `../../components/HyperswitchPrismConnectorPanel.tsx` | For provider id `pp_hyperswitch-prism_*globalpay`: persists the reference via `onInitiateSession` (and `sessionStorage` as backup) |
| Place Order button | `../../components/payment-buttons/GlobalPayPaymentButton.tsx` | Completes the cart, triggering the server-side charge |

## Payment flow (storefront ↔ test-project)

```
Storefront              Medusa (test-project)        UCS / Prism          GlobalPay
    |                          |                          |                    |
    |-- initiate session ----->|-- createClientAuthToken ---------------------->|
    |<- accessToken -----------|<-- access token -------------------------------|
    |                          |                          |                    |
    |== Hosted card form: tokenize card ======================================>|
    |   token-success → paymentReference                                       |
    |-- re-initiate session -->|  persist paymentReference in session.data     |
    |   ("Card saved — click Place Order")                                     |
    |                          |                          |                    |
    |-- Place order ---------->|-- authorize (reference,  |                    |
    |                          |   state.accessToken,     |                    |
    |                          |   CaptureMethod.AUTO) -->|--- /transactions ->|
    |<-- order ----------------|<-- ✅ CAPTURED -----------|<-- CHARGED --------|
    |                          |   funds collected; Refund only available      |
```

> **Result: CAPTURED** — `CaptureMethod.AUTOMATIC` is passed to UCS, which charges the card immediately using the stored `paymentReference`. Funds are collected at cart completion. Only refunds are available after this point — there is no separate capture step.

1. Initiate: the provider plugin fetches a client access token via `createClientAuthenticationToken` and returns it as `accessToken` in `payment_session.data`.
2. The wrapper calls `GlobalPayments.configure({ accessToken })` and mounts the hosted card form. Submitting the form tokenizes the card — nothing is charged.
3. On `token-success` the wrapper calls `onSubmit({ paymentReference })`; the panel **re-initiates the Medusa payment session** so the reference is stored in `session.data` (and mirrored to `sessionStorage`). The form then shows "Card saved — click Place Order".
4. Place Order completes the cart; the plugin authorizes server-side, sending the stored `paymentReference` and a fresh server access token in `state.accessToken` on the Prism call (required — flows without it fail with `FAILED_TO_OBTAIN_AUTH_TYPE`).
5. Capture/refund flow through UCS. GlobalPay webhooks are not supported by UCS — state is driven by the synchronous flows.

## Session data consumed

| Wrapper prop | Source in Medusa `session.data` |
|---|---|
| `accessToken` | `accessToken` |
| persisted on re-initiate | `paymentReference` |

## Test-project setup

- Provider id: `pp_hyperswitch-prism_hyperswitch-prism-globalpay` (config `id: hyperswitch-prism-globalpay`, `connector: "globalpay"`, `appId`/`appKey` in `connectorConfig`).
- No webhook configuration needed.
- The access token must carry the `PMT_POST_Create` permission, or tokenization fails with `ACTION_NOT_AUTHORIZED`.
