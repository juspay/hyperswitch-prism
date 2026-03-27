# PlacetoPay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/placetopay.json
Regenerate: python3 scripts/generators/docs/generate.py placetopay
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
#     placetopay=payment_pb2.PlacetopayConfig(api_key=...),
# ))

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { ConnectorClient } = require('hyperswitch-prism');

// Reuse this client for all flows
const client = new ConnectorClient({
    connector: 'Placetopay',
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
    .setConnector("Placetopay")
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
    connector: "Placetopay".to_string(),
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

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L101) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L92) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L114) · [Rust](../../examples/placetopay/rust/placetopay.rs#L112)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L126) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L118) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L136) · [Rust](../../examples/placetopay/rust/placetopay.rs#L135)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L145) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L137) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L152) · [Rust](../../examples/placetopay/rust/placetopay.rs#L151)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L182) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L172) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L174) · [Rust](../../examples/placetopay/rust/placetopay.rs#L174)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L204) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L194) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L193) · [Rust](../../examples/placetopay/rust/placetopay.rs#L193)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
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
        "card_number": {"value": "4111111111111111"},  # Card Identification
        "card_exp_month": {"value": "03"},
        "card_exp_year": {"value": "2030"},
        "card_cvc": {"value": "737"},
        "card_holder_name": {"value": "John Doe"}  # Cardholder Information
    }
}
```

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L226) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L215) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L211) · [Rust](../../examples/placetopay/rust/placetopay.rs#L211)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L235) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L224) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L223) · [Rust](../../examples/placetopay/rust/placetopay.rs#L223)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L244) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L233) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L233) · [Rust](../../examples/placetopay/rust/placetopay.rs#L230)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L145) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L137) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L241) · [Rust](../../examples/placetopay/rust/placetopay.rs#L237)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/placetopay/python/placetopay.py#L253) · [JavaScript](../../examples/placetopay/javascript/placetopay.js#L242) · [Kotlin](../../examples/placetopay/kotlin/placetopay.kt#L251) · [Rust](../../examples/placetopay/rust/placetopay.rs#L244)
