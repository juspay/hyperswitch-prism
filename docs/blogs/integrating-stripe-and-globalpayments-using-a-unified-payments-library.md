# Integrating Stripe & Global Payments using a Unified Payments Integration library

Most businesses integrate Stripe directly into the codebase, treating it as the unified payments layer. When the need for a second processor arises (due to new market expansion, or frozen payouts, or local compliance), it would be too late to discover that Stripe is wired through every call site creating a vendor lock-in at code level. While other layers of the stack would generally have vendor-neutral interfaces (JDBC, OpenTelemetry, Keycloak, LiteLLM), payments generally doesn't.

This is a guide for implementing a unified payment integration that can point to multiple payment processors.


## Use case for multiple payment processor integrations

A business accepting cards in both the US and the EU usually faces the same shape of problem: Stripe is the natural choice for USD acceptance, Global Payments is a strong choice for EUR acceptance, and the business ends up shipping two integrations within the same codebase - one wrapping Stripe Payment Intents, another wrapping GP-API. The impact will be: every new operation (capture, void, recurring charge) gets implemented twice. Refund mappings diverge. Status enumerations get hard to match in reconciliations.

A unified payment integration library collapses that surface to one. Your code compiles against one request schema, ships one payment service, and chooses the underlying processor at config time rather than at code time. 

This piece walks through that pattern of implementation for Stripe and Global Payments as the two connectors, and currency-based routing.

