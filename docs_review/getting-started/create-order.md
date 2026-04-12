# Quick Start

Now that the library is installed and configured, let's create your first payment order (different payment processor's use different terminology - order, intent, transaction and so on). Conceptually, it represents the user's intent to start a payment session. 

Once you implement this basic plumbing, you will be able to switch the request to any other payment processot, by changing one-line of code. 

- **If you depend on your Payment processor/ PCI vault for PCI compliance:** Lets start by creating a payment intent (order) with Stripe, which you can then use to complete the payment with Stripe.js on your frontend to collect payment details - if you are not PCI compliant.

- **If you are managing PCI compliance on your own:** You should be able to collect card data from the user and directly pass it to the payment processor. Ignore this section and directly jump to [First Payment](./first-payment.md).

## How to Create a Payment Order with Stripe?

### Prerequisites

- Library installed and configured (see [Installation and Configuration](./installation.md))

Create an order to get a `sessionToken` (client secret) from Stripe. The library will initiate the payment intent API of Stripe.

{% tabs %}

{% tab title="Node.js" %}

```javascript
const { PaymentClient } = require('hyperswitch-prism');

// stripeClient is configured in installation.md
const order = await stripeClient.createOrder({
    merchantOrderId: 'order-123',
    amount: {
        minorAmount: 1000,  // $10.00
        currency: 'USD'
    },
    orderType: 'PAYMENT',
    description: 'Test order'
});

console.log('Order ID:', order.connectorOrderId);
console.log('Client Secret:', order.sessionToken?.clientSecret);
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import PaymentClient
from hyperswitch_prism.generated import sdk_config_pb2

# stripe_client is configured in installation.md
order = stripe_client.create_order(
    sdk_config_pb2.CreateOrderRequest(
        merchant_order_id='order-123',
        amount=sdk_config_pb2.Amount(
            minor_amount=1000,
            currency='USD'
        ),
        order_type='PAYMENT',
        description='Test order'
    )
)

print(f"Order ID: {order.connector_order_id}")
print(f"Client Secret: {order.session_token.client_secret if order.session_token else 'N/A'}")
```

{% endtab %}

{% tab title="Java" %}

```java
import com.juspay.hyperswitch.prism.PaymentClient;
import com.juspay.hyperswitch.prism.types.*;

// stripeClient is configured in installation.md
CreateOrderResponse order = stripeClient.createOrder(
    CreateOrderRequest.builder()
        .merchantOrderId("order-123")
        .amount(Amount.of(1000, Currency.USD))
        .orderType("PAYMENT")
        .description("Test order")
        .build()
);

System.out.println("Order ID: " + order.getConnectorOrderId());
System.out.println("Client Secret: " + (order.getSessionToken() != null ? order.getSessionToken().getClientSecret() : "N/A"));
```

{% endtab %}

{% tab title="PHP" %}

```php
<?php
use HyperswitchPrism\PaymentClient;

// $stripeClient is configured in installation.md
$order = $stripeClient->createOrder([
    'merchant_order_id' => 'order-123',
    'amount' => [
        'minor_amount' => 1000,
        'currency' => 'USD'
    ],
    'order_type' => 'PAYMENT',
    'description' => 'Test order'
]);

echo "Order ID: " . $order->getConnectorOrderId() . "\n";
echo "Client Secret: " . ($order->getSessionToken()?->getClientSecret() ?? 'N/A') . "\n";
```

{% endtab %}

{% endtabs %}



## How to Create a Payment Order with Adyen?

### Prerequisites

- Library installed and configured (see [Installation and Configuration](./installation.md))

Create an order to get a `sessionToken` (session_id, session_data) from Adyen. The library will initiate the sessions API of Adyen.

{% tabs %}

{% tab title="Node.js" %}

```javascript
const { PaymentClient } = require('hyperswitch-prism');

// adyenClient is configured in installation.md
const order = await adyenClient.createOrder({
    merchantOrderId: 'order-456',
    amount: {
        minorAmount: 2500,  // €25.00
        currency: 'EUR'
    },
    orderType: 'PAYMENT',
    description: 'Test order',
    returnUrl: 'https://example.com/return'
});

console.log('Order ID:', order.connectorOrderId);
console.log('Session Token:', order.sessionToken);
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import PaymentClient
from hyperswitch_prism.generated import sdk_config_pb2

# adyen_client is configured in installation.md
order = adyen_client.create_order(
    sdk_config_pb2.CreateOrderRequest(
        merchant_order_id='order-456',
        amount=sdk_config_pb2.Amount(
            minor_amount=2500,
            currency='EUR'
        ),
        order_type='PAYMENT',
        description='Test order',
        return_url='https://example.com/return'
    )
)

print(f"Order ID: {order.connector_order_id}")
print(f"Session Token: {order.session_token}")
```

{% endtab %}

{% tab title="Java" %}

```java
import com.juspay.hyperswitch.prism.PaymentClient;
import com.juspay.hyperswitch.prism.types.*;

// adyenClient is configured in installation.md
CreateOrderResponse order = adyenClient.createOrder(
    CreateOrderRequest.builder()
        .merchantOrderId("order-456")
        .amount(Amount.of(2500, Currency.EUR))
        .orderType("PAYMENT")
        .description("Test order")
        .returnUrl("https://example.com/return")
        .build()
);

System.out.println("Order ID: " + order.getConnectorOrderId());
System.out.println("Session Token: " + order.getSessionToken());
```

{% endtab %}

{% tab title="PHP" %}

```php
<?php
use HyperswitchPrism\PaymentClient;

// $adyenClient is configured in installation.md
$order = $adyenClient->createOrder([
    'merchant_order_id' => 'order-456',
    'amount' => [
        'minor_amount' => 2500,
        'currency' => 'EUR'
    ],
    'order_type' => 'PAYMENT',
    'description' => 'Test order',
    'return_url' => 'https://example.com/return'
]);

echo "Order ID: " . $order->getConnectorOrderId() . "\n";
echo "Session Token: " . $order->getSessionToken() . "\n";
```

{% endtab %}

{% endtabs %}


## What Just Happened?

The library helped you create a Payment Order with the specified payment processor. You will receive a `connectorOrderId` and optionally a `sessionToken` in the response (Stripe returns a client secret, Adyen returns different session data).

The order is in `STARTED` state, ready for the next step - Authorize.

```json
{
    "connectorOrderId": "ord_xxx...",
    "status": "STARTED",
    "merchantOrderId": "order-123",
    "sessionToken": {
        "clientSecret": "pi_xxx..._secret_xxx"
    }
}
```

## How to use the secret to trigger the payment processor's checkout?

Now, pass the `sessionToken` data to your frontend and use it to initiate Stripe Elements or Adyen Card Component for PCI compliant card element. The users will be able to key in the card details on the Stripe Element/ Adyen Card Component and you will get a tokenized `payment_method_id`.

You can proceed to the making the [first payment](./first-payment.md)

## Next Steps

- Learn about [authorizing payments](./first-payment.md) after CreateOrder
- Explore [error handling](./first-payment.md#error-handling)
- See [all payment flows](./extending-to-more-flows.md)
