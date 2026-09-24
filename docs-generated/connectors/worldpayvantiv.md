# Worldpayvantiv

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/worldpayvantiv.json
Regenerate: python3 scripts/generators/docs/generate.py worldpayvantiv
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
        worldpayvantiv=payment_pb2.WorldpayvantivConfig(
            user=payment_methods_pb2.SecretString(value="YOUR_USER"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            base_url="YOUR_BASE_URL",
            report_group="YOUR_REPORT_GROUP",
            merchant_config_currency="YOUR_MERCHANT_CONFIG_CURRENCY",
            secondary_base_url="YOUR_SECONDARY_BASE_URL",
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
    connector: Connector.WORLDPAYVANTIV,
    environment: Environment.SANDBOX,
    auth: {
        worldpayvantiv: {
            user: { value: 'YOUR_USER' },
            password: { value: 'YOUR_PASSWORD' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            baseUrl: 'YOUR_BASE_URL',
            reportGroup: 'YOUR_REPORT_GROUP',
            merchantConfigCurrency: 'YOUR_MERCHANT_CONFIG_CURRENCY',
            secondaryBaseUrl: 'YOUR_SECONDARY_BASE_URL',
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
            .setWorldpayvantiv(WorldpayvantivConfig.newBuilder()
                .setUser(SecretString.newBuilder().setValue("YOUR_USER").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setReportGroup("YOUR_REPORT_GROUP")
                .setMerchantConfigCurrency("YOUR_MERCHANT_CONFIG_CURRENCY")
                .setSecondaryBaseUrl("YOUR_SECONDARY_BASE_URL")
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
            config: Some(connector_specific_config::Config::Worldpayvantiv(WorldpayvantivConfig {
                user: Some(hyperswitch_masking::Secret::new("YOUR_USER".to_string())),  // Authentication credential
                password: Some(hyperswitch_masking::Secret::new("YOUR_PASSWORD".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                report_group: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                merchant_config_currency: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                secondary_base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L142) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L124) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L182)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L161) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L140) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L198)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L186) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L162) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L221)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L211) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L184) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L244)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L233) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L203) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L263)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.IncrementalAuthorization](#paymentserviceincrementalauthorization) | Payments | `PaymentServiceIncrementalAuthorizationRequest` |
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
| Bancontact | ? |
| Apple Pay | ? |
| Apple Pay Dec | ? |
| Apple Pay SDK | ? |
| Google Pay | ? |
| Google Pay Dec | ? |
| Google Pay SDK | ? |
| PayPal SDK | ? |
| Amazon Pay | ? |
| Cash App | ? |
| PayPal | ? |
| WeChat Pay | ? |
| Alipay | ? |
| Revolut Pay | ? |
| MiFinity | ? |
| Bluecode | ? |
| Paze | ⚠ |
| Samsung Pay | ? |
| MB Way | ? |
| Satispay | ? |
| Wero | ? |
| GoPay | ? |
| GCash | ? |
| Momo | ? |
| Dana | ? |
| Kakao Pay | ? |
| Touch 'n Go | ? |
| Twint | ? |
| Vipps | ? |
| Swish | ? |
| Affirm | ? |
| Afterpay | ? |
| Klarna | ? |
| UPI Collect | ? |
| UPI Intent | ? |
| UPI QR | ? |
| Thailand | ? |
| Czech | ? |
| Finland | ? |
| FPX | ? |
| Poland | ? |
| Slovakia | ? |
| UK | ? |
| PIS | ⚠ |
| Generic | ? |
| WebPay | ? |
| Local | ? |
| iDEAL | ? |
| Sofort | ? |
| Trustly | ? |
| Giropay | ? |
| EPS | ? |
| Przelewy24 | ? |
| PSE | ? |
| BLIK | ? |
| Interac | ? |
| Bizum | ? |
| EFT | ? |
| DuitNow | ⚠ |
| ACH | ? |
| SEPA | ? |
| BACS | ? |
| Multibanco | ? |
| Instant | ? |
| Instant FI | ? |
| Instant PL | ? |
| Pix | ? |
| Permata | ? |
| BCA | ? |
| BNI VA | ? |
| BRI VA | ? |
| CIMB VA | ? |
| Danamon VA | ? |
| Mandiri VA | ? |
| Local | ? |
| Indonesian | ? |
| ACH | ? |
| SEPA | ? |
| BACS | ? |
| BECS | ? |
| SEPA Guaranteed | ? |
| Crypto | ⚠ |
| Reward | ? |
| Givex | ⚠ |
| PaySafeCard | ? |
| E-Voucher | ? |
| Boleto | ? |
| Efecty | ? |
| Pago Efectivo | ? |
| Red Compra | ? |
| Red Pagos | ? |
| Alfamart | ? |
| Indomaret | ? |
| Oxxo | ? |
| 7-Eleven | ? |
| Lawson | ? |
| Mini Stop | ? |
| Family Mart | ? |
| Seicomart | ? |
| Pay Easy | ? |

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

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L268) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L221) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L277) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L233) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L286) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L243) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.IncrementalAuthorization

Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceIncrementalAuthorizationRequest` |
| **Response** | `PaymentServiceIncrementalAuthorizationResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L295) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L251) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L304) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L267) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L313) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L296) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L331) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L318) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L326) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L322) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L306) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs)
