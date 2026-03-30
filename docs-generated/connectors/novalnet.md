# Novalnet

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/novalnet.json
Regenerate: python3 scripts/generators/docs/generate.py novalnet
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
        novalnet: {
        productActivationKey: { value: 'YOUR_PRODUCT_ACTIVATION_KEY' },
        paymentAccessKey: { value: 'YOUR_PAYMENT_ACCESS_KEY' },
        tariffId: { value: 'YOUR_TARIFF_ID' },
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
        config: Some(connector_specific_config::Config::Novalnet(NovalnetConfig {
                product_activation_key: Some(Secret::new("YOUR_PRODUCT_ACTIVATION_KEY".to_string())),
                payment_access_key: Some(Secret::new("YOUR_PAYMENT_ACCESS_KEY".to_string())),
                tariff_id: Some(Secret::new("YOUR_TARIFF_ID".to_string())),
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

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L38) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L57) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L37) · [Rust](../../examples/novalnet/rust/novalnet.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L80) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L118) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L51) · [Rust](../../examples/novalnet/rust/novalnet.rs#L73)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L113) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L165) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L61) · [Rust](../../examples/novalnet/rust/novalnet.rs#L115)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L151) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L218) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L71) · [Rust](../../examples/novalnet/rust/novalnet.rs#L163)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L179) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L261) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L81) · [Rust](../../examples/novalnet/rust/novalnet.rs#L201)

### Recurring / Mandate Payments

Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `PENDING` | Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls |
| `FAILED` | Setup failed — customer must re-enter payment details |

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L223) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L324) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L95) · [Rust](../../examples/novalnet/rust/novalnet.rs#L258)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L286) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L392) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L110) · [Rust](../../examples/novalnet/rust/novalnet.rs#L336)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/novalnet/python/novalnet.py#L324) · [JavaScript](../../examples/novalnet/javascript/novalnet.js#L445) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L124) · [Rust](../../examples/novalnet/rust/novalnet.rs#L387)

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

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.ts#L577) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt) · [Rust](../../examples/novalnet/rust/novalnet.rs#L516)

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | ✓ |
| Apple Pay | ✓ |
| SEPA | ✓ |
| BACS | ⚠ |
| ACH | ✓ |
| BECS | ⚠ |
| iDEAL | ⚠ |
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

##### SEPA Direct Debit

```python
"payment_method": {
    "sepa": {  # Sepa - Single Euro Payments Area direct debit
        "iban": {"value": "DE89370400440532013000"},  # International bank account number (iban) for SEPA
        "bank_account_holder_name": {"value": "John Doe"}  # Owner name for bank debit
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

##### PayPal Redirect

```python
"payment_method": {
    "paypal_redirect": {  # PayPal
        "email": {"value": "test@example.com"}  # PayPal's email address
    }
}
```

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.ts#L500) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L138) · [Rust](../../examples/novalnet/rust/novalnet.rs#L442)

#### capture

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.ts#L543) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L146) · [Rust](../../examples/novalnet/rust/novalnet.rs#L482)

#### get

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.ts#L562) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L154) · [Rust](../../examples/novalnet/rust/novalnet.rs#L499)

#### refund

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.ts#L586) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L170) · [Rust](../../examples/novalnet/rust/novalnet.rs#L551)

#### setup_recurring

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.ts#L607) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt) · [Rust](../../examples/novalnet/rust/novalnet.rs#L570)

#### void

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.ts#L652) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt#L186) · [Rust](../../examples/novalnet/rust/novalnet.rs#L615)
