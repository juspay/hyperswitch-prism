# ACI

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/aci.json
Regenerate: python3 scripts/generators/docs/generate.py aci
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
    #     aci=payment_pb2.AciConfig(api_key=...),
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
    connector: Connector.ACI,
    environment: Environment.SANDBOX,
    // auth: { aci: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Aci credentials here
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

**Examples:** [Python](../../examples/aci/aci.py#L23) · [JavaScript](../../examples/aci/aci.js) · [Kotlin](../../examples/aci/aci.kt#L28) · [Rust](../../examples/aci/aci.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/aci/aci.py#L55) · [JavaScript](../../examples/aci/aci.js) · [Kotlin](../../examples/aci/aci.kt#L57) · [Rust](../../examples/aci/aci.rs#L55)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/aci/aci.py#L98) · [JavaScript](../../examples/aci/aci.js) · [Kotlin](../../examples/aci/aci.kt#L97) · [Rust](../../examples/aci/aci.rs#L92)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/aci/aci.py#L143) · [JavaScript](../../examples/aci/aci.js) · [Kotlin](../../examples/aci/aci.kt#L139) · [Rust](../../examples/aci/aci.rs#L131)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/aci/aci.py#L181) · [JavaScript](../../examples/aci/aci.js) · [Kotlin](../../examples/aci/aci.kt#L174) · [Rust](../../examples/aci/aci.rs#L163)

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
| Alipay | ✓ |
| Revolut Pay | ⚠ |
| MiFinity | ⚠ |
| Bluecode | ⚠ |
| Paze | x |
| Samsung Pay | ⚠ |
| MB Way | ⚠ |
| Satispay | ⚠ |
| Wero | ⚠ |
| Affirm | ✓ |
| Afterpay | ✓ |
| Klarna | ✓ |
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
| iDEAL | ✓ |
| Sofort | ✓ |
| Trustly | ✓ |
| Giropay | ✓ |
| EPS | ✓ |
| Przelewy24 | ✓ |
| PSE | ⚠ |
| BLIK | ⚠ |
| Interac | ✓ |
| Bizum | ⚠ |
| EFT | ✓ |
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

##### iDEAL

```python
"payment_method": {
  "ideal": {
    "bank_name": "Ing"
  }
}
```

##### Klarna

```python
"payment_method": {
  "klarna": {}
}
```

##### Afterpay / Clearpay

```python
"payment_method": {
  "afterpay_clearpay": {}
}
```

##### Affirm

```python
"payment_method": {
  "affirm": {}
}
```

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L212) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### capture

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L239) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### get

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L256) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L269) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L288) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### recurring_charge

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L308) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### refund

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L331) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### setup_recurring

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts#L350) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)

#### void

**Examples:** [Python](../../examples/aci/aci.py) · [TypeScript](../../examples/aci/aci.ts) · [Kotlin](../../examples/aci/aci.kt) · [Rust](../../examples/aci/aci.rs)
