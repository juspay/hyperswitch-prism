# Connector Token

Used when card details are collected by the processor's JS SDK (e.g. Stripe.js, Adyen Drop-in). Raw card data never reaches your server — the processor returns an opaque token that Prism accepts directly.

There are two ways to use a token depending on your flow:

---

## Non-PCI: tokenize then authorize

**When:** Token was just collected from the processor's JS SDK in the browser right now.

**Step 1 — Collect and tokenize in the browser** using the processor's JS SDK (e.g. Stripe.js, Adyen Drop-in). Card details are entered and tokenized entirely client-side — they never reach your server or Prism. The SDK returns an opaque token (e.g. `pm_xxx`).

**Step 2 — Authorize** using `tokenAuthorize`. Pass the token in `connectorToken`:

{% tabs %}

{% tab title="Node.js" %}
```typescript
const paymentClient = new PaymentClient(config);

const response = await paymentClient.tokenAuthorize({
    merchantTransactionId: "txn_001",
    amount: { minorAmount: 1000, currency: Currency.USD },
    connectorToken: { value: "pm_1AbcXyzStripeTestToken" },
    address: { billingAddress: {} },
    captureMethod: CaptureMethod.AUTOMATIC,
    returnUrl: "https://example.com/return",
});
```
{% endtab %}

{% tab title="Python" %}
```python
from payments import (
    PaymentServiceTokenAuthorizeRequest, Money, Currency, CaptureMethod,
    PaymentAddress, Address, SecretString,
)

response = await payment_client.token_authorize(
    PaymentServiceTokenAuthorizeRequest(
        merchant_transaction_id="txn_001",
        amount=Money(minor_amount=1000, currency=Currency.USD),
        connector_token=SecretString(value="pm_1AbcXyzStripeTestToken"),
        address=PaymentAddress(billing_address=Address()),
        capture_method=CaptureMethod.AUTOMATIC,
        return_url="https://example.com/return",
    )
)
```
{% endtab %}

{% tab title="Kotlin" %}
```kotlin
val client = PaymentClient(config)

val request = PaymentServiceTokenAuthorizeRequest.newBuilder().apply {
    merchantTransactionId = "txn_001"
    amountBuilder.apply {
        minorAmount = 1000L
        currency = Currency.USD
    }
    connectorTokenBuilder.value = "pm_1AbcXyzStripeTestToken"
    addressBuilder.billingAddressBuilder.apply {}
    captureMethod = CaptureMethod.AUTOMATIC
    returnUrl = "https://example.com/return"
}.build()

val response = client.token_authorize(request)
```
{% endtab %}

{% endtabs %}

See [First Payment — non-PCI flow](../first-payment.md#non-pci-stripe-flow-get-the-payment_method_id-first) for the full end-to-end example including the browser-side tokenize step.

---

## Direct authorize with a token

**When:** You already hold a token from a previous tokenize call and want to charge it via the standard `authorize` flow. Pass it in `paymentMethod.token`:

{% tabs %}

{% tab title="Node.js" %}
```typescript
const paymentClient = new PaymentClient(config);

const response = await paymentClient.authorize({
    merchantTransactionId: "txn_002",
    amount: { minorAmount: 1000, currency: Currency.USD },
    paymentMethod: {
        token: { token: { value: "pm_1AbcXyzStripeTestToken" } },
    },
    address: { billingAddress: {} },
    captureMethod: CaptureMethod.AUTOMATIC,
    returnUrl: "https://example.com/return",
});
```
{% endtab %}

{% tab title="Python" %}
```python
response = await payment_client.authorize(ParseDict(
    {
        "merchant_transaction_id": "txn_002",
        "amount": { "minor_amount": 1000, "currency": "USD" },
        "payment_method": {
            "token": { "token": { "value": "pm_1AbcXyzStripeTestToken" } }
        },
        "address": { "billing_address": {} },
        "capture_method": "AUTOMATIC",
        "return_url": "https://example.com/return",
    },
    payment_pb2.PaymentServiceAuthorizeRequest(),
))
```
{% endtab %}

{% tab title="Kotlin" %}
```kotlin
val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
    merchantTransactionId = "txn_002"
    amountBuilder.apply {
        minorAmount = 1000L
        currency = Currency.USD
    }
    paymentMethodBuilder.tokenBuilder.tokenBuilder.value = "pm_1AbcXyzStripeTestToken"
    addressBuilder.billingAddressBuilder.apply {}
    captureMethod = CaptureMethod.AUTOMATIC
    returnUrl = "https://example.com/return"
}.build()

val response = client.authorize(request)
```
{% endtab %}

{% endtabs %}
