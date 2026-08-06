# Grabpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/grabpay.json
Regenerate: python3 scripts/generators/docs/generate.py grabpay
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        grabpay=payment_pb2.GrabpayConfig(
            partner_id=payment_methods_pb2.SecretString(value="YOUR_PARTNER_ID"),
            partner_secret=payment_methods_pb2.SecretString(value="YOUR_PARTNER_SECRET"),
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            client_secret=payment_methods_pb2.SecretString(value="YOUR_CLIENT_SECRET"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
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
    connector: Connector.GRABPAY,
    environment: Environment.SANDBOX,
    auth: {
        grabpay: {
            partnerId: { value: 'YOUR_PARTNER_ID' },
            partnerSecret: { value: 'YOUR_PARTNER_SECRET' },
            clientId: { value: 'YOUR_CLIENT_ID' },
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
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
            .setGrabpay(GrabpayConfig.newBuilder()
                .setPartnerId(SecretString.newBuilder().setValue("YOUR_PARTNER_ID").build())
                .setPartnerSecret(SecretString.newBuilder().setValue("YOUR_PARTNER_SECRET").build())
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
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
            config: Some(connector_specific_config::Config::Grabpay(GrabpayConfig {
                partner_id: Some(hyperswitch_masking::Secret::new("YOUR_PARTNER_ID".to_string())),  // Authentication credential
                partner_secret: Some(hyperswitch_masking::Secret::new("YOUR_PARTNER_SECRET".to_string())),  // Authentication credential
                client_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_ID".to_string())),  // Authentication credential
                client_secret: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_SECRET".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
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
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.VerifyRedirectResponse](#paymentserviceverifyredirectresponse) | Payments | `PaymentServiceVerifyRedirectResponseRequest` |

### Payments

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/grabpay/grabpay.py) · [TypeScript](../../examples/grabpay/grabpay.ts#L72) · [Kotlin](../../examples/grabpay/grabpay.kt#L43) · [Rust](../../examples/grabpay/grabpay.rs)

#### PaymentService.VerifyRedirectResponse

Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVerifyRedirectResponseRequest` |
| **Response** | `PaymentServiceVerifyRedirectResponseResponse` |

**Examples:** [Python](../../examples/grabpay/grabpay.py) · [TypeScript](../../examples/grabpay/grabpay.ts#L99) · [Kotlin](../../examples/grabpay/grabpay.kt#L88) · [Rust](../../examples/grabpay/grabpay.rs)
