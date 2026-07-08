---
name: demo-integration
description: >
  Embeds prism into any app and enables to connect to any payment processor
license: Apache-2.0
---


# Hyperswitch Prism — AI Integration Guide (llms.txt)

You are an AI assistant helping a developer integrate payment processors using the Hyperswitch Prism SDK. Follow this document exactly. Do NOT deviate from the prescribed flow.

---

## PHASE 0 — GATHER REQUIREMENTS (MANDATORY)

Before writing ANY code, you MUST ask the developer two questions. Do not skip this step. Do not assume answers.

### Question 1: Payment Processor Selection

Ask the developer @AskUserTool:

```
Which payment processor(s) do you want to integrate through Prism?

  1. Stripe
  2. Adyen
  3. Other (specify — Prism supports 81+ processors including PayPal, Cybersource, Braintree, Mollie, etc.)

You may select multiple processors.
```

Wait for their answer. Record their selection(s) as `SELECTED_PROCESSORS`.

### Question 2: PCI Compliance Strategy

Ask the developer @AskUserTool:

```
How will you handle PCI compliance?

  A. Let the payment processor manage PCI compliance (Non-PCI / Tokenized flow)
     → Card details are collected on the frontend using the processor's own SDK (e.g., Stripe Elements, Adyen Drop-in).
     → Your server never sees raw card numbers. You only handle opaque tokens.
     → Best for: most web/mobile apps, SaaS platforms, marketplaces.

  B. I will self-manage PCI compliance (PCI / Direct flow)
     → Your server directly handles raw card numbers.
     → Requires PCI DSS certification and a SAQ-D or equivalent audit.
     → Best for: payment facilitators, large enterprises with existing PCI scope.

Choose one option only: A or B.
```

Wait for their answer. Record their selection as `PCI_STRATEGY`.

---

## PHASE 1 — DETECT LANGUAGE (AUTOMATIC)

Do NOT ask the developer which language to use. Detect it automatically from the repository.

### Detection Rules (apply in order)

| Signal | Language |
|--------|----------|
| `package.json` exists in repo root | **Node.js (TypeScript)** |
| `requirements.txt` OR `pyproject.toml` OR `setup.py` OR `Pipfile` exists | **Python** |
| `pom.xml` OR `build.gradle` OR `build.gradle.kts` exists | **Java/Kotlin** |
| Multiple signals present | Prefer the one closest to the project's entry point / main source directory |
| No signal found | Ask the developer: "I couldn't detect your project language. Which SDK should I use: Node.js, Python, or Java?" |

Record the result as `SDK_LANGUAGE`.

### Install the SDK

Once language is detected, install the appropriate SDK:

**Node.js (v18+):**
```bash
npm install hyperswitch-prism
```

**Python (3.9+):**
```bash
pip install hyperswitch-prism
```

**Java/Kotlin (JDK 17+):**
```xml
<dependency>
  <groupId>io.hyperswitch</groupId>
  <artifactId>prism</artifactId>
  <version>0.0.1</version>
</dependency>
```

---

## PHASE 2 — FETCH LIVE DOCUMENTATION

Before writing integration code, you MUST fetch the latest Prism reference. This is non-negotiable. The upstream docs contain connector-specific required fields, working examples, and rules that change between releases.

### Step 2.1 — Fetch the master LLM context

```
Fetch: https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt
```

Read this document fully. It contains:
- Field Probe tool usage (discovers required fields per connector)
- Connector-specific examples (copy-paste ready code)
- Integration rules and status code reference
- Error handling patterns

### Step 2.2 — Fetch the generated API reference

```
Fetch: https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/docs-generated/llms.txt
```

This contains the master index of all 81+ connectors with links to per-connector documentation.

### Step 2.3 — Run Field Probe for each selected processor

For EACH processor in `SELECTED_PROCESSORS`, run the field probe to discover required fields:

```bash
npx hyperswitch-prism probe --connector <PROCESSOR_NAME> --flow authorize --payment-method card
npx hyperswitch-prism probe --connector <PROCESSOR_NAME> --all-flows
```

**Why this matters:** TypeScript types alone do NOT show which fields are required. Each connector has different required fields. Skipping this causes `IntegrationError: MISSING_REQUIRED_FIELD` at runtime.

---

## PHASE 3 — IMPLEMENT BASED ON PCI STRATEGY

Use the developer's `PCI_STRATEGY` answer to determine which integration pattern to follow.

---

### STRATEGY A: Non-PCI / Tokenized Flow

This is the 3-step tokenization pattern. The developer's server never handles raw card data.

#### How it works:

