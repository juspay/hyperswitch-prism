# Razorpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/razorpay.json
Regenerate: python3 scripts/generators/docs/generate.py razorpay
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Javascript</b></td><td><b>Kotlin</b></td><td><b>Python</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Javascript</summary>

```javascript
import { DirectPaymentClient, types } from 'hyperswitch-prism';

const config: types.IConnectorConfig = types.ConnectorConfig.create({
    options: types.SdkOptions.create({ environment: types.Environment.SANDBOX }),
    // connectorConfig: set your razorpay credentials here,
});
const client = new DirectPaymentClient(config);
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
import payments.DirectPaymentClient
import payments.ConnectorConfig
import payments.Environment

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = DirectPaymentClient(config)
```

</details>

</td>
<td valign="top">

<details><summary>Python</summary>

```python
from payments import PaymentClient
from payments.generated import sdk_config_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
client = PaymentClient(config)
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;

let config = ConnectorConfig {
    connector_config: None,  // TODO: set connector credentials
    options: Some(SdkOptions { environment: Environment::Sandbox.into() }),
};
let client = ConnectorClient::new(config, None).unwrap();
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

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L24) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L25) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L24) · [Rust](../../examples/razorpay/rust/razorpay.rs#L17)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L66) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L86) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L38) · [Rust](../../examples/razorpay/rust/razorpay.rs#L72)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L99) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L133) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L48) · [Rust](../../examples/razorpay/rust/razorpay.rs#L114)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L143) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L196) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L62) · [Rust](../../examples/razorpay/rust/razorpay.rs#L171)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [create_order](#create_order) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |

### Other

#### authorize

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
| UPI | ✓ |
| Affirm | ⚠ |
| Samsung Pay | ⚠ |

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

##### UPI Collect

```python
"payment_method": {
    "upi_collect": {  # UPI Collect
        "vpa_id": {"value": "test@upi"}  # Virtual Payment Address
    }
}
```

**Examples:** [Python](../../examples/razorpay/python/razorpay.py) · [JavaScript](../../examples/razorpay/javascript/razorpay.ts#L251) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L76) · [Rust](../../examples/razorpay/rust/razorpay.rs#L226)

#### capture

**Examples:** [Python](../../examples/razorpay/python/razorpay.py) · [JavaScript](../../examples/razorpay/javascript/razorpay.ts#L294) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L84) · [Rust](../../examples/razorpay/rust/razorpay.rs#L266)

#### create_order

**Examples:** [Python](../../examples/razorpay/python/razorpay.py) · [JavaScript](../../examples/razorpay/javascript/razorpay.ts#L313) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt) · [Rust](../../examples/razorpay/rust/razorpay.rs#L283)

#### get

**Examples:** [Python](../../examples/razorpay/python/razorpay.py) · [JavaScript](../../examples/razorpay/javascript/razorpay.ts#L327) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L100) · [Rust](../../examples/razorpay/rust/razorpay.rs#L299)

#### refund

**Examples:** [Python](../../examples/razorpay/python/razorpay.py) · [JavaScript](../../examples/razorpay/javascript/razorpay.ts#L342) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L108) · [Rust](../../examples/razorpay/rust/razorpay.rs#L316)
