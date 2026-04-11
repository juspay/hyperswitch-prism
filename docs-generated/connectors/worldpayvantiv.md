# Worldpayvantiv

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/worldpayvantiv.json
Regenerate: python3 scripts/generators/docs/generate.py worldpayvantiv
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
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     worldpayvantiv=payment_pb2.WorldpayvantivConfig(api_key=...),
# ))

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { ConnectorClient } = require('connector-service-node-ffi');

// Reuse this client for all flows
const client = new ConnectorClient({
    connector: 'Worldpayvantiv',
    environment: 'sandbox',
    connector_auth_type: {
        header_key: { api_key: 'YOUR_API_KEY' },
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setConnector("Worldpayvantiv")
    .setEnvironment(Environment.SANDBOX)
    .setAuth(
        ConnectorAuthType.newBuilder()
            .setHeaderKey(HeaderKey.newBuilder().setApiKey("YOUR_API_KEY"))
    )
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use connector_service_sdk::{ConnectorClient, ConnectorConfig};

let config = ConnectorConfig {
    connector: "Worldpayvantiv".to_string(),
    environment: Environment::Sandbox,
    auth: ConnectorAuth::HeaderKey { api_key: "YOUR_API_KEY".into() },
    ..Default::default()
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

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L149) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L103) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L138)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L168) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L119) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L154)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L193) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L141) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L177)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L218) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L163) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L200)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L240) · [JavaScript](../../examples/worldpayvantiv/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L182) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L219)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [FraudService.Get](#fraudserviceget) | Other | `FraudServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
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
    "card": {  # Generic card payment.
        "card_number": {"value": "4111111111111111"},  # Card Identification.
        "card_exp_month": {"value": "03"},
        "card_exp_year": {"value": "2030"},
        "card_cvc": {"value": "737"},
        "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
    }
}
```

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L263) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L248) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L201) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L237)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L272) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L257) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L213) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L249)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L290) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L275) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L231) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L263)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L299) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L284) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L259) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L270)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L317) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L302) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L281) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L284)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L326) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L292) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L291)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L308) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L293) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L269) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L277)

### Other

#### FraudService.Get

Retrieves fraud decision history and risk scores for a specific transaction. Supports customer service investigations and chargeback dispute preparation.

| | Message |
|---|---------|
| **Request** | `FraudServiceGetRequest` |
| **Response** | `FraudServiceGetResponse` |

**Examples:** [Python](../../examples/worldpayvantiv/worldpayvantiv.py#L281) · [TypeScript](../../examples/worldpayvantiv/worldpayvantiv.ts#L266) · [Kotlin](../../examples/worldpayvantiv/worldpayvantiv.kt#L223) · [Rust](../../examples/worldpayvantiv/worldpayvantiv.rs#L256)
