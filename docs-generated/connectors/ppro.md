# Ppro

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/ppro.json
Regenerate: python3 scripts/generators/docs/generate.py ppro
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
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     ppro=payment_pb2.PproConfig(api_key=...),
# ))

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { ConnectorClient } = require('connector-service-node-ffi');

// Reuse this client for all flows
const client = new ConnectorClient({
    connector: 'Ppro',
    environment: 'sandbox',
    connector_auth_type: {
        header_key: { api_key: 'YOUR_API_KEY' },
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setConnector("Ppro")
    .setEnvironment(Environment.SANDBOX)
    .setAuth(
        ConnectorAuthType.newBuilder()
            .setHeaderKey(HeaderKey.newBuilder().setApiKey("YOUR_API_KEY"))
    )
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use connector_service_sdk::{ConnectorClient, ConnectorConfig};

let config = ConnectorConfig {
    connector: "Ppro".to_string(),
    environment: Environment::Sandbox,
    auth: ConnectorAuth::HeaderKey { api_key: "YOUR_API_KEY".into() },
    ..Default::default()
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
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
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

**Examples:** [Python](../../examples/ppro/ppro.py#L130) · [TypeScript](../../examples/ppro/ppro.ts#L112) · [Kotlin](../../examples/ppro/ppro.kt#L82) · [Rust](../../examples/ppro/ppro.rs#L120)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/ppro/ppro.py#L139) · [TypeScript](../../examples/ppro/ppro.ts#L121) · [Kotlin](../../examples/ppro/ppro.kt#L92) · [Rust](../../examples/ppro/ppro.rs#L127)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/ppro/ppro.py#L166) · [TypeScript](../../examples/ppro/ppro.ts#L148) · [Kotlin](../../examples/ppro/ppro.kt#L141) · [Rust](../../examples/ppro/ppro.rs#L148)

#### PaymentService.VerifyRedirectResponse

Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVerifyRedirectResponseRequest` |
| **Response** | `PaymentServiceVerifyRedirectResponseResponse` |

**Examples:** [Python](../../examples/ppro/ppro.py#L184) · [TypeScript](../../examples/ppro/ppro.ts#L166) · [Kotlin](../../examples/ppro/ppro.kt#L163) · [Rust](../../examples/ppro/ppro.rs#L162)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/ppro/ppro.py#L193) · [TypeScript](../../examples/ppro/ppro.ts) · [Kotlin](../../examples/ppro/ppro.kt#L173) · [Rust](../../examples/ppro/ppro.rs#L169)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/ppro/ppro.py#L175) · [TypeScript](../../examples/ppro/ppro.ts#L157) · [Kotlin](../../examples/ppro/ppro.kt#L151) · [Rust](../../examples/ppro/ppro.rs#L155)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/ppro/ppro.py#L157) · [TypeScript](../../examples/ppro/ppro.ts#L139) · [Kotlin](../../examples/ppro/ppro.kt#L110) · [Rust](../../examples/ppro/ppro.rs#L141)
