# Finix

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/finix.json
Regenerate: python3 scripts/generators/docs/generate.py finix
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
    #     finix=payment_pb2.FinixConfig(api_key=...),
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
    connector: Connector.FINIX,
    environment: Environment.SANDBOX,
    // auth: { finix: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Finix credentials here
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
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L144) · [Kotlin](../../examples/finix/finix.kt#L77) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L162) · [Kotlin](../../examples/finix/finix.kt#L100) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L180) · [Kotlin](../../examples/finix/finix.kt#L139) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L198) · [Kotlin](../../examples/finix/finix.kt#L161) · [Rust](../../examples/finix/finix.rs)

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L207) · [Kotlin](../../examples/finix/finix.kt#L182) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts) · [Kotlin](../../examples/finix/finix.kt#L211) · [Rust](../../examples/finix/finix.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L189) · [Kotlin](../../examples/finix/finix.kt#L149) · [Rust](../../examples/finix/finix.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L171) · [Kotlin](../../examples/finix/finix.kt#L108) · [Rust](../../examples/finix/finix.rs)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L153) · [Kotlin](../../examples/finix/finix.kt#L87) · [Rust](../../examples/finix/finix.rs)
