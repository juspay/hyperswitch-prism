# Braintree

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/braintree.json
Regenerate: python3 scripts/generators/docs/generate.py braintree
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
#     braintree=payment_pb2.BraintreeConfig(api_key=...),
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
    connector: 'Braintree',
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
    .setConnector("Braintree")
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
    connector: "Braintree".to_string(),
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

### Card Payment (Authorize + Capture)

Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/braintree/python/braintree.py#L88) · [JavaScript](../../examples/braintree/javascript/braintree.js#L78) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L88) · [Rust](../../examples/braintree/rust/braintree.rs#L85)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/braintree/python/braintree.py#L113) · [JavaScript](../../examples/braintree/javascript/braintree.js#L104) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L110) · [Rust](../../examples/braintree/rust/braintree.rs#L108)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/braintree/python/braintree.py#L132) · [JavaScript](../../examples/braintree/javascript/braintree.js#L123) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L126) · [Rust](../../examples/braintree/rust/braintree.rs#L124)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/braintree/python/braintree.py#L154) · [JavaScript](../../examples/braintree/javascript/braintree.js#L145) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L145) · [Rust](../../examples/braintree/rust/braintree.rs#L143)

### Tokenize Payment Method

Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.

**Examples:** [Python](../../examples/braintree/python/braintree.py#L176) · [JavaScript](../../examples/braintree/javascript/braintree.js#L167) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L164) · [Rust](../../examples/braintree/rust/braintree.rs#L162)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

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
| Google Pay | ⚠ |
| Apple Pay | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| ACH | ⚠ |
| BECS | ⚠ |
| iDEAL | ⚠ |
| PayPal | ⚠ |
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

**Examples:** [Python](../../examples/braintree/python/braintree.py#L210) · [JavaScript](../../examples/braintree/javascript/braintree.js#L195) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L192) · [Rust](../../examples/braintree/rust/braintree.rs#L191)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/braintree/python/braintree.py#L219) · [JavaScript](../../examples/braintree/javascript/braintree.js#L204) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L204) · [Rust](../../examples/braintree/rust/braintree.rs#L203)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/braintree/python/braintree.py#L228) · [JavaScript](../../examples/braintree/javascript/braintree.js#L213) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L214) · [Rust](../../examples/braintree/rust/braintree.rs#L210)

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/braintree/python/braintree.py#L176) · [JavaScript](../../examples/braintree/javascript/braintree.js#L167) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L222) · [Rust](../../examples/braintree/rust/braintree.rs#L217)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/braintree/python/braintree.py#L237) · [JavaScript](../../examples/braintree/javascript/braintree.js#L222) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L248) · [Rust](../../examples/braintree/rust/braintree.rs#L244)
