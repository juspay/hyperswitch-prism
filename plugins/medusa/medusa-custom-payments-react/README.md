# @juspay-tech/medusa-custom-payments-react

[![npm](https://img.shields.io/npm/v/@juspay-tech/medusa-custom-payments-react?logo=npm)](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments-react)

React UI components for Hyperswitch Prism payment connectors. Ships two layers of API:

- **High-level Medusa components** — drop-in components for a Medusa v2 Next.js storefront checkout
- **Low-level connector wrappers** — standalone React components for any custom checkout

## Connector Support Matrix

| Connector | Connector Panel | Payment Button | Authorize Result |
|-----------|:---------------:|:--------------:|:----------------:|
| `adyen` | ✅ | ✅ | AUTHORIZED |
| `paypal` | ✅ | ✅ | CAPTURED |
| `stripe` | — | ✅ | AUTHORIZED |
| `globalpay` | ✅ | ✅ | CAPTURED |
| `mollie` | ✅ | ✅ | CAPTURED (Card after 3DS; Klarna after redirect) |
| `braintree` | ○ | ○ | — |
| `cybersource` | ○ | ○ | — |

**Legend**

| Symbol | Meaning |
|--------|---------|
| ✅ | Supported — React component available |
| ○ | No React component — connector has no client-side UI (wallet / redirect / server-side only) |
| — | Not in `HyperswitchPrismConnectorPanel`; use `StripeWrapper` directly (Stripe handles its own Elements context) |

> **Authorize result**
> - **AUTHORIZED** (`adyen`, `stripe`) — funds reserved; Capture or Void available as a next step
> - **CAPTURED** (`paypal`, `globalpay`) — funds collected immediately at authorize time; only Refund available afterward

> **Note:** All UI flows and connector integrations in this matrix are tested and verified under the **sandbox / test environment** of each connector. Production behavior should be validated separately before go-live.

## Installation

```bash
npm install @juspay-tech/medusa-custom-payments-react
# peer dependencies
npm install @adyen/adyen-web @stripe/react-stripe-js @stripe/stripe-js
```

📦 [View on npm](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments-react)

**Local development** (linked to this repo):
```json
"@juspay-tech/medusa-custom-payments-react": "file:/path/to/medusa-hyperswitch-prism/medusa-custom-payments-react"
```

After source changes, rebuild with:
```bash
cd medusa-custom-payments-react && npm run build
# then yarn install in your storefront
```

## Environment Variables

Add the following to your storefront `.env.local`:

```env
# Adyen — client key from your Adyen Customer Area (required for Adyen drop-in)
NEXT_PUBLIC_ADYEN_CLIENT_KEY=test_...
```

## Exports

| Export | Type | Description |
|--------|------|-------------|
| `HyperswitchPrismConnectorPanel` | Component | Renders the correct connector UI (Adyen/PayPal/GlobalPay/Mollie) for a selected payment method |
| `HyperswitchPrismPaymentButton` | Component | Auto-dispatches to the correct place-order button based on `providerId` |
| `MollieReturnHandler` | Component | Finalises the order on the Mollie 3DS return route (retry-polls the host's place-order action) |
| `AdyenWrapper` | Component | Low-level Adyen Web v6 drop-in wrapper |
| `PayPalWrapper` | Component | Low-level PayPal Buttons SDK wrapper |
| `GlobalPayWrapper` | Component | Low-level GlobalPay hosted card fields wrapper |
| `StripeWrapper` | Component | Low-level Stripe Payment Element wrapper |
| `MollieWrapper` | Component | Low-level Mollie Components (in-page card tokenization) wrapper |
| `MollieKlarnaForm` | Component | Klarna (Pay later) billing form for the Mollie redirect flow — collects name/email/postal address (Netherlands test defaults, all fields editable) and submits a `MollieKlarnaBilling`. Pair with a EUR session (Klarna via Mollie is EU-only) |
| `MollieKlarnaBilling` | Type | Shape of the Klarna billing the form collects: `firstName`, `lastName`, `email`, `line1`, `city`, `postalCode`, `country` (ISO 3166-1 alpha-2) |
| `StripePaymentButton` | Component | Place-order button for Stripe |
| `AdyenPaymentButton` | Component | Place-order button for Adyen |
| `PayPalPaymentButton` | Component | Place-order button for PayPal |
| `GlobalPayPaymentButton` | Component | Place-order button for GlobalPay |
| `MolliePaymentButton` | Component | Place-order button for Mollie (handles the 3DS redirect) |
| `ManualTestPaymentButton` | Component | Dev-only button for manual/test payment providers |
| `isHyperswitchPrism` | Utility | Returns `true` for any Hyperswitch Prism provider ID |
| `isHyperswitchPrismStripe` | Utility | Matches Stripe provider IDs (short and legacy forms) |
| `isHyperswitchPrismAdyen` | Utility | Matches Adyen provider IDs |
| `isHyperswitchPrismPaypal` | Utility | Matches PayPal provider IDs |
| `isHyperswitchPrismGlobalpay` | Utility | Matches GlobalPay provider IDs |
| `isHyperswitchPrismMollie` | Utility | Matches Mollie provider IDs |
| `isHyperswitchPrismPanel` | Utility | Matches every Prism connector with a client panel (adyen/paypal/globalpay/mollie — all except Stripe) |
| `HYPERSWITCH_PRISM_PROVIDER_IDS` | Constant | Map of connector name → canonical provider ID |

> The predicates are also exported from the **server-safe** subpath `@juspay-tech/medusa-custom-payments-react/predicates` — import them from there in Next.js server components.

---

## High-level Medusa Components

### `HyperswitchPrismConnectorPanel`

Renders the correct payment instrument UI for the selected Hyperswitch Prism provider. Returns `null` for Stripe (handled separately via `@stripe/react-stripe-js`) and unknown providers.

```tsx
import { HyperswitchPrismConnectorPanel } from "@juspay-tech/medusa-custom-payments-react"

<HyperswitchPrismConnectorPanel
  providerId={paymentMethod.id}
  sessionData={sessionForProvider(paymentMethod.id)?.data}
  adyenClientKey={process.env.NEXT_PUBLIC_ADYEN_CLIENT_KEY}
  onInitiateSession={async ({ paymentReference, id }) => {
    // GlobalPay: re-initiate to persist the tokenized card reference
    await initiatePaymentSession(cart, {
      provider_id: paymentMethod.id,
      data: { paymentReference, id },
    })
  }}
  onPaymentCompleted={() => router.push("...?step=review")}
  onError={(e) => setError(e.message)}
/>
```

| Prop | Type | Required | Description |
|------|------|----------|-------------|
| `providerId` | `string` | Yes | Provider ID of the selected method (e.g. `pp_hyperswitch-prism_adyen`) |
| `sessionData` | `Record<string, any> \| null` | Yes | The `.data` field from the active Medusa payment session |
| `adyenClientKey` | `string` | Yes (Adyen) | Adyen client key from `process.env.NEXT_PUBLIC_ADYEN_CLIENT_KEY` — required when Adyen is enabled |
| `onInitiateSession` | `(data: { paymentReference, id }) => Promise<void>` | Yes | Called by GlobalPay after tokenization to persist the payment reference |
| `onPaymentCompleted` | `(result?: any) => void` | No | Called after Adyen authorises or PayPal approves |
| `onError` | `(error: Error) => void` | Yes | Called on any connector-level error |
| `environment` | `"sandbox" \| "production"` | No | Defaults to `"sandbox"` |

### `HyperswitchPrismPaymentButton`

Auto-dispatches to the correct connector place-order button. Returns `null` for non-Hyperswitch Prism providers.

```tsx
import { HyperswitchPrismPaymentButton } from "@juspay-tech/medusa-custom-payments-react"

<HyperswitchPrismPaymentButton
  providerId={paymentSession?.provider_id}
  cart={cart}
  notReady={notReady}
  onPlaceOrder={placeOrder}
  buttonComponent={Button}
  data-testid="submit-order-button"
/>
```

| Prop | Type | Required | Description |
|------|------|----------|-------------|
| `providerId` | `string \| undefined` | Yes | Active payment session provider ID |
| `cart` | `StoreCart` | Yes | Medusa cart |
| `notReady` | `boolean` | Yes | Disables the button when checkout prerequisites are incomplete |
| `onPlaceOrder` | `() => Promise<void>` | Yes | Called to place the order |
| `buttonComponent` | `React.ComponentType` | No | Custom button component |
| `data-testid` | `string` | No | Test identifier forwarded to the rendered button |

### Storefront predicates

Import the predicates from the **server-safe subpath** `@juspay-tech/medusa-custom-payments-react/predicates` (pure, no `"use client"`), which is safe to use in Next.js **server** components. Do **not** import them from the package root in a server component — the root barrel pulls in client components and triggers a `createContext` error.

```ts
// safe in server OR client components
import {
  isHyperswitchPrism,
  isHyperswitchPrismStripe,
  isHyperswitchPrismMollie,
  isHyperswitchPrismPanel, // adyen | paypal | globalpay | mollie (everything except stripe)
} from "@juspay-tech/medusa-custom-payments-react/predicates"
```

`isHyperswitchPrismPanel` is the union to drive both `HyperswitchPrismConnectorPanel` and `HyperswitchPrismPaymentButton` (all Prism connectors that have a client panel — i.e. all except Stripe, which uses your own Stripe Elements). The predicates match both canonical short IDs (`pp_hyperswitch-prism_stripe`) and legacy long IDs for backward compatibility.

### Mollie (3DS) — panel, button, and return handler

Mollie cards complete via a 3DS redirect, so three pieces work together:

```tsx
// 1) Panel renders Mollie Components and persists the tokenized card + return URL
<HyperswitchPrismConnectorPanel
  providerId={providerId}
  sessionData={session?.data}
  mollieReturnUrl={`${window.location.origin}/[cc]/checkout/mollie-return`}
  onInitiateSession={(data) => initiatePaymentSession(cart, { provider_id, data })}
  onPaymentCompleted={() => router.push("...?step=review")}
  onError={(e) => setError(e.message)}
/>

// 2) Button places the order; on `requires_more` it follows Mollie's 3DS redirect
<HyperswitchPrismPaymentButton
  providerId={providerId} cart={cart} notReady={notReady}
  onPlaceOrder={placeOrder} buttonComponent={Button}
  backendUrl={process.env.NEXT_PUBLIC_MEDUSA_BACKEND_URL}
  publishableKey={process.env.NEXT_PUBLIC_MEDUSA_PUBLISHABLE_KEY}
/>

// 3) The route Mollie returns to (e.g. app/[cc]/checkout/mollie-return/page.tsx)
//    finalises the order by polling the place-order action until it succeeds.
<MollieReturnHandler
  onFinalize={async () => { await placeOrder() }}
  backHref="/checkout?step=payment"
  linkComponent={LocalizedClientLink}
/>
```

> **Note:** `onInitiateSession` is `(data: Record<string, unknown>) => Promise<void>` (≥ 0.0.5) — GlobalPay passes `{ paymentReference, id }`, Mollie passes `{ ...sessionData, cardToken, returnUrl }`; forward `data` straight to `initiatePaymentSession`.

---

## Low-level Connector Wrappers

Use these when building a custom (non-Medusa) checkout or embedding an individual payment form.

### `AdyenWrapper`

Renders the Adyen Web v6 drop-in.

```tsx
import { AdyenWrapper } from "@juspay-tech/medusa-custom-payments-react"

<AdyenWrapper
  sessionData={{
    clientKey: "test_xxx",
    session: { id: "CS...", sessionData: "Ab02b4c..." },
    currency: "EUR",
    minorAmount: 1000,
    countryCode: "GB",
  }}
  onPaymentCompleted={(result) => console.log("Authorised", result)}
  onPaymentFailed={(result) => console.error("Failed", result)}
  onError={(e) => console.error(e)}
/>
```

| Prop | Type | Required | Description |
|------|------|----------|-------------|
| `sessionData` | `Record<string, any>` | Yes | `clientKey`, `session.id`, `session.sessionData`, `currency`, `minorAmount`, `countryCode` |
| `onPaymentCompleted` | `(result: any) => void` | No | Called on `resultCode: "Authorised"` / `"Pending"` / `"Received"` |
| `onPaymentFailed` | `(result: any) => void` | No | Called on non-authorised result codes |
| `onSubmit` | `(paymentData: unknown) => void` | No | Advanced flow only |
| `onError` | `(error: Error) => void` | Yes | Called on SDK or network errors |

### `PayPalWrapper`

Renders the PayPal Buttons SDK.

```tsx
import { PayPalWrapper } from "@juspay-tech/medusa-custom-payments-react"

<PayPalWrapper
  clientId="AYour_PayPal_Client_Id"
  currency="USD"
  amount={49.99}
  environment="sandbox"
  onCreateOrder={() => Promise.resolve(sessionData.paypalOrderId)}
  onSubmit={({ orderId, payerId }) => advanceToReview()}
  onError={(e) => setError(e.message)}
/>
```

| Prop | Type | Required | Description |
|------|------|----------|-------------|
| `clientId` | `string` | Yes | PayPal app client ID |
| `currency` | `string` | Yes | ISO 4217 currency code |
| `amount` | `number` | Yes | Amount in major units |
| `environment` | `"sandbox" \| "production"` | No | Defaults to `"production"` |
| `onCreateOrder` | `() => Promise<string>` | Yes | Must resolve to a PayPal order ID |
| `onSubmit` | `(data: { orderId, payerId }) => void` | Yes | Called after PayPal approval |
| `onError` | `(error: Error) => void` | Yes | Called on SDK or approval errors |

### `GlobalPayWrapper`

Renders the GlobalPay hosted card fields. After tokenization, `onSubmit` is called with the `paymentReference` that must be persisted before place order.

```tsx
import { GlobalPayWrapper } from "@juspay-tech/medusa-custom-payments-react"

<GlobalPayWrapper
  accessToken={sessionData.accessToken}
  environment="sandbox"
  onSubmit={async ({ paymentReference }) => {
    await initiatePaymentSession(cart, {
      provider_id: providerId,
      data: { paymentReference },
    })
  }}
  onError={(e) => setError(e.message)}
/>
```

| Prop | Type | Required | Description |
|------|------|----------|-------------|
| `accessToken` | `string` | Yes | GlobalPay access token from the payment session |
| `environment` | `"sandbox" \| "production"` | No | Defaults to `"sandbox"` |
| `onSubmit` | `(data: { paymentReference }) => Promise<void>` | Yes | Called after successful card tokenization |
| `onError` | `(error: Error) => void` | Yes | Called on tokenization or SDK errors |

### `StripeWrapper`

Renders the Stripe Payment Element (standalone — does not require an `<Elements>` context).

```tsx
import { StripeWrapper } from "@juspay-tech/medusa-custom-payments-react"

<StripeWrapper
  publishableKey="pk_test_..."
  clientSecret={sessionData.client_secret}
  onSubmit={(paymentIntent) => advanceToReview()}
  onError={(e) => setError(e.message)}
/>
```

| Prop | Type | Required | Description |
|------|------|----------|-------------|
| `publishableKey` | `string` | Yes | Stripe publishable key |
| `clientSecret` | `string` | Yes | PaymentIntent client secret from the session |
| `onSubmit` | `(paymentData: unknown) => void` | Yes | Called after `confirmPayment` succeeds |
| `onError` | `(error: Error) => void` | Yes | Called on Stripe errors |

---

## Backend Plugin

For the Medusa backend plugin, see [`@juspay-tech/medusa-custom-payments`](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments).

## License

Apache-2.0
