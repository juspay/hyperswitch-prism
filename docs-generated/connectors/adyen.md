# Adyen

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/adyen.json
Regenerate: python3 scripts/generators/docs/generate.py adyen
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
    #     adyen=payment_pb2.AdyenConfig(api_key=...),
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
    connector: Connector.ADYEN,
    environment: Environment.SANDBOX,
    // auth: { adyen: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Adyen credentials here
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

**Examples:** [Python](../../examples/adyen/adyen.py#L23) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L28) · [Rust](../../examples/adyen/adyen.rs#L30)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/adyen/adyen.py#L65) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L67) · [Rust](../../examples/adyen/adyen.rs#L56)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/adyen/adyen.py#L118) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L117) · [Rust](../../examples/adyen/adyen.rs#L94)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/adyen/adyen.py#L173) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L169) · [Rust](../../examples/adyen/adyen.rs#L134)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [create_client_authentication_token](#create_client_authentication_token) | Other | `—` |
| [create_order](#create_order) | Other | `—` |
| [dispute_accept](#dispute_accept) | Other | `—` |
| [dispute_defend](#dispute_defend) | Other | `—` |
| [dispute_submit_evidence](#dispute_submit_evidence) | Other | `—` |
| [handle_event](#handle_event) | Other | `—` |
| [parse_event](#parse_event) | Other | `—` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [proxy_setup_recurring](#proxy_setup_recurring) | Other | `—` |
| [recurring_charge](#recurring_charge) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [setup_recurring](#setup_recurring) | Other | `—` |
| [token_authorize](#token_authorize) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Bancontact | ✓ |
| Apple Pay | ✓ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | ⚠ |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
| Google Pay SDK | ⚠ |
| PayPal SDK | ⚠ |
| Amazon Pay | ⚠ |
| Cash App | ⚠ |
| PayPal | ✓ |
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
| Thailand | ✓ |
| Czech | ✓ |
| Finland | ✓ |
| FPX | ✓ |
| Poland | ⚠ |
| Slovakia | ✓ |
| UK | ✓ |
| PIS | x |
| Generic | ⚠ |
| Local | ⚠ |
| iDEAL | ✓ |
| Sofort | ⚠ |
| Trustly | ✓ |
| Giropay | ⚠ |
| EPS | ✓ |
| Przelewy24 | ⚠ |
| PSE | ⚠ |
| BLIK | ✓ |
| Interac | ⚠ |
| Bizum | ✓ |
| EFT | ⚠ |
| DuitNow | x |
| ACH | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| Multibanco | ⚠ |
| Instant | ⚠ |
| Instant FI | ⚠ |
| Instant PL | ⚠ |
| Pix | ✓ |
| Permata | ✓ |
| BCA | ✓ |
| BNI VA | ✓ |
| BRI VA | ✓ |
| CIMB VA | ✓ |
| Danamon VA | ✓ |
| Mandiri VA | ✓ |
| Local | ⚠ |
| Indonesian | ⚠ |
| ACH | ✓ |
| SEPA | ✓ |
| BACS | ✓ |
| BECS | ⚠ |
| SEPA Guaranteed | ⚠ |
| Crypto | x |
| Reward | ⚠ |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | ⚠ |
| Boleto | ✓ |
| Efecty | ⚠ |
| Pago Efectivo | ⚠ |
| Red Compra | ⚠ |
| Red Pagos | ⚠ |
| Alfamart | ✓ |
| Indomaret | ✓ |
| Oxxo | ✓ |
| 7-Eleven | ✓ |
| Lawson | ✓ |
| Mini Stop | ✓ |
| Family Mart | ✓ |
| Seicomart | ✓ |
| Pay Easy | ✓ |

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

##### SEPA Direct Debit

```python
"payment_method": {
  "sepa": {
    "iban": "DE89370400440532013000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### BACS Direct Debit

```python
"payment_method": {
  "bacs": {
    "account_number": "55779911",
    "sort_code": "200000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### ACH Direct Debit

```python
"payment_method": {
  "ach": {
    "account_number": "000123456789",
    "routing_number": "110000000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### iDEAL

```python
"payment_method": {
  "ideal": {}
}
```

##### PayPal Redirect

```python
"payment_method": {
  "paypal_redirect": {
    "email": "test@example.com"
  }
}
```

##### BLIK

```python
"payment_method": {
  "blik": {
    "blik_code": "777124"
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

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L182) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### capture

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L211) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### create_client_authentication_token

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L228) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### create_order

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L240) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### dispute_accept

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L252) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### dispute_defend

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L264) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### dispute_submit_evidence

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L277) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### handle_event

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L290) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### parse_event

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L302) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### proxy_authorize

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L313) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L334) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### recurring_charge

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L359) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### refund

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L382) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### setup_recurring

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L401) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### token_authorize

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts#L432) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)

#### void

**Examples:** [Python](../../examples/adyen/adyen.py) · [TypeScript](../../examples/adyen/adyen.ts) · [Kotlin](../../examples/adyen/adyen.kt) · [Rust](../../examples/adyen/adyen.rs)
