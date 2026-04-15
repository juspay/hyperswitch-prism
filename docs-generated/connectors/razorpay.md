# Razorpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/razorpay.json
Regenerate: python3 scripts/generators/docs/generate.py razorpay
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
#     razorpay=payment_pb2.RazorpayConfig(api_key=...),
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
    connector: 'Razorpay',
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
    .setConnector("Razorpay")
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
    connector: "Razorpay".to_string(),
    environment: Environment::Sandbox,
    auth: ConnectorAuth::HeaderKey { api_key: "YOUR_API_KEY".into() },
    ..Default::default()
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
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
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
| Card | ? |
| Bancontact | ? |
| Apple Pay | ? |
| Apple Pay Dec | ? |
| Apple Pay SDK | ? |
| Google Pay | ? |
| Google Pay Dec | ? |
| Google Pay SDK | ? |
| PayPal SDK | ? |
| Amazon Pay | ? |
| Cash App | ? |
| PayPal | ? |
| WeChat Pay | ? |
| Alipay | ? |
| Revolut Pay | ? |
| MiFinity | ? |
| Bluecode | ? |
| Paze | x |
| Samsung Pay | ? |
| MB Way | ? |
| Satispay | ? |
| Wero | ? |
| Affirm | ? |
| Afterpay | ? |
| Klarna | ? |
| UPI Collect | ✓ |
| UPI Intent | ✓ |
| UPI QR | ✓ |
| Thailand | ? |
| Czech | ? |
| Finland | ? |
| FPX | ? |
| Poland | ? |
| Slovakia | ? |
| UK | ? |
| PIS | x |
| Generic | ? |
| Local | ? |
| iDEAL | ? |
| Sofort | ? |
| Trustly | ? |
| Giropay | ? |
| EPS | ? |
| Przelewy24 | ? |
| PSE | ? |
| BLIK | ? |
| Interac | ? |
| Bizum | ? |
| EFT | ? |
| DuitNow | x |
| ACH | ? |
| SEPA | ? |
| BACS | ? |
| Multibanco | ? |
| Instant | ? |
| Instant FI | ? |
| Instant PL | ? |
| Pix | ? |
| Permata | ? |
| BCA | ? |
| BNI VA | ? |
| BRI VA | ? |
| CIMB VA | ? |
| Danamon VA | ? |
| Mandiri VA | ? |
| Local | ? |
| Indonesian | ? |
| ACH | ? |
| SEPA | ? |
| BACS | ? |
| BECS | ? |
| SEPA Guaranteed | ? |
| Crypto | x |
| Reward | ? |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | ? |
| Boleto | ? |
| Efecty | ? |
| Pago Efectivo | ? |
| Red Compra | ? |
| Red Pagos | ? |
| Alfamart | ? |
| Indomaret | ? |
| Oxxo | ? |
| 7-Eleven | ? |
| Lawson | ? |
| Mini Stop | ? |
| Family Mart | ? |
| Seicomart | ? |
| Pay Easy | ? |

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### UPI Collect

```python
"payment_method": {
    "upi_collect": {  # UPI Collect.
        "vpa_id": {"value": "test@upi"}  # Virtual Payment Address.
    }
}
```

**Examples:** [Python](../../examples/razorpay/razorpay.py#L121) · [TypeScript](../../examples/razorpay/razorpay.ts#L106) · [Kotlin](../../examples/razorpay/razorpay.kt#L93) · [Rust](../../examples/razorpay/razorpay.rs#L112)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/razorpay/razorpay.py#L130) · [TypeScript](../../examples/razorpay/razorpay.ts#L115) · [Kotlin](../../examples/razorpay/razorpay.kt#L105) · [Rust](../../examples/razorpay/razorpay.rs#L124)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/razorpay/razorpay.py#L139) · [TypeScript](../../examples/razorpay/razorpay.ts#L124) · [Kotlin](../../examples/razorpay/razorpay.kt#L115) · [Rust](../../examples/razorpay/razorpay.rs#L131)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/razorpay/razorpay.py#L148) · [TypeScript](../../examples/razorpay/razorpay.ts#L133) · [Kotlin](../../examples/razorpay/razorpay.kt#L129) · [Rust](../../examples/razorpay/razorpay.rs#L138)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/razorpay/razorpay.py#L166) · [TypeScript](../../examples/razorpay/razorpay.ts#L151) · [Kotlin](../../examples/razorpay/razorpay.kt#L147) · [Rust](../../examples/razorpay/razorpay.rs#L152)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/razorpay/razorpay.py#L175) · [TypeScript](../../examples/razorpay/razorpay.ts#L160) · [Kotlin](../../examples/razorpay/razorpay.kt#L157) · [Rust](../../examples/razorpay/razorpay.rs#L159)
