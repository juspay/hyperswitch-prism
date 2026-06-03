# Truelayer

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/truelayer.json
Regenerate: python3 scripts/generators/docs/generate.py truelayer
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
        truelayer=payment_pb2.TruelayerConfig(
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            client_secret=payment_methods_pb2.SecretString(value="YOUR_CLIENT_SECRET"),
            merchant_account_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT_ID"),
            account_holder_name=payment_methods_pb2.SecretString(value="YOUR_ACCOUNT_HOLDER_NAME"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            kid=payment_methods_pb2.SecretString(value="YOUR_KID"),
            base_url="YOUR_BASE_URL",
            secondary_base_url="YOUR_SECONDARY_BASE_URL",
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
    connector: Connector.TRUELAYER,
    environment: Environment.SANDBOX,
    auth: {
        truelayer: {
            clientId: { value: 'YOUR_CLIENT_ID' },
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            merchantAccountId: { value: 'YOUR_MERCHANT_ACCOUNT_ID' },
            accountHolderName: { value: 'YOUR_ACCOUNT_HOLDER_NAME' },
            privateKey: { value: 'YOUR_PRIVATE_KEY' },
            kid: { value: 'YOUR_KID' },
            baseUrl: 'YOUR_BASE_URL',
            secondaryBaseUrl: 'YOUR_SECONDARY_BASE_URL',
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
            .setTruelayer(TruelayerConfig.newBuilder()
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setMerchantAccountId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ACCOUNT_ID").build())
                .setAccountHolderName(SecretString.newBuilder().setValue("YOUR_ACCOUNT_HOLDER_NAME").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setKid(SecretString.newBuilder().setValue("YOUR_KID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setSecondaryBaseUrl("YOUR_SECONDARY_BASE_URL")
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
            config: Some(connector_specific_config::Config::Truelayer(TruelayerConfig {
                client_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_ID".to_string())),  // Authentication credential
                client_secret: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_SECRET".to_string())),  // Authentication credential
                merchant_account_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ACCOUNT_ID".to_string())),  // Authentication credential
                account_holder_name: Some(hyperswitch_masking::Secret::new("YOUR_ACCOUNT_HOLDER_NAME".to_string())),  // Authentication credential
                private_key: Some(hyperswitch_masking::Secret::new("YOUR_PRIVATE_KEY".to_string())),  // Authentication credential
                kid: Some(hyperswitch_masking::Secret::new("YOUR_KID".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                secondary_base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |

### Payments

#### PaymentService.Authorize

Authorize a payment amount on a payment method. This reserves funds without capturing them, essential for verifying availability before finalizing.

| | Message |
|---|---------|
| **Request** | `PaymentServiceAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ⚠ |
| Bancontact | ⚠ |
| Apple Pay | ⚠ |
| Apple Pay Dec | ⚠ |
| Apple Pay SDK | ⚠ |
| Google Pay | ⚠ |
| Google Pay Dec | ⚠ |
| Google Pay SDK | ⚠ |
| PayPal SDK | ⚠ |
| Amazon Pay | ⚠ |
| Cash App | ⚠ |
| PayPal | ⚠ |
| WeChat Pay | ⚠ |
| Alipay | ⚠ |
| Revolut Pay | ⚠ |
| MiFinity | ⚠ |
| Bluecode | ⚠ |
| Paze | x |
| Samsung Pay | ⚠ |
| MB Way | ⚠ |
| Satispay | ⚠ |
| Wero | ⚠ |
| GoPay | ⚠ |
| GCash | ⚠ |
| Momo | ⚠ |
| Dana | ⚠ |
| Kakao Pay | ⚠ |
| Touch 'n Go | ⚠ |
| Twint | ⚠ |
| Vipps | ⚠ |
| Swish | ⚠ |
| Affirm | ⚠ |
| Afterpay | ⚠ |
| Klarna | ⚠ |
| UPI Collect | ⚠ |
| UPI Intent | ⚠ |
| UPI QR | ⚠ |
| Thailand | ⚠ |
| Czech | ⚠ |
| Finland | ⚠ |
| FPX | ⚠ |
| Poland | ⚠ |
| Slovakia | ⚠ |
| UK | ⚠ |
| PIS | x |
| Generic | ✓ |
| Local | ⚠ |
| iDEAL | ⚠ |
| Sofort | ⚠ |
| Trustly | ⚠ |
| Giropay | ⚠ |
| EPS | ⚠ |
| Przelewy24 | ⚠ |
| PSE | ⚠ |
| BLIK | ⚠ |
| Interac | ⚠ |
| Bizum | ⚠ |
| EFT | ⚠ |
| DuitNow | x |
| ACH | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| Multibanco | ⚠ |
| Instant | ⚠ |
| Instant FI | ⚠ |
| Instant PL | ⚠ |
| Pix | ⚠ |
| Permata | ⚠ |
| BCA | ⚠ |
| BNI VA | ⚠ |
| BRI VA | ⚠ |
| CIMB VA | ⚠ |
| Danamon VA | ⚠ |
| Mandiri VA | ⚠ |
| Local | ⚠ |
| Indonesian | ⚠ |
| ACH | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| BECS | ⚠ |
| SEPA Guaranteed | ⚠ |
| Crypto | x |
| Reward | ⚠ |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | ⚠ |
| Boleto | ⚠ |
| Efecty | ⚠ |
| Pago Efectivo | ⚠ |
| Red Compra | ⚠ |
| Red Pagos | ⚠ |
| Alfamart | ⚠ |
| Indomaret | ⚠ |
| Oxxo | ⚠ |
| 7-Eleven | ⚠ |
| Lawson | ⚠ |
| Mini Stop | ⚠ |
| Family Mart | ⚠ |
| Seicomart | ⚠ |
| Pay Easy | ⚠ |

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L106) · [Kotlin](../../examples/truelayer/truelayer.kt#L76) · [Rust](../../examples/truelayer/truelayer.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L133) · [Kotlin](../../examples/truelayer/truelayer.kt#L115) · [Rust](../../examples/truelayer/truelayer.rs)

### Authentication

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/truelayer/truelayer.py) · [TypeScript](../../examples/truelayer/truelayer.ts#L97) · [Kotlin](../../examples/truelayer/truelayer.kt#L66) · [Rust](../../examples/truelayer/truelayer.rs)
