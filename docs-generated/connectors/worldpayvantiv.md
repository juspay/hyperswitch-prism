# Worldpayvantiv

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/worldpayvantiv.json
Regenerate: python3 scripts/generators/docs/generate.py worldpayvantiv
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_USER`, `YOUR_PASSWORD`, `YOUR_MERCHANT_ID`, `YOUR_BASE_URL`, `YOUR_REPORT_GROUP`, `YOUR_MERCHANT_CONFIG_CURRENCY`, `YOUR_SECONDARY_BASE_URL` with your actual values.

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
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL
                report_group: Some("YOUR_REPORT_GROUP".to_string()),  // Connector setting
                merchant_config_currency: Some("YOUR_MERCHANT_CONFIG_CURRENCY".to_string()),  // Connector setting
                secondary_base_url: Some("YOUR_SECONDARY_BASE_URL".to_string()),  // Endpoint URL
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py#L142) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L154) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L124) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs#L182)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py#L161) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L173) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L140) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs#L198)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py#L186) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L199) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L162) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs#L221)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py#L211) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L225) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L184) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs#L244)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py#L233) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L247) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L203) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs#L263)

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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L268) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L221) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L277) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L233) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L286) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L243) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.IncrementalAuthorization

Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceIncrementalAuthorizationRequest` |
| **Response** | `PaymentServiceIncrementalAuthorizationResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L295) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L251) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L304) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L267) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L313) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L296) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L331) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L318) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L326) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.ts#L322) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.kt#L306) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpayvantiv/worldpayvantiv.rs)
