# Mollie Connector

Server-side provider logic for **Mollie** via the Hyperswitch Prism connector service. Mollie is a redirect-based payment provider — the customer is redirected to Mollie's hosted checkout page to complete payment, then returned to the storefront. Authorization is confirmed synchronously after redirect.

## Files

| File | Role |
|------|------|
| `index.ts` | `initiatePayment` — creates a Mollie payment and returns the checkout URL; authorize/capture/refund/cancel go through generic UCS paths |

## Payment flow

```
Storefront              Medusa backend               UCS / Prism            Mollie
    |                        |                            |                    |
    |-- initiate session --->|-- CreatePayment ---------->|--- POST /payments->|
    |<- checkoutUrl ---------|<---------------------------|<-- checkout URL ----|
    |                        |                            |                    |
    |== Redirect to Mollie hosted checkout =================================>|
    |   customer selects method and pays                                       |
    |<== Redirect back to storefront (redirectUrl) =========================|
    |                        |                            |                    |
    |-- place order -------->|-- authorizePayment ------->|                    |
    |                        |   getPaymentStatus ------->|--- GET /payments ->|
    |<-- order --------------|<-- ✅ AUTHORIZED -----------|<-- paid -----------|
    |                        |   (manual capture required separately)          |
```

> **Result: AUTHORIZED** — `authorizePayment` delegates to `getPaymentStatus` via UCS. Mollie's `paid` status is mapped to `AUTHORIZED` by UCS, meaning funds are reserved. An explicit capture call is required to settle the payment.

### Step-by-step

1. **initiatePayment** — calls UCS to create a Mollie payment object. Returns `checkoutUrl` and `paymentId` in `payment_session.data`.
2. **Redirect (client)** — the storefront redirects the customer to the `checkoutUrl`. Mollie renders its hosted checkout (iDEAL, credit card, Klarna, etc.). On completion, Mollie redirects the customer back to the `redirectUrl` configured in the connector.
3. **authorizePayment** — calls UCS `getPaymentStatus` to retrieve the Mollie payment status. Returns `authorized` when status is `paid`.
4. **Refund / Cancel** — go through UCS synchronously.

## Session data produced by `initiatePayment`

| Field | Description |
|-------|-------------|
| `checkoutUrl` | Mollie-hosted checkout URL to redirect the customer to |
| `paymentId` | Mollie payment ID for status polling |
| `minorAmount`, `currency` | Amount in minor units and ISO currency code |

## Webhooks

Not required for core flow. Mollie can send status webhooks but UCS handles state via synchronous `getPaymentStatus` on redirect return.

## Credentials (`connectorConfig`)

| Field | Type | Description |
|-------|------|-------------|
| `apiKey` | `{ value: string }` | Mollie API key (`test_...` / `live_...`) |
