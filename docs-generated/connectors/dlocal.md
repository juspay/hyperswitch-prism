# dLocal

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/dlocal.json
Regenerate: python3 scripts/generators/docs/generate.py dlocal
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
        dlocal=payment_pb2.DlocalConfig(
            x_login=payment_methods_pb2.SecretString(value="YOUR_X_LOGIN"),
            x_trans_key=payment_methods_pb2.SecretString(value="YOUR_X_TRANS_KEY"),
            secret=payment_methods_pb2.SecretString(value="YOUR_SECRET"),
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
    connector: Connector.DLOCAL,
    environment: Environment.SANDBOX,
    auth: {
        dlocal: {
            xLogin: { value: 'YOUR_X_LOGIN' },
            xTransKey: { value: 'YOUR_X_TRANS_KEY' },
            secret: { value: 'YOUR_SECRET' },
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
            .setDlocal(DlocalConfig.newBuilder()
                .setXLogin(SecretString.newBuilder().setValue("YOUR_X_LOGIN").build())
                .setXTransKey(SecretString.newBuilder().setValue("YOUR_X_TRANS_KEY").build())
                .setSecret(SecretString.newBuilder().setValue("YOUR_SECRET").build())
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
            config: Some(connector_specific_config::Config::Dlocal(DlocalConfig {
                x_login: Some(hyperswitch_masking::Secret::new("YOUR_X_LOGIN".to_string())),  // Authentication credential
                x_trans_key: Some(hyperswitch_masking::Secret::new("YOUR_X_TRANS_KEY".to_string())),  // Authentication credential
                secret: Some(hyperswitch_masking::Secret::new("YOUR_SECRET".to_string())),  // Authentication credential
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

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

### One-step Payment (Authorize + Capture)

Simple payment that authorizes and captures in one call. Use for immediate charges.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/dlocal/dlocal.py#L233) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L123) · [Rust](../../examples/dlocal/dlocal.rs#L287)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/dlocal/dlocal.py#L252) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L139) · [Rust](../../examples/dlocal/dlocal.rs#L303)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/dlocal/dlocal.py#L277) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L161) · [Rust](../../examples/dlocal/dlocal.rs#L326)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/dlocal/dlocal.py#L302) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L183) · [Rust](../../examples/dlocal/dlocal.rs#L349)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/dlocal/dlocal.py#L324) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L202) · [Rust](../../examples/dlocal/dlocal.rs#L368)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Authorize

Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.

| | Message |
|---|---------|
| **Request** | `PaymentServiceAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Bancontact | ⚠ |
| Apple Pay | ⚠ |
| Apple Pay Dec | ⚠ |
| Apple Pay SDK | ⚠ |
| Google Pay | ⚠ |
| Google Pay Dec | ⚠ |
| Google Pay SDK | ⚠ |
| PayPal SDK | ⚠ |
| Amazon Pay | ⚠ |
| Cash App | ⚠ |
| PayPal | ⚠ |
| WeChat Pay | ⚠ |
| Alipay | ⚠ |
| Revolut Pay | ⚠ |
| MiFinity | ⚠ |
| Bluecode | ⚠ |
| Paze | x |
| Samsung Pay | ⚠ |
| MB Way | ⚠ |
| Satispay | ⚠ |
| Wero | ⚠ |
| GoPay | ⚠ |
| GCash | ⚠ |
| Momo | ⚠ |
| Dana | ⚠ |
| Kakao Pay | ⚠ |
| Touch 'n Go | ⚠ |
| Twint | ⚠ |
| Vipps | ⚠ |
| Swish | ⚠ |
| Affirm | ⚠ |
| Afterpay | ⚠ |
| Klarna | ⚠ |
| UPI Collect | ⚠ |
| UPI Intent | ⚠ |
| UPI QR | ⚠ |
| Thailand | ⚠ |
| Czech | ⚠ |
| Finland | ⚠ |
| FPX | ⚠ |
| Poland | ⚠ |
| Slovakia | ⚠ |
| UK | ⚠ |
| PIS | x |
| Generic | ⚠ |
| Local | ⚠ |
| iDEAL | ⚠ |
| Sofort | ⚠ |
| Trustly | ⚠ |
| Giropay | ⚠ |
| EPS | ⚠ |
| Przelewy24 | ⚠ |
| PSE | ⚠ |
| BLIK | ⚠ |
| Interac | ⚠ |
| Bizum | ⚠ |
| EFT | ⚠ |
| DuitNow | x |
| ACH | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| Multibanco | ⚠ |
| Instant | ⚠ |
| Instant FI | ⚠ |
| Instant PL | ⚠ |
| Pix | ⚠ |
| Permata | ⚠ |
| BCA | ⚠ |
| BNI VA | ⚠ |
| BRI VA | ⚠ |
| CIMB VA | ⚠ |
| Danamon VA | ⚠ |
| Mandiri VA | ⚠ |
| Local | ⚠ |
| Indonesian | ⚠ |
| ACH | x |
| SEPA | x |
| BACS | x |
| BECS | x |
| SEPA Guaranteed | x |
| Crypto | x |
| Reward | ⚠ |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | ⚠ |
| Boleto | ⚠ |
| Efecty | ⚠ |
| Pago Efectivo | ⚠ |
| Red Compra | ⚠ |
| Red Pagos | ⚠ |
| Alfamart | ⚠ |
| Indomaret | ⚠ |
| Oxxo | ⚠ |
| 7-Eleven | ⚠ |
| Lawson | ⚠ |
| Mini Stop | ⚠ |
| Family Mart | ⚠ |
| Seicomart | ⚠ |
| Pay Easy | ⚠ |

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### Card (Raw PAN)

```python
"payment_method": {
  "card": {
    "card_number": "4111111111111111",
    "card_exp_month": "03",
    "card_exp_year": "2030",
    "card_cvc": "737",
    "card_holder_name": "John Doe"
  }
}
```

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L354) · [Kotlin](../../examples/dlocal/dlocal.kt#L220) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L363) · [Kotlin](../../examples/dlocal/dlocal.kt#L232) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L372) · [Kotlin](../../examples/dlocal/dlocal.kt#L242) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L381) · [Kotlin](../../examples/dlocal/dlocal.kt#L250) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L390) · [Kotlin](../../examples/dlocal/dlocal.kt#L284) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L408) · [Kotlin](../../examples/dlocal/dlocal.kt#L360) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L426) · [Kotlin](../../examples/dlocal/dlocal.kt#L382) · [Rust](../../examples/dlocal/dlocal.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts) · [Kotlin](../../examples/dlocal/dlocal.kt#L426) · [Rust](../../examples/dlocal/dlocal.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L417) · [Kotlin](../../examples/dlocal/dlocal.kt#L370) · [Rust](../../examples/dlocal/dlocal.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L399) · [Kotlin](../../examples/dlocal/dlocal.kt#L321) · [Rust](../../examples/dlocal/dlocal.rs)
