# Finix

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/finix.json
Regenerate: python3 scripts/generators/docs/generate.py finix
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
    #     finix=payment_pb2.FinixConfig(api_key=...),
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
    connector: Connector.FINIX,
    environment: Environment.SANDBOX,
    // auth: { finix: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Finix credentials here
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
| [recurring_charge](#recurring_charge) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [token_authorize](#token_authorize) | Other | `—` |
| [tokenize](#tokenize) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### capture

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L22) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### create_customer

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L39) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### get

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L52) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### recurring_charge

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L65) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### refund

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L88) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### refund_get

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L107) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### token_authorize

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L119) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### tokenize

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L136) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)

#### void

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts) · [Kotlin](../../examples/finix/finix.kt) · [Rust](../../examples/finix/finix.rs)
