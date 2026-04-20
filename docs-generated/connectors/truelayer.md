# Truelayer

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/truelayer.json
Regenerate: python3 scripts/generators/docs/generate.py truelayer
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
    #     truelayer=payment_pb2.TruelayerConfig(api_key=...),
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
    connector: Connector.TRUELAYER,
    environment: Environment.SANDBOX,
    // auth: { truelayer: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Truelayer credentials here
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
| [handle_event](#handle_event) | Other | `—` |
| [parse_event](#parse_event) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |

### Other

#### create_server_authentication_token

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L22) · [Kotlin](../../examples/truelayer/truelayer.kt) · [Rust](../../examples/truelayer/truelayer.rs)

#### get

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L32) · [Kotlin](../../examples/truelayer/truelayer.kt) · [Rust](../../examples/truelayer/truelayer.rs)

#### handle_event

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L47) · [Kotlin](../../examples/truelayer/truelayer.kt) · [Rust](../../examples/truelayer/truelayer.rs)

#### parse_event

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L59) · [Kotlin](../../examples/truelayer/truelayer.kt) · [Rust](../../examples/truelayer/truelayer.rs)

#### refund_get

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L70) · [Kotlin](../../examples/truelayer/truelayer.kt) · [Rust](../../examples/truelayer/truelayer.rs)
