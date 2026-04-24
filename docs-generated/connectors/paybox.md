# Paybox

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paybox.json
Regenerate: python3 scripts/generators/docs/generate.py paybox
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
        paybox=payment_pb2.PayboxConfig(
            site=payment_methods_pb2.SecretString(value="YOUR_SITE"),
            rank=payment_methods_pb2.SecretString(value="YOUR_RANK"),
            key=payment_methods_pb2.SecretString(value="YOUR_KEY"),
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
    connector: Connector.PAYBOX,
    environment: Environment.SANDBOX,
    auth: {
        paybox: {
            site: { value: 'YOUR_SITE' },
            rank: { value: 'YOUR_RANK' },
            key: { value: 'YOUR_KEY' },
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
            .setPaybox(PayboxConfig.newBuilder()
                .setSite(SecretString.newBuilder().setValue("YOUR_SITE").build())
                .setRank(SecretString.newBuilder().setValue("YOUR_RANK").build())
                .setKey(SecretString.newBuilder().setValue("YOUR_KEY").build())
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
            config: Some(connector_specific_config::Config::Paybox(PayboxConfig {
                site: Some(hyperswitch_masking::Secret::new("YOUR_SITE".to_string())),  // Authentication credential
                rank: Some(hyperswitch_masking::Secret::new("YOUR_RANK".to_string())),  // Authentication credential
                key: Some(hyperswitch_masking::Secret::new("YOUR_KEY".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/paybox/paybox.py#L181) · [JavaScript](../../examples/paybox/paybox.js) · [Kotlin](../../examples/paybox/paybox.kt#L119) · [Rust](../../examples/paybox/paybox.rs#L232)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/paybox/paybox.py#L200) · [JavaScript](../../examples/paybox/paybox.js) · [Kotlin](../../examples/paybox/paybox.kt#L135) · [Rust](../../examples/paybox/paybox.rs#L248)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/paybox/paybox.py#L225) · [JavaScript](../../examples/paybox/paybox.js) · [Kotlin](../../examples/paybox/paybox.kt#L157) · [Rust](../../examples/paybox/paybox.rs#L271)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/paybox/paybox.py#L250) · [JavaScript](../../examples/paybox/paybox.js) · [Kotlin](../../examples/paybox/paybox.kt#L179) · [Rust](../../examples/paybox/paybox.rs#L294)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/paybox/paybox.py#L272) · [JavaScript](../../examples/paybox/paybox.js) · [Kotlin](../../examples/paybox/paybox.kt#L198) · [Rust](../../examples/paybox/paybox.rs#L313)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
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
| Bancontact | x |
| Apple Pay | x |
| Apple Pay Dec | x |
| Apple Pay SDK | x |
| Google Pay | x |
| Google Pay Dec | x |
| Google Pay SDK | x |
| PayPal SDK | x |
| Amazon Pay | x |
| Cash App | x |
| PayPal | x |
| WeChat Pay | x |
| Alipay | x |
| Revolut Pay | x |
| MiFinity | x |
| Bluecode | x |
| Paze | x |
| Samsung Pay | x |
| MB Way | x |
| Satispay | x |
| Wero | x |
| Affirm | x |
| Afterpay | x |
| Klarna | x |
| UPI Collect | x |
| UPI Intent | x |
| UPI QR | x |
| Thailand | x |
| Czech | x |
| Finland | x |
| FPX | x |
| Poland | x |
| Slovakia | x |
| UK | x |
| PIS | x |
| Generic | x |
| Local | x |
| iDEAL | x |
| Sofort | x |
| Trustly | x |
| Giropay | x |
| EPS | x |
| Przelewy24 | x |
| PSE | x |
| BLIK | x |
| Interac | x |
| Bizum | x |
| EFT | x |
| DuitNow | x |
| ACH | x |
| SEPA | x |
| BACS | x |
| Multibanco | x |
| Instant | x |
| Instant FI | x |
| Instant PL | x |
| Pix | x |
| Permata | x |
| BCA | x |
| BNI VA | x |
| BRI VA | x |
| CIMB VA | x |
| Danamon VA | x |
| Mandiri VA | x |
| Local | x |
| Indonesian | x |
| ACH | x |
| SEPA | x |
| BACS | x |
| BECS | x |
| SEPA Guaranteed | x |
| Crypto | x |
| Reward | x |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | x |
| Boleto | x |
| Efecty | x |
| Pago Efectivo | x |
| Red Compra | x |
| Red Pagos | x |
| Alfamart | x |
| Indomaret | x |
| Oxxo | x |
| 7-Eleven | x |
| Lawson | x |
| Mini Stop | x |
| Family Mart | x |
| Seicomart | x |
| Pay Easy | x |

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

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L309) · [Kotlin](../../examples/paybox/paybox.kt#L216) · [Rust](../../examples/paybox/paybox.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L318) · [Kotlin](../../examples/paybox/paybox.kt#L228) · [Rust](../../examples/paybox/paybox.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L327) · [Kotlin](../../examples/paybox/paybox.kt#L238) · [Rust](../../examples/paybox/paybox.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L336) · [Kotlin](../../examples/paybox/paybox.kt#L246) · [Rust](../../examples/paybox/paybox.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L345) · [Kotlin](../../examples/paybox/paybox.kt#L274) · [Rust](../../examples/paybox/paybox.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L354) · [Kotlin](../../examples/paybox/paybox.kt#L305) · [Rust](../../examples/paybox/paybox.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L372) · [Kotlin](../../examples/paybox/paybox.kt#L327) · [Rust](../../examples/paybox/paybox.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts) · [Kotlin](../../examples/paybox/paybox.kt#L366) · [Rust](../../examples/paybox/paybox.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paybox/paybox.py) · [TypeScript](../../examples/paybox/paybox.ts#L363) · [Kotlin](../../examples/paybox/paybox.kt#L315) · [Rust](../../examples/paybox/paybox.rs)
