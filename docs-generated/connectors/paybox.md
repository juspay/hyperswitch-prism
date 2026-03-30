# Paybox

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paybox.json
Regenerate: python3 scripts/generators/docs/generate.py paybox
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
        paybox: {
        site: { value: 'YOUR_SITE' },
        rank: { value: 'YOUR_RANK' },
        key: { value: 'YOUR_KEY' },
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
        config: Some(connector_specific_config::Config::Paybox(PayboxConfig {
                site: Some(Secret::new("YOUR_SITE".to_string())),
                rank: Some(Secret::new("YOUR_RANK".to_string())),
                key: Some(Secret::new("YOUR_KEY".to_string())),
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

### Card Payment (Authorize + Capture)

Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/paybox/python/paybox.py#L24) · [JavaScript](../../examples/paybox/javascript/paybox.js#L30) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L23) · [Rust](../../examples/paybox/rust/paybox.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/paybox/python/paybox.py#L60) · [JavaScript](../../examples/paybox/javascript/paybox.js#L86) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L37) · [Rust](../../examples/paybox/rust/paybox.rs#L68)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/paybox/python/paybox.py#L87) · [JavaScript](../../examples/paybox/javascript/paybox.js#L128) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L47) · [Rust](../../examples/paybox/rust/paybox.rs#L105)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/paybox/python/paybox.py#L125) · [JavaScript](../../examples/paybox/javascript/paybox.js#L186) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L61) · [Rust](../../examples/paybox/rust/paybox.rs#L157)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/paybox/python/paybox.py#L161) · [JavaScript](../../examples/paybox/javascript/paybox.js#L238) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L75) · [Rust](../../examples/paybox/rust/paybox.rs#L207)

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

**Examples:** [Python](../../examples/paybox/python/paybox.py) · [JavaScript](../../examples/paybox/javascript/paybox.ts#L288) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L89) · [Rust](../../examples/paybox/rust/paybox.rs#L257)

#### capture

**Examples:** [Python](../../examples/paybox/python/paybox.py) · [JavaScript](../../examples/paybox/javascript/paybox.ts#L326) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L97) · [Rust](../../examples/paybox/rust/paybox.rs#L292)

#### get

**Examples:** [Python](../../examples/paybox/python/paybox.py) · [JavaScript](../../examples/paybox/javascript/paybox.ts#L345) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L105) · [Rust](../../examples/paybox/rust/paybox.rs#L309)

#### refund

**Examples:** [Python](../../examples/paybox/python/paybox.py) · [JavaScript](../../examples/paybox/javascript/paybox.ts#L360) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L113) · [Rust](../../examples/paybox/rust/paybox.rs#L326)

#### void

**Examples:** [Python](../../examples/paybox/python/paybox.py) · [JavaScript](../../examples/paybox/javascript/paybox.ts#L381) · [Kotlin](../../examples/paybox/kotlin/paybox.kt#L121) · [Rust](../../examples/paybox/rust/paybox.rs#L345)
