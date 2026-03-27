# Jpmorgan

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/jpmorgan.json
Regenerate: python3 scripts/generators/docs/generate.py jpmorgan
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
        jpmorgan: {
        clientId: { value: 'YOUR_CLIENT_ID' },
        clientSecret: { value: 'YOUR_CLIENT_SECRET' },
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
        config: Some(connector_specific_config::Config::Jpmorgan(JpmorganConfig {
                client_id: Some(Secret::new("YOUR_CLIENT_ID".to_string())),
                client_secret: Some(Secret::new("YOUR_CLIENT_SECRET".to_string())),
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

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L5) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L34) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L6) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L18)

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L13) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L104) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L10) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L30)

### Bank Transfer (SEPA / ACH / BACS)

Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L19) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L153) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L14) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L39)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L25) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L200) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L18) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L48)

### Void a Payment

Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L33) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L272) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L22) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L60)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py#L41) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.js#L334) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt#L26) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L72)

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

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.ts#L469) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L147)

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

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.ts#L398) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L84)

#### capture

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.ts#L443) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L123)

#### get

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.ts#L478) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L154)

#### refund

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.ts#L500) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L178)

#### void

**Examples:** [Python](../../examples/jpmorgan/python/jpmorgan.py) · [JavaScript](../../examples/jpmorgan/javascript/jpmorgan.ts#L528) · [Kotlin](../../examples/jpmorgan/kotlin/jpmorgan.kt) · [Rust](../../examples/jpmorgan/rust/jpmorgan.rs#L204)
