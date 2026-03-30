# Payload

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/payload.json
Regenerate: python3 scripts/generators/docs/generate.py payload
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
    // connectorConfig: set your payload credentials here,
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

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [refund](#refund) | Other | `—` |
| [void](#void) | Other | `—` |

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/payload/python/payload.py) · [JavaScript](../../examples/payload/javascript/payload.ts) · [Kotlin](../../examples/payload/kotlin/payload.kt) · [Rust](../../examples/payload/rust/payload.rs#L65)

### Other

#### capture

**Examples:** [Python](../../examples/payload/python/payload.py) · [JavaScript](../../examples/payload/javascript/payload.ts) · [Kotlin](../../examples/payload/kotlin/payload.kt#L24) · [Rust](../../examples/payload/rust/payload.rs#L17)

#### get

**Examples:** [Python](../../examples/payload/python/payload.py) · [JavaScript](../../examples/payload/javascript/payload.ts) · [Kotlin](../../examples/payload/kotlin/payload.kt#L32) · [Rust](../../examples/payload/rust/payload.rs#L41)

#### refund

**Examples:** [Python](../../examples/payload/python/payload.py) · [JavaScript](../../examples/payload/javascript/payload.ts) · [Kotlin](../../examples/payload/kotlin/payload.kt#L48) · [Rust](../../examples/payload/rust/payload.rs#L105)

#### void

**Examples:** [Python](../../examples/payload/python/payload.py) · [JavaScript](../../examples/payload/javascript/payload.ts) · [Kotlin](../../examples/payload/kotlin/payload.kt#L56) · [Rust](../../examples/payload/rust/payload.rs#L131)
