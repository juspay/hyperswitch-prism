# Fiservcommercehub

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiservcommercehub.json
Regenerate: python3 scripts/generators/docs/generate.py fiservcommercehub
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
    #     fiservcommercehub=payment_pb2.FiservcommercehubConfig(api_key=...),
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
    connector: Connector.FISERVCOMMERCEHUB,
    environment: Environment.SANDBOX,
    // auth: { fiservcommercehub: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Fiservcommercehub credentials here
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
| [create_server_authentication_token](#create_server_authentication_token) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### create_server_authentication_token

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L22) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### get

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L32) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### refund

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L47) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### refund_get

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L68) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### void

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)
