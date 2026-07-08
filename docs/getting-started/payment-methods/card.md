# Card

Raw card payment. The merchant collects card details directly and sends them to Prism. Requires PCI compliance — if you are not PCI compliant, use [Connector Token](./connector-token.md) instead.

{% tabs %}

{% tab title="Node.js" %}
```typescript
const paymentClient = new PaymentClient(config);

const response = await paymentClient.authorize({
    merchantTransactionId: "txn_001",
    amount: { minorAmount: 1000, currency: Currency.USD },
    paymentMethod: {
        card: {
            cardNumber: { value: "4111111111111111" },
            cardExpMonth: { value: "03" },
            cardExpYear: { value: "2030" },
            cardCvc: { value: "737" },
            cardHolderName: { value: "John Doe" },
        },
    },
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
    CardDetails, CardNumberType, PaymentMethod, SecretString,
)

response = await payment_client.authorize(
    PaymentServiceAuthorizeRequest(
        merchant_transaction_id="txn_001",
        amount=Money(minor_amount=1000, currency=Currency.USD),
        payment_method=PaymentMethod(
            card=CardDetails(
                card_number=CardNumberType(value="4111111111111111"),
                card_exp_month=SecretString(value="03"),
                card_exp_year=SecretString(value="2030"),
                card_cvc=SecretString(value="737"),
                card_holder_name=SecretString(value="John Doe"),
            )
        ),
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
    paymentMethodBuilder.cardBuilder.apply {
        cardNumberBuilder.value = "4111111111111111"
        cardExpMonthBuilder.value = "03"
        cardExpYearBuilder.value = "2030"
        cardCvcBuilder.value = "737"
        cardHolderNameBuilder.value = "John Doe"
    }
    captureMethod = CaptureMethod.AUTOMATIC
    addressBuilder.billingAddressBuilder.apply {}
    authType = AuthenticationType.NO_THREE_DS
    returnUrl = "https://example.com/return"
}.build()

val response = client.authorize(request)
```
{% endtab %}

{% endtabs %}
