# Wellsfargo

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/wellsfargo.json
Regenerate: python3 scripts/generators/docs/generate.py wellsfargo
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_MERCHANT_ACCOUNT`, `YOUR_API_SECRET`, `YOUR_BASE_URL` with your actual values.

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
        wellsfargo=payment_pb2.WellsfargoConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            merchant_account=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT"),
            api_secret=payment_methods_pb2.SecretString(value="YOUR_API_SECRET"),
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
    connector: Connector.WELLSFARGO,
    environment: Environment.SANDBOX,
    auth: {
        wellsfargo: {
            apiKey: { value: 'YOUR_API_KEY' },
            merchantAccount: { value: 'YOUR_MERCHANT_ACCOUNT' },
            apiSecret: { value: 'YOUR_API_SECRET' },
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
            .setWellsfargo(WellsfargoConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setMerchantAccount(SecretString.newBuilder().setValue("YOUR_MERCHANT_ACCOUNT").build())
                .setApiSecret(SecretString.newBuilder().setValue("YOUR_API_SECRET").build())
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
            config: Some(connector_specific_config::Config::Wellsfargo(WellsfargoConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                merchant_account: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ACCOUNT".to_string())),  // Authentication credential
                api_secret: Some(hyperswitch_masking::Secret::new("YOUR_API_SECRET".to_string())),  // Authentication credential
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL, e.g. https://sandbox.example.com
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py#L194) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L208) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L123) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs#L249)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py#L213) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L227) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L139) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs#L265)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py#L238) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L253) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L161) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs#L288)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py#L263) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L279) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L183) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs#L311)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py#L285) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L301) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L202) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs#L330)

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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L322) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L220) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L331) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L232) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L340) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L242) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L349) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L250) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L358) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L282) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L367) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L317) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L385) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L339) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L381) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.ts#L376) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.kt#L327) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/wellsfargo/wellsfargo.rs)
