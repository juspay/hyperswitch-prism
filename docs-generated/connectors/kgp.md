# Kgp

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/kgp.json
Regenerate: python3 scripts/generators/docs/generate.py kgp
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
        kgp=payment_pb2.KgpConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            public_key=payment_methods_pb2.SecretString(value="YOUR_PUBLIC_KEY"),
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
    connector: Connector.KGP,
    environment: Environment.SANDBOX,
    auth: {
        kgp: {
            apiKey: { value: 'YOUR_API_KEY' },
            publicKey: { value: 'YOUR_PUBLIC_KEY' },
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
            .setKgp(KgpConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setPublicKey(SecretString.newBuilder().setValue("YOUR_PUBLIC_KEY").build())
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
            config: Some(connector_specific_config::Config::Kgp(KgpConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                public_key: Some(hyperswitch_masking::Secret::new("YOUR_PUBLIC_KEY".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                secondary_base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/kgp/kgp.py) · [TypeScript](../../examples/kgp/kgp.ts#L118) · [Kotlin](../../examples/kgp/kgp.kt#L88) · [Rust](../../examples/kgp/kgp.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/kgp/kgp.py) · [TypeScript](../../examples/kgp/kgp.ts#L136) · [Kotlin](../../examples/kgp/kgp.kt#L111) · [Rust](../../examples/kgp/kgp.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/kgp/kgp.py) · [TypeScript](../../examples/kgp/kgp.ts#L154) · [Kotlin](../../examples/kgp/kgp.kt#L134) · [Rust](../../examples/kgp/kgp.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/kgp/kgp.py) · [TypeScript](../../examples/kgp/kgp.ts#L172) · [Kotlin](../../examples/kgp/kgp.kt#L156) · [Rust](../../examples/kgp/kgp.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/kgp/kgp.py) · [TypeScript](../../examples/kgp/kgp.ts) · [Kotlin](../../examples/kgp/kgp.kt#L177) · [Rust](../../examples/kgp/kgp.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/kgp/kgp.py) · [TypeScript](../../examples/kgp/kgp.ts#L163) · [Kotlin](../../examples/kgp/kgp.kt#L144) · [Rust](../../examples/kgp/kgp.rs)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/kgp/kgp.py) · [TypeScript](../../examples/kgp/kgp.ts#L127) · [Kotlin](../../examples/kgp/kgp.kt#L98) · [Rust](../../examples/kgp/kgp.rs)
