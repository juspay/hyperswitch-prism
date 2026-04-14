# First Payment

In the next few steps you will authorize the payment, handle errors, capture funds, and process refunds. And then you will be ready to send payment to any payment processor, without writing specialized code for each.

If you are **not PCI compliant**, first create a Stripe client authentication token on your backend, use the returned `client_secret` to initialize Stripe.js / Stripe Elements on the frontend, tokenize the payment method in the browser, and then use the resulting `payment_method_id` to authorize the payment.

If your payment processor API keys are enabled to accept PCI compliant raw card data directly, jump to [Authorize with Raw Card Details](#authorize-with-raw-card-details-pci-compliant).

## Non-PCI Stripe flow: get the `payment_method_id` first

### 1. Create a client authentication token on your backend

Use Prism's client authentication token flow to fetch the Stripe `client_secret` required by Stripe.js.

{% tabs %}

{% tab title="Node.js" %}

```javascript
const {
  MerchantAuthenticationClient,
  IntegrationError,
  ConnectorError,
  NetworkError,
  types,
} = require("hyperswitch-prism");

// Reuse stripeConfig from installation.md
const authClient = new MerchantAuthenticationClient(stripeConfig);

async function createClientAuthenticationToken() {
  try {
    const response = await authClient.createClientAuthenticationToken({
      merchantClientSessionId: "client_session_001",
      payment: {
        amount: {
          minorAmount: 1000,
          currency: types.Currency.USD,
        },
      },
      testMode: true,
    });

    const clientSecret =
      response.sessionData?.connectorSpecific?.stripe?.clientSecret?.value;

    console.log("Client secret:", clientSecret);
    return clientSecret;
  } catch (error) {
    if (error instanceof IntegrationError) {
      console.error(
        "Integration error:",
        error.errorCode,
        error.message,
      );
    } else if (error instanceof ConnectorError) {
      console.error(
        "Connector error:",
        error.errorCode,
        error.message,
      );
    } else if (error instanceof NetworkError) {
      console.error("Network error:", error.errorCode, error.message);
    } else {
      console.error(
        "Client auth token creation failed:",
        error.message || error,
      );
    }
    throw error;
  }
}
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import (
    MerchantAuthenticationClient,
    IntegrationError,
    ConnectorError,
    NetworkError,
)

# Reuse stripe_config from installation.md
auth_client = MerchantAuthenticationClient(stripe_config)

def create_client_authentication_token():
    try:
        response = auth_client.create_client_authentication_token({
            "merchant_client_session_id": "client_session_001",
            "payment": {
                "amount": {
                    "minor_amount": 1000,
                    "currency": "USD"
                }
            },
            "test_mode": True
        })

        client_secret = response.session_data.connector_specific.stripe.client_secret.value
        print(f"Client secret: {client_secret}")
        return client_secret
    except IntegrationError as error:
        print(f"Integration error: {error.error_code} - {error.error_message}")
        raise
    except ConnectorError as error:
        print(f"Connector error: {error.error_code} - {error.error_message}")
        raise
    except NetworkError as error:
        print(f"Network error: {error.error_code} - {error}")
        raise
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.ConnectorError;
import payments.IntegrationError;
import payments.MerchantAuthenticationClient;
import payments.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest;
import payments.NetworkError;

// Reuse stripeConfig from installation.md
MerchantAuthenticationClient authClient = new MerchantAuthenticationClient(stripeConfig);

public String createClientAuthenticationToken() {
    try {
        MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest request =
            MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest.newBuilder()
                .setMerchantClientSessionId("client_session_001")
                .setPayment(types.Payment.PaymentClientAuthenticationContext.newBuilder()
                    .setAmount(payments.Money.newBuilder()
                        .setMinorAmount(1000L)
                        .setCurrency(payments.Currency.USD)
                        .build())
                    .build())
                .setTestMode(true)
                .build();

        var response = authClient.create_client_authentication_token(request);
        String clientSecret = response.getSessionData()
            .getConnectorSpecific()
            .getStripe()
            .getClientSecret()
            .getValue();

        System.out.println("Client secret: " + clientSecret);
        return clientSecret;
    } catch (IntegrationError error) {
        System.err.println("Integration error: " + error.getErrorCode() + " - " + error.getMessage());
        throw error;
    } catch (ConnectorError error) {
        System.err.println("Connector error: " + error.getErrorCode() + " - " + error.getMessage());
        throw error;
    } catch (NetworkError error) {
        System.err.println("Network error: " + error.getErrorCode() + " - " + error.getMessage());
        throw error;
    }
}
```

{% endtab %}

{% tab title="PHP" %}

```php
<?php
use HyperswitchPrism\MerchantAuthenticationClient;

// Reuse $stripeConfig from installation.md
$authClient = new MerchantAuthenticationClient($stripeConfig);

function createClientAuthenticationToken($authClient) {
    try {
        $response = $authClient->createClientAuthenticationToken([
            'merchantClientSessionId' => 'client_session_001',
            'payment' => [
                'amount' => [
                    'minorAmount' => 1000,
                    'currency' => 'USD'
                ]
            ],
            'testMode' => true
        ]);

        $clientSecret = $response->getSessionData()
            ->getConnectorSpecific()
            ->getStripe()
            ->getClientSecret()
            ->getValue();

        echo "Client secret: " . $clientSecret . "\n";
        return $clientSecret;
    } catch (\Throwable $error) {
        echo "Client auth token creation failed: " . $error->getMessage() . "\n";
        throw $error;
    }
}
```

{% endtab %}

{% endtabs %}

### 2. Use the `client_secret` in Stripe.js / Stripe Elements

Initialize Stripe Elements on the frontend, collect the card details there, and let Stripe return a tokenized `payment_method_id`.

```html
<script src="https://js.stripe.com/v3/"></script>
<div id="card-element"></div>
<button id="submit">Pay</button>

<script>
  const stripe = Stripe(window.STRIPE_PUBLISHABLE_KEY); // pk_test_xxx
  const elements = stripe.elements({ clientSecret });
  const cardElement = elements.create("card");
  cardElement.mount("#card-element");

  document.getElementById("submit").addEventListener("click", async () => {
    const { error, paymentMethod } = await stripe.createPaymentMethod({
      type: "card",
      card: cardElement,
    });

    if (error) {
      console.error(error.message);
      return;
    }

    // Send paymentMethod.id to your backend
    console.log(paymentMethod.id); // pm_1234...
  });
</script>
```

## Authorize with Payment Method ID

Use the `payment_method_id` returned by Stripe.js / Stripe Elements to authorize the payment:

{% tabs %}

{% tab title="Node.js" %}

```javascript
const {
  IntegrationError,
  ConnectorError,
  NetworkError,
} = require("hyperswitch-prism");

async function authorizePayment(paymentMethodId) {
  try {
    const auth = await stripeClient.tokenAuthorize({
      merchantTransactionId: "txn_tokenized_001",
      merchantOrderId: "order-456",
      amount: {
        minorAmount: 1000,
        currency: "USD",
      },
      connectorToken: {
        value: paymentMethodId, // e.g. pm_1234...
      },
      address: {
        billingAddress: {},
      },
      captureMethod: "MANUAL",
      description: "First tokenized payment",
      testMode: true,
    });

    console.log("Authorized:", auth.connectorTransactionId, auth.status);
    return auth;
  } catch (error) {
    if (error instanceof IntegrationError) {
      console.error(
        "Integration error:",
        error.errorCode,
        error.message,
      );
    } else if (error instanceof ConnectorError) {
      console.error(
        "Connector error:",
        error.errorCode,
        error.message,
      );
    } else if (error instanceof NetworkError) {
      console.error("Network error:", error.errorCode, error.message);
    } else {
      console.error("Authorization failed:", error.message || error);
    }
    throw error;
  }
}
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import (
    IntegrationError,
    ConnectorError,
    NetworkError,
)

def authorize_payment(payment_method_id):
    try:
        auth = stripe_client.token_authorize({
            "merchant_transaction_id": "txn_tokenized_001",
            "merchant_order_id": "order-456",
            "amount": {
                "minor_amount": 1000,
                "currency": "USD"
            },
            "connector_token": {
                "value": payment_method_id  # e.g. pm_1234...
            },
            "address": {
                "billing_address": {}
            },
            "capture_method": "MANUAL",
            "description": "First tokenized payment",
            "test_mode": True
        })

        print(f"Authorized: {auth.connector_transaction_id}, {auth.status}")
        return auth
    except IntegrationError as error:
        print(f"Integration error: {error.error_code} - {error.error_message}")
        raise
    except ConnectorError as error:
        print(f"Connector error: {error.error_code} - {error.error_message}")
        raise
    except NetworkError as error:
        print(f"Network error: {error.error_code} - {error}")
        raise
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.Address;
import payments.CaptureMethod;
import payments.ConnectorError;
import payments.Currency;
import payments.IntegrationError;
import payments.NetworkError;
import payments.PaymentAddress;
import payments.PaymentServiceTokenAuthorizeRequest;
import payments.SecretString;

public void authorizePayment(String paymentMethodId) {
    try {
        PaymentServiceTokenAuthorizeRequest request = PaymentServiceTokenAuthorizeRequest.newBuilder()
            .setMerchantTransactionId("txn_tokenized_001")
            .setMerchantOrderId("order-456")
            .setAmount(payments.Money.newBuilder()
                .setMinorAmount(1000L)
                .setCurrency(Currency.USD)
                .build())
            .setConnectorToken(SecretString.newBuilder().setValue(paymentMethodId).build())
            .setAddress(PaymentAddress.newBuilder()
                .setBillingAddress(Address.newBuilder().build())
                .build())
            .setCaptureMethod(CaptureMethod.MANUAL)
            .setDescription("First tokenized payment")
            .setTestMode(true)
            .build();

        var auth = stripeClient.token_authorize(request);
        System.out.println("Authorized: " + auth.getConnectorTransactionId() + ", " + auth.getStatus());
    } catch (IntegrationError error) {
        System.err.println("Integration error: " + error.getErrorCode() + " - " + error.getMessage());
        throw error;
    } catch (ConnectorError error) {
        System.err.println("Connector error: " + error.getErrorCode() + " - " + error.getMessage());
        throw error;
    } catch (NetworkError error) {
        System.err.println("Network error: " + error.getErrorCode() + " - " + error.getMessage());
        throw error;
    }
}
```

{% endtab %}

{% tab title="PHP" %}

```php
<?php
function authorizePayment($paymentMethodId, $stripeClient) {
    try {
        $auth = $stripeClient->tokenAuthorize([
            'merchantTransactionId' => 'txn_tokenized_001',
            'merchantOrderId' => 'order-456',
            'amount' => [
                'minorAmount' => 1000,
                'currency' => 'USD'
            ],
            'connectorToken' => [
                'value' => $paymentMethodId // e.g. pm_1234...
            ],
            'address' => [
                'billingAddress' => []
            ],
            'captureMethod' => 'MANUAL',
            'description' => 'First tokenized payment',
            'testMode' => true
        ]);

        echo "Authorized: " . $auth->getConnectorTransactionId() . ", " . $auth->getStatus() . "\n";
        return $auth;
    } catch (\Throwable $error) {
        echo "Authorization failed: " . $error->getMessage() . "\n";
        throw $error;
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
// Reuse stripeClient from installation.md
const auth = await stripeClient.authorize({
  merchantTransactionId: "txn_raw_card_001",
  merchantOrderId: "order-456",
  amount: {
    minorAmount: 1000,
    currency: "USD",
  },
  paymentMethod: {
    card: {
      cardNumber: { value: "4242424242424242" },
      cardExpMonth: { value: "12" },
      cardExpYear: { value: "2027" },
      cardCvc: { value: "123" },
      cardHolderName: { value: "Jane Doe" },
    },
  },
  authType: "NO_THREE_DS",
  address: {},
  captureMethod: "AUTOMATIC",
  testMode: true,
});
```

{% endtab %}

{% tab title="Python" %}

```python
# Reuse stripe_client from installation.md
auth = stripe_client.authorize({
    "merchant_transaction_id": "txn_raw_card_001",
    "merchant_order_id": "order-456",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": {"value": "4242424242424242"},
            "card_exp_month": {"value": "12"},
            "card_exp_year": {"value": "2027"},
            "card_cvc": {"value": "123"},
            "card_holder_name": {"value": "Jane Doe"}
        }
    },
    "auth_type": "NO_THREE_DS",
    "address": {},
    "capture_method": "AUTOMATIC",
    "test_mode": True
})
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.AuthenticationType;
import payments.CaptureMethod;
import payments.Currency;
import payments.PaymentServiceAuthorizeRequest;

// Reuse stripeClient from installation.md
PaymentServiceAuthorizeRequest.Builder requestBuilder = PaymentServiceAuthorizeRequest.newBuilder();
requestBuilder.setMerchantTransactionId("txn_raw_card_001");
requestBuilder.setMerchantOrderId("order-456");
requestBuilder.getAmountBuilder().setMinorAmount(1000L).setCurrency(Currency.USD);
requestBuilder.getPaymentMethodBuilder().getCardBuilder().getCardNumberBuilder().setValue("4242424242424242");
requestBuilder.getPaymentMethodBuilder().getCardBuilder().getCardExpMonthBuilder().setValue("12");
requestBuilder.getPaymentMethodBuilder().getCardBuilder().getCardExpYearBuilder().setValue("2027");
requestBuilder.getPaymentMethodBuilder().getCardBuilder().getCardCvcBuilder().setValue("123");
requestBuilder.getPaymentMethodBuilder().getCardBuilder().getCardHolderNameBuilder().setValue("Jane Doe");
requestBuilder.getAddressBuilder().getBillingAddressBuilder();
requestBuilder.setAuthType(AuthenticationType.NO_THREE_DS);
requestBuilder.setCaptureMethod(CaptureMethod.AUTOMATIC);
requestBuilder.setTestMode(true);

var auth = stripeClient.authorize(requestBuilder.build());
```

{% endtab %}

{% tab title="PHP" %}

```php
<?php
// Reuse $stripeClient from installation.md
$auth = $stripeClient->authorize([
    'merchantTransactionId' => 'txn_raw_card_001',
    'merchantOrderId' => 'order-456',
    'amount' => [
        'minorAmount' => 1000,
        'currency' => 'USD'
    ],
    'paymentMethod' => [
        'card' => [
            'cardNumber' => ['value' => '4242424242424242'],
            'cardExpMonth' => ['value' => '12'],
            'cardExpYear' => ['value' => '2027'],
            'cardCvc' => ['value' => '123'],
            'cardHolderName' => ['value' => 'Jane Doe']
        ]
    ],
    'authType' => 'NO_THREE_DS',
    'address' => [],
    'captureMethod' => 'AUTOMATIC',
    'testMode' => true
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
const status = await stripeClient.get({
  connectorTransactionId: auth.connectorTransactionId,
  testMode: true,
});
console.log("Current status:", status.status);

// 2. Capture the funds (when order ships)
const capture = await stripeClient.capture({
  merchantCaptureId: "capture_001",
  connectorTransactionId: auth.connectorTransactionId,
  amountToCapture: { minorAmount: 1000, currency: "USD" },
  testMode: true,
});
console.log("Captured:", capture.status);

// 3. Process a partial refund (customer returns item)
const refund = await stripeClient.refund({
  merchantRefundId: "refund_001",
  connectorTransactionId: auth.connectorTransactionId,
  refundAmount: { minorAmount: 500, currency: "USD" },
  reason: "Customer return",
});
console.log("Refund ID:", refund.connectorRefundId);
```

{% endtab %}

{% tab title="Python" %}

```python
# 1. Check payment status
status = stripe_client.get({
    "merchant_transaction_id": "txn_tokenized_001",
    "connector_transaction_id": auth.connector_transaction_id
})
print(f"Current status: {status.status}")

# 2. Capture the funds
capture = stripe_client.capture({
    "merchant_transaction_id": "txn_tokenized_001",
    "connector_transaction_id": auth.connector_transaction_id,
    "amount": {"minor_amount": 1000, "currency": "USD"}
})
print(f"Captured: {capture.status}")

# 3. Process a partial refund
refund = stripe_client.refund({
    "merchant_refund_id": "refund_001",
    "connector_transaction_id": auth.connector_transaction_id,
    "amount": {"minor_amount": 500, "currency": "USD"},
    "reason": "Customer return"
})
print(f"Refund ID: {refund.connector_refund_id}")
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.PaymentServiceCaptureRequest;
import payments.PaymentServiceGetRequest;
import payments.PaymentServiceRefundRequest;

// 1. Check payment status
PaymentServiceGetRequest getRequest = PaymentServiceGetRequest.newBuilder()
    .setMerchantTransactionId("txn_tokenized_001")
    .setConnectorTransactionId(auth.getConnectorTransactionId())
    .build();
var status = stripeClient.get(getRequest);
System.out.println("Status: " + status.getStatus());

// 2. Capture the funds
PaymentServiceCaptureRequest captureRequest = PaymentServiceCaptureRequest.newBuilder()
    .setMerchantCaptureId("capture_001")
    .setConnectorTransactionId(auth.getConnectorTransactionId())
    .setAmountToCapture(payments.Money.newBuilder().setMinorAmount(1000L).setCurrency(payments.Currency.USD).build())
    .build();
var capture = stripeClient.capture(captureRequest);
System.out.println("Captured: " + capture.getStatus());

// 3. Process a partial refund
PaymentServiceRefundRequest refundRequest = PaymentServiceRefundRequest.newBuilder()
    .setMerchantRefundId("refund_001")
    .setConnectorTransactionId(auth.getConnectorTransactionId())
    .setPaymentAmount(1000L)
    .setRefundAmount(payments.Money.newBuilder().setMinorAmount(500L).setCurrency(payments.Currency.USD).build())
    .setReason("Customer return")
    .build();
var refund = stripeClient.refund(refundRequest);
System.out.println("Refund ID: " + refund.getConnectorRefundId());
```

{% endtab %}

{% tab title="PHP" %}

```php
// 1. Check payment status
$status = $stripeClient->get([
    'merchantTransactionId' => 'txn_tokenized_001',
    'connectorTransactionId' => $auth->getConnectorTransactionId()
]);
echo "Status: " . $status->getStatus() . "\n";

// 2. Capture the funds
$capture = $stripeClient->capture([
    'merchantCaptureId' => 'capture_001',
    'connectorTransactionId' => $auth->getConnectorTransactionId(),
    'amountToCapture' => ['minorAmount' => 1000, 'currency' => 'USD']
]);
echo "Captured: " . $capture->status . "\n";

// 3. Process a partial refund
$refund = $stripeClient->refund([
    'merchantRefundId' => 'refund_001',
    'connectorTransactionId' => $auth->getConnectorTransactionId(),
    'refundAmount' => ['minorAmount' => 500, 'currency' => 'USD'],
    'reason' => 'Customer return'
]);
echo "Refund ID: " . $refund->getConnectorRefundId() . "\n";
```

{% endtab %}

{% endtabs %}

## Error Scenarios

### Declined Card

```javascript
// Using test card: 4000000000000002 (declined)
const auth = await stripeClient.authorize({
  merchantTransactionId: "txn_declined_001",
  amount: { minorAmount: 1000, currency: "USD" },
  paymentMethod: {
    card: {
      cardNumber: { value: "4000000000000002" },
      cardExpMonth: { value: "12" },
      cardExpYear: { value: "2027" },
      cardCvc: { value: "123" },
      cardHolderName: { value: "Jane Doe" },
    },
  },
  authType: "NO_THREE_DS",
  address: {},
  captureMethod: "MANUAL",
  testMode: true,
});
```

### Network Timeout

```javascript
const { NetworkError } = require('hyperswitch-prism');

try {
  const auth = await stripeClient.authorize(request);
} catch (error) {
  if (error.errorCode === "CONNECT_TIMEOUT_EXCEEDED") {
    // Retry with exponential backoff
    await retryWithBackoff(() => stripeClient.authorize(request));
  }
}
```

## Business Use Cases

### E-commerce: Two-Step Flow

Authorize at checkout. Capture when you ship.

```javascript
// Assume paymentMethodId came from Stripe.js
const auth = await stripeClient.tokenAuthorize({
  merchantTransactionId: "txn_checkout_001",
  amount: { minorAmount: 9999, currency: "USD" },
  connectorToken: { value: paymentMethodId },
  address: { billingAddress: {} },
  captureMethod: "MANUAL",
  testMode: true,
});

// Later: when order ships
await stripeClient.capture({
  merchantCaptureId: "capture_checkout_001",
  connectorTransactionId: auth.connectorTransactionId,
  amountToCapture: { minorAmount: 9999, currency: "USD" },
});
```

### SaaS: Immediate Capture

For digital goods, capture immediately.

```javascript
// Assume paymentMethodId came from Stripe.js
const payment = await stripeClient.tokenAuthorize({
  merchantTransactionId: "txn_saas_001",
  amount: { minorAmount: 2900, currency: "USD" },
  connectorToken: { value: paymentMethodId },
  address: { billingAddress: {} },
  captureMethod: "AUTOMATIC",
  testMode: true,
});
// Status: CAPTURED or AUTHORIZED based on connector behavior
```

### Marketplace: Partial Refund

Customer returns one item from a multi-item order.

```javascript
await stripeClient.refund({
  merchantRefundId: "refund_marketplace_001",
  connectorTransactionId: "pi_abc123",
  refundAmount: { minorAmount: 2500, currency: "USD" },
  reason: "Item damaged in shipping",
});
```

## Key Takeaways

- **One error handler** works for all connectors
- **Unified error codes** tell you exactly what happened
- **connectorTransactionId** is the key identifier for all operations
- **Same code** works for Stripe, Adyen, PayPal, and 50+ more

See [extending payment flows](./extend-to-more-flows.md) for subscriptions, 3D Secure, and more.
