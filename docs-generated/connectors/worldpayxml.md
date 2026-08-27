# Worldpayxml

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/worldpayxml.json
Regenerate: python3 scripts/generators/docs/generate.py worldpayxml
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
        worldpayxml=payment_pb2.WorldpayxmlConfig(
            api_username=payment_methods_pb2.SecretString(value="YOUR_API_USERNAME"),
            api_password=payment_methods_pb2.SecretString(value="YOUR_API_PASSWORD"),
            merchant_code=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_CODE"),
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
    connector: Connector.WORLDPAYXML,
    environment: Environment.SANDBOX,
    auth: {
        worldpayxml: {
            apiUsername: { value: 'YOUR_API_USERNAME' },
            apiPassword: { value: 'YOUR_API_PASSWORD' },
            merchantCode: { value: 'YOUR_MERCHANT_CODE' },
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
            .setWorldpayxml(WorldpayxmlConfig.newBuilder()
                .setApiUsername(SecretString.newBuilder().setValue("YOUR_API_USERNAME").build())
                .setApiPassword(SecretString.newBuilder().setValue("YOUR_API_PASSWORD").build())
                .setMerchantCode(SecretString.newBuilder().setValue("YOUR_MERCHANT_CODE").build())
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
            config: Some(connector_specific_config::Config::Worldpayxml(WorldpayxmlConfig {
                api_username: Some(hyperswitch_masking::Secret::new("YOUR_API_USERNAME".to_string())),  // Authentication credential
                api_password: Some(hyperswitch_masking::Secret::new("YOUR_API_PASSWORD".to_string())),  // Authentication credential
                merchant_code: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_CODE".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L213) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L124) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L270)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L232) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L140) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L286)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L257) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L162) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L309)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L282) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L184) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L332)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py#L304) · [JavaScript](../../examples/worldpayxml/worldpayxml.js) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L203) · [Rust](../../examples/worldpayxml/worldpayxml.rs#L351)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
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
| Apple Pay | ✓ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | x |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
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
| GoPay | x |
| GCash | x |
| Momo | x |
| Dana | x |
| Kakao Pay | x |
| Touch 'n Go | x |
| Twint | x |
| Vipps | x |
| Swish | x |
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
        "token": "{\"protocolVersion\":\"ECv2\",\"signature\":\"probe\",\"signedMessage\":\"probe\"}"
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

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L339) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L221) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L348) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L233) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L357) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L243) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L366) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L251) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L375) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L280) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L393) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L346) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L411) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L368) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L420) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L376) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L418) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L402) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L356) · [Rust](../../examples/worldpayxml/worldpayxml.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/worldpayxml/worldpayxml.py) · [TypeScript](../../examples/worldpayxml/worldpayxml.ts#L384) · [Kotlin](../../examples/worldpayxml/worldpayxml.kt#L315) · [Rust](../../examples/worldpayxml/worldpayxml.rs)
