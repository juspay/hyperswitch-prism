# Finix

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/finix.json
Regenerate: python3 scripts/generators/docs/generate.py finix
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
    // connectorConfig: set your finix credentials here,
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

**Examples:** [Python](../../examples/finix/python/finix.py#L32) · [JavaScript](../../examples/finix/javascript/finix.js#L60) · [Kotlin](../../examples/finix/kotlin/finix.kt#L31) · [Rust](../../examples/finix/rust/finix.rs#L17)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/finix/python/finix.py#L69) · [JavaScript](../../examples/finix/javascript/finix.js#L117) · [Kotlin](../../examples/finix/kotlin/finix.kt#L45) · [Rust](../../examples/finix/rust/finix.rs#L68)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/finix/python/finix.py#L97) · [JavaScript](../../examples/finix/javascript/finix.js#L160) · [Kotlin](../../examples/finix/kotlin/finix.kt#L55) · [Rust](../../examples/finix/rust/finix.rs#L106)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/finix/python/finix.py#L132) · [JavaScript](../../examples/finix/javascript/finix.js#L210) · [Kotlin](../../examples/finix/kotlin/finix.kt#L65) · [Rust](../../examples/finix/rust/finix.rs#L151)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/finix/python/finix.py#L157) · [JavaScript](../../examples/finix/javascript/finix.js#L250) · [Kotlin](../../examples/finix/kotlin/finix.kt#L75) · [Rust](../../examples/finix/rust/finix.rs#L186)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/finix/python/finix.py#L196) · [JavaScript](../../examples/finix/javascript/finix.js#L309) · [Kotlin](../../examples/finix/kotlin/finix.kt#L89) · [Rust](../../examples/finix/rust/finix.rs#L239)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/finix/python/finix.py#L229) · [JavaScript](../../examples/finix/javascript/finix.js#L358) · [Kotlin](../../examples/finix/kotlin/finix.kt#L103) · [Rust](../../examples/finix/rust/finix.rs#L286)

### Create Customer

Register a customer record in the connector system. Returns a connector_customer_id that can be reused for recurring payments and tokenized card storage.

**Examples:** [Python](../../examples/finix/python/finix.py#L266) · [JavaScript](../../examples/finix/javascript/finix.js#L411) · [Kotlin](../../examples/finix/kotlin/finix.kt#L117) · [Rust](../../examples/finix/rust/finix.rs#L337)

### Tokenize Payment Method

Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.

**Examples:** [Python](../../examples/finix/python/finix.py#L277) · [JavaScript](../../examples/finix/javascript/finix.js#L428) · [Kotlin](../../examples/finix/kotlin/finix.kt#L127) · [Rust](../../examples/finix/rust/finix.rs#L354)

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

**Examples:** [Python](../../examples/finix/python/finix.py) · [JavaScript](../../examples/finix/javascript/finix.ts#L562) · [Kotlin](../../examples/finix/kotlin/finix.kt#L177) · [Rust](../../examples/finix/rust/finix.rs#L493)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/finix/python/finix.py) · [JavaScript](../../examples/finix/javascript/finix.ts#L517) · [Kotlin](../../examples/finix/kotlin/finix.kt) · [Rust](../../examples/finix/rust/finix.rs#L442)

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |
| Google Pay | ✓ |
| Apple Pay | ✓ |
| SEPA | ✓ |
| BACS | ✓ |
| ACH | ✓ |
| BECS | ✓ |
| iDEAL | ✓ |
| PayPal | ✓ |
| BLIK | ✓ |
| Klarna | ✓ |
| Afterpay | ✓ |
| UPI | ✓ |
| Affirm | ✓ |
| Samsung Pay | ✓ |

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
    "klarna": {  # Klarna - Swedish BNPL service
    }
}
```

##### Afterpay / Clearpay

```python
"payment_method": {
    "afterpay_clearpay": {  # Afterpay/Clearpay - BNPL service
    }
}
```

##### UPI Collect

```python
"payment_method": {
    "upi_collect": {  # UPI Collect
        "vpa_id": {"value": "test@upi"}  # Virtual Payment Address
    }
}
```

##### Affirm

```python
"payment_method": {
    "affirm": {  # Affirm - US BNPL service
    }
}
```

##### Samsung Pay

```python
"payment_method": {
    "samsung_pay": {  # Samsung
        "payment_credential": {
            "method": "3DS",  # Method type
            "recurring_payment": False,  # Whether this is a recurring payment
            "card_brand": "VISA",
            "card_last_four_digits": {"value": "1234"},  # Last four digits of card
            "token_data": {
                "type": "S",  # 3DS type
                "version": "100",  # 3DS version
                "data": {"value": "probe_samsung_token_data"}  # Token data
            }
        }
    }
}
```

**Examples:** [Python](../../examples/finix/python/finix.py) · [JavaScript](../../examples/finix/javascript/finix.ts#L459) · [Kotlin](../../examples/finix/kotlin/finix.kt#L137) · [Rust](../../examples/finix/rust/finix.rs#L389)

#### capture

**Examples:** [Python](../../examples/finix/python/finix.py) · [JavaScript](../../examples/finix/javascript/finix.ts#L498) · [Kotlin](../../examples/finix/kotlin/finix.kt#L145) · [Rust](../../examples/finix/rust/finix.rs#L425)

#### get

**Examples:** [Python](../../examples/finix/python/finix.py) · [JavaScript](../../examples/finix/javascript/finix.ts#L526) · [Kotlin](../../examples/finix/kotlin/finix.kt#L161) · [Rust](../../examples/finix/rust/finix.rs#L457)

#### refund

**Examples:** [Python](../../examples/finix/python/finix.py) · [JavaScript](../../examples/finix/javascript/finix.ts#L541) · [Kotlin](../../examples/finix/kotlin/finix.kt#L169) · [Rust](../../examples/finix/rust/finix.rs#L474)

#### void

**Examples:** [Python](../../examples/finix/python/finix.py) · [JavaScript](../../examples/finix/javascript/finix.ts#L571) · [Kotlin](../../examples/finix/kotlin/finix.kt#L185) · [Rust](../../examples/finix/rust/finix.rs#L526)