![demo store with Stripe and Globalpayments](https://iili.io/Bt2zo37.gif)

## Five step integration with the Hyperswitch Prism unified payments library

The flow is the same regardless of which payment processor handles the order. Each step below maps directly to a section in the Prism docs ([Installation and Configuration](https://docs.hyperswitch.io/integrations/prism/getting-started/installation), [First Payment](https://docs.hyperswitch.io/integrations/prism/getting-started/first-payment)) — adapted here for two connectors instead of one, with currency as a selector.

### 1. Install the payments library

Install the SDK in the language of your app's payment service. 
Node.js shown below, and equivalent commands exist for Python (`pip install hyperswitch-prism`), Java (Maven `io.hyperswitch:prism`), and PHP (`composer require hyperswitch-prism`).

```bash
npm install hyperswitch-prism
```

### 2. Configure the payment processor clients

Each payment processor is treated as a connector by the library. Every connector gets its own `ConnectorConfig`, and each `ConnectorConfig` produces its own `PaymentClient`. In this case the two clients will be Stripe and Global Payments. 

Note that Stripe takes a single `apiKey`, but Global Payments takes `appId`, `appKey`, and `baseUrl`.

```typescript
import { PaymentClient, MerchantAuthenticationClient, types } from 'hyperswitch-prism';

const stripeConfig: types.IConnectorConfig = {
  connectorConfig: {
    stripe: {
      apiKey: { value: process.env.STRIPE_API_KEY! },
      baseUrl: 'https://api.stripe.com',
    },
  },
};

const globalpayConfig: types.IConnectorConfig = {
  connectorConfig: {
    globalpay: {
      appId: { value: process.env.GP_APP_ID! },
      appKey: { value: process.env.GP_APP_KEY! },
      baseUrl: 'https://apis.globalpay.com',
    },
  },
};

const stripeClient = new PaymentClient(stripeConfig);
const globalpayClient = new PaymentClient(globalpayConfig);

// Currency-based connector selection — your routing rule.
function clientFor(currency: types.Currency) {
  return currency === types.Currency.USD ? stripeClient : globalpayClient;
}
```

The connector identifier is fixed at config time and lives inside the client. `clientFor(currency)` is the only line of code that knows about the currency rule. Every subsequent step calls a method on whichever client `clientFor` returned — the request shape and method names are identical across both.

### 3. Generate Session Token

Your backend should request the library for a client-side authentication token before the customer interacts with the hosted form. This is what Stripe's docs call a Payment Intent `client_secret` and what Global Payments calls a session ID. The library normalises the call but returns the connector's native field on `response.sessionData.connectorSpecific.<connector>`.

```typescript
async function createClientToken(order: { id: string; amount: number; currency: types.Currency }) {
  const config = order.currency === types.Currency.USD ? stripeConfig : globalpayConfig;
  const authClient = new MerchantAuthenticationClient(config);

  const response = await authClient.createClientAuthenticationToken({
    merchantClientSessionId: `sess_${order.id}`,
    payment: {
      amount: { minorAmount: order.amount, currency: order.currency },
    },
    testMode: true,
  });

  // Stripe returns clientSecret; Global Payments returns its own session field.
  if (order.currency === types.Currency.USD) {
    return response.sessionData?.connectorSpecific?.stripe?.clientSecret?.value;
  }
  return response.sessionData?.connectorSpecific?.globalpay;
}
```

The token is scoped to the connector. So, it is important to use the token created against `stripeConfig` to bootstrap a Stripe Payment Element, and a token created against `globalpayConfig` to bootstrap a Global Payments Hosted Payment Page. Your frontend cannot mix them.

### 4. Open the Hosted checkout from your frontend

Your frontend should use the client token to initialise the payment processor (PSP) hosted form, so that the customer enters their card directly into the PSP's surface. The form returns a connector-issued payment-method token to your frontend. 
This is where PCI scope sits with the PSP — the PAN is captured by Stripe's Payment Element or Global Payments' Hosted Payment Page, never traverses your origin, and is never touched by the payment integration library.

```html
<!-- USD orders: Stripe Payment Element -->
<script src="https://js.stripe.com/v3/"></script>
<div id="card-element"></div>
<button id="submit">Pay</button>

<script>
  const stripe = Stripe(window.STRIPE_PUBLISHABLE_KEY); // pk_xxx
  const elements = stripe.elements({ clientSecret });   // from step 3
  elements.create('card').mount('#card-element');

  document.getElementById('submit').addEventListener('click', async () => {
    const { error, paymentMethod } = await stripe.createPaymentMethod({
      type: 'card',
      card: elements.getElement('card'),
    });
    if (error) return console.error(error.message);
    // POST paymentMethod.id ("pm_1234...") to your backend → step 5
  });
</script>
```

For EUR orders, the same shape applies with Global Payments' Hosted Payment Page (HPP). You can embed the GP HPP using the session field returned in step 3, and the HPP posts back a Global Payments payment-method token reference. Your backend now holds a token (`pm_xxx` for Stripe, a GP payment-token for Global Payments) and the order's `merchantTransactionId`. 

### 5. Authorize the payment

Your backend calls `tokenAuthorize` on whichever client `clientFor(order.currency)` returns. The same function works against either connector - Stripe or Global payments.

```typescript
async function chargeOrder(order: {
  id: string;
  amount: number;
  currency: types.Currency;
  connectorToken: string; // from step 4
}) {
  const client = clientFor(order.currency); 
  // chooses the right client based on currency

  const auth = await client.tokenAuthorize({
    merchantTransactionId: order.id,
    merchantOrderId: `order_${order.id}`,
    amount: { minorAmount: order.amount, currency: order.currency },
    connectorToken: { value: order.connectorToken },
    address: { billingAddress: {} },
    captureMethod: types.CaptureMethod.AUTOMATIC,
    returnUrl: 'https://merchant.example/return',
    testMode: true,
  });

  if (auth.status === types.PaymentStatus.FAILURE) {
    throw new PaymentDeclined(auth.error?.code, auth.error?.message);
  }

  // Persist (merchantTransactionId, connector, connectorTransactionId) together.
  // Now onwards every downstream operation reads `connector` from this row, not the routing rule.
  await orders.markPaid(order.id, {
    connector: order.currency === types.Currency.USD ? 'stripe' : 'globalpay',
    connectorTransactionId: auth.connectorTransactionId,
  });

  return auth;
}
```

The request shape is identical across the two connectors. The `connectorToken` value is the only payload field that carries connector lineage, and your backend treats it as opaque. The `connector` string persisted alongside the transaction is what makes every downstream operation deterministic — refunds, captures, voids, recurring charges read the connector from storage, never recompute it from the routing rule.

## Integrating with AI agents?
Point your agent to [this skill file](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/demo-integration/SKILL.md) of payments integration library from your application codebase, and answer the followup questions with your integration requirements.

![prism integration skill](https://s13.gifyu.com/images/b7Wxe.gif)

## Extending to more operations - Capture, Refund, Recurring charges

Once the charge has settled, every follow-up operation against that transaction goes through the same `PaymentClient` shape — only the config changes based on which connector originated it.

```typescript
async function refundOrder(orderRef: { connector: 'stripe' | 'globalpay'; connectorTransactionId: string; amount: number; currency: types.Currency }) {
  const config = orderRef.connector === 'stripe' ? stripeConfig : globalpayConfig;
  const client = new PaymentClient(config);

  return client.refund({
    merchantRefundId: `ref_${Date.now()}`,
    connectorTransactionId: orderRef.connectorTransactionId,
    refundAmount: { minorAmount: orderRef.amount, currency: orderRef.currency },
    paymentAmount: orderRef.amount,
    reason: 'customer_request',
  });
}
```

For a subscription business, the recurring charge runs through `RecurringPaymentClient` against the same canonical schema:

```typescript
import { RecurringPaymentClient, types } from 'hyperswitch-prism';

const recurringClient = new RecurringPaymentClient(stripeConfig);
const charge = await recurringClient.charge({
  connectorRecurringPaymentId: { /* mandate reference returned at setup */ },
  amount: { minorAmount: 1999, currency: types.Currency.USD },
  paymentMethod: { token: { token: { value: subscriber.connectorPaymentMethodToken } } },
  connectorCustomerId: subscriber.connectorCustomerId,
  offSession: true,
  returnUrl: 'https://merchant.example/recurring-return',
});
```

The mandate is set up for the first time via `paymentClient.setupRecurring(...)` against whichever connector owns the customer relationship. The recurring `charge` call uses the same response status enum (`PaymentStatus`) the one-shot `authorize` call returns. Subscription dunning logic does not need a connector branch.


## Important points to note
- Your app owns currency-based routing logic. The library owns the request-response transformation and the API call to the payment processor once the routing decision is made.
- The PCI scope sits with the payment processor. Neither your app nor the library will handle the raw card data. 
- Your app will need to persist some information for the flows to be managed after authorize. The `connector` is a very important field because it encodes the historical fact of where the transaction lives. Every other future operation (refund, capture, etc.) shall read from the table below.

    ```sql
    CREATE TABLE order_payments (
      merchant_transaction_id   TEXT PRIMARY KEY,
      connector                 TEXT NOT NULL,           -- 'stripe' | 'globalpay'
      connector_transaction_id  TEXT NOT NULL,           -- pi_xxx | GP-API GUID
      connector_customer_id     TEXT,                    -- for recurring
      recurring_payment_id      TEXT,                    -- mandate reference
      currency                  TEXT NOT NULL,
      minor_amount              BIGINT NOT NULL,
      status                    INT NOT NULL             -- types.PaymentStatus enum value
    );
    ```

The canonical schema of the payment integration library - `PaymentService.Authorize`, `PaymentService.TokenAuthorize`, `PaymentService.Refund`, `PaymentService.Capture`, `PaymentService.Void`, `RecurringPaymentService.Charge`. It becomes the unified grammar to interact with any payment processor. Adding a third connector to the integration above is a matter of adding one more `ConnectorConfig` and extending the currency selector. The application code does not change.
