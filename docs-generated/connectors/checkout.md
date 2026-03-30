# Checkout.com

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/checkout.json
Regenerate: python3 scripts/generators/docs/generate.py checkout
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
    // connectorConfig: set your checkout credentials here,
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
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;

let config = ConnectorConfig {
    connector_config: None,  // TODO: set connector credentials
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

**Examples:** [Python](../../examples/checkout/python/checkout.py#L35) · [JavaScript](../../examples/checkout/javascript/checkout.js#L51) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L34) · [Rust](../../examples/checkout/rust/checkout.rs#L17)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L71) · [JavaScript](../../examples/checkout/javascript/checkout.js#L107) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L48) · [Rust](../../examples/checkout/rust/checkout.rs#L67)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L98) · [JavaScript](../../examples/checkout/javascript/checkout.js#L149) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L58) · [Rust](../../examples/checkout/rust/checkout.rs#L104)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/checkout/python/checkout.py#L123) · [JavaScript](../../examples/checkout/javascript/checkout.js#L189) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L68) · [Rust](../../examples/checkout/rust/checkout.rs#L139)

### Recurring / Mandate Payments

Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `PENDING` | Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls |
| `FAILED` | Setup failed — customer must re-enter payment details |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L161) · [JavaScript](../../examples/checkout/javascript/checkout.js#L247) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L82) · [Rust](../../examples/checkout/rust/checkout.rs#L191)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/checkout/python/checkout.py#L216) · [JavaScript](../../examples/checkout/javascript/checkout.js#L308) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L97) · [Rust](../../examples/checkout/rust/checkout.rs#L262)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/checkout/python/checkout.py#L248) · [JavaScript](../../examples/checkout/javascript/checkout.js#L356) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L111) · [Rust](../../examples/checkout/rust/checkout.rs#L308)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [refund](#refund) | Other | `—` |
| [setup_recurring](#setup_recurring) | Other | `—` |
| [void](#void) | Other | `—` |

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/checkout/python/checkout.py) · [JavaScript](../../examples/checkout/javascript/checkout.ts#L478) · [Kotlin](../../examples/checkout/kotlin/checkout.kt) · [Rust](../../examples/checkout/rust/checkout.rs#L427)

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | ? |
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

**Examples:** [Python](../../examples/checkout/python/checkout.py) · [JavaScript](../../examples/checkout/javascript/checkout.ts#L406) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L125) · [Rust](../../examples/checkout/rust/checkout.rs#L358)

#### capture

**Examples:** [Python](../../examples/checkout/python/checkout.py) · [JavaScript](../../examples/checkout/javascript/checkout.ts#L444) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L133) · [Rust](../../examples/checkout/rust/checkout.rs#L393)

#### get

**Examples:** [Python](../../examples/checkout/python/checkout.py) · [JavaScript](../../examples/checkout/javascript/checkout.ts#L463) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L141) · [Rust](../../examples/checkout/rust/checkout.rs#L410)

#### refund

**Examples:** [Python](../../examples/checkout/python/checkout.py) · [JavaScript](../../examples/checkout/javascript/checkout.ts#L487) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L157) · [Rust](../../examples/checkout/rust/checkout.rs#L460)

#### setup_recurring

**Examples:** [Python](../../examples/checkout/python/checkout.py) · [JavaScript](../../examples/checkout/javascript/checkout.ts#L508) · [Kotlin](../../examples/checkout/kotlin/checkout.kt) · [Rust](../../examples/checkout/rust/checkout.rs#L479)

#### void

**Examples:** [Python](../../examples/checkout/python/checkout.py) · [JavaScript](../../examples/checkout/javascript/checkout.ts#L548) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L173) · [Rust](../../examples/checkout/rust/checkout.rs#L519)
