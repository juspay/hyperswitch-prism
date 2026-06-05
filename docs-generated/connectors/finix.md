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
    connector_config=payment_pb2.ConnectorSpecificConfig(
        finix=payment_pb2.FinixConfig(
            finix_user_name=payment_methods_pb2.SecretString(value="YOUR_FINIX_USER_NAME"),
            finix_password=payment_methods_pb2.SecretString(value="YOUR_FINIX_PASSWORD"),
            merchant_identity_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_IDENTITY_ID"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
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
    connector: Connector.FINIX,
    environment: Environment.SANDBOX,
    auth: {
        finix: {
            finixUserName: { value: 'YOUR_FINIX_USER_NAME' },
            finixPassword: { value: 'YOUR_FINIX_PASSWORD' },
            merchantIdentityId: { value: 'YOUR_MERCHANT_IDENTITY_ID' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
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
            .setFinix(FinixConfig.newBuilder()
                .setFinixUserName(SecretString.newBuilder().setValue("YOUR_FINIX_USER_NAME").build())
                .setFinixPassword(SecretString.newBuilder().setValue("YOUR_FINIX_PASSWORD").build())
                .setMerchantIdentityId(SecretString.newBuilder().setValue("YOUR_MERCHANT_IDENTITY_ID").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
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
            config: Some(connector_specific_config::Config::Finix(FinixConfig {
                finix_user_name: Some(hyperswitch_masking::Secret::new("YOUR_FINIX_USER_NAME".to_string())),  // Authentication credential
                finix_password: Some(hyperswitch_masking::Secret::new("YOUR_FINIX_PASSWORD".to_string())),  // Authentication credential
                merchant_identity_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_IDENTITY_ID".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
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
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L127) · [Kotlin](../../examples/finix/finix.kt#L88) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L145) · [Kotlin](../../examples/finix/finix.kt#L111) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L163) · [Kotlin](../../examples/finix/finix.kt#L150) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L181) · [Kotlin](../../examples/finix/finix.kt#L172) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts) · [Kotlin](../../examples/finix/finix.kt#L193) · [Rust](../../examples/finix/finix.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L172) · [Kotlin](../../examples/finix/finix.kt#L160) · [Rust](../../examples/finix/finix.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L154) · [Kotlin](../../examples/finix/finix.kt#L119) · [Rust](../../examples/finix/finix.rs)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L136) · [Kotlin](../../examples/finix/finix.kt#L98) · [Rust](../../examples/finix/finix.rs)
