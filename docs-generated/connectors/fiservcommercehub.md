# Fiservcommercehub

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiservcommercehub.json
Regenerate: python3 scripts/generators/docs/generate.py fiservcommercehub
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
        fiservcommercehub=payment_pb2.FiservcommercehubConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            secret=payment_methods_pb2.SecretString(value="YOUR_SECRET"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
            terminal_id=payment_methods_pb2.SecretString(value="YOUR_TERMINAL_ID"),
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
    connector: Connector.FISERVCOMMERCEHUB,
    environment: Environment.SANDBOX,
    auth: {
        fiservcommercehub: {
            apiKey: { value: 'YOUR_API_KEY' },
            secret: { value: 'YOUR_SECRET' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            terminalId: { value: 'YOUR_TERMINAL_ID' },
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
            .setFiservcommercehub(FiservcommercehubConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setSecret(SecretString.newBuilder().setValue("YOUR_SECRET").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
                .setTerminalId(SecretString.newBuilder().setValue("YOUR_TERMINAL_ID").build())
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
            config: Some(connector_specific_config::Config::Fiservcommercehub(FiservcommercehubConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                secret: Some(hyperswitch_masking::Secret::new("YOUR_SECRET".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                terminal_id: Some(hyperswitch_masking::Secret::new("YOUR_TERMINAL_ID".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L408) · [JavaScript](../../examples/fiservcommercehub/fiservcommercehub.js) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L171) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L505)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L427) · [JavaScript](../../examples/fiservcommercehub/fiservcommercehub.js) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L187) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L521)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L452) · [JavaScript](../../examples/fiservcommercehub/fiservcommercehub.js) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L209) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L544)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L477) · [JavaScript](../../examples/fiservcommercehub/fiservcommercehub.js) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L231) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L567)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L499) · [JavaScript](../../examples/fiservcommercehub/fiservcommercehub.js) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L250) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L586)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentMethodAuthenticationService.Authenticate](#paymentmethodauthenticationserviceauthenticate) | Authentication | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.IncrementalAuthorization](#paymentserviceincrementalauthorization) | Payments | `PaymentServiceIncrementalAuthorizationRequest` |
| [PaymentMethodAuthenticationService.PostAuthenticate](#paymentmethodauthenticationservicepostauthenticate) | Authentication | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
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
| Bancontact | ⚠ |
| Apple Pay | ⚠ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | ⚠ |
| Google Pay | ⚠ |
| Google Pay Dec | ✓ |
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

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L550) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L309) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L559) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L321) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L577) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L341) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.IncrementalAuthorization

Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceIncrementalAuthorizationRequest` |
| **Response** | `PaymentServiceIncrementalAuthorizationResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L586) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L349) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L613) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L448) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L622) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L484) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L640) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L561) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L658) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L590) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L667) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L598) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L644) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L649) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L571) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L631) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L523) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

### Authentication

#### PaymentMethodAuthenticationService.Authenticate

Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServiceAuthenticateResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L541) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L268) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L568) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L331) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentMethodAuthenticationService.PostAuthenticate

Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePostAuthenticateResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L595) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L376) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L604) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L413) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs)
