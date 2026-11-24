# Flywire

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/flywire.json
Regenerate: python3 scripts/generators/docs/generate.py flywire
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
        flywire=payment_pb2.FlywireConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            shared_secret=payment_methods_pb2.SecretString(value="YOUR_SHARED_SECRET"),
            recipient_id="YOUR_RECIPIENT_ID",
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
    connector: Connector.FLYWIRE,
    environment: Environment.SANDBOX,
    auth: {
        flywire: {
            apiKey: { value: 'YOUR_API_KEY' },
            sharedSecret: { value: 'YOUR_SHARED_SECRET' },
            recipientId: 'YOUR_RECIPIENT_ID',
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
            .setFlywire(FlywireConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setSharedSecret(SecretString.newBuilder().setValue("YOUR_SHARED_SECRET").build())
                .setRecipientId("YOUR_RECIPIENT_ID")
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
            config: Some(connector_specific_config::Config::Flywire(FlywireConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                shared_secret: Some(hyperswitch_masking::Secret::new("YOUR_SHARED_SECRET".to_string())),  // Authentication credential
                recipient_id: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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
| [PaymentService.VerifyRedirectResponse](#paymentserviceverifyredirectresponse) | Payments | `PaymentServiceVerifyRedirectResponseRequest` |

### Payments

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/flywire/flywire.py) · [TypeScript](../../examples/flywire/flywire.ts#L92) · [Kotlin](../../examples/flywire/flywire.kt#L68) · [Rust](../../examples/flywire/flywire.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/flywire/flywire.py) · [TypeScript](../../examples/flywire/flywire.ts#L119) · [Kotlin](../../examples/flywire/flywire.kt#L107) · [Rust](../../examples/flywire/flywire.rs)

#### PaymentService.VerifyRedirectResponse

Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVerifyRedirectResponseRequest` |
| **Response** | `PaymentServiceVerifyRedirectResponseResponse` |

**Examples:** [Python](../../examples/flywire/flywire.py) · [TypeScript](../../examples/flywire/flywire.ts#L137) · [Kotlin](../../examples/flywire/flywire.kt#L129) · [Rust](../../examples/flywire/flywire.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/flywire/flywire.py) · [TypeScript](../../examples/flywire/flywire.ts#L128) · [Kotlin](../../examples/flywire/flywire.kt#L117) · [Rust](../../examples/flywire/flywire.rs)
