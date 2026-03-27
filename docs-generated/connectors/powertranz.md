# Powertranz

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/powertranz.json
Regenerate: python3 scripts/generators/docs/generate.py powertranz
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
        powertranz: {
        powerTranzId: { value: 'YOUR_POWER_TRANZ_ID' },
        powerTranzPassword: { value: 'YOUR_POWER_TRANZ_PASSWORD' },
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
        config: Some(connector_specific_config::Config::Powertranz(PowertranzConfig {
                power_tranz_id: Some(Secret::new("YOUR_POWER_TRANZ_ID".to_string())),
                power_tranz_password: Some(Secret::new("YOUR_POWER_TRANZ_PASSWORD".to_string())),
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

### Card Payment (Authorize + Capture)

Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/powertranz/python/powertranz.py#L5) · [JavaScript](../../examples/powertranz/javascript/powertranz.js#L28) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt#L6) · [Rust](../../examples/powertranz/rust/powertranz.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/powertranz/python/powertranz.py#L13) · [JavaScript](../../examples/powertranz/javascript/powertranz.js#L84) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt#L10) · [Rust](../../examples/powertranz/rust/powertranz.rs#L30)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/powertranz/python/powertranz.py#L19) · [JavaScript](../../examples/powertranz/javascript/powertranz.js#L126) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt#L14) · [Rust](../../examples/powertranz/rust/powertranz.rs#L39)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/powertranz/python/powertranz.py#L27) · [JavaScript](../../examples/powertranz/javascript/powertranz.js#L184) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt#L18) · [Rust](../../examples/powertranz/rust/powertranz.rs#L51)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/powertranz/python/powertranz.py#L35) · [JavaScript](../../examples/powertranz/javascript/powertranz.js#L232) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt#L22) · [Rust](../../examples/powertranz/rust/powertranz.rs#L63)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
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

**Examples:** [Python](../../examples/powertranz/python/powertranz.py) · [JavaScript](../../examples/powertranz/javascript/powertranz.ts#L282) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt) · [Rust](../../examples/powertranz/rust/powertranz.rs#L75)

#### capture

**Examples:** [Python](../../examples/powertranz/python/powertranz.py) · [JavaScript](../../examples/powertranz/javascript/powertranz.ts#L320) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt) · [Rust](../../examples/powertranz/rust/powertranz.rs#L107)

#### get

**Examples:** [Python](../../examples/powertranz/python/powertranz.py) · [JavaScript](../../examples/powertranz/javascript/powertranz.ts#L339) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt) · [Rust](../../examples/powertranz/rust/powertranz.rs#L124)

#### refund

**Examples:** [Python](../../examples/powertranz/python/powertranz.py) · [JavaScript](../../examples/powertranz/javascript/powertranz.ts#L354) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt) · [Rust](../../examples/powertranz/rust/powertranz.rs#L141)

#### void

**Examples:** [Python](../../examples/powertranz/python/powertranz.py) · [JavaScript](../../examples/powertranz/javascript/powertranz.ts#L375) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt) · [Rust](../../examples/powertranz/rust/powertranz.rs#L160)
