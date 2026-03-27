# Getnet

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/getnet.json
Regenerate: python3 scripts/generators/docs/generate.py getnet
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
        getnet: {
        apiKey: { value: 'YOUR_API_KEY' },
        apiSecret: { value: 'YOUR_API_SECRET' },
        sellerId: { value: 'YOUR_SELLER_ID' },
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
        config: Some(connector_specific_config::Config::Getnet(GetnetConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
                api_secret: Some(Secret::new("YOUR_API_SECRET".to_string())),
                seller_id: Some(Secret::new("YOUR_SELLER_ID".to_string())),
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

**Examples:** [Python](../../examples/getnet/python/getnet.py#L5) · [JavaScript](../../examples/getnet/javascript/getnet.js#L35) · [Kotlin](../../examples/getnet/kotlin/getnet.kt#L6) · [Rust](../../examples/getnet/rust/getnet.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/getnet/python/getnet.py#L13) · [JavaScript](../../examples/getnet/javascript/getnet.js#L105) · [Kotlin](../../examples/getnet/kotlin/getnet.kt#L10) · [Rust](../../examples/getnet/rust/getnet.rs#L30)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/getnet/python/getnet.py#L19) · [JavaScript](../../examples/getnet/javascript/getnet.js#L154) · [Kotlin](../../examples/getnet/kotlin/getnet.kt#L14) · [Rust](../../examples/getnet/rust/getnet.rs#L39)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/getnet/python/getnet.py#L27) · [JavaScript](../../examples/getnet/javascript/getnet.js#L226) · [Kotlin](../../examples/getnet/kotlin/getnet.kt#L18) · [Rust](../../examples/getnet/rust/getnet.rs#L51)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/getnet/python/getnet.py#L35) · [JavaScript](../../examples/getnet/javascript/getnet.js#L292) · [Kotlin](../../examples/getnet/kotlin/getnet.kt#L22) · [Rust](../../examples/getnet/rust/getnet.rs#L63)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |
| [void](#void) | Other | `—` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.ts#L427) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs#L138)

### Other

#### authorize

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
        "card_number": {"value": "4111111111111111"},  # Card Identification
        "card_exp_month": {"value": "03"},
        "card_exp_year": {"value": "2030"},
        "card_cvc": {"value": "737"},
        "card_holder_name": {"value": "John Doe"}  # Cardholder Information
    }
}
```

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.ts#L356) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs#L75)

#### capture

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.ts#L401) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs#L114)

#### get

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.ts#L436) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs#L145)

#### refund

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.ts#L458) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs#L169)

#### void

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.ts#L486) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs#L195)
