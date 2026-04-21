# Braintree

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/braintree.json
Regenerate: python3 scripts/generators/docs/generate.py braintree
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
        braintree=payment_pb2.BraintreeConfig(
            public_key=payment_methods_pb2.SecretString(value="YOUR_PUBLIC_KEY"),
            private_key=payment_methods_pb2.SecretString(value="YOUR_PRIVATE_KEY"),
            base_url="YOUR_BASE_URL",
            merchant_account_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ACCOUNT_ID"),
            merchant_config_currency="YOUR_MERCHANT_CONFIG_CURRENCY",
            apple_pay_supported_networks=["YOUR_APPLE_PAY_SUPPORTED_NETWORKS"],
            apple_pay_merchant_capabilities=["YOUR_APPLE_PAY_MERCHANT_CAPABILITIES"],
            apple_pay_label="YOUR_APPLE_PAY_LABEL",
            gpay_merchant_name="YOUR_GPAY_MERCHANT_NAME",
            gpay_merchant_id="YOUR_GPAY_MERCHANT_ID",
            gpay_allowed_auth_methods=["YOUR_GPAY_ALLOWED_AUTH_METHODS"],
            gpay_allowed_card_networks=["YOUR_GPAY_ALLOWED_CARD_NETWORKS"],
            paypal_client_id="YOUR_PAYPAL_CLIENT_ID",
            gpay_gateway_merchant_id="YOUR_GPAY_GATEWAY_MERCHANT_ID",
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
    connector: Connector.BRAINTREE,
    environment: Environment.SANDBOX,
    auth: {
        braintree: {
            publicKey: { value: 'YOUR_PUBLIC_KEY' },
            privateKey: { value: 'YOUR_PRIVATE_KEY' },
            baseUrl: 'YOUR_BASE_URL',
            merchantAccountId: { value: 'YOUR_MERCHANT_ACCOUNT_ID' },
            merchantConfigCurrency: 'YOUR_MERCHANT_CONFIG_CURRENCY',
            applePaySupportedNetworks: ['YOUR_APPLE_PAY_SUPPORTED_NETWORKS'],
            applePayMerchantCapabilities: ['YOUR_APPLE_PAY_MERCHANT_CAPABILITIES'],
            applePayLabel: 'YOUR_APPLE_PAY_LABEL',
            gpayMerchantName: 'YOUR_GPAY_MERCHANT_NAME',
            gpayMerchantId: 'YOUR_GPAY_MERCHANT_ID',
            gpayAllowedAuthMethods: ['YOUR_GPAY_ALLOWED_AUTH_METHODS'],
            gpayAllowedCardNetworks: ['YOUR_GPAY_ALLOWED_CARD_NETWORKS'],
            paypalClientId: 'YOUR_PAYPAL_CLIENT_ID',
            gpayGatewayMerchantId: 'YOUR_GPAY_GATEWAY_MERCHANT_ID',
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
            .setBraintree(BraintreeConfig.newBuilder()
                .setPublicKey(SecretString.newBuilder().setValue("YOUR_PUBLIC_KEY").build())
                .setPrivateKey(SecretString.newBuilder().setValue("YOUR_PRIVATE_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setMerchantAccountId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ACCOUNT_ID").build())
                .setMerchantConfigCurrency("YOUR_MERCHANT_CONFIG_CURRENCY")
                .addAllApplePaySupportedNetworks(listOf("YOUR_APPLE_PAY_SUPPORTED_NETWORKS"))
                .addAllApplePayMerchantCapabilities(listOf("YOUR_APPLE_PAY_MERCHANT_CAPABILITIES"))
                .setApplePayLabel("YOUR_APPLE_PAY_LABEL")
                .setGpayMerchantName("YOUR_GPAY_MERCHANT_NAME")
                .setGpayMerchantId("YOUR_GPAY_MERCHANT_ID")
                .addAllGpayAllowedAuthMethods(listOf("YOUR_GPAY_ALLOWED_AUTH_METHODS"))
                .addAllGpayAllowedCardNetworks(listOf("YOUR_GPAY_ALLOWED_CARD_NETWORKS"))
                .setPaypalClientId("YOUR_PAYPAL_CLIENT_ID")
                .setGpayGatewayMerchantId("YOUR_GPAY_GATEWAY_MERCHANT_ID")
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
            config: Some(connector_specific_config::Config::Braintree(BraintreeConfig {
                public_key: Some(hyperswitch_masking::Secret::new("YOUR_PUBLIC_KEY".to_string())),  // Authentication credential
                private_key: Some(hyperswitch_masking::Secret::new("YOUR_PRIVATE_KEY".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                merchant_account_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ACCOUNT_ID".to_string())),  // Authentication credential
                merchant_config_currency: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                apple_pay_supported_networks: vec!["value".to_string()],  // Array field
                apple_pay_merchant_capabilities: vec!["value".to_string()],  // Array field
                apple_pay_label: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                gpay_merchant_name: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                gpay_merchant_id: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                gpay_allowed_auth_methods: vec!["value".to_string()],  // Array field
                gpay_allowed_card_networks: vec!["value".to_string()],  // Array field
                paypal_client_id: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                gpay_gateway_merchant_id: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

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
| Bancontact | x |
| Apple Pay | x |
| Apple Pay Dec | x |
| Apple Pay SDK | ✓ |
| Google Pay | x |
| Google Pay Dec | x |
| Google Pay SDK | ✓ |
| PayPal SDK | ✓ |
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
| Interac | x |
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

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts) · [Kotlin](../../examples/braintree/braintree.kt) · [Rust](../../examples/braintree/braintree.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts#L125) · [Kotlin](../../examples/braintree/braintree.kt#L95) · [Rust](../../examples/braintree/braintree.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts#L143) · [Kotlin](../../examples/braintree/braintree.kt#L121) · [Rust](../../examples/braintree/braintree.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts#L152) · [Kotlin](../../examples/braintree/braintree.kt#L129) · [Rust](../../examples/braintree/braintree.rs)

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts#L170) · [Kotlin](../../examples/braintree/braintree.kt#L152) · [Rust](../../examples/braintree/braintree.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts) · [Kotlin](../../examples/braintree/braintree.kt#L178) · [Rust](../../examples/braintree/braintree.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts#L161) · [Kotlin](../../examples/braintree/braintree.kt#L139) · [Rust](../../examples/braintree/braintree.rs)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/braintree/braintree.py) · [TypeScript](../../examples/braintree/braintree.ts#L134) · [Kotlin](../../examples/braintree/braintree.kt#L105) · [Rust](../../examples/braintree/braintree.rs)
