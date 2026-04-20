# Ppro

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/ppro.json
Regenerate: python3 scripts/generators/docs/generate.py ppro
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
    #     ppro=payment_pb2.PproConfig(api_key=...),
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
    connector: Connector.PPRO,
    environment: Environment.SANDBOX,
    // auth: { ppro: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Ppro credentials here
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
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [handle_event](#handle_event) | Other | `—` |
| [parse_event](#parse_event) | Other | `—` |
| [recurring_charge](#recurring_charge) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### capture

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts#L22) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)

#### get

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts#L39) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)

#### handle_event

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts#L52) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)

#### parse_event

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts#L64) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)

#### recurring_charge

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts#L75) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)

#### refund

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts#L98) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)

#### refund_get

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts#L117) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)

#### void

**Examples:** [Python](../../examples/ppro/ppro.py) · [TypeScript](../../examples/ppro/ppro.ts) · [Kotlin](../../examples/ppro/ppro.kt) · [Rust](../../examples/ppro/ppro.rs)
