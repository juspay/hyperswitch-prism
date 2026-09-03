# Boost

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/boost.json
Regenerate: python3 scripts/generators/docs/generate.py boost
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_CLIENT_ID`, `YOUR_MERCHANT_SECRET`, `YOUR_PUBLIC_KEY`, `YOUR_BASE_URL` with your actual values.

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
        boost=payment_pb2.BoostConfig(
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            merchant_secret=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_SECRET"),
            public_key=payment_methods_pb2.SecretString(value="YOUR_PUBLIC_KEY"),
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
    connector: Connector.BOOST,
    environment: Environment.SANDBOX,
    auth: {
        boost: {
            clientId: { value: 'YOUR_CLIENT_ID' },
            merchantSecret: { value: 'YOUR_MERCHANT_SECRET' },
            publicKey: { value: 'YOUR_PUBLIC_KEY' },
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
            .setBoost(BoostConfig.newBuilder()
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setMerchantSecret(SecretString.newBuilder().setValue("YOUR_MERCHANT_SECRET").build())
                .setPublicKey(SecretString.newBuilder().setValue("YOUR_PUBLIC_KEY").build())
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
            config: Some(connector_specific_config::Config::Boost(BoostConfig {
                client_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_ID".to_string())),  // Authentication credential
                merchant_secret: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_SECRET".to_string())),  // Authentication credential
                public_key: Some(hyperswitch_masking::Secret::new("YOUR_PUBLIC_KEY".to_string())),  // Authentication credential
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
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |

### Payments

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.ts#L74) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.kt#L55) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.ts#L101) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.kt#L94) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/boost/boost.rs)
