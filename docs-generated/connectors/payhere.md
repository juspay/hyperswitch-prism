# Payhere

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/payhere.json
Regenerate: python3 scripts/generators/docs/generate.py payhere
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2, events_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        payhere=payment_pb2.PayhereConfig(
            app_id=payment_methods_pb2.SecretString(value="YOUR_APP_ID"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            app_secret=payment_methods_pb2.SecretString(value="YOUR_APP_SECRET"),
            merchant_secret=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_SECRET"),
            base_url="YOUR_BASE_URL",
        ),
    ),
)

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { PaymentClient } = require('hyperswitch-prism');
const { ConnectorConfig, Environment, Connector } = require('hyperswitch-prism').types;

const config = ConnectorConfig.create({
    connector: Connector.PAYHERE,
    environment: Environment.SANDBOX,
    auth: {
        payhere: {
            appId: { value: 'YOUR_APP_ID' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            appSecret: { value: 'YOUR_APP_SECRET' },
            merchantSecret: { value: 'YOUR_MERCHANT_SECRET' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setPayhere(PayhereConfig.newBuilder()
                .setAppId(SecretString.newBuilder().setValue("YOUR_APP_ID").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setAppSecret(SecretString.newBuilder().setValue("YOUR_APP_SECRET").build())
                .setMerchantSecret(SecretString.newBuilder().setValue("YOUR_MERCHANT_SECRET").build())
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;

let config = ConnectorConfig {
    connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Payhere(PayhereConfig {
                app_id: Some(hyperswitch_masking::Secret::new("YOUR_APP_ID".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                app_secret: Some(hyperswitch_masking::Secret::new("YOUR_APP_SECRET".to_string())),  // Authentication credential
                merchant_secret: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_SECRET".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                ..Default::default()
            })),
        }),
    options: Some(SdkOptions {
        environment: Environment::Sandbox.into(),
    }),
};
```

</details>

</td>
</tr>
</table>

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.VerifyRedirectResponse](#paymentserviceverifyredirectresponse) | Payments | `PaymentServiceVerifyRedirectResponseRequest` |

### Payments

#### PaymentService.VerifyRedirectResponse

Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVerifyRedirectResponseRequest` |
| **Response** | `PaymentServiceVerifyRedirectResponseResponse` |

**Examples:** [Python](../../examples/payhere/payhere.py) · [TypeScript](../../examples/payhere/payhere.ts#L93) · [Kotlin](../../examples/payhere/payhere.kt#L84) · [Rust](../../examples/payhere/payhere.rs)

### Authentication

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/payhere/payhere.py) · [TypeScript](../../examples/payhere/payhere.ts#L66) · [Kotlin](../../examples/payhere/payhere.kt#L43) · [Rust](../../examples/payhere/payhere.rs)
