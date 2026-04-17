# Powertranz

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/powertranz.json
Regenerate: python3 scripts/generators/docs/generate.py powertranz
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
        powertranz=payment_pb2.PowertranzConfig(
            power_tranz_id=payment_methods_pb2.SecretString(value="YOUR_POWER_TRANZ_ID"),
            power_tranz_password=payment_methods_pb2.SecretString(value="YOUR_POWER_TRANZ_PASSWORD"),
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
    connector: Connector.POWERTRANZ,
    environment: Environment.SANDBOX,
    auth: {
        powertranz: {
            powerTranzId: { value: 'YOUR_POWER_TRANZ_ID' },
            powerTranzPassword: { value: 'YOUR_POWER_TRANZ_PASSWORD' },
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
            .setPowertranz(PowertranzConfig.newBuilder()
                .setPowerTranzId(SecretString.newBuilder().setValue("YOUR_POWER_TRANZ_ID").build())
                .setPowerTranzPassword(SecretString.newBuilder().setValue("YOUR_POWER_TRANZ_PASSWORD").build())
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
            config: Some(connector_specific_config::Config::Powertranz(PowertranzConfig {
                power_tranz_id: Some(hyperswitch_masking::Secret::new("YOUR_POWER_TRANZ_ID".to_string())),  // Authentication credential
                power_tranz_password: Some(hyperswitch_masking::Secret::new("YOUR_POWER_TRANZ_PASSWORD".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/powertranz/powertranz.py#L120) · [JavaScript](../../examples/powertranz/powertranz.js) · [Kotlin](../../examples/powertranz/powertranz.kt#L111) · [Rust](../../examples/powertranz/powertranz.rs#L156)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/powertranz/powertranz.py#L139) · [JavaScript](../../examples/powertranz/powertranz.js) · [Kotlin](../../examples/powertranz/powertranz.kt#L127) · [Rust](../../examples/powertranz/powertranz.rs#L172)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/powertranz/powertranz.py#L164) · [JavaScript](../../examples/powertranz/powertranz.js) · [Kotlin](../../examples/powertranz/powertranz.kt#L149) · [Rust](../../examples/powertranz/powertranz.rs#L195)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/powertranz/powertranz.py#L189) · [JavaScript](../../examples/powertranz/powertranz.js) · [Kotlin](../../examples/powertranz/powertranz.kt#L171) · [Rust](../../examples/powertranz/powertranz.rs#L218)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/powertranz/powertranz.py#L211) · [JavaScript](../../examples/powertranz/powertranz.js) · [Kotlin](../../examples/powertranz/powertranz.kt#L190) · [Rust](../../examples/powertranz/powertranz.rs#L237)

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

**Examples:** [Python](../../examples/powertranz/powertranz.py) · [TypeScript](../../examples/powertranz/powertranz.ts#L244) · [Kotlin](../../examples/powertranz/powertranz.kt#L208) · [Rust](../../examples/powertranz/powertranz.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/powertranz/powertranz.py) · [TypeScript](../../examples/powertranz/powertranz.ts#L253) · [Kotlin](../../examples/powertranz/powertranz.kt#L220) · [Rust](../../examples/powertranz/powertranz.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/powertranz/powertranz.py) · [TypeScript](../../examples/powertranz/powertranz.ts#L262) · [Kotlin](../../examples/powertranz/powertranz.kt#L230) · [Rust](../../examples/powertranz/powertranz.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/powertranz/powertranz.py) · [TypeScript](../../examples/powertranz/powertranz.ts#L271) · [Kotlin](../../examples/powertranz/powertranz.kt#L238) · [Rust](../../examples/powertranz/powertranz.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/powertranz/powertranz.py) · [TypeScript](../../examples/powertranz/powertranz.ts#L280) · [Kotlin](../../examples/powertranz/powertranz.kt#L266) · [Rust](../../examples/powertranz/powertranz.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/powertranz/powertranz.py) · [TypeScript](../../examples/powertranz/powertranz.ts) · [Kotlin](../../examples/powertranz/powertranz.kt#L288) · [Rust](../../examples/powertranz/powertranz.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/powertranz/powertranz.py) · [TypeScript](../../examples/powertranz/powertranz.ts#L289) · [Kotlin](../../examples/powertranz/powertranz.kt#L276) · [Rust](../../examples/powertranz/powertranz.rs)
