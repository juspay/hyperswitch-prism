# Twoc Twop Paco

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/twoc_twop_paco.json
Regenerate: python3 scripts/generators/docs/generate.py twoc_twop_paco
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
        twoc_twop_paco=payment_pb2.TwocTwopPacoConfig(
            access_token=payment_methods_pb2.SecretString(value="YOUR_ACCESS_TOKEN"),
            office_id=payment_methods_pb2.SecretString(value="YOUR_OFFICE_ID"),
            paco_kid=payment_methods_pb2.SecretString(value="YOUR_PACO_KID"),
            merchant_signing_private_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_SIGNING_PRIVATE_KEY"),
            merchant_encryption_private_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY"),
            paco_signing_public_key=payment_methods_pb2.SecretString(value="YOUR_PACO_SIGNING_PUBLIC_KEY"),
            paco_encryption_public_key=payment_methods_pb2.SecretString(value="YOUR_PACO_ENCRYPTION_PUBLIC_KEY"),
            response_audience=payment_methods_pb2.SecretString(value="YOUR_RESPONSE_AUDIENCE"),
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
    connector: Connector.TWOC_TWOP_PACO,
    environment: Environment.SANDBOX,
    auth: {
        twocTwopPaco: {
            accessToken: { value: 'YOUR_ACCESS_TOKEN' },
            officeId: { value: 'YOUR_OFFICE_ID' },
            pacoKid: { value: 'YOUR_PACO_KID' },
            merchantSigningPrivateKey: { value: 'YOUR_MERCHANT_SIGNING_PRIVATE_KEY' },
            merchantEncryptionPrivateKey: { value: 'YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY' },
            pacoSigningPublicKey: { value: 'YOUR_PACO_SIGNING_PUBLIC_KEY' },
            pacoEncryptionPublicKey: { value: 'YOUR_PACO_ENCRYPTION_PUBLIC_KEY' },
            responseAudience: { value: 'YOUR_RESPONSE_AUDIENCE' },
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
            .setTwocTwopPaco(TwocTwopPacoConfig.newBuilder()
                .setAccessToken(SecretString.newBuilder().setValue("YOUR_ACCESS_TOKEN").build())
                .setOfficeId(SecretString.newBuilder().setValue("YOUR_OFFICE_ID").build())
                .setPacoKid(SecretString.newBuilder().setValue("YOUR_PACO_KID").build())
                .setMerchantSigningPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_SIGNING_PRIVATE_KEY").build())
                .setMerchantEncryptionPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY").build())
                .setPacoSigningPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_SIGNING_PUBLIC_KEY").build())
                .setPacoEncryptionPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_ENCRYPTION_PUBLIC_KEY").build())
                .setResponseAudience(SecretString.newBuilder().setValue("YOUR_RESPONSE_AUDIENCE").build())
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
    connector_config: None,  // TODO: Add your connector config here,
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
| [PaymentService.VerifyRedirectResponse](#paymentserviceverifyredirectresponse) | Payments | `PaymentServiceVerifyRedirectResponseRequest` |

### Payments

#### PaymentService.VerifyRedirectResponse

Verify and process redirect responses from 3D Secure or other external flows. Validates authentication results and updates payment state accordingly.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVerifyRedirectResponseRequest` |
| **Response** | `PaymentServiceVerifyRedirectResponseResponse` |

**Examples:** [Python](../../examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](../../examples/twoc_twop_paco/twoc_twop_paco.ts#L40) · [Kotlin](../../examples/twoc_twop_paco/twoc_twop_paco.kt#L44) · [Rust](../../examples/twoc_twop_paco/twoc_twop_paco.rs)
