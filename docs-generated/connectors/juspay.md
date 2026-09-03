# Juspay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/juspay.json
Regenerate: python3 scripts/generators/docs/generate.py juspay
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_MERCHANT_ID`, `YOUR_JUSPAY_ENCRYPTION_PUBLIC_KEY`, `YOUR_RESPONSE_DECRYPTION_PRIVATE_KEY`, `YOUR_CARD_SYNC_KEY_ID`, `YOUR_BASE_URL` with your actual values.

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
        juspay=payment_pb2.JuspayConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            juspay_encryption_public_key=payment_methods_pb2.SecretString(value="YOUR_JUSPAY_ENCRYPTION_PUBLIC_KEY"),
            response_decryption_private_key=payment_methods_pb2.SecretString(value="YOUR_RESPONSE_DECRYPTION_PRIVATE_KEY"),
            card_sync_key_id=payment_methods_pb2.SecretString(value="YOUR_CARD_SYNC_KEY_ID"),
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
    connector: Connector.JUSPAY,
    environment: Environment.SANDBOX,
    auth: {
        juspay: {
            apiKey: { value: 'YOUR_API_KEY' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            juspayEncryptionPublicKey: { value: 'YOUR_JUSPAY_ENCRYPTION_PUBLIC_KEY' },
            responseDecryptionPrivateKey: { value: 'YOUR_RESPONSE_DECRYPTION_PRIVATE_KEY' },
            cardSyncKeyId: { value: 'YOUR_CARD_SYNC_KEY_ID' },
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
            .setJuspay(JuspayConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setJuspayEncryptionPublicKey(SecretString.newBuilder().setValue("YOUR_JUSPAY_ENCRYPTION_PUBLIC_KEY").build())
                .setResponseDecryptionPrivateKey(SecretString.newBuilder().setValue("YOUR_RESPONSE_DECRYPTION_PRIVATE_KEY").build())
                .setCardSyncKeyId(SecretString.newBuilder().setValue("YOUR_CARD_SYNC_KEY_ID").build())
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
            config: Some(connector_specific_config::Config::Juspay(JuspayConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                juspay_encryption_public_key: Some(hyperswitch_masking::Secret::new("YOUR_JUSPAY_ENCRYPTION_PUBLIC_KEY".to_string())),  // Authentication credential
                response_decryption_private_key: Some(hyperswitch_masking::Secret::new("YOUR_RESPONSE_DECRYPTION_PRIVATE_KEY".to_string())),  // Authentication credential
                card_sync_key_id: Some(hyperswitch_masking::Secret::new("YOUR_CARD_SYNC_KEY_ID".to_string())),  // Authentication credential
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py#L140) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L145) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L123) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs#L175)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py#L159) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L164) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L139) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs#L191)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py#L184) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L190) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L161) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs#L214)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py#L206) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L212) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L180) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs#L233)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
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
| Bancontact | ⚠ |
| Apple Pay | ✓ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | ✓ |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
| Google Pay SDK | ✓ |
| PayPal SDK | ✓ |
| Amazon Pay | ✓ |
| Cash App | ⚠ |
| PayPal | ✓ |
| WeChat Pay | ✓ |
| Alipay | ✓ |
| Revolut Pay | ⚠ |
| MiFinity | ⚠ |
| Bluecode | ⚠ |
| Paze | x |
| Samsung Pay | ✓ |
| MB Way | ⚠ |
| Satispay | ⚠ |
| Wero | ⚠ |
| GoPay | ✓ |
| GCash | ✓ |
| Momo | ✓ |
| Dana | ⚠ |
| Kakao Pay | ✓ |
| Touch 'n Go | ✓ |
| Twint | ⚠ |
| Vipps | ⚠ |
| Swish | ⚠ |
| Affirm | ⚠ |
| Afterpay | ⚠ |
| Klarna | ⚠ |
| UPI Collect | ✓ |
| UPI Intent | ✓ |
| UPI QR | ✓ |
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
| PaySafeCard | ⚠ |
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

##### Google Pay

```python
"payment_method": {
  "google_pay_sdk": {
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

##### Apple Pay

```python
"payment_method": {
  "apple_pay_sdk": {
    "payment_data": {
      "encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSIsImhlYWRlciI6eyJlcGhlbWVyYWxQdWJsaWNLZXkiOiJwcm9iZSIsInB1YmxpY0tleUhhc2giOiJwcm9iZSIsInRyYW5zYWN0aW9uSWQiOiJwcm9iZV90eG5faWQifX0="
    },
    "payment_method": {
      "display_name": "Visa 1111",
      "network": "Visa",
      "type": "debit"
    },
    "transaction_identifier": "probe_txn_id"
  }
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

##### UPI Collect

```python
"payment_method": {
  "upi_collect": {
    "vpa_id": "test@upi"
  }
}
```

##### Samsung Pay

```python
"payment_method": {
  "samsung_pay_sdk": {
    "payment_credential": {
      "method": "3DS",
      "recurring_payment": false,
      "card_brand": "VISA",
      "card_last_four_digits": "1234",
      "token_data": {
        "type": "S",
        "version": "100",
        "data": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6InNhbXN1bmdfcHJvYmVfa2V5XzEyMyJ9.eyJwYXltZW50TWV0aG9kVG9rZW4iOiJwcm9iZV9zYW1zdW5nX3Rva2VuIn0.ZHVtbXlfc2lnbmF0dXJl"
      }
    }
  }
}
```

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L233) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L198) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L242) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L210) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L251) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L220) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L260) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L234) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L269) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L242) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L278) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L271) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L293) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.ts#L287) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.kt#L281) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/juspay/juspay.rs)
