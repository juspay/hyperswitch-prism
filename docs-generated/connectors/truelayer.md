# Truelayer

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/truelayer.json
Regenerate: python3 scripts/generators/docs/generate.py truelayer
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
        truelayer: {
        clientId: { value: 'YOUR_CLIENT_ID' },
        clientSecret: { value: 'YOUR_CLIENT_SECRET' },
        merchantAccountId: { value: 'YOUR_MERCHANT_ACCOUNT_ID' },
        accountHolderName: { value: 'YOUR_ACCOUNT_HOLDER_NAME' },
        privateKey: { value: 'YOUR_PRIVATE_KEY' },
        kid: { value: 'YOUR_KID' },
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
        config: Some(connector_specific_config::Config::Truelayer(TruelayerConfig {
                client_id: Some(Secret::new("YOUR_CLIENT_ID".to_string())),
                client_secret: Some(Secret::new("YOUR_CLIENT_SECRET".to_string())),
                merchant_account_id: Some(Secret::new("YOUR_MERCHANT_ACCOUNT_ID".to_string())),
                account_holder_name: Some(Secret::new("YOUR_ACCOUNT_HOLDER_NAME".to_string())),
                private_key: Some(Secret::new("YOUR_PRIVATE_KEY".to_string())),
                kid: Some(Secret::new("YOUR_KID".to_string())),
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
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [get](#get) | Other | `—` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/truelayer/python/truelayer.py) · [JavaScript](../../examples/truelayer/javascript/truelayer.ts) · [Kotlin](../../examples/truelayer/kotlin/truelayer.kt) · [Rust](../../examples/truelayer/rust/truelayer.rs#L18)

### Other

#### get

**Examples:** [Python](../../examples/truelayer/python/truelayer.py) · [JavaScript](../../examples/truelayer/javascript/truelayer.ts) · [Kotlin](../../examples/truelayer/kotlin/truelayer.kt#L23) · [Rust](../../examples/truelayer/rust/truelayer.rs#L25)
