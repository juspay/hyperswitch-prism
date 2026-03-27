# Braintree

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/braintree.json
Regenerate: python3 scripts/generators/docs/generate.py braintree
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
        braintree: {
        publicKey: { value: 'YOUR_PUBLIC_KEY' },
        privateKey: { value: 'YOUR_PRIVATE_KEY' },
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
import payments.PaymentClient
import payments.ConnectorConfig

val config = ConnectorConfig.newBuilder()
    .setEnvironment(Environment.SANDBOX)
    .build()
val client = PaymentClient(config)
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
        config: Some(connector_specific_config::Config::Braintree(BraintreeConfig {
                public_key: Some(Secret::new("YOUR_PUBLIC_KEY".to_string())),
                private_key: Some(Secret::new("YOUR_PRIVATE_KEY".to_string())),
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

**Examples:** [Python](../../examples/braintree/python/braintree.py#L6) · [JavaScript](../../examples/braintree/javascript/braintree.js#L51) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L6) · [Rust](../../examples/braintree/rust/braintree.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/braintree/python/braintree.py#L14) · [JavaScript](../../examples/braintree/javascript/braintree.js#L108) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L10) · [Rust](../../examples/braintree/rust/braintree.rs#L30)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/braintree/python/braintree.py#L20) · [JavaScript](../../examples/braintree/javascript/braintree.js#L151) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L14) · [Rust](../../examples/braintree/rust/braintree.rs#L39)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/braintree/python/braintree.py#L28) · [JavaScript](../../examples/braintree/javascript/braintree.js#L200) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L18) · [Rust](../../examples/braintree/rust/braintree.rs#L51)

### Tokenize Payment Method

Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.

**Examples:** [Python](../../examples/braintree/python/braintree.py#L36) · [JavaScript](../../examples/braintree/javascript/braintree.js#L253) · [Kotlin](../../examples/braintree/kotlin/braintree.kt#L22) · [Rust](../../examples/braintree/rust/braintree.rs#L63)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [void](#void) | Other | `—` |

### Payments

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/braintree/python/braintree.py) · [JavaScript](../../examples/braintree/javascript/braintree.ts#L354) · [Kotlin](../../examples/braintree/kotlin/braintree.kt) · [Rust](../../examples/braintree/rust/braintree.rs#L162)

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

**Examples:** [Python](../../examples/braintree/python/braintree.py) · [JavaScript](../../examples/braintree/javascript/braintree.ts#L281) · [Kotlin](../../examples/braintree/kotlin/braintree.kt) · [Rust](../../examples/braintree/rust/braintree.rs#L95)

#### capture

**Examples:** [Python](../../examples/braintree/python/braintree.py) · [JavaScript](../../examples/braintree/javascript/braintree.ts#L320) · [Kotlin](../../examples/braintree/kotlin/braintree.kt) · [Rust](../../examples/braintree/rust/braintree.rs#L128)

#### get

**Examples:** [Python](../../examples/braintree/python/braintree.py) · [JavaScript](../../examples/braintree/javascript/braintree.ts#L339) · [Kotlin](../../examples/braintree/kotlin/braintree.kt) · [Rust](../../examples/braintree/rust/braintree.rs#L145)

#### void

**Examples:** [Python](../../examples/braintree/python/braintree.py) · [JavaScript](../../examples/braintree/javascript/braintree.ts#L363) · [Kotlin](../../examples/braintree/kotlin/braintree.kt) · [Rust](../../examples/braintree/rust/braintree.rs#L192)
