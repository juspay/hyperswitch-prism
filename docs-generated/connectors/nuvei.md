# Nuvei

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/nuvei.json
Regenerate: python3 scripts/generators/docs/generate.py nuvei
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
#     nuvei=payment_pb2.NuveiConfig(api_key=...),
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
    connector: 'Nuvei',
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
    .setConnector("Nuvei")
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
    connector: "Nuvei".to_string(),
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

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L109) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L99) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L124) · [Rust](../../examples/nuvei/rust/nuvei.rs#L119)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L134) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L125) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L146) · [Rust](../../examples/nuvei/rust/nuvei.rs#L142)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L153) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L144) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L162) · [Rust](../../examples/nuvei/rust/nuvei.rs#L158)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L190) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L179) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L184) · [Rust](../../examples/nuvei/rust/nuvei.rs#L181)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L212) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L201) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L203) · [Rust](../../examples/nuvei/rust/nuvei.rs#L200)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [ConnectorSessionService.CreateSessionToken](#connectorsessionservicecreatesessiontoken) | Authentication | `ConnectorSessionServiceCreateSessionTokenRequest` |
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
| Google Pay | x |
| Apple Pay | x |
| SEPA | x |
| BACS | x |
| ACH | x |
| BECS | x |
| iDEAL | x |
| PayPal | x |
| BLIK | x |
| Klarna | x |
| Afterpay | x |
| UPI | x |
| Affirm | x |
| Samsung Pay | x |

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

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L234) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L222) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L221) · [Rust](../../examples/nuvei/rust/nuvei.rs#L218)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L243) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L231) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L233) · [Rust](../../examples/nuvei/rust/nuvei.rs#L230)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L270) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L253) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L256) · [Rust](../../examples/nuvei/rust/nuvei.rs#L249)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L153) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L144) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L264) · [Rust](../../examples/nuvei/rust/nuvei.rs#L256)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L279) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L262) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L274) · [Rust](../../examples/nuvei/rust/nuvei.rs#L263)

### Authentication

#### ConnectorSessionService.CreateSessionToken

Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.

| | Message |
|---|---------|
| **Request** | `ConnectorSessionServiceCreateSessionTokenRequest` |
| **Response** | `ConnectorSessionServiceCreateSessionTokenResponse` |

**Examples:** [Python](../../examples/nuvei/python/nuvei.py#L252) · [JavaScript](../../examples/nuvei/javascript/nuvei.js#L240) · [Kotlin](../../examples/nuvei/kotlin/nuvei.kt#L243) · [Rust](../../examples/nuvei/rust/nuvei.rs#L237)
