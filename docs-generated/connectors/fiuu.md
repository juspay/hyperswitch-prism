# Fiuu

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiuu.json
Regenerate: python3 scripts/generators/docs/generate.py fiuu
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
        fiuu: {
        merchantId: { value: 'YOUR_MERCHANT_ID' },
        verifyKey: { value: 'YOUR_VERIFY_KEY' },
        secretKey: { value: 'YOUR_SECRET_KEY' },
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
        config: Some(connector_specific_config::Config::Fiuu(FiuuConfig {
                merchant_id: Some(Secret::new("YOUR_MERCHANT_ID".to_string())),
                verify_key: Some(Secret::new("YOUR_VERIFY_KEY".to_string())),
                secret_key: Some(Secret::new("YOUR_SECRET_KEY".to_string())),
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

**Examples:** [Python](../../examples/fiuu/python/fiuu.py#L5) · [JavaScript](../../examples/fiuu/javascript/fiuu.js#L61) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt#L6) · [Rust](../../examples/fiuu/rust/fiuu.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/fiuu/python/fiuu.py#L13) · [JavaScript](../../examples/fiuu/javascript/fiuu.js#L118) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt#L10) · [Rust](../../examples/fiuu/rust/fiuu.rs#L30)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/fiuu/python/fiuu.py#L19) · [JavaScript](../../examples/fiuu/javascript/fiuu.js#L161) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt#L14) · [Rust](../../examples/fiuu/rust/fiuu.rs#L39)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/fiuu/python/fiuu.py#L25) · [JavaScript](../../examples/fiuu/javascript/fiuu.js#L211) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt#L18) · [Rust](../../examples/fiuu/rust/fiuu.rs#L48)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/fiuu/python/fiuu.py#L33) · [JavaScript](../../examples/fiuu/javascript/fiuu.js#L271) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt#L22) · [Rust](../../examples/fiuu/rust/fiuu.rs#L60)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/fiuu/python/fiuu.py#L41) · [JavaScript](../../examples/fiuu/javascript/fiuu.js#L320) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt#L26) · [Rust](../../examples/fiuu/rust/fiuu.rs#L72)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [refund](#refund) | Other | `—` |
| [void](#void) | Other | `—` |

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/fiuu/python/fiuu.py) · [JavaScript](../../examples/fiuu/javascript/fiuu.ts#L444) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt) · [Rust](../../examples/fiuu/rust/fiuu.rs#L151)

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

**Examples:** [Python](../../examples/fiuu/python/fiuu.py) · [JavaScript](../../examples/fiuu/javascript/fiuu.ts#L371) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt) · [Rust](../../examples/fiuu/rust/fiuu.rs#L84)

#### capture

**Examples:** [Python](../../examples/fiuu/python/fiuu.py) · [JavaScript](../../examples/fiuu/javascript/fiuu.ts#L410) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt) · [Rust](../../examples/fiuu/rust/fiuu.rs#L117)

#### get

**Examples:** [Python](../../examples/fiuu/python/fiuu.py) · [JavaScript](../../examples/fiuu/javascript/fiuu.ts#L429) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt) · [Rust](../../examples/fiuu/rust/fiuu.rs#L134)

#### refund

**Examples:** [Python](../../examples/fiuu/python/fiuu.py) · [JavaScript](../../examples/fiuu/javascript/fiuu.ts#L453) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt) · [Rust](../../examples/fiuu/rust/fiuu.rs#L190)

#### void

**Examples:** [Python](../../examples/fiuu/python/fiuu.py) · [JavaScript](../../examples/fiuu/javascript/fiuu.ts#L475) · [Kotlin](../../examples/fiuu/kotlin/fiuu.kt) · [Rust](../../examples/fiuu/rust/fiuu.rs#L210)
