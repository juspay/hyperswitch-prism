# Bluesnap

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/bluesnap.json
Regenerate: python3 scripts/generators/docs/generate.py bluesnap
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Javascript</b></td><td><b>Kotlin</b></td><td><b>Python</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Javascript</summary>

```javascript
import { DirectPaymentClient, types } from 'hyperswitch-prism';

const config: types.IConnectorConfig = types.ConnectorConfig.create({
    options: types.SdkOptions.create({ environment: types.Environment.SANDBOX }),
    connectorConfig: types.ConnectorSpecificConfig.create({
        bluesnap: {
        username: { value: 'YOUR_USERNAME' },
        password: { value: 'YOUR_PASSWORD' },
        },
    }),
});
const client = new DirectPaymentClient(config);
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
import payments.DirectPaymentClient
import payments.ConnectorConfig
import payments.Environment

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = DirectPaymentClient(config)
```

</details>

</td>
<td valign="top">

<details><summary>Python</summary>

```python
from payments import PaymentClient
from payments.generated import sdk_config_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
client = PaymentClient(config)
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;

let config = ConnectorConfig {
    connector_config: Some(ConnectorSpecificConfig {
        config: Some(connector_specific_config::Config::Bluesnap(BluesnapConfig {
                username: Some(Secret::new("YOUR_USERNAME".to_string())),
                password: Some(Secret::new("YOUR_PASSWORD".to_string())),
            ..Default::default()
        })),
    }),
    options: Some(SdkOptions { environment: Environment::Sandbox.into() }),
};
let client = ConnectorClient::new(config, None).unwrap();
```

</details>

</td>
</tr>
</table>

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

### Card Payment (Authorize + Capture)

Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py#L27) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.js#L28) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L26) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py#L63) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.js#L84) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L40) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L68)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py#L90) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.js#L126) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L50) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L105)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py#L124) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.js#L175) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L60) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L149)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py#L152) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.js#L217) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L70) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L186)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py#L190) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.js#L275) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L84) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L238)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py#L222) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.js#L323) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L98) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L284)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [void](#void) | Other | `—` |

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | ✓ |
| Apple Pay | ✓ |
| SEPA | ⚠ |
| BACS | ⚠ |
| ACH | ✓ |
| BECS | ⚠ |
| iDEAL | ⚠ |
| PayPal | ⚠ |
| BLIK | ⚠ |
| Klarna | ⚠ |
| Afterpay | ⚠ |
| UPI | ⚠ |
| Affirm | ⚠ |
| Samsung Pay | ⚠ |

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### Card (Raw PAN)

```python
"payment_method": {
    "card": {  # Generic card payment
        "card_number": {"value": "4111111111111111"},  # Card Identification
        "card_exp_month": {"value": "03"},
        "card_exp_year": {"value": "2030"},
        "card_cvc": {"value": "737"},
        "card_holder_name": {"value": "John Doe"}  # Cardholder Information
    }
}
```

##### Google Pay

```python
"payment_method": {
    "google_pay": {  # Google Pay
        "type": "CARD",  # Type of payment method
        "description": "Visa 1111",  # User-facing description of the payment method
        "info": {
            "card_network": "VISA",  # Card network name
            "card_details": "1111"  # Card details (usually last 4 digits)
        },
        "tokenization_data": {
            "encrypted_data": {  # Encrypted Google Pay payment data
                "token_type": "PAYMENT_GATEWAY",  # The type of the token
                "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"  # Token generated for the wallet
            }
        }
    }
}
```

##### Apple Pay

```python
"payment_method": {
    "apple_pay": {  # Apple Pay
        "payment_data": {
            "encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"  # Encrypted Apple Pay payment data as string
        },
        "payment_method": {
            "display_name": "Visa 1111",
            "network": "Visa",
            "type": "debit"
        },
        "transaction_identifier": "probe_txn_id"  # Transaction identifier
    }
}
```

##### ACH Direct Debit

```python
"payment_method": {
    "ach": {  # Ach - Automated Clearing House
        "account_number": {"value": "000123456789"},  # Account number for ach bank debit payment
        "routing_number": {"value": "110000000"},  # Routing number for ach bank debit payment
        "bank_account_holder_name": {"value": "John Doe"}  # Bank account holder name
    }
}
```

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.ts#L373) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L112) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L334)

#### capture

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.ts#L411) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L120) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L369)

#### get

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.ts#L430) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L128) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L386)

#### refund

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.ts#L445) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L136) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L403)

#### void

**Examples:** [Python](../../examples/bluesnap/python/bluesnap.py) · [JavaScript](../../examples/bluesnap/javascript/bluesnap.ts#L466) · [Kotlin](../../examples/bluesnap/kotlin/bluesnap.kt#L144) · [Rust](../../examples/bluesnap/rust/bluesnap.rs#L422)
