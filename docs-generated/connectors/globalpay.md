# Globalpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/globalpay.json
Regenerate: python3 scripts/generators/docs/generate.py globalpay
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
        globalpay=payment_pb2.GlobalpayConfig(
            app_id=payment_methods_pb2.SecretString(value="YOUR_APP_ID"),
            app_key=payment_methods_pb2.SecretString(value="YOUR_APP_KEY"),
            account_name=payment_methods_pb2.SecretString(value="YOUR_ACCOUNT_NAME"),
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
    connector: Connector.GLOBALPAY,
    environment: Environment.SANDBOX,
    auth: {
        globalpay: {
            appId: { value: 'YOUR_APP_ID' },
            appKey: { value: 'YOUR_APP_KEY' },
            accountName: { value: 'YOUR_ACCOUNT_NAME' },
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
            .setGlobalpay(GlobalpayConfig.newBuilder()
                .setAppId(SecretString.newBuilder().setValue("YOUR_APP_ID").build())
                .setAppKey(SecretString.newBuilder().setValue("YOUR_APP_KEY").build())
                .setAccountName(SecretString.newBuilder().setValue("YOUR_ACCOUNT_NAME").build())
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
            config: Some(connector_specific_config::Config::Globalpay(GlobalpayConfig {
                app_id: Some(hyperswitch_masking::Secret::new("YOUR_APP_ID".to_string())),  // Authentication credential
                app_key: Some(hyperswitch_masking::Secret::new("YOUR_APP_KEY".to_string())),  // Authentication credential
                account_name: Some(hyperswitch_masking::Secret::new("YOUR_ACCOUNT_NAME".to_string())),  // Authentication credential
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
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L206) · [Kotlin](../../examples/globalpay/globalpay.kt#L116) · [Rust](../../examples/globalpay/globalpay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L233) · [Kotlin](../../examples/globalpay/globalpay.kt#L152) · [Rust](../../examples/globalpay/globalpay.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L242) · [Kotlin](../../examples/globalpay/globalpay.kt#L160) · [Rust](../../examples/globalpay/globalpay.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L251) · [Kotlin](../../examples/globalpay/globalpay.kt#L199) · [Rust](../../examples/globalpay/globalpay.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L269) · [Kotlin](../../examples/globalpay/globalpay.kt#L228) · [Rust](../../examples/globalpay/globalpay.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts) · [Kotlin](../../examples/globalpay/globalpay.kt#L274) · [Rust](../../examples/globalpay/globalpay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L260) · [Kotlin](../../examples/globalpay/globalpay.kt#L209) · [Rust](../../examples/globalpay/globalpay.rs)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L215) · [Kotlin](../../examples/globalpay/globalpay.kt#L126) · [Rust](../../examples/globalpay/globalpay.rs)

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py) · [TypeScript](../../examples/globalpay/globalpay.ts#L224) · [Kotlin](../../examples/globalpay/globalpay.kt#L142) · [Rust](../../examples/globalpay/globalpay.rs)