```
STEP 1 (Backend)  → Create a client authentication token via Prism
STEP 2 (Frontend) → Use processor's frontend SDK to collect card & tokenize
STEP 3 (Backend)  → Authorize payment using the opaque token via Prism
```

#### Step A.1 — Backend: Create Client Authentication Token

**Node.js:**
```typescript
import { MerchantAuthenticationClient, PaymentClient, types } from 'hyperswitch-prism';

// Configure for each processor
// Stripe:
const stripeConfig: types.ConnectorConfig = {
  connectorConfig: {
    stripe: { apiKey: { value: process.env.STRIPE_SECRET_KEY! } }
  }
};

// Adyen:
const adyenConfig: types.ConnectorConfig = {
  connectorConfig: {
    adyen: {
      apiKey: { value: process.env.ADYEN_API_KEY! },
      merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT! }
    }
  }
};

// Create client auth token (returns client secret for frontend)
async function createPaymentSession(processorConfig: types.ConnectorConfig, amount: number, currency: types.Currency) {
  const authClient = new MerchantAuthenticationClient(processorConfig);

  const response = await authClient.createClientAuthenticationToken({
    merchantClientSessionId: `session_${Date.now()}`,
    payment: {
      amount: { minorAmount: amount, currency }
    },
    testMode: true
  });

  // Extract processor-specific client secret
  const clientSecret =
    response.sessionData?.connectorSpecific?.stripe?.clientSecret?.value
    || response.sessionData?.connectorSpecific?.adyen?.clientSecret?.value;

  return { clientSecret, sessionData: response.sessionData };
}
```

**Python:**
```python
from payments import MerchantAuthenticationClient, SecretString
from payments.generated import sdk_config_pb2, payment_pb2
import os

# Stripe config
stripe_cfg = sdk_config_pb2.ConnectorConfig()
stripe_cfg.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
    stripe=payment_pb2.StripeConfig(
        api_key=SecretString(value=os.environ["STRIPE_SECRET_KEY"])
    )
))

# Create client auth token
auth_client = MerchantAuthenticationClient(stripe_cfg)
response = auth_client.create_client_authentication_token(
    payment_pb2.CreateClientAuthenticationTokenRequest(
        merchant_client_session_id=f"session_{int(time.time())}",
        payment=payment_pb2.PaymentInfo(
            amount=payment_pb2.MinorUnit(minor_amount=1000, currency=payment_pb2.Currency.USD)
        ),
        test_mode=True
    )
)
```

#### Step A.2 — Frontend: Tokenize Card Details

This step happens in the browser. Use the processor's own frontend SDK.

**Stripe (using Stripe.js + Elements):**
```html
<script src="https://js.stripe.com/v3/"></script>
<script>
  const stripe = Stripe('pk_test_YOUR_PUBLISHABLE_KEY');
  const elements = stripe.elements({ clientSecret: clientSecretFromBackend });
  const paymentElement = elements.create('payment');
  paymentElement.mount('#payment-element');

  // On form submit:
  const { error, paymentIntent } = await stripe.confirmPayment({
    elements,
    confirmParams: { return_url: window.location.origin + '/payment-complete' },
    redirect: 'if_required'
  });
  // paymentIntent.payment_method is the token (pm_xxx) to send to your backend
</script>
```

**Adyen (using Adyen Web Drop-in):**
```html
<script src="https://checkoutshopper-test.adyen.com/checkoutshopper/sdk/5.59.0/adyen.js"></script>
<link rel="stylesheet" href="https://checkoutshopper-test.adyen.com/checkoutshopper/sdk/5.59.0/adyen.css"/>
<script>
  const checkout = await AdyenCheckout({
    clientKey: 'test_YOUR_CLIENT_KEY',
    environment: 'test',
    session: { id: sessionIdFromBackend, sessionData: sessionDataFromBackend },
    onPaymentCompleted: (result) => {
      // result contains the token to send to your backend
      sendToBackend(result);
    }
  });
  checkout.create('dropin').mount('#dropin-container');
</script>
```

#### Step A.3 — Backend: Authorize Payment with Token

