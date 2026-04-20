# Billwerk

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/billwerk.json
Regenerate: python3 scripts/generators/docs/generate.py billwerk
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
    #     billwerk=payment_pb2.BillwerkConfig(api_key=...),
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
    connector: Connector.BILLWERK,
    environment: Environment.SANDBOX,
    // auth: { billwerk: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Billwerk credentials here
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
| [recurring_charge](#recurring_charge) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [token_authorize](#token_authorize) | Other | `—` |
| [token_setup_recurring](#token_setup_recurring) | Other | `—` |
| [tokenize](#tokenize) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### capture

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L22) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### get

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L39) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### recurring_charge

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L53) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### refund

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L76) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### refund_get

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L95) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### token_authorize

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L107) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### token_setup_recurring

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L124) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### tokenize

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts#L144) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)

#### void

**Examples:** [Python](../../examples/billwerk/billwerk.py) · [TypeScript](../../examples/billwerk/billwerk.ts) · [Kotlin](../../examples/billwerk/billwerk.kt) · [Rust](../../examples/billwerk/billwerk.rs)
