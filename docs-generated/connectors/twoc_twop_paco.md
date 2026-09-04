# Twoc Twop Paco

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/twoc_twop_paco.json
Regenerate: python3 scripts/generators/docs/generate.py twoc_twop_paco
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_ACCESS_TOKEN`, `YOUR_OFFICE_ID`, `YOUR_PACO_KID`, `YOUR_MERCHANT_SIGNING_PRIVATE_KEY`, `YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY`, `YOUR_PACO_SIGNING_PUBLIC_KEY`, `YOUR_PACO_ENCRYPTION_PUBLIC_KEY`, `YOUR_RESPONSE_AUDIENCE`, `YOUR_BASE_URL` with your actual values.

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
        twoc_twop_paco=payment_pb2.TwocTwopPacoConfig(
            access_token=payment_methods_pb2.SecretString(value="YOUR_ACCESS_TOKEN"),
            office_id=payment_methods_pb2.SecretString(value="YOUR_OFFICE_ID"),
            paco_kid=payment_methods_pb2.SecretString(value="YOUR_PACO_KID"),
            merchant_signing_private_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_SIGNING_PRIVATE_KEY"),
            merchant_encryption_private_key=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY"),
            paco_signing_public_key=payment_methods_pb2.SecretString(value="YOUR_PACO_SIGNING_PUBLIC_KEY"),
            paco_encryption_public_key=payment_methods_pb2.SecretString(value="YOUR_PACO_ENCRYPTION_PUBLIC_KEY"),
            response_audience=payment_methods_pb2.SecretString(value="YOUR_RESPONSE_AUDIENCE"),
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
    connector: Connector.TWOC_TWOP_PACO,
    environment: Environment.SANDBOX,
    auth: {
        twocTwopPaco: {
            accessToken: { value: 'YOUR_ACCESS_TOKEN' },
            officeId: { value: 'YOUR_OFFICE_ID' },
            pacoKid: { value: 'YOUR_PACO_KID' },
            merchantSigningPrivateKey: { value: 'YOUR_MERCHANT_SIGNING_PRIVATE_KEY' },
            merchantEncryptionPrivateKey: { value: 'YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY' },
            pacoSigningPublicKey: { value: 'YOUR_PACO_SIGNING_PUBLIC_KEY' },
            pacoEncryptionPublicKey: { value: 'YOUR_PACO_ENCRYPTION_PUBLIC_KEY' },
            responseAudience: { value: 'YOUR_RESPONSE_AUDIENCE' },
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
            .setTwocTwopPaco(TwocTwopPacoConfig.newBuilder()
                .setAccessToken(SecretString.newBuilder().setValue("YOUR_ACCESS_TOKEN").build())
                .setOfficeId(SecretString.newBuilder().setValue("YOUR_OFFICE_ID").build())
                .setPacoKid(SecretString.newBuilder().setValue("YOUR_PACO_KID").build())
                .setMerchantSigningPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_SIGNING_PRIVATE_KEY").build())
                .setMerchantEncryptionPrivateKey(SecretString.newBuilder().setValue("YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY").build())
                .setPacoSigningPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_SIGNING_PUBLIC_KEY").build())
                .setPacoEncryptionPublicKey(SecretString.newBuilder().setValue("YOUR_PACO_ENCRYPTION_PUBLIC_KEY").build())
                .setResponseAudience(SecretString.newBuilder().setValue("YOUR_RESPONSE_AUDIENCE").build())
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
            config: Some(connector_specific_config::Config::TwocTwopPaco(TwocTwopPacoConfig {
                access_token: Some(hyperswitch_masking::Secret::new("YOUR_ACCESS_TOKEN".to_string())),  // Authentication credential
                office_id: Some(hyperswitch_masking::Secret::new("YOUR_OFFICE_ID".to_string())),  // Authentication credential
                paco_kid: Some(hyperswitch_masking::Secret::new("YOUR_PACO_KID".to_string())),  // Authentication credential
                merchant_signing_private_key: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_SIGNING_PRIVATE_KEY".to_string())),  // Authentication credential
                merchant_encryption_private_key: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY".to_string())),  // Authentication credential
                paco_signing_public_key: Some(hyperswitch_masking::Secret::new("YOUR_PACO_SIGNING_PUBLIC_KEY".to_string())),  // Authentication credential
                paco_encryption_public_key: Some(hyperswitch_masking::Secret::new("YOUR_PACO_ENCRYPTION_PUBLIC_KEY".to_string())),  // Authentication credential
                response_audience: Some(hyperswitch_masking::Secret::new("YOUR_RESPONSE_AUDIENCE".to_string())),  // Authentication credential
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py#L100) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L120) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L130) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs#L138)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py#L119) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L139) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L146) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs#L154)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py#L144) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L165) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L168) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs#L177)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentMethodAuthenticationService.Authenticate](#paymentmethodauthenticationserviceauthenticate) | Authentication | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| [PaymentMethodAuthenticationService.PostAuthenticate](#paymentmethodauthenticationservicepostauthenticate) | Authentication | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |

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

**Payment method objects** — use these in the `payment_method` field of the Authorize request.

##### Card (Raw PAN)

```python
"payment_method": {
  "card": {
    "card_number": {
      "value": "4111111111111111"
    },
    "card_exp_month": {
      "value": "12"
    },
    "card_exp_year": {
      "value": "2027"
    },
    "card_cvc": {
      "value": "123"
    },
    "card_holder_name": {
      "value": "Test Customer"
    },
    "card_type": "credit"
  }
}
```

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L190) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L189) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L199) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L201) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L208) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L209) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L219) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L226) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L229) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L235) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L237) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L244) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L247) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

### Authentication

#### PaymentMethodAuthenticationService.Authenticate

Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServiceAuthenticateResponse` |

**Supported payment method types:**

| Payment Method | Supported |
|----------------|:---------:|
| Card | ✓ |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)

#### PaymentMethodAuthenticationService.PostAuthenticate

Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePostAuthenticateResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.ts#L253) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.kt#L257) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/twoc_twop_paco/twoc_twop_paco.rs)
