# Checkout.com

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/checkout.json
Regenerate: python3 scripts/generators/docs/generate.py checkout
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
#     checkout=payment_pb2.CheckoutConfig(api_key=...),
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
    connector: 'Checkout',
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
    .setConnector("Checkout")
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
    connector: "Checkout".to_string(),
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

**Examples:** [Python](../../examples/checkout/python/checkout.py#L88) · [JavaScript](../../examples/checkout/javascript/checkout.js#L78) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L106) · [Rust](../../examples/checkout/rust/checkout.rs#L98)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L113) · [JavaScript](../../examples/checkout/javascript/checkout.js#L104) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L128) · [Rust](../../examples/checkout/rust/checkout.rs#L121)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L132) · [JavaScript](../../examples/checkout/javascript/checkout.js#L123) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L144) · [Rust](../../examples/checkout/rust/checkout.rs#L137)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/checkout/python/checkout.py#L174) · [JavaScript](../../examples/checkout/javascript/checkout.js#L162) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L180) · [Rust](../../examples/checkout/rust/checkout.rs#L175)

### Recurring / Mandate Payments

Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `PENDING` | Mandate stored — save connector_transaction_id for future RecurringPaymentService.Charge calls |
| `FAILED` | Setup failed — customer must re-enter payment details |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L211) · [JavaScript](../../examples/checkout/javascript/checkout.js#L197) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L202) · [Rust](../../examples/checkout/rust/checkout.rs#L198)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/checkout/python/checkout.py#L280) · [JavaScript](../../examples/checkout/javascript/checkout.js#L257) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L264) · [Rust](../../examples/checkout/rust/checkout.rs#L258)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/checkout/python/checkout.py#L302) · [JavaScript](../../examples/checkout/javascript/checkout.js#L279) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L283) · [Rust](../../examples/checkout/rust/checkout.rs#L277)

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

**Examples:** [Python](../../examples/checkout/python/checkout.py#L324) · [JavaScript](../../examples/checkout/javascript/checkout.js#L300) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L301) · [Rust](../../examples/checkout/rust/checkout.rs#L295)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L333) · [JavaScript](../../examples/checkout/javascript/checkout.js#L309) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L313) · [Rust](../../examples/checkout/rust/checkout.rs#L307)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L342) · [JavaScript](../../examples/checkout/javascript/checkout.js#L318) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L323) · [Rust](../../examples/checkout/rust/checkout.rs#L314)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L174) · [JavaScript](../../examples/checkout/javascript/checkout.js#L162) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L360) · [Rust](../../examples/checkout/rust/checkout.rs#L347)

#### PaymentService.SetupRecurring

Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L384) · [JavaScript](../../examples/checkout/javascript/checkout.js#L356) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L370) · [Rust](../../examples/checkout/rust/checkout.rs#L354)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L431) · [JavaScript](../../examples/checkout/javascript/checkout.js#L396) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L409) · [Rust](../../examples/checkout/rust/checkout.rs#L394)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/checkout/python/checkout.py#L351) · [JavaScript](../../examples/checkout/javascript/checkout.js#L327) · [Kotlin](../../examples/checkout/kotlin/checkout.kt#L331) · [Rust](../../examples/checkout/rust/checkout.rs#L321)
