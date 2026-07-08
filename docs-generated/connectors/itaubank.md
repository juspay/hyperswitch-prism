# Itaubank

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/itaubank.json
Regenerate: python3 scripts/generators/docs/generate.py itaubank
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
        itaubank=payment_pb2.ItaubankConfig(
            client_secret=payment_methods_pb2.SecretString(value="YOUR_CLIENT_SECRET"),
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            certificates=payment_methods_pb2.SecretString(value="YOUR_CERTIFICATES"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            base_url="YOUR_BASE_URL",
            secondary_base_url="YOUR_SECONDARY_BASE_URL",
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
    connector: Connector.ITAUBANK,
    environment: Environment.SANDBOX,
    auth: {
        itaubank: {
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            clientId: { value: 'YOUR_CLIENT_ID' },
            certificates: { value: 'YOUR_CERTIFICATES' },
            privateKey: { value: 'YOUR_PRIVATE_KEY' },
            baseUrl: 'YOUR_BASE_URL',
            secondaryBaseUrl: 'YOUR_SECONDARY_BASE_URL',
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
            .setItaubank(ItaubankConfig.newBuilder()
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setCertificates(SecretString.newBuilder().setValue("YOUR_CERTIFICATES").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setSecondaryBaseUrl("YOUR_SECONDARY_BASE_URL")
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
            config: Some(connector_specific_config::Config::Itaubank(ItaubankConfig {
                client_secret: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_SECRET".to_string())),  // Authentication credential
                client_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_ID".to_string())),  // Authentication credential
                certificates: Some(hyperswitch_masking::Secret::new("YOUR_CERTIFICATES".to_string())),  // Authentication credential
                private_key: Some(hyperswitch_masking::Secret::new("YOUR_PRIVATE_KEY".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                secondary_base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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

**Examples:** [Python](../../examples/itaubank/itaubank.py) · [TypeScript](../../examples/itaubank/itaubank.ts#L37) · [Kotlin](../../examples/itaubank/itaubank.kt#L40) · [Rust](../../examples/itaubank/itaubank.rs)
