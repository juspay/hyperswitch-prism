# Iatapay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/iatapay.json
Regenerate: python3 scripts/generators/docs/generate.py iatapay
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
        iatapay: {
        clientId: { value: 'YOUR_CLIENT_ID' },
        merchantId: { value: 'YOUR_MERCHANT_ID' },
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
        config: Some(connector_specific_config::Config::Iatapay(IatapayConfig {
                client_id: Some(Secret::new("YOUR_CLIENT_ID".to_string())),
                merchant_id: Some(Secret::new("YOUR_MERCHANT_ID".to_string())),
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

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [authorize](#authorize) | Other | `—` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [get](#get) | Other | `—` |
| [refund](#refund) | Other | `—` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/iatapay/python/iatapay.py) · [JavaScript](../../examples/iatapay/javascript/iatapay.ts) · [Kotlin](../../examples/iatapay/kotlin/iatapay.kt) · [Rust](../../examples/iatapay/rust/iatapay.rs#L53)

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ⚠ |
| Google Pay | ⚠ |
| Apple Pay | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| ACH | ⚠ |
| BECS | ⚠ |
| iDEAL | ✓ |
| PayPal | ⚠ |
| BLIK | ⚠ |
| Klarna | ⚠ |
| Afterpay | ⚠ |
| UPI | ✓ |
| Affirm | ⚠ |
| Samsung Pay | ⚠ |

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### iDEAL

```python
"payment_method": {
    "ideal": {
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

**Examples:** [Python](../../examples/iatapay/python/iatapay.py) · [JavaScript](../../examples/iatapay/javascript/iatapay.ts) · [Kotlin](../../examples/iatapay/kotlin/iatapay.kt) · [Rust](../../examples/iatapay/rust/iatapay.rs#L18)

#### get

**Examples:** [Python](../../examples/iatapay/python/iatapay.py) · [JavaScript](../../examples/iatapay/javascript/iatapay.ts) · [Kotlin](../../examples/iatapay/kotlin/iatapay.kt) · [Rust](../../examples/iatapay/rust/iatapay.rs#L60)

#### refund

**Examples:** [Python](../../examples/iatapay/python/iatapay.py) · [JavaScript](../../examples/iatapay/javascript/iatapay.ts) · [Kotlin](../../examples/iatapay/kotlin/iatapay.kt) · [Rust](../../examples/iatapay/rust/iatapay.rs#L85)
