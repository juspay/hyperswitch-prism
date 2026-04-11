# Globalpay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/globalpay.json
Regenerate: python3 scripts/generators/docs/generate.py globalpay
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
#     globalpay=payment_pb2.GlobalpayConfig(api_key=...),
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
    connector: 'Globalpay',
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
    .setConnector("Globalpay")
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
    connector: "Globalpay".to_string(),
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

**Examples:** [Python](../../examples/globalpay/globalpay.py#L233) · [JavaScript](../../examples/globalpay/globalpay.js) · [Kotlin](../../examples/globalpay/globalpay.kt#L137) · [Rust](../../examples/globalpay/globalpay.rs#L224)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L252) · [JavaScript](../../examples/globalpay/globalpay.js) · [Kotlin](../../examples/globalpay/globalpay.kt#L153) · [Rust](../../examples/globalpay/globalpay.rs#L240)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/globalpay/globalpay.py#L277) · [JavaScript](../../examples/globalpay/globalpay.js) · [Kotlin](../../examples/globalpay/globalpay.kt#L175) · [Rust](../../examples/globalpay/globalpay.rs#L263)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/globalpay/globalpay.py#L302) · [JavaScript](../../examples/globalpay/globalpay.js) · [Kotlin](../../examples/globalpay/globalpay.kt#L197) · [Rust](../../examples/globalpay/globalpay.rs#L286)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/globalpay/globalpay.py#L324) · [JavaScript](../../examples/globalpay/globalpay.js) · [Kotlin](../../examples/globalpay/globalpay.kt#L216) · [Rust](../../examples/globalpay/globalpay.rs#L305)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [FraudService.Get](#fraudserviceget) | Other | `FraudServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
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
| Apple Pay | ⚠ |
| Apple Pay Dec | ⚠ |
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
| Sofort | ⚠ |
| Trustly | ⚠ |
| Giropay | ⚠ |
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
| SEPA | ⚠ |
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
    "card": {  # Generic card payment.
        "card_number": {"value": "4111111111111111"},  # Card Identification.
        "card_exp_month": {"value": "03"},
        "card_exp_year": {"value": "2030"},
        "card_cvc": {"value": "737"},
        "card_holder_name": {"value": "John Doe"}  # Cardholder Information.
    }
}
```

##### iDEAL

```python
"payment_method": {
    "ideal": {
    }
}
```

**Examples:** [Python](../../examples/globalpay/globalpay.py#L347) · [TypeScript](../../examples/globalpay/globalpay.ts#L327) · [Kotlin](../../examples/globalpay/globalpay.kt#L235) · [Rust](../../examples/globalpay/globalpay.rs#L323)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L356) · [TypeScript](../../examples/globalpay/globalpay.ts#L336) · [Kotlin](../../examples/globalpay/globalpay.kt#L247) · [Rust](../../examples/globalpay/globalpay.rs#L335)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L392) · [TypeScript](../../examples/globalpay/globalpay.ts#L372) · [Kotlin](../../examples/globalpay/globalpay.kt#L291) · [Rust](../../examples/globalpay/globalpay.rs#L363)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L401) · [TypeScript](../../examples/globalpay/globalpay.ts#L381) · [Kotlin](../../examples/globalpay/globalpay.kt#L326) · [Rust](../../examples/globalpay/globalpay.rs#L370)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L419) · [TypeScript](../../examples/globalpay/globalpay.ts#L399) · [Kotlin](../../examples/globalpay/globalpay.kt#L355) · [Rust](../../examples/globalpay/globalpay.rs#L384)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L428) · [TypeScript](../../examples/globalpay/globalpay.ts) · [Kotlin](../../examples/globalpay/globalpay.kt#L383) · [Rust](../../examples/globalpay/globalpay.rs#L391)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L410) · [TypeScript](../../examples/globalpay/globalpay.ts#L390) · [Kotlin](../../examples/globalpay/globalpay.kt#L336) · [Rust](../../examples/globalpay/globalpay.rs#L377)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L365) · [TypeScript](../../examples/globalpay/globalpay.ts#L345) · [Kotlin](../../examples/globalpay/globalpay.kt#L257) · [Rust](../../examples/globalpay/globalpay.rs#L342)

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L374) · [TypeScript](../../examples/globalpay/globalpay.ts#L354) · [Kotlin](../../examples/globalpay/globalpay.kt#L273) · [Rust](../../examples/globalpay/globalpay.rs#L349)

### Other

#### FraudService.Get

Retrieves fraud decision history and risk scores for a specific transaction. Supports customer service investigations and chargeback dispute preparation.

| | Message |
|---|---------|
| **Request** | `FraudServiceGetRequest` |
| **Response** | `FraudServiceGetResponse` |

**Examples:** [Python](../../examples/globalpay/globalpay.py#L383) · [TypeScript](../../examples/globalpay/globalpay.ts#L363) · [Kotlin](../../examples/globalpay/globalpay.kt#L283) · [Rust](../../examples/globalpay/globalpay.rs#L356)
