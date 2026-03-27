# Payme

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/payme.json
Regenerate: python3 scripts/generators/docs/generate.py payme
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
        payme: {
        sellerPaymeId: { value: 'YOUR_SELLER_PAYME_ID' },
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
        config: Some(connector_specific_config::Config::Payme(PaymeConfig {
                seller_payme_id: Some(Secret::new("YOUR_SELLER_PAYME_ID".to_string())),
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

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [capture](#capture) | Other | `—` |
| [create_order](#create_order) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### capture

**Examples:** [Python](../../examples/payme/python/payme.py) · [JavaScript](../../examples/payme/javascript/payme.ts) · [Kotlin](../../examples/payme/kotlin/payme.kt) · [Rust](../../examples/payme/rust/payme.rs#L18)

#### create_order

**Examples:** [Python](../../examples/payme/python/payme.py) · [JavaScript](../../examples/payme/javascript/payme.ts) · [Kotlin](../../examples/payme/kotlin/payme.kt) · [Rust](../../examples/payme/rust/payme.rs#L35)

#### get

**Examples:** [Python](../../examples/payme/python/payme.py) · [JavaScript](../../examples/payme/javascript/payme.ts) · [Kotlin](../../examples/payme/kotlin/payme.kt) · [Rust](../../examples/payme/rust/payme.rs#L51)

#### refund

**Examples:** [Python](../../examples/payme/python/payme.py) · [JavaScript](../../examples/payme/javascript/payme.ts) · [Kotlin](../../examples/payme/kotlin/payme.kt) · [Rust](../../examples/payme/rust/payme.rs#L68)

#### void

**Examples:** [Python](../../examples/payme/python/payme.py) · [JavaScript](../../examples/payme/javascript/payme.ts) · [Kotlin](../../examples/payme/kotlin/payme.kt) · [Rust](../../examples/payme/rust/payme.rs#L87)
