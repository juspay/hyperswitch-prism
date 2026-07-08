# SEPA Direct Debit

Single Euro Payments Area direct debit. Debits funds directly from the customer's EU bank account using their IBAN. Used for EUR payments across eurozone countries.

{% tabs %}

{% tab title="Node.js" %}
```typescript
const paymentClient = new PaymentClient(config);

const response = await paymentClient.authorize({
    merchantTransactionId: "txn_001",
    amount: { minorAmount: 1000, currency: Currency.EUR },
    paymentMethod: {
        sepa: {
            iban: { value: "DE89370400440532013000" },
            bankAccountHolderName: { value: "John Doe" },
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
    PaymentMethod, Sepa, SecretString,
)

response = await payment_client.authorize(
    PaymentServiceAuthorizeRequest(
        merchant_transaction_id="txn_001",
        amount=Money(minor_amount=1000, currency=Currency.EUR),
        payment_method=PaymentMethod(
            sepa=Sepa(
                iban=SecretString(value="DE89370400440532013000"),
                bank_account_holder_name=SecretString(value="John Doe"),
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
        currency = Currency.EUR
    }
    paymentMethodBuilder.sepaBuilder.apply {
        ibanBuilder.value = "DE89370400440532013000"
        bankAccountHolderNameBuilder.value = "John Doe"
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