**Node.js:**
```typescript
// SAME code structure works for ALL processors — only config changes
async function authorizeWithToken(
  processorConfig: types.ConnectorConfig,
  token: string,
  amount: number,
  currency: types.Currency
) {
  const client = new PaymentClient(processorConfig);

  const auth = await client.tokenAuthorize({
    merchantTransactionId: `txn_${Date.now()}`,
    merchantOrderId: `order_${Date.now()}`,
    amount: { minorAmount: amount, currency },
    connectorToken: { value: token }, // pm_xxx (Stripe) or Adyen session token
    address: { billingAddress: {} },
    captureMethod: types.CaptureMethod.MANUAL, // or AUTOMATIC for immediate capture
    testMode: true
  });

  // CRITICAL: status is a NUMBER, not a string
  if (auth.status === types.PaymentStatus.AUTHORIZED) {       // === 6
    console.log('Payment authorized:', auth.connectorTransactionId);
  } else if (auth.status === types.PaymentStatus.CHARGED) {   // === 8 (if AUTOMATIC capture)
    console.log('Payment charged:', auth.connectorTransactionId);
  }

  return auth;
}
```

**Python:**
```python
client = PaymentClient(processor_cfg)
result = client.token_authorize(
    payment_pb2.TokenAuthorizeRequest(
        merchant_transaction_id=f"txn_{int(time.time())}",
        merchant_order_id=f"order_{int(time.time())}",
        amount=payment_pb2.MinorUnit(minor_amount=amount, currency=currency),
        connector_token=SecretString(value=token),
        capture_method=payment_pb2.CaptureMethod.MANUAL,
        test_mode=True,
    )
)
```

---

### STRATEGY B: PCI / Direct Flow

The developer's server handles raw card numbers directly. Requires PCI DSS compliance.

#### How it works:

```
STEP 1 (Backend) → Collect card details on your server
STEP 2 (Backend) → Send card details directly to Prism's authorize() method
```

#### Step B.1 — Authorize with Raw Card Data

**Node.js — Stripe:**
```typescript
import { PaymentClient, types, IntegrationError, ConnectorError, NetworkError } from 'hyperswitch-prism';

const client = new PaymentClient({
  connectorConfig: {
    stripe: { apiKey: { value: process.env.STRIPE_API_KEY! } }
  }
});

const authResult = await client.authorize({
  merchantTransactionId: 'txn_001',
  amount: { minorAmount: 1000, currency: types.Currency.USD },
  captureMethod: types.CaptureMethod.MANUAL,
  paymentMethod: {
    card: {
      cardNumber: { value: '4111111111111111' },
      cardExpMonth: { value: '12' },
      cardExpYear: { value: '2027' },
      cardCvc: { value: '123' },
      cardHolderName: { value: 'Jane Doe' }
    }
  },
  address: { billingAddress: {} },
  authType: types.AuthenticationType.NO_THREE_DS,
  testMode: true
});

// CRITICAL: status is a NUMBER, not a string
if (authResult.status === types.PaymentStatus.AUTHORIZED) { // === 6
  console.log('Authorized:', authResult.connectorTransactionId);
}
```

**Node.js — Adyen (requires browserInfo):**
```typescript
const client = new PaymentClient({
  connectorConfig: {
    adyen: {
      apiKey: { value: process.env.ADYEN_API_KEY! },
      merchantAccount: { value: process.env.ADYEN_MERCHANT_ACCOUNT! }
    }
  }
});

const authResult = await client.authorize({
  merchantTransactionId: 'txn_adyen_001',
  amount: { minorAmount: 1000, currency: types.Currency.EUR },
  captureMethod: types.CaptureMethod.MANUAL,
  paymentMethod: {
    card: {
      cardNumber: { value: '4111111111111111' },
      cardExpMonth: { value: '03' },
      cardExpYear: { value: '2030' },
      cardCvc: { value: '737' },
      cardHolderName: { value: 'Jane Doe' }
    }
  },
  // ⚠️ REQUIRED for Adyen — omitting this throws IntegrationError
  browserInfo: {
    colorDepth: 24,
    screenHeight: 900,
    screenWidth: 1440,
    javaEnabled: false,
    javaScriptEnabled: true,
    language: 'en-US',
    timeZoneOffsetMinutes: 0,
    acceptHeader: 'text/html,*/*;q=0.8',
    userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)'
  },
  address: { billingAddress: {} },
  authType: types.AuthenticationType.NO_THREE_DS,
  testMode: true
});
```

**Python — Stripe:**
```python
from payments import PaymentClient, SecretString
from payments.generated import sdk_config_pb2, payment_pb2
import os

cfg = sdk_config_pb2.ConnectorConfig()
cfg.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
    stripe=payment_pb2.StripeConfig(
        api_key=SecretString(value=os.environ["STRIPE_API_KEY"])
    )
))
client = PaymentClient(cfg)

request = payment_pb2.PaymentServiceAuthorizeRequest(
    merchant_transaction_id="txn_001",
    amount=payment_pb2.MinorUnit(minor_amount=1000, currency=payment_pb2.Currency.USD),
    capture_method=payment_pb2.CaptureMethod.AUTOMATIC,
    payment_method=payment_pb2.PaymentMethodData(
        card=payment_pb2.Card(
            card_number=SecretString(value="4111111111111111"),
            card_exp_month=SecretString(value="12"),
            card_exp_year=SecretString(value="2027"),
            card_cvc=SecretString(value="123"),
        )
    ),
    test_mode=True,
)

result = client.authorize(request)
# result.status == payment_pb2.CHARGED (8) for AUTOMATIC capture
```

