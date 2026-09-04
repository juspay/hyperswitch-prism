# Worldpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/worldpay.json
Regenerate: python3 scripts/generators/docs/generate.py worldpay
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_USERNAME`, `YOUR_PASSWORD`, `YOUR_ENTITY_ID`, `YOUR_BASE_URL`, `YOUR_MERCHANT_NAME` with your actual values.

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
        worldpay=payment_pb2.WorldpayConfig(
            username=payment_methods_pb2.SecretString(value="YOUR_USERNAME"),
            password=payment_methods_pb2.SecretString(value="YOUR_PASSWORD"),
            entity_id=payment_methods_pb2.SecretString(value="YOUR_ENTITY_ID"),
            base_url="YOUR_BASE_URL",
            merchant_name=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_NAME"),
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
    connector: Connector.WORLDPAY,
    environment: Environment.SANDBOX,
    auth: {
        worldpay: {
            username: { value: 'YOUR_USERNAME' },
            password: { value: 'YOUR_PASSWORD' },
            entityId: { value: 'YOUR_ENTITY_ID' },
            baseUrl: 'YOUR_BASE_URL',
            merchantName: { value: 'YOUR_MERCHANT_NAME' },
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
            .setWorldpay(WorldpayConfig.newBuilder()
                .setUsername(SecretString.newBuilder().setValue("YOUR_USERNAME").build())
                .setPassword(SecretString.newBuilder().setValue("YOUR_PASSWORD").build())
                .setEntityId(SecretString.newBuilder().setValue("YOUR_ENTITY_ID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setMerchantName(SecretString.newBuilder().setValue("YOUR_MERCHANT_NAME").build())
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
            config: Some(connector_specific_config::Config::Worldpay(WorldpayConfig {
                username: Some(hyperswitch_masking::Secret::new("YOUR_USERNAME".to_string())),  // Authentication credential
                password: Some(hyperswitch_masking::Secret::new("YOUR_PASSWORD".to_string())),  // Authentication credential
                entity_id: Some(hyperswitch_masking::Secret::new("YOUR_ENTITY_ID".to_string())),  // Authentication credential
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL, e.g. https://sandbox.example.com
                merchant_name: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_NAME".to_string())),  // Authentication credential
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py#L157) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L165) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L117) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs#L197)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py#L176) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L184) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L133) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs#L213)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py#L201) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L210) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L155) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs#L236)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py#L226) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L236) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L177) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs#L259)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py#L248) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L258) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L196) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs#L278)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.IncrementalAuthorization](#paymentserviceincrementalauthorization) | Payments | `PaymentServiceIncrementalAuthorizationRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
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
| Card | ✓ |
| Bancontact | ⚠ |
| Apple Pay | ✓ |
| Apple Pay Dec | ? |
| Apple Pay SDK | x |
| Google Pay | ✓ |
| Google Pay Dec | ? |
| Google Pay SDK | x |
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L279) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L214) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L288) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L226) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L297) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L236) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

#### PaymentService.IncrementalAuthorization

Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceIncrementalAuthorizationRequest` |
| **Response** | `PaymentServiceIncrementalAuthorizationResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L306) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L244) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L315) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L260) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L333) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L320) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L342) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L342) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L330) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.ts#L324) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.kt#L289) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/worldpay/worldpay.rs)
