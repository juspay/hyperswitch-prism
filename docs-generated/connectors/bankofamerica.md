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
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     bankofamerica=payment_pb2.BankofamericaConfig(api_key=...),
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
    connector: 'Bankofamerica',
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
    .setConnector("Bankofamerica")
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
    connector: "Bankofamerica".to_string(),
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

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L276) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L117) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L263)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L295) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L133) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L279)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L320) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L155) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L302)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L345) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L177) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L325)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L367) · [JavaScript](../../examples/bankofamerica/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L196) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L344)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
| [PaymentService.TokenSetupRecurring](#paymentservicetokensetuprecurring) | Payments | `PaymentServiceTokenSetupRecurringRequest` |
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

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L389) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L366) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L214) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L362)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L398) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L375) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L226) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L374)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L416) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L393) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L252) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L388)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L425) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L402) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L260) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L395)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L434) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L411) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L289) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L402)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L443) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L420) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L320) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L409)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L461) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L438) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L342) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L423)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L470) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L447) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L381) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L433)

#### PaymentService.TokenSetupRecurring

Setup a recurring mandate using a connector token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L479) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L456) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L403) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L440)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L488) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L439) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L447)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L452) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L429) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L330) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L416)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/bankofamerica/bankofamerica.py#L407) · [TypeScript](../../examples/bankofamerica/bankofamerica.ts#L384) · [Kotlin](../../examples/bankofamerica/bankofamerica.kt#L236) · [Rust](../../examples/bankofamerica/bankofamerica.rs#L381)
