# CyberSource

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/cybersource.json
Regenerate: python3 scripts/generators/docs/generate.py cybersource
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
        cybersource: {
        apiKey: { value: 'YOUR_API_KEY' },
        merchantAccount: { value: 'YOUR_MERCHANT_ACCOUNT' },
        apiSecret: { value: 'YOUR_API_SECRET' },
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
        config: Some(connector_specific_config::Config::Cybersource(CybersourceConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
                merchant_account: Some(Secret::new("YOUR_MERCHANT_ACCOUNT".to_string())),
                api_secret: Some(Secret::new("YOUR_API_SECRET".to_string())),
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

**Examples:** [Python](../../examples/cybersource/python/cybersource.py#L37) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L79) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L38) · [Rust](../../examples/cybersource/rust/cybersource.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/cybersource/python/cybersource.py#L76) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L138) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L52) · [Rust](../../examples/cybersource/rust/cybersource.rs#L71)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/cybersource/python/cybersource.py#L106) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L183) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L62) · [Rust](../../examples/cybersource/rust/cybersource.rs#L111)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/cybersource/python/cybersource.py#L143) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L235) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L72) · [Rust](../../examples/cybersource/rust/cybersource.rs#L158)

### Recurring / Mandate Payments

Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `PENDING` | Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls |
| `FAILED` | Setup failed — customer must re-enter payment details |

**Examples:** [Python](../../examples/cybersource/python/cybersource.py#L184) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L296) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L86) · [Rust](../../examples/cybersource/rust/cybersource.rs#L213)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/cybersource/python/cybersource.py#L243) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L361) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L101) · [Rust](../../examples/cybersource/rust/cybersource.rs#L288)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/cybersource/python/cybersource.py#L283) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L417) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L115) · [Rust](../../examples/cybersource/rust/cybersource.rs#L342)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
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

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L554) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt) · [Rust](../../examples/cybersource/rust/cybersource.rs#L499)

### Authentication

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L545) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt) · [Rust](../../examples/cybersource/rust/cybersource.rs#L467)

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
| ACH | ⚠ |
| BECS | ⚠ |
| iDEAL | ⚠ |
| PayPal | ⚠ |
| BLIK | ⚠ |
| Klarna | ⚠ |
| Afterpay | ⚠ |
| UPI | ⚠ |
| Affirm | ⚠ |
| Samsung Pay | ? |

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

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L470) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L129) · [Rust](../../examples/cybersource/rust/cybersource.rs#L395)

#### capture

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L511) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L137) · [Rust](../../examples/cybersource/rust/cybersource.rs#L433)

#### get

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L530) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L145) · [Rust](../../examples/cybersource/rust/cybersource.rs#L450)

#### refund

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L563) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L169) · [Rust](../../examples/cybersource/rust/cybersource.rs#L532)

#### setup_recurring

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L584) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt) · [Rust](../../examples/cybersource/rust/cybersource.rs#L551)

#### void

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.ts#L628) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt#L185) · [Rust](../../examples/cybersource/rust/cybersource.rs#L595)
