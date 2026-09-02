# Saferpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/saferpay.json
Regenerate: python3 scripts/generators/docs/generate.py saferpay
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
        saferpay=payment_pb2.SaferpayConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            key1=payment_methods_pb2.SecretString(value="YOUR_KEY1"),
            api_secret=payment_methods_pb2.SecretString(value="YOUR_API_SECRET"),
            key2=payment_methods_pb2.SecretString(value="YOUR_KEY2"),
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
    connector: Connector.SAFERPAY,
    environment: Environment.SANDBOX,
    auth: {
        saferpay: {
            apiKey: { value: 'YOUR_API_KEY' },
            key1: { value: 'YOUR_KEY1' },
            apiSecret: { value: 'YOUR_API_SECRET' },
            key2: { value: 'YOUR_KEY2' },
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
            .setSaferpay(SaferpayConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setKey1(SecretString.newBuilder().setValue("YOUR_KEY1").build())
                .setApiSecret(SecretString.newBuilder().setValue("YOUR_API_SECRET").build())
                .setKey2(SecretString.newBuilder().setValue("YOUR_KEY2").build())
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
            config: Some(connector_specific_config::Config::Saferpay(SaferpayConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                key1: Some(hyperswitch_masking::Secret::new("YOUR_KEY1".to_string())),  // Authentication credential
                api_secret: Some(hyperswitch_masking::Secret::new("YOUR_API_SECRET".to_string())),  // Authentication credential
                key2: Some(hyperswitch_masking::Secret::new("YOUR_KEY2".to_string())),  // Authentication credential
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
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/saferpay/saferpay.py) · [TypeScript](../../examples/saferpay/saferpay.ts#L92) · [Kotlin](../../examples/saferpay/saferpay.kt#L72) · [Rust](../../examples/saferpay/saferpay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/saferpay/saferpay.py) · [TypeScript](../../examples/saferpay/saferpay.ts#L101) · [Kotlin](../../examples/saferpay/saferpay.kt#L82) · [Rust](../../examples/saferpay/saferpay.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/saferpay/saferpay.py) · [TypeScript](../../examples/saferpay/saferpay.ts) · [Kotlin](../../examples/saferpay/saferpay.kt#L130) · [Rust](../../examples/saferpay/saferpay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/saferpay/saferpay.py) · [TypeScript](../../examples/saferpay/saferpay.ts#L119) · [Kotlin](../../examples/saferpay/saferpay.kt#L118) · [Rust](../../examples/saferpay/saferpay.rs)

### Authentication

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/saferpay/saferpay.py) · [TypeScript](../../examples/saferpay/saferpay.ts#L110) · [Kotlin](../../examples/saferpay/saferpay.kt#L90) · [Rust](../../examples/saferpay/saferpay.rs)
