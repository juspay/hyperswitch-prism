# Trustly

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/trustly.json
Regenerate: python3 scripts/generators/docs/generate.py trustly
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
    #     trustly=payment_pb2.TrustlyConfig(api_key=...),
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
    connector: Connector.TRUSTLY,
    environment: Environment.SANDBOX,
    // auth: { trustly: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Trustly credentials here
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
| [handle_event](#handle_event) | Other | `—` |
| [parse_event](#parse_event) | Other | `—` |

### Other

#### handle_event

**Examples:** [Python](../../examples/trustly/trustly.py) · [TypeScript](../../examples/trustly/trustly.ts#L22) · [Kotlin](../../examples/trustly/trustly.kt) · [Rust](../../examples/trustly/trustly.rs)

#### parse_event

**Examples:** [Python](../../examples/trustly/trustly.py) · [TypeScript](../../examples/trustly/trustly.ts#L34) · [Kotlin](../../examples/trustly/trustly.kt) · [Rust](../../examples/trustly/trustly.rs)
