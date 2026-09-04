# Tamara

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/tamara.json
Regenerate: python3 scripts/generators/docs/generate.py tamara
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_BASE_URL` with your actual values.

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
        tamara=payment_pb2.TamaraConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
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
    connector: Connector.TAMARA,
    environment: Environment.SANDBOX,
    auth: {
        tamara: {
            apiKey: { value: 'YOUR_API_KEY' },
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
            .setTamara(TamaraConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
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
            config: Some(connector_specific_config::Config::Tamara(TamaraConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL
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
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.VerifyRedirectResponse](#paymentserviceverifyredirectresponse) | Payments | `PaymentServiceVerifyRedirectResponseRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.ts#L112) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.kt#L88) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.ts#L121) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.kt#L98) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.ts#L148) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.kt#L137) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.rs)

#### PaymentService.VerifyRedirectResponse

Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVerifyRedirectResponseRequest` |
| **Response** | `PaymentServiceVerifyRedirectResponseResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.ts#L166) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.kt#L159) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.kt#L169) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.ts#L157) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.kt#L147) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tamara/tamara.rs)
