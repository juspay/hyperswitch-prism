# Citigate

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/citigate.json
Regenerate: python3 scripts/generators/docs/generate.py citigate
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_KEY1`, `YOUR_BASE_URL` with your actual values.

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
        citigate=payment_pb2.CitigateConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            key1=payment_methods_pb2.SecretString(value="YOUR_KEY1"),
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
    connector: Connector.CITIGATE,
    environment: Environment.SANDBOX,
    auth: {
        citigate: {
            apiKey: { value: 'YOUR_API_KEY' },
            key1: { value: 'YOUR_KEY1' },
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
            .setCitigate(CitigateConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setKey1(SecretString.newBuilder().setValue("YOUR_KEY1").build())
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
            config: Some(connector_specific_config::Config::Citigate(CitigateConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                key1: Some(hyperswitch_masking::Secret::new("YOUR_KEY1".to_string())),  // Authentication credential
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py#L149) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L157) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L127) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs#L185)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py#L168) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L176) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L143) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs#L201)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py#L193) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L202) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L165) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs#L224)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py#L218) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L228) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L187) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs#L247)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py#L240) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L250) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L206) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs#L266)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L271) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L224) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L280) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L236) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L289) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L246) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L298) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L254) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L307) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L296) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L318) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.ts#L316) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.kt#L306) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/citigate/citigate.rs)
