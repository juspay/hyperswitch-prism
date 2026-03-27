# Razorpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/razorpay.json
Regenerate: python3 scripts/generators/docs/generate.py razorpay
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
#     razorpay=payment_pb2.RazorpayConfig(api_key=...),
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
    connector: 'Razorpay',
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
    .setConnector("Razorpay")
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
    connector: "Razorpay".to_string(),
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

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L83) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L76) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L98) · [Rust](../../examples/razorpay/rust/razorpay.rs#L96)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L108) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L102) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L120) · [Rust](../../examples/razorpay/rust/razorpay.rs#L119)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L127) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L121) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L136) · [Rust](../../examples/razorpay/rust/razorpay.rs#L135)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L164) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L156) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L158) · [Rust](../../examples/razorpay/rust/razorpay.rs#L158)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
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
| UPI | ✓ |
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

##### UPI Collect

```python
"payment_method": {
    "upi_collect": {  # UPI Collect
        "vpa_id": {"value": "test@upi"}  # Virtual Payment Address
    }
}
```

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L186) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L177) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L176) · [Rust](../../examples/razorpay/rust/razorpay.rs#L176)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L195) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L186) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L188) · [Rust](../../examples/razorpay/rust/razorpay.rs#L188)

#### PaymentService.CreateOrder

Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L204) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L195) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L198) · [Rust](../../examples/razorpay/rust/razorpay.rs#L195)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L223) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L209) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L212) · [Rust](../../examples/razorpay/rust/razorpay.rs#L208)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/razorpay/python/razorpay.py#L127) · [JavaScript](../../examples/razorpay/javascript/razorpay.js#L121) · [Kotlin](../../examples/razorpay/kotlin/razorpay.kt#L220) · [Rust](../../examples/razorpay/rust/razorpay.rs#L215)
