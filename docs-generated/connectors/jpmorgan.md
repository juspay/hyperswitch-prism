# Jpmorgan

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/jpmorgan.json
Regenerate: python3 scripts/generators/docs/generate.py jpmorgan
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
        jpmorgan=payment_pb2.JpmorganConfig(
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            client_secret=payment_methods_pb2.SecretString(value="YOUR_CLIENT_SECRET"),
            base_url="YOUR_BASE_URL",
            company_name=payment_methods_pb2.SecretString(value="YOUR_COMPANY_NAME"),
            product_name=payment_methods_pb2.SecretString(value="YOUR_PRODUCT_NAME"),
            merchant_purchase_description=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_PURCHASE_DESCRIPTION"),
            statement_descriptor=payment_methods_pb2.SecretString(value="YOUR_STATEMENT_DESCRIPTOR"),
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
    connector: Connector.JPMORGAN,
    environment: Environment.SANDBOX,
    auth: {
        jpmorgan: {
            clientId: { value: 'YOUR_CLIENT_ID' },
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            baseUrl: 'YOUR_BASE_URL',
            companyName: { value: 'YOUR_COMPANY_NAME' },
            productName: { value: 'YOUR_PRODUCT_NAME' },
            merchantPurchaseDescription: { value: 'YOUR_MERCHANT_PURCHASE_DESCRIPTION' },
            statementDescriptor: { value: 'YOUR_STATEMENT_DESCRIPTOR' },
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
            .setJpmorgan(JpmorganConfig.newBuilder()
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setCompanyName(SecretString.newBuilder().setValue("YOUR_COMPANY_NAME").build())
                .setProductName(SecretString.newBuilder().setValue("YOUR_PRODUCT_NAME").build())
                .setMerchantPurchaseDescription(SecretString.newBuilder().setValue("YOUR_MERCHANT_PURCHASE_DESCRIPTION").build())
                .setStatementDescriptor(SecretString.newBuilder().setValue("YOUR_STATEMENT_DESCRIPTOR").build())
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
            config: Some(connector_specific_config::Config::Jpmorgan(JpmorganConfig {
                client_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_ID".to_string())),  // Authentication credential
                client_secret: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_SECRET".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                company_name: Some(hyperswitch_masking::Secret::new("YOUR_COMPANY_NAME".to_string())),  // Authentication credential
                product_name: Some(hyperswitch_masking::Secret::new("YOUR_PRODUCT_NAME".to_string())),  // Authentication credential
                merchant_purchase_description: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_PURCHASE_DESCRIPTION".to_string())),  // Authentication credential
                statement_descriptor: Some(hyperswitch_masking::Secret::new("YOUR_STATEMENT_DESCRIPTOR".to_string())),  // Authentication credential
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

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

### One-step Payment (Authorize + Capture)

Simple payment that authorizes and captures in one call. Use for immediate charges.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Payment authorized and captured — funds will be settled automatically |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py#L183) · [JavaScript](../../examples/jpmorgan/jpmorgan.js) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L152) · [Rust](../../examples/jpmorgan/jpmorgan.rs#L224)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py#L202) · [JavaScript](../../examples/jpmorgan/jpmorgan.js) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L168) · [Rust](../../examples/jpmorgan/jpmorgan.rs#L240)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py#L227) · [JavaScript](../../examples/jpmorgan/jpmorgan.js) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L190) · [Rust](../../examples/jpmorgan/jpmorgan.rs#L263)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py#L252) · [JavaScript](../../examples/jpmorgan/jpmorgan.js) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L212) · [Rust](../../examples/jpmorgan/jpmorgan.rs#L286)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py#L274) · [JavaScript](../../examples/jpmorgan/jpmorgan.js) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L231) · [Rust](../../examples/jpmorgan/jpmorgan.rs#L305)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
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
| Card | ✓ |
| Bancontact | x |
| Apple Pay | ⚠ |
| Apple Pay Dec | ⚠ |
| Apple Pay SDK | ⚠ |
| Google Pay | ? |
| Google Pay Dec | x |
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
| ACH | ✓ |
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

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### Card (Raw PAN)

```python
"payment_method": {
  "card": {
    "card_number": "4111111111111111",
    "card_exp_month": "03",
    "card_exp_year": "2030",
    "card_cvc": "737",
    "card_holder_name": "John Doe"
  }
}
```

##### ACH Direct Debit

```python
"payment_method": {
  "ach": {
    "account_number": "000123456789",
    "routing_number": "110000000",
    "bank_account_holder_name": "John Doe"
  }
}
```

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L308) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L249) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L317) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L261) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L344) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L297) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L353) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L305) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L371) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L334) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L362) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L362) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L315) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L326) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L271) · [Rust](../../examples/jpmorgan/jpmorgan.rs)

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/jpmorgan/jpmorgan.py) · [TypeScript](../../examples/jpmorgan/jpmorgan.ts#L335) · [Kotlin](../../examples/jpmorgan/jpmorgan.kt#L287) · [Rust](../../examples/jpmorgan/jpmorgan.rs)