---

## PHASE 4 — IMPLEMENT CAPTURE, REFUND, VOID (Both Strategies)

After authorization succeeds, implement these follow-up operations. This code is IDENTICAL regardless of PCI strategy — both tokenized and direct flows produce the same `connectorTransactionId`.

### Capture (for MANUAL capture method)

```typescript
const captureResult = await client.capture({
  merchantCaptureId: `cap_${Date.now()}`,
  connectorTransactionId: authResult.connectorTransactionId!,
  amountToCapture: { minorAmount: 1000, currency: types.Currency.USD },
  testMode: true
});
// captureResult.status === types.PaymentStatus.CHARGED (8)
```

### Refund

```typescript
const refundResult = await client.refund({
  merchantRefundId: `ref_${Date.now()}`,
  connectorTransactionId: authResult.connectorTransactionId!,
  refundAmount: { minorAmount: 500, currency: types.Currency.USD },
  reason: 'RETURN',
  // ⚠️ Adyen: reason MUST be enum: OTHER|RETURN|DUPLICATE|FRAUD|CUSTOMER_REQUEST
  // Stripe: accepts free-text
  testMode: true
});
// ⚠️ Use RefundStatus, NOT PaymentStatus
// refundResult.status === types.RefundStatus.REFUND_SUCCESS (4)
```

### Void (cancel before capture)

```typescript
const voidResult = await client.void({
  merchantVoidId: `void_${Date.now()}`,
  connectorTransactionId: authResult.connectorTransactionId!,
  cancellationReason: 'Customer cancelled',
  testMode: true
});
// voidResult.status === types.PaymentStatus.VOIDED (11)
```

---

## PHASE 5 — ERROR HANDLING (MANDATORY)

Wrap ALL Prism calls in this error handling pattern:

```typescript
import { IntegrationError, ConnectorError, NetworkError, types } from 'hyperswitch-prism';

async function safePaymentCall<T>(operation: () => Promise<T>): Promise<T | null> {
  try {
    return await operation();
  } catch (error) {
    if (error instanceof IntegrationError) {
      // Bad config, missing required field, serialization error
      // DO NOT retry. Fix the request structure.
      console.error('[IntegrationError]', error.errorCode, error.message);
    } else if (error instanceof ConnectorError) {
      // Response transformation failed
      // DO NOT retry automatically. Log and investigate.
      console.error('[ConnectorError]', error.errorCode, error.message);
    } else if (error instanceof NetworkError) {
      // Timeout, DNS failure, connection refused
      // SAFE to retry with exponential backoff
      console.error('[NetworkError]', error.message);
    }
    return null;
  }
}

// Usage:
const auth = await safePaymentCall(() => client.authorize(request));
if (auth && auth.status === types.PaymentStatus.AUTHORIZED) {
  // proceed to capture
}
```

---

## PHASE 6 — SUMMARIZE IMPLEMENTATION

After all code is written, provide the developer with a summary in this exact format:

```
## Implementation Summary

### What was integrated
- Processor(s): [list SELECTED_PROCESSORS]
- PCI strategy: [A: Non-PCI Tokenized / B: PCI Direct]
- SDK language: [SDK_LANGUAGE]
- SDK version: hyperswitch-prism@latest

### Files created or modified
- [list each file path and what it does]

### Environment variables required
- [list every env var needed, per processor]

### How to test

1. Set environment variables:
   [list the exact export commands with placeholder values]

2. Install dependencies:
   [npm install / pip install / maven command]

3. Start the server:
   [exact command]

4. Test the payment flow:
   [For Non-PCI: explain how to open the frontend, fill the form, and submit]
   [For PCI: provide a curl command to hit the authorize endpoint]

5. Verify in processor dashboard:
   - Stripe: https://dashboard.stripe.com/test/payments
   - Adyen: https://ca-test.adyen.com/
   - PayPal: https://www.sandbox.paypal.com/

### Test card numbers
| Processor | Card Number          | Expiry | CVC | Expected Result |
|-----------|----------------------|--------|-----|-----------------|
| Stripe    | 4242 4242 4242 4242  | 12/27  | 123 | Success         |
| Stripe    | 4000 0000 0000 0002  | 12/27  | 123 | Decline         |
| Adyen     | 4111 1111 1111 1111  | 03/30  | 737 | Success         |
| Adyen     | 5500 0000 0000 0004  | 03/30  | 737 | Decline         |
| PayPal    | 4111 1111 1111 1111  | 12/27  | 123 | Success         |
```

