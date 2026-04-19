# Fiservcommercehub

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiservcommercehub.json
Regenerate: python3 scripts/generators/docs/generate.py fiservcommercehub
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
        fiservcommercehub=payment_pb2.FiservcommercehubConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            secret=payment_methods_pb2.SecretString(value="YOUR_SECRET"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            terminal_id=payment_methods_pb2.SecretString(value="YOUR_TERMINAL_ID"),
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
    connector: Connector.FISERVCOMMERCEHUB,
    environment: Environment.SANDBOX,
    auth: {
        fiservcommercehub: {
            apiKey: { value: 'YOUR_API_KEY' },
            secret: { value: 'YOUR_SECRET' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            terminalId: { value: 'YOUR_TERMINAL_ID' },
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
            .setFiservcommercehub(FiservcommercehubConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setSecret(SecretString.newBuilder().setValue("YOUR_SECRET").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setTerminalId(SecretString.newBuilder().setValue("YOUR_TERMINAL_ID").build())
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
            config: Some(connector_specific_config::Config::Fiservcommercehub(FiservcommercehubConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                secret: Some(hyperswitch_masking::Secret::new("YOUR_SECRET".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                terminal_id: Some(hyperswitch_masking::Secret::new("YOUR_TERMINAL_ID".to_string())),  // Authentication credential
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
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L112) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L105) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L121) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L113) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L142) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L130) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L123) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

### Authentication

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L103) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L95) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)
