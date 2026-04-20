# CryptoPay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/cryptopay.json
Regenerate: python3 scripts/generators/docs/generate.py cryptopay
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
    #     cryptopay=payment_pb2.CryptopayConfig(api_key=...),
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
    connector: Connector.CRYPTOPAY,
    environment: Environment.SANDBOX,
    // auth: { cryptopay: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Cryptopay credentials here
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
| [get](#get) | Other | `—` |
| [handle_event](#handle_event) | Other | `—` |
| [parse_event](#parse_event) | Other | `—` |

### Other

#### get

**Examples:** [Python](../../examples/cryptopay/cryptopay.py) · [TypeScript](../../examples/cryptopay/cryptopay.ts#L22) · [Kotlin](../../examples/cryptopay/cryptopay.kt) · [Rust](../../examples/cryptopay/cryptopay.rs)

#### handle_event

**Examples:** [Python](../../examples/cryptopay/cryptopay.py) · [TypeScript](../../examples/cryptopay/cryptopay.ts#L35) · [Kotlin](../../examples/cryptopay/cryptopay.kt) · [Rust](../../examples/cryptopay/cryptopay.rs)

#### parse_event

**Examples:** [Python](../../examples/cryptopay/cryptopay.py) · [TypeScript](../../examples/cryptopay/cryptopay.ts#L47) · [Kotlin](../../examples/cryptopay/cryptopay.kt) · [Rust](../../examples/cryptopay/cryptopay.rs)
