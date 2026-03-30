# Hipay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/hipay.json
Regenerate: python3 scripts/generators/docs/generate.py hipay
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
    connectorConfig: types.ConnectorSpecificConfig.create({
        hipay: {
        apiKey: { value: 'YOUR_API_KEY' },
        apiSecret: { value: 'YOUR_API_SECRET' },
        },
    }),
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
use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;

let config = ConnectorConfig {
    connector_config: Some(ConnectorSpecificConfig {
        config: Some(connector_specific_config::Config::Hipay(HipayConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
                api_secret: Some(Secret::new("YOUR_API_SECRET".to_string())),
            ..Default::default()
        })),
    }),
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

**Examples:** [Python](../../examples/hipay/python/hipay.py#L28) · [JavaScript](../../examples/hipay/javascript/hipay.js#L68) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L27) · [Rust](../../examples/hipay/rust/hipay.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/hipay/python/hipay.py#L64) · [JavaScript](../../examples/hipay/javascript/hipay.js#L124) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L41) · [Rust](../../examples/hipay/rust/hipay.rs#L68)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/hipay/python/hipay.py#L91) · [JavaScript](../../examples/hipay/javascript/hipay.js#L166) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L51) · [Rust](../../examples/hipay/rust/hipay.rs#L105)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/hipay/python/hipay.py#L129) · [JavaScript](../../examples/hipay/javascript/hipay.js#L224) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L65) · [Rust](../../examples/hipay/rust/hipay.rs#L157)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/hipay/python/hipay.py#L161) · [JavaScript](../../examples/hipay/javascript/hipay.js#L272) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L79) · [Rust](../../examples/hipay/rust/hipay.rs#L203)

### Tokenize Payment Method

Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.

**Examples:** [Python](../../examples/hipay/python/hipay.py#L197) · [JavaScript](../../examples/hipay/javascript/hipay.js#L324) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L93) · [Rust](../../examples/hipay/rust/hipay.rs#L253)

### Tokenized Payment (Authorize + Capture)

Authorize using a connector-issued payment method token (e.g. Stripe pm_xxx). Card data never touches your server — only the token is sent. Capture settles the reserved funds.

**Examples:** [Python](../../examples/hipay/python/hipay.py#L220) · [JavaScript](../../examples/hipay/javascript/hipay.js#L354) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L103) · [Rust](../../examples/hipay/rust/hipay.rs#L285)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [TokenizedPaymentService.Authorize](#tokenizedpaymentserviceauthorize) | Non-PCI Payments | `TokenizedPaymentServiceAuthorizeRequest` |
| [void](#void) | Other | `—` |

### Payments

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.ts#L485) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L150) · [Rust](../../examples/hipay/rust/hipay.rs#L411)

### Other

#### authorize

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

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.ts#L392) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L118) · [Rust](../../examples/hipay/rust/hipay.rs#L323)

#### capture

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.ts#L430) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L126) · [Rust](../../examples/hipay/rust/hipay.rs#L358)

#### get

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.ts#L449) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L134) · [Rust](../../examples/hipay/rust/hipay.rs#L375)

#### refund

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.ts#L464) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L142) · [Rust](../../examples/hipay/rust/hipay.rs#L392)

#### void

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.ts#L503) · [Kotlin](../../examples/hipay/kotlin/hipay.kt#L166) · [Rust](../../examples/hipay/rust/hipay.rs#L464)
