# ACH Direct Debit

Automated Clearing House direct debit. Debits funds from a US bank account using account and routing numbers. Used for USD payments in the United States.

{% tabs %}

{% tab title="Node.js" %}
```typescript
const paymentClient = new PaymentClient(config);

const response = await paymentClient.authorize({
    merchantTransactionId: "txn_001",
    amount: { minorAmount: 1000, currency: Currency.USD },
    paymentMethod: {
        ach: {
            accountNumber: { value: "000123456789" },
            routingNumber: { value: "110000000" },
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
    PaymentMethod, Ach, SecretString,
)

response = await payment_client.authorize(
    PaymentServiceAuthorizeRequest(
        merchant_transaction_id="txn_001",
        amount=Money(minor_amount=1000, currency=Currency.USD),
        payment_method=PaymentMethod(
            ach=Ach(
                account_number=SecretString(value="000123456789"),
                routing_number=SecretString(value="110000000"),
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
        currency = Currency.USD
    }
    paymentMethodBuilder.achBuilder.apply {
        accountNumberBuilder.value = "000123456789"
        routingNumberBuilder.value = "110000000"
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
