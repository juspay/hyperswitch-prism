# Bankofamerica

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/bankofamerica.json
Regenerate: python3 scripts/generators/docs/generate.py bankofamerica
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
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     bankofamerica=payment_pb2.BankofamericaConfig(api_key=...),
    # ),
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
    connector: Connector.BANKOFAMERICA,
    environment: Environment.SANDBOX,
    // auth: { bankofamerica: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Bankofamerica credentials here
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
    connector_config: None,  // TODO: Add your connector config here,
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

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L188) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L117) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L236)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L207) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L133) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L252)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L232) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L155) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L275)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L257) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L177) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L298)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L279) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L196) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L317)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
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
| Bancontact | ⚠ |
| Apple Pay | ✓ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | ⚠ |
| Google Pay | ⚠ |
| Google Pay Dec | ⚠ |
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

##### Apple Pay

```python
"payment_method": {
  "apple_pay_sdk": {
    "payment_data": {
      "encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"
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

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L313) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L214) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L322) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L226) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L331) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L236) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L340) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L244) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L349) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L274) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L358) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L306) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L376) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L328) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L385) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L336) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L375) · [Rust](../../examples/bankofamerica/bankofamerica.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L367) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L316) · [Rust](../../examples/bankofamerica/bankofamerica.rs)
