# Forte

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/forte.json
Regenerate: python3 scripts/generators/docs/generate.py forte
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
        forte: {
        apiAccessId: { value: 'YOUR_API_ACCESS_ID' },
        organizationId: { value: 'YOUR_ORGANIZATION_ID' },
        locationId: { value: 'YOUR_LOCATION_ID' },
        apiSecretKey: { value: 'YOUR_API_SECRET_KEY' },
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
        config: Some(connector_specific_config::Config::Forte(ForteConfig {
                api_access_id: Some(Secret::new("YOUR_API_ACCESS_ID".to_string())),
                organization_id: Some(Secret::new("YOUR_ORGANIZATION_ID".to_string())),
                location_id: Some(Secret::new("YOUR_LOCATION_ID".to_string())),
                api_secret_key: Some(Secret::new("YOUR_API_SECRET_KEY".to_string())),
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

**Examples:** [Python](../../examples/forte/python/forte.py#L23) · [JavaScript](../../examples/forte/javascript/forte.js#L30) · [Kotlin](../../examples/forte/kotlin/forte.kt#L22) · [Rust](../../examples/forte/rust/forte.rs#L18)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/forte/python/forte.py#L52) · [JavaScript](../../examples/forte/javascript/forte.js#L73) · [Kotlin](../../examples/forte/kotlin/forte.kt#L32) · [Rust](../../examples/forte/rust/forte.rs#L56)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/forte/python/forte.py#L79) · [JavaScript](../../examples/forte/javascript/forte.js#L114) · [Kotlin](../../examples/forte/kotlin/forte.kt#L42) · [Rust](../../examples/forte/rust/forte.rs#L92)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/forte/python/forte.py#L113) · [JavaScript](../../examples/forte/javascript/forte.js#L163) · [Kotlin](../../examples/forte/kotlin/forte.kt#L56) · [Rust](../../examples/forte/rust/forte.rs#L139)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [get](#get) | Other | `—` |
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

**Examples:** [Python](../../examples/forte/python/forte.py) · [JavaScript](../../examples/forte/javascript/forte.ts#L214) · [Kotlin](../../examples/forte/kotlin/forte.kt#L70) · [Rust](../../examples/forte/rust/forte.rs#L190)

#### get

**Examples:** [Python](../../examples/forte/python/forte.py) · [JavaScript](../../examples/forte/javascript/forte.ts#L253) · [Kotlin](../../examples/forte/kotlin/forte.kt#L78) · [Rust](../../examples/forte/rust/forte.rs#L226)

#### void

**Examples:** [Python](../../examples/forte/python/forte.py) · [JavaScript](../../examples/forte/javascript/forte.ts#L268) · [Kotlin](../../examples/forte/kotlin/forte.kt#L86) · [Rust](../../examples/forte/rust/forte.rs#L243)
