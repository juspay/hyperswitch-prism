# Payme

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/payme.json
Regenerate: python3 scripts/generators/docs/generate.py payme
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
    #     payme=payment_pb2.PaymeConfig(api_key=...),
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
    connector: Connector.PAYME,
    environment: Environment.SANDBOX,
    // auth: { payme: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Payme credentials here
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

**Examples:** [Python](../../examples/payme/payme.py#L23) · [JavaScript](../../examples/payme/payme.js) · [Kotlin](../../examples/payme/payme.kt#L28) · [Rust](../../examples/payme/payme.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/payme/payme.py#L57) · [JavaScript](../../examples/payme/payme.js) · [Kotlin](../../examples/payme/payme.kt#L59) · [Rust](../../examples/payme/payme.rs#L57)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/payme/payme.py#L102) · [JavaScript](../../examples/payme/payme.js) · [Kotlin](../../examples/payme/payme.kt#L101) · [Rust](../../examples/payme/payme.rs#L96)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/payme/payme.py#L149) · [JavaScript](../../examples/payme/payme.js) · [Kotlin](../../examples/payme/payme.kt#L145) · [Rust](../../examples/payme/payme.rs#L137)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/payme/payme.py#L191) · [JavaScript](../../examples/payme/payme.js) · [Kotlin](../../examples/payme/payme.kt#L184) · [Rust](../../examples/payme/payme.rs#L172)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [create_order](#create_order) | Other | `—` |
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

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts#L229) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)

#### capture

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts#L259) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)

#### create_order

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts#L276) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)

#### get

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts#L288) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts#L301) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)

#### refund

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts#L323) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)

#### refund_get

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts#L342) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)

#### void

**Examples:** [Python](../../examples/payme/payme.py) · [TypeScript](../../examples/payme/payme.ts) · [Kotlin](../../examples/payme/payme.kt) · [Rust](../../examples/payme/payme.rs)
