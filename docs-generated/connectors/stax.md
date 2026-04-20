# Stax

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/stax.json
Regenerate: python3 scripts/generators/docs/generate.py stax
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
    #     stax=payment_pb2.StaxConfig(api_key=...),
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
    connector: Connector.STAX,
    environment: Environment.SANDBOX,
    // auth: { stax: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Stax credentials here
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
| [create_customer](#create_customer) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [token_authorize](#token_authorize) | Other | `—` |
| [tokenize](#tokenize) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### capture

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L22) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)

#### create_customer

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L39) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)

#### get

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L52) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)

#### refund

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L65) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)

#### refund_get

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L84) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)

#### token_authorize

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L96) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)

#### tokenize

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L113) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)

#### void

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts) · [Kotlin](../../examples/stax/stax.kt) · [Rust](../../examples/stax/stax.rs)
