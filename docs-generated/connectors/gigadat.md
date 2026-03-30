# Gigadat

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/gigadat.json
Regenerate: python3 scripts/generators/docs/generate.py gigadat
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Javascript</b></td><td><b>Kotlin</b></td><td><b>Python</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Javascript</summary>

```javascript
import { DirectPaymentClient, types } from 'hyperswitch-prism';

const config: types.IConnectorConfig = types.ConnectorConfig.create({
    options: types.SdkOptions.create({ environment: types.Environment.SANDBOX }),
    connectorConfig: types.ConnectorSpecificConfig.create({
        gigadat: {
        campaignId: { value: 'YOUR_CAMPAIGN_ID' },
        accessToken: { value: 'YOUR_ACCESS_TOKEN' },
        securityToken: { value: 'YOUR_SECURITY_TOKEN' },
        },
    }),
});
const client = new DirectPaymentClient(config);
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
import payments.DirectPaymentClient
import payments.ConnectorConfig
import payments.Environment

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = DirectPaymentClient(config)
```

</details>

</td>
<td valign="top">

<details><summary>Python</summary>

```python
from payments import PaymentClient
from payments.generated import sdk_config_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
client = PaymentClient(config)
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;

let config = ConnectorConfig {
    connector_config: Some(ConnectorSpecificConfig {
        config: Some(connector_specific_config::Config::Gigadat(GigadatConfig {
                campaign_id: Some(Secret::new("YOUR_CAMPAIGN_ID".to_string())),
                access_token: Some(Secret::new("YOUR_ACCESS_TOKEN".to_string())),
                security_token: Some(Secret::new("YOUR_SECURITY_TOKEN".to_string())),
            ..Default::default()
        })),
    }),
    options: Some(SdkOptions { environment: Environment::Sandbox.into() }),
};
let client = ConnectorClient::new(config, None).unwrap();
```

</details>

</td>
</tr>
</table>

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |

### Other

#### get

**Examples:** [Python](../../examples/gigadat/python/gigadat.py) · [JavaScript](../../examples/gigadat/javascript/gigadat.ts) · [Kotlin](../../examples/gigadat/kotlin/gigadat.kt#L12) · [Rust](../../examples/gigadat/rust/gigadat.rs#L18)

#### refund

**Examples:** [Python](../../examples/gigadat/python/gigadat.py) · [JavaScript](../../examples/gigadat/javascript/gigadat.ts) · [Kotlin](../../examples/gigadat/kotlin/gigadat.kt#L20) · [Rust](../../examples/gigadat/rust/gigadat.rs#L35)
