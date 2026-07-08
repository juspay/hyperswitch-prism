# Qwikcilver

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/qwikcilver.json
Regenerate: python3 scripts/generators/docs/generate.py qwikcilver
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
    connector_config=payment_pb2.ConnectorSpecificConfig(
        qwikcilver=payment_pb2.QwikcilverConfig(
            bootstrap_bearer_token=payment_methods_pb2.SecretString(value="YOUR_BOOTSTRAP_BEARER_TOKEN"),
            terminal_id=payment_methods_pb2.SecretString(value="YOUR_TERMINAL_ID"),
            username=payment_methods_pb2.SecretString(value="YOUR_USERNAME"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
            base_url="YOUR_BASE_URL",
        ),
    ),
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
    connector: Connector.QWIKCILVER,
    environment: Environment.SANDBOX,
    auth: {
        qwikcilver: {
            bootstrapBearerToken: { value: 'YOUR_BOOTSTRAP_BEARER_TOKEN' },
            terminalId: { value: 'YOUR_TERMINAL_ID' },
            username: { value: 'YOUR_USERNAME' },
            password: { value: 'YOUR_PASSWORD' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setQwikcilver(QwikcilverConfig.newBuilder()
                .setBootstrapBearerToken(SecretString.newBuilder().setValue("YOUR_BOOTSTRAP_BEARER_TOKEN").build())
                .setTerminalId(SecretString.newBuilder().setValue("YOUR_TERMINAL_ID").build())
                .setUsername(SecretString.newBuilder().setValue("YOUR_USERNAME").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
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
    connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Qwikcilver(QwikcilverConfig {
                bootstrap_bearer_token: Some(hyperswitch_masking::Secret::new("YOUR_BOOTSTRAP_BEARER_TOKEN".to_string())),  // Authentication credential
                terminal_id: Some(hyperswitch_masking::Secret::new("YOUR_TERMINAL_ID".to_string())),  // Authentication credential
                username: Some(hyperswitch_masking::Secret::new("YOUR_USERNAME".to_string())),  // Authentication credential
                password: Some(hyperswitch_masking::Secret::new("YOUR_PASSWORD".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                ..Default::default()
            })),
        }),
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
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |

### Authentication

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/qwikcilver/qwikcilver.py) · [TypeScript](../../examples/qwikcilver/qwikcilver.ts#L36) · [Kotlin](../../examples/qwikcilver/qwikcilver.kt#L39) · [Rust](../../examples/qwikcilver/qwikcilver.rs)
