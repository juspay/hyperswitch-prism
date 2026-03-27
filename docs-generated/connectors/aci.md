# ACI

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/aci.json
Regenerate: python3 scripts/generators/docs/generate.py aci
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
#     aci=payment_pb2.AciConfig(api_key=...),
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
    connector: 'Aci',
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
    .setConnector("Aci")
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
    connector: "Aci".to_string(),
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

**Examples:** [Python](../../examples/aci/python/aci.py#L89) · [JavaScript](../../examples/aci/javascript/aci.js#L79) · [Kotlin](../../examples/aci/kotlin/aci.kt#L107) · [Rust](../../examples/aci/rust/aci.rs#L99)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/aci/python/aci.py#L114) · [JavaScript](../../examples/aci/javascript/aci.js#L105) · [Kotlin](../../examples/aci/kotlin/aci.kt#L129) · [Rust](../../examples/aci/rust/aci.rs#L122)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/aci/python/aci.py#L133) · [JavaScript](../../examples/aci/javascript/aci.js#L124) · [Kotlin](../../examples/aci/kotlin/aci.kt#L145) · [Rust](../../examples/aci/rust/aci.rs#L138)

### Recurring / Mandate Payments

Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `PENDING` | Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls |
| `FAILED` | Setup failed — customer must re-enter payment details |

**Examples:** [Python](../../examples/aci/python/aci.py#L170) · [JavaScript](../../examples/aci/javascript/aci.js#L159) · [Kotlin](../../examples/aci/kotlin/aci.kt#L167) · [Rust](../../examples/aci/rust/aci.rs#L161)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/aci/python/aci.py#L239) · [JavaScript](../../examples/aci/javascript/aci.js#L219) · [Kotlin](../../examples/aci/kotlin/aci.kt#L229) · [Rust](../../examples/aci/rust/aci.rs#L221)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/aci/python/aci.py#L261) · [JavaScript](../../examples/aci/javascript/aci.js#L241) · [Kotlin](../../examples/aci/kotlin/aci.kt#L248) · [Rust](../../examples/aci/rust/aci.rs#L240)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
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
| SEPA | ⚠ |
| BACS | ⚠ |
| ACH | ⚠ |
| BECS | ⚠ |
| iDEAL | ✓ |
| PayPal | ⚠ |
| BLIK | ⚠ |
| Klarna | ✓ |
| Afterpay | ✓ |
| UPI | ⚠ |
| Affirm | ✓ |
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

##### iDEAL

```python
"payment_method": {
    "ideal": {
        "bank_name": "Ing"  # The bank name for ideal
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

##### Affirm

```python
"payment_method": {
    "affirm": {  # Affirm - US BNPL service
    }
}
```

**Examples:** [Python](../../examples/aci/python/aci.py#L283) · [JavaScript](../../examples/aci/javascript/aci.js#L262) · [Kotlin](../../examples/aci/kotlin/aci.kt#L266) · [Rust](../../examples/aci/rust/aci.rs#L258)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/aci/python/aci.py#L292) · [JavaScript](../../examples/aci/javascript/aci.js#L271) · [Kotlin](../../examples/aci/kotlin/aci.kt#L278) · [Rust](../../examples/aci/rust/aci.rs#L270)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/aci/python/aci.py#L301) · [JavaScript](../../examples/aci/javascript/aci.js#L280) · [Kotlin](../../examples/aci/kotlin/aci.kt#L288) · [Rust](../../examples/aci/rust/aci.rs#L277)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/aci/python/aci.py#L133) · [JavaScript](../../examples/aci/javascript/aci.js#L124) · [Kotlin](../../examples/aci/kotlin/aci.kt#L325) · [Rust](../../examples/aci/rust/aci.rs#L310)

#### PaymentService.SetupRecurring

Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/aci/python/aci.py#L343) · [JavaScript](../../examples/aci/javascript/aci.js#L318) · [Kotlin](../../examples/aci/kotlin/aci.kt#L335) · [Rust](../../examples/aci/rust/aci.rs#L317)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/aci/python/aci.py#L390) · [JavaScript](../../examples/aci/javascript/aci.js#L358) · [Kotlin](../../examples/aci/kotlin/aci.kt#L374) · [Rust](../../examples/aci/rust/aci.rs#L357)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/aci/python/aci.py#L310) · [JavaScript](../../examples/aci/javascript/aci.js#L289) · [Kotlin](../../examples/aci/kotlin/aci.kt#L296) · [Rust](../../examples/aci/rust/aci.rs#L284)
