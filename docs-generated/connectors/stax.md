# Stax

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/stax.json
Regenerate: python3 scripts/generators/docs/generate.py stax
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
#     stax=payment_pb2.StaxConfig(api_key=...),
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
    connector: 'Stax',
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
    .setConnector("Stax")
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
    connector: "Stax".to_string(),
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

### Card Payment (Authorize + Capture)

Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/stax/python/stax.py#L89) · [JavaScript](../../examples/stax/javascript/stax.js#L78) · [Kotlin](../../examples/stax/kotlin/stax.kt#L104) · [Rust](../../examples/stax/rust/stax.rs#L98)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/stax/python/stax.py#L114) · [JavaScript](../../examples/stax/javascript/stax.js#L104) · [Kotlin](../../examples/stax/kotlin/stax.kt#L126) · [Rust](../../examples/stax/rust/stax.rs#L121)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/stax/python/stax.py#L133) · [JavaScript](../../examples/stax/javascript/stax.js#L123) · [Kotlin](../../examples/stax/kotlin/stax.kt#L142) · [Rust](../../examples/stax/rust/stax.rs#L137)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/stax/python/stax.py#L174) · [JavaScript](../../examples/stax/javascript/stax.js#L161) · [Kotlin](../../examples/stax/kotlin/stax.kt#L177) · [Rust](../../examples/stax/rust/stax.rs#L174)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/stax/python/stax.py#L211) · [JavaScript](../../examples/stax/javascript/stax.js#L196) · [Kotlin](../../examples/stax/kotlin/stax.kt#L199) · [Rust](../../examples/stax/rust/stax.rs#L197)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/stax/python/stax.py#L233) · [JavaScript](../../examples/stax/javascript/stax.js#L218) · [Kotlin](../../examples/stax/kotlin/stax.kt#L218) · [Rust](../../examples/stax/rust/stax.rs#L216)

### Create Customer

Register a customer record in the connector system. Returns a connector_customer_id that can be reused for recurring payments and tokenized card storage.

**Examples:** [Python](../../examples/stax/python/stax.py#L255) · [JavaScript](../../examples/stax/javascript/stax.js#L240) · [Kotlin](../../examples/stax/kotlin/stax.kt#L237) · [Rust](../../examples/stax/rust/stax.rs#L235)

### Tokenize Payment Method

Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.

**Examples:** [Python](../../examples/stax/python/stax.py#L276) · [JavaScript](../../examples/stax/javascript/stax.js#L256) · [Kotlin](../../examples/stax/kotlin/stax.kt#L253) · [Rust](../../examples/stax/rust/stax.rs#L250)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

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

**Examples:** [Python](../../examples/stax/python/stax.py#L313) · [JavaScript](../../examples/stax/javascript/stax.js#L287) · [Kotlin](../../examples/stax/kotlin/stax.kt#L284) · [Rust](../../examples/stax/rust/stax.rs#L282)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py#L322) · [JavaScript](../../examples/stax/javascript/stax.js#L296) · [Kotlin](../../examples/stax/kotlin/stax.kt#L296) · [Rust](../../examples/stax/rust/stax.rs#L294)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py#L331) · [JavaScript](../../examples/stax/javascript/stax.js#L305) · [Kotlin](../../examples/stax/kotlin/stax.kt#L319) · [Rust](../../examples/stax/rust/stax.rs#L313)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py#L174) · [JavaScript](../../examples/stax/javascript/stax.js#L161) · [Kotlin](../../examples/stax/kotlin/stax.kt#L327) · [Rust](../../examples/stax/rust/stax.rs#L320)

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py#L276) · [JavaScript](../../examples/stax/javascript/stax.js#L256) · [Kotlin](../../examples/stax/kotlin/stax.kt#L337) · [Rust](../../examples/stax/rust/stax.rs#L327)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py#L340) · [JavaScript](../../examples/stax/javascript/stax.js#L314) · [Kotlin](../../examples/stax/kotlin/stax.kt#L366) · [Rust](../../examples/stax/rust/stax.rs#L357)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/stax/python/stax.py#L255) · [JavaScript](../../examples/stax/javascript/stax.js#L240) · [Kotlin](../../examples/stax/kotlin/stax.kt#L306) · [Rust](../../examples/stax/rust/stax.rs#L301)
