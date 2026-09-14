# PayPal Connector

Server-side provider logic for **PayPal** via the Hyperswitch Prism connector service. The PayPal order is created server-side during `initiatePayment`; the customer approves it in the PayPal popup client-side; the actual **capture happens server-side** when the cart is completed.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment`, `authorizePayment`, `refundPayment`, `handleWebhook` |
| `../webhook-common.ts` | Shared webhook verification, outcome store, event mapping (used by adyen + paypal) |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism           PayPal
    |                        |                            |                    |
    |-- initiate session --->|-- create order (REST) ----->|--- /orders ------->|
    |<- paypalClientId, -----|<---------------------------|<-- orderId ---------|
    |   paypalOrderId        |                            |                    |
    |                        |                            |                    |
    |== PayPal Buttons → popup: customer approves order ======================>|
    |   onApprove({ orderId, payerId })                                        |
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|                    |
    |                        |   CaptureMethod.AUTOMATIC  |--- capture order ->|
    |<-- order --------------|<-- ✅ CAPTURED ------------|<-- CHARGED --------|
```

> **Result: CAPTURED** — `CaptureMethod.AUTOMATIC` is passed to UCS, which captures the PayPal order immediately. Funds are collected at the time the cart is completed. Only refunds are available after this point (no separate capture step).

### Step-by-step

1. **initiatePayment** — creates a PayPal order server-side via UCS. Returns `paypalClientId` and `paypalOrderId` in `payment_session.data`. The order already exists on PayPal at this point.
2. **PayPal Buttons (client)** — the React `PayPalWrapper` loads the JS SDK, calls `onCreateOrder` which resolves to the existing `paypalOrderId` (no new order is created). The customer approves in the popup.
3. **onApprove** — emits `{ orderId, payerId }`. If `includeShippingData` / `includeCustomerData` are enabled, the shipping address and payer details are also forwarded. These flags must match the provider's `connectorConfig`.
4. **authorizePayment** — called when the cart is completed. Captures the approved order through UCS using the JS-SDK token (`paymentMethod: { paypalSdk: { token } }`). Returns `authorized` (or `captured` if `capture: true`).
5. **Refunds** — go back through UCS. PayPal webhooks are also verified and processed.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `paypalClientId` | PayPal client ID for loading the JS SDK |
| `paypalOrderId` | The server-created PayPal order ID |
| `currency`, `minorAmount` | Currency and amount in minor units |

## Webhook (optional)

- **URL**: `{backend_url}/hooks/payment/{provider_id}`
- **Verification**: PayPal's `verify-webhook-signature` REST API using the connector credentials plus the webhook ID.
- **Setup**: create a webhook in the PayPal developer dashboard pointing at the URL above, then set the generated webhook ID as `webhookSecret` (env `PAYPAL_WEBHOOK_ID`).

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `clientId` | `{ value: string }` | PayPal client ID |
| `clientSecret` | `{ value: string }` | PayPal client secret |
| `shippingPreference` | `string` | `"NO_SHIPPING"` (default) or `"GET_FROM_FILE"` |
| `includeShippingData` | `boolean` | Forward shipping address from approved order |
| `includeCustomerData` | `boolean` | Forward payer details from approved order |
| `webhookSecret` | `string` | PayPal webhook ID (not wrapped in `{ value }`) |

## Test credentials

Use PayPal sandbox buyer credentials from developer.paypal.com → Sandbox → Accounts.

| Card | Expiry | CVV |
|------|--------|-----|
| `4032 0366 9170 5063` | `10/2028` | `901` |
