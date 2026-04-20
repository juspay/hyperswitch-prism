# Fiserv

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiserv.json
Regenerate: python3 scripts/generators/docs/generate.py fiserv
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
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     fiserv=payment_pb2.FiservConfig(api_key=...),
    # ),
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
    connector: Connector.FISERV,
    environment: Environment.SANDBOX,
    // auth: { fiserv: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Fiserv credentials here
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

**Examples:** [Python](../../examples/fiserv/fiserv.py#L23) · [JavaScript](../../examples/fiserv/fiserv.js) · [Kotlin](../../examples/fiserv/fiserv.kt#L28) · [Rust](../../examples/fiserv/fiserv.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/fiserv/fiserv.py#L54) · [JavaScript](../../examples/fiserv/fiserv.js) · [Kotlin](../../examples/fiserv/fiserv.kt#L56) · [Rust](../../examples/fiserv/fiserv.rs#L55)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/fiserv/fiserv.py#L96) · [JavaScript](../../examples/fiserv/fiserv.js) · [Kotlin](../../examples/fiserv/fiserv.kt#L95) · [Rust](../../examples/fiserv/fiserv.rs#L92)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/fiserv/fiserv.py#L140) · [JavaScript](../../examples/fiserv/fiserv.js) · [Kotlin](../../examples/fiserv/fiserv.kt#L136) · [Rust](../../examples/fiserv/fiserv.rs#L131)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/fiserv/fiserv.py#L177) · [JavaScript](../../examples/fiserv/fiserv.js) · [Kotlin](../../examples/fiserv/fiserv.kt#L170) · [Rust](../../examples/fiserv/fiserv.rs#L163)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### authorize

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
| PaySafeCard | x |
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

**Examples:** [Python](../../examples/fiserv/fiserv.py) · [TypeScript](../../examples/fiserv/fiserv.ts#L212) · [Kotlin](../../examples/fiserv/fiserv.kt) · [Rust](../../examples/fiserv/fiserv.rs)

#### capture

**Examples:** [Python](../../examples/fiserv/fiserv.py) · [TypeScript](../../examples/fiserv/fiserv.ts#L239) · [Kotlin](../../examples/fiserv/fiserv.kt) · [Rust](../../examples/fiserv/fiserv.rs)

#### get

**Examples:** [Python](../../examples/fiserv/fiserv.py) · [TypeScript](../../examples/fiserv/fiserv.ts#L256) · [Kotlin](../../examples/fiserv/fiserv.kt) · [Rust](../../examples/fiserv/fiserv.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/fiserv/fiserv.py) · [TypeScript](../../examples/fiserv/fiserv.ts#L269) · [Kotlin](../../examples/fiserv/fiserv.kt) · [Rust](../../examples/fiserv/fiserv.rs)

#### refund

**Examples:** [Python](../../examples/fiserv/fiserv.py) · [TypeScript](../../examples/fiserv/fiserv.ts#L288) · [Kotlin](../../examples/fiserv/fiserv.kt) · [Rust](../../examples/fiserv/fiserv.rs)

#### refund_get

**Examples:** [Python](../../examples/fiserv/fiserv.py) · [TypeScript](../../examples/fiserv/fiserv.ts#L307) · [Kotlin](../../examples/fiserv/fiserv.kt) · [Rust](../../examples/fiserv/fiserv.rs)

#### void

**Examples:** [Python](../../examples/fiserv/fiserv.py) · [TypeScript](../../examples/fiserv/fiserv.ts) · [Kotlin](../../examples/fiserv/fiserv.kt) · [Rust](../../examples/fiserv/fiserv.rs)
