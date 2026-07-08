# Klarna

Buy Now Pay Later (BNPL) payment method. The customer pays later or in instalments — Klarna handles the credit decision. Prism redirects to Klarna's hosted flow via `return_url`.

Connector-specific options (e.g. preferred locale, billing details) can be passed via `connectorFeatureData`.

{% tabs %}

{% tab title="Node.js" %}
```typescript
const paymentClient = new PaymentClient(config);

const response = await paymentClient.authorize({
    merchantTransactionId: "txn_001",
    amount: { minorAmount: 1000, currency: Currency.USD },
    paymentMethod: { klarna: {} },
    captureMethod: CaptureMethod.AUTOMATIC,
    address: { billingAddress: {} },
    authType: AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return",
});
```
{% endtab %}

{% tab title="Python" %}
```python
from payments import (
    PaymentClient, PaymentServiceAuthorizeRequest, Money, Currency,
    CaptureMethod, AuthenticationType, PaymentAddress, Address,
    PaymentMethod, Klarna,
)

response = await payment_client.authorize(
    PaymentServiceAuthorizeRequest(
        merchant_transaction_id="txn_001",
        amount=Money(minor_amount=1000, currency=Currency.USD),
        payment_method=PaymentMethod(klarna=Klarna()),
        capture_method=CaptureMethod.AUTOMATIC,
        address=PaymentAddress(billing_address=Address()),
        auth_type=AuthenticationType.NO_THREE_DS,
        return_url="https://example.com/return",
    )
)
```
{% endtab %}

{% tab title="Kotlin" %}
```kotlin
val client = PaymentClient(config)

val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
    merchantTransactionId = "txn_001"
    amountBuilder.apply {
        minorAmount = 1000L
        currency = Currency.USD
    }
    paymentMethodBuilder.klarnaBuilder.apply {}
    captureMethod = CaptureMethod.AUTOMATIC
    addressBuilder.billingAddressBuilder.apply {}
    authType = AuthenticationType.NO_THREE_DS
    returnUrl = "https://example.com/return"
}.build()

val response = client.authorize(request)
```
{% endtab %}

{% endtabs %}
