# Shift4

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/shift4.json
Regenerate: python3 scripts/generators/docs/generate.py shift4
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
    #     shift4=payment_pb2.Shift4Config(api_key=...),
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
    connector: Connector.SHIFT4,
    environment: Environment.SANDBOX,
    // auth: { shift4: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Shift4 credentials here
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

**Examples:** [Python](../../examples/shift4/shift4.py#L23) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L28) · [Rust](../../examples/shift4/shift4.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/shift4/shift4.py#L55) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L57) · [Rust](../../examples/shift4/shift4.rs#L55)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/shift4/shift4.py#L98) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L97) · [Rust](../../examples/shift4/shift4.rs#L92)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/shift4/shift4.py#L143) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L139) · [Rust](../../examples/shift4/shift4.rs#L131)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [create_client_authentication_token](#create_client_authentication_token) | Other | `—` |
| [create_customer](#create_customer) | Other | `—` |
| [get](#get) | Other | `—` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [recurring_charge](#recurring_charge) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [token_authorize](#token_authorize) | Other | `—` |

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
| iDEAL | ✓ |
| Sofort | x |
| Trustly | x |
| Giropay | x |
| EPS | ✓ |
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

##### iDEAL

```python
"payment_method": {
  "ideal": {}
}
```

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L176) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### capture

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L203) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### create_client_authentication_token

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L220) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### create_customer

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L232) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### get

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L245) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L258) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### recurring_charge

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L277) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### refund

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L300) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### refund_get

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L319) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)

#### token_authorize

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L331) · [Kotlin](../../examples/shift4/shift4.kt) · [Rust](../../examples/shift4/shift4.rs)
