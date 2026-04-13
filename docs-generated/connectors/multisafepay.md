# Multisafepay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/multisafepay.json
Regenerate: python3 scripts/generators/docs/generate.py multisafepay
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     multisafepay=payment_pb2.MultisafepayConfig(api_key=...),
# ))

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { ConnectorClient } = require('connector-service-node-ffi');

// Reuse this client for all flows
const client = new ConnectorClient({
    connector: 'Multisafepay',
    environment: 'sandbox',
    connector_auth_type: {
        header_key: { api_key: 'YOUR_API_KEY' },
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setConnector("Multisafepay")
    .setEnvironment(Environment.SANDBOX)
    .setAuth(
        ConnectorAuthType.newBuilder()
            .setHeaderKey(HeaderKey.newBuilder().setApiKey("YOUR_API_KEY"))
    )
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use connector_service_sdk::{ConnectorClient, ConnectorConfig};

let config = ConnectorConfig {
    connector: "Multisafepay".to_string(),
    environment: Environment::Sandbox,
    auth: ConnectorAuth::HeaderKey { api_key: "YOUR_API_KEY".into() },
    ..Default::default()
};
```

</details>

</td>
</tr>
</table>

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L69) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L64) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L84) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L84)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L88) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L83) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L100) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L100)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L143) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L135) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L149) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L151)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L180) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L170) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L171) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L174)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |

### Payments

#### PaymentService.Authorize

Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.

| | Message |
|---|---------|
| **Request** | `PaymentServiceAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | ✓ |
| Apple Pay | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| ACH | ⚠ |
| BECS | ⚠ |
| iDEAL | ✓ |
| PayPal | ✓ |
| BLIK | ⚠ |
| Klarna | ⚠ |
| Afterpay | ⚠ |
| UPI | ⚠ |
| Affirm | ⚠ |
| Samsung Pay | ⚠ |

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### Card (Raw PAN)

```python
"payment_method": {
    "card": {  # Generic card payment
        "card_number": "4111111111111111",  # Card Identification
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "737",
        "card_holder_name": "John Doe"  # Cardholder Information
    }
}
```

##### Google Pay

```python
"payment_method": {
    "google_pay": {  # Google Pay
        "type": "CARD",  # Type of payment method
        "description": "Visa 1111",  # User-facing description of the payment method
        "info": {
            "card_network": "VISA",  # Card network name
            "card_details": "1111"  # Card details (usually last 4 digits)
        },
        "tokenization_data": {
            "encrypted_data": {  # Encrypted Google Pay payment data
                "token_type": "PAYMENT_GATEWAY",  # The type of the token
                "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"  # Token generated for the wallet
            }
        }
    }
}
```

##### iDEAL

```python
"payment_method": {
    "ideal": {
    }
}
```

##### PayPal Redirect

```python
"payment_method": {
    "paypal_redirect": {  # PayPal
        "email": "test@example.com"  # PayPal's email address
    }
}
```

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L202) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L191) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L189) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L192)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L211) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L200) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L201) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L204)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L143) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L135) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L209) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L211)
