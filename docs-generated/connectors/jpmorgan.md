# Jpmorgan

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/jpmorgan.json
Regenerate: python3 scripts/generators/docs/generate.py jpmorgan
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
#     jpmorgan=payment_pb2.JpmorganConfig(api_key=...),
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
    connector: 'Jpmorgan',
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
    .setConnector("Jpmorgan")
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
    connector: "Jpmorgan".to_string(),
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

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L116) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L106) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L137) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L133)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L141) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L132) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L159) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L156)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L160) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L151) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L175) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L172)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L209) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L197) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L218) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L217)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L253) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L239) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L240) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L240)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L275) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L261) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L259) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L259)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
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

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L297) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L282) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L277) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L277)

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L306) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L291) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L289) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L289)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L330) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L310) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L309) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L305)

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L209) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L197) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L317) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L312)

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L339) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L319) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L327) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L319)

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L315) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L300) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L299) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L296)
