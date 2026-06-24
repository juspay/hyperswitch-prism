# Tsystransit

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/tsystransit.json
Regenerate: python3 scripts/generators/docs/generate.py tsystransit
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
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     tsystransit=payment_pb2.TsystransitConfig(api_key=...),
    # ),
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
    connector: Connector.TSYSTRANSIT,
    environment: Environment.SANDBOX,
    // auth: { tsystransit: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Tsystransit credentials here
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
    connector_config: None,  // TODO: Add your connector config here,
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
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/tsystransit/tsystransit.py) · [TypeScript](../../examples/tsystransit/tsystransit.ts#L80) · [Kotlin](../../examples/tsystransit/tsystransit.kt#L79) · [Rust](../../examples/tsystransit/tsystransit.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/tsystransit/tsystransit.py) · [TypeScript](../../examples/tsystransit/tsystransit.ts#L89) · [Kotlin](../../examples/tsystransit/tsystransit.kt#L89) · [Rust](../../examples/tsystransit/tsystransit.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/tsystransit/tsystransit.py) · [TypeScript](../../examples/tsystransit/tsystransit.ts#L98) · [Kotlin](../../examples/tsystransit/tsystransit.kt#L97) · [Rust](../../examples/tsystransit/tsystransit.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](../../examples/tsystransit/tsystransit.py) · [TypeScript](../../examples/tsystransit/tsystransit.ts#L116) · [Kotlin](../../examples/tsystransit/tsystransit.kt#L119) · [Rust](../../examples/tsystransit/tsystransit.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/tsystransit/tsystransit.py) · [TypeScript](../../examples/tsystransit/tsystransit.ts) · [Kotlin](../../examples/tsystransit/tsystransit.kt#L127) · [Rust](../../examples/tsystransit/tsystransit.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/tsystransit/tsystransit.py) · [TypeScript](../../examples/tsystransit/tsystransit.ts#L107) · [Kotlin](../../examples/tsystransit/tsystransit.kt#L107) · [Rust](../../examples/tsystransit/tsystransit.rs)
