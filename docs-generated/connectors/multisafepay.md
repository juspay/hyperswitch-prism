# Multisafepay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/multisafepay.json
Regenerate: python3 scripts/generators/docs/generate.py multisafepay
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
        multisafepay: {
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
import payments.PaymentClient
import payments.ConnectorConfig

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = PaymentClient(config)
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
        config: Some(connector_specific_config::Config::Multisafepay(MultisafepayConfig {
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

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L5) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L27) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L6) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L18)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L11) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L73) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L10) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L27)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L17) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L126) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L14) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L36)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py#L25) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.js#L188) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt#L18) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L48)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | ✓ |
| Apple Pay | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| ACH | ⚠ |
| BECS | ⚠ |
| iDEAL | ✓ |
| PayPal | ✓ |
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

##### iDEAL

```python
"payment_method": {
    "ideal": {
    }
}
```

##### PayPal Redirect

```python
"payment_method": {
    "paypal_redirect": {  # PayPal
        "email": {"value": "test@example.com"}  # PayPal's email address
    }
}
```

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.ts#L242) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L60)

#### get

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.ts#L284) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L96)

#### refund

**Examples:** [Python](../../examples/multisafepay/python/multisafepay.py) · [JavaScript](../../examples/multisafepay/javascript/multisafepay.ts#L299) · [Kotlin](../../examples/multisafepay/kotlin/multisafepay.kt) · [Rust](../../examples/multisafepay/rust/multisafepay.rs#L113)
