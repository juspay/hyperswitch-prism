# Netcetera

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/netcetera.json
Regenerate: python3 scripts/generators/docs/generate.py netcetera
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
        netcetera=payment_pb2.NetceteraConfig(
            certificate=payment_methods_pb2.SecretString(value="YOUR_CERTIFICATE"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            three_ds_requestor_id="YOUR_THREE_DS_REQUESTOR_ID",
            three_ds_requestor_name="YOUR_THREE_DS_REQUESTOR_NAME",
            merchant_configuration_id="YOUR_MERCHANT_CONFIGURATION_ID",
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
    connector: Connector.NETCETERA,
    environment: Environment.SANDBOX,
    auth: {
        netcetera: {
            certificate: { value: 'YOUR_CERTIFICATE' },
            privateKey: { value: 'YOUR_PRIVATE_KEY' },
            threeDsRequestorId: 'YOUR_THREE_DS_REQUESTOR_ID',
            threeDsRequestorName: 'YOUR_THREE_DS_REQUESTOR_NAME',
            merchantConfigurationId: 'YOUR_MERCHANT_CONFIGURATION_ID',
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
            .setNetcetera(NetceteraConfig.newBuilder()
                .setCertificate(SecretString.newBuilder().setValue("YOUR_CERTIFICATE").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setThreeDsRequestorId("YOUR_THREE_DS_REQUESTOR_ID")
                .setThreeDsRequestorName("YOUR_THREE_DS_REQUESTOR_NAME")
                .setMerchantConfigurationId("YOUR_MERCHANT_CONFIGURATION_ID")
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
            config: Some(connector_specific_config::Config::Netcetera(NetceteraConfig {
                certificate: Some(hyperswitch_masking::Secret::new("YOUR_CERTIFICATE".to_string())),  // Authentication credential
                private_key: Some(hyperswitch_masking::Secret::new("YOUR_PRIVATE_KEY".to_string())),  // Authentication credential
                three_ds_requestor_id: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                three_ds_requestor_name: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                merchant_configuration_id: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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
| [PaymentMethodAuthenticationService.Authenticate](#paymentmethodauthenticationserviceauthenticate) | Authentication | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| [PaymentMethodAuthenticationService.PostAuthenticate](#paymentmethodauthenticationservicepostauthenticate) | Authentication | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |

### Authentication

#### PaymentMethodAuthenticationService.Authenticate

Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServiceAuthenticateResponse` |

**Examples:** [Python](../../examples/netcetera/netcetera.py) · [TypeScript](../../examples/netcetera/netcetera.ts#L101) · [Kotlin](../../examples/netcetera/netcetera.kt#L42) · [Rust](../../examples/netcetera/netcetera.rs)

#### PaymentMethodAuthenticationService.PostAuthenticate

Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePostAuthenticateResponse` |

**Examples:** [Python](../../examples/netcetera/netcetera.py) · [TypeScript](../../examples/netcetera/netcetera.ts#L110) · [Kotlin](../../examples/netcetera/netcetera.kt#L69) · [Rust](../../examples/netcetera/netcetera.rs)

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/netcetera/netcetera.py) · [TypeScript](../../examples/netcetera/netcetera.ts#L119) · [Kotlin](../../examples/netcetera/netcetera.kt#L95) · [Rust](../../examples/netcetera/netcetera.rs)
