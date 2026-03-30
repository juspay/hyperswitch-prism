# Hyperpg

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/hyperpg.json
Regenerate: python3 scripts/generators/docs/generate.py hyperpg
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
        hyperpg: {
        username: { value: 'YOUR_USERNAME' },
        password: { value: 'YOUR_PASSWORD' },
        merchantId: { value: 'YOUR_MERCHANT_ID' },
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
use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;

let config = ConnectorConfig {
    connector_config: Some(ConnectorSpecificConfig {
        config: Some(connector_specific_config::Config::Hyperpg(HyperpgConfig {
                username: Some(Secret::new("YOUR_USERNAME".to_string())),
                password: Some(Secret::new("YOUR_PASSWORD".to_string())),
                merchant_id: Some(Secret::new("YOUR_MERCHANT_ID".to_string())),
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

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L22) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L29) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L21) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L18)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L49) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L71) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L31) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L55)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py#L87) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js#L129) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L45) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L107)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |

### Other

#### authorize

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

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.ts#L180) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L59) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L158)

#### get

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.ts#L218) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L67) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L193)

#### refund

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.ts#L234) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt#L75) · [Rust](../../examples/hyperpg/rust/hyperpg.rs#L211)
