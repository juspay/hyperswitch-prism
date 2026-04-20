# CyberSource

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/cybersource.json
Regenerate: python3 scripts/generators/docs/generate.py cybersource
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
    #     cybersource=payment_pb2.CybersourceConfig(api_key=...),
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
    connector: Connector.CYBERSOURCE,
    environment: Environment.SANDBOX,
    // auth: { cybersource: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Cybersource credentials here
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

**Examples:** [Python](../../examples/cybersource/cybersource.py#L23) · [JavaScript](../../examples/cybersource/cybersource.js) · [Kotlin](../../examples/cybersource/cybersource.kt#L28) · [Rust](../../examples/cybersource/cybersource.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/cybersource/cybersource.py#L55) · [JavaScript](../../examples/cybersource/cybersource.js) · [Kotlin](../../examples/cybersource/cybersource.kt#L57) · [Rust](../../examples/cybersource/cybersource.rs#L56)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/cybersource/cybersource.py#L98) · [JavaScript](../../examples/cybersource/cybersource.js) · [Kotlin](../../examples/cybersource/cybersource.kt#L97) · [Rust](../../examples/cybersource/cybersource.rs#L94)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/cybersource/cybersource.py#L143) · [JavaScript](../../examples/cybersource/cybersource.js) · [Kotlin](../../examples/cybersource/cybersource.kt#L139) · [Rust](../../examples/cybersource/cybersource.rs#L134)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/cybersource/cybersource.py#L184) · [JavaScript](../../examples/cybersource/cybersource.js) · [Kotlin](../../examples/cybersource/cybersource.kt#L177) · [Rust](../../examples/cybersource/cybersource.rs#L169)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authenticate](#authenticate) | Other | `—` |
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [post_authenticate](#post_authenticate) | Other | `—` |
| [pre_authenticate](#pre_authenticate) | Other | `—` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [recurring_charge](#recurring_charge) | Other | `—` |
| [recurring_revoke](#recurring_revoke) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [refund_get](#refund_get) | Other | `—` |
| [token_authorize](#token_authorize) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### authenticate

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L225) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Bancontact | ⚠ |
| Apple Pay | ✓ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | ⚠ |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
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
| Samsung Pay | ✓ |
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

##### Google Pay

```python
"payment_method": {
  "google_pay": {
    "type": "CARD",
    "description": "Visa 1111",
    "info": {
      "card_network": "VISA",
      "card_details": "1111"
    },
    "tokenization_data": {
      "encrypted_data": {
        "token_type": "PAYMENT_GATEWAY",
        "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"
      }
    }
  }
}
```

##### Apple Pay

```python
"payment_method": {
  "apple_pay": {
    "payment_data": {
      "encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"
    },
    "payment_method": {
      "display_name": "Visa 1111",
      "network": "Visa",
      "type": "debit"
    },
    "transaction_identifier": "probe_txn_id"
  }
}
```

##### Samsung Pay

```python
"payment_method": {
  "samsung_pay": {
    "payment_credential": {
      "method": "3DS",
      "recurring_payment": false,
      "card_brand": "VISA",
      "card_last_four_digits": "1234",
      "token_data": {
        "type": "S",
        "version": "100",
        "data": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6InNhbXN1bmdfcHJvYmVfa2V5XzEyMyJ9.eyJwYXltZW50TWV0aG9kVG9rZW4iOiJwcm9iZV9zYW1zdW5nX3Rva2VuIn0.ZHVtbXlfc2lnbmF0dXJl"
      }
    }
  }
}
```

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L246) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### capture

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L275) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### get

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L292) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### post_authenticate

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L305) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### pre_authenticate

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L322) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L339) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### recurring_charge

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L360) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### recurring_revoke

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L383) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### refund

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L395) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### refund_get

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L414) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### token_authorize

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts#L426) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)

#### void

**Examples:** [Python](../../examples/cybersource/cybersource.py) · [TypeScript](../../examples/cybersource/cybersource.ts) · [Kotlin](../../examples/cybersource/cybersource.kt) · [Rust](../../examples/cybersource/cybersource.rs)
