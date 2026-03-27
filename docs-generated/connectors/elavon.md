# Elavon

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/elavon.json
Regenerate: python3 scripts/generators/docs/generate.py elavon
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
        elavon: {
        sslMerchantId: { value: 'YOUR_SSL_MERCHANT_ID' },
        sslUserId: { value: 'YOUR_SSL_USER_ID' },
        sslPin: { value: 'YOUR_SSL_PIN' },
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
        config: Some(connector_specific_config::Config::Elavon(ElavonConfig {
                ssl_merchant_id: Some(Secret::new("YOUR_SSL_MERCHANT_ID".to_string())),
                ssl_user_id: Some(Secret::new("YOUR_SSL_USER_ID".to_string())),
                ssl_pin: Some(Secret::new("YOUR_SSL_PIN".to_string())),
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

**Examples:** [Python](../../examples/elavon/python/elavon.py#L5) · [JavaScript](../../examples/elavon/javascript/elavon.js#L29) · [Kotlin](../../examples/elavon/kotlin/elavon.kt#L6) · [Rust](../../examples/elavon/rust/elavon.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/elavon/python/elavon.py#L13) · [JavaScript](../../examples/elavon/javascript/elavon.js#L85) · [Kotlin](../../examples/elavon/kotlin/elavon.kt#L10) · [Rust](../../examples/elavon/rust/elavon.rs#L30)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/elavon/python/elavon.py#L19) · [JavaScript](../../examples/elavon/javascript/elavon.js#L127) · [Kotlin](../../examples/elavon/kotlin/elavon.kt#L14) · [Rust](../../examples/elavon/rust/elavon.rs#L39)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/elavon/python/elavon.py#L27) · [JavaScript](../../examples/elavon/javascript/elavon.js#L185) · [Kotlin](../../examples/elavon/kotlin/elavon.kt#L18) · [Rust](../../examples/elavon/rust/elavon.rs#L51)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
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
| UPI | ⚠ |
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

**Examples:** [Python](../../examples/elavon/python/elavon.py) · [JavaScript](../../examples/elavon/javascript/elavon.ts#L235) · [Kotlin](../../examples/elavon/kotlin/elavon.kt) · [Rust](../../examples/elavon/rust/elavon.rs#L63)

#### capture

**Examples:** [Python](../../examples/elavon/python/elavon.py) · [JavaScript](../../examples/elavon/javascript/elavon.ts#L273) · [Kotlin](../../examples/elavon/kotlin/elavon.kt) · [Rust](../../examples/elavon/rust/elavon.rs#L95)

#### get

**Examples:** [Python](../../examples/elavon/python/elavon.py) · [JavaScript](../../examples/elavon/javascript/elavon.ts#L292) · [Kotlin](../../examples/elavon/kotlin/elavon.kt) · [Rust](../../examples/elavon/rust/elavon.rs#L112)

#### refund

**Examples:** [Python](../../examples/elavon/python/elavon.py) · [JavaScript](../../examples/elavon/javascript/elavon.ts#L307) · [Kotlin](../../examples/elavon/kotlin/elavon.kt) · [Rust](../../examples/elavon/rust/elavon.rs#L129)
