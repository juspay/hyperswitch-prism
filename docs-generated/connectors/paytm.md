# Paytm

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paytm.json
Regenerate: python3 scripts/generators/docs/generate.py paytm
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
        paytm: {
        merchantId: { value: 'YOUR_MERCHANT_ID' },
        merchantKey: { value: 'YOUR_MERCHANT_KEY' },
        website: { value: 'YOUR_WEBSITE' },
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
        config: Some(connector_specific_config::Config::Paytm(PaytmConfig {
                merchant_id: Some(Secret::new("YOUR_MERCHANT_ID".to_string())),
                merchant_key: Some(Secret::new("YOUR_MERCHANT_KEY".to_string())),
                website: Some(Secret::new("YOUR_WEBSITE".to_string())),
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
| [MerchantAuthenticationService.CreateSessionToken](#merchantauthenticationservicecreatesessiontoken) | Authentication | `MerchantAuthenticationServiceCreateSessionTokenRequest` |
| [get](#get) | Other | `—` |

### Authentication

#### MerchantAuthenticationService.CreateSessionToken

Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateSessionTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateSessionTokenResponse` |

**Examples:** [Python](../../examples/paytm/python/paytm.py) · [JavaScript](../../examples/paytm/javascript/paytm.ts) · [Kotlin](../../examples/paytm/kotlin/paytm.kt) · [Rust](../../examples/paytm/rust/paytm.rs#L47)

### Other

#### authorize

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | x |
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
| UPI | ✓ |
| Affirm | x |
| Samsung Pay | x |

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### UPI Collect

```python
"payment_method": {
    "upi_collect": {  # UPI Collect
        "vpa_id": {"value": "test@upi"}  # Virtual Payment Address
    }
}
```

**Examples:** [Python](../../examples/paytm/python/paytm.py) · [JavaScript](../../examples/paytm/javascript/paytm.ts) · [Kotlin](../../examples/paytm/kotlin/paytm.kt) · [Rust](../../examples/paytm/rust/paytm.rs#L18)

#### get

**Examples:** [Python](../../examples/paytm/python/paytm.py) · [JavaScript](../../examples/paytm/javascript/paytm.ts) · [Kotlin](../../examples/paytm/kotlin/paytm.kt) · [Rust](../../examples/paytm/rust/paytm.rs#L62)
