# Fiservemea

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiservemea.json
Regenerate: python3 scripts/generators/docs/generate.py fiservemea
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_API_SECRET`, `YOUR_BASE_URL` with your actual values.

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
        fiservemea=payment_pb2.FiservemeaConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
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
    connector: Connector.FISERVEMEA,
    environment: Environment.SANDBOX,
    auth: {
        fiservemea: {
            apiKey: { value: 'YOUR_API_KEY' },
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
            .setFiservemea(FiservemeaConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
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
            config: Some(connector_specific_config::Config::Fiservemea(FiservemeaConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                api_secret: Some(hyperswitch_masking::Secret::new("YOUR_API_SECRET".to_string())),  // Authentication credential
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py#L127) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L138) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L120) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs#L165)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py#L146) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L157) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L136) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs#L181)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py#L171) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L183) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L158) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs#L204)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py#L196) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L209) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L180) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs#L227)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py#L218) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L231) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L199) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs#L246)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
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
| ACH | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| BECS | ⚠ |
| SEPA Guaranteed | ⚠ |
| Crypto | x |
| Reward | ⚠ |
| Givex | x |
| PaySafeCard | ⚠ |
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L252) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L217) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L261) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L229) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L270) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L239) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L279) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L247) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L288) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L276) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L306) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L298) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L306) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.ts#L297) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.kt#L286) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/fiservemea/fiservemea.rs)
