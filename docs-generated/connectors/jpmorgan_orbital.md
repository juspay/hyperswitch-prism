# Jpmorgan Orbital

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/jpmorgan_orbital.json
Regenerate: python3 scripts/generators/docs/generate.py jpmorgan_orbital
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
        jpmorgan_orbital=payment_pb2.JpmorganOrbitalConfig(
            username=payment_methods_pb2.SecretString(value="YOUR_USERNAME"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            bin="YOUR_BIN",
            terminal_id="YOUR_TERMINAL_ID",
            base_url="YOUR_BASE_URL",
            merchant_config_currency="YOUR_MERCHANT_CONFIG_CURRENCY",
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
    connector: Connector.JPMORGAN_ORBITAL,
    environment: Environment.SANDBOX,
    auth: {
        jpmorganOrbital: {
            username: { value: 'YOUR_USERNAME' },
            password: { value: 'YOUR_PASSWORD' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            bin: 'YOUR_BIN',
            terminalId: 'YOUR_TERMINAL_ID',
            baseUrl: 'YOUR_BASE_URL',
            merchantConfigCurrency: 'YOUR_MERCHANT_CONFIG_CURRENCY',
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
            .setJpmorganOrbital(JpmorganOrbitalConfig.newBuilder()
                .setUsername(SecretString.newBuilder().setValue("YOUR_USERNAME").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setBin("YOUR_BIN")
                .setTerminalId("YOUR_TERMINAL_ID")
                .setBaseUrl("YOUR_BASE_URL")
                .setMerchantConfigCurrency("YOUR_MERCHANT_CONFIG_CURRENCY")
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
    connector_config: None,  // TODO: Add your connector config here,
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

**Examples:** [Python](../../examples/jpmorgan_orbital/jpmorgan_orbital.py#L89) · [JavaScript](../../examples/jpmorgan_orbital/jpmorgan_orbital.js) · [Kotlin](../../examples/jpmorgan_orbital/jpmorgan_orbital.kt#L85) · [Rust](../../examples/jpmorgan_orbital/jpmorgan_orbital.rs#L107)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/jpmorgan_orbital/jpmorgan_orbital.py#L108) · [JavaScript](../../examples/jpmorgan_orbital/jpmorgan_orbital.js) · [Kotlin](../../examples/jpmorgan_orbital/jpmorgan_orbital.kt#L101) · [Rust](../../examples/jpmorgan_orbital/jpmorgan_orbital.rs#L123)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |

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

**Examples:** [Python](../../examples/jpmorgan_orbital/jpmorgan_orbital.py) · [TypeScript](../../examples/jpmorgan_orbital/jpmorgan_orbital.ts#L136) · [Kotlin](../../examples/jpmorgan_orbital/jpmorgan_orbital.kt#L119) · [Rust](../../examples/jpmorgan_orbital/jpmorgan_orbital.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/jpmorgan_orbital/jpmorgan_orbital.py) · [TypeScript](../../examples/jpmorgan_orbital/jpmorgan_orbital.ts#L145) · [Kotlin](../../examples/jpmorgan_orbital/jpmorgan_orbital.kt#L131) · [Rust](../../examples/jpmorgan_orbital/jpmorgan_orbital.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/jpmorgan_orbital/jpmorgan_orbital.py) · [TypeScript](../../examples/jpmorgan_orbital/jpmorgan_orbital.ts#L154) · [Kotlin](../../examples/jpmorgan_orbital/jpmorgan_orbital.kt#L139) · [Rust](../../examples/jpmorgan_orbital/jpmorgan_orbital.rs)
