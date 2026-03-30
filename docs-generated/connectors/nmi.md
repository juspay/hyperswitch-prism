# Nmi

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/nmi.json
Regenerate: python3 scripts/generators/docs/generate.py nmi
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
        nmi: {
        apiKey: { value: 'YOUR_API_KEY' },
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
        config: Some(connector_specific_config::Config::Nmi(NmiConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
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

**Examples:** [Python](../../examples/nmi/python/nmi.py#L25) · [JavaScript](../../examples/nmi/javascript/nmi.js#L27) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L24) · [Rust](../../examples/nmi/rust/nmi.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/nmi/python/nmi.py#L61) · [JavaScript](../../examples/nmi/javascript/nmi.js#L83) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L38) · [Rust](../../examples/nmi/rust/nmi.rs#L68)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/nmi/python/nmi.py#L88) · [JavaScript](../../examples/nmi/javascript/nmi.js#L125) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L48) · [Rust](../../examples/nmi/rust/nmi.rs#L105)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/nmi/python/nmi.py#L113) · [JavaScript](../../examples/nmi/javascript/nmi.js#L165) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L58) · [Rust](../../examples/nmi/rust/nmi.rs#L140)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/nmi/python/nmi.py#L151) · [JavaScript](../../examples/nmi/javascript/nmi.js#L223) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L72) · [Rust](../../examples/nmi/rust/nmi.rs#L192)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/nmi/python/nmi.py#L183) · [JavaScript](../../examples/nmi/javascript/nmi.js#L271) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L86) · [Rust](../../examples/nmi/rust/nmi.rs#L238)

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
| Google Pay | ⚠ |
| Apple Pay | ⚠ |
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

**Examples:** [Python](../../examples/nmi/python/nmi.py) · [JavaScript](../../examples/nmi/javascript/nmi.ts#L321) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L100) · [Rust](../../examples/nmi/rust/nmi.rs#L288)

#### capture

**Examples:** [Python](../../examples/nmi/python/nmi.py) · [JavaScript](../../examples/nmi/javascript/nmi.ts#L359) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L108) · [Rust](../../examples/nmi/rust/nmi.rs#L323)

#### get

**Examples:** [Python](../../examples/nmi/python/nmi.py) · [JavaScript](../../examples/nmi/javascript/nmi.ts#L378) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L116) · [Rust](../../examples/nmi/rust/nmi.rs#L340)

#### refund

**Examples:** [Python](../../examples/nmi/python/nmi.py) · [JavaScript](../../examples/nmi/javascript/nmi.ts#L393) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L124) · [Rust](../../examples/nmi/rust/nmi.rs#L357)

#### void

**Examples:** [Python](../../examples/nmi/python/nmi.py) · [JavaScript](../../examples/nmi/javascript/nmi.ts#L414) · [Kotlin](../../examples/nmi/kotlin/nmi.kt#L132) · [Rust](../../examples/nmi/rust/nmi.rs#L376)
