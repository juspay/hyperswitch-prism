# Redsys

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/redsys.json
Regenerate: python3 scripts/generators/docs/generate.py redsys
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
        redsys=payment_pb2.RedsysConfig(
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            terminal_id=payment_methods_pb2.SecretString(value="YOUR_TERMINAL_ID"),
            sha256_pwd=payment_methods_pb2.SecretString(value="YOUR_SHA256_PWD"),
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
    connector: Connector.REDSYS,
    environment: Environment.SANDBOX,
    auth: {
        redsys: {
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            terminalId: { value: 'YOUR_TERMINAL_ID' },
            sha256Pwd: { value: 'YOUR_SHA256_PWD' },
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
            .setRedsys(RedsysConfig.newBuilder()
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setTerminalId(SecretString.newBuilder().setValue("YOUR_TERMINAL_ID").build())
                .setSha256Pwd(SecretString.newBuilder().setValue("YOUR_SHA256_PWD").build())
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
            config: Some(connector_specific_config::Config::Redsys(RedsysConfig {
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                terminal_id: Some(hyperswitch_masking::Secret::new("YOUR_TERMINAL_ID".to_string())),  // Authentication credential
                sha256_pwd: Some(hyperswitch_masking::Secret::new("YOUR_SHA256_PWD".to_string())),  // Authentication credential
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
| [PaymentMethodAuthenticationService.Authenticate](#paymentmethodauthenticationserviceauthenticate) | Authentication | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
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

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L161) · [Kotlin](../../examples/redsys/redsys.kt#L136) · [Rust](../../examples/redsys/redsys.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L170) · [Kotlin](../../examples/redsys/redsys.kt#L146) · [Rust](../../examples/redsys/redsys.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L188) · [Kotlin](../../examples/redsys/redsys.kt#L182) · [Rust](../../examples/redsys/redsys.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts) · [Kotlin](../../examples/redsys/redsys.kt#L204) · [Rust](../../examples/redsys/redsys.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L197) · [Kotlin](../../examples/redsys/redsys.kt#L192) · [Rust](../../examples/redsys/redsys.rs)

### Authentication

#### PaymentMethodAuthenticationService.Authenticate

Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServiceAuthenticateResponse` |

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L152) · [Kotlin](../../examples/redsys/redsys.kt#L88) · [Rust](../../examples/redsys/redsys.rs)

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/redsys/redsys.py) · [TypeScript](../../examples/redsys/redsys.ts#L179) · [Kotlin](../../examples/redsys/redsys.kt#L154) · [Rust](../../examples/redsys/redsys.rs)
