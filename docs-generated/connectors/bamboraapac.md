# Bamboraapac

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/bamboraapac.json
Regenerate: python3 scripts/generators/docs/generate.py bamboraapac
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_USERNAME`, `YOUR_PASSWORD`, `YOUR_ACCOUNT_NUMBER`, `YOUR_BASE_URL` with your actual values.

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
        bamboraapac=payment_pb2.BamboraapacConfig(
            username=payment_methods_pb2.SecretString(value="YOUR_USERNAME"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
            account_number=payment_methods_pb2.SecretString(value="YOUR_ACCOUNT_NUMBER"),
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
    connector: Connector.BAMBORAAPAC,
    environment: Environment.SANDBOX,
    auth: {
        bamboraapac: {
            username: { value: 'YOUR_USERNAME' },
            password: { value: 'YOUR_PASSWORD' },
            accountNumber: { value: 'YOUR_ACCOUNT_NUMBER' },
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
            .setBamboraapac(BamboraapacConfig.newBuilder()
                .setUsername(SecretString.newBuilder().setValue("YOUR_USERNAME").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setAccountNumber(SecretString.newBuilder().setValue("YOUR_ACCOUNT_NUMBER").build())
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
            config: Some(connector_specific_config::Config::Bamboraapac(BamboraapacConfig {
                username: Some(hyperswitch_masking::Secret::new("YOUR_USERNAME".to_string())),  // Authentication credential
                password: Some(hyperswitch_masking::Secret::new("YOUR_PASSWORD".to_string())),  // Authentication credential
                account_number: Some(hyperswitch_masking::Secret::new("YOUR_ACCOUNT_NUMBER".to_string())),  // Authentication credential
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py#L195) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L205) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L111) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs#L246)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py#L214) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L224) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L127) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs#L262)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py#L239) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L250) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L149) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs#L285)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py#L264) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L276) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L171) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs#L308)

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
| GoPay | x |
| GCash | x |
| Momo | x |
| Dana | x |
| Kakao Pay | x |
| Touch 'n Go | x |
| Twint | x |
| Vipps | x |
| Swish | x |
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L297) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L189) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L306) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L201) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L315) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L211) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L324) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L219) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L333) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L248) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L351) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L311) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L369) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L333) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L360) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L321) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.ts#L342) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.kt#L280) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/bamboraapac/bamboraapac.rs)
