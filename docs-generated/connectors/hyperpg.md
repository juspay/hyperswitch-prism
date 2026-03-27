# Hyperpg

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/hyperpg.json
Regenerate: python3 scripts/generators/docs/generate.py hyperpg
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
#     hyperpg=payment_pb2.HyperpgConfig(api_key=...),
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
    connector: 'Hyperpg',
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
    .setConnector("Hyperpg")
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
    connector: "Hyperpg".to_string(),
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

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L66) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L61) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L81) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L81)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L85) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L80) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L97) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L97)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L122) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L115) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L119) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L120)

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
| Google Pay | x |
| Apple Pay | x |
| SEPA | ⚠ |
| BACS | ⚠ |
| ACH | ⚠ |
| BECS | ⚠ |
| iDEAL | ⚠ |
| PayPal | x |
| BLIK | ⚠ |
| Klarna | x |
| Afterpay | x |
| UPI | ⚠ |
| Affirm | x |
| Samsung Pay | x |

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### Card (Raw PAN)

```python
"payment_method": {
    "card": {  # Generic card payment
        "card_number": {"value": "4111111111111111"},  # Card Identification
        "card_exp_month": {"value": "03"},
        "card_exp_year": {"value": "2030"},
        "card_cvc": {"value": "737"},
        "card_holder_name": {"value": "John Doe"}  # Cardholder Information
    }
}
```

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L144) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L136) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L137) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L138)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L153) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L145) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L149) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L150)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L85) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L80) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L157) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L157)
