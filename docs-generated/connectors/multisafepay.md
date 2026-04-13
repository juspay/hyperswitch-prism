# Multisafepay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/multisafepay.json
Regenerate: python3 scripts/generators/docs/generate.py multisafepay
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
        multisafepay=payment_pb2.MultisafepayConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
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
    connector: Connector.MULTISAFEPAY,
    environment: Environment.SANDBOX,
    auth: {
        multisafepay: {
            apiKey: { value: 'YOUR_API_KEY' },
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
            .setMultisafepay(MultisafepayConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
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
            config: Some(connector_specific_config::Config::Multisafepay(MultisafepayConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/multisafepay/multisafepay.py#L123) · [JavaScript](../../examples/multisafepay/multisafepay.js) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L97) · [Rust](../../examples/multisafepay/multisafepay.rs#L154)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/multisafepay/multisafepay.py#L142) · [JavaScript](../../examples/multisafepay/multisafepay.js) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L113) · [Rust](../../examples/multisafepay/multisafepay.rs#L170)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/multisafepay/multisafepay.py#L167) · [JavaScript](../../examples/multisafepay/multisafepay.js) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L135) · [Rust](../../examples/multisafepay/multisafepay.rs#L193)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
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
| Apple Pay | ⚠ |
| Apple Pay Dec | ⚠ |
| Apple Pay SDK | ⚠ |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
| Google Pay SDK | ⚠ |
| PayPal SDK | ⚠ |
| Amazon Pay | ⚠ |
| Cash App | ⚠ |
| PayPal | ✓ |
| WeChat Pay | ⚠ |
| Alipay | ✓ |
| Revolut Pay | ⚠ |
| MiFinity | ⚠ |
| Bluecode | ⚠ |
| Paze | x |
| Samsung Pay | ⚠ |
| MB Way | ⚠ |
| Satispay | ⚠ |
| Wero | ⚠ |
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
| Generic | ⚠ |
| Local | ⚠ |
| iDEAL | ✓ |
| Sofort | ✓ |
| Trustly | ✓ |
| Giropay | ✓ |
| EPS | ✓ |
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
| SEPA | ✓ |
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

##### Google Pay

```python
"payment_method": {
  "google_pay": {
    "type": "CARD",
    "description": "Visa 1111",
    "info": {
      "card_network": "VISA",
      "card_details": "1111"
    },
    "tokenization_data": {
      "encrypted_data": {
        "token_type": "PAYMENT_GATEWAY",
        "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"
      }
    }
  }
}
```

##### SEPA Direct Debit

```python
"payment_method": {
  "sepa": {
    "iban": "DE89370400440532013000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### iDEAL

```python
"payment_method": {
  "ideal": {}
}
```

##### PayPal Redirect

```python
"payment_method": {
  "paypal_redirect": {
    "email": "test@example.com"
  }
}
```

**Examples:** [Python](../../examples/multisafepay/multisafepay.py) · [TypeScript](../../examples/multisafepay/multisafepay.ts#L197) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L153) · [Rust](../../examples/multisafepay/multisafepay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/multisafepay/multisafepay.py) · [TypeScript](../../examples/multisafepay/multisafepay.ts#L215) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L181) · [Rust](../../examples/multisafepay/multisafepay.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/multisafepay/multisafepay.py) · [TypeScript](../../examples/multisafepay/multisafepay.ts#L224) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L189) · [Rust](../../examples/multisafepay/multisafepay.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/multisafepay/multisafepay.py) · [TypeScript](../../examples/multisafepay/multisafepay.ts#L233) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L221) · [Rust](../../examples/multisafepay/multisafepay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/multisafepay/multisafepay.py) · [TypeScript](../../examples/multisafepay/multisafepay.ts#L242) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L231) · [Rust](../../examples/multisafepay/multisafepay.rs)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/multisafepay/multisafepay.py) · [TypeScript](../../examples/multisafepay/multisafepay.ts#L206) · [Kotlin](../../examples/multisafepay/multisafepay.kt#L165) · [Rust](../../examples/multisafepay/multisafepay.rs)
