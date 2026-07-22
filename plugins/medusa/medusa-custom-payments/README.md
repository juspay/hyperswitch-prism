# @juspay-tech/medusa-custom-payments

[![npm](https://img.shields.io/npm/v/@juspay-tech/medusa-custom-payments?logo=npm)](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments)

Custom payments provider plugin for Medusa v2. Enables payment processing through multiple connectors — Stripe, Adyen, PayPal, GlobalPay, Braintree, Cybersource, Mollie, and Authorize.Net — via a single Prism-powered provider.

Powered by [Hyperswitch Prism](https://github.com/juspay/hyperswitch-prism), an open-source unified connector service (UCS) that abstracts connector-specific APIs behind a single interface.

## Compatibility

| Requirement | Version |
|-------------|---------|
| Medusa | `>= 2.15.0` |
| Node.js | `>= 20` |
| Framework | Medusa v2 |

## Installation

```bash
npm install @juspay-tech/medusa-custom-payments
```

📦 [View on npm](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments)

## Setup

### 1. Environment variables

```env
# Stripe
STRIPE_API_KEY=sk_test_...

# Adyen
ADYEN_API_KEY=AQE...
ADYEN_MERCHANT_ACCOUNT=YourMerchantAccount
ADYEN_WEBHOOK_SECRET=...   # hex HMAC key — required for webhook processing

# PayPal
PAYPAL_CLIENT_ID=...
PAYPAL_CLIENT_SECRET=...
PAYPAL_WEBHOOK_ID=...       # webhook ID — required for webhook processing

# GlobalPay
GLOBALPAY_APP_ID=...
GLOBALPAY_APP_KEY=...

# Mollie
MOLLIE_API_KEY=test_...          # Mollie API key (test_… / live_…)
MOLLIE_PROFILE_TOKEN=pfl_...     # public profile id; surfaced to the storefront for Mollie Components

# Authorize.Net
AUTHORIZEDOTNET_NAME=...              # API Login ID
AUTHORIZEDOTNET_TRANSACTION_KEY=...   # Transaction Key
AUTHORIZEDOTNET_BASE_URL=https://apitest.authorize.net/xml/v1/request.api   # optional endpoint override
```

### 2. Register providers in `medusa-config.ts`

```ts
import { defineConfig, Modules } from "@medusajs/framework/utils"

export default defineConfig({
  modules: [
    {
      key: Modules.PAYMENT,
      resolve: "@medusajs/payment",
      options: {
        providers: [
          {
            resolve: "@juspay-tech/medusa-custom-payments",
            id: "stripe",
            options: {
              connector: "stripe",
              connectorConfig: {
                apiKey: { value: process.env.STRIPE_API_KEY ?? "" },
                // Publishable key (not a secret) — surfaced in the payment
                // session as `publishableKey` for the storefront's Elements.
                publishableKey: process.env.STRIPE_PUBLISHABLE_KEY ?? "",
              },
              environment: "SANDBOX",
            },
          },
          {
            resolve: "@juspay-tech/medusa-custom-payments",
            id: "adyen",
            options: {
              connector: "adyen",
              connectorConfig: {
                apiKey: { value: process.env.ADYEN_API_KEY ?? "" },
                merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT ?? "" },
                // Client key (not a secret) — surfaced in the payment session as
                // `publishableKey` for the storefront's Adyen drop-in.
                publishableKey: process.env.ADYEN_CLIENT_KEY ?? "",
              },
              webhookSecret: process.env.ADYEN_WEBHOOK_SECRET, // required for webhook processing
              environment: "SANDBOX",
            },
          },
          {
            resolve: "@juspay-tech/medusa-custom-payments",
            id: "paypal",
            options: {
              connector: "paypal",
              connectorConfig: {
                clientId: { value: process.env.PAYPAL_CLIENT_ID ?? "" },
                clientSecret: { value: process.env.PAYPAL_CLIENT_SECRET ?? "" },
                // "NO_SHIPPING" (default) — hides address collection in the PayPal popup
                // "GET_FROM_FILE" — uses the shipping address already collected by the storefront
                shippingPreference: "NO_SHIPPING",
              },
              webhookSecret: process.env.PAYPAL_WEBHOOK_ID, // required for webhook processing
              environment: "SANDBOX",
            },
          },
          {
            resolve: "@juspay-tech/medusa-custom-payments",
            id: "globalpay",
            options: {
              connector: "globalpay",
              connectorConfig: {
                appId: { value: process.env.GLOBALPAY_APP_ID ?? "" },
                appKey: { value: process.env.GLOBALPAY_APP_KEY ?? "" },
              },
              environment: "SANDBOX",
            },
          },
          {
            resolve: "@juspay-tech/medusa-custom-payments",
            id: "mollie",
            options: {
              connector: "mollie",
              connectorConfig: {
                apiKey: { value: process.env.MOLLIE_API_KEY ?? "" },
                // Public profile id (pfl_…), not a secret — surfaced in the
                // payment session as `profileId` for the storefront's Mollie
                // Components. The same credentials serve Card and Klarna.
                profileToken: { value: process.env.MOLLIE_PROFILE_TOKEN ?? "" },
              },
              environment: "SANDBOX",
            },
          },
          {
            resolve: "@juspay-tech/medusa-custom-payments",
            id: "authorizedotnet",
            options: {
              connector: "authorizedotnet",
              connectorConfig: {
                // API Login ID + Transaction Key (both server-side secrets).
                name: { value: process.env.AUTHORIZEDOTNET_NAME ?? "" },
                transactionKey: { value: process.env.AUTHORIZEDOTNET_TRANSACTION_KEY ?? "" },
                // Optional endpoint override (defaults to the connector's environment).
                baseUrl: process.env.AUTHORIZEDOTNET_BASE_URL,
              },
              environment: "SANDBOX",
            },
          },
        ],
      },
    },
  ],
})
```

### 3. Assign providers to regions

In the Medusa Admin, assign each provider to the regions where it should be available.

### 4. Switch to production

Change `environment` per provider when going live:

```ts
environment: process.env.NODE_ENV === "production" ? "PRODUCTION" : "SANDBOX",
```

### 5. Start the backend

```bash
npx medusa develop
```

## Plugin Options

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `connector` | `string` | Yes | — | `"stripe"`, `"adyen"`, `"paypal"`, `"globalpay"`, `"braintree"`, `"cybersource"`, `"mollie"`, `"authorizedotnet"` |
| `connectorConfig` | `object` | Yes | — | Connector-specific credentials (see examples above) |
| `environment` | `string` | No | `"SANDBOX"` | `"SANDBOX"` or `"PRODUCTION"` |
| `capture` | `boolean` | No | `false` | Auto-capture on authorization |

## Connector Credentials Reference

| Connector | Required fields in `connectorConfig` |
|-----------|--------------------------------------|
| `stripe` | `apiKey`, `publishableKey` (optional — surfaced to the storefront) |
| `adyen` | `apiKey`, `merchantAccount`, `publishableKey` (optional Adyen client key — surfaced to the storefront) |
| `paypal` | `clientId`, `clientSecret`, `shippingPreference` (optional) |
| `globalpay` | `appId`, `appKey` |
| `braintree` | `publicKey`, `privateKey` |
| `cybersource` | `apiKey`, `merchantAccount`, `apiSecret` |
| `mollie` | `apiKey`, `profileToken` (public profile id `pfl_…` — surfaced to the storefront for Mollie Components) |
| `authorizedotnet` | `name` (API Login ID), `transactionKey` (Transaction Key); `baseUrl` (optional endpoint override) |

Each credential value is provided as `{ value: string }` to support secret manager integrations.

## Connector Support Matrix

| Connector | Authorize | Capture | Void | Refund | Webhook |
|-----------|:---------:|:-------:|:----:|:------:|:-------:|
| `adyen` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `paypal` | — | ✅ | ✅ | ✅ | ✅ |
| `stripe` | ✅ | ✅ | ✅ | ✅ | ○ |
| `globalpay` | — | ✅ | ✅ | ✅ | ○ |
| `braintree` | ✅ | ✅ | ✅ | ✅ | ○ |
| `cybersource` | ✅ | ✅ | ✅ | ✅ | ○ |
| `mollie` | — | ✅ | ✅ | ✅ | ○ |
| `authorizedotnet` | — | ○ | ○ | ○ | ○ |

**Legend**

| Symbol | Meaning |
|--------|---------|
| ✅ | Supported |
| ○ | Not supported — state driven by synchronous flows |
| — | Not a separate step: connector captures funds immediately at authorize time (`CaptureMethod.AUTOMATIC`); payment goes straight to **CAPTURED** so only Refund is available afterward |

> **Authorize result by connector**
> - `adyen`, `stripe`, `braintree`, `cybersource` → payment lands as **AUTHORIZED** (funds reserved; Capture or Void available next)
> - `paypal`, `globalpay`, `mollie`, `authorizedotnet` → payment lands as **CAPTURED** (funds collected immediately via `CaptureMethod.AUTOMATIC`; Refund only). For `mollie` this is reached after the customer completes the redirect — the 3DS page for Card, or the Mollie-hosted Klarna checkout for Klarna (Pay later, EUR-only); `authorizedotnet` authorizes a raw card with no redirect.

> **Note:** All flows in the matrix above are tested and verified under the **sandbox / test environment** of each connector. Production behavior should be validated separately before go-live.

## Webhooks

All webhook processing goes through the hyperswitch-prism connector service (`EventService.HandleEvent`) — this provider contains no connector-specific webhook parsing or signature code.

Webhook URL pattern:

- Medusa: `{backend_url}/hooks/payment/{provider_id}`

| Connector | Webhook events | Source verification | `webhookSecret` value |
|-----------|---------------|--------------------|-----------------------|
| `adyen` | payment + refund | HMAC-SHA256 | Hex HMAC key from Customer Area → Webhooks → Additional settings (**required**) |
| `paypal` | payment + refund + dispute | PayPal `verify-webhook-signature` API | Webhook ID from the developer dashboard (**required**) |
| `stripe`, `globalpay`, `braintree`, `cybersource`, `mollie`, `authorizedotnet` | not supported | — | unused |

For unsupported connectors, incoming webhooks are acknowledged and ignored (`NOT_SUPPORTED`); payment state is driven by the synchronous flows (`authorizePayment` / `getPaymentStatus`).

### Verification is mandatory

Webhook events that cannot be source-verified are **always rejected** — in development and production alike. There is no bypass option:

- Without a configured `webhookSecret`, adyen/paypal webhooks are rejected before reaching the connector service.
- An event whose signature fails verification (wrong key, tampered payload, forged request) is acknowledged but never mapped to a payment action.

### Setup

**Adyen** — in the Customer Area create a Standard webhook pointing at your webhook URL, generate an HMAC key under Additional settings, and configure it (hex string, exactly as displayed) as `webhookSecret`. Set `ADYEN_WEBHOOK_SECRET` in your environment.

**PayPal** — in the developer dashboard create a webhook for your app pointing at your webhook URL, then configure the generated **webhook ID** as `webhookSecret`. Set `PAYPAL_WEBHOOK_ID` in your environment.

### Configuration example

The webhook URL is derived from the provider `id` you set in `medusa-config.ts`. With `identifier = "hyperswitch-prism"`, a provider registered as `id: "adyen"` is reachable at:

```
{backend_url}/hooks/payment/hyperswitch-prism_adyen
```

**1. Add the secrets to your environment**

```env
# Adyen — hex HMAC key from Customer Area → Developers → Webhooks → Additional settings
ADYEN_WEBHOOK_SECRET=AB12CD34EF...
# PayPal — webhook ID from the developer dashboard
PAYPAL_WEBHOOK_ID=WH-1AB23456CD789...
```

**2. Pass `webhookSecret` in the provider options** (`medusa-config.ts`)

```ts
{
  resolve: "@juspay-tech/medusa-custom-payments",
  id: "adyen",
  options: {
    connector: "adyen",
    connectorConfig: {
      apiKey: { value: process.env.ADYEN_API_KEY ?? "" },
      merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT ?? "" },
    },
    webhookSecret: process.env.ADYEN_WEBHOOK_SECRET, // required for webhook processing
    environment: "SANDBOX",
  },
},
{
  resolve: "@juspay-tech/medusa-custom-payments",
  id: "paypal",
  options: {
    connector: "paypal",
    connectorConfig: {
      clientId: { value: process.env.PAYPAL_CLIENT_ID ?? "" },
      clientSecret: { value: process.env.PAYPAL_CLIENT_SECRET ?? "" },
    },
    webhookSecret: process.env.PAYPAL_WEBHOOK_ID, // required for webhook processing
    environment: "SANDBOX",
  },
},
```

**3. Register the URL at the connector dashboard**

| Connector | Provider `id` | Webhook URL to register |
|-----------|---------------|-------------------------|
| Adyen | `adyen` | `{backend_url}/hooks/payment/hyperswitch-prism_adyen` |
| PayPal | `paypal` | `{backend_url}/hooks/payment/hyperswitch-prism_paypal` |

> **Local development** — connector dashboards must reach your backend over the public internet. Expose your local server with a tunnel (e.g. `ngrok http 9000`) and register the tunnel URL, for example `https://abcd-1234.ngrok.io/hooks/payment/hyperswitch-prism_adyen`.

The connector is taken from each provider's `connector`

Note: Adyen refunds are asynchronous — the REFUND webhook is the settlement confirmation and is logged by the provider (Medusa has no refund webhook action, so it is acknowledged as `NOT_SUPPORTED`).

## Test Cards

### Stripe

| Card Number | Expiry | CVV |
|-------------|--------|-----|
| `4242 4242 4242 4242` | `03/2030` | `737` |

### Adyen

| Card Number | Expiry | CVV |
|-------------|--------|-----|
| `4111 1111 4555 1142` | `03/2030` | `737` |

### GlobalPay

| Card Number | Expiry | CVV |
|-------------|--------|-----|
| `4263 9700 0000 5262` | `03/2030` | `737` |

### PayPal

| Card Number | Expiry | CVV |
|-------------|--------|-----|
| `4032 0366 9170 5063` | `10/2028` | `901` |

### Mollie

| Card Number | Expiry | CVV |
|-------------|--------|-----|
| `4111 1111 1111 1111` | `12/2030` | `123` |

> In Mollie **test mode** you then pick the outcome (Paid / Failed / …) on Mollie's hosted test screen. Choose **Paid** to drive the payment to `CAPTURED`.

## Storefront Integration

For React/Next.js storefront integration, see the companion package [`@juspay-tech/medusa-custom-payments-react`](https://www.npmjs.com/package/@juspay-tech/medusa-custom-payments-react).

## License

Apache-2.0
