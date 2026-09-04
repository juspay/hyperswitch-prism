# Billwerk

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/billwerk.json
Regenerate: python3 scripts/generators/docs/generate.py billwerk
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_PUBLIC_API_KEY`, `YOUR_BASE_URL`, `YOUR_SECONDARY_BASE_URL` with your actual values.

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
        billwerk=payment_pb2.BillwerkConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            public_api_key=payment_methods_pb2.SecretString(value="YOUR_PUBLIC_API_KEY"),
            base_url="YOUR_BASE_URL",
            secondary_base_url="YOUR_SECONDARY_BASE_URL",
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
    connector: Connector.BILLWERK,
    environment: Environment.SANDBOX,
    auth: {
        billwerk: {
            apiKey: { value: 'YOUR_API_KEY' },
            publicApiKey: { value: 'YOUR_PUBLIC_API_KEY' },
            baseUrl: 'YOUR_BASE_URL',
            secondaryBaseUrl: 'YOUR_SECONDARY_BASE_URL',
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
            .setBillwerk(BillwerkConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setPublicApiKey(SecretString.newBuilder().setValue("YOUR_PUBLIC_API_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setSecondaryBaseUrl("YOUR_SECONDARY_BASE_URL")
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
            config: Some(connector_specific_config::Config::Billwerk(BillwerkConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                public_api_key: Some(hyperswitch_masking::Secret::new("YOUR_PUBLIC_API_KEY".to_string())),  // Authentication credential
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL, e.g. https://sandbox.example.com
                secondary_base_url: Some("YOUR_SECONDARY_BASE_URL".to_string()),  // Endpoint URL, e.g. https://sandbox.example.com
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
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
| [PaymentService.TokenSetupRecurring](#paymentservicetokensetuprecurring) | Payments | `PaymentServiceTokenSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts#L154) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L90) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts#L163) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L100) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts#L181) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L139) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts#L199) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L161) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)

#### PaymentService.TokenSetupRecurring

Setup a recurring mandate using a connector token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts#L208) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L182) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L222) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts#L190) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L149) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.ts#L172) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.kt#L108) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/billwerk/billwerk.rs)
