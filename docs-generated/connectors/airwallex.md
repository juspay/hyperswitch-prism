# Airwallex

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/airwallex.json
Regenerate: python3 scripts/generators/docs/generate.py airwallex
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
        airwallex: {
        apiKey: { value: 'YOUR_API_KEY' },
        clientId: { value: 'YOUR_CLIENT_ID' },
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
        config: Some(connector_specific_config::Config::Airwallex(AirwallexConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
                client_id: Some(Secret::new("YOUR_CLIENT_ID".to_string())),
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

**Examples:** [Python](../../examples/airwallex/python/airwallex.py#L26) · [JavaScript](../../examples/airwallex/javascript/airwallex.js#L34) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L27) · [Rust](../../examples/airwallex/rust/airwallex.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/airwallex/python/airwallex.py#L77) · [JavaScript](../../examples/airwallex/javascript/airwallex.js#L105) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L41) · [Rust](../../examples/airwallex/rust/airwallex.rs#L83)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/airwallex/python/airwallex.py#L112) · [JavaScript](../../examples/airwallex/javascript/airwallex.js#L155) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L51) · [Rust](../../examples/airwallex/rust/airwallex.rs#L128)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/airwallex/python/airwallex.py#L165) · [JavaScript](../../examples/airwallex/javascript/airwallex.js#L228) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L65) · [Rust](../../examples/airwallex/rust/airwallex.rs#L195)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/airwallex/python/airwallex.py#L212) · [JavaScript](../../examples/airwallex/javascript/airwallex.js#L291) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L79) · [Rust](../../examples/airwallex/rust/airwallex.rs#L256)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [capture](#capture) | Other | `—` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [create_order](#create_order) | Other | `—` |
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

**Examples:** [Python](../../examples/airwallex/python/airwallex.py) · [JavaScript](../../examples/airwallex/javascript/airwallex.ts#L428) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt) · [Rust](../../examples/airwallex/rust/airwallex.rs#L388)

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
| iDEAL | ✓ |
| PayPal | x |
| BLIK | ✓ |
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

##### iDEAL

```python
"payment_method": {
    "ideal": {
    }
}
```

##### BLIK

```python
"payment_method": {
    "blik": {
        "blik_code": "777124"
    }
}
```

**Examples:** [Python](../../examples/airwallex/python/airwallex.py) · [JavaScript](../../examples/airwallex/javascript/airwallex.ts#L356) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L93) · [Rust](../../examples/airwallex/rust/airwallex.rs#L321)

#### capture

**Examples:** [Python](../../examples/airwallex/python/airwallex.py) · [JavaScript](../../examples/airwallex/javascript/airwallex.ts#L402) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L101) · [Rust](../../examples/airwallex/rust/airwallex.rs#L364)

#### create_order

**Examples:** [Python](../../examples/airwallex/python/airwallex.py) · [JavaScript](../../examples/airwallex/javascript/airwallex.ts#L437) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt) · [Rust](../../examples/airwallex/rust/airwallex.rs#L395)

#### get

**Examples:** [Python](../../examples/airwallex/python/airwallex.py) · [JavaScript](../../examples/airwallex/javascript/airwallex.ts#L458) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L125) · [Rust](../../examples/airwallex/rust/airwallex.rs#L418)

#### refund

**Examples:** [Python](../../examples/airwallex/python/airwallex.py) · [JavaScript](../../examples/airwallex/javascript/airwallex.ts#L480) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L133) · [Rust](../../examples/airwallex/rust/airwallex.rs#L442)

#### void

**Examples:** [Python](../../examples/airwallex/python/airwallex.py) · [JavaScript](../../examples/airwallex/javascript/airwallex.ts#L508) · [Kotlin](../../examples/airwallex/kotlin/airwallex.kt#L141) · [Rust](../../examples/airwallex/rust/airwallex.rs#L468)
