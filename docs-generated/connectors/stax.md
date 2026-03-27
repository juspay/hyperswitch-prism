# Stax

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/stax.json
Regenerate: python3 scripts/generators/docs/generate.py stax
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
        stax: {
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
        config: Some(connector_specific_config::Config::Stax(StaxConfig {
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

**Examples:** [Python](../../examples/stax/python/stax.py#L7) · [JavaScript](../../examples/stax/javascript/stax.js#L62) · [Kotlin](../../examples/stax/kotlin/stax.kt#L6) · [Rust](../../examples/stax/rust/stax.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/stax/python/stax.py#L15) · [JavaScript](../../examples/stax/javascript/stax.js#L119) · [Kotlin](../../examples/stax/kotlin/stax.kt#L10) · [Rust](../../examples/stax/rust/stax.rs#L30)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/stax/python/stax.py#L21) · [JavaScript](../../examples/stax/javascript/stax.js#L162) · [Kotlin](../../examples/stax/kotlin/stax.kt#L14) · [Rust](../../examples/stax/rust/stax.rs#L39)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/stax/python/stax.py#L27) · [JavaScript](../../examples/stax/javascript/stax.js#L202) · [Kotlin](../../examples/stax/kotlin/stax.kt#L18) · [Rust](../../examples/stax/rust/stax.rs#L48)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/stax/python/stax.py#L35) · [JavaScript](../../examples/stax/javascript/stax.js#L261) · [Kotlin](../../examples/stax/kotlin/stax.kt#L22) · [Rust](../../examples/stax/rust/stax.rs#L60)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/stax/python/stax.py#L43) · [JavaScript](../../examples/stax/javascript/stax.js#L310) · [Kotlin](../../examples/stax/kotlin/stax.kt#L26) · [Rust](../../examples/stax/rust/stax.rs#L72)

### Create Customer

Register a customer record in the connector system. Returns a connector_customer_id that can be reused for recurring payments and tokenized card storage.

**Examples:** [Python](../../examples/stax/python/stax.py#L51) · [JavaScript](../../examples/stax/javascript/stax.js#L363) · [Kotlin](../../examples/stax/kotlin/stax.kt#L30) · [Rust](../../examples/stax/rust/stax.rs#L84)

### Tokenize Payment Method

Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.

**Examples:** [Python](../../examples/stax/python/stax.py#L57) · [JavaScript](../../examples/stax/javascript/stax.js#L380) · [Kotlin](../../examples/stax/kotlin/stax.kt#L34) · [Rust](../../examples/stax/rust/stax.rs#L101)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [void](#void) | Other | `—` |

### Payments

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py) · [JavaScript](../../examples/stax/javascript/stax.ts#L514) · [Kotlin](../../examples/stax/kotlin/stax.kt) · [Rust](../../examples/stax/rust/stax.rs#L237)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py) · [JavaScript](../../examples/stax/javascript/stax.ts#L469) · [Kotlin](../../examples/stax/kotlin/stax.kt) · [Rust](../../examples/stax/rust/stax.rs#L186)

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | ⚠ |
| Apple Pay | ⚠ |
| SEPA | ✓ |
| BACS | ✓ |
| ACH | ✓ |
| BECS | ✓ |
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

##### SEPA Direct Debit

```python
"payment_method": {
    "sepa": {  # Sepa - Single Euro Payments Area direct debit
        "iban": {"value": "DE89370400440532013000"},  # International bank account number (iban) for SEPA
        "bank_account_holder_name": {"value": "John Doe"}  # Owner name for bank debit
    }
}
```

##### BACS Direct Debit

```python
"payment_method": {
    "bacs": {  # Bacs - Bankers' Automated Clearing Services
        "account_number": {"value": "55779911"},  # Account number for Bacs payment method
        "sort_code": {"value": "200000"},  # Sort code for Bacs payment method
        "bank_account_holder_name": {"value": "John Doe"}  # Holder name for bank debit
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

##### BECS Direct Debit

```python
"payment_method": {
    "becs": {  # Becs - Bulk Electronic Clearing System - Australian direct debit
        "account_number": {"value": "000123456"},  # Account number for Becs payment method
        "bsb_number": {"value": "000000"},  # Bank-State-Branch (bsb) number
        "bank_account_holder_name": {"value": "John Doe"}  # Owner name for bank debit
    }
}
```

**Examples:** [Python](../../examples/stax/python/stax.py) · [JavaScript](../../examples/stax/javascript/stax.ts#L411) · [Kotlin](../../examples/stax/kotlin/stax.kt) · [Rust](../../examples/stax/rust/stax.rs#L136)

#### capture

**Examples:** [Python](../../examples/stax/python/stax.py) · [JavaScript](../../examples/stax/javascript/stax.ts#L450) · [Kotlin](../../examples/stax/kotlin/stax.kt) · [Rust](../../examples/stax/rust/stax.rs#L169)

#### get

**Examples:** [Python](../../examples/stax/python/stax.py) · [JavaScript](../../examples/stax/javascript/stax.ts#L478) · [Kotlin](../../examples/stax/kotlin/stax.kt) · [Rust](../../examples/stax/rust/stax.rs#L201)

#### refund

**Examples:** [Python](../../examples/stax/python/stax.py) · [JavaScript](../../examples/stax/javascript/stax.ts#L493) · [Kotlin](../../examples/stax/kotlin/stax.kt) · [Rust](../../examples/stax/rust/stax.rs#L218)

#### void

**Examples:** [Python](../../examples/stax/python/stax.py) · [JavaScript](../../examples/stax/javascript/stax.ts#L523) · [Kotlin](../../examples/stax/kotlin/stax.kt) · [Rust](../../examples/stax/rust/stax.rs#L270)
