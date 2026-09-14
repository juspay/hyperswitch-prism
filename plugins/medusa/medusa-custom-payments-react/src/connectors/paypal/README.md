# PayPal Connector

React wrapper around the **PayPal JS SDK** (`loadPayPalScript` in `utils.ts`). Renders PayPal Buttons: the shopper approves the order in the PayPal popup, but **nothing is charged client-side** — the capture happens server-side when the cart is completed.

## Components

| Piece | File | Role |
|---|---|---|
| `PayPalWrapper` | `PayPalWrapper.tsx` | Loads the SDK, renders Buttons, forwards `{orderId, payerId}` on approval |
| Panel dispatch | `../../components/HyperswitchPrismConnectorPanel.tsx` | Supplies `paypalClientId`/`paypalOrderId` from `session.data` for provider id `pp_hyperswitch-prism_*paypal` |
| Place Order button | `../../components/payment-buttons/PayPalPaymentButton.tsx` | Completes the cart — this is what triggers the actual capture |

## Payment flow (storefront ↔ test-project)

```
Storefront              Medusa (test-project)        UCS / Prism           PayPal
    |                          |                          |                    |
    |-- initiate session ----->|-- create order (REST) ------------------------>|
    |<- paypalClientId, -------|<-- orderId ------------------------------------|
    |   paypalOrderId          |                          |                    |
    |                          |                          |                    |
    |== PayPal Buttons → popup: customer approves order ======================>|
    |   onApprove({orderId, payerId} [+shipping/customer])                     |
    |                          |                          |                    |
    |-- Place order ---------->|-- capture (paypalSdk     |                    |
    |                          |   token, CaptureMethod   |                    |
    |                          |   .AUTOMATIC) ---------->|--- capture order ->|
    |<-- order ----------------|<-- ✅ CAPTURED -----------|<-- COMPLETED ------|
    |                          |   funds collected; Refund only available      |
```

> **Result: CAPTURED** — `CaptureMethod.AUTOMATIC` is passed to UCS, which captures the PayPal order immediately at cart completion. Funds are collected in one step. Only refunds are available after this point — there is no separate capture step.

1. Initiate: the provider plugin creates the PayPal order **server-side** during `initiatePayment` and returns `paypalClientId` + `paypalOrderId` in `payment_session.data`.
2. The wrapper's `onCreateOrder` simply resolves to that existing `paypalOrderId` (it must be a real PayPal order id).
3. The shopper approves in the popup; `onApprove` emits `{orderId, payerId}` — plus the shipping address / payer details when `includeShippingData` / `includeCustomerData` are enabled (these **must match** the provider's `connectorConfig` flags).
4. Place Order completes the cart; the plugin captures the approved order through UCS using the JS-SDK token (`paymentMethod: { paypalSdk: { token } }`).
5. Refunds go back through UCS; PayPal webhooks are also verified and processed by UCS (`PAYPAL_WEBHOOK_ID`).

## Session data consumed

| Wrapper prop | Source in Medusa `session.data` |
|---|---|
| `clientId` | `paypalClientId` |
| `onCreateOrder` result | `paypalOrderId` |
| `currency`, `amount` | `currency`, `minorAmount / 100` |

## Test-project setup

- Provider id: `pp_hyperswitch-prism_hyperswitch-prism-paypal` (config `id: hyperswitch-prism-paypal`, `connector: "paypal"`, `clientId`/`clientSecret` in `connectorConfig`).
- Optional webhook URL: `https://<host>/hooks/payment/hyperswitch-prism_hyperswitch-prism-paypal`.
