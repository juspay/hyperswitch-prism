# Stax

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/stax.json
Regenerate: python3 scripts/generators/docs/generate.py stax
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
        stax=payment_pb2.StaxConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
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
    connector: Connector.STAX,
    environment: Environment.SANDBOX,
    auth: {
        stax: {
            apiKey: { value: 'YOUR_API_KEY' },
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
            .setStax(StaxConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
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
            config: Some(connector_specific_config::Config::Stax(StaxConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
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
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
| [PaymentService.TokenSetupRecurring](#paymentservicetokensetuprecurring) | Payments | `PaymentServiceTokenSetupRecurringRequest` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L181) · [Kotlin](../../examples/stax/stax.kt#L88) · [Rust](../../examples/stax/stax.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L199) · [Kotlin](../../examples/stax/stax.kt#L111) · [Rust](../../examples/stax/stax.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L217) · [Kotlin](../../examples/stax/stax.kt#L150) · [Rust](../../examples/stax/stax.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L235) · [Kotlin](../../examples/stax/stax.kt#L172) · [Rust](../../examples/stax/stax.rs)

#### PaymentService.TokenSetupRecurring

Setup a recurring mandate using a connector token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L244) · [Kotlin](../../examples/stax/stax.kt#L193) · [Rust](../../examples/stax/stax.rs)

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L253) · [Kotlin](../../examples/stax/stax.kt#L229) · [Rust](../../examples/stax/stax.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts) · [Kotlin](../../examples/stax/stax.kt#L258) · [Rust](../../examples/stax/stax.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L226) · [Kotlin](../../examples/stax/stax.kt#L160) · [Rust](../../examples/stax/stax.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L208) · [Kotlin](../../examples/stax/stax.kt#L119) · [Rust](../../examples/stax/stax.rs)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/stax/stax.py) · [TypeScript](../../examples/stax/stax.ts#L190) · [Kotlin](../../examples/stax/stax.kt#L98) · [Rust](../../examples/stax/stax.rs)
