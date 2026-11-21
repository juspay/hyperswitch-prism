# Asiapay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/asiapay.json
Regenerate: python3 scripts/generators/docs/generate.py asiapay
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
        asiapay=payment_pb2.AsiapayConfig(
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            login_id=payment_methods_pb2.SecretString(value="YOUR_LOGIN_ID"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
            secure_hash_secret=payment_methods_pb2.SecretString(value="YOUR_SECURE_HASH_SECRET"),
            secure_hash_algorithm="YOUR_SECURE_HASH_ALGORITHM",
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
    connector: Connector.ASIAPAY,
    environment: Environment.SANDBOX,
    auth: {
        asiapay: {
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            loginId: { value: 'YOUR_LOGIN_ID' },
            password: { value: 'YOUR_PASSWORD' },
            secureHashSecret: { value: 'YOUR_SECURE_HASH_SECRET' },
            secureHashAlgorithm: 'YOUR_SECURE_HASH_ALGORITHM',
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
            .setAsiapay(AsiapayConfig.newBuilder()
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setLoginId(SecretString.newBuilder().setValue("YOUR_LOGIN_ID").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setSecureHashSecret(SecretString.newBuilder().setValue("YOUR_SECURE_HASH_SECRET").build())
                .setSecureHashAlgorithm("YOUR_SECURE_HASH_ALGORITHM")
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
            config: Some(connector_specific_config::Config::Asiapay(AsiapayConfig {
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                login_id: Some(hyperswitch_masking::Secret::new("YOUR_LOGIN_ID".to_string())),  // Authentication credential
                password: Some(hyperswitch_masking::Secret::new("YOUR_PASSWORD".to_string())),  // Authentication credential
                secure_hash_secret: Some(hyperswitch_masking::Secret::new("YOUR_SECURE_HASH_SECRET".to_string())),  // Authentication credential
                secure_hash_algorithm: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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
| [PaymentService.VerifyRedirectResponse](#paymentserviceverifyredirectresponse) | Payments | `PaymentServiceVerifyRedirectResponseRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L112) · [Kotlin](../../examples/asiapay/asiapay.kt#L88) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L121) · [Kotlin](../../examples/asiapay/asiapay.kt#L98) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L148) · [Kotlin](../../examples/asiapay/asiapay.kt#L137) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.VerifyRedirectResponse

Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVerifyRedirectResponseRequest` |
| **Response** | `PaymentServiceVerifyRedirectResponseResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L166) · [Kotlin](../../examples/asiapay/asiapay.kt#L159) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts) · [Kotlin](../../examples/asiapay/asiapay.kt#L169) · [Rust](../../examples/asiapay/asiapay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L157) · [Kotlin](../../examples/asiapay/asiapay.kt#L147) · [Rust](../../examples/asiapay/asiapay.rs)
