# Paysafe

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paysafe.json
Regenerate: python3 scripts/generators/docs/generate.py paysafe
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
    #     paysafe=payment_pb2.PaysafeConfig(api_key=...),
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
    connector: Connector.PAYSAFE,
    environment: Environment.SANDBOX,
    // auth: { paysafe: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Paysafe credentials here
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
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [token_authorize](#token_authorize) | Other | `—` |
| [tokenize](#tokenize) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### capture

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L22) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)

#### get

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L39) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)

#### refund

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L52) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)

#### refund_get

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L71) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)

#### token_authorize

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L83) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)

#### tokenize

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L100) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)

#### void

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)
