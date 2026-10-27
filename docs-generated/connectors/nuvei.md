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
from payments.generated import sdk_config_pb2, payment_pb2, events_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        nuvei=payment_pb2.NuveiConfig(
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            merchant_site_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_SITE_ID"),
            merchant_secret=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_SECRET"),
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
    connector: Connector.NUVEI,
    environment: Environment.SANDBOX,
    auth: {
        nuvei: {
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            merchantSiteId: { value: 'YOUR_MERCHANT_SITE_ID' },
            merchantSecret: { value: 'YOUR_MERCHANT_SECRET' },
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
            .setNuvei(NuveiConfig.newBuilder()
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setMerchantSiteId(SecretString.newBuilder().setValue("YOUR_MERCHANT_SITE_ID").build())
                .setMerchantSecret(SecretString.newBuilder().setValue("YOUR_MERCHANT_SECRET").build())
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
            config: Some(connector_specific_config::Config::Nuvei(NuveiConfig {
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                merchant_site_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_SITE_ID".to_string())),  // Authentication credential
                merchant_secret: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_SECRET".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/nuvei/nuvei.py#L272) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L132) · [Rust](../../examples/nuvei/nuvei.rs#L334)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/nuvei/nuvei.py#L291) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L148) · [Rust](../../examples/nuvei/nuvei.rs#L350)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/nuvei/nuvei.py#L316) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L170) · [Rust](../../examples/nuvei/nuvei.rs#L373)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/nuvei/nuvei.py#L341) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L192) · [Rust](../../examples/nuvei/nuvei.rs#L396)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/nuvei/nuvei.py#L363) · [JavaScript](../../examples/nuvei/nuvei.js) · [Kotlin](../../examples/nuvei/nuvei.kt#L211) · [Rust](../../examples/nuvei/nuvei.rs#L415)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [MerchantAuthenticationService.CreateServerSessionAuthenticationToken](#merchantauthenticationservicecreateserversessionauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
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
| Thailand | x |
| Czech | x |
| Finland | x |
| FPX | x |
| Poland | x |
| Slovakia | x |
| UK | x |
| PIS | ⚠ |
| Generic | x |
| WebPay | ⚠ |
| Local | x |
| iDEAL | ✓ |
| Sofort | ✓ |
| Trustly | x |
| Giropay | ✓ |
| EPS | ✓ |
| Przelewy24 | x |
| PSE | x |
| BLIK | x |
| Interac | x |
| Bizum | x |
| EFT | x |
| DuitNow | ⚠ |
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
| ACH | ✓ |
| SEPA | x |
| BACS | x |
| BECS | x |
| SEPA Guaranteed | x |
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

##### ACH Direct Debit

```python
"payment_method": {
  "ach": {
    "account_number": "000123456789",
    "routing_number": "110000000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### iDEAL

```python
"payment_method": {
  "ideal": {}
}
```

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L408) · [Kotlin](../../examples/nuvei/nuvei.kt#L229) · [Rust](../../examples/nuvei/nuvei.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L417) · [Kotlin](../../examples/nuvei/nuvei.kt#L241) · [Rust](../../examples/nuvei/nuvei.rs)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L435) · [Kotlin](../../examples/nuvei/nuvei.kt#L267) · [Rust](../../examples/nuvei/nuvei.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L453) · [Kotlin](../../examples/nuvei/nuvei.kt#L296) · [Rust](../../examples/nuvei/nuvei.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L498) · [Kotlin](../../examples/nuvei/nuvei.kt#L428) · [Rust](../../examples/nuvei/nuvei.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L516) · [Kotlin](../../examples/nuvei/nuvei.kt#L450) · [Rust](../../examples/nuvei/nuvei.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts) · [Kotlin](../../examples/nuvei/nuvei.kt#L498) · [Rust](../../examples/nuvei/nuvei.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L507) · [Kotlin](../../examples/nuvei/nuvei.kt#L438) · [Rust](../../examples/nuvei/nuvei.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L489) · [Kotlin](../../examples/nuvei/nuvei.kt#L377) · [Rust](../../examples/nuvei/nuvei.rs)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L426) · [Kotlin](../../examples/nuvei/nuvei.kt#L251) · [Rust](../../examples/nuvei/nuvei.rs)

#### MerchantAuthenticationService.CreateServerSessionAuthenticationToken

Create a server-side session with the connector. Establishes session state for multi-step operations like 3DS verification or wallet authorization.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L444) · [Kotlin](../../examples/nuvei/nuvei.kt#L281) · [Rust](../../examples/nuvei/nuvei.rs)

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/nuvei/nuvei.py) · [TypeScript](../../examples/nuvei/nuvei.ts#L480) · [Kotlin](../../examples/nuvei/nuvei.kt#L335) · [Rust](../../examples/nuvei/nuvei.rs)
