# Redsys

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/redsys.json
Regenerate: python3 scripts/generators/docs/generate.py redsys
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     redsys=payment_pb2.RedsysConfig(api_key=...),
    # ),
)

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { PaymentClient } = require('hyperswitch-prism');
const { ConnectorConfig, Environment, Connector } = require('hyperswitch-prism').types;

const config = ConnectorConfig.create({
    connector: Connector.REDSYS,
    environment: Environment.SANDBOX,
    // auth: { redsys: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Redsys credentials here
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;

let config = ConnectorConfig {
    connector_config: None,  // TODO: Add your connector config here,
    options: Some(SdkOptions {
        environment: Environment::Sandbox.into(),
    }),
};
```

</details>

</td>
</tr>
</table>

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authenticate](#authenticate) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [pre_authenticate](#pre_authenticate) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### authenticate

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L22) · [Kotlin](../../examples/redsys/redsys.kt) · [Rust](../../examples/redsys/redsys.rs)

#### capture

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L43) · [Kotlin](../../examples/redsys/redsys.kt) · [Rust](../../examples/redsys/redsys.rs)

#### get

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L60) · [Kotlin](../../examples/redsys/redsys.kt) · [Rust](../../examples/redsys/redsys.rs)

#### pre_authenticate

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L73) · [Kotlin](../../examples/redsys/redsys.kt) · [Rust](../../examples/redsys/redsys.rs)

#### refund

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L90) · [Kotlin](../../examples/redsys/redsys.kt) · [Rust](../../examples/redsys/redsys.rs)

#### refund_get

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L109) · [Kotlin](../../examples/redsys/redsys.kt) · [Rust](../../examples/redsys/redsys.rs)

#### void

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts) · [Kotlin](../../examples/redsys/redsys.kt) · [Rust](../../examples/redsys/redsys.rs)
