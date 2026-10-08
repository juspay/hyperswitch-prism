# Globalpayments Realex

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/globalpayments_realex.json
Regenerate: python3 scripts/generators/docs/generate.py globalpayments_realex
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

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
        globalpayments_realex=payment_pb2.GlobalpaymentsRealexConfig(
            shared_secret=payment_methods_pb2.SecretString(value="YOUR_SHARED_SECRET"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            account=payment_methods_pb2.SecretString(value="YOUR_ACCOUNT"),
            refund_password=payment_methods_pb2.SecretString(value="YOUR_REFUND_PASSWORD"),
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
    connector: Connector.GLOBALPAYMENTS_REALEX,
    environment: Environment.SANDBOX,
    auth: {
        globalpaymentsRealex: {
            sharedSecret: { value: 'YOUR_SHARED_SECRET' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            account: { value: 'YOUR_ACCOUNT' },
            refundPassword: { value: 'YOUR_REFUND_PASSWORD' },
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
            .setGlobalpaymentsRealex(GlobalpaymentsRealexConfig.newBuilder()
                .setSharedSecret(SecretString.newBuilder().setValue("YOUR_SHARED_SECRET").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setAccount(SecretString.newBuilder().setValue("YOUR_ACCOUNT").build())
                .setRefundPassword(SecretString.newBuilder().setValue("YOUR_REFUND_PASSWORD").build())
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

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py#L103) · [JavaScript](../../examples/globalpayments_realex/globalpayments_realex.js) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L101) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs#L127)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py#L122) · [JavaScript](../../examples/globalpayments_realex/globalpayments_realex.js) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L117) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs#L143)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py#L147) · [JavaScript](../../examples/globalpayments_realex/globalpayments_realex.js) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L139) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs#L166)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py#L169) · [JavaScript](../../examples/globalpayments_realex/globalpayments_realex.js) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L158) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs#L185)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
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
| Paze | ⚠ |
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
| PIS | ⚠ |
| Generic | ⚠ |
| WebPay | ⚠ |
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
| DuitNow | ⚠ |
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
| Crypto | ⚠ |
| Reward | ⚠ |
| Givex | ⚠ |
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

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py) · [TypeScript](../../examples/globalpayments_realex/globalpayments_realex.ts#L200) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L176) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py) · [TypeScript](../../examples/globalpayments_realex/globalpayments_realex.ts#L209) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L188) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py) · [TypeScript](../../examples/globalpayments_realex/globalpayments_realex.ts#L218) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L198) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py) · [TypeScript](../../examples/globalpayments_realex/globalpayments_realex.ts#L227) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L206) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/globalpayments_realex/globalpayments_realex.py) · [TypeScript](../../examples/globalpayments_realex/globalpayments_realex.ts) · [Kotlin](../../examples/globalpayments_realex/globalpayments_realex.kt#L235) · [Rust](../../examples/globalpayments_realex/globalpayments_realex.rs)
