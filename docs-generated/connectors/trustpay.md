# TrustPay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/trustpay.json
Regenerate: python3 scripts/generators/docs/generate.py trustpay
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
        trustpay: {
        apiKey: { value: 'YOUR_API_KEY' },
        projectId: { value: 'YOUR_PROJECT_ID' },
        secretKey: { value: 'YOUR_SECRET_KEY' },
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
        config: Some(connector_specific_config::Config::Trustpay(TrustpayConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
                project_id: Some(Secret::new("YOUR_PROJECT_ID".to_string())),
                secret_key: Some(Secret::new("YOUR_SECRET_KEY".to_string())),
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

### Card Payment (Automatic Capture)

Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/trustpay/python/trustpay.py#L5) · [JavaScript](../../examples/trustpay/javascript/trustpay.js#L35) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt#L6) · [Rust](../../examples/trustpay/rust/trustpay.rs#L18)

### Refund a Payment

Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.

**Examples:** [Python](../../examples/trustpay/python/trustpay.py#L11) · [JavaScript](../../examples/trustpay/javascript/trustpay.js#L96) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt#L10) · [Rust](../../examples/trustpay/rust/trustpay.rs#L27)

### Get Payment Status

Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.

**Examples:** [Python](../../examples/trustpay/python/trustpay.py#L19) · [JavaScript](../../examples/trustpay/javascript/trustpay.js#L180) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt#L14) · [Rust](../../examples/trustpay/rust/trustpay.rs#L39)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [create_order](#create_order) | Other | `—` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/trustpay/python/trustpay.py) · [JavaScript](../../examples/trustpay/javascript/trustpay.ts#L313) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt) · [Rust](../../examples/trustpay/rust/trustpay.rs#L102)

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
| iDEAL | ✓ |
| PayPal | ⚠ |
| BLIK | ✓ |
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

**Examples:** [Python](../../examples/trustpay/python/trustpay.py) · [JavaScript](../../examples/trustpay/javascript/trustpay.ts#L256) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt) · [Rust](../../examples/trustpay/rust/trustpay.rs#L51)

#### create_order

**Examples:** [Python](../../examples/trustpay/python/trustpay.py) · [JavaScript](../../examples/trustpay/javascript/trustpay.ts#L322) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt) · [Rust](../../examples/trustpay/rust/trustpay.rs#L109)

#### get

**Examples:** [Python](../../examples/trustpay/python/trustpay.py) · [JavaScript](../../examples/trustpay/javascript/trustpay.ts#L343) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt) · [Rust](../../examples/trustpay/rust/trustpay.rs#L132)

#### refund

**Examples:** [Python](../../examples/trustpay/python/trustpay.py) · [JavaScript](../../examples/trustpay/javascript/trustpay.ts#L365) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt) · [Rust](../../examples/trustpay/rust/trustpay.rs#L156)
