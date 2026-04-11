# Nuvei

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/nuvei.json
Regenerate: python3 scripts/generators/docs/generate.py nuvei
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
#     nuvei=payment_pb2.NuveiConfig(api_key=...),
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
    connector: 'Nuvei',
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
    .setConnector("Nuvei")
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
    connector: "Nuvei".to_string(),
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

**Examples:** [Python](../../examples/nuvei/nuvei.py#L172) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L128) · [Rust](../../examples/nuvei/nuvei.rs#L166)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L191) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L144) · [Rust](../../examples/nuvei/nuvei.rs#L182)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/nuvei/nuvei.py#L216) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L166) · [Rust](../../examples/nuvei/nuvei.rs#L205)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/nuvei/nuvei.py#L241) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L188) · [Rust](../../examples/nuvei/nuvei.rs#L228)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/nuvei/nuvei.py#L263) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L207) · [Rust](../../examples/nuvei/nuvei.rs#L247)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [MerchantAuthenticationService.CreateServerSessionAuthenticationToken](#merchantauthenticationservicecreateserversessionauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest` |
| [FraudService.Get](#fraudserviceget) | Other | `FraudServiceGetRequest` |
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
| ACH | ? |
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

**Examples:** [Python](../../examples/nuvei/nuvei.py#L286) · [TypeScript](../../examples/nuvei/nuvei.ts#L268) · [Kotlin](../../examples/nuvei/nuvei.kt#L226) · [Rust](../../examples/nuvei/nuvei.rs#L265)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L295) · [TypeScript](../../examples/nuvei/nuvei.ts#L277) · [Kotlin](../../examples/nuvei/nuvei.kt#L238) · [Rust](../../examples/nuvei/nuvei.rs#L277)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L313) · [TypeScript](../../examples/nuvei/nuvei.ts#L295) · [Kotlin](../../examples/nuvei/nuvei.kt#L264) · [Rust](../../examples/nuvei/nuvei.rs#L291)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L340) · [TypeScript](../../examples/nuvei/nuvei.ts#L322) · [Kotlin](../../examples/nuvei/nuvei.kt#L301) · [Rust](../../examples/nuvei/nuvei.rs#L312)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L358) · [TypeScript](../../examples/nuvei/nuvei.ts) · [Kotlin](../../examples/nuvei/nuvei.kt#L323) · [Rust](../../examples/nuvei/nuvei.rs#L326)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L349) · [TypeScript](../../examples/nuvei/nuvei.ts#L331) · [Kotlin](../../examples/nuvei/nuvei.kt#L311) · [Rust](../../examples/nuvei/nuvei.rs#L319)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L304) · [TypeScript](../../examples/nuvei/nuvei.ts#L286) · [Kotlin](../../examples/nuvei/nuvei.kt#L248) · [Rust](../../examples/nuvei/nuvei.rs#L284)

#### MerchantAuthenticationService.CreateServerSessionAuthenticationToken

Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L322) · [TypeScript](../../examples/nuvei/nuvei.ts#L304) · [Kotlin](../../examples/nuvei/nuvei.kt#L278) · [Rust](../../examples/nuvei/nuvei.rs#L298)

### Other

#### FraudService.Get

Retrieves fraud decision history and risk scores for a specific transaction. Supports customer service investigations and chargeback dispute preparation.

| | Message |
|---|---------|
| **Request** | `FraudServiceGetRequest` |
| **Response** | `FraudServiceGetResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L331) · [TypeScript](../../examples/nuvei/nuvei.ts#L313) · [Kotlin](../../examples/nuvei/nuvei.kt#L293) · [Rust](../../examples/nuvei/nuvei.rs#L305)
