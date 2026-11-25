# Paynamics

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paynamics.json
Regenerate: python3 scripts/generators/docs/generate.py paynamics
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
        paynamics=payment_pb2.PaynamicsConfig(
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            merchant_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_KEY"),
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
    connector: Connector.PAYNAMICS,
    environment: Environment.SANDBOX,
    auth: {
        paynamics: {
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            merchantKey: { value: 'YOUR_MERCHANT_KEY' },
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
            .setPaynamics(PaynamicsConfig.newBuilder()
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setMerchantKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_KEY").build())
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
            config: Some(connector_specific_config::Config::Paynamics(PaynamicsConfig {
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                merchant_key: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_KEY".to_string())),  // Authentication credential
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
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
