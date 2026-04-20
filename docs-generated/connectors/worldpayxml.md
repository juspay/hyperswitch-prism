# Worldpayxml

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/worldpayxml.json
Regenerate: python3 scripts/generators/docs/generate.py worldpayxml
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
        worldpayxml=payment_pb2.WorldpayxmlConfig(
            api_username=payment_methods_pb2.SecretString(value="YOUR_API_USERNAME"),
            api_password=payment_methods_pb2.SecretString(value="YOUR_API_PASSWORD"),
            merchant_code=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_CODE"),
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
    connector: Connector.WORLDPAYXML,
    environment: Environment.SANDBOX,
    auth: {
        worldpayxml: {
            apiUsername: { value: 'YOUR_API_USERNAME' },
            apiPassword: { value: 'YOUR_API_PASSWORD' },
            merchantCode: { value: 'YOUR_MERCHANT_CODE' },
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
            .setWorldpayxml(WorldpayxmlConfig.newBuilder()
                .setApiUsername(SecretString.newBuilder().setValue("YOUR_API_USERNAME").build())
                .setApiPassword(SecretString.newBuilder().setValue("YOUR_API_PASSWORD").build())
                .setMerchantCode(SecretString.newBuilder().setValue("YOUR_MERCHANT_CODE").build())
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
            config: Some(connector_specific_config::Config::Worldpayxml(WorldpayxmlConfig {
                api_username: Some(hyperswitch_masking::Secret::new("YOUR_API_USERNAME".to_string())),  // Authentication credential
                api_password: Some(hyperswitch_masking::Secret::new("YOUR_API_PASSWORD".to_string())),  // Authentication credential
                merchant_code: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_CODE".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L121) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L112) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L157)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L140) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L128) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L173)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L165) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L150) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L196)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L190) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L172) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L219)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L212) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L191) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L238)

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

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L245) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L209) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L254) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L221) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L263) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L231) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L272) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L239) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L281) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L267) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L289) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L290) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L277) · [Rust](../../examples/worldpayxml/worldpayxml.rs)
