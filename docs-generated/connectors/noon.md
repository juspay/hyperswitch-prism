# Noon

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/noon.json
Regenerate: python3 scripts/generators/docs/generate.py noon
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
#     noon=payment_pb2.NoonConfig(api_key=...),
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
    connector: 'Noon',
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
    .setConnector("Noon")
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
    connector: "Noon".to_string(),
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
| [RecurringPaymentService.Revoke](#recurringpaymentservicerevoke) | Mandates | `RecurringPaymentServiceRevokeRequest` |
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

**Examples:** [Python](../../examples/noon/noon.py#L130) · [TypeScript](../../examples/noon/noon.ts#L112) · [Kotlin](../../examples/noon/noon.kt#L78) · [Rust](../../examples/noon/noon.rs#L119)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/noon/noon.py#L139) · [TypeScript](../../examples/noon/noon.ts#L121) · [Kotlin](../../examples/noon/noon.kt#L88) · [Rust](../../examples/noon/noon.rs#L126)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/noon/noon.py#L175) · [TypeScript](../../examples/noon/noon.ts#L157) · [Kotlin](../../examples/noon/noon.kt#L150) · [Rust](../../examples/noon/noon.rs#L154)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/noon/noon.py#L193) · [TypeScript](../../examples/noon/noon.ts) · [Kotlin](../../examples/noon/noon.kt#L172) · [Rust](../../examples/noon/noon.rs#L168)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/noon/noon.py#L184) · [TypeScript](../../examples/noon/noon.ts#L166) · [Kotlin](../../examples/noon/noon.kt#L160) · [Rust](../../examples/noon/noon.rs#L161)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/noon/noon.py#L157) · [TypeScript](../../examples/noon/noon.ts#L139) · [Kotlin](../../examples/noon/noon.kt#L106) · [Rust](../../examples/noon/noon.rs#L140)

#### RecurringPaymentService.Revoke

Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceRevokeRequest` |
| **Response** | `RecurringPaymentServiceRevokeResponse` |

**Examples:** [Python](../../examples/noon/noon.py#L166) · [TypeScript](../../examples/noon/noon.ts#L148) · [Kotlin](../../examples/noon/noon.kt#L138) · [Rust](../../examples/noon/noon.rs#L147)
