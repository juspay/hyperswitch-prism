# Paysafe

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paysafe.json
Regenerate: python3 scripts/generators/docs/generate.py paysafe
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
        paysafe=payment_pb2.PaysafeConfig(
            username=payment_methods_pb2.SecretString(value="YOUR_USERNAME"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
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
    connector: Connector.PAYSAFE,
    environment: Environment.SANDBOX,
    auth: {
        paysafe: {
            username: { value: 'YOUR_USERNAME' },
            password: { value: 'YOUR_PASSWORD' },
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
            .setPaysafe(PaysafeConfig.newBuilder()
                .setUsername(SecretString.newBuilder().setValue("YOUR_USERNAME").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
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
            config: Some(connector_specific_config::Config::Paysafe(PaysafeConfig {
                username: Some(hyperswitch_masking::Secret::new("YOUR_USERNAME".to_string())),  // Authentication credential
                password: Some(hyperswitch_masking::Secret::new("YOUR_PASSWORD".to_string())),  // Authentication credential
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
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |

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
| Card | ? |
| Bancontact | ? |
| Apple Pay | ? |
| Apple Pay Dec | ? |
| Apple Pay SDK | ? |
| Google Pay | ? |
| Google Pay Dec | ? |
| Google Pay SDK | ? |
| PayPal SDK | ? |
| Amazon Pay | ? |
| Cash App | ? |
| PayPal | ? |
| WeChat Pay | ? |
| Alipay | ? |
| Revolut Pay | ? |
| MiFinity | ? |
| Bluecode | ? |
| Paze | x |
| Samsung Pay | ? |
| MB Way | ? |
| Satispay | ? |
| Wero | ? |
| GoPay | ? |
| GCash | ? |
| Momo | ? |
| Dana | ? |
| Kakao Pay | ? |
| Touch 'n Go | ? |
| Twint | ? |
| Vipps | ? |
| Swish | ? |
| Affirm | ? |
| Afterpay | ? |
| Klarna | ? |
| UPI Collect | ? |
| UPI Intent | ? |
| UPI QR | ? |
| Thailand | ? |
| Czech | ? |
| Finland | ? |
| FPX | ? |
| Poland | ? |
| Slovakia | ? |
| UK | ? |
| PIS | x |
| Generic | ? |
| Local | ? |
| iDEAL | ? |
| Sofort | ? |
| Trustly | ? |
| Giropay | ? |
| EPS | ? |
| Przelewy24 | ? |
| PSE | ? |
| BLIK | ? |
| Interac | ? |
| Bizum | ? |
| EFT | ? |
| DuitNow | x |
| ACH | ? |
| SEPA | ? |
| BACS | ? |
| Multibanco | ? |
| Instant | ? |
| Instant FI | ? |
| Instant PL | ? |
| Pix | ? |
| Permata | ? |
| BCA | ? |
| BNI VA | ? |
| BRI VA | ? |
| CIMB VA | ? |
| Danamon VA | ? |
| Mandiri VA | ? |
| Local | ? |
| Indonesian | ? |
| ACH | ? |
| SEPA | ? |
| BACS | ? |
| BECS | ? |
| SEPA Guaranteed | ? |
| Crypto | x |
| Reward | ? |
| Givex | x |
| PaySafeCard | ✓ |
| E-Voucher | ? |
| Boleto | ? |
| Efecty | ? |
| Pago Efectivo | ? |
| Red Compra | ? |
| Red Pagos | ? |
| Alfamart | ? |
| Indomaret | ? |
| Oxxo | ? |
| 7-Eleven | ? |
| Lawson | ? |
| Mini Stop | ? |
| Family Mart | ? |
| Seicomart | ? |
| Pay Easy | ? |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts) · [Kotlin](../../examples/paysafe/paysafe.kt) · [Rust](../../examples/paysafe/paysafe.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L155) · [Kotlin](../../examples/paysafe/paysafe.kt#L106) · [Rust](../../examples/paysafe/paysafe.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L173) · [Kotlin](../../examples/paysafe/paysafe.kt#L129) · [Rust](../../examples/paysafe/paysafe.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L191) · [Kotlin](../../examples/paysafe/paysafe.kt#L165) · [Rust](../../examples/paysafe/paysafe.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L209) · [Kotlin](../../examples/paysafe/paysafe.kt#L187) · [Rust](../../examples/paysafe/paysafe.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L200) · [Kotlin](../../examples/paysafe/paysafe.kt#L175) · [Rust](../../examples/paysafe/paysafe.rs)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L164) · [Kotlin](../../examples/paysafe/paysafe.kt#L116) · [Rust](../../examples/paysafe/paysafe.rs)

### Authentication

#### PaymentMethodAuthenticationService.Authenticate

Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServiceAuthenticateResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L146) · [Kotlin](../../examples/paysafe/paysafe.kt#L79) · [Rust](../../examples/paysafe/paysafe.rs)

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py) · [TypeScript](../../examples/paysafe/paysafe.ts#L182) · [Kotlin](../../examples/paysafe/paysafe.kt#L137) · [Rust](../../examples/paysafe/paysafe.rs)
