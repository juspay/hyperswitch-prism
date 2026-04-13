# Apple Pay

Wallet payment using a tokenized card from the customer's Apple Wallet. The encrypted payment data is obtained from the Apple Pay JS API (or native SDK) on the client side — it is never constructed manually.

> **Before you send this request:** Complete the Apple Pay payment sheet on the client. The `PKPaymentToken` object contains `paymentData` (base64-encoded encrypted blob), `paymentMethod` (display name, network, type), and `transactionIdentifier`. Pass these directly into the fields below.

{% tabs %}

{% tab title="Node.js" %}
```typescript
const paymentClient = new PaymentClient(config);

// applePayToken: PKPaymentToken from the Apple Pay JS / native callback
const response = await paymentClient.authorize({
    merchantTransactionId: "txn_001",
    amount: { minorAmount: 1000, currency: Currency.USD },
    paymentMethod: {
        applePay: {
            paymentData: {
                encryptedData: applePayToken.paymentData,  // base64 string
            },
            paymentMethod: {
                displayName: applePayToken.paymentMethod.displayName,
                network: applePayToken.paymentMethod.network,
                type: applePayToken.paymentMethod.type,
            },
            transactionIdentifier: applePayToken.transactionIdentifier,
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
    PaymentMethod, AppleWallet,
)

# apple_pay_token: PKPaymentToken from the Apple Pay callback
response = await payment_client.authorize(
    PaymentServiceAuthorizeRequest(
        merchant_transaction_id="txn_001",
        amount=Money(minor_amount=1000, currency=Currency.USD),
        payment_method=PaymentMethod(
            apple_pay=AppleWallet(
                payment_data=AppleWallet.PaymentData(
                    encrypted_data=apple_pay_token["paymentData"],  # base64 string
                ),
                payment_method=AppleWallet.PaymentMethod(
                    display_name=apple_pay_token["paymentMethod"]["displayName"],
                    network=apple_pay_token["paymentMethod"]["network"],
                    type=apple_pay_token["paymentMethod"]["type"],
                ),
                transaction_identifier=apple_pay_token["transactionIdentifier"],
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

// applePayToken: PKPaymentToken from the Apple Pay callback
val request = PaymentServiceAuthorizeRequest.newBuilder().apply {
    merchantTransactionId = "txn_001"
    amountBuilder.apply {
        minorAmount = 1000L
        currency = Currency.USD
    }
    paymentMethodBuilder.applePayBuilder.apply {
        paymentDataBuilder.encryptedData = applePayToken.paymentData
        paymentMethodBuilder.apply {
            displayName = applePayToken.paymentMethod.displayName
            network = applePayToken.paymentMethod.network
            pmType = applePayToken.paymentMethod.type
        }
        transactionIdentifier = applePayToken.transactionIdentifier
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
