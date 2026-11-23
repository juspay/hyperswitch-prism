# Axisbank

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/axisbank.json
Regenerate: python3 scripts/generators/docs/generate.py axisbank
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2, events_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        axisbank=payment_pb2.AxisbankConfig(
            merchant_kid=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_KID"),
            juspay_kid=payment_methods_pb2.SecretString(value="YOUR_JUSPAY_KID"),
            merchant_private_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_PRIVATE_KEY"),
            juspay_public_key=payment_methods_pb2.SecretString(value="YOUR_JUSPAY_PUBLIC_KEY"),
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
    connector: Connector.AXISBANK,
    environment: Environment.SANDBOX,
    auth: {
        axisbank: {
            merchantKid: { value: 'YOUR_MERCHANT_KID' },
            juspayKid: { value: 'YOUR_JUSPAY_KID' },
            merchantPrivateKey: { value: 'YOUR_MERCHANT_PRIVATE_KEY' },
            juspayPublicKey: { value: 'YOUR_JUSPAY_PUBLIC_KEY' },
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
            .setAxisbank(AxisbankConfig.newBuilder()
                .setMerchantKid(SecretString.newBuilder().setValue("YOUR_MERCHANT_KID").build())
                .setJuspayKid(SecretString.newBuilder().setValue("YOUR_JUSPAY_KID").build())
                .setMerchantPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_PRIVATE_KEY").build())
                .setJuspayPublicKey(SecretString.newBuilder().setValue("YOUR_JUSPAY_PUBLIC_KEY").build())
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
            config: Some(connector_specific_config::Config::Axisbank(AxisbankConfig {
                merchant_kid: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_KID".to_string())),  // Authentication credential
                juspay_kid: Some(hyperswitch_masking::Secret::new("YOUR_JUSPAY_KID".to_string())),  // Authentication credential
                merchant_private_key: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_PRIVATE_KEY".to_string())),  // Authentication credential
                juspay_public_key: Some(hyperswitch_masking::Secret::new("YOUR_JUSPAY_PUBLIC_KEY".to_string())),  // Authentication credential
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
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
