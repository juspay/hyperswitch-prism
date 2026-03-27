# First Payment

In the next few steps you will authorize the payment, handle errors, capture funds, and process refunds. And then you will be ready to send payment to any payment processor, without writing specialized code for each.

You will have a `payment_method_id` if you depend on your processor for PCI compliance. Use this to [Authorize with Payment Method ID](#authorize-with-payment-method-id).

Alternatively if your Payment processor API keys are enabled to accept PCI compliant raw card data, that will suffice to make the first payment. Jump to [Authorize with Raw Card Details](#authorize-with-raw-card-details-pci-compliant).

## Authorize with Payment Method ID

Use the `payment_method_id` from [Quick Start](./quick-start.md) to authorize the payment:

{% tabs %}

{% tab title="Node.js" %}

```javascript
const { ConnectorClient, Currency, CaptureMethod } = require('@juspay/hyperswitch-prism');

async function authorizePayment(paymentMethodId) {
    const client = new ConnectorClient({
        connectors: {
            stripe: { apiKey: process.env.STRIPE_API_KEY }
        }
    });

    try {
        // Authorize using the payment_method_id from Stripe or Adyen
        const auth = await client.payments.authorize({
            amount: { minorAmount: 1000, currency: Currency.USD },
            merchantOrderId: 'order-456',
            paymentMethod: {
                paymentMethodId: paymentMethodId  // e.g., 'pm_1234...'
            },
            captureMethod: CaptureMethod.AUTOMATIC  // Authorize only, capture later
        });

        console.log('Authorized:', auth.paymentId, auth.status);  // AUTHORIZED
        return auth;

    } catch (error) {
        handlePaymentError(error);
    }
}

function handlePaymentError(error) {
    switch (error.code) {
        case 'PAYMENT_DECLINED':
            console.error('Card declined:', error.message);
            break;
        case 'EXPIRED_CARD':
            console.error('Card expired:', error.message);
            break;
        case 'INSUFFICIENT_FUNDS':
            console.error('Insufficient funds:', error.message);
            break;
        case 'NETWORK_TIMEOUT':
            console.error('Network issue:', error.message);
            break;
        default:
            console.error('Payment failed:', error.message);
            console.error('Request ID:', error.requestId);
    }
}
```

{% endtab %}

{% tab title="Python" %}

```python
import os
from connector_service import ConnectorClient, Currency, CaptureMethod
from connector_service.errors import PaymentDeclinedError, NetworkTimeoutError

client = ConnectorClient(
    connectors={"stripe": {"api_key": os.environ["STRIPE_API_KEY"]}}
)

def authorize_payment(payment_method_id):
    try:
        auth = client.payments.authorize(
            amount={"minor_amount": 1000, "currency": Currency.USD},
            merchant_order_id="order-456",
            payment_method={
                "payment_method_id": payment_method_id  # e.g., 'pm_1234...'
            },
            capture_method=CaptureMethod.MANUAL
        )
        print(f"Authorized: {auth.payment_id}, {auth.status}")
        return auth

    except PaymentDeclinedError as e:
        print(f"Card declined: {e.message}")
    except NetworkTimeoutError as e:
        print(f"Network issue: {e.message}")
    except Exception as e:
        print(f"Payment failed: {e}")
```

{% endtab %}

{% tab title="Java" %}

```java
import com.juspay.connectorservice.*;
import com.juspay.connectorservice.errors.*;

public class FirstPayment {
    public ConnectorClient client = ConnectorClient.builder()
        .connector("stripe", StripeConfig.builder()
            .apiKey(System.getenv("STRIPE_API_KEY"))
            .build())
        .build();

    public void authorizePayment(String paymentMethodId) {
        try {
            AuthorizeResponse auth = client.payments().authorize(
                AuthorizeRequest.builder()
                    .amount(Amount.of(1000, Currency.USD))
                    .merchantOrderId("order-456")
                    .paymentMethod(PaymentMethod.byId(paymentMethodId))
                    .captureMethod(CaptureMethod.MANUAL)
                    .build()
            );
            System.out.println("Authorized: " + auth.getPaymentId());

        } catch (PaymentDeclinedError e) {
            System.err.println("Card declined: " + e.getMessage());
        } catch (NetworkTimeoutError e) {
            System.err.println("Network issue: " + e.getMessage());
        } catch (PaymentError e) {
            System.err.println("Payment failed: " + e.getMessage());
            System.err.println("Request ID: " + e.getRequestId());
        }
    }
}
```

{% endtab %}

{% tab title="PHP" %}

```php
<?php
use ConnectorService\ConnectorClient;
use ConnectorService\Enum\Currency;
use ConnectorService\Enum\CaptureMethod;

$client = new ConnectorClient([
    'connectors' => [
        'stripe' => ['api_key' => $_ENV['STRIPE_API_KEY']]
    ]
]);

function authorizePayment($paymentMethodId) use ($client) {
    try {
        $auth = $client->payments()->authorize([
            'amount' => ['minor_amount' => 1000, 'currency' => Currency::USD],
            'merchant_order_id' => 'order-456',
            'payment_method' => [
                'payment_method_id' => $paymentMethodId  // e.g., 'pm_1234...'
            ],
            'capture_method' => CaptureMethod::MANUAL
        ]);
        echo "Authorized: " . $auth->getPaymentId() . "\n";
        return $auth;

    } catch (PaymentDeclinedException $e) {
        echo "Card declined: " . $e->getMessage() . "\n";
    } catch (NetworkTimeoutException $e) {
        echo "Network issue: " . $e->getMessage() . "\n";
    } catch (Exception $e) {
        echo "Payment failed: " . $e->getMessage() . "\n";
    }
}
```

{% endtab %}

{% endtabs %}

## Authorize with Raw Card Details (PCI Compliant)

If you're PCI compliant and collect card details directly:

{% tabs %}

{% tab title="Node.js" %}

```javascript
const auth = await client.payments.authorize({
    amount: { minorAmount: 1000, currency: Currency.USD },
    merchantOrderId: 'order-456',
    paymentMethod: {
        card: {
            cardNumber: '4242424242424242',
            expiryMonth: '12',
            expiryYear: '2027',
            cardHolderName: 'Jane Doe'
        }
    },
    captureMethod: CaptureMethod.AUTOMATIC  // Charge immediately
});
```

{% endtab %}

{% tab title="Python" %}

```python
auth = client.payments.authorize(
    amount={"minor_amount": 1000, "currency": Currency.USD},
    merchant_order_id="order-456",
    payment_method={
        "card": {
            "card_number": "4242424242424242",
            "expiry_month": "12",
            "expiry_year": "2027",
            "card_holder_name": "Jane Doe"
        }
    },
    capture_method=CaptureMethod.AUTOMATIC
)
```

{% endtab %}

{% tab title="Java" %}

```java
AuthorizeResponse auth = client.payments().authorize(
    AuthorizeRequest.builder()
        .amount(Amount.of(1000, Currency.USD))
        .merchantOrderId("order-456")
        .paymentMethod(PaymentMethod.card(
            "4242424242424242", "12", "2027", "Jane Doe"))
        .captureMethod(CaptureMethod.AUTOMATIC)
        .build()
);
```

{% endtab %}

{% tab title="PHP" %}

```php
$auth = $client->payments()->authorize([
    'amount' => ['minor_amount' => 1000, 'currency' => Currency::USD],
    'merchant_order_id' => 'order-456',
    'payment_method' => [
        'card' => [
            'card_number' => '4242424242424242',
            'expiry_month' => '12',
            'expiry_year' => '2027',
            'card_holder_name' => 'Jane Doe'
        ]
    ],
    'capture_method' => CaptureMethod::AUTOMATIC
]);
```

{% endtab %}

{% endtabs %}

## Complete Payment Flow

After authorization, capture funds and handle refunds:

{% tabs %}

{% tab title="Node.js" %}

```javascript
// 1. Check payment status
const status = await client.payments.get({
    paymentId: auth.paymentId
});
console.log('Current status:', status.status);

// 2. Capture the funds (when order ships)
const capture = await client.payments.capture({
    paymentId: auth.paymentId,
    amount: { minorAmount: 1000, currency: Currency.USD }
});
console.log('Captured:', capture.status);  // CAPTURED

// 3. Process a partial refund (customer returns item)
const refund = await client.payments.refund({
    paymentId: auth.paymentId,
    amount: { minorAmount: 500, currency: Currency.USD },  // Refund $5
    reason: 'Customer return'
});
console.log('Refund ID:', refund.refundId);
```

{% endtab %}

{% tab title="Python" %}

```python
# 1. Check payment status
status = client.payments.get(payment_id=auth.payment_id)
print(f"Current status: {status.status}")

# 2. Capture the funds
capture = client.payments.capture(
    payment_id=auth.payment_id,
    amount={"minor_amount": 1000, "currency": Currency.USD}
)
print(f"Captured: {capture.status}")

# 3. Process a partial refund
refund = client.payments.refund(
    payment_id=auth.payment_id,
    amount={"minor_amount": 500, "currency": Currency.USD},
    reason="Customer return"
)
print(f"Refund ID: {refund.refund_id}")
```

{% endtab %}

{% tab title="Java" %}

```java
// 1. Check payment status
PaymentResponse status = client.payments().get(
    GetPaymentRequest.builder()
        .paymentId(auth.getPaymentId())
        .build()
);
System.out.println("Status: " + status.getStatus());

// 2. Capture the funds
CaptureResponse capture = client.payments().capture(
    CaptureRequest.builder()
        .paymentId(auth.getPaymentId())
        .amount(Amount.of(1000, Currency.USD))
        .build()
);
System.out.println("Captured: " + capture.getStatus());

// 3. Process a partial refund
RefundResponse refund = client.payments().refund(
    RefundRequest.builder()
        .paymentId(auth.getPaymentId())
        .amount(Amount.of(500, Currency.USD))
        .reason("Customer return")
        .build()
);
System.out.println("Refund ID: " + refund.getRefundId());
```

{% endtab %}

{% tab title="PHP" %}

```php
// 1. Check payment status
$status = $client->payments()->get(['payment_id' => $auth->getPaymentId()]);
echo "Status: " . $status->getStatus() . "\n";

// 2. Capture the funds
$capture = $client->payments()->capture([
    'payment_id' => $auth->getPaymentId(),
    'amount' => ['minor_amount' => 1000, 'currency' => Currency::USD]
]);
echo "Captured: " . $capture->getStatus() . "\n";

// 3. Process a partial refund
$refund = $client->payments()->refund([
    'payment_id' => $auth->getPaymentId(),
    'amount' => ['minor_amount' => 500, 'currency' => Currency::USD],
    'reason' => 'Customer return'
]);
echo "Refund ID: " . $refund->getRefundId() . "\n";
```

{% endtab %}

{% endtabs %}

## Error Scenarios

### Declined Card

```javascript
// Using test card: 4000000000000002 (declined)
const auth = await client.payments.authorize({
    paymentMethod: { paymentMethodId: 'pm_declined' }
});
// Throws: PaymentDeclinedError with code 'PAYMENT_DECLINED'
```

### Network Timeout

```javascript
try {
    const auth = await client.payments.authorize({...});
} catch (error) {
    if (error.code === 'NETWORK_TIMEOUT') {
        // Retry with exponential backoff
        await retryWithBackoff(() => client.payments.authorize(request));
    }
}
```

## Business Use Cases

### E-commerce: Two-Step Flow

Authorize at checkout. Capture when you ship.

```javascript
// Checkout: authorize only
const auth = await client.payments.authorize({
    amount: { minorAmount: 9999, currency: Currency.USD },
    captureMethod: CaptureMethod.MANUAL
});

// Later: when order ships
await client.payments.capture({
    paymentId: auth.paymentId,
    amount: { minorAmount: 9999, currency: Currency.USD }
});
```

### SaaS: Immediate Capture

For digital goods, capture immediately.

```javascript
const payment = await client.payments.authorize({
    amount: { minorAmount: 2900, currency: Currency.USD },
    captureMethod: CaptureMethod.AUTOMATIC
});
// Status: CAPTURED
```

### Marketplace: Partial Refund

Customer returns one item from a multi-item order.

```javascript
await client.payments.refund({
    paymentId: 'pay_abc123',
    amount: { minorAmount: 2500, currency: Currency.USD },
    reason: 'Item damaged in shipping'
});
```

## Key Takeaways

- **One error handler** works for all connectors
- **Unified error codes** tell you exactly what happened
- **Request IDs** enable support to trace issues
- **Same code** works for Stripe, Adyen, PayPal, and 50+ more

See [extending payment flows](./extending-to-more-flows.md) for subscriptions, 3D Secure, and more.
