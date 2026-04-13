# Bankofamerica

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/bankofamerica.json
Regenerate: python3 scripts/generators/docs/generate.py bankofamerica
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
#     bankofamerica=payment_pb2.BankofamericaConfig(api_key=...),
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
    connector: 'Bankofamerica',
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
    .setConnector("Bankofamerica")
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
    connector: "Bankofamerica".to_string(),
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

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L93) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L84) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L109) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L104)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L118) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L110) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L131) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L127)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L137) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L129) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L147) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L143)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L174) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L164) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L169) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L166)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L196) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L186) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L188) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L185)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
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
        "card_number": "4111111111111111",  # Card Identification
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "737",
        "card_holder_name": "John Doe"  # Cardholder Information
    }
}
```

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L218) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L207) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L206) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L203)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L227) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L216) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L218) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L215)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L236) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L225) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L228) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L222)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L137) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L129) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L236) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L229)

#### PaymentService.SetupRecurring

Setup a recurring payment instruction for future payments/ debits. This could be for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L245) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L234) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L246) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L236)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py#L292) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js#L274) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt#L285) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs#L276)
