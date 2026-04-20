# Revolv3

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/revolv3.json
Regenerate: python3 scripts/generators/docs/generate.py revolv3
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
    #     revolv3=payment_pb2.Revolv3Config(api_key=...),
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
    connector: Connector.REVOLV3,
    environment: Environment.SANDBOX,
    // auth: { revolv3: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Revolv3 credentials here
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

**Examples:** [Python](../../examples/revolv3/revolv3.py#L23) · [JavaScript](../../examples/revolv3/revolv3.js) · [Kotlin](../../examples/revolv3/revolv3.kt#L28) · [Rust](../../examples/revolv3/revolv3.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/revolv3/revolv3.py#L54) · [JavaScript](../../examples/revolv3/revolv3.js) · [Kotlin](../../examples/revolv3/revolv3.kt#L56) · [Rust](../../examples/revolv3/revolv3.rs#L55)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/revolv3/revolv3.py#L96) · [JavaScript](../../examples/revolv3/revolv3.js) · [Kotlin](../../examples/revolv3/revolv3.kt#L95) · [Rust](../../examples/revolv3/revolv3.rs#L92)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/revolv3/revolv3.py#L140) · [JavaScript](../../examples/revolv3/revolv3.js) · [Kotlin](../../examples/revolv3/revolv3.kt#L136) · [Rust](../../examples/revolv3/revolv3.rs#L131)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [proxy_setup_recurring](#proxy_setup_recurring) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [setup_recurring](#setup_recurring) | Other | `—` |
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

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts#L174) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)

#### capture

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts#L201) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts#L218) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts#L237) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)

#### refund

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts#L257) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)

#### refund_get

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts#L276) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)

#### setup_recurring

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts#L288) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)

#### void

**Examples:** [Python](../../examples/revolv3/revolv3.py) · [TypeScript](../../examples/revolv3/revolv3.ts) · [Kotlin](../../examples/revolv3/revolv3.kt) · [Rust](../../examples/revolv3/revolv3.rs)
