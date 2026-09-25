# Shift4

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/shift4.json
Regenerate: python3 scripts/generators/docs/generate.py shift4
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
        shift4=payment_pb2.Shift4Config(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
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
    connector: Connector.SHIFT4,
    environment: Environment.SANDBOX,
    auth: {
        shift4: {
            apiKey: { value: 'YOUR_API_KEY' },
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
            .setShift4(Shift4Config.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
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
            config: Some(connector_specific_config::Config::Shift4(Shift4Config {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
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

**Examples:** [Python](../../examples/shift4/shift4.py#L324) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L121) · [Rust](../../examples/shift4/shift4.rs#L412)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/shift4/shift4.py#L343) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L137) · [Rust](../../examples/shift4/shift4.rs#L428)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/shift4/shift4.py#L368) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L159) · [Rust](../../examples/shift4/shift4.rs#L451)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/shift4/shift4.py#L393) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L181) · [Rust](../../examples/shift4/shift4.rs#L474)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/shift4/shift4.py#L415) · [JavaScript](../../examples/shift4/shift4.js) · [Kotlin](../../examples/shift4/shift4.kt#L200) · [Rust](../../examples/shift4/shift4.rs#L493)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [PaymentService.IncrementalAuthorization](#paymentserviceincrementalauthorization) | Payments | `PaymentServiceIncrementalAuthorizationRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
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
| Bancontact | x |
| Apple Pay | ✓ |
| Apple Pay Dec | x |
| Apple Pay SDK | x |
| Google Pay | ✓ |
| Google Pay Dec | x |
| Google Pay SDK | x |
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
| Sofort | x |
| Trustly | x |
| Giropay | x |
| EPS | ✓ |
| Przelewy24 | x |
| PSE | ⚠ |
| BLIK | x |
| Interac | x |
| Bizum | x |
| EFT | x |
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

##### Google Pay

```python
"payment_method": {
  "google_pay_sdk": {
    "type": "CARD",
    "description": "Visa 1111",
    "info": {
      "card_network": "VISA",
      "card_details": "1111"
    },
    "tokenization_data": {
      "encrypted_data": {
        "token_type": "PAYMENT_GATEWAY",
        "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"
      }
    }
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

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L469) · [Kotlin](../../examples/shift4/shift4.kt#L218) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L478) · [Kotlin](../../examples/shift4/shift4.kt#L230) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L505) · [Kotlin](../../examples/shift4/shift4.kt#L269) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.IncrementalAuthorization

Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceIncrementalAuthorizationRequest` |
| **Response** | `PaymentServiceIncrementalAuthorizationResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L523) · [Kotlin](../../examples/shift4/shift4.kt#L293) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L550) · [Kotlin](../../examples/shift4/shift4.kt#L353) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L559) · [Kotlin](../../examples/shift4/shift4.kt#L382) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L577) · [Kotlin](../../examples/shift4/shift4.kt#L448) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L595) · [Kotlin](../../examples/shift4/shift4.kt#L470) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L604) · [Kotlin](../../examples/shift4/shift4.kt#L512) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.TokenSetupRecurring

Setup a recurring mandate using a connector token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L613) · [Kotlin](../../examples/shift4/shift4.kt#L533) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts) · [Kotlin](../../examples/shift4/shift4.kt#L576) · [Rust](../../examples/shift4/shift4.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L586) · [Kotlin](../../examples/shift4/shift4.kt#L458) · [Rust](../../examples/shift4/shift4.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L568) · [Kotlin](../../examples/shift4/shift4.kt#L417) · [Rust](../../examples/shift4/shift4.rs)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L496) · [Kotlin](../../examples/shift4/shift4.kt#L256) · [Rust](../../examples/shift4/shift4.rs)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L487) · [Kotlin](../../examples/shift4/shift4.kt#L240) · [Rust](../../examples/shift4/shift4.rs)

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/shift4/shift4.py) · [TypeScript](../../examples/shift4/shift4.ts#L541) · [Kotlin](../../examples/shift4/shift4.kt#L324) · [Rust](../../examples/shift4/shift4.rs)
