# Betterpayment

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/betterpayment.json
Regenerate: python3 scripts/generators/docs/generate.py betterpayment
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
        betterpayment=payment_pb2.BetterpaymentConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            api_password=payment_methods_pb2.SecretString(value="YOUR_API_PASSWORD"),
            webhook_key=payment_methods_pb2.SecretString(value="YOUR_WEBHOOK_KEY"),
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
    connector: Connector.BETTERPAYMENT,
    environment: Environment.SANDBOX,
    auth: {
        betterpayment: {
            apiKey: { value: 'YOUR_API_KEY' },
            apiPassword: { value: 'YOUR_API_PASSWORD' },
            webhookKey: { value: 'YOUR_WEBHOOK_KEY' },
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
            .setBetterpayment(BetterpaymentConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setApiPassword(SecretString.newBuilder().setValue("YOUR_API_PASSWORD").build())
                .setWebhookKey(SecretString.newBuilder().setValue("YOUR_WEBHOOK_KEY").build())
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
            config: Some(connector_specific_config::Config::Betterpayment(BetterpaymentConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                api_password: Some(hyperswitch_masking::Secret::new("YOUR_API_PASSWORD".to_string())),  // Authentication credential
                webhook_key: Some(hyperswitch_masking::Secret::new("YOUR_WEBHOOK_KEY".to_string())),  // Authentication credential
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
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |

### Payments

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/betterpayment/betterpayment.py) · [TypeScript](../../examples/betterpayment/betterpayment.ts#L87) · [Kotlin](../../examples/betterpayment/betterpayment.kt#L68) · [Rust](../../examples/betterpayment/betterpayment.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/betterpayment/betterpayment.py) · [TypeScript](../../examples/betterpayment/betterpayment.ts#L114) · [Kotlin](../../examples/betterpayment/betterpayment.kt#L107) · [Rust](../../examples/betterpayment/betterpayment.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/betterpayment/betterpayment.py) · [TypeScript](../../examples/betterpayment/betterpayment.ts#L123) · [Kotlin](../../examples/betterpayment/betterpayment.kt#L117) · [Rust](../../examples/betterpayment/betterpayment.rs)
