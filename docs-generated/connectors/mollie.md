# Mollie

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/mollie.json
Regenerate: python3 scripts/generators/docs/generate.py mollie
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
    connectorConfig: types.ConnectorSpecificConfig.create({
        mollie: {
        apiKey: { value: 'YOUR_API_KEY' },
        },
    }),
});
const client = new DirectPaymentClient(config);
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
import payments.PaymentClient
import payments.ConnectorConfig

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = PaymentClient(config)
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
use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;

let config = ConnectorConfig {
    connector_config: Some(ConnectorSpecificConfig {
        config: Some(connector_specific_config::Config::Mollie(MollieConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
            ..Default::default()
        })),
    }),
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

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/mollie/python/mollie.py#L5) · [JavaScript](../../examples/mollie/javascript/mollie.js#L27) · [Kotlin](../../examples/mollie/kotlin/mollie.kt#L6) · [Rust](../../examples/mollie/rust/mollie.rs#L18)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/mollie/python/mollie.py#L11) · [JavaScript](../../examples/mollie/javascript/mollie.js#L70) · [Kotlin](../../examples/mollie/kotlin/mollie.kt#L10) · [Rust](../../examples/mollie/rust/mollie.rs#L27)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/mollie/python/mollie.py#L19) · [JavaScript](../../examples/mollie/javascript/mollie.js#L129) · [Kotlin](../../examples/mollie/kotlin/mollie.kt#L14) · [Rust](../../examples/mollie/rust/mollie.rs#L39)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/mollie/python/mollie.py#L27) · [JavaScript](../../examples/mollie/javascript/mollie.js#L178) · [Kotlin](../../examples/mollie/kotlin/mollie.kt#L18) · [Rust](../../examples/mollie/rust/mollie.rs#L51)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | x |
| Apple Pay | x |
| SEPA | x |
| BACS | x |
| ACH | x |
| BECS | x |
| iDEAL | x |
| PayPal | x |
| BLIK | x |
| Klarna | x |
| Afterpay | x |
| UPI | x |
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

**Examples:** [Python](../../examples/mollie/python/mollie.py) · [JavaScript](../../examples/mollie/javascript/mollie.ts#L229) · [Kotlin](../../examples/mollie/kotlin/mollie.kt) · [Rust](../../examples/mollie/rust/mollie.rs#L63)

#### get

**Examples:** [Python](../../examples/mollie/python/mollie.py) · [JavaScript](../../examples/mollie/javascript/mollie.ts#L268) · [Kotlin](../../examples/mollie/kotlin/mollie.kt) · [Rust](../../examples/mollie/rust/mollie.rs#L96)

#### refund

**Examples:** [Python](../../examples/mollie/python/mollie.py) · [JavaScript](../../examples/mollie/javascript/mollie.ts#L283) · [Kotlin](../../examples/mollie/kotlin/mollie.kt) · [Rust](../../examples/mollie/rust/mollie.rs#L113)

#### void

**Examples:** [Python](../../examples/mollie/python/mollie.py) · [JavaScript](../../examples/mollie/javascript/mollie.ts#L304) · [Kotlin](../../examples/mollie/kotlin/mollie.kt) · [Rust](../../examples/mollie/rust/mollie.rs#L132)
