# TrustPay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/trustpay.json
Regenerate: python3 scripts/generators/docs/generate.py trustpay
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
        trustpay=payment_pb2.TrustpayConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            project_id=payment_methods_pb2.SecretString(value="YOUR_PROJECT_ID"),
            secret_key=payment_methods_pb2.SecretString(value="YOUR_SECRET_KEY"),
            base_url="YOUR_BASE_URL",
            base_url_bank_redirects="YOUR_BASE_URL_BANK_REDIRECTS",
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
    connector: Connector.TRUSTPAY,
    environment: Environment.SANDBOX,
    auth: {
        trustpay: {
            apiKey: { value: 'YOUR_API_KEY' },
            projectId: { value: 'YOUR_PROJECT_ID' },
            secretKey: { value: 'YOUR_SECRET_KEY' },
            baseUrl: 'YOUR_BASE_URL',
            baseUrlBankRedirects: 'YOUR_BASE_URL_BANK_REDIRECTS',
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
            .setTrustpay(TrustpayConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setProjectId(SecretString.newBuilder().setValue("YOUR_PROJECT_ID").build())
                .setSecretKey(SecretString.newBuilder().setValue("YOUR_SECRET_KEY").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setBaseUrlBankRedirects("YOUR_BASE_URL_BANK_REDIRECTS")
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
            config: Some(connector_specific_config::Config::Trustpay(TrustpayConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                project_id: Some(hyperswitch_masking::Secret::new("YOUR_PROJECT_ID".to_string())),  // Authentication credential
                secret_key: Some(hyperswitch_masking::Secret::new("YOUR_SECRET_KEY".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                base_url_bank_redirects: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
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

**Examples:** [Python](../../examples/trustpay/trustpay.py#L219) · [JavaScript](../../examples/trustpay/trustpay.js) · [Kotlin](../../examples/trustpay/trustpay.kt#L133) · [Rust](../../examples/trustpay/trustpay.rs#L271)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/trustpay/trustpay.py#L238) · [JavaScript](../../examples/trustpay/trustpay.js) · [Kotlin](../../examples/trustpay/trustpay.kt#L149) · [Rust](../../examples/trustpay/trustpay.rs#L287)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/trustpay/trustpay.py#L263) · [JavaScript](../../examples/trustpay/trustpay.js) · [Kotlin](../../examples/trustpay/trustpay.kt#L171) · [Rust](../../examples/trustpay/trustpay.rs#L310)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
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
| Card | ✓ |
| Bancontact | ⚠ |
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
| Thailand | ⚠ |
| Czech | ⚠ |
| Finland | ⚠ |
| FPX | ⚠ |
| Poland | ⚠ |
| Slovakia | ⚠ |
| UK | ⚠ |
| PIS | x |
| Generic | ⚠ |
| Local | ⚠ |
| iDEAL | ✓ |
| Sofort | ✓ |
| Trustly | ⚠ |
| Giropay | ✓ |
| EPS | ✓ |
| Przelewy24 | ⚠ |
| PSE | x |
| BLIK | ✓ |
| Interac | ⚠ |
| Bizum | ⚠ |
| EFT | ⚠ |
| DuitNow | x |
| ACH | x |
| SEPA | ✓ |
| BACS | x |
| Multibanco | x |
| Instant | ✓ |
| Instant FI | ✓ |
| Instant PL | ✓ |
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

##### iDEAL

```python
"payment_method": {
  "ideal": {}
}
```

##### BLIK

```python
"payment_method": {
  "blik": {
    "blik_code": "777124"
  }
}
```

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L293) · [Kotlin](../../examples/trustpay/trustpay.kt#L189) · [Rust](../../examples/trustpay/trustpay.rs)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L302) · [Kotlin](../../examples/trustpay/trustpay.kt#L201) · [Rust](../../examples/trustpay/trustpay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L320) · [Kotlin](../../examples/trustpay/trustpay.kt#L232) · [Rust](../../examples/trustpay/trustpay.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L338) · [Kotlin](../../examples/trustpay/trustpay.kt#L250) · [Rust](../../examples/trustpay/trustpay.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L356) · [Kotlin](../../examples/trustpay/trustpay.kt#L335) · [Rust](../../examples/trustpay/trustpay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L365) · [Kotlin](../../examples/trustpay/trustpay.kt#L345) · [Rust](../../examples/trustpay/trustpay.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L347) · [Kotlin](../../examples/trustpay/trustpay.kt#L297) · [Rust](../../examples/trustpay/trustpay.rs)

### Authentication

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/trustpay/trustpay.py) · [TypeScript](../../examples/trustpay/trustpay.ts#L311) · [Kotlin](../../examples/trustpay/trustpay.kt#L222) · [Rust](../../examples/trustpay/trustpay.rs)
