# Razorpay V2

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/razorpayv2.json
Regenerate: python3 scripts/generators/docs/generate.py razorpayv2
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     razorpayv2=payment_pb2.Razorpayv2Config(api_key=...),
# ))

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { ConnectorClient } = require('connector-service-node-ffi');

// Reuse this client for all flows
const client = new ConnectorClient({
    connector: 'Razorpayv2',
    environment: 'sandbox',
    connector_auth_type: {
        header_key: { api_key: 'YOUR_API_KEY' },
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setConnector("Razorpayv2")
    .setEnvironment(Environment.SANDBOX)
    .setAuth(
        ConnectorAuthType.newBuilder()
            .setHeaderKey(HeaderKey.newBuilder().setApiKey("YOUR_API_KEY"))
    )
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use connector_service_sdk::{ConnectorClient, ConnectorConfig};

let config = ConnectorConfig {
    connector: "Razorpayv2".to_string(),
    environment: Environment::Sandbox,
    auth: ConnectorAuth::HeaderKey { api_key: "YOUR_API_KEY".into() },
    ..Default::default()
};
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

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L66) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L61) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L82) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L81)

### Wallet Payment (Google Pay / Apple Pay)

Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L85) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L80) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L98) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L97)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L137) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L129) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L144) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L145)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L179) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L168) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L180) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L183)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L216) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L203) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L202) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L206)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |

### Payments

#### PaymentService.Authorize

Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.

| | Message |
|---|---------|
| **Request** | `PaymentServiceAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

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
        "card_number": "4111111111111111",  # Card Identification
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "737",
        "card_holder_name": "John Doe"  # Cardholder Information
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
        "iban": "DE89370400440532013000",  # International bank account number (iban) for SEPA
        "bank_account_holder_name": "John Doe"  # Owner name for bank debit
    }
}
```

##### BACS Direct Debit

```python
"payment_method": {
    "bacs": {  # Bacs - Bankers' Automated Clearing Services
        "account_number": "55779911",  # Account number for Bacs payment method
        "sort_code": "200000",  # Sort code for Bacs payment method
        "bank_account_holder_name": "John Doe"  # Holder name for bank debit
    }
}
```

##### ACH Direct Debit

```python
"payment_method": {
    "ach": {  # Ach - Automated Clearing House
        "account_number": "000123456789",  # Account number for ach bank debit payment
        "routing_number": "110000000",  # Routing number for ach bank debit payment
        "bank_account_holder_name": "John Doe"  # Bank account holder name
    }
}
```

##### BECS Direct Debit

```python
"payment_method": {
    "becs": {  # Becs - Bulk Electronic Clearing System - Australian direct debit
        "account_number": "000123456",  # Account number for Becs payment method
        "bsb_number": "000000",  # Bank-State-Branch (bsb) number
        "bank_account_holder_name": "John Doe"  # Owner name for bank debit
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
        "email": "test@example.com"  # PayPal's email address
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
        "vpa_id": "test@upi"  # Virtual Payment Address
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
            "card_last_four_digits": "1234",  # Last four digits of card
            "token_data": {
                "type": "S",  # 3DS type
                "version": "100",  # 3DS version
                "data": "probe_samsung_token_data"  # Token data
            }
        }
    }
}
```

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L238) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L224) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L220) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L224)

#### PaymentService.CreateOrder

Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L247) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L233) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L232) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L236)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L266) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L247) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L246) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L249)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py#L179) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js#L168) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt#L254) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs#L256)
