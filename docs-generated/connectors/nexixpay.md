# Nexixpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/nexixpay.json
Regenerate: python3 scripts/generators/docs/generate.py nexixpay
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
        nexixpay: {
        apiKey: { value: 'YOUR_API_KEY' },
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
        config: Some(connector_specific_config::Config::Nexixpay(NexixpayConfig {
                api_key: Some(Secret::new("YOUR_API_KEY".to_string())),
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
| [capture](#capture) | Other | `—` |
| [get](#get) | Other | `—` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| [refund](#refund) | Other | `—` |
| [void](#void) | Other | `—` |

### Authentication

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/nexixpay/python/nexixpay.py) · [JavaScript](../../examples/nexixpay/javascript/nexixpay.ts) · [Kotlin](../../examples/nexixpay/kotlin/nexixpay.kt) · [Rust](../../examples/nexixpay/rust/nexixpay.rs#L52)

### Other

#### capture

**Examples:** [Python](../../examples/nexixpay/python/nexixpay.py) · [JavaScript](../../examples/nexixpay/javascript/nexixpay.ts) · [Kotlin](../../examples/nexixpay/kotlin/nexixpay.kt#L22) · [Rust](../../examples/nexixpay/rust/nexixpay.rs#L18)

#### get

**Examples:** [Python](../../examples/nexixpay/python/nexixpay.py) · [JavaScript](../../examples/nexixpay/javascript/nexixpay.ts) · [Kotlin](../../examples/nexixpay/kotlin/nexixpay.kt#L30) · [Rust](../../examples/nexixpay/rust/nexixpay.rs#L35)

#### refund

**Examples:** [Python](../../examples/nexixpay/python/nexixpay.py) · [JavaScript](../../examples/nexixpay/javascript/nexixpay.ts) · [Kotlin](../../examples/nexixpay/kotlin/nexixpay.kt#L46) · [Rust](../../examples/nexixpay/rust/nexixpay.rs#L84)

#### void

**Examples:** [Python](../../examples/nexixpay/python/nexixpay.py) · [JavaScript](../../examples/nexixpay/javascript/nexixpay.ts) · [Kotlin](../../examples/nexixpay/kotlin/nexixpay.kt#L54) · [Rust](../../examples/nexixpay/rust/nexixpay.rs#L103)
