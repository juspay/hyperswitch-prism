# Gigadat

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/gigadat.json
Regenerate: python3 scripts/generators/docs/generate.py gigadat
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
        gigadat=payment_pb2.GigadatConfig(
            campaign_id=payment_methods_pb2.SecretString(value="YOUR_CAMPAIGN_ID"),
            access_token=payment_methods_pb2.SecretString(value="YOUR_ACCESS_TOKEN"),
            security_token=payment_methods_pb2.SecretString(value="YOUR_SECURITY_TOKEN"),
            base_url="YOUR_BASE_URL",
            site="YOUR_SITE",
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
    connector: Connector.GIGADAT,
    environment: Environment.SANDBOX,
    auth: {
        gigadat: {
            campaignId: { value: 'YOUR_CAMPAIGN_ID' },
            accessToken: { value: 'YOUR_ACCESS_TOKEN' },
            securityToken: { value: 'YOUR_SECURITY_TOKEN' },
            baseUrl: 'YOUR_BASE_URL',
            site: 'YOUR_SITE',
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
            .setGigadat(GigadatConfig.newBuilder()
                .setCampaignId(SecretString.newBuilder().setValue("YOUR_CAMPAIGN_ID").build())
                .setAccessToken(SecretString.newBuilder().setValue("YOUR_ACCESS_TOKEN").build())
                .setSecurityToken(SecretString.newBuilder().setValue("YOUR_SECURITY_TOKEN").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setSite("YOUR_SITE")
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
            config: Some(connector_specific_config::Config::Gigadat(GigadatConfig {
                campaign_id: Some(hyperswitch_masking::Secret::new("YOUR_CAMPAIGN_ID".to_string())),  // Authentication credential
                access_token: Some(hyperswitch_masking::Secret::new("YOUR_ACCESS_TOKEN".to_string())),  // Authentication credential
                security_token: Some(hyperswitch_masking::Secret::new("YOUR_SECURITY_TOKEN".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                site: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |

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
| Card | x |
| Bancontact | x |
| Apple Pay | x |
| Apple Pay Dec | x |
| Apple Pay SDK | x |
| Google Pay | x |
| Google Pay Dec | x |
| Google Pay SDK | x |
| PayPal SDK | x |
| Amazon Pay | x |
| Cash App | x |
| PayPal | x |
| WeChat Pay | x |
| Alipay | x |
| Revolut Pay | x |
| MiFinity | x |
| Bluecode | x |
| Paze | x |
| Samsung Pay | x |
| MB Way | x |
| Satispay | x |
| Wero | x |
| Affirm | x |
| Afterpay | x |
| Klarna | x |
| UPI Collect | x |
| UPI Intent | x |
| UPI QR | x |
| Thailand | x |
| Czech | x |
| Finland | x |
| FPX | x |
| Poland | x |
| Slovakia | x |
| UK | x |
| PIS | x |
| Generic | x |
| Local | x |
| iDEAL | x |
| Sofort | x |
| Trustly | x |
| Giropay | x |
| EPS | x |
| Przelewy24 | x |
| PSE | x |
| BLIK | x |
| Interac | ✓ |
| Bizum | x |
| EFT | x |
| DuitNow | x |
| ACH | x |
| SEPA | x |
| BACS | x |
| Multibanco | x |
| Instant | x |
| Instant FI | x |
| Instant PL | x |
| Pix | x |
| Permata | x |
| BCA | x |
| BNI VA | x |
| BRI VA | x |
| CIMB VA | x |
| Danamon VA | x |
| Mandiri VA | x |
| Local | x |
| Indonesian | x |
| ACH | x |
| SEPA | x |
| BACS | x |
| BECS | x |
| SEPA Guaranteed | x |
| Crypto | x |
| Reward | x |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | x |
| Boleto | x |
| Efecty | x |
| Pago Efectivo | x |
| Red Compra | x |
| Red Pagos | x |
| Alfamart | x |
| Indomaret | x |
| Oxxo | x |
| 7-Eleven | x |
| Lawson | x |
| Mini Stop | x |
| Family Mart | x |
| Seicomart | x |
| Pay Easy | x |

**Examples:** [Python](../../examples/gigadat/gigadat.py) · [TypeScript](../../examples/gigadat/gigadat.ts) · [Kotlin](../../examples/gigadat/gigadat.kt) · [Rust](../../examples/gigadat/gigadat.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/gigadat/gigadat.py) · [TypeScript](../../examples/gigadat/gigadat.ts#L55) · [Kotlin](../../examples/gigadat/gigadat.kt#L65) · [Rust](../../examples/gigadat/gigadat.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/gigadat/gigadat.py) · [TypeScript](../../examples/gigadat/gigadat.ts#L64) · [Kotlin](../../examples/gigadat/gigadat.kt#L73) · [Rust](../../examples/gigadat/gigadat.rs)