---

## CRITICAL RULES — NEVER VIOLATE THESE

### Rule 1: Status codes are NUMBERS, not strings

```typescript
// ❌ WRONG — always evaluates to false
if (response.status === 'CHARGED') { }
if (response.status === 'AUTHORIZED') { }

// ✅ CORRECT
if (response.status === 8) { }
if (response.status === types.PaymentStatus.CHARGED) { }   // === 8
if (response.status === types.PaymentStatus.AUTHORIZED) { } // === 6
```

### Rule 2: PaymentStatus vs RefundStatus — value 4 means DIFFERENT things

```typescript
// PaymentStatus 4 = AUTHENTICATION_PENDING (authorize/capture/void)
// RefundStatus 4  = REFUND_SUCCESS (refund only)

// ✅ CORRECT
if (auth.status === types.PaymentStatus.AUTHORIZED) { }       // authorize flow
if (refund.status === types.RefundStatus.REFUND_SUCCESS) { }   // refund flow
```

### Rule 3: Always run Field Probe before writing code

```bash
npx hyperswitch-prism probe --connector <name> --flow authorize --payment-method card
```

TypeScript types alone do NOT reveal which fields are required per connector.

### Rule 4: Connector config formats vary

```typescript
// Stripe — apiKey only
{ stripe: { apiKey: { value: '...' } } }

// Adyen — apiKey + merchantAccount
{ adyen: { apiKey: { value: '...' }, merchantAccount: { value: '...' } } }

// PayPal — clientId + clientSecret
{ paypal: { clientId: { value: '...' }, clientSecret: { value: '...' } } }
```

### Rule 5: Adyen ALWAYS requires browserInfo for card payments

Omitting `browserInfo` for Adyen throws `IntegrationError: MISSING_REQUIRED_FIELD: browser_info`.

### Rule 6: Guard optional fields

```typescript
const txId = authResult.connectorTransactionId ?? '';
const errMsg = response.error?.message ?? 'Unknown error';
```

---

## STATUS CODE REFERENCE

### PaymentStatus (authorize, capture, void)

| Code | Enum Name                   | Meaning              |
|------|-----------------------------|----------------------|
| 0    | UNSPECIFIED                 | Unknown              |
| 1    | STARTED                     | Initiated            |
| 4    | AUTHENTICATION_PENDING      | 3DS redirect needed  |
| 5    | AUTHENTICATION_SUCCESSFUL   | 3DS passed           |
| 6    | AUTHORIZED                  | Funds held           |
| 7    | AUTHORIZATION_FAILED        | Declined             |
| 8    | CHARGED                     | Captured             |
| 11   | VOIDED                      | Cancelled            |
| 20   | PENDING                     | Async processing     |
| 21   | FAILURE                     | Soft decline         |

### RefundStatus (refund only)

| Code | Enum Name            | Meaning       |
|------|----------------------|---------------|
| 1    | REFUND_FAILURE       | Failed        |
| 2    | REFUND_MANUAL_REVIEW | Under review  |
| 3    | REFUND_PENDING       | Processing    |
| 4    | REFUND_SUCCESS       | Completed     |

---

## SERVICE CLIENTS REFERENCE

| Client                              | Methods                                                            |
|-------------------------------------|--------------------------------------------------------------------|
| PaymentClient                       | authorize, tokenAuthorize, capture, refund, void, get, sync        |
| MerchantAuthenticationClient        | createServerAuthenticationToken, createClientAuthenticationToken    |
| CustomerClient                      | create                                                             |
| PaymentMethodClient                 | tokenize                                                           |
| PaymentMethodAuthenticationClient   | preAuthenticate, authenticate, postAuthenticate                    |
| RecurringPaymentClient              | setup, charge, revoke                                              |
| EventClient                         | handleEvent                                                        |
| RefundClient                        | get, createRefund, updateRefund                                    |
| DisputeClient                       | accept, defend, submitEvidence, get                                |

---

## ADDITIONAL RESOURCES

- Repository: https://github.com/juspay/hyperswitch-prism
- Full examples: https://github.com/juspay/hyperswitch-prism/tree/main/examples
- Field Probe data: https://github.com/juspay/hyperswitch-prism/tree/main/data/field_probe
- Generated API docs: https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/docs-generated/llms.txt
- Master LLM context: https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt