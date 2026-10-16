# Paypal

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paypal.json
Regenerate: python3 scripts/generators/docs/generate.py paypal
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
        paypal=payment_pb2.PaypalConfig(
            client_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_ID"),
            client_secret=payment_methods_pb2.SecretString(value="YOUR_CLIENT_SECRET"),
            payer_id=payment_methods_pb2.SecretString(value="YOUR_PAYER_ID"),
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
    connector: Connector.PAYPAL,
    environment: Environment.SANDBOX,
    auth: {
        paypal: {
            clientId: { value: 'YOUR_CLIENT_ID' },
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            payerId: { value: 'YOUR_PAYER_ID' },
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
            .setPaypal(PaypalConfig.newBuilder()
                .setClientId(SecretString.newBuilder().setValue("YOUR_CLIENT_ID").build())
                .setClientSecret(SecretString.newBuilder().setValue("YOUR_CLIENT_SECRET").build())
                .setPayerId(SecretString.newBuilder().setValue("YOUR_PAYER_ID").build())
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
            config: Some(connector_specific_config::Config::Paypal(PaypalConfig {
                client_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_ID".to_string())),  // Authentication credential
                client_secret: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_SECRET".to_string())),  // Authentication credential
                payer_id: Some(hyperswitch_masking::Secret::new("YOUR_PAYER_ID".to_string())),  // Authentication credential
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
| `AUTHORIZED` | Funds reserved — await confirmed capture before treating the payment as paid |
| `CHARGED` | Payment captured — funds collected; apply your fulfillment rules |
| `PENDING` | Payment processing — await webhook for final status before fulfilling |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/paypal/paypal.py#L423) · [JavaScript](../../examples/paypal/paypal.js) · [Kotlin](../../examples/paypal/paypal.kt#L199) · [Rust](../../examples/paypal/paypal.rs#L541)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/paypal/paypal.py#L442) · [JavaScript](../../examples/paypal/paypal.js) · [Kotlin](../../examples/paypal/paypal.kt#L215) · [Rust](../../examples/paypal/paypal.rs#L571)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/paypal/paypal.py#L467) · [JavaScript](../../examples/paypal/paypal.js) · [Kotlin](../../examples/paypal/paypal.kt#L237) · [Rust](../../examples/paypal/paypal.rs#L618)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/paypal/paypal.py#L492) · [JavaScript](../../examples/paypal/paypal.js) · [Kotlin](../../examples/paypal/paypal.kt#L259) · [Rust](../../examples/paypal/paypal.rs#L659)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/paypal/paypal.py#L514) · [JavaScript](../../examples/paypal/paypal.js) · [Kotlin](../../examples/paypal/paypal.kt#L278) · [Rust](../../examples/paypal/paypal.rs#L696)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentMethodAuthenticationService.Authenticate](#paymentmethodauthenticationserviceauthenticate) | Authentication | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentMethodAuthenticationService.PostAuthenticate](#paymentmethodauthenticationservicepostauthenticate) | Authentication | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
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
| Apple Pay | ✓ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | x |
| Google Pay | x |
| Google Pay Dec | ✓ |
| Google Pay SDK | x |
| PayPal SDK | ✓ |
| Amazon Pay | x |
| Cash App | x |
| PayPal | ✓ |
| WeChat Pay | x |
| Alipay | x |
| Revolut Pay | x |
| MiFinity | x |
| Bluecode | x |
| Paze | ⚠ |
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
| PIS | ⚠ |
| Generic | x |
| WebPay | x |
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
| Crypto | ⚠ |
| Reward | x |
| Givex | ⚠ |
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

##### iDEAL

```python
"payment_method": {
  "ideal": {}
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

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L576) · [Kotlin](../../examples/paypal/paypal.kt#L340) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L585) · [Kotlin](../../examples/paypal/paypal.kt#L352) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L603) · [Kotlin](../../examples/paypal/paypal.kt#L379) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L621) · [Kotlin](../../examples/paypal/paypal.kt#L411) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L657) · [Kotlin](../../examples/paypal/paypal.kt#L484) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L675) · [Kotlin](../../examples/paypal/paypal.kt#L583) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L693) · [Kotlin](../../examples/paypal/paypal.kt#L612) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts) · [Kotlin](../../examples/paypal/paypal.kt#L659) · [Rust](../../examples/paypal/paypal.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L684) · [Kotlin](../../examples/paypal/paypal.kt#L593) · [Rust](../../examples/paypal/paypal.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L666) · [Kotlin](../../examples/paypal/paypal.kt#L521) · [Rust](../../examples/paypal/paypal.rs)

### Authentication

#### PaymentMethodAuthenticationService.Authenticate

Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServiceAuthenticateResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L567) · [Kotlin](../../examples/paypal/paypal.kt#L296) · [Rust](../../examples/paypal/paypal.rs)

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L594) · [Kotlin](../../examples/paypal/paypal.kt#L362) · [Rust](../../examples/paypal/paypal.rs)

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L612) · [Kotlin](../../examples/paypal/paypal.kt#L401) · [Rust](../../examples/paypal/paypal.rs)

#### PaymentMethodAuthenticationService.PostAuthenticate

Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePostAuthenticateResponse` |

**Examples:** [Python](../../examples/paypal/paypal.py) · [TypeScript](../../examples/paypal/paypal.ts#L648) · [Kotlin](../../examples/paypal/paypal.kt#L450) · [Rust](../../examples/paypal/paypal.rs)
