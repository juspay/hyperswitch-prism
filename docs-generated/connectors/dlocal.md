# dLocal

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/dlocal.json
Regenerate: python3 scripts/generators/docs/generate.py dlocal
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
        dlocal=payment_pb2.DlocalConfig(
            x_login=payment_methods_pb2.SecretString(value="YOUR_X_LOGIN"),
            x_trans_key=payment_methods_pb2.SecretString(value="YOUR_X_TRANS_KEY"),
            secret=payment_methods_pb2.SecretString(value="YOUR_SECRET"),
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
    connector: Connector.DLOCAL,
    environment: Environment.SANDBOX,
    auth: {
        dlocal: {
            xLogin: { value: 'YOUR_X_LOGIN' },
            xTransKey: { value: 'YOUR_X_TRANS_KEY' },
            secret: { value: 'YOUR_SECRET' },
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
            .setDlocal(DlocalConfig.newBuilder()
                .setXLogin(SecretString.newBuilder().setValue("YOUR_X_LOGIN").build())
                .setXTransKey(SecretString.newBuilder().setValue("YOUR_X_TRANS_KEY").build())
                .setSecret(SecretString.newBuilder().setValue("YOUR_SECRET").build())
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
            config: Some(connector_specific_config::Config::Dlocal(DlocalConfig {
                x_login: Some(hyperswitch_masking::Secret::new("YOUR_X_LOGIN".to_string())),  // Authentication credential
                x_trans_key: Some(hyperswitch_masking::Secret::new("YOUR_X_TRANS_KEY".to_string())),  // Authentication credential
                secret: Some(hyperswitch_masking::Secret::new("YOUR_SECRET".to_string())),  // Authentication credential
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
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L105) · [Kotlin](../../examples/dlocal/dlocal.kt#L85) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L114) · [Kotlin](../../examples/dlocal/dlocal.kt#L95) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L141) · [Kotlin](../../examples/dlocal/dlocal.kt#L134) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts) · [Kotlin](../../examples/dlocal/dlocal.kt#L156) · [Rust](../../examples/dlocal/dlocal.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L150) · [Kotlin](../../examples/dlocal/dlocal.kt#L144) · [Rust](../../examples/dlocal/dlocal.rs)
