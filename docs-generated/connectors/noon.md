# Noon

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/noon.json
Regenerate: python3 scripts/generators/docs/generate.py noon
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_APPLICATION_IDENTIFIER`, `YOUR_BUSINESS_IDENTIFIER`, `YOUR_BASE_URL` with your actual values.

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
        noon=payment_pb2.NoonConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            application_identifier=payment_methods_pb2.SecretString(value="YOUR_APPLICATION_IDENTIFIER"),
            business_identifier=payment_methods_pb2.SecretString(value="YOUR_BUSINESS_IDENTIFIER"),
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
    connector: Connector.NOON,
    environment: Environment.SANDBOX,
    auth: {
        noon: {
            apiKey: { value: 'YOUR_API_KEY' },
            applicationIdentifier: { value: 'YOUR_APPLICATION_IDENTIFIER' },
            businessIdentifier: { value: 'YOUR_BUSINESS_IDENTIFIER' },
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
            .setNoon(NoonConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setApplicationIdentifier(SecretString.newBuilder().setValue("YOUR_APPLICATION_IDENTIFIER").build())
                .setBusinessIdentifier(SecretString.newBuilder().setValue("YOUR_BUSINESS_IDENTIFIER").build())
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
            config: Some(connector_specific_config::Config::Noon(NoonConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                application_identifier: Some(hyperswitch_masking::Secret::new("YOUR_APPLICATION_IDENTIFIER".to_string())),  // Authentication credential
                business_identifier: Some(hyperswitch_masking::Secret::new("YOUR_BUSINESS_IDENTIFIER".to_string())),  // Authentication credential
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py#L195) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L219) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L122) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs#L257)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py#L214) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L238) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L138) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs#L273)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py#L239) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L264) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L160) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs#L296)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py#L264) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L290) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L182) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs#L319)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py#L286) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L312) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L201) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs#L338)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [RecurringPaymentService.Revoke](#recurringpaymentservicerevoke) | Mandates | `RecurringPaymentServiceRevokeRequest` |
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
| Bancontact | x |
| Apple Pay | ✓ |
| Apple Pay Dec | ? |
| Apple Pay SDK | x |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
| Google Pay SDK | x |
| PayPal SDK | x |
| Amazon Pay | x |
| Cash App | x |
| PayPal | ✓ |
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L333) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L219) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L342) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L231) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L351) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L241) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L378) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L280) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L387) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L311) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L414) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L388) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L410) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L423) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L398) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L396) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L344) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)

#### RecurringPaymentService.Revoke

Cancel an existing recurring payment mandate. Stops future automatic charges on customer's stored consent for subscription cancellations.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceRevokeRequest` |
| **Response** | `RecurringPaymentServiceRevokeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.ts#L405) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.kt#L376) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/noon/noon.rs)
