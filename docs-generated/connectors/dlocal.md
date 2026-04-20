# dLocal

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/dlocal.json
Regenerate: python3 scripts/generators/docs/generate.py dlocal
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
    #     dlocal=payment_pb2.DlocalConfig(api_key=...),
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
    connector: Connector.DLOCAL,
    environment: Environment.SANDBOX,
    // auth: { dlocal: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Dlocal credentials here
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

**Examples:** [Python](../../examples/dlocal/dlocal.py#L23) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L28) · [Rust](../../examples/dlocal/dlocal.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/dlocal/dlocal.py#L57) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L59) · [Rust](../../examples/dlocal/dlocal.rs#L56)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/dlocal/dlocal.py#L102) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L101) · [Rust](../../examples/dlocal/dlocal.rs#L94)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/dlocal/dlocal.py#L149) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L145) · [Rust](../../examples/dlocal/dlocal.rs#L134)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/dlocal/dlocal.py#L189) · [JavaScript](../../examples/dlocal/dlocal.js) · [Kotlin](../../examples/dlocal/dlocal.kt#L182) · [Rust](../../examples/dlocal/dlocal.rs#L167)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [proxy_setup_recurring](#proxy_setup_recurring) | Other | `—` |
| [recurring_charge](#recurring_charge) | Other | `—` |
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
| ACH | x |
| SEPA | x |
| BACS | ⚠ |
| BECS | ⚠ |
| SEPA Guaranteed | x |
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

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L222) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### capture

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L251) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### get

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L268) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L281) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L302) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### recurring_charge

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L324) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### refund

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L350) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### refund_get

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L369) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### setup_recurring

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts#L381) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)

#### void

**Examples:** [Python](../../examples/dlocal/dlocal.py) · [TypeScript](../../examples/dlocal/dlocal.ts) · [Kotlin](../../examples/dlocal/dlocal.kt) · [Rust](../../examples/dlocal/dlocal.rs)
